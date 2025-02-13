"""Main module for running the Kwaak agent against the SWE-bench dataset.

This module orchestrates the entire benchmark process:
1. Loads the SWE-bench dataset
2. Prepares test instances
3. Runs benchmarks
4. Collects and saves results

The module supports running a subset of the dataset (first 10 items per repository)
and handles proper cleanup of system resources.

Typical usage:
    $ uv run kwaak-bench-swe
"""

from datasets import load_dataset
import os
import yaml
import subprocess
import json
import logging

from .benchmark import Benchmark
from .swe_bench_instance import SWEBenchInstance

from swebench.harness.prepare_images import main as prepare_images

# Configuration constants
DATASET_NAME = "princeton-nlp/SWE-bench_Verified"
SPLIT = "test"

def cleanup_processes():
    """Kill any existing LLM proxy and workspace provider processes.
    
    This function ensures a clean environment by terminating any running
    instances of the Amsterdam LLM proxy and Derrick workspace provider.
    It uses pkill to find and terminate these processes.
    
    The function is designed to be fault-tolerant and will not raise exceptions
    if the processes are not found or cannot be killed.
    
    Returns:
        None
    """
    try:
        # Find and kill amsterdam processes
        subprocess.run(["pkill", "-f", "amsterdam"], check=False)
        # Find and kill derrick processes
        subprocess.run(["pkill", "-f", "derrick"], check=False)
    except Exception as e:
        print(f"Warning: Error cleaning up processes: {e}")

def main():
    """Run the SWE-bench benchmark with the Kwaak agent.
    
    This function orchestrates the entire benchmark process:
    1. Sets up logging
    2. Cleans up any existing processes
    3. Loads and filters the SWE-bench dataset
    4. Creates benchmark instances
    5. Runs trials for each instance
    6. Collects and saves results
    
    The function processes a subset of the dataset by taking the first 10 items
    from each repository. Results are saved in both detailed JSON format and
    the SWE-bench submission format (predictions.jsonl).
    
    Environment Requirements:
        - Docker must be running
        - Python 3.11 or higher
        - Sufficient disk space for Docker images
    
    Returns:
        None
    """

    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Clean up any existing processes
    cleanup_processes()
    
    # Load the dataset
    dataset = load_dataset(DATASET_NAME, split=SPLIT)
    logging.info(f"Total items in test split: {len(dataset)}\n")
    predictions = []

    all_repos = list(set([item["repo"] for item in dataset]))

    # Convert dataset to list and sort by instance_id
    dataset_list = list(dataset)
    dataset_list.sort(key=lambda x: x["instance_id"])

    # Get the first 10 items for each repo from the dataset
    raw_dataset_items = []
    for repo in all_repos:
        repo_items = [item for item in dataset_list if item["repo"] == repo]
        raw_dataset_items.extend(repo_items[:10])

    dataset_items = SWEBenchInstance.from_dataset(raw_dataset_items)
    instance_ids = [item.instance_id for item in dataset_items]

    # prepare_images(
    #     DATASET_NAME,
    #     SPLIT,
    #     instance_ids,
    #     4, # max workers
    #     False, # force rebuild
    #     8192, # open file limit
    # )

    output_path = os.path.join(os.getcwd(), "results")
    os.makedirs(output_path, exist_ok=True)

    kwaak_version = "0.1.0"
    benchmark_name = f"swe-bench-kwaak-{kwaak_version}"
    benchmark = Benchmark(benchmark_name, dataset_items, output_path)

    while result := benchmark.run_next_trial():
        logging.info(f"Running trial {result.instance.instance_id}")

    for name, result in benchmark.results.items():
        if result.failed():
            continue

        prediction = {
            "instance_id": result.instance.instance_id,
            "model_name_or_path": benchmark_name,
            "model_patch": result.patch,
            "run_name": name
        }

        predictions.append(prediction)

    with open("predictions.jsonl", "w") as f:
        for prediction in predictions:
            f.write(json.dumps(prediction) + "\n")

    with open("swe_bench_results.json", "w") as f:
        # Convert results to a dictionary of serializable results
        serializable_results = {name: result.to_dict() for name, result in benchmark.results.items()}
        json.dump(serializable_results, f, indent=2)

if __name__ == "__main__":
    main()
