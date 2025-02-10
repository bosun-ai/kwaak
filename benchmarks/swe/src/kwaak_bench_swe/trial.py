from dataclasses import dataclass, asdict
from typing import Any
from .swe_bench_instance import SWEBenchInstance

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
  
  def __init__(self, item: SWEBenchInstance, name: str) -> None:
    self.item = item
    self.name = name

  def run(self) -> TrialResult:
    """Run the trial."""
    return TrialResult(
      instance=self.item,
      run_failed=False,
      validation_failed=False,
      error="Not Implemented"
    )
