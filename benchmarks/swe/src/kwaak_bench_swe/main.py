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
import subprocess
import json
import logging
import docker

from .benchmark import Benchmark
from .swe_bench_instance import SWEBenchInstance

from swebench.harness.prepare_images import main as prepare_images
from swebench.harness.test_spec.test_spec import (
    get_test_specs_from_dataset,
)

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
    
    This function orchestrates the benchmark process. It can either:
    1. Run a single instance if --instance is specified
    2. Run a subset of the dataset (first 2 items per repository) by default
    
    Results are saved in both detailed JSON format and the SWE-bench 
    submission format (predictions.jsonl).
    
    Environment Requirements:
        - Docker must be running
        - Python 3.11 or higher
        - Sufficient disk space for Docker images
    
    Command-line Arguments:
        --instance: Optional instance ID to run a single test case
                   e.g., psf__requests-2317
    
    Returns:
        None
    """

    import argparse
    
    # Set up argument parser
    parser = argparse.ArgumentParser(description='Run SWE-bench benchmark with Kwaak agent')
    parser.add_argument('--instance', type=str, help='Instance ID to run a single test case')
    args = parser.parse_args()
    
    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Clean up any existing processes
    cleanup_processes()
    
    # Load the dataset
    dataset = load_dataset(DATASET_NAME, split=SPLIT)
    logging.info(f"Total items in test split: {len(dataset)}\n")
    predictions = []

    # Convert dataset to list and sort by instance_id
    dataset_list = list(dataset)
    dataset_list.sort(key=lambda x: x["instance_id"])
    
    # Filter dataset based on command line arguments
    raw_dataset_items = []
    if args.instance:
        # Find the specific instance
        instance_items = [item for item in dataset_list if item["instance_id"] == args.instance]
        if not instance_items:
            logging.error(f"Instance {args.instance} not found in dataset")
            return
        raw_dataset_items = instance_items
        logging.info(f"Running single instance: {args.instance}")
    else:
        # Get the first 2 items for each repo from the dataset
        all_repos = list(set([item["repo"] for item in dataset]))
        for repo in all_repos:
            repo_items = [item for item in dataset_list if item["repo"] == repo]
            raw_dataset_items.extend(repo_items[:2])
        logging.info(f"Running first 2 items from {len(all_repos)} repositories")

    dataset_items = SWEBenchInstance.from_dataset(raw_dataset_items)
    instance_ids = [item.instance_id for item in dataset_items]

    test_specs = get_test_specs_from_dataset(raw_dataset_items, 'swebench', 'latest')
    for spec in test_specs:
        spec.arch = 'x86_64'
 
    images_to_pull = [
    #     'swebench/' + spec.base_image_key for spec in test_specs
    # ] + [
    #     'swebench/' + spec.env_image_key for spec in test_specs
    # ] + [
        spec.instance_image_key for spec in test_specs
    ]

    docker_client = docker.from_env()
    for image in images_to_pull:
        logging.info(f"Pulling image {image}")
        docker_client.images.pull(image)

    # prepare_images(
    #     DATASET_NAME,
    #     SPLIT,
    #     instance_ids,
    #     4, # max workers
    #     False, # force rebuild
    #     8192, # open file limit
    #     "swebench", # namespace
    #     "latest" # tag
    # )
    

    output_path = os.path.join(os.getcwd(), "results")
    os.makedirs(output_path, exist_ok=True)

    kwaak_version = "0.8.1"
    benchmark_name = f"swe-bench-kwaak-{kwaak_version}"
    benchmark = Benchmark(benchmark_name, dataset_items, output_path)

    logging.info(f"Benchmark name: {benchmark_name}\n")
    logging.info(f"Output path: {output_path}\n")

    while result := benchmark.run_next_trial():
        logging.info(f"Done running trial {result.instance.instance_id}: {result.error or 'Success'}")

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
