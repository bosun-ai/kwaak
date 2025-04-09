#!/usr/bin/env python3

import argparse
import json
import logging
import os
import sys
from pathlib import Path
from typing import Dict, Any

from kwaak_bench_ragas.baseline.model import generate_text_cheap_large_context, generate_text_large_model

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(Path("logs/kwaak_bench_ragas_baseline.log"), mode="a")
    ]
)

logger = logging.getLogger(__name__)


def parse_args():
    parser = argparse.ArgumentParser(description="Run baseline code knowledge extraction system")
    parser.add_argument(
        "directory",
        type=str,
        help="Directory to analyze"
    )
    parser.add_argument(
        "question",
        type=str,
        help="Question to answer about the codebase"
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
    root_path = Path(directory_path)
    
    # First process files at the root level
    root_files = [f for f in root_path.iterdir() if f.is_file()]
    for file_path in sorted(root_files):
        try:
            relative_path = file_path.relative_to(root_path)
            with open(file_path, 'r', encoding='utf-8', errors='replace') as f:
                content = f.read()
            all_files_content.append(f"File: {relative_path}\n\n{content}\n\n")
        except Exception as e:
            logger.warning(f"Error reading file {file_path}: {e}")
    
    # Then process subdirectories recursively
    for subdir in sorted([d for d in root_path.iterdir() if d.is_dir()]):
        for root, _, files in os.walk(subdir):
            root_path_obj = Path(root)
            for filename in sorted(files):
                file_path = root_path_obj / filename
                try:
                    relative_path = file_path.relative_to(root_path)
                    with open(file_path, 'r', encoding='utf-8', errors='replace') as f:
                        content = f.read()
                    all_files_content.append(f"File: {relative_path}\n\n{content}\n\n")
                except Exception as e:
                    logger.warning(f"Error reading file {file_path}: {e}")
    
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


def main() -> Dict[str, Any]:
    # Create logs directory if it doesn't exist
    Path("logs").mkdir(exist_ok=True)
    
    args = parse_args()
    directory = args.directory
    question = args.question
    
    logger.info(f"Starting baseline code knowledge extraction for directory: {directory}")
    logger.info(f"Question: {question}")
    
    try:
        # Read all files in the directory
        all_files_string = read_directory_into_string(directory)
        
        # Extract relevant information
        relevant_info = get_relevant_information(all_files_string, question)
        
        # Answer the question
        answer = answer_question(relevant_info, question)
        
        # Create result
        result = {
            "question": question,
            "answer": answer,
            "context": relevant_info
        }
        
        # Print result
        print(json.dumps(result, indent=2))
        
        # Save result to file
        Path("results").mkdir(exist_ok=True)
        result_file = Path("results/baseline_result.json")
        with open(result_file, "w") as f:
            json.dump(result, f, indent=2)
            
        logger.info(f"Saved result to {result_file}")
        
        return result
        
    except Exception as e:
        logger.error(f"Error in baseline system: {e}")
        print(f"Error: {e}")
        return {
            "question": question,
            "error": str(e)
        }
    
    logger.info("Baseline code knowledge extraction system completed")


if __name__ == "__main__":
    main()
