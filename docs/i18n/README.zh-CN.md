# aien-translate

<!-- languages -->
[English](../../README.md) · **简体中文** · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> 本页面由运行在AIEN自有模型上的aien-translate自动翻译。[英文版](../../README.md)为原始版本。

使用您自己的AI模型，将您的网站翻译成31种语言。无需API密钥，无需云服务，访问者的内容永远不会离开您的服务器。

aien-translate 是 [translate.js](https://github.com/xnx3/translate) 的翻译服务器。它使用 AIEN 自有的推理运行时进行翻译。它也兼容任何提供相同 OpenAI 风格聊天 API 的服务器。

## 通过3个步骤翻译您的网站

**1. 启动服务器。** 您需要 Rust 和一个正在运行的模型服务器（AIEN 的 `aien-serve-http`，或任何 OpenAI 风格的聊天服务器）。

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. 将 translate.js 添加到您的页面。** 从 [其仓库](https://github.com/xnx3/translate) 下载 `translate.js` 并与您的网站一起托管。然后在 `</body>` 之前添加以下代码，将您的服务器地址填入：

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. 打开您的页面。** 会出现一个语言菜单，选择语言后即可原地翻译页面。如需从您自己的代码中选择语言，请调用 `translate.changeLanguage('japanese')`。

### 检查是否正常工作

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

第一个命令报告正在使用的模型。第二个命令返回 `"text"` 中的翻译结果，例如 `["こんにちは世界"]`。

## 它保护的内容

每次翻译在访问者看到之前都会经过检查：

- 链接、电子邮件地址、反引号中的代码，以及 `{name}` 或 `%s` 等占位符必须原样通过。
- 保留列表中的产品名称（AIEN、Omega、Allen 等）永远不会被翻译。使用 `--keep-file` 可提供您自己的列表。
- 如果翻译为空，或远长于原文，则会被丢弃。

如果任何检查失败，访问者将看到原始文本，而不是损坏的翻译。

## 它的工作原理

- translate.js 将页面文本发送到 `/translate.json`。服务器将文本分批发送给模型，一次一个请求，因为 GPU 可能被共享。
- 每次翻译都会被保存（SQLite），因此相同的文本不会被重复翻译。重复访问将瞬间完成。
- translate.js 在 `//` 处拆分网页地址。服务器会保留这些部分，以确保链接正确重建。
- 如果模型对某一批次的回答无法与输入匹配，该批次将被拆分并重试，直至单个项目。

## 选项

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

端点：`/translate.json`、`/language.json`、`/connectTest.json`、`/init.json` 和 `/health`。不提供 `/ip.json`（位置查询）。

## 语言

英语、简体中文、繁体中文、日语、韩语、西班牙语、法语、德语、葡萄牙语、意大利语、俄语、乌克兰语、波兰语、荷兰语、瑞典语、丹麦语、挪威语、芬兰语、捷克语、罗马尼亚语、匈牙利语、希腊语、土耳其语、阿拉伯语、波斯语、印地语、孟加拉语、泰语、越南语、印度尼西亚语和马来语。列表位于 `src/langs.rs` 中。

## 测试

- `cargo test` 运行单元测试，以及针对假模型的端到端测试（无需 GPU）。
- `demo/` 包含一个使用真实 translate.js 的页面。启动该文件夹并运行 `python demo/check.py URL`（需要 Playwright）以查看翻译后的页面。

## 此页面的其他语言版本

此页面的其他语言版本由 aien-translate 本身通过 `scripts/translate_readme.py` 制作。它们是机器翻译，英文版本为原始版本。

## 限制

- 速度取决于GPU。在AIEN的运行时环境中，当GPU空闲时，一批次大约需要一秒钟；当其他任务正在使用GPU时，耗时会更长。
- 这些是机器翻译。它们适合阅读，但未经人工审核。

## 许可证

AGPL-3.0。详见 [LICENSE](../../LICENSE)。
