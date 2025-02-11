from dataclasses import dataclass, asdict
from typing import Any
from .swe_bench_instance import SWEBenchInstance
from .docker_instance import DockerInstance

@dataclass
class TrialResult:
  instance: SWEBenchInstance
  run_failed: bool = False
  validation_failed: bool = False
  error: str | None = None
  patch: str | None = None

  def failed(self) -> bool:
    return self.run_failed or self.validation_failed or (self.error is not None)

  def to_dict(self) -> dict[str, Any]:
    """Convert the TrialResult to a dictionary for JSON serialization."""
    result = asdict(self)
    # Convert the SWEBenchInstance to a dict if it has a to_dict method
    if hasattr(self.instance, 'to_dict'):
        result['instance'] = self.instance.to_dict()
    return result

class Trial:
  item: SWEBenchInstance
  name: str
  container: DockerInstance

  def __init__(self, item: SWEBenchInstance, name: str) -> None:
    self.item = item
    self.name = name
    self.container = DockerInstance(self.item)

  def run(self) -> TrialResult:
    logging.info(f"Running trial {self.name}")
    # Run the docker image associated with the instance
    # In the docker container, check out the base commit and throw away future commits
    # In the docker container, apply the test patch
    # In the docker container, run the test suite to validate the patch
    # In the docker container, run the agent
    # In the docker container, run git diff between the base commit and the current working tree to get the patch

    self.container.run()



    self.container.cleanup()

    """Run the trial."""
    return TrialResult(
      instance=self.item,
      run_failed=False,
      validation_failed=False,
      error="Not Implemented"
    )

  # The trial goes like this:
  
