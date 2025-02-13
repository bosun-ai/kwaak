"""Docker container management for SWE-bench test execution.

This module provides classes for managing Docker containers used in test execution:
- ExecResult: Represents the result of a command execution in a container
- DockerInstance: Manages a Docker container for a specific test instance

The module handles:
- Container lifecycle management
- Command execution
- File operations between host and container
- Resource cleanup

Typical usage:
    instance = DockerInstance(swe_instance, "./results")
    instance.run("test-1")
    result = instance.exec("make test")
    instance.cleanup()
"""

import logging
from typing import Self
import os

from docker import DockerClient, from_env as docker_from_env
from docker.models.containers import Container
from docker.errors import ImageNotFound

import swebench.harness.docker_utils as docker_utils

from .swe_bench_instance import SWEBenchInstance

class ExecResult:
  """Represents the result of a command execution in a Docker container.
  
  This class encapsulates both the output and exit code of a command
  executed in a Docker container, making it easy to check both the
  result and success status of the command.
  
  Attributes:
      output: The command's stdout/stderr output
      exit_code: The command's exit code (0 for success)
  """
  output: str
  exit_code: int

  def __init__(self, output: str, exit_code: int) -> None:
    self.output = output
    self.exit_code = exit_code

class DockerInstance:
  """Manages a Docker container for a specific SWE-bench test instance.
  
  This class handles the complete lifecycle of a Docker container used
  for test execution, including:
  - Container creation and startup
  - Volume mounting
  - Command execution
  - File operations
  - Resource cleanup
  
  The class ensures proper isolation of test environments and handles
  all Docker-related operations safely.
  """
  client: DockerClient
  instance: SWEBenchInstance
  container: Container

  instance_dir: str

  def __init__(self, instance: SWEBenchInstance, results_dir: str):
    """Initialize a new Docker instance manager.
    
    Args:
        instance: The SWE-bench instance this container will run
        results_dir: Directory for storing results and artifacts
    """
    self.client = docker_from_env()
    self.instance = instance
    self.instance_dir = os.path.join(results_dir, "container")

  def run(self, run_id: str) -> Self:
    """Create and start a Docker container for test execution.
    
    This method:
    1. Creates the instance directory
    2. Ensures the required image is available
    3. Creates and starts the container
    4. Sets up volume mounts
    
    Args:
        run_id: Unique identifier for this container run
        
    Returns:
        Self: The instance itself for method chaining
        
    Raises:
        ImageNotFound: If the required Docker image is not available
    """
    os.makedirs(self.instance_dir, exist_ok=True)

    try:
      self.client.images.get(self.instance.instance_image_key)
    except ImageNotFound:
      self.client.images.pull(self.instance.instance_image_key)

    logging.info(f"Creating container for {self.instance.instance_id}...")

    self.container = self.client.containers.create(
        image=self.instance.instance_image_key,
        name=self.instance.get_instance_container_name(run_id),
        user="root",
        detach=True,
        command="tail -f /dev/null",
        platform="linux/x86_64",
        mounts=[
          {
            "type": "bind",
            "source": self.instance_dir,
            "target": "/tmp",
          }
        ]
    )
    logging.info(f"Container for {self.instance.instance_id} created: {container.id}")
    container.start()
    logging.info(f"Container for {self.instance.instance_id} started: {container.id}")

    return self
  
  def write_string_to_file(self, string: str, filepath: str) -> None:
    """Write a string to a file in the container.
    
    This method writes the string to a file in the instance directory
    and then copies it to the specified location in the container.
    
    Args:
        string: Content to write to the file
        filepath: Target path in the container
    """
    src_path = os.path.join(self.instance_dir, filepath)
    dst_path = os.path.join("/tmp", filepath)

    with open(src_path, "w") as f:
      f.write(string)

    self.container.exec_run(f"cp {dst_path} {filepath}")

  def cleanup(self) -> None:
    """Clean up container resources.
    
    This method ensures proper cleanup of the container and its
    resources using the SWE-bench docker utilities.
    """
    docker_utils.cleanup_container(self.container)

  def exec(self, command: str) -> ExecResult:
    """Execute a command in the container.
    
    Args:
        command: The shell command to execute
        
    Returns:
        ExecResult: Object containing the command's output and exit code
    """
    result = self.container.exec_run(command)
    return ExecResult(result.output, result.exit_code)