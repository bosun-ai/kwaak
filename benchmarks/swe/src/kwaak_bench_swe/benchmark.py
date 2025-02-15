"""Benchmark management module for SWE-bench evaluation.

This module provides the Benchmark class which manages the execution and results
of multiple trials across a set of SWE-bench instances. It handles:
- Trial execution orchestration
- Result persistence and loading
- Progress tracking
- Output file management

Typical usage:
    benchmark = Benchmark("my-benchmark", instances, "./results")
    while result := benchmark.run_next_trial():
        print(f"Trial completed: {result}")
"""

import os
import json
from typing import Any

from .swe_bench_instance import SWEBenchInstance
from .trial import Trial, TrialResult

class Benchmark:
    """Manages the execution and results of SWE-bench trials.
    
    This class orchestrates the execution of trials across multiple SWE-bench
    instances, manages result persistence, and tracks progress. It provides
    functionality to run trials sequentially and maintain their results.
    
    Attributes:
        name: str
            Identifier for this benchmark run
        instances: list[SWEBenchInstance]
            List of SWE-bench instances to evaluate
        results: dict[str, TrialResult]
            Dictionary mapping trial names to their results
        output_path: str
            Directory path where results are stored
    """
    
    name: str
    instances: list[SWEBenchInstance]
    results: dict[str, TrialResult]
    output_path: str

    def __init__(self, name: str, instances: list[SWEBenchInstance], output_path: str):
        """Initialize a new benchmark run.
        
        Args:
            name: Identifier for this benchmark run
            instances: List of SWE-bench instances to evaluate
            output_path: Base directory for storing results
        
        The constructor will:
        1. Create the benchmark-specific output directory
        2. Load any existing results from previous runs
        3. Initialize the results tracking dictionary
        """
        self.name = name
        self.instances = instances
        self.results = {}

        self.output_path = os.path.join(output_path, name)
        os.makedirs(self.output_path, exist_ok=True)
        
        # Load existing results from JSON files
        for file in os.listdir(self.output_path):
            if file.endswith('.json'):
                file_path = os.path.join(self.output_path, file)
                if os.path.isfile(file_path):
                    with open(file_path, "r") as f:
                        run_name = file.split(".json")[0]
                        data = json.load(f)
                        # Convert instance data back to SWEBenchInstance
                        if 'instance' in data:
                            data['instance'] = SWEBenchInstance(**data['instance'])
                        self.results[run_name] = TrialResult(**data)

    def run_name(self, instance: SWEBenchInstance) -> str:
        """Generate a unique name for a trial run.
        
        Args:
            instance: The SWE-bench instance being evaluated
            
        Returns:
            A string combining the benchmark name and instance ID
        """
        return f"{self.name}-{instance.instance_id}"

    def add_result(self, run_name: str, result: TrialResult) -> None:
        """Store a trial result and persist it to disk.
        
        Args:
            run_name: Unique identifier for the trial run
            result: The trial's execution result
            
        The result is both stored in memory and written to a JSON file in the
        benchmark's output directory. The JSON file will contain the serialized
        form of the TrialResult, including any nested SWEBenchInstance data.
        """
        self.results[run_name] = result
        with open(os.path.join(self.output_path, f"{run_name}.json"), "w") as f:
            json.dump(result.to_dict(), f, indent=2)

    def next_run(self) -> dict[str, Any] | None:
        """Find the next instance that needs to be evaluated.
        
        Returns:
            A dictionary containing the next instance and its run name,
            or None if all instances have been evaluated. The dictionary
            has the following structure:
            {"instance": SWEBenchInstance, "run_name": str}
        """
        for instance in self.instances:
            run_name = self.run_name(instance)
            if run_name not in self.results:
                return {
                    "instance": instance,
                    "run_name": run_name
                }
        return None

    def run_next_trial(self) -> TrialResult | None:
        """Execute the next pending trial in the benchmark.
        
        This method:
        1. Finds the next unevaluated instance
        2. Creates and executes a trial for that instance
        3. Stores the result
        
        Returns:
            The result of the trial execution, or None if all trials
            have been completed
            
        This method is typically used in a while loop to process
        all remaining trials sequentially.
        """
        next_run = self.next_run()
        if next_run is None:
            return None
        trial = Trial(next_run["instance"], next_run["run_name"], self.output_path)
        result = trial.run()
        self.add_result(next_run["run_name"], result)

        return result
