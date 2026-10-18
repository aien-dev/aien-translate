# aien-translate

A translation server for [translate.js](https://github.com/xnx3/translate) that
translates with AIEN's own inference runtime (`aien-serve-http`, Omega engine)
instead of Google, Microsoft or a cloud model. No API key, and page text never
leaves the machine running it.

It speaks translate.js's `translate.service` protocol, so a website only needs:

```html
<script src="translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://YOUR-SERVER/');
  translate.execute();
</script>
```

## What it does

- `POST /translate.json` with `from`, `to`, `text` (JSON array), exactly as
  translate.js sends it. Also `/language.json`, `/connectTest.json`,
  `/init.json`, `/ip.json` (refused: no location lookup), and `/health`.
- Sends text in batches to the engine's OpenAI-style `/v1/chat/completions`,
  one request at a time because the GPU is shared.
- **Saves every translation** (SQLite), so the same text is never translated
  twice.
- **Checks every translation before using it.** Links, email addresses, code in
  backticks, placeholders like `{name}` or `%s`, and a list of names (AIEN,
  Omega, Allen, ...) must come through unchanged, and the result must not be
  empty or far longer than the source. If a check fails, the original text is
  shown instead.
- Keeps link pieces in place: translate.js cuts text at `//`, so
  `see https://x.com now` arrives as `see https:` and `x.com now`. The server
  pins those pieces to the edge they came from so the link reassembles.
- If the model's answer for a batch can't be matched up item by item, the batch
  is split and retried, down to single items.
- 31 languages (see `src/langs.rs`), using translate.js's language ids.

## Run

```
cargo build --release
./target/release/aien-translate \
  --listen 127.0.0.1:18097 \
  --engine http://127.0.0.1:18091 \
  --cache ~/.local/state/aien-translate/cache.sqlite \
  [--keep-file names.txt]
```

## Test

- `cargo test`: unit tests plus end-to-end protocol tests against a fake engine
  (no GPU).
- `demo/`: a page using the real translate.js. Serve `demo/` and run
  `demo/check.py URL` (Playwright) to see the translated page.

## Limits

- Speed depends on the shared GPU: about 1 second per batch when it is idle,
  much slower when another job is using it.
- Machine translation. Good for reading, but not reviewed by a person.
- The location lookup (`ip.json`) is not offered.
