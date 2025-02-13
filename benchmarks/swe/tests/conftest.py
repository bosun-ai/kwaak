"""Common pytest fixtures for the test suite."""

import pytest
from pathlib import Path
from datasets import load_dataset
from kwaak_bench_swe.swe_bench_instance import SWEBenchInstance
from kwaak_bench_swe.docker_instance import DockerInstance

@pytest.fixture
def temp_results_dir(tmp_path):
    """Create a temporary directory for test results."""
    results_dir = tmp_path / "results"
    results_dir.mkdir()
    return str(results_dir)

DATASET = load_dataset("princeton-nlp/SWE-bench_Verified", split="test")
INSTANCE_ITEM = next(item for item in DATASET if item["instance_id"] == "psf__requests-1142")

@pytest.fixture
def mock_swe_instance():
    """Create a mock SWE-bench instance for testing using psf/requests-1142 from the dataset."""
    return SWEBenchInstance.from_dataset([INSTANCE_ITEM])[0]

@pytest.fixture
def mock_docker_instance(mock_swe_instance, temp_results_dir, mocker):
    """Create a mock Docker instance that doesn't actually create containers."""
    # Mock the docker client and container
    mock_container = mocker.MagicMock()
    mock_container.exec_run.return_value.output = b"test output"
    mock_container.exec_run.return_value.exit_code = 0
    
    mocker.patch('docker.from_env')
    
    instance = DockerInstance(mock_swe_instance, temp_results_dir)
    instance.container = mock_container
    return instance
