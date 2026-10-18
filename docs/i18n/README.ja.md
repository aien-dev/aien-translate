# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · **日本語** · [한국어](README.ko.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> このページはAIEN独自のモデルで実行中のaien-translateによって自動翻訳されました。[英語版](../../README.md)がオリジナルです。

31の言語にあなたのウェブサイトを独自のAIモデルで翻訳。APIキーもクラウドサービスも不要で、訪問者のテキストはあなたのサーバーから出ません。

aien-translate は [translate.js](https://github.com/xnx3/translate) の翻訳サーバーです。AIEN独自の推論ランタイムで翻訳を行います。また、同じ OpenAI 風のチャット API を提供する任意のサーバーとも連携可能です。

## ウェブサイトを3ステップで翻訳

**1. サーバーを起動する。** Rust と動作中のモデルサーバー（AIENの `aien-serve-http`、または任意の OpenAI 風チャットサーバー）が必要です。

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. translate.js をページに追加する。** [リポジトリ](https://github.com/xnx3/translate) から `translate.js` をダウンロードし、あなたのサイトと一緒にホスティングしてください。その後、`</body>` の直前に以下の行を追加し、あなたのサーバーのアドレスを指定します：

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. ページを開く。** 言語メニューが表示され、言語を選択するとページが即座に翻訳されます。コードから言語を指定するには、`translate.changeLanguage('japanese')` を呼び出します。

### 動作しているか確認してください

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

最初のコマンドは使用中のモデルを報告します。2番目のコマンドは `"text"` として翻訳を返します。たとえば `["こんにちは世界"]` のようにです。

## 保護される内容

訪問者が表示する前にすべての翻訳が確認されます：

- リンク、メールアドレス、バックティック内のコード、および `{name}` や `%s` のようなプレースホルダーは変更されずに通過する必要があります。
- キーリストに登録された製品名（AIEN、Omega、Allen など）は翻訳されません。独自のリストを指定するには `--keep-file` を使用してください。
- 空の翻訳、または元のテキストと比べて大幅に長い翻訳は破棄されます。

チェックのいずれかが失敗した場合、訪問者は破損した翻訳の代わりに元のテキストを表示します。

## 仕組み

- translate.js はページのテキストを `/translate.json` に送信します。サーバーはGPUが共有されている可能性があるため、バッチごとに1回ずつリクエストを送信します。
- すべての翻訳は（SQLiteで）保存されるため、同じテキストは2回以上翻訳されません。再訪問は瞬時に完了します。
- translate.js はWebアドレスを `//` で分割します。サーバーはその部分を保持するため、リンクが正しく再構築されます。
- モデルのバッチに対する回答が入力と一致しない場合、バッチは単一の項目まで分割され、再試行されます。

## オプション

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

エンドポイント: `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json`, `/health`。`/ip.json`（位置情報の照会）は提供されていません。

## 言語

英語、簡体中国語、繁体中国語、日本語、韓国語、スペイン語、フランス語、ドイツ語、ポルトガル語、イタリア語、ロシア語、ウクライナ語、ポーランド語、オランダ語、スウェーデン語、デンマーク語、ノルウェー語、フィンランド語、チェコ語、ルーマニア語、ハンガリー語、ギリシャ語、トルコ語、アラビア語、ペルシア語、ヒンディー語、ベンガル語、タイ語、ベトナム語、インドネシア語、マレー語。リストは `src/langs.rs` にあります。

## テスト

- `cargo test` はユニットテストを実行し、本物のGPUを必要としない仮想モデルに対するエンドツーエンドテストも実行します。
- `demo/` には実際の translate.js を使用するページがあります。フォルダをサーブして `python demo/check.py URL` を実行すると（Playwrightが必要）、翻訳されたページを確認できます。

## このページの翻訳

このページの他の言語版は、aien-translate 自身が `scripts/translate_readme.py` を使って作成しました。これらは機械翻訳であり、英語版がオリジナルです。

## 制限

- 速度はGPUに依存します。AIENのランタイムでは、GPUが空いている場合、バッチ処理に約1秒かかりますが、他のジョブが使用中だと大幅に時間がかかります。
- これらは機械翻訳です。読みやすさはありますが、誰かが確認したわけではありません。

## ライセンス

AGPL-3.0。[LICENSE](../../LICENSE) を参照してください。
