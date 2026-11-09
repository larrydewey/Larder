//! Optional LLM assistance: turning free text or a photo into a structured
//! recipe, and parsing messy ingredient lines. Supports the Anthropic
//! Messages API (default) and any OpenAI-compatible chat completions API
//! (OpenAI, Ollama, LM Studio, ...).

use base64::Engine;
use larder_core::{Ingredient, Instruction, RecipeData};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    #[serde(alias = "claude")]
    Anthropic,
    #[serde(alias = "ollama", alias = "openai-compatible")]
    OpenAi,
}

/// Connection settings for the LLM provider.
#[derive(Clone, Debug, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
}

impl LlmConfig {
    /// Builds a config from a provider name plus optional overrides, filling defaults.
    pub fn from_parts(
        provider: &str,
        api_key: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<Option<Self>, String> {
        let p = provider.trim().to_ascii_lowercase();
        let cfg = match p.as_str() {
            "" | "none" | "off" => return Ok(None),
            "anthropic" | "claude" => LlmConfig {
                provider: LlmProvider::Anthropic,
                api_key,
                model: model.unwrap_or_else(|| "claude-opus-5-5".into()),
                base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com".into()),
            },
            "openai" | "ollama" | "openai-compatible" => LlmConfig {
                provider: LlmProvider::OpenAi,
                api_key,
                model: model.unwrap_or_else(|| if p == "ollama" { "llama3.2-vision".into() } else { "gpt-4o-mini".into() }),
                base_url: base_url.unwrap_or_else(|| {
                    if p == "ollama" {
                        "http://localhost:11434/v1".into()
                    } else {
                        "https://api.openai.com/v1".into()
                    }
                }),
            },
            other => return Err(format!("unknown LLM provider '{other}' (expected anthropic, openai, ollama or none)")),
        };
        Ok(Some(cfg))
    }
}

const ANTHROPIC_VERSION: &str = "2026-10-21";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

pub struct Llm {
    cfg: LlmConfig,
    http: reqwest::Client,
}

const SYSTEM: &str = "You convert recipes into structured data for a recipe manager. \
Transcribe faithfully: keep the author's quantities, units, wording and step order; do not invent ingredients, \
steps, times or yields that are not stated or clearly implied. Write ingredient lines exactly as they would appear \
in a cookbook (e.g. \"2 cups all-purpose flour, sifted\"). When the ingredients or steps are grouped under \
sub-headings (e.g. \"For the sauce\"), emit the heading as its own line prefixed with \"## \". \
Times are in minutes. Use an empty string or null when a field is unknown.";

fn recipe_schema() -> Value {
    let str_arr = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "description", "yield_quantity", "yield_unit", "prep_time_minutes", "cook_time_minutes",
                     "total_time_minutes", "ingredients", "instructions", "tags", "categories", "notes"],
        "properties": {
            "name": { "type": "string" },
            "description": { "type": "string" },
            "yield_quantity": { "type": ["number", "null"] },
            "yield_unit": { "type": "string" },
            "prep_time_minutes": { "type": ["integer", "null"] },
            "cook_time_minutes": { "type": ["integer", "null"] },
            "total_time_minutes": { "type": ["integer", "null"] },
            "ingredients": str_arr,
            "instructions": str_arr,
            "tags": str_arr,
            "categories": str_arr,
            "notes": { "type": "string" }
        }
    })
}

fn ingredients_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["ingredients"],
        "properties": {
            "ingredients": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["quantity", "quantity_max", "unit", "food", "note"],
                    "properties": {
                        "quantity": { "type": ["number", "null"] },
                        "quantity_max": { "type": ["number", "null"] },
                        "unit": { "type": ["string", "null"] },
                        "food": { "type": ["string", "null"] },
                        "note": { "type": ["string", "null"] }
                    }
                }
            }
        }
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LlmRecipe {
    name: String,
    description: String,
    yield_quantity: Option<f64>,
    yield_unit: String,
    prep_time_minutes: Option<i64>,
    cook_time_minutes: Option<i64>,
    total_time_minutes: Option<i64>,
    ingredients: Vec<String>,
    instructions: Vec<String>,
    tags: Vec<String>,
    categories: Vec<String>,
    notes: String,
}

impl LlmRecipe {
    fn into_data(self) -> RecipeData {
        let mut section: Option<String> = None;
        let mut ingredients = vec![];
        for line in self.ingredients {
            if let Some(h) = line.trim().strip_prefix("##") {
                section = Some(h.trim().to_string()).filter(|s| !s.is_empty());
                continue;
            }
            let mut ing = Ingredient::parse(&line);
            ing.section = section.clone();
            ingredients.push(ing);
        }
        let mut section: Option<String> = None;
        let mut instructions = vec![];
        for line in self.instructions {
            if let Some(h) = line.trim().strip_prefix("##") {
                section = Some(h.trim().to_string()).filter(|s| !s.is_empty());
                continue;
            }
            instructions.push(Instruction { section: section.clone(), text: line });
        }
        let mut data = RecipeData {
            name: self.name,
            description: self.description,
            yield_quantity: self.yield_quantity,
            yield_unit: self.yield_unit,
            prep_time_minutes: self.prep_time_minutes,
            cook_time_minutes: self.cook_time_minutes,
            total_time_minutes: self.total_time_minutes,
            notes: self.notes,
            ingredients,
            instructions,
            tags: self.tags,
            categories: self.categories,
            ..Default::default()
        };
        data.normalize();
        data
    }
}

pub struct ImageInput {
    pub bytes: Vec<u8>,
    pub media_type: String,
}

impl Llm {
    pub fn new(cfg: LlmConfig, http: reqwest::Client) -> Self {
        Self { cfg, http }
    }

    pub fn describe(&self) -> String {
        format!("{:?} ({})", self.cfg.provider, self.cfg.model)
    }

    /// Extracts a recipe from free text (a pasted recipe, an email, OCR output ...).
    pub async fn recipe_from_text(&self, text: &str) -> anyhow::Result<RecipeData> {
        let prompt = format!("Extract the recipe from the text below.\n\n<recipe_text>\n{text}\n</recipe_text>");
        let v = self.complete_json(&prompt, None, recipe_schema()).await?;
        let r: LlmRecipe = serde_json::from_value(v)?;
        if r.name.trim().is_empty() && r.ingredients.is_empty() {
            anyhow::bail!("no recipe found in the text");
        }
        Ok(r.into_data())
    }

    /// Extracts a recipe from a photo (cookbook page, handwritten card, screenshot).
    pub async fn recipe_from_image(&self, image: ImageInput) -> anyhow::Result<RecipeData> {
        let prompt = "Transcribe the recipe in this image (it may be a cookbook page, a handwritten recipe card or a screenshot). \
If there is no recipe in the image, return an empty name and empty lists.";
        let v = self.complete_json(prompt, Some(image), recipe_schema()).await?;
        let r: LlmRecipe = serde_json::from_value(v)?;
        if r.name.trim().is_empty() && r.ingredients.is_empty() {
            anyhow::bail!("no recipe found in the image");
        }
        Ok(r.into_data())
    }

    /// Parses ingredient lines into structured parts.
    pub async fn parse_ingredients(&self, lines: &[String]) -> anyhow::Result<Vec<Ingredient>> {
        let numbered: Vec<String> = lines.iter().enumerate().map(|(i, l)| format!("{}. {}", i + 1, l)).collect();
        let prompt = format!(
            "Parse each ingredient line into quantity (a number; convert fractions like 1 1/2 to 1.5), quantity_max \
(upper bound of a range, else null), unit (singular, abbreviated where common: tsp, tbsp, cup, oz, lb, g, kg, ml, l, \
clove, can, ...; null if none), food (the ingredient itself, without preparation notes) and note (preparation or \
other remarks). Return exactly one entry per line, in order.\n\n{}",
            numbered.join("\n")
        );
        let v = self.complete_json(&prompt, None, ingredients_schema()).await?;
        #[derive(Deserialize)]
        struct Parsed {
            ingredients: Vec<Ingredient>,
        }
        let parsed: Parsed = serde_json::from_value(v)?;
        let mut out = vec![];
        for (i, line) in lines.iter().enumerate() {
            let mut ing = parsed.ingredients.get(i).cloned().unwrap_or_default();
            ing.original = line.clone();
            if let Some(u) = ing.unit.take() {
                ing.unit = Some(larder_core::units::canonical_unit(&u).map(str::to_string).unwrap_or(u));
            }
            if ing.food.is_none() && ing.quantity.is_none() {
                ing = Ingredient::parse(line);
            }
            out.push(ing);
        }
        Ok(out)
    }

    async fn complete_json(&self, prompt: &str, image: Option<ImageInput>, schema: Value) -> anyhow::Result<Value> {
        let text = match self.cfg.provider {
            LlmProvider::Anthropic => self.anthropic(prompt, image, schema).await?,
            LlmProvider::OpenAi => self.openai(prompt, image, schema).await?,
        };
        parse_json_loose(&text)
    }

    async fn anthropic(&self, prompt: &str, image: Option<ImageInput>, schema: Value) -> anyhow::Result<String> {
        let mut content = vec![];
        if let Some(img) = image {
            content.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": img.media_type,
                    "data": base64::engine::general_purpose::STANDARD.encode(&img.bytes),
                }
            }));
        }
        content.push(json!({ "type": "text", "text": prompt }));
        let model = self.cfg.model.as_str();
        let mut body = json!({
            "model": model,
            "max_tokens": 16000,
            "system": SYSTEM,
            "output_config": {
                // Transcription is a simple task; low effort keeps it fast and cheap.
                "effort": "low",
                "format": { "type": "json_schema", "schema": schema }
            },
            "messages": [{ "role": "user", "content": content }]
        });
        let use_fallbacks = supports_default_fallbacks(model) && self.cfg.base_url.contains("api.anthropic.com");
        if use_fallbacks {
            body["fallbacks"] = json!("default");
        }
        let mut req = self
            .http
            .post(format!("{}/v1/messages", self.cfg.base_url.trim_end_matches('/')))
            .timeout(std::time::Duration::from_secs(180))
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body);
        if use_fallbacks {
            req = req.header("anthropic-beta", FALLBACK_BETA);
        }
        if let Some(key) = &self.cfg.api_key {
            req = req.header("x-api-key", key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let v: Value = resp.json().await?;
        if !status.is_success() {
            let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
            anyhow::bail!("LLM request failed ({status}): {msg}");
        }
        match v["stop_reason"].as_str() {
            Some("refusal") => anyhow::bail!("the model declined to process this request"),
            Some("max_tokens") => anyhow::bail!("the recipe was too long for the model's output limit"),
            _ => {}
        }
        let text: String = v["content"]
            .as_array()
            .map(|blocks| blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(""))
            .unwrap_or_default();
        if text.is_empty() {
            anyhow::bail!("the model returned no text");
        }
        Ok(text)
    }

    async fn openai(&self, prompt: &str, image: Option<ImageInput>, schema: Value) -> anyhow::Result<String> {
        let user_content = match image {
            Some(img) => json!([
                { "type": "image_url", "image_url": { "url": format!("data:{};base64,{}", img.media_type,
                    base64::engine::general_purpose::STANDARD.encode(&img.bytes)) } },
                { "type": "text", "text": prompt }
            ]),
            None => json!(prompt),
        };
        let body = json!({
            "model": self.cfg.model,
            "messages": [
                { "role": "system", "content": SYSTEM },
                { "role": "user", "content": user_content }
            ],
            "response_format": { "type": "json_schema", "json_schema": { "name": "result", "strict": true, "schema": schema } }
        });
        let mut req = self
            .http
            .post(format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/')))
            .timeout(std::time::Duration::from_secs(300))
            .json(&body);
        if let Some(key) = &self.cfg.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let v: Value = resp.json().await?;
        if !status.is_success() {
            let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
            anyhow::bail!("LLM request failed ({status}): {msg}");
        }
        v["choices"][0]["message"]["content"].as_str().map(str::to_string).ok_or_else(|| anyhow::anyhow!("the model returned no content"))
    }
}

/// Models that accept the server-side `fallbacks: "default"` refusal routing.
fn supports_default_fallbacks(model: &str) -> bool {
    matches!(model, "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-1" | "claude-sonnet-5-5")
}

/// Parses JSON, tolerating code fences or leading prose from less strict providers.
fn parse_json_loose(text: &str) -> anyhow::Result<Value> {
    if let Ok(v) = serde_json::from_str(text.trim()) {
        return Ok(v);
    }
    let start = text.find('{');
    let end = text.rfind('}');
    if let (Some(s), Some(e)) = (start, end) {
        if let Ok(v) = serde_json::from_str(&text[s..=e]) {
            return Ok(v);
        }
    }
    anyhow::bail!("the model did not return valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_json() {
        assert!(parse_json_loose("```json\n{\"a\":1}\n```").is_ok());
        assert!(parse_json_loose("nope").is_err());
    }

    #[test]
    fn llm_recipe_sections() {
        let r = LlmRecipe {
            name: "Pie".into(),
            ingredients: vec!["## Crust".into(), "2 cups flour".into(), "## Filling".into(), "3 apples".into()],
            instructions: vec!["Mix.".into()],
            ..Default::default()
        };
        let d = r.into_data();
        assert_eq!(d.ingredients[0].section.as_deref(), Some("Crust"));
        assert_eq!(d.ingredients[1].section.as_deref(), Some("Filling"));
        assert_eq!(d.ingredients[1].quantity, Some(3.0));
    }
}
