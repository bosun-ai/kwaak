class ExecResult:
  def __init__(self, output: str, error: str, returncode: int) -> None:
    self.output = output
    self.error = error
    self.returncode = returncode

class Docker:
  def __init__(self, instance: SWEBenchInstance):
    pass

  def run(self) -> ExecResult:
    return self

  def cleanup(self, instance: SWEBenchInstance) -> None:
    pass

  def exec(self, instance: SWEBenchInstance, command: str) -> ExecResult:
    pass