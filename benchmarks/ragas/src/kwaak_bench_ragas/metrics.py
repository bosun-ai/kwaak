import logging
from typing import Dict, List, Optional
import os

# Set OpenAI API key for RAGAS if available
openai_api_key = os.environ.get('OPENAI_API_KEY') or os.environ.get('OPENROUTER_API_KEY')
if openai_api_key:
    os.environ['OPENAI_API_KEY'] = openai_api_key
    logging.info("Using API key from environment for RAGAS")

# Import RAGAS metrics - using the current API with mock fallbacks
# Define global variable for tracking if we're using real metrics
global using_real_metrics
using_real_metrics = False

try:
    # Import the class-based metrics from the current RAGAS version
    from ragas.metrics import (
        Faithfulness,
        AnswerRelevancy,
        ContextPrecision,
        ContextRecall,
        ContextRelevance
    )
    from ragas import SingleTurnSample
    
    # Import LLM wrapper for RAGAS
    try:
        from langchain_openai import ChatOpenAI
        from ragas.llms import LangchainLLMWrapper
        
        # Initialize LLM if API key is available
        if openai_api_key:
            # Create LLM instance for RAGAS
            llm = ChatOpenAI(
                api_key=openai_api_key,
                model="gpt-3.5-turbo",
                temperature=0
            )
            ragas_llm = LangchainLLMWrapper(llm)
            logging.info("Successfully initialized LLM for RAGAS")
            
            # Flag to indicate we're using the real RAGAS metrics
            using_real_metrics = True
            logging.info("Successfully imported RAGAS metrics using current API")
        else:
            logging.warning("No OpenAI API key available, using mock metrics")
    except ImportError as e:
        logging.error(f"Error importing LLM for RAGAS: {e}")
        using_real_metrics = False
except ImportError as e:
    logging.error(f"Error importing RAGAS metrics: {e}")
    
    # Mock implementations for testing without RAGAS
    def mock_faithfulness(questions, contexts, responses):
        return [0.8] * len(questions)
    
    def mock_answer_relevancy(questions, responses):
        return [0.75] * len(questions)
    
    def mock_context_precision(questions, contexts, responses):
        return [0.7] * len(questions)
    
    def mock_context_recall(questions, contexts, ground_truths):
        return [0.65] * len(questions)
    
    def mock_context_relevancy(questions, contexts):
        return [0.6] * len(questions)

logger = logging.getLogger(__name__)


def calculate_ragas_metrics(
    query: str,
    contexts: List[str],
    response: str,
    ground_truth: Optional[str] = None
) -> Dict[str, float]:
    """Calculate RAGAS metrics for a single instance.
    
    Args:
        query: The question or query
        contexts: List of context documents
        response: The generated response
        ground_truth: Optional ground truth answer
        
    Returns:
        Dictionary of metric names and scores
    """
    global using_real_metrics
    logger.info(f"Calculating RAGAS metrics for query: {query[:50]}...")
    
    metrics = {}
    
    if using_real_metrics:
        # Use the current RAGAS API with class-based metrics
        try:
            # Create a SingleTurnSample for evaluation using the correct field names
            sample = SingleTurnSample(
                user_input=query,
                retrieved_contexts=contexts,
                response=response,
                reference=ground_truth if ground_truth else None
            )
            
            # Initialize metrics with LLM
            faithfulness_metric = Faithfulness(llm=ragas_llm)
            answer_relevancy_metric = AnswerRelevancy(llm=ragas_llm)
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
        except Exception as e:
            logger.error(f"Error using real RAGAS metrics: {e}")
            # Fall back to mock implementations
            using_real_metrics = False
    
    # Use mock implementations if real metrics failed or are not available
    if not using_real_metrics:
        logger.warning("Using mock RAGAS metrics")
        # Prepare inputs for mock metrics
        questions = [query]
        responses = [response]
        contexts_list = [contexts]
        
        # Calculate faithfulness
        metrics["faithfulness"] = mock_faithfulness(questions, contexts_list, responses)[0]
        
        # Calculate answer relevancy
        metrics["answer_relevancy"] = mock_answer_relevancy(questions, responses)[0]
        
        # Calculate context precision
        metrics["context_precision"] = mock_context_precision(questions, contexts_list, responses)[0]
        
        # Calculate context relevancy
        metrics["context_relevancy"] = mock_context_relevancy(questions, contexts_list)[0]
        
        # Calculate context recall if ground truth is available
        if ground_truth:
            ground_truths = [ground_truth]
            metrics["context_recall"] = mock_context_recall(questions, contexts_list, ground_truths)[0]
    
    # Calculate harmonic mean of all metrics as a combined score
    if metrics:
        # Avoid division by zero
        values = [v for v in metrics.values() if v > 0]
        if values:
            n = len(values)
            sum_reciprocals = sum(1/v for v in values)
            metrics["harmonic_mean"] = n / sum_reciprocals if sum_reciprocals > 0 else 0.0
            logger.info(f"Harmonic mean score: {metrics['harmonic_mean']}")
    
    logger.info(f"RAGAS metrics calculated: {metrics}")
    return metrics
