import json
import logging
from pathlib import Path
from typing import Any, Dict, List, Optional

import datasets

logger = logging.getLogger(__name__)


def load_dataset(dataset_name: str, split: str = "test") -> List[Dict[str, Any]]:
    """Load a dataset for RAGAS evaluation.
    
    Args:
        dataset_name: Name of the dataset to load
        split: Dataset split to use (default: "test")
        
    Returns:
        List of instances for evaluation
    """
    logger.info(f"Loading dataset: {dataset_name}")
    
    # Check if it's a local dataset file
    local_path = Path(f"datasets/{dataset_name}.json")
    if local_path.exists():
        logger.info(f"Loading local dataset from {local_path}")
        return _load_local_dataset(local_path)
    
    # Try to load from Hugging Face datasets
    try:
        logger.info(f"Attempting to load dataset from Hugging Face: {dataset_name}")
        return _load_hf_dataset(dataset_name, split)
    except Exception as e:
        logger.warning(f"Failed to load dataset from Hugging Face: {e}")
    
    # Fall back to default dataset
    if dataset_name != "default":
        logger.warning(f"Dataset {dataset_name} not found, falling back to default")
        return load_dataset("default")
    
    # Create a simple default dataset
    logger.info("Creating default sample dataset")
    return _create_default_dataset()


def _load_local_dataset(path: Path) -> List[Dict[str, Any]]:
    """Load a dataset from a local JSON file."""
    with open(path, "r") as f:
        data = json.load(f)
    
    # Ensure the dataset has the expected format
    if isinstance(data, list):
        # Add dataset_name to each instance if not present
        dataset_name = path.stem
        for item in data:
            if "dataset_name" not in item:
                item["dataset_name"] = dataset_name
        return data
    elif isinstance(data, dict) and "instances" in data:
        # Extract instances and add dataset_name if not present
        dataset_name = data.get("name", path.stem)
        instances = data["instances"]
        for item in instances:
            if "dataset_name" not in item:
                item["dataset_name"] = dataset_name
        return instances
    else:
        raise ValueError(f"Invalid dataset format in {path}")


def _load_hf_dataset(dataset_name: str, split: str) -> List[Dict[str, Any]]:
    """Load a dataset from Hugging Face datasets."""
    dataset = datasets.load_dataset(dataset_name, split=split)
    
    # Convert to list of dictionaries
    instances = []
    for item in dataset:
        instance = dict(item)
        if "dataset_name" not in instance:
            instance["dataset_name"] = dataset_name
        instances.append(instance)
    
    return instances


def _create_default_dataset() -> List[Dict[str, Any]]:
    """Create a default sample dataset for testing."""
    return [
        {
            "id": "sample-1",
            "dataset_name": "default",
            "question": "What are the main features of Python?",
            "contexts": [
                "Python is an interpreted, high-level, general-purpose programming language.",
                "Python features a dynamic type system and automatic memory management.",
                "Python supports multiple programming paradigms, including object-oriented, imperative, functional and procedural."
            ],
            "ground_truths": ["Python is an interpreted, high-level language with features including dynamic typing, automatic memory management, and support for multiple programming paradigms such as object-oriented, imperative, functional, and procedural."]
        },
        {
            "id": "sample-2",
            "dataset_name": "default",
            "question": "How does garbage collection work in Python?",
            "contexts": [
                "Python uses reference counting for memory management.",
                "When the reference count of an object drops to zero, it is garbage collected.",
                "Python also has a cyclic garbage collector that can detect and collect circular references."
            ],
            "ground_truths": ["Python uses reference counting for memory management, where objects are garbage collected when their reference count drops to zero. It also has a cyclic garbage collector to handle circular references."]
        }
    ]


def save_dataset(instances: List[Dict[str, Any]], name: str) -> Path:
    """Save a dataset to a local file.
    
    Args:
        instances: List of instances to save
        name: Name of the dataset
        
    Returns:
        Path to the saved dataset file
    """
    # Create datasets directory if it doesn't exist
    datasets_dir = Path("datasets")
    datasets_dir.mkdir(exist_ok=True)
    
    # Save dataset
    path = datasets_dir / f"{name}.json"
    with open(path, "w") as f:
        json.dump(instances, f, indent=2)
    
    logger.info(f"Saved dataset to {path}")
    return path
