use std::{borrow::Cow, str::FromStr};

use anyhow::{Context as _, Result};
use swiftide::{
    chat_completion::{errors::ToolError, ToolOutput},
    traits::{AgentContext, Command},
};
use swiftide_macros::tool;

use crate::util::accept_non_zero_exit;

// TODO:
// - Fix hunk header parsing
// - Check if line numbers match?
// - Handle ambigious

const REPLACE_PATCH_DESCRIPTION: &str = "Replace content with a Unified format git patch

Here is an example of a Unified format git patch:

```patch
--- a/src/evaluations/patch.rs
+++ b/src/evaluations/patch.rs
@@ -43,7 +43,7 @@ fn prompt() -> String {
             self._content_consumed = True
         ```
 
-        Apply only these fixes, do not make any other changes to the code. The file is long and the modifications are small.
+        Apply only these fixes, do not make any other changes to the code. The file is long and the modifications are small. Start by reading the file.
     \"}.to_string()
 }
 
-- 
```
";

#[tool(
    description = REPLACE_PATCH_DESCRIPTION,
    param(name = "file_name", description = "Full path of the file"),
    param(name = "patch", description = "Unified format git patch to apply"),
)]
async fn patch_file(
    context: &dyn AgentContext,
    file_name: &str,
    patch: &str,
) -> Result<ToolOutput, ToolError> {
    let cmd = Command::ReadFile(file_name.into());
    let old_content = accept_non_zero_exit(context.exec_cmd(&cmd).await)?.output;

    // let patch = fix_hunk_headers(&content, &patch);

    let patch = match diffy::Patch::from_str(patch) {
        Ok(patch) => patch,
        Err(err) => {
            return Ok(ToolOutput::Fail(format!("Failed to parse patch: {err}")));
        }
    };
    let patched = diffy::apply(&old_content, &patch).context("Failed to apply patch")?;

    let cmd = Command::WriteFile(file_name.into(), patched);
    context.exec_cmd(&cmd).await?;

    Ok(ToolOutput::Text("Patch applied successfully".into()))
}

// llms are dumb and cannot count
//
// However, with a patch we can reasonably fix the headers
// by searching in the neighboring lines of the original hunk header
fn find_candidates<'a>(content: &str, hunks: &'a [Hunk]) -> Vec<Candidate<'a>> {
    let mut candidates = Vec::new();

    for (line_n, line) in content.lines().enumerate() {
        // 1. Check if a hunk matches the line, then create a candidate if it does
        if let Some(hunk) = hunks.iter().find(|h| h.matches(line, 0, false)) {
            tracing::warn!(line, "Found hunk match; creating new candidate");
            candidates.push(Candidate::new(line_n, hunk));
        }

        // 2. For each active candidate, check if the next line matches. If it does, increment the
        // the index of the candidate. Otherwise, remove the candidate
        candidates.retain_mut(|c| {
            if c.is_complete() {
                tracing::warn!("Candidate already completed");
                true
            } else if c.next_line_matches(line) {
                tracing::warn!(line, "Candidate matched line");
                c.current_line += 1;
                true
            } else {
                tracing::warn!(line, "Removing candidate");
                false
            }
        });
    }

    candidates
}

/// Takes a list of candidates and rebuits the hunk headers
fn rebuild_hunks(candidates: &[Candidate<'_>]) -> Vec<Hunk> {
    // Assume that the candidates are sorted by the start line
    // Then we can just iterate over the candidates and update the ranges
    //
    // TODO: Deal with duplicated candidates

    let mut current_offset: isize = 0;
    let mut hunks = Vec::new();

    for candidate in candidates {
        let source_header = candidate.updated_source_header();

        let dest_header = candidate.updated_dest_header(current_offset);
        current_offset += candidate.offset();

        let mut hunk = candidate.hunk.clone();
        hunk.header.fixed_source_range = Some(source_header);
        hunk.header.fixed_dest_range = Some(dest_header);
        hunks.push(hunk);
    }

    hunks
}

/// Splits the patch into a tuple of the hunk header with the full hunk (including the header)
fn parse_hunks(patch: &str) -> Result<Vec<Hunk>> {
    let mut hunks = Vec::new();
    let mut current_hunk_lines = Vec::new();

    for line in patch.lines() {
        if line.starts_with("@@") {
            if !current_hunk_lines.is_empty() {
                let hunk = Hunk::from_str(&current_hunk_lines.join("\n"))?;
                hunks.push(hunk);
            }

            current_hunk_lines = vec![line];
        } else if !current_hunk_lines.is_empty() {
            current_hunk_lines.push(line);
        }
    }

    if !current_hunk_lines.is_empty() {
        let hunk = Hunk::from_str(&current_hunk_lines.join("\n"))?;
        hunks.push(hunk);
    }

    Ok(hunks)
}

#[derive(Clone, Debug)]
struct HeaderRange {
    /// The line number the patch starts at
    start: usize,
    /// The line numbers visible for the patch
    range: usize,
}

#[derive(Clone, Debug)]
struct HunkHeader {
    source_range: HeaderRange,
    dest_range: HeaderRange,

    // Optional values after fixing the ranges
    fixed_source_range: Option<HeaderRange>,
    fixed_dest_range: Option<HeaderRange>,
}

#[derive(Clone, Debug, strum_macros::EnumIs)]
enum HunkLine {
    Context(String),
    Added(String),
    Removed(String),
}

impl HunkLine {
    pub fn content(&self) -> &str {
        match self {
            HunkLine::Context(s) => s,
            HunkLine::Added(s) => s,
            HunkLine::Removed(s) => s,
        }
    }

    pub fn as_patch_line(&self) -> Cow<str> {
        match self {
            HunkLine::Context(s) => Cow::Borrowed(s),
            HunkLine::Added(s) => Cow::Owned(format!("+{}", s)),
            HunkLine::Removed(s) => Cow::Owned(format!("-{}", s)),
        }
    }
}

#[derive(Clone, Debug)]
struct Hunk {
    /// The parsed header of the hunk
    header: HunkHeader,

    /// Parsed lines of the hunk
    lines: Vec<HunkLine>,

    /// The full hunk body
    body: String,
}

impl Hunk {
    fn matchable_lines(&self) -> impl Iterator<Item = &HunkLine> {
        self.lines
            .iter()
            .filter(|l| l.is_removed() || l.is_context())
    }

    pub fn matches(&self, line: &str, index: usize, log: bool) -> bool {
        let expected = self
            .matchable_lines()
            .skip(index)
            .map(HunkLine::content)
            .next();

        let outcome = expected.map(str::trim) == Some(line.trim());

        if log {
            if outcome {
                tracing::warn!(line, expected, "Matched line");
            } else {
                tracing::debug!(line, expected, "Did not match line");
            }
        }
        outcome
    }

    pub fn render_updated(&self) -> Result<String> {
        // Extract any context after the second @@ block to add to the new header line
        // i.e. with `@@ -1,2 +2,1 @@ my_function()` we want my_function() to be included
        let header_context = self
            .body
            .lines()
            .next()
            .unwrap_or_default()
            .rsplit("@@")
            .next()
            .unwrap_or_default();

        let source = self
            .header
            .fixed_source_range
            .as_ref()
            .context("Expected")?;
        let dest = self.header.fixed_dest_range.as_ref().context("Expected")?;

        let mut updated = format!(
            "@@ -{},{} +{},{} @@{header_context}\n",
            source.start + 1,
            source.range,
            dest.start + 1,
            dest.range
        );

        for line in &self.lines {
            updated.push_str(&line.as_patch_line());
            updated.push('\n');
        }

        Ok(updated)
    }
}

/// A hunk that is found in a file
#[derive(Clone, Debug)]
struct Candidate<'a> {
    /// The line number in the file we started at
    start: usize,

    /// The current line we are matchin against
    current_line: usize,

    hunk: &'a Hunk,
}

impl<'a> Candidate<'a> {
    pub fn new(line: usize, hunk: &'a Hunk) -> Self {
        Self {
            start: line,
            current_line: 0,
            hunk,
        }
    }

    /// Number difference in visible lines between the source and destination for the next hunk
    ///
    /// If lines were added, the following hunk will start at an increased line number, if lines
    /// were removed, the following hunk will start at a decreased line number
    pub fn offset(&self) -> isize {
        self.hunk.lines.iter().filter(|l| l.is_added()).count() as isize
            - self.hunk.lines.iter().filter(|l| l.is_removed()).count() as isize
    }

    pub fn next_line_matches(&self, line: &str) -> bool {
        self.hunk.matches(line, self.current_line, true)
    }

    pub fn is_complete(&self) -> bool {
        // We increment one over the current line, so if we are at the end of the hunk, we are done
        self.current_line == self.hunk.matchable_lines().count()
    }

    pub fn updated_source_header(&self) -> HeaderRange {
        let source_lines = self
            .hunk
            .lines
            .iter()
            .filter(|l| l.is_removed() || l.is_context())
            .count();

        let source_start = self.start;

        HeaderRange {
            start: source_start,
            range: source_lines,
        }
    }

    pub fn updated_dest_header(&self, offset: isize) -> HeaderRange {
        let dest_lines = self
            .hunk
            .lines
            .iter()
            .filter(|l| l.is_added() || l.is_context())
            .count();

        // The offset is the sum off removed and added lines by preceding hunks
        let dest_start = self.start.saturating_add_signed(offset);

        HeaderRange {
            start: dest_start,
            range: dest_lines,
        }
    }
}

impl FromStr for Hunk {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let header: HunkHeader = s.parse()?;
        let lines = s
            .lines()
            .skip(1)
            .map(FromStr::from_str)
            .collect::<Result<Vec<HunkLine>>>()?;

        Ok(Hunk {
            header,
            lines,
            body: s.into(),
        })
    }
}

impl FromStr for HunkLine {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(line) = s.strip_prefix('+') {
            Ok(HunkLine::Added(line.into()))
        } else if let Some(line) = s.strip_prefix('-') {
            Ok(HunkLine::Removed(line.into()))
        } else {
            Ok(HunkLine::Context(s.into()))
        }
    }
}

// For the header we just parse, as there is nothing to borrow
impl std::str::FromStr for HunkHeader {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("@@") {
            anyhow::bail!("Hunk header must start with @@");
        }

        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 4 {
            anyhow::bail!("Invalid hunk header format");
        }

        let old_range = parts[1].split(',').collect::<Vec<&str>>();
        let new_range = parts[2].split(',').collect::<Vec<&str>>();

        if old_range.len() != 2 || new_range.len() != 2 {
            anyhow::bail!("Invalid range format in hunk header");
        }

        let old_lines = HeaderRange {
            start: old_range[0]
                .replace('-', "")
                .parse()
                .context("Invalid old start line")?,
            range: old_range[1].parse().context("Invalid old range")?,
        };

        let new_lines = HeaderRange {
            start: new_range[0]
                .replace('+', "")
                .parse()
                .context("Invalid new start line")?,
            range: new_range[1].parse().context("Invalid new range")?,
        };

        Ok(HunkHeader {
            source_range: old_lines,
            dest_range: new_lines,
            fixed_source_range: None,
            fixed_dest_range: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BAD_SINGLE_HUNK: &str = indoc::indoc! {"--- a/src/evaluations/fixtures/swebench_2148/models.py
+++ b/src/evaluations/fixtures/swebench_2148/models.py
@@ -637,6 +637,7 @@ def iter_content(self, chunk_size=1, decode_unicode=False):
                 except IncompleteRead as e:
                     raise ChunkedEncodingError(e)
                 except DecodeError as e:
                     raise ContentDecodingError(e)
+                except socket.error as e:
+                    raise ConnectionError(e)
             except AttributeError:
                 # Standard file-like object.
                 while True:
"};
    const BAD_PATCH: &str = indoc::indoc! {"--- a/src/evaluations/fixtures/swebench_2148/models.py
+++ b/src/evaluations/fixtures/swebench_2148/models.py
@@ -637,6 +637,7 @@ def iter_content(self, chunk_size=1, decode_unicode=False):
                 except IncompleteRead as e:
                     raise ChunkedEncodingError(e)
                 except DecodeError as e:
                     raise ContentDecodingError(e)
+                except socket.error as e:
+                    raise ConnectionError(e)
             except AttributeError:
                 # Standard file-like object.
                 while True:
@@ -652,8 +653,9 @@ def iter_content(self, chunk_size=1, decode_unicode=False):
                     yield chunk
 
-            self._content_consumed = True
+            
+            
 
+        
         # simulate reading small chunks of the content
         reused_chunks = iter_slices(self._content, chunk_size)
         
@@ -664,6 +666,8 @@ def iter_content(self, chunk_size=1, decode_unicode=False):
 
         if decode_unicode:
             chunks = stream_decode_response_unicode(chunks, self)
+
+        finally:
+            self._content_consumed = True
 
         return chunks


"};

    #[test]
    fn test_split_patch_into_hunks() {
        // TODO: Add some more unit tests to check parsing is correct
        let hunks = parse_hunks(BAD_PATCH).unwrap();
        assert_eq!(hunks.len(), 3);

        let header = &hunks[0].header;

        assert_eq!(header.source_range.start, 637);
        assert_eq!(header.source_range.range, 6);

        assert_eq!(header.dest_range.start, 637);
        assert_eq!(header.dest_range.range, 7);

        let header = &hunks[1].header;
        assert_eq!(header.source_range.start, 652);
        assert_eq!(header.source_range.range, 8);

        assert_eq!(header.dest_range.start, 653);
        assert_eq!(header.dest_range.range, 9);

        let header = &hunks[2].header;

        assert_eq!(header.source_range.start, 664);
        assert_eq!(header.source_range.range, 6);

        assert_eq!(header.dest_range.start, 666);
        assert_eq!(header.dest_range.range, 8);
    }

    #[test_log::test]
    fn test_find_candidates_single_hunk() {
        let hunks = parse_hunks(&BAD_SINGLE_HUNK).unwrap();
        assert_eq!(hunks.len(), 1);
        let content = std::fs::read_to_string("src/evaluations/fixtures/swebench_2148/models.py")
            .expect("Failed to read file");
        let candidates = find_candidates(&content, &hunks);
        dbg!(&candidates);
        assert_eq!(candidates.len(), 1);

        let hunk = rebuild_hunks(&candidates).first().unwrap().clone();

        dbg!(&hunk);

        assert_eq!(hunk.header.fixed_source_range.as_ref().unwrap().start, 641); // One less than
                                                                                 // in the source file
        assert_eq!(hunk.header.fixed_source_range.as_ref().unwrap().range, 7);
        assert_eq!(hunk.header.fixed_dest_range.as_ref().unwrap().start, 641);
        assert_eq!(hunk.header.fixed_dest_range.as_ref().unwrap().range, 9);
        assert_eq!(candidates.first().unwrap().offset(), 2);

        insta::assert_snapshot!(hunk.render_updated().unwrap());
    }

    #[test_log::test]
    fn test_find_candidates_multiple_hunks() {
        let hunks = parse_hunks(&BAD_PATCH).unwrap();
        let content = std::fs::read_to_string("src/evaluations/fixtures/swebench_2148/models.py")
            .expect("Failed to read file");

        let adjusted_hunks = find_candidates(&content, &hunks);
        assert_eq!(adjusted_hunks.len(), hunks.len());
    }

    // #[test]
    // fn test_fix_hunk_header() {
    //     diffy::Patch::from_str(BAD_PATCH).unwrap();
    // }
}
