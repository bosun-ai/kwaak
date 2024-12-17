use anyhow::{Context as _, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use swiftide::{
    query::StreamExt as _,
    traits::{Command, CommandError, CommandOutput, ToolExecutor},
};
use tokio::io::AsyncReadExt as _;
use tracing::{error, info};

use bollard::{
    container::{
        Config, CreateContainerOptions, KillContainerOptions, LogOutput, StartContainerOptions,
    },
    exec::{CreateExecOptions, StartExecResults},
    image::BuildImageOptions,
    Docker,
};
use ignore::WalkBuilder;
use tokio_tar::{Builder, Header};

use crate::repository::Repository;

/// Starts up a docker container from the dockerfile configured in the repository
///
/// - Build a docker image with bollard and start it up
/// - Ensure current context (ie the code) is present in the container
/// - implement the Workspace trait for this
///
#[derive(Clone)]
pub struct RunningDockerExecutor {
    container_id: String,
    docker: Docker,
    // docker file?
    // handle?
}

#[derive(Clone, Debug)]
pub struct DockerExecutor {
    context_path: PathBuf,
    image_name: String,
    #[allow(dead_code)]
    working_dir: PathBuf,
    dockerfile: PathBuf,
}

impl Default for DockerExecutor {
    fn default() -> Self {
        Self {
            context_path: ".".into(),
            image_name: "docker-executor".into(),
            working_dir: ".".into(),
            dockerfile: "Dockerfile".into(),
        }
    }
}

impl DockerExecutor {
    pub fn from_repository(repository: &Repository) -> DockerExecutor {
        let mut executor = DockerExecutor::default();
        let dockerfile = &repository.config().docker.dockerfile;

        if std::fs::metadata(dockerfile).is_err() {
            error!("Dockerfile not found at {}", dockerfile.display());
            // TODO: Clean me up
            panic!("Running in docker requires a Dockerfile");
        }
        executor
            .with_context_path(&repository.config().docker.context)
            .with_image_name(&repository.config().project_name)
            .with_dockerfile(dockerfile);

        executor
    }

    pub fn with_context_path(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.context_path = path.into();

        self
    }

    pub fn with_image_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.image_name = name.into();

        self
    }

    pub fn with_dockerfile(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.dockerfile = path.into();
        self
    }

    #[allow(dead_code)]
    pub fn with_working_dir(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.working_dir = path.into();

        self
    }

    pub async fn start(self) -> Result<RunningDockerExecutor> {
        RunningDockerExecutor::start(&self.context_path, &self.dockerfile, &self.image_name).await
    }
}

#[async_trait]
impl ToolExecutor for RunningDockerExecutor {
    #[tracing::instrument(skip(self), err)]
    async fn exec_cmd(&self, cmd: &Command) -> Result<CommandOutput, CommandError> {
        match cmd {
            Command::Shell(cmd) => self.exec_shell(cmd).await,
            Command::ReadFile(path) => self.read_file(path).await,
            Command::WriteFile(path, content) => self.write_file(path, content).await,
            _ => unimplemented!(),
        }
    }
}

impl RunningDockerExecutor {
    /// Starts a docker container with a given context and image name
    pub async fn start(
        context_path: &Path,
        dockerfile: &Path,
        image_name: &str,
    ) -> Result<RunningDockerExecutor> {
        let docker = Docker::connect_with_socket_defaults().unwrap();

        // TODO: Handle dockerfile not being named `Dockerfile` or missing
        // let dockerfile_path = &repository.config().docker.dockerfile;

        tracing::warn!(
            "Creating archive for context from {}",
            context_path.display()
        );
        let context = build_context_as_tar(context_path).await?;

        let image_name = format!("kwaak-{image_name}");
        let build_options = BuildImageOptions {
            t: image_name.as_str(),
            rm: true,
            #[allow(clippy::unnecessary_to_owned)]
            dockerfile: &dockerfile.to_string_lossy().into_owned(),
            ..Default::default()
        };

        tracing::warn!("Building docker image with name {image_name}");
        {
            let mut build_stream = docker.build_image(build_options, None, Some(context.into()));

            while let Some(log) = build_stream.next().await {
                match log {
                    Ok(output) => {
                        if let Some(stream) = output.stream {
                            info!("{}", stream);
                        }
                    }
                    // TODO: This can happen if 2 threads build the same image in parallel, and
                    // should be handled
                    Err(e) => error!("Error during build: {:?}", e),
                }
            }
        }

        let config = Config {
            image: Some(image_name.as_str()),
            tty: Some(true),
            host_config: Some(bollard::models::HostConfig {
                auto_remove: Some(true),
                binds: Some(vec![String::from(
                    "/var/run/docker.sock:/var/run/docker.sock",
                )]),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Add a random suffix so multiple containers do not conflict
        let random_suffix = uuid::Uuid::new_v4().to_string();
        let container_name = format!("kwaak-{image_name}-{random_suffix}");
        let create_options = CreateContainerOptions {
            name: container_name.as_str(),
            ..Default::default()
        };

        tracing::warn!("Creating container from image {image_name}");
        let container_id = docker
            .create_container(Some(create_options), config)
            .await?
            .id;

        tracing::warn!("Starting container {container_id}");
        docker
            .start_container(&container_id, None::<StartContainerOptions<String>>)
            .await?;

        Ok(RunningDockerExecutor {
            container_id,
            docker,
        })
    }

    async fn exec_shell(&self, cmd: &str) -> Result<CommandOutput, CommandError> {
        let cmd = vec!["sh", "-c", cmd];
        tracing::debug!("Executing command {cmd}", cmd = cmd.join(" "));

        let exec = self
            .docker
            .create_exec(
                &self.container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd),
                    ..Default::default()
                },
            )
            .await
            .context("Failed to execute command in docker")?
            .id;

        let mut stdout = String::new();
        let mut stderr = String::new();

        if let StartExecResults::Attached { mut output, .. } = self
            .docker
            .start_exec(&exec, None)
            .await
            .context("Failed to attach to docker exec")?
        {
            while let Some(Ok(msg)) = output.next().await {
                match msg {
                    LogOutput::StdErr { .. } => stderr.push_str(&msg.to_string()),
                    LogOutput::StdOut { .. } => stdout.push_str(&msg.to_string()),
                    _ => {
                        stderr
                            .push_str("Command appears to wait for input, which is not supported");
                        break;
                    }
                }
            }
        } else {
            todo!();
        }

        let exec_inspect = self
            .docker
            .inspect_exec(&exec)
            .await
            .context("Failed to inspect docker exec")?;
        let exit_code = exec_inspect.exit_code.unwrap_or(0);

        // Trim both stdout and stderr to remove surrounding whitespace and newlines
        let output = stdout.trim().to_string() + stderr.trim();

        if exit_code == 0 {
            Ok(output.into())
        } else {
            Err(CommandError::FailedWithOutput(output.into()))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn read_file(&self, path: &Path) -> Result<CommandOutput, CommandError> {
        self.exec_shell(&format!("cat {}", path.display())).await
    }

    #[tracing::instrument(skip(self, content))]
    async fn write_file(&self, path: &Path, content: &str) -> Result<CommandOutput, CommandError> {
        let cmd = indoc::formatdoc! {r#"
            cat << 'EOFKWAAK' > {path}
            {content}
            EOFKWAAK"#,
            path = path.display(),
            content = content.trim_end()

        };

        let write_file_result = self.exec_shell(&cmd).await;

        // If the directory or file does not exist, create it
        if let Err(CommandError::FailedWithOutput(write_file)) = &write_file_result {
            if ["No such file or directory", "Directory nonexistent"]
                .iter()
                .any(|&s| write_file.output.contains(s))
            {
                let path = path.parent().context("No parent directory")?;
                let mkdircmd = format!("mkdir -p {}", path.display());
                let _ = self.exec_shell(&mkdircmd).await?;

                return self.exec_shell(&cmd).await;
            }
        }

        write_file_result
    }
}

impl Drop for RunningDockerExecutor {
    fn drop(&mut self) {
        tracing::warn!(
            "Stopping container {container_id}",
            container_id = self.container_id
        );
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.docker
                    .kill_container(
                        &self.container_id,
                        Some(KillContainerOptions { signal: "SIGKILL" }),
                    )
                    .await
            })
        });

        if let Err(e) = result {
            tracing::warn!(error = %e, "Error stopping container, might not be stopped");
        }
    }
}

// Iterate over all the files in the context directory and adds it to an in memory
// tar. Respects .gitignore and .dockerignore.
async fn build_context_as_tar(context_path: &Path) -> Result<Vec<u8>> {
    let buffer = Vec::new();

    let mut tar = Builder::new(buffer);

    // Ensure we *do* include the .git directory
    // let overrides = OverrideBuilder::new(context_path).add(".git")?.build()?;

    for entry in WalkBuilder::new(context_path)
        // .overrides(overrides)
        .hidden(false)
        .add_custom_ignore_filename(".dockerignore")
        .build()
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let mut file = tokio::fs::File::open(path).await?;
            let mut buffer_content = Vec::new();
            file.read_to_end(&mut buffer_content).await?;

            let mut header = Header::new_gnu();
            header.set_size(buffer_content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();

            let relative_path = path.strip_prefix(context_path)?;
            tar.append_data(&mut header, relative_path, &*buffer_content)
                .await?;
        }
    }

    let result = tar.into_inner().await?;

    Ok(result.clone())
}

#[cfg(test)]
mod tests {

    use bollard::secret::ContainerStateStatusEnum;

    use super::*;

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_runs_docker_and_echos() {
        let executor = DockerExecutor::default()
            .with_context_path(".")
            .with_image_name("tests")
            .to_owned()
            .start()
            .await
            .unwrap();

        let output = executor
            .exec_cmd(&Command::Shell("echo hello".to_string()))
            .await
            .unwrap();

        assert_eq!(output.to_string(), "hello");
    }

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_context_present() {
        let executor = DockerExecutor::default()
            .with_context_path(".")
            .with_image_name("tests")
            .with_working_dir("/app")
            .to_owned()
            .start()
            .await
            .unwrap();

        // Verify that the working directory is set correctly
        // TODO: Annoying this needs to be updated when files change in the root. Think of something better.
        let ls = executor
            .exec_cmd(&Command::Shell("ls -a".to_string()))
            .await
            .unwrap();

        assert!(ls.to_string().contains("Cargo.toml"));
    }

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_write_and_read_file_with_quotes() {
        let content = r#"This is a "test" content with 'quotes' and special characters: \n \t"#;
        let path = Path::new("test_file.txt");

        let executor = DockerExecutor::default()
            .with_context_path(".")
            .with_image_name("test-files")
            .with_working_dir("/app")
            .to_owned()
            .start()
            .await
            .unwrap();

        // Write the content to the file
        let _ = executor
            .exec_cmd(&Command::write_file(path, content))
            .await
            .unwrap();

        let output = executor.exec_cmd(&Command::shell("ls")).await.unwrap();

        // Read the content from the file
        //
        let read_file = executor.exec_cmd(&Command::read_file(path)).await.unwrap();

        assert_eq!(content, read_file.output);
    }

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_write_and_read_file_markdown() {
        let content = r#"# Example

        ```rust
        fn main() {
            let hello = "world";
            println!("Hello, {}", hello);
            }
        ```

        ```shell
        $ cargo run
        ```"#;
        let path = Path::new("test_file.txt");

        let executor = DockerExecutor::default()
            .with_context_path(".")
            .with_image_name("test-files-md")
            .with_working_dir("/app")
            .to_owned()
            .start()
            .await
            .unwrap();

        // Write the content to the file
        let _ = executor
            .exec_cmd(&Command::write_file(path, content))
            .await
            .unwrap();

        let output = executor.exec_cmd(&Command::shell("ls")).await.unwrap();

        // Read the content from the file
        //
        let read_file = executor.exec_cmd(&Command::read_file(path)).await.unwrap();

        assert_eq!(content, read_file.output);
    }

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_assert_container_stopped_on_drop() {
        let executor = DockerExecutor::default()
            .with_context_path(".")
            .with_image_name("test-drop")
            .with_working_dir("/app")
            .to_owned()
            .start()
            .await
            .unwrap();

        let docker = executor.docker.clone();
        let container_id = executor.container_id.clone();

        // assert it started
        let container = docker.inspect_container(&container_id, None).await.unwrap();
        assert_eq!(
            container.state.as_ref().unwrap().status,
            Some(ContainerStateStatusEnum::RUNNING)
        );

        drop(executor);

        // assert it stopped
        let container = docker.inspect_container(&container_id, None).await.unwrap();
        assert_eq!(
            container.state.as_ref().unwrap().status,
            Some(ContainerStateStatusEnum::REMOVING)
        );
    }

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_create_file_subdirectory_that_does_not_exist() {
        let content = r#"# Example

        ```rust
        fn main() {
            let hello = "world";
            println!("Hello, {}", hello);
            }
        ```

        ```shell
        $ cargo run
        ```"#;
        let path = Path::new("doesnot/exist/test_file.txt");

        let executor = DockerExecutor::default()
            .with_context_path(".")
            .with_image_name("test-files-missing-dir")
            .with_working_dir("/app")
            .to_owned()
            .start()
            .await
            .unwrap();

        // Write the content to the file
        let _ = executor
            .exec_cmd(&Command::write_file(path, content))
            .await
            .unwrap();

        let output = executor.exec_cmd(&Command::shell("ls")).await.unwrap();

        // Read the content from the file
        //
        let read_file = executor.exec_cmd(&Command::read_file(path)).await.unwrap();

        // Assert that the written content matches the read content
        assert_eq!(content, read_file.output);
    }

    #[test_log::test(tokio::test(flavor = "multi_thread"))]
    async fn test_custom_dockerfile() {
        let context_path = tempfile::tempdir().unwrap();

        std::process::Command::new("cp")
            .arg("Dockerfile")
            .arg(context_path.path().join("Dockerfile.custom"))
            .output()
            .unwrap();

        let executor = DockerExecutor::default()
            .with_context_path(context_path.path())
            .with_image_name("test-custom")
            .with_dockerfile("Dockerfile.custom")
            .to_owned()
            .start()
            .await
            .unwrap();

        let output = executor
            .exec_cmd(&Command::shell("echo hello"))
            .await
            .unwrap();
        assert_eq!(output.to_string(), "hello");
    }
}
