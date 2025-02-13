from dataclasses import dataclass, asdict
from typing import Any
from .swe_bench_instance import SWEBenchInstance
from .docker_instance import DockerInstance
import logging

from swebench.harness.grading import get_eval_report

@dataclass
class TrialResult:
  instance: SWEBenchInstance
  run_failed: bool = False
  validation_failed: bool = False
  success: bool = False
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
  results_dir: str

  def __init__(self, item: SWEBenchInstance, name: str, results_dir: str) -> None:
    self.item = item
    self.name = name
    self.results_dir = results_dir
    self.container = DockerInstance(self.item, self.results_dir)

  def run(self) -> TrialResult:
    logging.info(f"Running trial {self.name}")

    self.container.run()

    self.container.write_string_to_file(self.item.test_patch, "/tmp/test.patch")
    patch_result = self.container.exec("git apply /tmp/test.patch")

    initial_git_ref = self.establish_initial_git_ref()

    if patch_result.exit_code != 0:
      logging.info(f"Test Patch failed: {patch_result}")
      return TrialResult(
        instance=self.item,
        run_failed=False,
        validation_failed=True,
        error="Patch failed",
      )
    
    pre_patch_results = self.container.exec(self.item.test_cmd)
    pre_patch_results_path = os.path.join(self.results_dir, f"{self.name}-pre_patch_test_results.txt")
    
    # write results to file in results_dir
    with open(pre_patch_results_path, "w") as f:
      f.write(pre_patch_results.output)

    # Run the agent
    self.install_agent()
    self.run_agent()

    diff = self.container.exec(f"git diff {initial_git_ref}").output

    prediction = {
      "instance_id": self.item.instance_id,
      "model_name_or_path": self.name,
      "model_patch": diff,
    }

    test_results = self.container.exec(self.item.test_cmd)
    test_results_path = os.path.join(self.results_dir, f"{self.name}-test_results.txt")
    
    with open(test_results_path, "w") as f:
      f.write(test_results.output)

    model_patch_path = os.path.join(self.results_dir, f"{self.name}-patch.diff")

    with open(model_patch_path, "w") as f:
      f.write(diff)

    result = self.evaluate_results(prediction, test_results.output)
    
    # TODO: Uncomment next line when debugging is done:
    # self.container.cleanup()

    return result

  def establish_initial_git_ref(self):
      commit_context = "git config user.name 'agent-test-harness'; git config user.email 'agent-test-harness@bosun.ai';"
      result = self.container.exec(f"{commit_context} git commit -a -m \"benchmark-head\" 1>/dev/null; git rev-parse HEAD")
      if result.exit_code != 0:
          raise Exception(f"Failed to establish initial git ref: {result.output}")
      return result.output.strip()

  def install_agent(self):
    # agent is located at the root of the git repository of the cwd
    agent_root = subprocess.check_output(["git", "rev-parse", "--show-toplevel"]).decode().strip()

    # check if cross is installed
    if subprocess.run(["cross", "--version"], check=False) != 0:
      subprocess.run(["cargo install cross --git https://github.com/cross-rs/cross"], check=True)

    # we use cargo build to ensure the agent is built for the x96_64 architecture
    subprocess.run(["cross", "build", "--target", "x86_64-unknown-linux-gnu", "--release"], cwd=agent_root)
    # copy the agent binary to the root of the results directory
    agent_path = os.path.join(agent_root, "target", "x86_64-unknown-linux-gnu", "release", "kwaak")
    subprocess.run(["cp", agent_path, self.container.instance_dir])

  def run_agent(self):
    # We setup a kwaak.toml file in the instance directory, then we run the agent
    # using the kwaak command in run-agent mode with an initial-message with a prompt
    # that includes the problem statement from instance.
    pass

  def evaluate_results(self, prediction: dict, results: str) -> TrialResult:
    instance_id = self.item.instance_id

    test_spec = {
      "instance_id": instance_id,
      "FAIL_TO_PASS": self.item.FAIL_TO_PASS,
      "PASS_TO_PASS": self.item.PASS_TO_PASS,
    }

    report = get_eval_report(test_spec, prediction, results)
    resolved = report[instance_id]['resolved']

    logging.info(
      f"report: {report}\n"
      f"Result for {instance_id}: resolved: {resolved}"
    )

    report_path = os.join(self.results_dir, f"{self.name}-report.json")
    
    with open(report_path, "w") as f:
      json.dump(report, f, indent=2)

    return TrialResult(
      instance=self.item,
      run_failed=False,
      validation_failed=False,
      patch=prediction["model_patch"],
      success=resolved,
      error=None
    )
