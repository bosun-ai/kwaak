import os
import json

from .swe_bench_instance import SWEBenchInstance
from .trial import Trial, TrialResult

class Benchmark:
    name: str
    instances: list[SWEBenchInstance]
    results: dict[str, TrialResult]
    output_path: str

    def __init__(self, name: str, instances: list[SWEBenchInstance], output_path: str):
        self.name = name
        self.instances = instances
        self.results = {}

        self.output_path = os.path.join(output_path, name)
        os.makedirs(self.output_path, exist_ok=True)
        
        for file in os.listdir(self.output_path):
            with open(os.path.join(self.output_path, file), "r") as f:
                run_name = file.split(".json")[0]
                data = json.load(f)
                # Convert instance data back to SWEBenchInstance
                if 'instance' in data:
                    data['instance'] = SWEBenchInstance(**data['instance'])
                self.results[run_name] = TrialResult(**data)

    def run_name(self, instance: SWEBenchInstance):
        return f"{self.name}-{instance.instance_id}"

    def add_result(self, run_name: str, result: TrialResult):
        self.results[run_name] = result
        with open(os.path.join(self.output_path, f"{run_name}.json"), "w") as f:
            json.dump(result.to_dict(), f, indent=2)

    def next_run(self):
        for instance in self.instances:
            run_name = self.run_name(instance)
            if run_name not in self.results:
                return {
                    "instance": instance,
                    "run_name": run_name
                }
        return None

    def run_next_trial(self) -> TrialResult | None:
        next_run = self.next_run()
        if next_run is None:
            return None
        trial = Trial(next_run["instance"], next_run["run_name"])
        result = trial.run()
        self.add_result(next_run["run_name"], result)

        return result
