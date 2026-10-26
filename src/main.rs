use aien_translate::{app, cache::Cache, engine::Engine, Admission, AppState, Limits, DEFAULT_KEEP};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

const USAGE: &str = "aien-translate [--listen ADDR] [--engine URL] [--cache FILE] [--keep-file FILE] [--auth-token-file FILE]

  --listen           loopback address only          (default 127.0.0.1:18097)
  --engine           AIEN inference server          (default http://127.0.0.1:18091)
  --cache            saved translations             (default ~/.local/state/aien-translate/cache.sqlite)
  --keep-file        names never translated, one per line (default: built-in list)
  --auth-token-file  bearer token for a trusted local proxy; required when proxying to external clients";

#[tokio::main]
async fn main() {
    let mut listen = "127.0.0.1:18097".to_string();
    let mut engine = "http://127.0.0.1:18091".to_string();
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let mut cache = PathBuf::from(home).join(".local/state/aien-translate/cache.sqlite");
    let mut keep: Vec<String> = DEFAULT_KEEP.iter().map(|s| s.to_string()).collect();
    let mut token_file: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "--listen" => listen = val(),
            "--engine" => engine = val(),
            "--cache" => cache = PathBuf::from(val()),
            "--auth-token-file" => token_file = Some(PathBuf::from(val())),
            "--keep-file" => {
                let path = val();
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| die(&format!("{path}: {e}")));
                keep = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(String::from).collect();
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => die(&format!("unknown argument {other}\n\n{USAGE}")),
        }
    }
    let addr: SocketAddr = listen.parse().unwrap_or_else(|e| die(&format!("invalid --listen address: {e}")));
    if !addr.ip().is_loopback() {
        die("--listen must be loopback; expose through an authenticated, rate-limited reverse proxy");
    }
    let token = token_file.map(|path| {
        let metadata = std::fs::metadata(&path).unwrap_or_else(|e| die(&format!("token file {}: {e}", path.display())));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                die("token file must not be accessible to group or others");
            }
        }
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| die(&format!("token file {}: {e}", path.display())));
        let token = raw.trim_end_matches(['\r', '\n']);
        if token.len() < 32 || !token.bytes().all(|b| b.is_ascii_graphic()) {
            die("token must be a single line of at least 32 printable ASCII bytes without spaces");
        }
        token.to_string()
    });

    let state = Arc::new(AppState {
        engine: Engine::new(&engine),
        cache: Cache::open(&cache).unwrap_or_else(|e| die(&format!("cache {}: {e}", cache.display()))),
        keep,
        limits: Limits::default(),
        admission: Admission::new(token),
    });
    match state.engine.model().await {
        Ok(m) => eprintln!("aien-translate: engine {engine} serves {m}"),
        Err(e) => eprintln!("aien-translate: engine not ready yet ({e}); will retry per request"),
    }
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| die(&format!("{listen}: {e}")));
    eprintln!("aien-translate: listening on http://{listen}");
    axum::serve(listener, app(state).into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

fn die(msg: &str) -> ! {
    eprintln!("aien-translate: {msg}");
    std::process::exit(2);
}
