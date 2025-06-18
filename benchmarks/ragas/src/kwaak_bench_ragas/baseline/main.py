#!/usr/bin/env python3

import argparse
import json
import logging
import os
import subprocess
import sys
from pathlib import Path
from typing import Dict, Any, List, Optional
from binaryornot.check import is_binary

from kwaak_bench_ragas.baseline.model import generate_text_cheap_large_context, generate_text_large_model

# Create logs directory if it doesn't exist
Path("logs").mkdir(exist_ok=True)

log_level = os.environ.get("LOG_LEVEL", "INFO").upper()

logging.basicConfig(
level=log_level,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(Path("logs/kwaak_bench_ragas_baseline.log"), mode="a")
    ]
)

logger = logging.getLogger(__name__)


def parse_args():
    parser = argparse.ArgumentParser(description="Run baseline code knowledge extraction system")
    
    # Add required arguments
    parser.add_argument(
        "--directory",
        type=str,
        required=True,
        help="Directory to analyze"
    )
    
    # Add mutually exclusive group for single question vs dataset
    mode_group = parser.add_mutually_exclusive_group(required=True)
    
    mode_group.add_argument(
        "--dataset",
        type=str,
        help="Path to a RAGAS dataset JSON file"
    )
    
    mode_group.add_argument(
        "--question",
        type=str,
        help="Question to answer about the codebase (for single question mode)"
    )
    

    
    parser.add_argument(
        "--record-ground-truths",
        action="store_true",
        help="Record answers in the ground_truths field instead of the answer field"
    )
    
    return parser.parse_args()


def read_directory_into_string(directory_path: str) -> str:
    """Read all files in a directory recursively into a single string.

    Args:
        directory_path: Path to the directory to read
        
    Returns:
        String containing all file contents with relative paths
    """
    logger.info(f"Reading directory: {directory_path}")
    
    all_files_content = []
    all_files = subprocess.check_output(["git", "ls-files", directory_path], text=True).splitlines()

    for file in all_files:
        try:
            if is_binary(file):
                continue
            with open(file, 'tr', encoding='utf-8', errors='replace') as f:
                content = f.read()
            all_files_content.append(f"File: {file}\n\n{content}\n\n")
        except Exception as e:
            logger.warning(f"Error reading file {file}: {e}")
    
    return "\n\n".join(all_files_content)


def get_relevant_information(all_files_string: str, question: str) -> str:
    """Extract relevant information from files based on a question.
    
    Args:
        all_files_string: String containing all file contents
        question: Question to answer
        
    Returns:
        Extracted relevant information
    """
    logger.info("Extracting relevant information for the question")
    
    prompt = f"We are considering the following question:\n\n<question>{question}</question>\n\nExtract all information that is relevant to the question from the following files:\n\n{all_files_string}"
    
    try:
        return generate_text_cheap_large_context(prompt)
    except Exception as e:
        logger.error(f"Error extracting relevant information: {e}")
        raise


def answer_question(relevant_information: str, question: str) -> str:
    """Answer a question based on relevant information.
    
    Args:
        relevant_information: Extracted relevant information
        question: Question to answer
        
    Returns:
        Answer to the question
    """
    logger.info("Answering question using relevant information")
    
    prompt = f"Given the following information:\n\n{relevant_information}\n\nAnswer the following question: \n\n#{question}"
    
    try:
        return generate_text_large_model(prompt)
    except Exception as e:
        logger.error(f"Error answering question: {e}")
        raise


def process_question(directory: str, question: str) -> str:
    """Process a single question using the baseline system.
    
    Args:
        directory: Directory to analyze
        question: Question to answer
        
    Returns:
        Answer to the question
    """
    logger.info(f"Processing question: {question} for directory: {directory}")
    
    # Read all files in the directory
    all_files_string = read_directory_into_string(directory)
    
    # Extract relevant information
    relevant_info = get_relevant_information(all_files_string, question)
    logger.info("Answering question using relevant information")
    
    # Answer the question
    answer = answer_question(relevant_info, question)
    
    return answer


def load_dataset(dataset_path: str) -> List[Dict[str, Any]]:
    """Load a RAGAS dataset from a JSON file.
    
    Args:
        dataset_path: Path to the dataset JSON file
        
    Returns:
        List of dataset instances
    """
    logger.info(f"Loading dataset from {dataset_path}")
    
    try:
        with open(dataset_path, 'r') as f:
            dataset = json.load(f)
        
        # Validate dataset format
        if not isinstance(dataset, list):
            raise ValueError("Dataset must be a list of instances")
            
        for instance in dataset:
            if "question" not in instance and "query" not in instance:
                raise ValueError("Each instance must have a 'question' field")
                
        return dataset
    
    except Exception as e:
        logger.error(f"Error loading dataset: {e}")
        raise


def process_dataset(dataset_path: str, directory: str, record_ground_truths: bool = False) -> str:
    """Process a dataset using the baseline system.
    
    Args:
        dataset_path: Path to the dataset
        directory: Directory to analyze
        record_ground_truths: Whether to record answers as ground truths
        
    Returns:
        Path to the processed dataset
    """
    logger.info(f"Processing dataset from {dataset_path} for directory: {directory}")
    
    # Create results directory if it doesn't exist
    Path("results").mkdir(exist_ok=True)
    
    # Load the dataset
    logger.info(f"Loading dataset from {dataset_path}")
    with open(dataset_path, 'r') as f:
        dataset = json.load(f)
    
    # Process each instance
    for instance in dataset:
        question = instance.get('question')
        if not question:
            logger.warning(f"Skipping instance without question: {instance}")
            continue
        
        answer = process_question(directory, question)
        
        # Store the answer in the appropriate field
        if record_ground_truths:
            if 'ground_truths' not in instance:
                instance['ground_truths'] = []
            instance['ground_truths'].append(answer)
        else:
            instance['answer'] = answer
    
    # Save the processed dataset
    output_path = f"results/{Path(dataset_path).stem}_processed.json"
    with open(output_path, 'w') as f:
        json.dump(dataset, f, indent=2)
    
    logger.info(f"Saved processed dataset to {output_path}")
    
    return output_path


def main() -> Dict[str, Any]:
    # Create results directory if it doesn't exist
    Path("results").mkdir(exist_ok=True)
    
    args = parse_args()
    
    if args.dataset:
        # Process dataset mode
            
        logger.info(f"Starting baseline processing for dataset: {args.dataset}")
        
        # Process dataset
        output_path = process_dataset(
            args.dataset,
            args.directory,
            args.record_ground_truths
        )
        
        # Load the processed dataset to count instances
        with open(output_path, 'r') as f:
            processed_dataset = json.load(f)
        instance_count = len(processed_dataset)
            
        print(f"Processed {instance_count} instances. Results saved to {output_path}")
        
        return {
            "dataset": output_path,
            "instances": instance_count
        }
        
    else:
        # Single question mode
            
        logger.info(f"Starting baseline code knowledge extraction for directory: {args.directory}")
        logger.info(f"Question: {args.question}")
        
        # Process single question
        answer = process_question(args.directory, args.question)
        
        # Create result dictionary
        result = {
            "question": args.question,
            "answer": answer
        }
        
        # Print result
        print(json.dumps(result, indent=2))
        
        # Save result to file
        result_file = Path("results/baseline_result.json")
        with open(result_file, "w") as f:
            json.dump(result, f, indent=2)
                
        logger.info(f"Saved result to {result_file}")
        
        return result


if __name__ == "__main__":
    main()
