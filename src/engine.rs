//! Talks to AIEN's inference server (aien-serve-http, OpenAI-style API).

use crate::langs::Lang;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};

pub struct Engine {
    client: reqwest::Client,
    base: String,
    model: Mutex<Option<String>>,
    /// The GPU is shared, so send one request at a time.
    gate: Semaphore,
}

impl Engine {
    pub fn new(base: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .expect("http client"),
            base: base.trim_end_matches('/').to_string(),
            model: Mutex::new(None),
            gate: Semaphore::new(1),
        }
    }

    /// Model id served by the engine, asked once and remembered.
    pub async fn model(&self) -> Result<String, String> {
        let mut m = self.model.lock().await;
        if let Some(id) = m.as_ref() {
            return Ok(id.clone());
        }
        let v: Value = self
            .client
            .get(format!("{}/v1/models", self.base))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("engine unreachable: {e}"))?
            .json()
            .await
            .map_err(|e| format!("engine model list unreadable: {e}"))?;
        let id = v["data"][0]["id"].as_str().ok_or("engine serves no model")?.to_string();
        *m = Some(id.clone());
        Ok(id)
    }

    /// Translate a batch. Returns one entry per input, or an error if the
    /// model's answer could not be matched up with the inputs.
    pub async fn translate(
        &self,
        from: Option<&Lang>,
        to: &Lang,
        texts: &[String],
        keep: &[String],
    ) -> Result<Vec<String>, String> {
        let model = self.model().await?;
        let from_name = from.map(|l| l.english).unwrap_or("its original language");
        let keep_list = if keep.is_empty() { "none".to_string() } else { keep.join(", ") };
        let system = format!(
            "You are a professional translator for software websites and documentation.\n\
             Translate every string in the JSON array the user sends from {from_name} into {to}.\n\
             Rules:\n\
             - Reply with ONLY a JSON array of exactly {n} strings, in the same order. No commentary.\n\
             - Keep these exactly as written: URLs, email addresses, anything in backticks, commands, \
             file paths, code, placeholders like {{name}}, {{{{name}}}} or %s, and these names: {keep_list}.\n\
             - Translate meaning naturally; keep punctuation style and capitalization style sensible for {to}.\n\
             - If a string is already in {to} or cannot be translated, return it unchanged.",
            to = to.english,
            n = texts.len(),
        );
        let user = serde_json::to_string(texts).unwrap();
        let chars: usize = texts.iter().map(|t| t.chars().count()).sum();
        let max_tokens = (128 + chars * 3).min(8192);
        let body = json!({
            "model": model,
            "temperature": 0,
            "max_tokens": max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });

        let _permit = self.gate.acquire().await.map_err(|e| e.to_string())?;
        let resp = self
            .client
            .post(format!("{}/v1/chat/completions", self.base))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("engine request failed: {e}"))?;
        let status = resp.status();
        let v: Value = resp.json().await.map_err(|e| format!("engine reply unreadable: {e}"))?;
        if !status.is_success() {
            return Err(format!("engine error {status}: {}", v["error"]["message"]));
        }
        let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
        parse_array(content, texts.len())
    }
}

/// Pull the JSON array of strings out of a model reply.
pub fn parse_array(content: &str, expect: usize) -> Result<Vec<String>, String> {
    let start = content.find('[').ok_or("no JSON array in reply")?;
    let end = content.rfind(']').ok_or("no JSON array in reply")?;
    if end < start {
        return Err("no JSON array in reply".into());
    }
    let arr: Vec<String> =
        serde_json::from_str(&content[start..=end]).map_err(|e| format!("reply is not a string array: {e}"))?;
    if arr.len() != expect {
        return Err(format!("reply has {} items, expected {expect}", arr.len()));
    }
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::parse_array;

    #[test]
    fn parses_fenced_reply() {
        let r = parse_array("```json\n[\"你好\", \"世界\"]\n```", 2).unwrap();
        assert_eq!(r, vec!["你好", "世界"]);
    }

    #[test]
    fn rejects_wrong_count() {
        assert!(parse_array("[\"a\"]", 2).is_err());
    }
}
