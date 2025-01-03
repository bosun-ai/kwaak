# Github search results

{% for item in items -%}
{% for match in item.text_matches -%}

**repository**: {{item.repository.full_name}}
**url**: {{item.html_url}}

```
{{match.fragment}}
```

{% endfor -%}
{% endfor -%}
