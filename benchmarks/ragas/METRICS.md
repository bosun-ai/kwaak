# Codebase Knowledge Extraction & RAGAS

We’re measuring the quality of retrieved knowledge. The basic flow is that we ask for information and the system gives an answer that most precisely fits the request. RAGAS offers a framework for determining the quality of such a response. It does this according a list of metrics, each of with are presented with:

- The question
- A reference answer (”ground truth”, checked by a human)
- The answer the system gives
- The context the system retrieved to generate the answer

## Response (answer) Relevancy

Response relevancy measures how much of the information in the answer is relevant to the original question, by generating questions based on the answer and then returning the average distance between those questions and the original question. If this number is low, then there is information in the answer that is superfluous, or there is information that is missing.

A potential problem with this, is that if the answer that is generated is quite long, or is more of an assignment with an open solution space like writing code, then the questions that could be generated from the answer might have a relatively large embedding distance from the original question. This might affect the sensitivity of this metric.

## Context recall

Context recall measures how much of the claims in the reference answer are in the retrieved context. It would have been better if instead we had a reference context to compare against, but it is assumed that this would be too labor intensive to generate. If this number is low, then there is information in the reference answer that is not in the retrieved contexts.

A potential problem with this is that the reference answer might include claims that are not directly in the retrieved context but instead are inferred from the retrieved context. For example if the context indicated a certain variable is available, then the answer might use that variable. For the metric to be accurate, the system generating the claims needs to be quite sophisticated.

## Context precision

Context Precision measures how much of the information in the `retrieved_contexts` is in the reference answer. If this number is low, then too much irrelevant information is retrieved.

Similar to the context recall, because the reference answer might depend on information that would be inferred from the retrieved context, there needs to be a sophisticated system for judging if the information in the reference answer might be inferred from the contexts.

## Faithfulness

Faithfulness measures how many of the claims made in the answer are supported by the retrieved contexts. If this number is low, then the system is hallucinating during the final answer generation step or is given conflicting information.

Claims in the answer might be inferred or compiled from information in the retrieved contexts. It needs to be verified that the system that finds the claims can handle inferred or compiled claims.