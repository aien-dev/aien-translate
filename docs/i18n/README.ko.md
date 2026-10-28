# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · **한국어** · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> 이 페이지는 AIEN의 자체 모델에서 실행 중인 aien-translate에 의해 자동으로 번역되었습니다. [영문 버전](../../README.md)이 원본입니다.

31개의 언어로 당신의 웹사이트를 자체 AI 모델로 번역하세요. API 키도 필요 없고, 클라우드 서비스도 필요 없으며, 방문자의 텍스트는 절대 당신의 서버를 떠나지 않습니다.

aien-translate는 [translate.js](https://github.com/xnx3/translate)를 위한 번역 서버입니다. AIEN의 자체 추론 런타임으로 번역을 수행합니다. 또한 동일한 OpenAI 스타일의 채팅 API를 제공하는 모든 서버와도 작동합니다.

## 3단계로 웹사이트를 번역하세요

**1. 서버를 시작하세요.** Rust와 실행 중인 모델 서버(AIEN의 `aien-serve-http` 또는 어떤 OpenAI 스타일의 채팅 서버든)가 필요합니다.

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. 페이지에 translate.js를 추가하세요.** [그 리포지토리](https://github.com/xnx3/translate)에서 `translate.js`를 다운로드하여 사이트와 함께 호스팅하세요. 그런 다음 `</body>` 직전에 다음 줄을 추가하세요. 서버 주소를 입력하세요:

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. 페이지를 열어보세요.** 언어 메뉴가 나타나며, 언어를 선택하면 페이지가 즉시 번역됩니다. 코드에서 언어를 선택하려면 `translate.changeLanguage('japanese')`를 호출하세요.

### 작동 여부 확인하기

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

첫 번째 명령은 사용 중인 모델을 보고합니다. 두 번째 명령은 `"text"`에 번역 결과를 반환합니다. 예를 들어 `["こんにちは世界"]`와 같습니다.

## 무엇을 보호합니까

모든 번역은 방문자가 보기에 앞서 검사됩니다:

- 링크, 이메일 주소, 백틱 안의 코드, `{name}` 또는 `%s`와 같은 자리 표시자는 변경 없이 그대로 전달되어야 합니다.
- 보관 목록에 있는 제품 이름(AIEN, Omega, Allen 등)은 절대 번역되지 않습니다. 사용자 정의 목록을 제공하려면 `--keep-file`를 사용하세요.
- 번역 결과가 비어 있거나 원문보다 훨씬 길 경우, 해당 번역은 버려집니다.

어떤 검사도 실패하면, 방문자는 손상된 번역 대신 원문을 보게 됩니다.

## 작동 방식

- translate.js는 페이지의 텍스트를 `/translate.json`으로 보냅니다. 서버는 GPU가 공유될 수 있으므로, 한 번에 하나의 요청씩 배치로 모델에 전달합니다.
- 모든 번역은 (SQLite에) 저장되므로 동일한 텍스트는 두 번 번역되지 않습니다. 반복 방문 시 즉시 처리됩니다.
- translate.js는 웹 주소를 `//` 기준으로 나눕니다. 서버는 해당 부분을 그대로 유지하므로 링크가 올바르게 재구성됩니다.
- 모델의 응답이 입력과 일치하지 않으면, 해당 배치는 단일 항목까지 분할하여 다시 시도합니다.

## 옵션

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

엔드포인트: `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json`, `/health`. `/ip.json`(위치 조회)는 제공되지 않습니다.

## 언어

영어, 간체 중국어, 번체 중국어, 일본어, 한국어, 스페인어, 프랑스어, 독일어, 포르투갈어, 이탈리아어, 러시아어, 우크라이나어, 폴란드어, 네덜란드어, 스웨덴어, 덴마크어, 노르웨이어, 핀란드어, 체코어, 루마니아어, 헝가리어, 그리스어, 터키어, 아랍어, 페르시아어, 힌디어, 벵골어, 태국어, 베트남어, 인도네시아어, 말레이어. 목록은 `src/langs.rs`에 있습니다.

## 테스트

- `cargo test`는 단위 테스트와 가짜 모델을 사용한 엔드투엔드 테스트를 실행합니다(GPU 필요 없음).
- `demo/`에는 실제 translate.js를 사용하는 페이지가 있습니다. 폴더를 서빙하고 `python demo/check.py URL`을 실행하세요(Playwright 필요). 번역된 페이지를 확인할 수 있습니다.

## 이 페이지의 번역본

이 페이지의 다른 언어 버전은 aien-translate 자체가 `scripts/translate_readme.py`를 사용해 만들었습니다. 이 번역은 기계 번역이며, 영문 버전이 원본입니다.

## 제한

- 속도는 GPU에 따라 달라집니다. AIEN 런타임에서 GPU가 사용 가능할 경우 배치 처리는 약 1초가 걸리며, 다른 작업이 사용 중이면 훨씬 더 오래 걸립니다.
- 이 번역은 기계 번역입니다. 읽기에 적합하지만, 사람의 검토는 이루어지지 않았습니다.

## 라이선스

AGPL-3.0. [LICENSE](../../LICENSE)를 참조하세요.
