import json
import logging
import random
import string
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Any

from kwaak_bench_ragas.evaluation import RagasEvaluation

logger = logging.getLogger(__name__)


@dataclass
class BenchmarkResult:
    """Results of a RAGAS benchmark run."""
    dataset_name: str
    start_time: datetime
    end_time: Optional[datetime] = None
    trials: Dict[str, Dict[str, Any]] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "dataset_name": self.dataset_name,
            "start_time": self.start_time.isoformat(),
            "end_time": self.end_time.isoformat() if self.end_time else None,
            "trials": self.trials,
        }
    
    def save(self, path: Path) -> None:
        """Save benchmark results to a JSON file."""
        with open(path, "w") as f:
            json.dump(self.to_dict(), f, indent=2)


class RagasBenchmark:
    """Benchmark runner for RAGAS evaluation."""
    
    def __init__(self, results_path: Path):
        self.results_path = results_path
        self.results_path.mkdir(exist_ok=True)
    
    def run(self, dataset: List[Dict[str, Any]], timeout: int = 3600) -> BenchmarkResult:
        """Run the benchmark on the given dataset."""
        dataset_name = dataset[0].get("dataset_name", "default") if dataset else "default"
        benchmark_dir = self.results_path / dataset_name
        benchmark_dir.mkdir(exist_ok=True)
        
        result = BenchmarkResult(
            dataset_name=dataset_name,
            start_time=datetime.now(),
        )
        
        logger.info(f"Starting benchmark with {len(dataset)} instances")
        
        for instance in dataset:
            # Generate a short, readable ID if not provided
            if "id" in instance:
                instance_id = instance["id"]
            else:
                # Generate an 8-character random string
                chars = string.ascii_letters + string.digits
                instance_id = ''.join(random.choice(chars) for _ in range(8))
                
            logger.info(f"Running evaluation for instance: {instance_id}")
            
            trial_dir = benchmark_dir / instance_id
            trial_dir.mkdir(exist_ok=True)
            
            evaluation = RagasEvaluation(
                instance=instance,
                output_dir=trial_dir,
                timeout=timeout
            )
            
            try:
                trial_result = evaluation.run()
                result.trials[instance_id] = trial_result.to_dict()
                
                # Save individual trial result
                trial_result_path = benchmark_dir / f"{instance_id}.json"
                with open(trial_result_path, "w") as f:
                    json.dump(trial_result.to_dict(), f, indent=2)
                
                # Save metrics separately for easy access
                metrics_path = benchmark_dir / f"{instance_id}-metrics.json"
                with open(metrics_path, "w") as f:
                    json.dump(trial_result.metrics, f, indent=2)
                
                # Generate and save report
                report_path = benchmark_dir / f"{instance_id}-report.json"
                report = evaluation.generate_report(trial_result)
                with open(report_path, "w") as f:
                    json.dump(report, f, indent=2)
                
            except Exception as e:
                logger.error(f"Error running evaluation for {instance_id}: {e}")
                result.trials[instance_id] = {"error": str(e)}
        
        result.end_time = datetime.now()
        
        # Save overall benchmark results
        result_path = self.results_path / f"{dataset_name}_results.json"
        result.save(result_path)
        
        logger.info(f"Benchmark completed. Results saved to {result_path}")
        return result
    
    def evaluate_trial(self, trial_id: str) -> Dict[str, Any]:
        """Evaluate a specific trial from saved results."""
        # Find the trial results
        for dataset_dir in self.results_path.iterdir():
            if not dataset_dir.is_dir():
                continue
            
            trial_path = dataset_dir / f"{trial_id}.json"
            if trial_path.exists():
                logger.info(f"Found trial results at {trial_path}")
                
                with open(trial_path, "r") as f:
                    trial_data = json.load(f)
                
                # Re-evaluate metrics if needed
                # This could be implemented to recalculate metrics with updated algorithms
                
                logger.info(f"Evaluation for trial {trial_id} completed")
                return trial_data
        
        logger.error(f"Trial {trial_id} not found")
        return {"error": f"Trial {trial_id} not found"}
