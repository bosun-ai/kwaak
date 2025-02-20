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
import argparse

from .benchmark import Benchmark
from .swe_bench_instance import SWEBenchInstance
from .trial import Trial

from swebench.harness.prepare_images import main as prepare_images
from swebench.harness.test_spec.test_spec import (
    get_test_specs_from_dataset,
)

# Configuration constants
DATASET_NAME = "princeton-nlp/SWE-bench_Verified"
SPLIT = "test"

def evaluate_trial(instance_id: str, results_path: str) -> None:
    """Evaluate a specific trial's results.
    
    Args:
        instance_id: The ID of the instance to evaluate
        results_path: Path to the directory containing the trial results and prediction.json
    """
    # Load the dataset to get the instance
    dataset = load_dataset(DATASET_NAME, split=SPLIT)
    dataset_list = list(dataset)
    instance_items = [item for item in dataset_list if item["instance_id"] == instance_id]
    if not instance_items:
        logging.error(f"Instance {instance_id} not found in dataset")
        return
    
    # Create SWEBenchInstance
    instance = SWEBenchInstance.from_dataset([instance_items[0]])[0]
    
    # Create trial
    trial = Trial(instance, instance_id, results_path)
    
    # Load prediction
    prediction_path = os.path.join(results_path, "prediction.json")
    if not os.path.exists(prediction_path):
        logging.error(f"prediction.json not found in {results_path}")
        return
    
    with open(prediction_path, "r") as f:
        prediction = json.load(f)
    
    # Find test results file
    test_results_file = None
    for file in os.listdir(results_path):
        if file.endswith("-test_results.txt") and not file.endswith("-pre_patch_test_results.txt"):
            test_results_file = file
            break
    
    if not test_results_file:
        logging.error(f"No test results file found in {results_path}")
        return
    
    # Evaluate results
    result = trial.evaluate_results(prediction, os.path.join(results_path, test_results_file))
    
    # Print evaluation results
    logging.info(f"Evaluation results for {instance_id}:")
    logging.info(f"Success: {result.success}")
    logging.info(f"Error: {result.error or 'None'}")
    logging.info(f"Validation failed: {result.validation_failed}")

def main():
    """Run the SWE-bench benchmark with the Kwaak agent.

    This function orchestrates the benchmark process. It can either:
    1. Run a single instance if --instance is specified
    2. Run a subset of the dataset (first 2 items per repository) by default
    3. Evaluate a specific trial's results if --evaluate and --results-path are specified

    Results are saved in both detailed JSON format and the SWE-bench 
    submission format (predictions.jsonl).

    Environment Requirements:
        - Docker must be running
        - Python 3.11 or higher
        - Sufficient disk space for Docker images

    Command-line Arguments:
        --instance: Optional instance ID to run a single test case
                   e.g., psf__requests-2317
        --evaluate: Instance ID to evaluate results for
        --results-path: Path to directory containing trial results

    Returns:
        None
    """

    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Set up argument parser
    parser = argparse.ArgumentParser(description='Run SWE-bench benchmark with Kwaak agent')
    parser.add_argument('--instance', type=str, help='Instance ID to run a single test case')
    parser.add_argument('--evaluate', type=str, help='Instance ID to evaluate results for')
    parser.add_argument('--results-path', type=str, help='Path to directory containing trial results')
    args = parser.parse_args()
    
    # If evaluating a specific trial
    if args.evaluate:
        if not args.results_path:
            logging.error("--results-path is required when using --evaluate")
            return
        evaluate_trial(args.evaluate, args.results_path)
        return
    
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

    # instance_ids = [item.instance_id for item in dataset_items]
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

    kwaak_version = "0.10.0"
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
