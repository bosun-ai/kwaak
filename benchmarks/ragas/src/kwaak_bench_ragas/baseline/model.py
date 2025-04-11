import os
import logging
from typing import List, Dict, Any, Optional, Union, Literal
from abc import ABC, abstractmethod

from openai import OpenAI

logger = logging.getLogger(__name__)


class BaseModelClient(ABC):
    """Base class for model clients."""
    
    @abstractmethod
    def generate_text(self, prompt: str) -> str:
        """Generate text response for a text prompt."""
        pass


class OpenRouterClient(BaseModelClient):
    """Base client for interacting with models via OpenRouter."""
    
    def __init__(self, model_name: str, api_key: Optional[str] = None):
        """Initialize the OpenRouter client.
        
        Args:
            model_name: Name of the model to use
            api_key: OpenRouter API key. If None, will try to get from environment variable.
        """
        # Get API key from environment variable if not provided
        self.api_key = api_key or os.environ.get("OPENROUTER_API_KEY")
        if not self.api_key:
            raise ValueError("OpenRouter API key not provided and not found in environment variable OPENROUTER_API_KEY")
        
        # Initialize OpenAI client with OpenRouter base URL
        self.client = OpenAI(
            base_url="https://openrouter.ai/api/v1",
            api_key=self.api_key,
        )
        
        self.model = model_name
        logger.info(f"Initialized OpenRouterClient with model: {self.model}")
    
    def generate_text(self, prompt: str) -> str:
        """Generate text response for a text prompt.
        
        Args:
            prompt: Text prompt to send to the model
            
        Returns:
            Generated text response
        """
        try:
            logger.info(f"Sending request to model: {self.model}")
            completion = self.client.chat.completions.create(
                extra_headers={
                    "HTTP-Referer": "https://github.com/bosun-ai/kwaak",  # Site URL for rankings
                    "X-Title": "Kwaak RAGAS Baseline",  # Site title for rankings
                },
                model=self.model,
                messages=[
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            )
            
            # Debug the response
            logger.info(f"Received response from model: {self.model}")
            logger.debug(f"Full response: {completion}")
            
            if not hasattr(completion, 'choices') or not completion.choices:
                error_msg = f"Invalid response format: no choices found in response"
                logger.error(error_msg)
                logger.debug(f"Response content: {completion}")
                raise ValueError(error_msg)
                
            if not hasattr(completion.choices[0], 'message') or not completion.choices[0].message:
                error_msg = f"Invalid response format: no message found in first choice"
                logger.error(error_msg)
                logger.debug(f"First choice content: {completion.choices[0]}")
                raise ValueError(error_msg)
                
            if not hasattr(completion.choices[0].message, 'content'):
                error_msg = f"Invalid response format: no content found in message"
                logger.error(error_msg)
                logger.debug(f"Message content: {completion.choices[0].message}")
                raise ValueError(error_msg)
            
            response = completion.choices[0].message.content
            return response
        except Exception as e:
            logger.error(f"Error generating text: {e}")
            # Re-raise the exception to fail immediately
            raise


class GeminiClient(OpenRouterClient):
    """Client for interacting with the Gemini model via OpenRouter."""
    
    def __init__(self, api_key: Optional[str] = None):
        """Initialize the Gemini client.
        
        Args:
            api_key: OpenRouter API key. If None, will try to get from environment variable.
        """
        super().__init__("google/gemini-2.0-flash-lite-001", api_key)


class ClaudeSonnetClient(OpenRouterClient):
    """Client for interacting with the Claude Sonnet model via OpenRouter."""
    
    def __init__(self, api_key: Optional[str] = None):
        """Initialize the Claude Sonnet client.
        
        Args:
            api_key: OpenRouter API key. If None, will try to get from environment variable.
        """
        super().__init__("anthropic/claude-3.7-sonnet", api_key)


# Convenience functions for generating text with different models

def generate_text_cheap_large_context(prompt: str, api_key: Optional[str] = None) -> str:
    """Generate text using the Gemini Flash model (cheaper, large context).
    
    Args:
        prompt: The text prompt to send to the model
        api_key: Optional API key. If None, will use environment variable
        
    Returns:
        Generated text response
    """
    client = GeminiClient(api_key)
    return client.generate_text(prompt)


def generate_text_large_model(prompt: str, api_key: Optional[str] = None) -> str:
    """Generate text using the Claude Sonnet model (more powerful).
    
    Args:
        prompt: The text prompt to send to the model
        api_key: Optional API key. If None, will use environment variable
        
    Returns:
        Generated text response
    """
    client = ClaudeSonnetClient(api_key)
    return client.generate_text(prompt)
