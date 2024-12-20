{{description}}

---

_This pull request was created by [kwaak](https://github.com/bosun-ai/kwaak), a free, open-source, autonomous coding agent tool. Pull requests are tracked in bosun-ai/kwaak#48_

<details>
<summary>Message History</summary>

{% for message in messages -%}
<details>
  <summary>{{message.role}}</summary>

```markdown
{{message.content}}
```
</details>
{% if message.role is containing("Assistant") -%}

---
{% endif -%}
{% endfor -%}

</details>
