from dataclasses import dataclass, asdict
from typing import Any
import json

@dataclass
class SWEBenchInstance:
    repo: str
    instance_id: str
    base_commit: str
    patch: str
    test_patch: str
    problem_statement: str
    hints_text: str
    created_at: str
    version: str
    FAIL_TO_PASS: list[str]
    PASS_TO_PASS: list[str]
    environment_setup_commit: str

    def to_dict(self) -> dict[str, Any]:
        """Convert the instance to a dictionary for JSON serialization."""
        return asdict(self)

    # class method from_dataset iterates over the dataset and returns a list of SWEBenchInstance objects
    @classmethod
    def from_dataset(cls, dataset):
        return [
            cls(
                **{
                    **item,
                    "FAIL_TO_PASS": json.loads(item["FAIL_TO_PASS"]),
                    "PASS_TO_PASS": json.loads(item["PASS_TO_PASS"])
                }
            )
            for item in dataset
        ]

