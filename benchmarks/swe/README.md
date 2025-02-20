# Kwaak SWE-Bench Runner

A Python package for running and evaluating the Kwaak agent against the SWE-bench dataset, a benchmark for evaluating LLMs on real-world software engineering tasks.

## Overview

This package provides a complete test harness for evaluating the Kwaak agent against SWE-bench:
- Loads and processes the SWE-bench dataset
- Manages Docker containers for isolated test environments
- Executes test cases with proper environment setup
- Evaluates and grades test results
- Generates submission-ready predictions

## Usage

Requires Python 3.11 or higher and Docker.

Run the benchmark using uv:
```bash
uv run kwaak-bench-swe
```

This will:
1. Load the SWE-bench test dataset
2. Take the first 2 items from each repository
3. For each test case:
   - Create an isolated Docker container
   - Set up the test environment
   - Apply test patches
   - Run the Kwaak agent (with 60-minute timeout)
   - Execute test suite
   - Evaluate results
4. Generate predictions in SWE-bench submission format

### Command-line Options

```bash
# Run a specific test case
uv run kwaak-bench-swe --instance psf__requests-2317

# Evaluate results for a specific trial
uv run kwaak-bench-swe --evaluate psf__requests-2317 --results-path /path/to/results
```

## Project Structure

- `src/kwaak_bench_swe/`
  - `main.py` - Entry point and benchmark orchestration
  - `benchmark.py` - Benchmark runner and result management
  - `trial.py` - Test execution and evaluation
  - `swe_bench_instance.py` - SWE-bench test case representation
  - `docker_instance.py` - Docker container management

## Output

The benchmark generates several outputs in the `results` directory:
1. `{benchmark-name}/{trial-name}.json` - Detailed trial results
2. `{benchmark-name}/{trial-name}-pre_patch_test_results.txt` - Initial test results
3. `{benchmark-name}/{trial-name}-test_results.txt` - Post-patch test results
4. `{benchmark-name}/{trial-name}-patch.diff` - Generated patch
5. `{benchmark-name}/{trial-name}-report.json` - Evaluation report
6. `{benchmark-name}/{trial-name}/agent_result.txt` - Kwaak agent output or timeout message
7. `predictions.jsonl` - SWE-bench submission format predictions

## Development

### Contributing
1. Ensure all code is properly typed
2. Maintain JSON serialization support for result objects
3. Follow the existing pattern of using dataclasses for data structures
4. Test Docker container isolation when making changes to test execution
