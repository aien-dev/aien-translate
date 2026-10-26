//! aien-translate: a translation server that speaks the translate.js
//! `translate.service` protocol and translates with AIEN's own runtime.
//!
//! Endpoints (POST or GET, form-encoded like translate.js sends):
//! - `/translate.json`  from, to, text (JSON array of strings)
//! - `/language.json`   languages offered
//! - `/connectTest.json`, `/init.json`, `/ip.json`  translate.js housekeeping
//! - `/health`          is the engine reachable

pub mod cache;
pub mod engine;
pub mod guard;
pub mod langs;

use axum::{
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode, Uri},
    routing::{any, get},
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use subtle::ConstantTimeEq;

/// Bump when the prompt changes, so old saved translations are not reused.
pub const PROMPT_VERSION: &str = "p4";

/// Names that are never translated unless the operator overrides the list.
pub const DEFAULT_KEEP: &[&str] = &[
    "AIEN", "AIENOS", "Aienix", "Omega", "OMEGA", "Interplane", "INTERPLANE", "Allen", "Synapse",
    "Atlas", "Cortex", "Radicle", "Spark", "DGX", "GB10", "NVIDIA", "translate.js", "GitHub", "Rust",
    "Linux",
];

const BATCH_ITEMS: usize = 40;
const BATCH_CHARS: usize = 3000;

pub struct Limits {
    pub max_texts: usize,
    pub max_chars: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_texts: 40, max_chars: 3000 }
    }
}

const MAX_REQUEST_BYTES: usize = 16_384;
const CLIENTS_MAX: usize = 1024;
const REQUESTS_PER_MINUTE: u8 = 6;

struct ClientWindow {
    started: Instant,
    requests: u8,
}

/// A bounded, non-queuing admission gate. Socket IPs are used, not spoofable
/// forwarding headers. A reverse proxy must rate-limit its own real clients.
pub struct Admission {
    token: Option<String>,
    active: Semaphore,
    clients: Mutex<HashMap<std::net::IpAddr, ClientWindow>>,
}

impl Admission {
    pub fn new(token: Option<String>) -> Self {
        Self { token, active: Semaphore::new(1), clients: Mutex::new(HashMap::new()) }
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some(token) = &self.token else { return true };
        let Some(supplied) = headers.get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer ")) else { return false };
        supplied.len() == token.len() && bool::from(supplied.as_bytes().ct_eq(token.as_bytes()))
    }

    fn allow_client(&self, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let mut clients = self.clients.lock().unwrap();
        if clients.len() >= CLIENTS_MAX {
            clients.retain(|_, window| now.duration_since(window.started) < Duration::from_secs(60));
        }
        let ip = addr.ip();
        if !clients.contains_key(&ip) && clients.len() == CLIENTS_MAX {
            return false;
        }
        let window = clients.entry(ip).or_insert(ClientWindow { started: now, requests: 0 });
        if now.duration_since(window.started) >= Duration::from_secs(60) {
            *window = ClientWindow { started: now, requests: 0 };
        }
        if window.requests >= REQUESTS_PER_MINUTE { return false }
        window.requests += 1;
        true
    }
}

pub struct AppState {
    pub engine: engine::Engine,
    pub cache: cache::Cache,
    pub keep: Vec<String>,
    pub limits: Limits,
    pub admission: Admission,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub from_cache: usize,
    pub translated: usize,
    pub kept_original: usize,
    pub nothing_to_do: usize,
}

/// Split text into (kept lead, core to translate, kept trail).
///
/// The lead and trail are whitespace, plus pieces of a link that translate.js
/// cut apart: it splits text at "//", so "Read the docs at https://x.com now"
/// arrives as "Read the docs at https:" and "x.com now". Those pieces must
/// stay at the edge they came from or the link breaks when reassembled.
fn split_ws(s: &str) -> (&str, &str, &str) {
    use regex::Regex;
    use std::sync::OnceLock;
    static TAIL: OnceLock<Regex> = OnceLock::new();
    static HEAD: OnceLock<Regex> = OnceLock::new();
    let tail = TAIL.get_or_init(|| Regex::new(r"(?i)\s*\bhttps?:$").unwrap());
    let head = HEAD.get_or_init(|| {
        Regex::new(
            r"(?i)^(www\.)?[a-z0-9-]+(\.[a-z0-9-]+)*\.(com|org|net|io|dev|ai|app|co|me|info|edu|gov|cn|uk|de)(/\S*)?(\s+|$)",
        )
        .unwrap()
    });

    let trimmed = s.trim();
    if trimmed.is_empty() {
        return (s, "", "");
    }
    let mut a = s.find(trimmed).unwrap_or(0);
    let mut b = a + trimmed.len();
    if let Some(m) = tail.find(&s[a..b]) {
        b = a + m.start();
    }
    if let Some(m) = head.find(&s[a..b]) {
        a += m.end();
    }
    if a > b {
        a = b;
    }
    (&s[..a], &s[a..b], &s[b..])
}

pub async fn translate_texts(
    st: &AppState,
    from_id: &str,
    to_id: &str,
    texts: &[String],
) -> Result<(Vec<String>, Stats), String> {
    let to = langs::find(to_id).ok_or_else(|| format!("unsupported target language: {to_id}"))?;
    let from = langs::find(from_id);
    let mut stats = Stats::default();
    let mut out: Vec<String> = texts.to_vec();

    if from.map(|f| f.id) == Some(to.id) {
        stats.nothing_to_do = texts.len();
        return Ok((out, stats));
    }

    let model = st.engine.model().await?;
    let from_key = from.map(|f| f.id).unwrap_or("auto");

    // Which outputs each distinct text fills.
    let mut want: HashMap<String, Vec<usize>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for (i, t) in texts.iter().enumerate() {
        let (_, core, _) = split_ws(t);
        if core.is_empty() || guard::nothing_to_translate(core) {
            stats.nothing_to_do += 1;
            continue;
        }
        let key = cache::Cache::key(&model, from_key, to.id, core);
        if let Some(hit) = st.cache.get(&key) {
            let (lead, _, trail) = split_ws(t);
            out[i] = format!("{lead}{hit}{trail}");
            stats.from_cache += 1;
            continue;
        }
        if !want.contains_key(core) {
            order.push(core.to_string());
        }
        want.entry(core.to_string()).or_default().push(i);
    }

    // Batch the remaining texts; if the model's answer for a batch can't be
    // matched up, split the batch and try again, down to single items.
    let mut queue: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut cur_chars = 0;
    for core in order {
        let c = core.chars().count();
        if !cur.is_empty() && (cur.len() >= BATCH_ITEMS || cur_chars + c > BATCH_CHARS) {
            queue.push(std::mem::take(&mut cur));
            cur_chars = 0;
        }
        cur_chars += c;
        cur.push(core);
    }
    if !cur.is_empty() {
        queue.push(cur);
    }
    queue.reverse();

    let mut done: HashMap<String, Option<String>> = HashMap::new();
    while let Some(batch) = queue.pop() {
        match st.engine.translate(from, to, &batch, &st.keep).await {
            Ok(results) => {
                for (src, tr) in batch.iter().zip(results) {
                    let tr = guard::tidy(src, &tr);
                    if guard::check(src, &tr, &st.keep).is_none() {
                        st.cache.put(&cache::Cache::key(&model, from_key, to.id, src), &model, from_key, to.id, src, &tr);
                        done.insert(src.clone(), Some(tr));
                    } else {
                        done.insert(src.clone(), None);
                    }
                }
            }
            Err(e) if batch.len() > 1 => {
                eprintln!("aien-translate: batch of {} failed ({e}); splitting", batch.len());
                let mut a = batch;
                let b = a.split_off(a.len() / 2);
                queue.push(b);
                queue.push(a);
            }
            Err(e) => {
                eprintln!("aien-translate: item failed ({e}); keeping original");
                done.insert(batch[0].clone(), None);
            }
        }
    }

    for (core, idxs) in want {
        let result = done.get(&core).cloned().flatten();
        for i in idxs {
            match &result {
                Some(tr) => {
                    let (lead, _, trail) = split_ws(&texts[i]);
                    out[i] = format!("{lead}{tr}{trail}");
                    stats.translated += 1;
                }
                None => stats.kept_original += 1,
            }
        }
    }
    Ok((out, stats))
}

/// Merge query string and body parameters (form or JSON body).
fn params(uri: &Uri, headers: &HeaderMap, body: &Bytes) -> HashMap<String, String> {
    let mut p: HashMap<String, String> = HashMap::new();
    if let Some(q) = uri.query() {
        p.extend(form_urlencoded::parse(q.as_bytes()).into_owned());
    }
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("json"));
    if is_json {
        if let Ok(Value::Object(m)) = serde_json::from_slice::<Value>(body) {
            for (k, v) in m {
                let s = match v {
                    Value::String(s) => s,
                    other => other.to_string(),
                };
                p.insert(k, s);
            }
        }
    } else {
        p.extend(form_urlencoded::parse(body).into_owned());
    }
    p
}

/// translate.js sends `text` as a JSON array; accept a plain string too, and
/// one extra layer of URL encoding.
fn parse_text(raw: &str) -> Vec<String> {
    let mut s = raw.trim().to_string();
    if s.starts_with("%5B") || s.starts_with("%5b") || s.starts_with("%22") {
        if let Some((_, v)) = form_urlencoded::parse(format!("x={s}").as_bytes()).next() {
            s = v.into_owned();
        }
    }
    match serde_json::from_str::<Value>(&s) {
        Ok(Value::Array(a)) => a
            .into_iter()
            .map(|v| match v {
                Value::String(s) => s,
                other => other.to_string(),
            })
            .collect(),
        Ok(Value::String(one)) => vec![one],
        _ => vec![raw.to_string()],
    }
}

fn fail(info: impl Into<String>) -> Json<Value> {
    Json(json!({"result": 0, "info": info.into()}))
}

async fn translate_h(
    State(st): State<Arc<AppState>>, ConnectInfo(peer): ConnectInfo<SocketAddr>,
    uri: Uri, headers: HeaderMap, body: Bytes,
) -> (StatusCode, Json<Value>) {
    if !st.admission.authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, fail("unauthorized"));
    }
    if uri.query().is_some_and(|q| q.len() > MAX_REQUEST_BYTES) {
        return (StatusCode::PAYLOAD_TOO_LARGE, fail("request too large"));
    }
    let p = params(&uri, &headers, &body);
    let (Some(to), Some(text)) = (p.get("to"), p.get("text")) else {
        return (StatusCode::BAD_REQUEST, fail("missing 'to' or 'text'"));
    };
    let from = p.get("from").cloned().unwrap_or_default();
    let texts = parse_text(text);
    let chars: usize = texts.iter().map(|t| t.chars().count()).sum();
    if texts.len() > st.limits.max_texts || chars > st.limits.max_chars {
        return (StatusCode::PAYLOAD_TOO_LARGE, fail(format!(
            "request too large: {} texts / {chars} characters (limit {} / {})",
            texts.len(), st.limits.max_texts, st.limits.max_chars
        )));
    }
    if !st.admission.allow_client(peer) {
        return (StatusCode::TOO_MANY_REQUESTS, fail("client rate limit"));
    }
    let Ok(_permit) = st.admission.active.try_acquire() else {
        return (StatusCode::TOO_MANY_REQUESTS, fail("translation busy"));
    };
    let response = match translate_texts(&st, &from, to, &texts).await {
        Ok((out, s)) => Json(json!({
            "result": 1,
            "info": "success",
            "from": from,
            "to": to,
            "text": out,
            "aien": {
                "from_cache": s.from_cache,
                "translated": s.translated,
                "kept_original": s.kept_original,
                "nothing_to_do": s.nothing_to_do,
            },
        })),
        Err(e) => fail(e),
    };
    (StatusCode::OK, response)
}


async fn language_h() -> Json<Value> {
    let list: Vec<Value> = langs::LANGS
        .iter()
        .map(|l| json!({"id": l.id, "name": l.name, "serviceId": l.code}))
        .collect();
    Json(json!({"result": 1, "info": "success", "list": list}))
}

async fn connect_h(uri: Uri, headers: HeaderMap, body: Bytes) -> Json<Value> {
    let p = params(&uri, &headers, &body);
    Json(json!({"result": 1, "info": p.get("host").cloned().unwrap_or_default()}))
}

async fn init_h() -> Json<Value> {
    Json(json!({"result": 1, "info": "success", "version": "1.0.0", "serviceList": ["translate.service"]}))
}

async fn ip_h() -> Json<Value> {
    fail("location lookup is not offered by this server")
}

async fn health_h(State(st): State<Arc<AppState>>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    if !st.admission.authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, fail("unauthorized"));
    }
    match st.engine.model().await {
        Ok(m) => (StatusCode::OK, Json(json!({"result": 1, "info": "ok", "model": m, "languages": langs::LANGS.len()}))),
        Err(e) => (StatusCode::OK, fail(e)),
    }
}

pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/translate.json", any(translate_h))
        .route("/language.json", any(language_h))
        .route("/connectTest.json", any(connect_h))
        .route("/init.json", any(init_h))
        .route("/ip.json", any(ip_h))
        .route("/health", get(health_h))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_kept() {
        assert_eq!(split_ws("  hi there \n"), ("  ", "hi there", " \n"));
        assert_eq!(split_ws("   "), ("   ", "", ""));
    }

    #[test]
    fn cut_link_pieces_stay_at_the_edges() {
        assert_eq!(split_ws("Read the docs at https:"), ("", "Read the docs at", " https:"));
        assert_eq!(split_ws("www.aienos.com and run"), ("www.aienos.com ", "and run", ""));
        assert_eq!(split_ws("github.com/aien-dev/x"), ("github.com/aien-dev/x", "", ""));
        assert_eq!(split_ws("Node is fast"), ("", "Node is fast", ""));
    }

    #[test]
    fn text_forms() {
        assert_eq!(parse_text("[\"a\",\"b\"]"), vec!["a", "b"]);
        assert_eq!(parse_text("%5B%22a%22%5D"), vec!["a"]);
        assert_eq!(parse_text("plain"), vec!["plain"]);
    }
}
