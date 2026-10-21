# AI Models & Licensing Guide

ReleaseWeave integrates with local AI providers (Ollama) to synthesize verifiable release notes while ensuring complete source code privacy on the user's machine.

---

## 1. Model Licensing vs Project Licensing

> [!IMPORTANT]
> **Licensing Separation:**
> - The **ReleaseWeave** application codebase is licensed under the **Apache License 2.0** (see [LICENSE](LICENSE)).
> - AI models downloaded and executed locally through Ollama are governed by **distinct licenses and terms** issued by their respective creators and organizations. ReleaseWeave does not distribute model weights within this repository.

Summary of recommended local models:

| Model | Creator / Organization | Model License | Commercial Use |
|---|---|---|:---:|
| **Qwen 2.5 Coder (7B / 14B)** *(Default)* | Alibaba Cloud / Qwen Team | Apache 2.0 | Permitted |
| **Llama 3.2 (3B / 8B)** | Meta AI | Llama 3.2 Community License | Permitted (<700M active users) |
| **Mistral 7B** | Mistral AI | Apache 2.0 | Permitted |
| **DeepSeek Coder V2** | DeepSeek | DeepSeek License | Permitted |

---

## 2. Running Ollama for ReleaseWeave

1. **Install Ollama:**
   Follow instructions at [ollama.com](https://ollama.com).

2. **Start Ollama Service:**
   ```bash
   ollama serve
   ```
   By default, Ollama listens on `http://localhost:11434`.

3. **Download Recommended Model:**
   ```bash
   # Default model tested with ReleaseWeave JSON schema
   ollama pull qwen2.5-coder:7b
   ```

4. **Verify Connectivity:**
   Test connectivity via terminal:
   ```bash
   curl http://localhost:11434/api/tags
   ```

---

## 3. Structured JSON Output & Provenance Verification

ReleaseWeave dispatches system instructions to Ollama's `/api/chat` endpoint with `format: "json"`. The model must output valid JSON conforming to this schema:

```json
{
  "items": [
    {
      "category": "feature | fix | refactor | docs | perf | test | chore | other",
      "title": "Concise summary of change",
      "description": "Detailed explanation of user impact",
      "commit_hashes": ["abc1234def5678"]
    }
  ]
}
```

### Hallucination Prevention Mechanism
- Every hash in `commit_hashes` is checked against the actual Git commit history analyzed in the active range.
- If a model invents a commit hash not present in the repository, the item is flagged as `unsourced` and its confidence score is reduced to `0.3`.
- Users can switch to pure deterministic template generation at any time without invoking AI models.
