use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;

#[derive(Debug)]
pub struct EvalOutput {
    iteration_dir: PathBuf,
}

impl EvalOutput {
    pub fn new(eval_type: &str, iteration: u32) -> Result<Self> {
        let output_dir = Path::new("evals");
        let eval_dir = output_dir.join(eval_type);
        let iteration_dir = eval_dir.join(format!("iteration_{iteration}"));
        fs::create_dir_all(&iteration_dir)?;

        Ok(Self {
            iteration_dir,
        })
    }

    pub fn write_agent_log(&self, content: &str) -> Result<()> {
        fs::write(self.iteration_dir.join("agent.log"), content)?;
        Ok(())
    }

    pub fn write_diff(&self, content: &str) -> Result<()> {
        fs::write(self.iteration_dir.join("changes.diff"), content)?;
        Ok(())
    }

    pub fn write_file(&self, filename: &str, content: &str) -> Result<()> {
        fs::write(self.iteration_dir.join(filename), content)?;
        Ok(())
    }
}
