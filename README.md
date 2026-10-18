# aien-translate

<!-- languages -->
**English** · [简体中文](docs/i18n/README.zh-CN.md) · [繁體中文](docs/i18n/README.zh-TW.md) · [日本語](docs/i18n/README.ja.md) · [한국어](docs/i18n/README.ko.md) · [Español](docs/i18n/README.es.md) · [Français](docs/i18n/README.fr.md) · [Deutsch](docs/i18n/README.de.md) · [Português](docs/i18n/README.pt.md) · [Русский](docs/i18n/README.ru.md)
<!-- /languages -->

Translate your website into 31 languages with your own AI model. No API key, no cloud service, and your visitors' text never leaves your server.

aien-translate is a translation server for [translate.js](https://github.com/xnx3/translate). It translates with AIEN's own inference runtime. It also works with any server that offers the same OpenAI-style chat API.

## Translate your website in 3 steps

**1. Start the server.** You need Rust and a running model server (AIEN's `aien-serve-http`, or any OpenAI-style chat server).

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. Add translate.js to your page.** Download `translate.js` from [its repository](https://github.com/xnx3/translate) and host it with your site. Then add these lines just before `</body>`, with your server's address:

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. Open your page.** A language menu appears, and choosing a language translates the page in place. To pick a language from your own code, call `translate.changeLanguage('japanese')`.

### Check that it works

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

The first command reports the model in use. The second returns the translation in `"text"`, for example `["こんにちは世界"]`.

## What it protects

Every translation is checked before a visitor sees it:

- Links, email addresses, code in backticks, and placeholders such as `{name}` or `%s` must come through unchanged.
- Product names on the keep list (AIEN, Omega, Allen, and others) are never translated. Use `--keep-file` to supply your own list.
- A translation that is empty, or far longer than the original, is thrown away.

If any check fails, the visitor sees the original text instead of a broken translation.

## How it works

- translate.js sends the page's text to `/translate.json`. The server sends it to the model in batches, one request at a time, because the GPU may be shared.
- Every translation is saved (SQLite), so the same text is never translated twice. Repeat visits are instant.
- translate.js cuts web addresses apart at `//`. The server keeps those pieces in place so links are rebuilt correctly.
- If the model's answer for a batch can't be matched to the inputs, the batch is split and retried, down to single items.

## Options

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

Endpoints: `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json` and `/health`. `/ip.json` (location lookup) is not offered.

## Languages

English, Simplified and Traditional Chinese, Japanese, Korean, Spanish, French, German, Portuguese, Italian, Russian, Ukrainian, Polish, Dutch, Swedish, Danish, Norwegian, Finnish, Czech, Romanian, Hungarian, Greek, Turkish, Arabic, Persian, Hindi, Bengali, Thai, Vietnamese, Indonesian and Malay. The list is in `src/langs.rs`.

## Tests

- `cargo test` runs unit tests, plus end-to-end tests against a fake model (no GPU needed).
- `demo/` has a page that uses the real translate.js. Serve the folder and run `python demo/check.py URL` (needs Playwright) to see the translated page.

## Translations of this page

The other-language versions of this page were made by aien-translate itself, with `scripts/translate_readme.py`. They are machine translations, and the English version is the original.

## Limits

- Speed depends on the GPU. On AIEN's runtime, a batch takes about a second when the GPU is free, and much longer when another job is using it.
- These are machine translations. They are good for reading, but no person has reviewed them.

## License

AGPL-3.0. See [LICENSE](LICENSE).
