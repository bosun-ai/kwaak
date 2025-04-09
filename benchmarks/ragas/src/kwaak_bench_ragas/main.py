#!/usr/bin/env python3

import argparse
import logging
import sys
from pathlib import Path

from kwaak_bench_ragas.benchmark import RagasBenchmark
from kwaak_bench_ragas.dataset import load_dataset

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(Path("logs/kwaak_bench_ragas.log"))
    ]
)

logger = logging.getLogger(__name__)


def parse_args():
    parser = argparse.ArgumentParser(description="Run RAGAS evaluation on Kwaak agent")
    parser.add_argument(
        "--dataset",
        type=str,
        default="default",
        help="Name of the dataset to use for evaluation"
    )
    parser.add_argument(
        "--results-path",
        type=str,
        default="results",
        help="Path to store evaluation results"
    )
    parser.add_argument(
        "--evaluate",
        type=str,
        help="Evaluate a specific trial"
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        help="Limit the number of instances to evaluate"
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=3600,  # 1 hour
        help="Timeout in seconds for each evaluation"
    )
    return parser.parse_args()


def main():
    args = parse_args()
    
    # Create logs directory if it doesn't exist
    Path("logs").mkdir(exist_ok=True)
    
    # Create results directory if it doesn't exist
    results_path = Path(args.results_path)
    results_path.mkdir(exist_ok=True)
    
    logger.info(f"Starting RAGAS evaluation with dataset: {args.dataset}")
    
    if args.evaluate:
        # Evaluate a specific trial
        logger.info(f"Evaluating trial: {args.evaluate}")
        benchmark = RagasBenchmark(results_path=results_path)
        benchmark.evaluate_trial(args.evaluate)
        return
    
    # Load dataset
    dataset = load_dataset(args.dataset)
    
    if args.limit:
        logger.info(f"Limiting to {args.limit} instances")
        dataset = dataset[:args.limit]
    
    # Run benchmark
    benchmark = RagasBenchmark(results_path=results_path)
    benchmark.run(dataset, timeout=args.timeout)
    
    logger.info("RAGAS evaluation completed successfully")


if __name__ == "__main__":
    main()
