//! End-to-end protocol tests against a fake engine (no GPU needed).

use aien_translate::{app, cache::Cache, engine::Engine, Admission, AppState, Limits};
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
    (start_with(fake).await, calls)
}

/// Start aien-translate in front of the given fake engine.
async fn start_with(fake: Router) -> String {
    let fl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let faddr = fl.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(fl, fake).await.unwrap() });

    let state = Arc::new(AppState {
        engine: Engine::new(&format!("http://{faddr}")),
        cache: Cache::in_memory().unwrap(),
        keep: vec!["AIEN".into()],
        limits: Limits { max_texts: 40, max_chars: 3000 },
        admission: Admission::new(Some("test-token-0123456789012345678901234567".into())),
    });
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app(state).into_make_service_with_connect_info::<std::net::SocketAddr>()).await.unwrap() });
    format!("http://{addr}")
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
        .bearer_auth("test-token-0123456789012345678901234567")
        .body(body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn anonymous_translation_cannot_use_inference() {
    let (base, calls) = start_counted().await;
    let response = reqwest::Client::new()
        .post(format!("{base}/translate.json"))
        .body("from=english&to=japanese&text=uncached")
        .send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn bad_token_and_cross_origin_are_not_admitted() {
    let (base, calls) = start_counted().await;
    let client = reqwest::Client::new();
    let rejected = client.post(format!("{base}/translate.json"))
        .bearer_auth("wrong-token").body("to=japanese&text=uncached")
        .send().await.unwrap();
    assert_eq!(rejected.status(), reqwest::StatusCode::UNAUTHORIZED);
    let preflight = client.request(reqwest::Method::OPTIONS, format!("{base}/translate.json"))
        .header("origin", "https://untrusted.example")
        .header("access-control-request-method", "POST")
        .send().await.unwrap();
    assert!(preflight.headers().get("access-control-allow-origin").is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let allowed = post_like_translate_js(&base, &["uncached"]).await;
    assert_eq!(allowed["result"], 1, "{allowed}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn peer_rate_limit_refuses_uncached_work() {
    let (base, calls) = start_counted().await;
    for n in 0..6 {
        let text = format!("uncached {n}");
        assert_eq!(post_like_translate_js(&base, &[&text]).await["result"], 1);
    }
    let response = reqwest::Client::new()
        .post(format!("{base}/translate.json"))
        .bearer_auth("test-token-0123456789012345678901234567")
        .body("from=english&to=japanese&text=seventh")
        .send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(calls.load(Ordering::SeqCst), 6);
}

#[test]
fn non_loopback_binding_is_refused() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_aien-translate"))
        .args(["--listen", "0.0.0.0:0"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--listen must be loopback"));
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
        .bearer_auth("test-token-0123456789012345678901234567")
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

#[tokio::test]
async fn body_limit_rejects_before_inference() {
    let (base, calls) = start_counted().await;
    let response = reqwest::Client::new()
        .post(format!("{base}/translate.json"))
        .bearer_auth("test-token-0123456789012345678901234567")
        .body(format!("to=japanese&text={}", "x".repeat(17_000)))
        .send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn in_flight_request_does_not_queue_another() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let fake = Router::new()
        .route("/v1/models", get(|| async { Json(json!({"data": [{"id": "fake-model"}]})) }))
        .route("/v1/chat/completions", post({
            let entered = entered.clone();
            let release = release.clone();
            move |Json(req): Json<Value>| {
                let entered = entered.clone();
                let release = release.clone();
                let calls = calls.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    fake_chat(State(calls), Json(req)).await
                }
            }
        }));
    let base = start_with(fake).await;
    let first_base = base.clone();
    let first = tokio::spawn(async move { post_like_translate_js(&first_base, &["first text"]).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), entered.notified()).await.unwrap();
    let response = reqwest::Client::new()
        .post(format!("{base}/translate.json"))
        .bearer_auth("test-token-0123456789012345678901234567")
        .body("from=english&to=japanese&text=second%20text")
        .send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    release.notify_one();
    assert_eq!(first.await.unwrap()["result"], 1);
}

fn sse(delta: &str) -> String {
    format!("data: {}\n\n", json!({"choices": [{"index": 0, "delta": {"content": delta}}]}))
}

/// The model finishes the list, then keeps writing junk (as seen on the real
/// server). aien-translate must hang up once the list is complete instead of
/// waiting for the junk to end.
#[tokio::test]
async fn stops_reading_once_the_list_is_complete() {
    let junk_sent = Arc::new(AtomicUsize::new(0));
    let js = junk_sent.clone();
    let fake = Router::new()
        .route("/v1/models", get(|| async { Json(json!({"data": [{"id": "fake-model"}]})) }))
        .route(
            "/v1/chat/completions",
            post(move |Json(req): Json<Value>| {
                let js = js.clone();
                async move {
                    let items: Vec<String> =
                        serde_json::from_str(req["messages"][1]["content"].as_str().unwrap()).unwrap();
                    let list = serde_json::to_string(&items.iter().map(|s| format!("ZH:{s}")).collect::<Vec<_>>()).unwrap();
                    if req["stream"] != json!(true) {
                        // Not streaming: the whole reply, junk included, arrives only at the end.
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        let content = format!("{list}\n\n[{{\"name\": \"junk\"");
                        return axum::response::Response::builder()
                            .header("content-type", "application/json")
                            .body(axum::body::Body::from(
                                json!({"choices": [{"message": {"role": "assistant", "content": content}}]}).to_string(),
                            ))
                            .unwrap();
                    }
                    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::convert::Infallible>>(1);
                    tokio::spawn(async move {
                        for piece in list.as_bytes().chunks(4) {
                            if tx.send(Ok(sse(std::str::from_utf8(piece).unwrap()))).await.is_err() {
                                return;
                            }
                        }
                        for _ in 0..100 {
                            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                            if tx.send(Ok(sse("{\"name\": \"junk\"} "))).await.is_err() {
                                return;
                            }
                            js.fetch_add(1, Ordering::SeqCst);
                        }
                        let _ = tx.send(Ok("data: [DONE]\n\n".to_string())).await;
                    });
                    axum::response::Response::builder()
                        .header("content-type", "text/event-stream")
                        .body(axum::body::Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx)))
                        .unwrap()
                }
            }),
        );
    let base = start_with(fake).await;
    let t = std::time::Instant::now();
    let r = post_like_translate_js(&base, &["alpha", "beta"]).await;
    let took = t.elapsed();
    assert_eq!(r["result"], 1, "{r}");
    assert_eq!(r["text"], json!(["ZH:alpha", "ZH:beta"]));
    assert!(took < std::time::Duration::from_millis(1500), "took {took:?}: kept reading after the list closed");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(junk_sent.load(Ordering::SeqCst) < 10, "fake engine kept streaming: {} junk chunks", junk_sent.load(Ordering::SeqCst));
}

/// The model server answers 503 "busy, retry shortly" while it finishes a
/// cancelled reply or while another job holds the GPU. aien-translate must
/// wait and retry, not give up and show the original text.
#[tokio::test]
async fn busy_engine_is_retried() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let fake = Router::new()
        .route("/v1/models", get(|| async { Json(json!({"data": [{"id": "fake-model"}]})) }))
        .route(
            "/v1/chat/completions",
            post(move |Json(req): Json<Value>| {
                let c = c.clone();
                async move {
                    if c.fetch_add(1, Ordering::SeqCst) < 2 {
                        return axum::response::Response::builder()
                            .status(503)
                            .header("content-type", "application/json")
                            .header("retry-after", "0")
                            .body(axum::body::Body::from(
                                json!({"error": {"message": "one generation at a time: the engine is busy with another request, retry shortly"}}).to_string(),
                            ))
                            .unwrap();
                    }
                    let items: Vec<String> =
                        serde_json::from_str(req["messages"][1]["content"].as_str().unwrap()).unwrap();
                    let list = serde_json::to_string(&items.iter().map(|s| format!("ZH:{s}")).collect::<Vec<_>>()).unwrap();
                    axum::response::Response::builder()
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(
                            json!({"choices": [{"message": {"role": "assistant", "content": list}}]}).to_string(),
                        ))
                        .unwrap()
                }
            }),
        );
    let base = start_with(fake).await;
    let r = post_like_translate_js(&base, &["alpha", "beta"]).await;
    assert_eq!(r["text"], json!(["ZH:alpha", "ZH:beta"]), "{r}");
    assert_eq!(r["aien"]["kept_original"], 0);
    assert_eq!(calls.load(Ordering::SeqCst), 3, "one request, retried twice");
}
