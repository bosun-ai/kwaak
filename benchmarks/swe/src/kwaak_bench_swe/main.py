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


def copy_kwaak_caches(benchmark_name: str = None):
    """Copy kwaak caches from running containers to the cache directory.
    
    This function:
    1. Gets all running containers with the sweb.eval prefix
    2. For each container:
        - Extracts the instance_id from the container name
        - Looks up the repo from the dataset
        - Creates the cache directory structure
        - Copies the kwaak cache from the container
        
    Args:
        benchmark_name: Optional name of the benchmark. If not provided, will use the default format.
    """
    # Load the dataset to get repo information
    dataset = load_dataset(DATASET_NAME, split=SPLIT)
    dataset_list = list(dataset)
    
    # Get Docker client
    client = docker.from_env()
    
    # Get all running containers with sweb.eval prefix
    containers = client.containers.list(filters={'name': 'sweb.eval'})
    
    for container in containers:
        # Extract instance_id from container name
        # Format: sweb.eval.<instance_id>.<instance_id>-1
        parts = container.name.split('.')
        if len(parts) != 4:
            logging.warning(f"Unexpected container name format: {container.name}")
            continue
            
        # The instance ID is in the format 'owner__repo-number'
        instance_id = parts[2]  # This will be the full instance ID
        if instance_id.endswith('-1'):
            # If this is the first trial, remove the -1
            instance_id = instance_id[:-2]
        
        # Find the dataset item
        dataset_item = next((item for item in dataset_list if item['instance_id'] == instance_id), None)
        if not dataset_item:
            logging.warning(f"Could not find dataset item for instance {instance_id}")
            continue
            
        # Get repo information and version
        repo = dataset_item['repo']
        version = dataset_item['version']
        repo_dir = repo.replace('/', '_')
        
        # Get the benchmark name if not provided
        if not benchmark_name:
            kwaak_version = "0.8.1"
            benchmark_name = f"swe-bench-kwaak-{kwaak_version}"
            
        # Construct cache directory path
        results_dir = os.path.join(os.getcwd(), "results", benchmark_name)
        cache_dir = os.path.join(results_dir, "cache")
        repo_version_dir = os.path.join(cache_dir, repo_dir, version)
        os.makedirs(repo_version_dir, exist_ok=True)
        
        # Copy the kwaak cache from the container
        try:
            # Create a temporary directory for extraction
            temp_dir = os.path.join(cache_dir, f"{instance_id}_temp")
            os.makedirs(temp_dir, exist_ok=True)
            
            # Create a temporary tar file
            temp_tar = os.path.join(cache_dir, f"{instance_id}_cache.tar")
            
            # Get the cache from the container
            bits, stat = container.get_archive('/root/.cache/kwaak')
            
            # Write the tar file
            with open(temp_tar, 'wb') as f:
                for chunk in bits:
                    f.write(chunk)
                    
            # Extract the tar file to the temporary directory
            subprocess.run(['tar', 'xf', temp_tar, '-C', temp_dir], check=True)
            
            # Move the contents of the kwaak directory to the repo version directory
            kwaak_dir = os.path.join(temp_dir, 'kwaak')
            if os.path.exists(kwaak_dir):
                # Move all contents from kwaak_dir to repo_version_dir
                for item in os.listdir(kwaak_dir):
                    src = os.path.join(kwaak_dir, item)
                    dst = os.path.join(repo_version_dir, item)
                    if os.path.exists(dst):
                        if os.path.isdir(dst):
                            os.system(f'rm -rf {dst}')
                        else:
                            os.remove(dst)
                    os.rename(src, dst)
            
            # Clean up temporary files
            os.remove(temp_tar)
            os.system(f'rm -rf {temp_dir}')
            
            logging.info(f"Successfully copied cache for {instance_id} to {repo_version_dir}")
            
        except Exception as e:
            logging.error(f"Failed to copy cache for {instance_id}: {e}")

def main():
    """Run the SWE-bench benchmark with the Kwaak agent.

    This function orchestrates the benchmark process. It can either:
    1. Run a single instance if --instance is specified
    2. Run a subset of the dataset (first 2 items per repository) by default
    3. Evaluate a specific trial's results if --evaluate and --results-path are specified
    4. Copy kwaak caches from running containers if --copy-caches is specified

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
        --copy-caches: Copy kwaak caches from running containers

    Returns:
        None
    """

    # Configure logging
    logging.basicConfig(level=logging.INFO)
    
    # Clean up any existing processes
    cleanup_processes()
    
    
    # Set up argument parser
    parser = argparse.ArgumentParser(description='Run SWE-bench benchmark with Kwaak agent')
    parser.add_argument('--instance', type=str, help='Instance ID to run a single test case')
    parser.add_argument('--evaluate', type=str, help='Instance ID to evaluate results for')
    parser.add_argument('--results-path', type=str, help='Path to directory containing trial results')
    parser.add_argument('--copy-caches', action='store_true', help='Copy kwaak caches from running containers')
    args = parser.parse_args()
    
    # If evaluating a specific trial
    if args.evaluate:
        if not args.results_path:
            logging.error("--results-path is required when using --evaluate")
            return
        evaluate_trial(args.evaluate, args.results_path)
        return
        
    if args.copy_caches:
        kwaak_version = "0.8.1"
        benchmark_name = f"swe-bench-kwaak-{kwaak_version}"
        copy_kwaak_caches(benchmark_name)
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
