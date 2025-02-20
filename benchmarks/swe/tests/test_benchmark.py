"""Tests for the benchmark module."""

import os
import json
import tempfile
import pytest

from kwaak_bench_swe.benchmark import Benchmark
from kwaak_bench_swe.swe_bench_instance import SWEBenchInstance
from kwaak_bench_swe.trial import TrialResult




def test_benchmark_initialization(mock_swe_instance, temp_results_dir):
    """Test benchmark initialization and result loading."""
    # Create a benchmark with a single instance
    benchmark = Benchmark("test-bench", [mock_swe_instance], temp_results_dir)

    # Verify that the output directory was created
    assert os.path.exists(os.path.join(temp_results_dir, "test-bench"))

    # Verify that the instance was stored
    assert len(benchmark.instances) == 1
    assert benchmark.instances[0].instance_id == "psf__requests-1142"

    # Verify that results are empty initially
    assert len(benchmark.results) == 0


def test_benchmark_result_persistence(mock_swe_instance, temp_results_dir):
    """Test saving and loading benchmark results."""
    benchmark = Benchmark("test-bench", [mock_swe_instance], temp_results_dir)
    next_run = benchmark.next_run()
    assert next_run is not None
    run_name = next_run["run_name"]

    # Create a test result
    result = TrialResult(
        instance=mock_swe_instance,
        run_failed=False,
        validation_failed=False,
        success=True,
        error=None,
        patch="test patch"
    )

    # Save the result
    benchmark.results[run_name] = result
    os.makedirs(os.path.join(temp_results_dir, "test-bench", mock_swe_instance.instance_id, "1"), exist_ok=True)
    with open(os.path.join(temp_results_dir, "test-bench", mock_swe_instance.instance_id, "1", "result.json"), "w") as f:
        json.dump(result.to_dict(), f, indent=2)

    # Create a new benchmark instance and verify that it loads the existing result
    new_benchmark = Benchmark("test-bench", [mock_swe_instance], temp_results_dir)
    assert run_name in new_benchmark.results
    loaded_result = new_benchmark.results[run_name]
    assert loaded_result.success is True
    assert loaded_result.patch == "test patch"


def test_benchmark_next_run(mock_swe_instance, temp_results_dir):
    """Test finding the next instance to evaluate."""
    benchmark = Benchmark("test-bench", [mock_swe_instance], temp_results_dir)

    # Initially, the instance should be available to run
    next_run = benchmark.next_run()
    assert next_run is not None
    assert next_run["instance"] == mock_swe_instance
    assert next_run["run_name"] == f"{mock_swe_instance.instance_id}-1"

    # After adding a result, there should be no more instances to run
    result = TrialResult(
        instance=mock_swe_instance,
        run_failed=False,
        validation_failed=False,
        success=True,
        error=None,
        patch="test patch"
    )
    benchmark.results[next_run["run_name"]] = result
    assert benchmark.next_run() is None
