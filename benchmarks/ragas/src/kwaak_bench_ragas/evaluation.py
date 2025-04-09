import json
import logging
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

from kwaak_bench_ragas.metrics import calculate_ragas_metrics

logger = logging.getLogger(__name__)


@dataclass
class EvaluationResult:
    """Results of a RAGAS evaluation run."""
    instance_id: str
    query: str
    context: List[str]
    response: Optional[str] = None
    ground_truth: Optional[str] = None
    start_time: float = field(default_factory=time.time)
    end_time: Optional[float] = None
    duration: Optional[float] = None
    metrics: Dict[str, float] = field(default_factory=dict)
    error: Optional[str] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "instance_id": self.instance_id,
            "query": self.query,
            "context": self.context,
            "response": self.response,
            "ground_truth": self.ground_truth,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "duration": self.duration,
            "metrics": self.metrics,
            "error": self.error,
        }


class RagasEvaluation:
    """RAGAS evaluation runner for a single instance."""
    
    def __init__(self, instance: Dict[str, Any], output_dir: Path, timeout: int = 3600):
        self.instance = instance
        self.output_dir = output_dir
        self.timeout = timeout
        self.output_dir.mkdir(exist_ok=True)
    
    def run(self) -> EvaluationResult:
        """Run the evaluation for a single instance."""
        instance_id = self.instance.get("id", str(hash(json.dumps(self.instance, sort_keys=True))))
        query = self.instance.get("query", "")
        context = self.instance.get("context", [])
        ground_truth = self.instance.get("ground_truth")
        
        result = EvaluationResult(
            instance_id=instance_id,
            query=query,
            context=context,
            ground_truth=ground_truth,
        )
        
        try:
            # Run the Kwaak agent to get a response
            response = self._run_kwaak_agent()
            result.response = response
            
            # Calculate RAGAS metrics
            if response:
                metrics = calculate_ragas_metrics(
                    query=query,
                    contexts=context,
                    response=response,
                    ground_truth=ground_truth
                )
                result.metrics = metrics
            
            # Record end time and duration
            result.end_time = time.time()
            result.duration = result.end_time - result.start_time
            
        except Exception as e:
            logger.error(f"Error in evaluation: {e}")
            result.error = str(e)
            result.end_time = time.time()
            result.duration = result.end_time - result.start_time
        
        # Save agent output
        if result.response:
            agent_output_path = self.output_dir / "agent_result.txt"
            with open(agent_output_path, "w") as f:
                f.write(result.response)
        
        return result
    
    def _run_kwaak_agent(self) -> str:
        """Run the Kwaak agent and return its response."""
        # For now, we'll use the ground truth as the response if available
        # This allows us to evaluate the baseline system's output against itself
        ground_truth = self.instance.get('ground_truth')
        if ground_truth:
            logger.info(f"Using ground truth as response for query: {self.instance.get('query', '')}")
            return ground_truth
        
        # If no ground truth is available, we could run the baseline system
        # This would require importing and calling the baseline system
        try:
            from kwaak_bench_ragas.baseline.main import read_directory_into_string, get_relevant_information, answer_question
            
            query = self.instance.get('query', '')
            context_string = '\n'.join(self.instance.get('context', []))
            
            logger.info(f"Running baseline system for query: {query}")
            relevant_info = get_relevant_information(context_string, query)
            response = answer_question(relevant_info, query)
            
            return response
        except Exception as e:
            logger.error(f"Error running baseline system: {e}")
            return f"Error generating response: {str(e)}"
    
    def generate_report(self, result: EvaluationResult) -> Dict[str, Any]:
        """Generate a comprehensive evaluation report."""
        report = {
            "instance_id": result.instance_id,
            "query": result.query,
            "metrics": result.metrics,
            "duration": result.duration,
            "summary": self._generate_summary(result),
            "timestamp": time.time(),
        }
        
        return report
    
    def _generate_summary(self, result: EvaluationResult) -> Dict[str, Any]:
        """Generate a summary of the evaluation results."""
        metrics = result.metrics
        
        # Calculate overall score (simple average for now)
        overall_score = sum(metrics.values()) / len(metrics) if metrics else 0
        
        return {
            "overall_score": overall_score,
            "strengths": self._identify_strengths(metrics),
            "weaknesses": self._identify_weaknesses(metrics),
            "recommendations": self._generate_recommendations(metrics),
        }
    
    def _identify_strengths(self, metrics: Dict[str, float]) -> List[str]:
        """Identify strengths based on metrics."""
        strengths = []
        
        # Example logic - in a real implementation this would be more sophisticated
        for metric, value in metrics.items():
            if value > 0.8:  # Arbitrary threshold
                strengths.append(f"Strong performance in {metric}: {value:.2f}")
        
        return strengths
    
    def _identify_weaknesses(self, metrics: Dict[str, float]) -> List[str]:
        """Identify weaknesses based on metrics."""
        weaknesses = []
        
        # Example logic - in a real implementation this would be more sophisticated
        for metric, value in metrics.items():
            if value < 0.5:  # Arbitrary threshold
                weaknesses.append(f"Weak performance in {metric}: {value:.2f}")
        
        return weaknesses
    
    def _generate_recommendations(self, metrics: Dict[str, float]) -> List[str]:
        """Generate recommendations based on metrics."""
        recommendations = []
        
        # Example logic - in a real implementation this would be more sophisticated
        if metrics.get("faithfulness", 1.0) < 0.7:
            recommendations.append("Improve faithfulness by ensuring responses are grounded in the provided context")
        
        if metrics.get("answer_relevancy", 1.0) < 0.7:
            recommendations.append("Enhance answer relevancy by focusing more directly on the query")
        
        if metrics.get("context_precision", 1.0) < 0.7:
            recommendations.append("Improve context precision by retrieving more relevant documents")
        
        return recommendations
