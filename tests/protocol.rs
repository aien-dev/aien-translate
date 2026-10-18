//! End-to-end protocol tests against a fake engine (no GPU needed).

use aien_translate::{app, cache::Cache, engine::Engine, AppState, Limits};
use axum::{extract::State, routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;


/// Fake engine: "translates" by prefixing "ZH:". Misbehaves on purpose for
/// strings containing DROPLINK (drops the URL) or BADCOUNT (wrong item count
/// when batched with others).
async fn fake_chat(State(calls): State<Arc<AtomicUsize>>, Json(req): Json<Value>) -> Json<Value> {
    calls.fetch_add(1, Ordering::SeqCst);
    let user = req["messages"][1]["content"].as_str().unwrap();
    let items: Vec<String> = serde_json::from_str(user).unwrap();
    let mut out: Vec<String> = items
        .iter()
        .map(|s| {
            if s.contains("DROPLINK") {
                "ZH: link gone".to_string()
            } else {
                format!("ZH:{s}")
            }
        })
        .collect();
    if items.len() > 1 && items.iter().any(|s| s.contains("BADCOUNT")) {
        out.pop();
    }
    let content = format!("```json\n{}\n```", serde_json::to_string(&out).unwrap());
    Json(json!({"choices": [{"message": {"role": "assistant", "content": content}}]}))
}

async fn start() -> String {
    start_counted().await.0
}

async fn start_counted() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let fake = Router::new()
        .route("/v1/models", get(|| async { Json(json!({"data": [{"id": "fake-model"}]})) }))
        .route("/v1/chat/completions", post(fake_chat))
        .with_state(calls.clone());
    let fl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let faddr = fl.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(fl, fake).await.unwrap() });

    let state = Arc::new(AppState {
        engine: Engine::new(&format!("http://{faddr}")),
        cache: Cache::in_memory().unwrap(),
        keep: vec!["AIEN".into()],
        limits: Limits { max_texts: 50, max_chars: 5000 },
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app(state)).await.unwrap() });
    (format!("http://{addr}"), calls)
}

/// Post exactly the way translate.js does: hand-joined form fields, with
/// `text` = encodeURIComponent(JSON.stringify(array)).
async fn post_like_translate_js(base: &str, texts: &[&str]) -> Value {
    let json_text = serde_json::to_string(texts).unwrap();
    let encoded: String = form_urlencoded::byte_serialize(json_text.as_bytes()).collect::<String>().replace('+', "%20");
    let body = format!("from=english&to=chinese_simplified&text={encoded}&browserDefaultLanguage=english");
    reqwest::Client::new()
        .post(format!("{base}/translate.json?v=4.1.0.20260526"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn translate_js_protocol_round_trip() {
    let (base, calls) = start_counted().await;

    let r = post_like_translate_js(&base, &["Hello world", "  Run AIEN + more ", "12,345", "Hello world"]).await;
    assert_eq!(r["result"], 1, "{r}");
    let text: Vec<String> = serde_json::from_value(r["text"].clone()).unwrap();
    assert_eq!(text, vec!["ZH:Hello world", "  ZH:Run AIEN + more ", "12,345", "ZH:Hello world"]);
    assert_eq!(r["aien"]["translated"], 3);
    assert_eq!(r["aien"]["nothing_to_do"], 1);

    // Second time: all from the saved translations, no engine call.
    let before = calls.load(Ordering::SeqCst);
    let r2 = post_like_translate_js(&base, &["Hello world"]).await;
    assert_eq!(r2["text"][0], "ZH:Hello world");
    assert_eq!(r2["aien"]["from_cache"], 1);
    assert_eq!(calls.load(Ordering::SeqCst), before);
}

#[tokio::test]
async fn broken_translation_keeps_original() {
    let base = start().await;
    let r = post_like_translate_js(&base, &["See https://aienos.com DROPLINK"]).await;
    assert_eq!(r["text"][0], "See https://aienos.com DROPLINK");
    assert_eq!(r["aien"]["kept_original"], 1);
}

#[tokio::test]
async fn mismatched_batch_is_split_and_recovers() {
    let base = start().await;
    let r = post_like_translate_js(&base, &["one", "two BADCOUNT", "three"]).await;
    let text: Vec<String> = serde_json::from_value(r["text"].clone()).unwrap();
    assert_eq!(text, vec!["ZH:one", "ZH:two BADCOUNT", "ZH:three"]);
}

#[tokio::test]
async fn housekeeping_endpoints() {
    let base = start().await;
    let c = reqwest::Client::new();
    let lang: Value = c.post(format!("{base}/language.json")).send().await.unwrap().json().await.unwrap();
    assert_eq!(lang["result"], 1);
    assert!(lang["list"].as_array().unwrap().iter().any(|l| l["id"] == "chinese_simplified"));

    let ct: Value = c
        .post(format!("{base}/connectTest.json"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("host=https://x/")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ct["info"], "https://x/");

    let bad: Value = c
        .post(format!("{base}/translate.json"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("from=english&to=klingon&text=%5B%22hi%22%5D")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bad["result"], 0);
}

#[tokio::test]
async fn oversized_request_is_refused() {
    let base = start().await;
    let many: Vec<&str> = vec!["x y"; 60];
    let r = post_like_translate_js(&base, &many).await;
    assert_eq!(r["result"], 0);
}
