{{description}}

---

_This pull request was created by [kwaak](https://github.com/bosun-ai/kwaak), a free, open-source, autonomous coding agent tool. Pull requests are tracked in bosun-ai/kwaak#48_

### Pull Request Title Guidelines

Please use [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/) for the pull request title. Here are some common types:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation updates
- `refactor:` for code changes that neither fix a bug nor add a feature
- `test:` for adding or correcting tests

{% if messages | length > 0 -%}
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
{% endif -%}
