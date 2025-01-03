# Github search results

{% if items|length == 0 -%}
No results found.
{% endif -%}
{% for item in items -%}
{% for match in item.text_matches -%}
**repository**: {{item.repository.full_name}}
**url**: {{item.html_url}}?raw=true
```
{{match.fragment}}
```
---
{% endfor -%}
{% endfor -%}
