# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · **Português** · [Русский](README.ru.md)
<!-- /languages -->

> Esta página foi traduzida automaticamente pelo aien-translate, em execução no próprio modelo do AIEN. A [versão em inglês](../../README.md) é a original.

Traduza seu site para 31 idiomas com seu próprio modelo de IA. Sem chave de API, sem serviço em nuvem e os textos dos visitantes nunca deixam seu servidor.

aien-translate é um servidor de tradução para [translate.js](https://github.com/xnx3/translate). Ele traduz com o próprio tempo de execução de inferência da AIEN. Também funciona com qualquer servidor que ofereça uma API de chat no estilo OpenAI.

## Traduza seu site em 3 etapas

**1. Inicie o servidor.** Você precisa do Rust e de um servidor de modelo em execução (o `aien-serve-http` da AIEN, ou qualquer servidor de chat no estilo OpenAI).

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. Adicione translate.js à sua página.** Baixe `translate.js` do [repositório](https://github.com/xnx3/translate) e hospede-o com seu site. Em seguida, adicione estas linhas logo antes de `</body>`, com o endereço do seu servidor:

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. Abra sua página.** Um menu de idiomas aparece, e escolher um idioma traduz a página no local. Para selecionar um idioma a partir do seu próprio código, chame `translate.changeLanguage('japanese')`.

### Verifique se está funcionando

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

O primeiro comando relata o modelo em uso. O segundo retorna a tradução em `"text"`, por exemplo `["こんにちは世界"]`.

## O que ele protege

Toda tradução é verificada antes que um visitante a veja:

- Links, endereços de e-mail, códigos entre acentos graves e placeholders como `{name}` ou `%s` devem passar sem alterações.
- Nomes de produtos na lista de proteção (AIEN, Omega, Allen e outros) nunca são traduzidos. Use `--keep-file` para fornecer sua própria lista.
- Uma tradução vazia ou muito mais longa que o original é descartada.

Se qualquer verificação falhar, o visitante verá o texto original em vez de uma tradução corrompida.

## Como funciona

- translate.js envia o texto da página para `/translate.json`. O servidor envia para o modelo em lotes, um pedido de cada vez, porque a GPU pode estar compartilhada.
- Toda tradução é salva (SQLite), então o mesmo texto nunca é traduzido duas vezes. Visitas repetidas são instantâneas.
- translate.js separa os endereços web em `//`. O servidor mantém essas partes no lugar para que os links sejam reconstruídos corretamente.
- Se a resposta do modelo para um lote não puder ser associada aos inputs, o lote é dividido e tentado novamente, até itens individuais.

## Opções

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

Endpoints: `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json` e `/health`. `/ip.json` (pesquisa de localização) não é oferecido.

## Idiomas

Inglês, Chinês Simplificado e Tradicional, Japonês, Coreano, Espanhol, Francês, Alemão, Português, Italiano, Russo, Ucraniano, Polonês, Holandês, Sueco, Dinamarquês, Norueguês, Finlandês, Checo, Romeno, Húngaro, Grego, Turco, Árabe, Persa, Hindi, Bengali, Tailandês, Vietnamita, Indonésio e Malaio. A lista está em `src/langs.rs`.

## Testes

- `cargo test` executa testes unitários, além de testes de ponta a ponta contra um modelo falso (não é necessário GPU).
- `demo/` contém uma página que usa o translate.js real. Sirva a pasta e execute `python demo/check.py URL` (necessita Playwright) para ver a página traduzida.

## Traduções desta página

As versões deste texto em outros idiomas foram feitas pelo próprio aien-translate, com `scripts/translate_readme.py`. São traduções automáticas e a versão em inglês é a original.

## Limites

- A velocidade depende da GPU. Na execução do AIEN, um lote leva cerca de um segundo quando a GPU está livre, e muito mais tempo quando outra tarefa está usando-a.
- Estas são traduções automáticas. São boas para leitura, mas não foram revisadas por ninguém.

## Licença

AGPL-3.0. Veja [LICENSE](../../LICENSE).
