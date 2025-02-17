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
    results_dir: str

    def __init__(self, name: str, instances: list[SWEBenchInstance], results_dir: str):
        """Initialize a new benchmark run.
        
        Args:
            name: Identifier for this benchmark run
            instances: List of SWE-bench instances to evaluate
            results_dir: Base directory for storing results
        
        The constructor will:
        1. Create the benchmark-specific output directory
        2. Load any existing results from previous runs
        3. Initialize the results tracking dictionary
        """
        self.name = name
        self.instances = instances
        self.results = {}

        self.results_dir = os.path.join(results_dir, name)
        os.makedirs(self.results_dir, exist_ok=True)
        
        # Load existing results from JSON files
        for instance_dir in os.listdir(self.results_dir):
            instance_path = os.path.join(self.results_dir, instance_dir)

            for run_dir in os.listdir(instance_path):
                run_path = os.path.join(instance_path, run_dir)
                result_path = os.path.join(run_path, "result.json")
                
                if os.path.exists(result_path):
                    run_name = f"{instance_dir}-{run_dir}"
                    
                    data = json.load(open(result_path, "r"))
                    if 'instance' in data:
                        data['instance'] = SWEBenchInstance(**data['instance'])

                    self.results[run_name] = TrialResult(**data)

    def next_run(self) -> dict[str, Any] | None:
        """Find the next instance that needs to be evaluated.
        
        Returns:
            A dictionary containing the next instance and its run name,
            or None if all instances have been evaluated. The dictionary
            has the following structure:
            {"instance": SWEBenchInstance, "run": int, "run_name": str}
        """
        for instance in self.instances:
            run_name = f"{instance.instance_id}-1"
            if run_name not in self.results:
                return {
                    "instance": instance,
                    "run": 1,
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

        run_name = next_run["run_name"]
        instance = next_run["instance"]
        run = next_run["run"]

        run_path = os.path.join(self.results_dir, instance.instance_id, str(run))
        os.makedirs(run_path, exist_ok=True)

        trial = Trial(instance, run_name, run_path)
        result = trial.run()

        self.results[run_name] = result
        with open(os.path.join(run_path, f"result.json"), "w") as f:
            json.dump(result.to_dict(), f, indent=2)

        return result
