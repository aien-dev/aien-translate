# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Español](README.es.md) · [Français](README.fr.md) · **Deutsch** · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> Diese Seite wurde automatisch mit aien-translate übersetzt, das auf AIENs eigenem Modell läuft. Die [englische Version](../../README.md) ist die Originalfassung.

Übersetzen Sie Ihre Website in 31 Sprachen mit Ihrem eigenen KI-Modell. Kein API-Schlüssel, keine Cloud-Dienste, und die Texte Ihrer Besucher verlassen niemals Ihren Server.

aien-translate ist ein Übersetzungs-Server für [translate.js](https://github.com/xnx3/translate). Er übersetzt mit der eigenen Inferenzlaufzeit von AIEN. Er funktioniert auch mit jedem Server, der die gleiche OpenAI-ähnliche Chat-API anbietet.

## Übersetzen Sie Ihre Website in 3 Schritten

**1. Starten Sie den Server.** Sie benötigen Rust und einen laufenden Modellserver (AIENs `aien-serve-http` oder einen beliebigen OpenAI-ähnlichen Chat-Server).

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. Fügen Sie translate.js Ihrer Seite hinzu.** Laden Sie `translate.js` aus [seinem Repository](https://github.com/xnx3/translate) herunter und stellen Sie es zusammen mit Ihrer Website bereit. Fügen Sie diese Zeilen kurz vor `</body>` ein, wobei Sie die Adresse Ihres Servers angeben:

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. Öffnen Sie Ihre Seite.** Ein Sprachmenü erscheint, und beim Auswählen einer Sprache wird die Seite direkt übersetzt. Um eine Sprache aus Ihrem eigenen Code heraus auszuwählen, rufen Sie `translate.changeLanguage('japanese')` auf.

### Überprüfen Sie, ob es funktioniert

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

Der erste Befehl meldet das verwendete Modell. Der zweite gibt die Übersetzung im Feld `"text"` zurück, beispielsweise `["こんにちは世界"]`.

## Was es schützt

Jede Übersetzung wird überprüft, bevor ein Besucher sie sieht:

- Links, E-Mail-Adressen, Code in Backticks und Platzhalter wie `{name}` oder `%s` müssen unverändert durchkommen.
- Produktnamen auf der Whitelist (AIEN, Omega, Aien und andere) werden niemals übersetzt. Verwenden Sie `--keep-file`, um Ihre eigene Liste bereitzustellen.
- Eine leere Übersetzung oder eine, die deutlich länger als der Originaltext ist, wird verworfen.

Wenn eine Überprüfung fehlschlägt, sieht der Besucher stattdessen den ursprünglichen Text anstelle einer fehlerhaften Übersetzung.

## Wie es funktioniert

- translate.js sendet den Text der Seite an `/translate.json`. Der Server sendet ihn in Batches, eine Anfrage nach der anderen, da die GPU möglicherweise geteilt wird.
- Jede Übersetzung wird gespeichert (SQLite), sodass derselbe Text nie zweimal übersetzt wird. Wiederholte Besuche sind sofort möglich.
- translate.js trennt Web-Adressen an `//`. Der Server behält diese Teile an ihrem Platz, sodass Links korrekt wiederhergestellt werden.
- Wenn die Antwort des Modells für ein Batch nicht den Eingaben zugeordnet werden kann, wird das Batch aufgeteilt und erneut versucht, bis hinunter zu einzelnen Elementen.

## Optionen

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

Endpunkte: `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json` und `/health`. `/ip.json` (Standortabfrage) wird nicht angeboten.

## Sprachen

Englisch, Vereinfachtes und Traditionelles Chinesisch, Japanisch, Koreanisch, Spanisch, Französisch, Deutsch, Portugiesisch, Italienisch, Russisch, Ukrainisch, Polnisch, Niederländisch, Schwedisch, Dänisch, Norwegisch, Finnisch, Tschechisch, Rumänisch, Ungarisch, Griechisch, Türkisch, Arabisch, Persisch, Hindi, Bengali, Thai, Vietnamesisch, Indonesisch und Malay. Die Liste befindet sich in `src/langs.rs`.

## Tests

- `cargo test` führt Einheitstests sowie End-to-End-Tests gegen ein fiktives Modell durch (keine GPU erforderlich).
- `demo/` enthält eine Seite, die das echte translate.js verwendet. Starten Sie den Ordner und führen Sie `python demo/check.py URL` aus (benötigt Playwright), um die übersetzte Seite zu sehen.

## Übersetzungen dieser Seite

Die Versionen dieser Seite in anderen Sprachen wurden von aien-translate selbst erstellt, mit `scripts/translate_readme.py`. Sie sind maschinelle Übersetzungen, und die englische Version ist die Originalfassung.

## Grenzen

- Die Geschwindigkeit hängt von der GPU ab. Bei AIENs Laufzeit dauert eine Batch etwa eine Sekunde, wenn die GPU frei ist, und viel länger, wenn ein anderer Job sie nutzt.
- Dies sind maschinelle Übersetzungen. Sie sind gut zum Lesen, wurden jedoch von keinem Menschen überprüft.

## Lizenz

AGPL-3.0. Siehe [LICENSE](../../LICENSE).
