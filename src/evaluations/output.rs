use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct EvalMetrics {
    pub time_spent: Duration,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl EvalMetrics {
    pub fn new(time_spent: Duration, input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            time_spent,
            input_tokens,
            output_tokens,
        }
    }
}

#[derive(Debug)]
pub struct EvalOutput {
    iteration_dir: PathBuf,
    pub(crate) start_time: Instant,
}

impl EvalOutput {
    pub fn new(eval_type: &str, iteration: u32) -> Result<Self> {
        let output_dir = Path::new("evals");
        let eval_dir = output_dir.join(eval_type);
        let iteration_dir = eval_dir.join(format!("iteration_{iteration}"));

        fs::remove_dir_all(&iteration_dir)?;
        fs::create_dir_all(&iteration_dir)?;

        Ok(Self {
            iteration_dir,
            start_time: Instant::now(),
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

    pub fn write_file(&self, name: &str, content: &str) -> Result<()> {
        fs::write(self.iteration_dir.join(name), content)?;
        Ok(())
    }

    pub fn write_metrics(&self, metrics: &EvalMetrics) -> Result<()> {
        let content = format!(
            "Time spent: {:.2}s\nInput tokens: {}\nOutput tokens: {}\n",
            metrics.time_spent.as_secs_f64(),
            metrics.input_tokens,
            metrics.output_tokens,
        );

        fs::write(self.iteration_dir.join("metrics"), content)?;
        Ok(())
    }

    pub fn elapsed_time(&self) -> Duration {
        self.start_time.elapsed()
    }
}
