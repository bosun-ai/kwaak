# Kwaak RAGAS Evaluation Runner

A Python package for evaluating the Kwaak agent using RAGAS (Retrieval Augmented Generation Assessment), a framework for evaluating RAG systems.

## Overview

This package provides a complete test harness for evaluating the Kwaak agent using RAGAS:
- Loads and processes evaluation datasets
- Executes evaluation with proper environment setup
- Evaluates results using RAGAS metrics
- Generates comprehensive evaluation reports

## Usage

Requires Python 3.11 or higher.

Run the evaluation using uv:
```bash
uv run kwaak-bench-ragas
```

This will:
1. Load the evaluation dataset
2. For each test case:
   - Set up the evaluation environment
   - Run the Kwaak agent
   - Collect responses
   - Evaluate using RAGAS metrics
3. Generate evaluation reports

### Command-line Options

```bash
# Run a specific evaluation dataset
uv run kwaak-bench-ragas --dataset <dataset_name>

# Evaluate results for a specific trial
uv run kwaak-bench-ragas --evaluate <trial_name> --results-path /path/to/results
```

## Project Structure

- `src/kwaak_bench_ragas/`
  - `main.py` - Entry point and evaluation orchestration
  - `benchmark.py` - Benchmark runner and result management
  - `evaluation.py` - Evaluation execution and metrics collection
  - `dataset.py` - Dataset representation and loading
  - `metrics.py` - RAGAS metrics implementation

## Output

The evaluation generates several outputs in the `results` directory:
1. `{evaluation-name}/{trial-name}.json` - Detailed trial results
2. `{evaluation-name}/{trial-name}-metrics.json` - RAGAS metrics results
3. `{evaluation-name}/{trial-name}-report.json` - Comprehensive evaluation report
4. `{evaluation-name}/{trial-name}/agent_result.txt` - Kwaak agent output

## Development

### Contributing
1. Ensure all code is properly typed
2. Maintain JSON serialization support for result objects
3. Follow the existing pattern of using dataclasses for data structures
4. Add new RAGAS metrics as needed in the metrics module
