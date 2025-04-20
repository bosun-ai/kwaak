import logging
import typing as t
from dataclasses import dataclass, field
import json
import os

from ragas.metrics import Faithfulness
from ragas.metrics._faithfulness import (
    StatementGeneratorInput,
    StatementGeneratorOutput,
    NLIStatementInput,
    NLIStatementOutput,
    StatementFaithfulnessAnswer
)
from ragas.dataset_schema import SingleTurnSample
from ragas.prompt import PydanticPrompt

logger = logging.getLogger(__name__)

class NLIStatementPrompt(PydanticPrompt[NLIStatementInput, NLIStatementOutput]):
    instruction = """
    Your task is to judge the faithfulness of a statement based on the given context.
    You must return verdict as 1 if the statement can be directly inferred based
     on the context or 0 if the statement can not be directly inferred based on the context. If
     the context contains code, return a verdict as 1 if the statement is a valid inference of the
     resulting functionality of the code.
    """
    input_model = NLIStatementInput
    output_model = NLIStatementOutput
    examples = [
        (
            NLIStatementInput(
                context="""John is a student at XYZ University. He is pursuing a degree in Computer Science. He is enrolled in several courses this semester, including Data Structures, Algorithms, and Database Management. John is a diligent student and spends a significant amount of time studying and completing assignments. He often stays late in the library to work on his projects.""",
                statements=[
                    "John is majoring in Biology.",
                    "John is taking a course on Artificial Intelligence.",
                    "John is a dedicated student.",
                    "John has a part-time job.",
                ],
            ),
            NLIStatementOutput(
                statements=[
                    StatementFaithfulnessAnswer(
                        statement="John is majoring in Biology.",
                        reason="John's major is explicitly mentioned as Computer Science. There is no information suggesting he is majoring in Biology.",
                        verdict=0,
                    ),
                    StatementFaithfulnessAnswer(
                        statement="John is taking a course on Artificial Intelligence.",
                        reason="The context mentions the courses John is currently enrolled in, and Artificial Intelligence is not mentioned. Therefore, it cannot be deduced that John is taking a course on AI.",
                        verdict=0,
                    ),
                    StatementFaithfulnessAnswer(
                        statement="John is a dedicated student.",
                        reason="The context states that he spends a significant amount of time studying and completing assignments. Additionally, it mentions that he often stays late in the library to work on his projects, which implies dedication.",
                        verdict=1,
                    ),
                    StatementFaithfulnessAnswer(
                        statement="John has a part-time job.",
                        reason="There is no information given in the context about John having a part-time job.",
                        verdict=0,
                    ),
                ]
            ),
        ),
        (
            NLIStatementInput(
                context="Photosynthesis is a process used by plants, algae, and certain bacteria to convert light energy into chemical energy.",
                statements=[
                    "Albert Einstein was a genius.",
                ],
            ),
            NLIStatementOutput(
                statements=[
                    StatementFaithfulnessAnswer(
                        statement="Albert Einstein was a genius.",
                        reason="The context and statement are unrelated",
                        verdict=0,
                    )
                ]
            ),
        ),
    ]

@dataclass
class DetailedFaithfulness(Faithfulness):
    """Extended version of RAGAS Faithfulness metric with detailed logging of statements and verdicts."""
    name: str = "detailed_faithfulness"
    log_dir: str = field(default_factory=lambda: os.path.join(os.getcwd(), "faithfulness_logs"))
    
    def __post_init__(self):
        super().__post_init__()
        # Create log directory if it doesn't exist
        os.makedirs(self.log_dir, exist_ok=True)
        
    async def _create_verdicts(self, row: t.Dict, statements: t.List[str], callbacks) -> NLIStatementOutput:
        """Override to log the verdicts for each statement."""
        verdicts = await super()._create_verdicts(row, statements, callbacks)
        
        # Log the verdicts
        logger.info(f"Verdict results for {len(verdicts.statements)} statements:")
        faithful_count = 0
        for i, verdict in enumerate(verdicts.statements):
            faithful = "✓" if verdict.verdict else "✗"
            faithful_count += verdict.verdict
            logger.info(f"  Statement {i+1}: {faithful} - {verdict.statement}")
            logger.info(f"    Reason: {verdict.reason}")
            
        logger.info(f"Faithfulness score: {faithful_count}/{len(verdicts.statements)} = {faithful_count/len(verdicts.statements) if len(verdicts.statements) > 0 else 0}")
        
        # Save verdicts to a file
        # Try to get instance_id from different possible locations
        instance_id = row.get("id", None)
        if instance_id is None and "instance_id" in row:
            instance_id = row["instance_id"]
        if instance_id is None:
            logger.warning(f"Could not find instance_id in row: {row.keys()}")
            instance_id = "unknown"
        logger.info(f"Using instance_id: {instance_id}")
        verdicts_file = os.path.join(self.log_dir, f"{instance_id}_verdicts.json")
        with open(verdicts_file, "w") as f:
            json.dump({
                "question": row["user_input"],
                "contexts": row["retrieved_contexts"],
                "response": row["response"],
                "verdicts": [
                    {
                        "statement": v.statement,
                        "reason": v.reason,
                        "verdict": v.verdict
                    } for v in verdicts.statements
                ]
            }, f, indent=2)
            
        return verdicts
    
    async def _ascore(self, row: t.Dict, callbacks) -> float:
        """Override to provide more detailed logging of the faithfulness calculation process."""
        logger.info(f"Calculating detailed faithfulness for question: {row['user_input'][:100]}...")
        return await super()._ascore(row, callbacks)

# Create an instance of the detailed faithfulness metric
detailed_faithfulness = DetailedFaithfulness()
