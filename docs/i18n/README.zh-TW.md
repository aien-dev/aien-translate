# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · **繁體中文** · [日本語](README.ja.md) · [한국어](README.ko.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> 本頁由AIEN自家模型運行的aien-translate自動翻譯。[英文版](../../README.md)為原始版本。

使用您自己的AI模型，將網站翻譯成31種語言。無需API金鑰，無需雲端服務，訪客的文字永遠不會離開您的伺服器。

aien-translate 是 [translate.js](https://github.com/xnx3/translate) 的翻譯伺服器。它使用 AIEN 自有的推理執行環境進行翻譯。同時也支援任何提供相同 OpenAI 風格聊天 API 的伺服器。

## 在3個步驟內翻譯您的網站

**1. 啟動伺服器。** 您需要 Rust 和一個正在運行的模型伺服器（AIEN 的 `aien-serve-http`，或任何 OpenAI 風格的聊天伺服器）。

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. 將 translate.js 加入您的頁面。** 從 [其儲存庫](https://github.com/xnx3/translate) 下載 `translate.js`，並與您的網站一起托管。然後在 `</body>` 之前加入以下程式碼，並填入您的伺服器位址：

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. 開啟您的頁面。** 會出現語言選單，選擇語言後，頁面會即時翻譯。若要從您自己的程式碼中選擇語言，請呼叫 `translate.changeLanguage('japanese')`。

### 檢查是否運作正常

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

第一個指令會報告正在使用的模型。第二個指令會以 `"text"` 返回翻譯結果，例如 `["こんにちは世界"]`。

## 它保護什麼

每次翻譯在訪客看到之前都會經過檢查：

- 連結、電子郵件地址、反引號中的程式碼，以及像 `{name}` 或 `%s` 這樣的佔位符必須完全不變地通過。
- 保留名單上的產品名稱（AIEN、Omega、Aien 等）永遠不會被翻譯。可使用 `--keep-file` 提供您自己的名單。
- 若翻譯結果為空，或遠長於原文，則會被捨棄。

若任何檢查失敗，訪客將看到原始文字，而非錯誤的翻譯。

## 它如何運作

- translate.js 將頁面的文字傳送至 `/translate.json`。伺服器會以批次方式，一次一個請求地將文字送至模型，因為 GPU 可能被共用。
- 每次翻譯都會儲存（SQLite），因此相同的文字不會被重複翻譯。重複造訪時立即完成。
- translate.js 會在 `//` 處切分網址。伺服器會保留這些片段，以正確重建連結。
- 若模型對某一批次的回應無法與輸入匹配，該批次會被拆分並重試，直到單一項目為止。

## 選項

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

端點：`/translate.json`、`/language.json`、`/connectTest.json`、`/init.json` 和 `/health`。不提供 `/ip.json`（位置查詢）。

## 語言

英文、簡體中文、繁體中文、日文、韓文、西班牙文、法文、德文、葡萄牙文、義大利文、俄文、烏克蘭文、波蘭文、荷蘭文、瑞典文、丹麥文、挪威文、芬蘭文、捷克文、羅馬尼亞文、匈牙利文、希臘文、土耳其文、阿拉伯文、波斯文、印地文、孟加拉文、泰文、越南文、印尼文和馬來文。清單位於 `src/langs.rs` 中。

## 測試

- `cargo test` 會執行單元測試，以及對虛擬模型的端對端測試（無需 GPU）。
- `demo/` 中有一個使用真實 translate.js 的頁面。啟動該資料夾並執行 `python demo/check.py URL`（需安裝 Playwright）以查看翻譯後的頁面。

## 此頁面的其他語言版本

此頁面的其他語言版本由 aien-translate 本身製作，使用 `scripts/translate_readme.py`。這些是機器翻譯，英文版本為原始版本。

## 限制

- 速度取決於GPU。在AIEN的執行環境中，當GPU空閒時，一批次大約需要一秒，當有其他工作正在使用GPU時，則會花更長時間。
- 這些是機器翻譯。適合閱讀，但並無人工審核。

## 授權

AGPL-3.0。詳見 [LICENSE](../../LICENSE)。
