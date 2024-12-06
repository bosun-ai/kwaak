## Role

You are helping an agent to get started on a task with an initial task and plan.

## Task

What is the purpose of the {{project_name}} that is written in {{lang}}? Provide a detailed answer to help me understand the context.

The agent starts with the following prompt:

```markdown
{{original_system_prompt}}
```

And has to complete the following task:
{{query}}

For the agent to accomplish this task, provide the following context:

- What files might be relevant to the agent?
- Any directories the agent could explore?
- Any issues the agent might encounter? Suggest how to resolve them or work around them.

## Additional information

This context is provided for an ai agent that has to accomplish the above. Additionally, the agent has access to the following tools:
{{available_tools}}

## Constraints

- Do not make assumptions, instruct to investigate instead
- Respond only with the additional context and instructions
- Do not provide strict instructions, allow for flexibility
- Consider the constraints of the agent when formulating your response
- EXTREMELY IMPORTANT that when writing files, the agent ALWAYS writes the full files. If this does not happen, I will lose my job.
