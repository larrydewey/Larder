# AI assist

AI assist is optional. When it is on, Larder can use a large language model to:

- turn **a photo of a recipe** (cookbook page, handwritten card, screenshot) into a recipe:
  the **Photo** tab on the Import screen;
- turn **messy text** into a recipe: the **Use AI to structure the recipe** switch on the
  **Text** tab;
- parse ingredient lines (available through the API with `use_llm: true`; the app's editor
  uses the rule-based parser).

Supported providers are **Anthropic** (Claude), **OpenAI**, and any **OpenAI-compatible** API,
including a local **Ollama** server.

## Turning it on

=== "Server (connected mode and web)"

    A server administrator sets environment variables:

    ```bash
    LARDER_LLM_PROVIDER=anthropic          # or openai, ollama
    LARDER_LLM_API_KEY=sk-ant-...          # not needed for Ollama
    # optional:
    LARDER_LLM_MODEL=claude-opus-5-5
    LARDER_LLM_BASE_URL=https://api.anthropic.com
    ```

    | Provider value | Default model | Default base URL |
    |---|---|---|
    | `anthropic` (or `claude`) | `claude-opus-5-5` | `https://api.anthropic.com` |
    | `openai` (or `openai-compatible`) | `gpt-4o-mini` | `https://api.openai.com/v1` |
    | `ollama` | `llama3.2-vision` | `http://localhost:11434/v1` |
    | empty, `none`, `off` | AI assist disabled | |

    If `LARDER_LLM_API_KEY` is not set, `ANTHROPIC_API_KEY` (for `anthropic`) or
    `OPENAI_API_KEY` (for `openai`) is used. An unknown provider name stops the server from
    starting. Every user of the server can then use AI assist; the server's key is used.

    For Ollama in Docker Compose, point the base URL at the Ollama container, for example
    `LARDER_LLM_BASE_URL=http://ollama:11434/v1`, and use a vision-capable model if you want
    photo imports.

=== "Standalone Android"

    Go to **Settings → Preferences → AI assistant** ("Off · reads recipe photos and messy
    text"). In the **AI assistant** dialog:

    | Field | Notes |
    |---|---|
    | **Provider** | **Anthropic**, **OpenAI-compatible** or **Ollama (self-hosted)** |
    | **API key** | Stored only on this device |
    | **Model (optional)** | Defaults as in the server table |
    | **Base URL (optional)** | For example your Ollama server, `http://192.168.1.20:11434/v1` |

    **Save** turns it on ("AI assistant enabled"); **Turn off** removes the settings. The
    phone talks to the provider directly.

When AI assist is not available, the Photo tab is hidden and the text switch is not shown.
Calling an AI endpoint on the API without it returns `400` with code `llm_disabled` and the
message "AI features are not enabled on this server".

## How it works

- With **Anthropic**, Larder calls the Messages API with a structured-output JSON schema at
  **low effort**, which is enough for transcription and keeps it fast and cheap. With the
  default model (and the other models that support it) on `api.anthropic.com`, it also opts
  into the server-side refusal fallback (`fallbacks: "default"`), so a request that a safety
  classifier declines is retried on a fallback model instead of failing. Requests time out
  after 3 minutes.
- With **OpenAI-compatible** providers, Larder calls `/chat/completions` with a strict JSON
  schema response format. Requests time out after 5 minutes. Replies wrapped in code fences or
  prose are tolerated.
- The model is instructed to transcribe faithfully: keep the author's quantities, units,
  wording and step order, and not invent anything. Section headings come back as `## Heading`
  lines and become ingredient and step sections.
- The result goes through the same normalisation and ingredient parser as any other recipe,
  and opens in the editor for you to check before saving.

## What is sent to the provider

!!! warning "Privacy"

    Using AI assist sends your content to a third party unless you run a local model.

| Action | Sent |
|---|---|
| Photo import | The photo, re-encoded as JPEG and downscaled to at most 2000 px (server) or as picked on the phone (at most 2400 px), plus a fixed instruction prompt |
| Text import with AI | The full text you pasted, plus a fixed instruction prompt |
| Ingredient parsing with AI (API) | The ingredient lines, numbered |

Nothing else is sent: no account details, no other recipes. On the server the provider's API
key is sent only to the configured base URL. Check your provider's data-retention policy, or
use Ollama to keep everything on your own hardware.

## Troubleshooting

| Message | Meaning |
|---|---|
| "AI features are not enabled on this server" | The server has no `LARDER_LLM_PROVIDER`. |
| "Set up an AI provider in Settings to use AI imports." | Standalone mode without AI settings. |
| "no recipe found in the image" / "in the text" | The model found no recipe. |
| "the model declined to process this request" | The provider refused. |
| "the recipe was too long for the model's output limit" | Split the text or use a model with a larger output. |
| "LLM request failed (401 …)" | Wrong or missing API key. |
| "the model did not return valid JSON" | The model ignored the schema; local models are more prone to this. Try a larger model. |
