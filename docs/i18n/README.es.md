# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · **Español** · [Français](README.fr.md) · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> Esta página fue traducida automáticamente por aien-translate, que se ejecuta con el propio modelo de AIEN. La [versión en inglés](../../README.md) es la original.

Traduce tu sitio web a 31 idiomas con tu propio modelo de IA. Sin clave de API, sin servicio en la nube y el texto de tus visitantes nunca abandona tu servidor.

aien-translate es un servidor de traducción para [translate.js](https://github.com/xnx3/translate). Traduce con el tiempo de ejecución de inferencia propio de AIEN. También funciona con cualquier servidor que ofrezca una API de chat estilo OpenAI.

## Traduce tu sitio web en 3 pasos

**1. Inicia el servidor.** Necesitas Rust y un servidor de modelos en funcionamiento (el `aien-serve-http` de AIEN, o cualquier servidor de chat con estilo OpenAI).

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. Añade translate.js a tu página.** Descarga `translate.js` desde [su repositorio](https://github.com/xnx3/translate) y almacénalo con tu sitio. Luego añade estas líneas justo antes de `</body>`, con la dirección de tu servidor:

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. Abre tu página.** Aparece un menú de idiomas, y al elegir un idioma se traduce la página en su lugar. Para seleccionar un idioma desde tu propio código, llama a `translate.changeLanguage('japanese')`.

### Comprueba que funciona

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

El primer comando informa sobre el modelo en uso. El segundo devuelve la traducción en `"text"`, por ejemplo `["こんにちは世界"]`.

## Lo que protege

Cada traducción se verifica antes de que un visitante la vea:

- Los enlaces, direcciones de correo electrónico, código entre acentos graves y marcadores como `{name}` o `%s` deben pasar sin cambios.
- Los nombres de productos en la lista de conservación (AIEN, Omega, Allen y otros) nunca se traducen. Usa `--keep-file` para proporcionar tu propia lista.
- Una traducción vacía o mucho más larga que el original se descarta.

Si alguna verificación falla, el visitante verá el texto original en lugar de una traducción defectuosa.

## Cómo funciona

- translate.js envía el texto de la página a `/translate.json`. El servidor lo envía al modelo por lotes, uno a la vez, porque la GPU puede estar compartida.
- Cada traducción se guarda (SQLite), por lo que el mismo texto nunca se traduce dos veces. Las visitas posteriores son instantáneas.
- translate.js separa las direcciones web en `//`. El servidor mantiene esas partes en su lugar para reconstruir correctamente los enlaces.
- Si la respuesta del modelo para un lote no puede coincidir con las entradas, el lote se divide y se vuelve a intentar, hasta llegar a elementos individuales.

## Opciones

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

Endpoints: `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json` y `/health`. `/ip.json` (búsqueda de ubicación) no está disponible.

## Idiomas

Inglés, chino simplificado y tradicional, japonés, coreano, español, francés, alemán, portugués, italiano, ruso, ucraniano, polaco, neerlandés, sueco, danés, noruego, finlandés, checo, rumano, húngaro, griego, turco, árabe, persa, hindi, bengalí, tailandés, vietnamita, indonesio y malayo. La lista está en `src/langs.rs`.

## Pruebas

- `cargo test` ejecuta pruebas unitarias, además de pruebas de extremo a extremo contra un modelo falso (no se necesita GPU).
- `demo/` contiene una página que usa translate.js real. Sirve la carpeta y ejecuta `python demo/check.py URL` (requiere Playwright) para ver la página traducida.

## Traducciones de esta página

Las versiones en otros idiomas de esta página fueron creadas por aien-translate mismo, con `scripts/translate_readme.py`. Son traducciones automáticas, y la versión en inglés es la original.

## Límites

- La velocidad depende de la GPU. En la ejecución de AIEN, un lote tarda aproximadamente un segundo cuando la GPU está libre, y mucho más tiempo cuando otra tarea la está usando.
- Estas son traducciones automáticas. Son útiles para leer, pero ninguna persona las ha revisado.

## Licencia

AGPL-3.0. Vea [LICENSE](../../LICENSE).
