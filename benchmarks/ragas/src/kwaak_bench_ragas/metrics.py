import logging
from typing import Dict, List, Optional
import os

# Set OpenAI API key for RAGAS if available
openai_api_key = os.environ.get('OPENAI_API_KEY') or os.environ.get('OPENROUTER_API_KEY')
if openai_api_key:
    os.environ['OPENAI_API_KEY'] = openai_api_key
    logging.info("Using API key from environment for RAGAS")

# Import the class-based metrics from the current RAGAS version
from ragas.metrics import (
    Faithfulness,
    AnswerRelevancy,
    ContextPrecision,
    ContextRecall,
    ContextRelevance
)
from ragas import SingleTurnSample
    
# Import LLM wrapper and embeddings for RAGAS
from langchain_openai import ChatOpenAI, OpenAIEmbeddings
from ragas.llms import LangchainLLMWrapper
from ragas.embeddings import LangchainEmbeddingsWrapper

# Initialize LLM and embeddings if API key is available
if openai_api_key:
    # Create LLM instance for RAGAS
    llm = ChatOpenAI(
        api_key=openai_api_key,
        model="gpt-3.5-turbo",
        temperature=0
    )
    ragas_llm = LangchainLLMWrapper(llm)
    
    # Create embeddings instance for RAGAS
    embeddings = OpenAIEmbeddings(
        api_key=openai_api_key,
        model="text-embedding-ada-002"
    )
    ragas_embeddings = LangchainEmbeddingsWrapper(embeddings)
    logging.info("Successfully initialized LLM and embeddings for RAGAS")
else:
    logging.error("No OpenAI API key available, metrics will be mocked")
    exit(1)


logger = logging.getLogger(__name__)


def calculate_ragas_metrics(
    query: str,
    contexts: List[str],
    response: str,
    ground_truth: Optional[str] = None
) -> Dict[str, float]:
    """Calculate RAGAS metrics for a single instance.
    
    Args:
        query: The question (using query parameter for backward compatibility)
        contexts: List of context documents
        response: The generated answer
        ground_truth: Optional ground truth answer
        
    Returns:
        Dictionary of metric names and scores
    """
    logger.info(f"Calculating RAGAS metrics for question: {query[:50]}...")
    
    metrics = {}
    
    # Create a SingleTurnSample for evaluation using the field names that RAGAS expects
    sample = SingleTurnSample(
        user_input=query,  # RAGAS still expects user_input internally
        retrieved_contexts=contexts,
        response=response,
        reference=ground_truth if ground_truth else None
    )
    
    # Initialize metrics with LLM and embeddings as needed
    faithfulness_metric = Faithfulness(llm=ragas_llm)
    answer_relevancy_metric = AnswerRelevancy(llm=ragas_llm, embeddings=ragas_embeddings)
    context_precision_metric = ContextPrecision(llm=ragas_llm)
    context_recall_metric = ContextRecall(llm=ragas_llm)
    
    # Try to use ContextRelevance if available, otherwise use ContextPrecision
    try:
        context_relevancy_metric = ContextRelevance(llm=ragas_llm)
    except Exception:
        context_relevancy_metric = ContextPrecision(llm=ragas_llm)
        
    # Calculate faithfulness
    try:
        faith_score = faithfulness_metric.single_turn_score(sample)
        metrics["faithfulness"] = faith_score
        logger.info(f"Faithfulness score: {metrics['faithfulness']}")
    except Exception as e:
        logger.error(f"Error calculating faithfulness: {e}")
        metrics["faithfulness"] = 0.0
        
    # Calculate answer relevancy
    try:
        relevancy_score = answer_relevancy_metric.single_turn_score(sample)
        metrics["answer_relevancy"] = relevancy_score
        logger.info(f"Answer relevancy score: {metrics['answer_relevancy']}")
    except Exception as e:
        logger.error(f"Error calculating answer relevancy: {e}")
        metrics["answer_relevancy"] = 0.0
        
    # Calculate context precision
    try:
        precision_score = context_precision_metric.single_turn_score(sample)
        metrics["context_precision"] = precision_score
        logger.info(f"Context precision score: {metrics['context_precision']}")
    except Exception as e:
        logger.error(f"Error calculating context precision: {e}")
        metrics["context_precision"] = 0.0
        
    # Calculate context relevancy
    try:
        relevancy_score = context_relevancy_metric.single_turn_score(sample)
        metrics["context_relevancy"] = relevancy_score
        logger.info(f"Context relevancy score: {metrics['context_relevancy']}")
    except Exception as e:
        logger.error(f"Error calculating context relevancy: {e}")
        metrics["context_relevancy"] = 0.0
        
    # Calculate context recall if ground truth is available
    if ground_truth:
        try:
            recall_score = context_recall_metric.single_turn_score(sample)
            metrics["context_recall"] = recall_score
            logger.info(f"Context recall score: {metrics['context_recall']}")
        except Exception as e:
            logger.error(f"Error calculating context recall: {e}")
            metrics["context_recall"] = 0.0
    
    # Calculate harmonic mean of all metrics as a combined score
    if metrics:
        # Avoid division by zero
        values = [v for v in metrics.values() if v > 0]
        if values:
            n = len(values)
            sum_reciprocals = sum(1/v for v in values)
            metrics["harmonic_mean"] = n / sum_reciprocals if sum_reciprocals > 0 else 0.0
            logger.info(f"Harmonic mean score: {metrics['harmonic_mean']}")
    
    logger.info(f"RAGAS metrics calculated:\n\n{metrics}")
    return metrics
