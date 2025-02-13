"""Integration tests for the trial module using real Docker."""

import pytest
from kwaak_bench_swe.trial import Trial, TrialResult


def test_trial_with_real_docker(mock_swe_instance, temp_results_dir, mocker):
    """Test trial execution with real Docker but simulated agent changes."""
    trial = Trial(mock_swe_instance, "test-1", temp_results_dir)
    
    try:
        # Mock run_agent to simulate agent making changes
        def mock_run_agent():
            # Make a simple change that would normally be made by the agent
            trial.container.exec("echo 'test change' > /testbed/test.txt")
            # Stage the changes so they'll be included in the git diff
            trial.container.exec("git add /testbed/test.txt")
            trial.container.exec('git commit -m "test change"')
        
        mocker.patch.object(trial, 'run_agent', side_effect=mock_run_agent)
        mocker.patch.object(trial, 'install_agent')  # Skip agent installation
        
        # Run the trial
        result = trial.run()
        
        # Verify the result
        assert isinstance(result, TrialResult)
        assert not result.failed()
        
        # Verify that test.txt was created and its contents are in the diff
        cat_result = trial.container.exec("cat /testbed/test.txt")
        assert cat_result.exit_code == 0
        assert cat_result.output.strip() == "test change"
        
        # The patch in the result should contain our change
        assert "test change" in result.patch
        
    finally:
        # Clean up
        if hasattr(trial, 'container'):
            trial.container.cleanup()
