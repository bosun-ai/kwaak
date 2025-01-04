{% if answer -%}
{{ answer }}
{% else -%}
{% for item in results -%}
title: {{ item.title }}
**url**: {{ item.url }}
{{ item.content }}
---

{% endfor -%}
{% endif -%}

{% if answer %}

## Relevant urls

{% for item in results -%}
{{ item.title }}
{{ item.url }}

{% endfor -%}
{% endif %}

{% if follow_up_questions -%}
## Follow up questions

{% for item in follow_up_questions -%}
- {{ item }}
{% endfor -%}
{% endif -%}
