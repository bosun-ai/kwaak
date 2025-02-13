"""Tests for the Docker instance management module."""

import os
import pytest
import tempfile
from docker import from_env as docker_from_env

from kwaak_bench_swe.docker_instance import DockerInstance, ExecResult

@pytest.fixture(autouse=True)
def cleanup_containers():
    """Clean up any existing containers before each test."""
    client = docker_from_env()
    for container in client.containers.list(all=True):
        if container.name.startswith("sweb.eval."):
            container.remove(force=True)
    yield

def test_docker_instance_initialization(mock_swe_instance, temp_results_dir):
    """Test Docker instance initialization."""
    instance = DockerInstance(mock_swe_instance, temp_results_dir)
    assert instance.instance == mock_swe_instance
    assert instance.instance_dir == os.path.join(temp_results_dir, "container")

def test_docker_instance_run(mock_swe_instance, temp_results_dir):
    """Test Docker container creation and startup."""
    instance = DockerInstance(mock_swe_instance, temp_results_dir)
    
    # Run the container
    instance.run("test-1")
    
    try:
        # Verify container is running
        instance.container.reload()
        assert instance.container.status == "running"
        
        # Verify mount point
        mounts = instance.container.attrs["Mounts"]
        assert len(mounts) == 1
        assert mounts[0]["Type"] == "bind"
        assert mounts[0]["Source"] == instance.instance_dir
        assert mounts[0]["Destination"] == "/tmp"  # Docker API uses 'Destination' instead of 'Target'
        
        # Verify platform
        assert instance.container.attrs["Platform"] == "linux"  # Docker API returns just 'linux'
        
        # Test basic command execution
        result = instance.exec("echo 'hello world'")
        assert result.exit_code == 0
        assert result.output.decode().strip() == "hello world"
    finally:
        instance.cleanup()

def test_docker_instance_write_file(mock_swe_instance, temp_results_dir):
    """Test writing files to the container."""
    instance = DockerInstance(mock_swe_instance, temp_results_dir)
    instance.run("test-1")
    
    try:
        # Write a test file
        test_content = "test content"
        instance.write_string_to_file(test_content, "test.txt")
        
        # Verify file exists and has correct content
        result = instance.exec("cat test.txt")
        assert result.exit_code == 0
        assert result.output.decode().strip() == test_content
    finally:
        instance.cleanup()

def test_docker_instance_exec(mock_swe_instance, temp_results_dir):
    """Test command execution in the container."""
    instance = DockerInstance(mock_swe_instance, temp_results_dir)
    instance.run("test-1")
    
    try:
        # Test successful command
        result = instance.exec("echo 'success'")
        assert result.exit_code == 0
        assert result.output.decode().strip() == "success"
        
        # Test failed command
        result = instance.exec("nonexistent-command")
        assert result.exit_code != 0
    finally:
        instance.cleanup()

def test_docker_instance_cleanup(mock_swe_instance, temp_results_dir):
    """Test container cleanup."""
    instance = DockerInstance(mock_swe_instance, temp_results_dir)
    instance.run("test-1")
    container_id = instance.container.id
    
    # Cleanup the container
    instance.cleanup()
    
    # Verify container no longer exists
    with pytest.raises(Exception):
        instance.client.containers.get(container_id)
