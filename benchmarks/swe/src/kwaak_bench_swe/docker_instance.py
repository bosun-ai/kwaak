import logging
from typing import Self
import os

from docker import DockerClient, from_env as docker_from_env
from docker.models.containers import Container
from docker.errors import ImageNotFound

import swebench.harness.docker_utils as docker_utils

from .swe_bench_instance import SWEBenchInstance

class ExecResult:
  output: str
  exit_code: int

  def __init__(self, output: str, exit_code: int) -> None:
    self.output = output
    self.exit_code = exit_code

class DockerInstance:
  client: DockerClient
  instance: SWEBenchInstance
  container: Container

  instance_dir: str

  def __init__(self, instance: SWEBenchInstance, results_dir: str):
    self.client = docker_from_env()
    self.instance = instance
    self.instance_dir = os.path.join(results_dir, "container")

  def run(self, run_id: str) -> Self:
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
    src_path = os.path.join(self.instance_dir, filepath)
    dst_path = os.path.join("/tmp", filepath)

    with open(src_path, "w") as f:
      f.write(string)

    self.container.exec_run(f"cp {dst_path} {filepath}")

  def cleanup(self) -> None:
    docker_utils.cleanup_container(self.container)

  def exec(self, command: str) -> ExecResult:
    result = self.container.exec_run(command)
    return ExecResult(result.output, result.exit_code)