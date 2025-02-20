"""Unit tests for the trial module."""

import os
import pytest
from kwaak_bench_swe.trial import Trial, TrialResult
from swebench.harness.test_spec.test_spec import TestSpec


def test_trial_result_failed():
    """Test the failed() method of TrialResult."""
    # Test with no failures
    result = TrialResult(instance=None)
    assert not result.failed()

    # Test with run failure
    result = TrialResult(instance=None, run_failed=True)
    assert result.failed()

    # Test with validation failure
    result = TrialResult(instance=None, validation_failed=True)
    assert result.failed()

    # Test with error
    result = TrialResult(instance=None, error="Test error")
    assert result.failed()


def test_trial_result_to_dict(mock_swe_instance):
    """Test the to_dict() method of TrialResult."""
    result = TrialResult(
        instance=mock_swe_instance,
        run_failed=False,
        validation_failed=False,
        error=None,
        patch="test patch"
    )
    
    result_dict = result.to_dict()
    assert isinstance(result_dict, dict)
    assert result_dict["instance"]["repo"] == "psf/requests"
    assert result_dict["patch"] == "test patch"
    assert not result_dict["run_failed"]
    assert not result_dict["validation_failed"]
    assert result_dict["error"] is None


def test_trial_initialization(mock_swe_instance, temp_results_dir):
    """Test Trial class initialization."""
    trial = Trial(mock_swe_instance, "test-1", temp_results_dir)
    assert trial.item == mock_swe_instance
    assert trial.name == "test-1"
    assert trial.results_dir == temp_results_dir


def test_trial_establish_git_ref(mock_swe_instance, temp_results_dir, mock_docker_instance, mocker):
    """Test establishing initial git reference."""
    trial = Trial(mock_swe_instance, "test-1", temp_results_dir)
    trial.container = mock_docker_instance.container
    
    # Mock successful git command
    mock_docker_instance.container.exec.return_value = mocker.Mock(
        output=b"test-hash\n",
        exit_code=0
    )
    
    ref = trial.establish_initial_git_ref()
    assert ref == "test-hash"
    
    # Verify git commands were called
    calls = mock_docker_instance.container.exec.call_args_list
    assert len(calls) == 5  # git config name, email, commit, add, and rev-parse
    assert "git config user.name" in calls[0].args[0]
    assert "git config user.email" in calls[1].args[0]
    assert "git add" in calls[2].args[0]
    assert "git commit" in calls[3].args[0]
    assert "git rev-parse" in calls[4].args[0]


def test_trial_run(mock_swe_instance, temp_results_dir, mock_docker_instance, mocker):
    """Test the complete trial run."""
    trial = Trial(mock_swe_instance, "test-1", temp_results_dir)
    trial.container = mock_docker_instance.container
    
    # Mock environment variables
    mocker.patch.dict(os.environ, {'OPENAI_API_KEY': 'test-key'})
    
    # Mock successful container execution
    exec_mock = mocker.Mock()
    exec_mock.side_effect = [
        mocker.Mock(output=b"test output\n", exit_code=0),  # test patch
        mocker.Mock(output=b"test output\n", exit_code=0),  # git config
        mocker.Mock(output=b"test output\n", exit_code=0),  # git add
        mocker.Mock(output=b"test output\n", exit_code=0),  # git commit
        mocker.Mock(output=b"test output\n", exit_code=0),  # git rev-parse
    ]
    mock_docker_instance.container.exec = exec_mock
    
    # Mock methods
    mocker.patch.object(trial, 'establish_initial_git_ref', return_value="test-hash")
    mocker.patch.object(trial, 'install_agent')
    mocker.patch.object(trial, 'run_agent')
    mocker.patch.object(trial, 'evaluate_results', return_value=TrialResult(
        instance=mock_swe_instance,
        success=True,
        patch="test patch"
    ))
    
    result = trial.run()
    assert isinstance(result, TrialResult)
    assert result.success
    assert not result.failed()
    assert result.patch == "test patch"


def test_trial_run_with_error(mock_swe_instance, temp_results_dir, mock_docker_instance, mocker):
    """Test trial run with an error during execution."""
    trial = Trial(mock_swe_instance, "test-1", temp_results_dir)
    trial.container = mock_docker_instance.container
    
    # Mock container.exec to fail when applying patch
    mock_docker_instance.container.exec.return_value = mocker.Mock(
        output=b"Test error",
        exit_code=1
    )
    
    result = trial.run()
    assert isinstance(result, TrialResult)
    assert result.failed()
    assert result.error == "Patch failed: Test error"


def test_trial_invoke_kwaak_timeout(mock_swe_instance, temp_results_dir, mock_docker_instance, mocker):
    """Test that invoke_kwaak handles timeouts properly."""
    import time
    
    # Mock environment variables
    mocker.patch.dict(os.environ, {'OPENAI_API_KEY': 'test-key'})
    
    trial = Trial(mock_swe_instance, "test-1", temp_results_dir)
    trial.container = mock_docker_instance.container
    
    # Mock a long-running command that will timeout
    def mock_exec(*args, **kwargs):
        raise TimeoutError("Command timed out")
    
    mock_docker_instance.container.exec.side_effect = mock_exec
    
    # Run invoke_kwaak
    trial.invoke_kwaak()
    
    # Check that timeout message was written to agent_result.txt
    agent_result_path = os.path.join(temp_results_dir, "agent_result.txt")
    with open(agent_result_path, "r") as f:
        content = f.read()
        assert content == "Error: Command timed out"
