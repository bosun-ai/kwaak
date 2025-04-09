import unittest
from kwaak_bench_ragas.metrics import calculate_ragas_metrics


class TestMetrics(unittest.TestCase):
    def test_calculate_metrics(self):
        # Test with sample data
        query = "What are the main features of Python?"
        contexts = [
            "Python is an interpreted, high-level, general-purpose programming language.",
            "Python features a dynamic type system and automatic memory management.",
            "Python supports multiple programming paradigms, including object-oriented, imperative, functional and procedural."
        ]
        response = "Python is an interpreted, high-level language with dynamic typing, automatic memory management, and support for multiple programming paradigms."
        ground_truth = "Python is an interpreted, high-level language with features including dynamic typing, automatic memory management, and support for multiple programming paradigms such as object-oriented, imperative, functional, and procedural."
        
        metrics = calculate_ragas_metrics(
            query=query,
            contexts=contexts,
            response=response,
            ground_truth=ground_truth
        )
        
        # Check that all expected metrics are present
        self.assertIn("faithfulness", metrics)
        self.assertIn("answer_relevancy", metrics)
        self.assertIn("context_precision", metrics)
        self.assertIn("context_relevancy", metrics)
        self.assertIn("context_recall", metrics)
        
        # Check that all metrics are between 0 and 1
        for metric, value in metrics.items():
            self.assertGreaterEqual(value, 0.0)
            self.assertLessEqual(value, 1.0)


if __name__ == "__main__":
    unittest.main()
