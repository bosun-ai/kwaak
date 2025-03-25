use std::{borrow::Cow, str::FromStr};

use anyhow::{Context as _, Result};
use diffy::Patch;
use swiftide::{
    chat_completion::{errors::ToolError, ToolOutput},
    traits::{AgentContext, Command},
};
use swiftide_macros::tool;

const REPLACE_PATCH_DESCRIPTION: &str = "Replace content with a Unified format git patch.

Use this tool to make multiple edits in a file.

Here is an example of a Unified format git patch:

```patch
--- a/src/evaluations/patch.rs
+++ b/src/evaluations/patch.rs
@@ -43,6 +43,6 @@ fn prompt() -> String {
             self._content_consumed = True
 
-        Apply only these fixes, do not make any other changes to the code. The file is long and the modifications are small.
+        Apply only these fixes, do not make any other changes to the code. The file is long and the modifications are small. Start by reading the file.
     \"}.to_string()
 }
 
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
    let mut old_content = context
        .exec_cmd(&cmd)
        .await
        .with_context(|| format!("Failed to read file {file_name}"))?
        .output;

    // Patches are very strict on the last line being a newline
    if !old_content.ends_with('\n') {
        old_content.push('\n');
    }

    let old_hunks = parse_hunks(&patch).context("Failed to parse patch")?;
    let candidates = find_candidates(&old_content, &old_hunks);
    let new_hunks = rebuild_hunks(&candidates);

    let updated_patch =
        rebuild_patch(&patch, &new_hunks).context("Failed to render fixed patch")?;
    let diffy_patch = Patch::from_str(&updated_patch).context("Failed to parse patch")?;

    tracing::debug!(file_name, input_patch = %patch, %updated_patch, "Applying patch");

    let patched = match diffy::apply(&old_content, &diffy_patch) {
        Ok(patched) => patched,
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to apply patch: {e:#}").into());
        }
    };
    let cmd = Command::WriteFile(file_name.into(), patched);
    let output = context.exec_cmd(&cmd).await?;

    tracing::debug!(output = ?output, "Patch applied");

    if new_hunks.len() != old_hunks.len() {
        let failed = old_hunks
            .iter()
            .filter(|h| !new_hunks.iter().any(|h2| h2.body == h.body))
            .collect::<Vec<_>>();

        return Ok(ToolOutput::Fail(indoc::formatdoc! {"
            Failed to apply all hunks. {failed_len} hunks failed to apply.

            The following hunks failed to apply as their context lines could not be matched to the file, other hunks were applied successfully:

            ---
            {failed}
            ---

            Make sure all lines are correct. Are you also sure that the changes have not been applied already?
            ",
        failed_len = failed.len(),
        failed = failed.iter().map(|h| h.body.as_str()).collect::<Vec<_>>().join("\n")
        }));
    }

    Ok("Patch applied successfully".into())
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
            tracing::trace!(line, "Found hunk match; creating new candidate");
            candidates.push(Candidate::new(line_n, hunk));
        }

        // 2. For each active candidate, check if the next line matches. If it does, increment the
        // the index of the candidate. Otherwise, remove the candidate
        let mut new_candidates = Vec::new();
        candidates.retain_mut(|c| {
            if c.is_complete() {
                tracing::trace!("Candidate already completed");
                true
            } else if c.next_line_matches(line) {
                tracing::trace!(line, "Candidate matched line");
                c.current_line += 1;
                true
            } else if line.trim().is_empty() {
                tracing::trace!(line, "Current line is empty; keeping candidate around");
                // We create a new candidate with a whitespace line added at the index of this
                // candidate plus one. This helps with LLMs misjudging whitespace in the context
                let mut new_hunk: Hunk = c.hunk.clone().into_owned();
                new_hunk.lines.insert(
                    c.current_line.saturating_add(1),
                    HunkLine::Context(line.into()),
                );
                let mut new_candidate = Candidate::new(c.start, new_hunk);
                new_candidate.current_line = c.current_line + 1;

                new_candidates.push(new_candidate);
                false
            } else {
                tracing::trace!(line, "Removing candidate");
                false
            }
        });
        candidates.append(&mut new_candidates);
    }

    candidates
}

/// Takes a list of candidates and rebuits the hunk headers
///
/// Filters out duplicates. The resulting hunks should result in a valid patch.
fn rebuild_hunks(candidates: &[Candidate<'_>]) -> Vec<Hunk> {
    // Assume that the candidates are sorted by the start line
    // Then we can just iterate over the candidates and update the ranges

    let mut current_offset: isize = 0;
    // TODO: Should be a hashset on the body
    let mut hunks: Vec<Hunk> = Vec::new();

    for candidate in candidates {
        let source_header = candidate.updated_source_header();

        let dest_header = candidate.updated_dest_header(current_offset);
        current_offset += candidate.offset();

        // Could probably continue the cow, but at this point the number of hunks should be small
        let mut hunk = candidate.hunk.clone().into_owned();
        hunk.header.fixed_source = Some(source_header);
        hunk.header.fixed_dest = Some(dest_header);

        // Filter duplicates. A hunk is a duplicate if the hunk body is the same. If a duplicate
        // is detected, prefer the one with the fixed_source closest to the original source line
        // If so, we swap it with the existing hunk.

        if let Some(existing) = hunks.iter_mut().find(|h| *h.body == hunk.body) {
            let (Some(existing_source), Some(new_source)) =
                (&existing.header.fixed_source, &hunk.header.fixed_source)
            else {
                tracing::warn!("Potential bad duplicate when rebuilding patch; could be a bug, please check the edit");
                continue;
            };

            #[allow(clippy::cast_possible_wrap)]
            if ((existing_source.start as isize)
                .saturating_sub_unsigned(existing.header.source.start))
            .abs()
                < ((new_source.start as isize).saturating_sub_unsigned(hunk.header.source.start))
                    .abs()
            {
                continue;
            }
            *existing = hunk;
        } else {
            hunks.push(hunk);
        }
    }

    hunks
}

/// Takes the file lines from the original patch if possible, then rebuilds the patch
fn rebuild_patch(original: &str, hunks: &[Hunk]) -> Result<String> {
    let mut new_patch = original.lines().take(2).collect::<Vec<_>>().join("\n");
    new_patch.push('\n');

    debug_assert!(
        !new_patch.is_empty(),
        "Original file lines in patch tools are empty"
    );

    for hunk in hunks {
        new_patch.push_str(&hunk.render_updated()?);
    }

    Ok(new_patch)
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
    source: HeaderRange,
    #[allow(dead_code)]
    dest: HeaderRange,

    // Optional values after fixing the ranges
    fixed_source: Option<HeaderRange>,
    fixed_dest: Option<HeaderRange>,
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
            HunkLine::Removed(s) | HunkLine::Context(s) | HunkLine::Added(s) => s,
        }
    }

    pub fn as_patch_line(&self) -> Cow<str> {
        match self {
            HunkLine::Context(s) => {
                if s.starts_with(' ') {
                    Cow::Borrowed(s)
                } else {
                    Cow::Owned(format!(" {s}"))
                }
            }
            HunkLine::Added(s) => Cow::Owned(format!("+{s}")),
            HunkLine::Removed(s) => Cow::Owned(format!("-{s}")),
        }
    }
}

#[derive(Clone, Debug)]
struct Hunk {
    /// The parsed header of the hunk
    header: HunkHeader,

    /// Parsed lines of the hunk
    lines: Vec<HunkLine>,

    /// The original full hunk body
    body: String,
}

impl<'a> From<&'a Hunk> for Cow<'a, Hunk> {
    fn from(val: &'a Hunk) -> Self {
        Cow::Borrowed(val)
    }
}

impl From<Hunk> for Cow<'_, Hunk> {
    fn from(val: Hunk) -> Self {
        Cow::Owned(val)
    }
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
                tracing::trace!(line, expected, "Matched line");
            } else {
                tracing::trace!(line, expected, "Did not match line");
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
            .fixed_source
            .as_ref()
            .context("Expected updated source")?;
        let dest = self
            .header
            .fixed_dest
            .as_ref()
            .context("Expected updated dest")?;

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

        Ok(updated.trim().to_string())
    }
}

/// A hunk that is found in a file
#[derive(Clone, Debug)]
struct Candidate<'a> {
    /// The line number in the file we started at
    start: usize,

    /// The current line we are matching against
    current_line: usize,

    hunk: Cow<'a, Hunk>,
}

impl<'a> Candidate<'a> {
    pub fn new(line: usize, hunk: impl Into<Cow<'a, Hunk>>) -> Self {
        Self {
            start: line,
            current_line: 0,
            hunk: hunk.into(),
        }
    }

    /// Number difference in visible lines between the source and destination for the next hunk
    ///
    /// If lines were added, the following hunk will start at an increased line number, if lines
    /// were removed, the following hunk will start at a decreased line number
    #[allow(clippy::cast_possible_wrap)]
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

impl FromStr for HunkHeader {
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
            source: old_lines,
            dest: new_lines,
            fixed_source: None,
            fixed_dest: None,
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
        let hunks = parse_hunks(BAD_PATCH).unwrap();
        assert_eq!(hunks.len(), 3);

        let header = &hunks[0].header;

        assert_eq!(header.source.start, 637);
        assert_eq!(header.source.range, 6);

        assert_eq!(header.dest.start, 637);
        assert_eq!(header.dest.range, 7);

        let header = &hunks[1].header;
        assert_eq!(header.source.start, 652);
        assert_eq!(header.source.range, 8);

        assert_eq!(header.dest.start, 653);
        assert_eq!(header.dest.range, 9);

        let header = &hunks[2].header;

        assert_eq!(header.source.start, 664);
        assert_eq!(header.source.range, 6);

        assert_eq!(header.dest.start, 666);
        assert_eq!(header.dest.range, 8);
    }

    #[test_log::test]
    fn test_find_candidates_single_hunk() {
        let hunks = parse_hunks(&BAD_SINGLE_HUNK).unwrap();
        assert_eq!(hunks.len(), 1);
        let content = std::fs::read_to_string("src/evaluations/fixtures/swebench_2148/models.py")
            .expect("Failed to read file");
        let candidates = find_candidates(&content, &hunks);
        assert_eq!(candidates.len(), 1);

        let hunk = rebuild_hunks(&candidates).first().unwrap().clone();

        assert_eq!(hunk.header.fixed_source.as_ref().unwrap().start, 641); // One less than
                                                                           // in the source file
        assert_eq!(hunk.header.fixed_source.as_ref().unwrap().range, 7);
        assert_eq!(hunk.header.fixed_dest.as_ref().unwrap().start, 641);
        assert_eq!(hunk.header.fixed_dest.as_ref().unwrap().range, 9);
        assert_eq!(candidates.first().unwrap().offset(), 2);

        insta::assert_snapshot!(hunk.render_updated().unwrap());

        let patch_str = rebuild_patch(&BAD_SINGLE_HUNK, &[hunk]).unwrap();
        Patch::from_str(&patch_str).expect("Failed to parse patch");
    }

    #[test_log::test]
    fn test_find_candidates_multiple_hunks() {
        let hunks = parse_hunks(&BAD_PATCH).unwrap();
        let content = std::fs::read_to_string("src/evaluations/fixtures/swebench_2148/models.py")
            .expect("Failed to read file");

        let candidates = find_candidates(&content, &hunks);
        assert_eq!(candidates.len(), hunks.len());

        let expected_ranges = [
            ((641, 7), (641, 9)),
            ((651, 7), (653, 9)),
            ((661, 6), (665, 9)),
        ];

        let hunks = rebuild_hunks(&candidates);

        for (hunk, (source, dest)) in hunks.iter().zip(expected_ranges.iter()) {
            assert_eq!(hunk.header.fixed_source.as_ref().unwrap().start, source.0);
            assert_eq!(hunk.header.fixed_source.as_ref().unwrap().range, source.1);
            assert_eq!(hunk.header.fixed_dest.as_ref().unwrap().start, dest.0);
            assert_eq!(hunk.header.fixed_dest.as_ref().unwrap().range, dest.1);
        }

        insta::assert_snapshot!(hunks
            .iter()
            .map(Hunk::render_updated)
            .collect::<Result<Vec<_>>>()
            .unwrap()
            .join("\n"));

        let patch_str = rebuild_patch(&BAD_SINGLE_HUNK, &hunks).unwrap();
        println!("{patch_str}");
        Patch::from_str(&patch_str).expect("Failed to parse patch");
    }

    #[test_log::test]
    fn test_applying_patch() {
        let content = "abc\n";
        //spellchecker:off
        let patch = indoc::indoc! {"
            --- a/test.txt
            +++ b/test.txt
            @@ -3,1 +1,1 @@
            -abc
            +abd
            "};
        //spellchecker:on

        let hunks = parse_hunks(patch).unwrap();
        let candidates = find_candidates(content, &hunks);
        let hunks = rebuild_hunks(&candidates);
        let updated_patch = rebuild_patch(patch, &hunks).unwrap();

        let patch = Patch::from_str(&updated_patch).unwrap();
        let updated = match diffy::apply(content, &patch) {
            Ok(updated) => updated,
            Err(e) => {
                tracing::error!(%e, "Failed to apply patch");
                panic!("Failed to apply patch");
            }
        };
        assert_eq!(updated, "abd\n"); // spellchecker:disable-line
    }

    #[test_log::test]
    fn test_ambiguity() {
        let content = "\
            a
            b
            c
            b
            d
            ";

        // If there are multiple candidates, when rebuilding the patch it should take the one
        // closest to the original number (perhaps also consider only if multiple hunks )
        let patch = indoc::indoc! {"
            --- a/test.txt
            +++ b/test.txt
            @@ -4,1 +4,1 @@
            -b
            +e
            "};

        let hunks = parse_hunks(patch).unwrap();
        let candidates = find_candidates(content, &hunks);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].start, 1);
        assert_eq!(candidates[1].start, 3);

        let hunks = rebuild_hunks(&candidates);
        assert_eq!(hunks.len(), 1);
        let hunk = hunks.first().unwrap();
        assert_eq!(hunk.header.fixed_source.as_ref().unwrap().start, 3);
    }

    #[test_log::test]
    fn test_flexible_whitespace_in_content() {
        let mut content = indoc::indoc! {"
            a
            b

            c
            "}
        .to_string();

        if !content.ends_with('\n') {
            content.push('\n');
        }
        let patch = indoc::indoc! {"
            --- a/test.txt
            +++ b/test.txt
            @@ -4,1 +4,1 @@
            a
            -b
            +e
            c"};

        let hunks = parse_hunks(patch).unwrap();
        let candidates = find_candidates(&content, &hunks);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].start, 0);

        let hunks = rebuild_hunks(&candidates);
        assert_eq!(hunks.len(), 1);
        let hunk = hunks.first().unwrap();
        dbg!(&hunk.lines);
        assert_eq!(hunk.header.fixed_source.as_ref().unwrap().start, 0);
        // The updated patch will now have the whitespace line added
        assert_eq!(hunk.header.fixed_source.as_ref().unwrap().range, 4);

        let mut updated_patch = rebuild_patch(patch, &hunks).unwrap();
        updated_patch.push('\n');
        println!("---\n{updated_patch}\n---");
        println!("---\n{content}\n---");
        let patch = Patch::from_str(&updated_patch).unwrap();

        let updated_content = diffy::apply(&content, &patch).unwrap();
        assert_eq!(updated_content, "a\ne\n\nc\n");
    }
}
