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
        // While the engine restarts (model swap, rebuild) the list is unreachable or empty; wait.
        let deadline = std::time::Instant::now() + DOWN_WAIT_LIMIT;
        let id = loop {
            let got: Result<String, String> = async {
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
                Ok(v["data"][0]["id"]
                    .as_str()
                    .ok_or("engine serves no model")?
                    .to_string())
            }
            .await;
            match got {
                Ok(id) => break id,
                Err(e) if std::time::Instant::now() < deadline => {
                    eprintln!("aien-translate: {e}; waiting for the engine");
                    tokio::time::sleep(DOWN_RETRY).await;
                }
                Err(e) => return Err(e),
            }
        };
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
        let keep_list = if keep.is_empty() {
            "none".to_string()
        } else {
            keep.join(", ")
        };
        let system = format!(
            "You are a professional translator for software websites and documentation.\n\
             Translate every string in the JSON array the user sends from {from_name} into {to}.\n\
             Rules:\n\
             - Reply with ONLY a JSON array of exactly {n} strings, in the same order. No commentary.\n\
             - Keep these exactly as written: URLs, email addresses, anything in backticks, commands, \
             file paths, code, placeholders like {{name}}, {{{{name}}}} or %s, and these names: {keep_list}.\n\
             - Strings may contain markers like <g1>...</g1> (around words that are a link or bold) and <x2/> (code): \
             keep every marker exactly once, around the translated words they belong to.\n\
             - Translate meaning naturally; keep punctuation style and capitalization style sensible for {to}.\n\
             - If a string is already in {to} or cannot be translated, return it unchanged.",
            to = to.english,
            n = texts.len(),
        );
        let user = serde_json::to_string(texts).unwrap();
        let chars: usize = texts.iter().map(|t| t.chars().count()).sum();
        let max_tokens = (128 + chars * 3).min(ENGINE_MAX_TOKENS);
        let body = json!({
            "model": model,
            "temperature": 0,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });

        let _permit = self.gate.acquire().await.map_err(|e| e.to_string())?;
        let resp = match self.send_when_free(&body).await {
            Ok(r) => r,
            Err(e) => {
                *self.model.lock().await = None;
                return Err(e);
            }
        };
        let status = resp.status();
        let streaming = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"));
        if !status.is_success() || !streaming {
            let v: Value = resp
                .json()
                .await
                .map_err(|e| format!("engine reply unreadable: {e}"))?;
            if !status.is_success() {
                return Err(format!("engine error {status}: {}", v["error"]["message"]));
            }
            let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("");
            return parse_array(content, texts.len());
        }
        let content = read_until_list_closes(resp).await?;
        parse_array(&content, texts.len())
    }

    /// Send a chat request, waiting while the engine says it is busy (503).
    /// It is busy for a moment after a reply is cut short, and for longer when
    /// another job holds the GPU. It says how long to wait in `Retry-After`.
    async fn send_when_free(&self, body: &Value) -> Result<reqwest::Response, String> {
        let deadline = std::time::Instant::now() + BUSY_WAIT_LIMIT;
        let down_deadline = std::time::Instant::now() + DOWN_WAIT_LIMIT;
        let mut said_down = false;
        loop {
            let resp = match self
                .client
                .post(format!("{}/v1/chat/completions", self.base))
                .json(body)
                .send()
                .await
            {
                Ok(r) => r,
                // The engine restarts when its model is swapped or rebuilt. Wait for it to come back
                // rather than failing every batch (which would leave whole languages in English).
                Err(e) if e.is_connect() && std::time::Instant::now() < down_deadline => {
                    if !said_down {
                        eprintln!(
                            "aien-translate: engine unreachable ({e}); waiting for it to come back"
                        );
                        said_down = true;
                    }
                    *self.model.lock().await = None;
                    tokio::time::sleep(DOWN_RETRY).await;
                    continue;
                }
                Err(e) => return Err(format!("engine request failed: {e}")),
            };
            if resp.status() != reqwest::StatusCode::SERVICE_UNAVAILABLE {
                return Ok(resp);
            }
            let wait = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<f64>().ok())
                .map(Duration::from_secs_f64)
                .unwrap_or(Duration::from_secs(1))
                .clamp(Duration::from_millis(200), Duration::from_secs(5));
            if std::time::Instant::now() + wait > deadline {
                return Ok(resp);
            }
            tokio::time::sleep(wait).await;
        }
    }
}

/// How long to keep retrying a busy engine before giving up on a batch.
const BUSY_WAIT_LIMIT: Duration = Duration::from_secs(30 * 60);

/// How long to wait for an engine that is down (restarting, model being swapped) before
/// giving up on a batch, and how often to try again meanwhile.
const DOWN_WAIT_LIMIT: Duration = Duration::from_secs(45 * 60);
const DOWN_RETRY: Duration = Duration::from_secs(15);

/// aien-serve-http clamps `max_tokens` to this, so asking for more only hides
/// where a reply will stop.
pub const ENGINE_MAX_TOKENS: usize = 4096;

/// Read a streamed reply, and hang up as soon as its list is complete. The
/// model sometimes keeps writing after the list (seen: invented text up to the
/// token cap, 140 s of GPU time); dropping the connection makes the server
/// stop generating.
async fn read_until_list_closes(mut resp: reqwest::Response) -> Result<String, String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut content = String::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("engine stream failed: {e}"))?
    {
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                return Ok(content);
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(err) = v.get("error") {
                return Err(format!("engine error: {}", err["message"]));
            }
            if let Some(d) = v["choices"][0]["delta"]["content"].as_str() {
                content.push_str(d);
            }
        }
        if list_closed(&content) {
            return Ok(content);
        }
    }
    Ok(content)
}

/// Where a reply's list stands, scanning from its first `[` and skipping any
/// brackets inside strings (translations can contain text like `["x"]`).
enum ListScan {
    NoList,
    Closed { start: usize, end: usize },
    Open { start: usize, in_string: bool },
}

fn scan_list(content: &str) -> ListScan {
    let Some(start) = content.find('[') else {
        return ListScan::NoList;
    };
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (i, ch) in content[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return ListScan::Closed {
                        start,
                        end: start + i,
                    };
                }
            }
            _ => {}
        }
    }
    ListScan::Open { start, in_string }
}

/// True once the reply contains a complete list.
pub fn list_closed(content: &str) -> bool {
    matches!(scan_list(content), ListScan::Closed { .. })
}

/// Pull the JSON array of strings out of a model reply. Text after the list is
/// ignored. A list whose strings are all complete but whose closing `]` is
/// missing (the model sometimes stops one character early) is closed and
/// accepted; the item count is still checked.
pub fn parse_array(content: &str, expect: usize) -> Result<Vec<String>, String> {
    let list = match scan_list(content) {
        ListScan::NoList => return Err("no JSON array in reply".into()),
        ListScan::Closed { start, end } => content[start..=end].to_string(),
        ListScan::Open {
            in_string: true, ..
        } => return Err("reply ends in the middle of a string".into()),
        ListScan::Open {
            start,
            in_string: false,
        } => {
            let mut s = content[start..]
                .trim_end()
                .trim_end_matches(',')
                .to_string();
            s.push(']');
            s
        }
    };
    let arr: Vec<String> =
        serde_json::from_str(&list).map_err(|e| format!("reply is not a string array: {e}"))?;
    if arr.len() != expect {
        return Err(format!("reply has {} items, expected {expect}", arr.len()));
    }
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::parse_array;
    use serde_json::Value;

    fn fixture(name: &str) -> (Vec<String>, String) {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let items = serde_json::from_value(v["items"].clone()).unwrap();
        (items, v["content"].as_str().unwrap().to_string())
    }

    #[test]
    fn parses_fenced_reply() {
        let r = parse_array("```json\n[\"你好\", \"世界\"]\n```", 2).unwrap();
        assert_eq!(r, vec!["你好", "世界"]);
    }

    #[test]
    fn rejects_wrong_count() {
        assert!(parse_array("[\"a\"]", 2).is_err());
    }

    #[test]
    fn bracket_inside_a_string_is_not_the_end() {
        let r = parse_array(r#"["e.g. `[\"x\"]` here", "two"]"#, 2).unwrap();
        assert_eq!(r, vec![r#"e.g. `["x"]` here"#, "two"]);
    }

    #[test]
    fn text_after_the_list_is_ignored() {
        let r = parse_array("[\"a\", \"b\"]\n\n[{\"name\": \"junk\"", 2).unwrap();
        assert_eq!(r, vec!["a", "b"]);
    }

    #[test]
    fn one_missing_closing_bracket_is_repaired() {
        assert_eq!(parse_array("[\"a\", \"b\"", 2).unwrap(), vec!["a", "b"]);
        assert_eq!(parse_array("[\"a\", \"b\",\n", 2).unwrap(), vec!["a", "b"]);
    }

    #[test]
    fn reply_cut_off_inside_a_string_is_refused() {
        assert!(parse_array("[\"a\", \"b is cut o", 2).is_err());
    }

    /// Real reply: the model ended without the list's closing `]`, and one
    /// item contains `["こんにちは世界"]`.
    #[test]
    fn real_reply_missing_close_ja() {
        let (items, content) = fixture("reply_missing_close_ja.json");
        let r = parse_array(&content, items.len()).unwrap();
        assert_eq!(r.len(), 29);
        assert!(r[28].contains("オリジナル"), "{}", r[28]);
    }

    /// Real reply: a complete list, then invented text until the token cap.
    #[test]
    fn real_reply_trailing_junk_fr() {
        let (items, content) = fixture("reply_trailing_junk_fr.json");
        let r = parse_array(&content, items.len()).unwrap();
        assert_eq!(r.len(), 29);
        assert!(!r.iter().any(|s| s.contains("\"name\"")));
    }

    #[test]
    fn list_closed_detection() {
        use super::list_closed;
        assert!(!list_closed("[\"a\", \"b"));
        assert!(!list_closed("[\"a ] still inside\""));
        assert!(list_closed("[\"a\", \"b\"]"));
        assert!(list_closed("sure: [\"a\"] and more"));
        assert!(!list_closed("no list yet"));
    }
}
