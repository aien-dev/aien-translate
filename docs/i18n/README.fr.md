# aien-translate

<!-- languages -->
[English](../../README.md) · [简体中文](README.zh-CN.md) · [繁體中文](README.zh-TW.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Español](README.es.md) · **Français** · [Deutsch](README.de.md) · [Português](README.pt.md) · [Русский](README.ru.md)
<!-- /languages -->

> Cette page a été traduite automatiquement par aien-translate, en cours d'exécution sur le modèle propre d'AIEN. La [version anglaise](../../README.md) est l'originale.

Traduisez votre site web en 31 langues avec votre propre modèle d'IA. Aucune clé API, aucun service cloud, et les textes de vos visiteurs ne quittent jamais votre serveur.

aien-translate est un serveur de traduction pour [translate.js](https://github.com/xnx3/translate). Il traduit grâce au runtime d'inférence propre à AIEN. Il fonctionne également avec tout serveur offrant une API de chat de style OpenAI.

## Traduisez votre site en 3 étapes

**1. Démarrez le serveur.** Vous avez besoin de Rust et d'un serveur de modèle en cours d'exécution (le `aien-serve-http` d'AIEN, ou tout serveur de chat de style OpenAI).

```
git clone https://github.com/aien-dev/aien-translate
cd aien-translate
cargo build --release
./target/release/aien-translate --listen 0.0.0.0:18097 --engine http://127.0.0.1:18091
```

**2. Ajoutez translate.js à votre page.** Téléchargez `translate.js` depuis [son dépôt](https://github.com/xnx3/translate) et hébergez-le avec votre site. Ajoutez ensuite ces lignes juste avant `</body>`, en indiquant l'adresse de votre serveur :

```html
<script src="/translate.js"></script>
<script>
  translate.service.use('translate.service');
  translate.request.setHost('https://your-server.example/');
  translate.execute();
</script>
```

**3. Ouvrez votre page.** Un menu de langue apparaît, et en choisissant une langue, la page est traduite sur place. Pour sélectionner une langue depuis votre propre code, appelez `translate.changeLanguage('japanese')`.

### Vérifiez que cela fonctionne

```
curl http://127.0.0.1:18097/health
curl -X POST http://127.0.0.1:18097/translate.json \
  -d 'from=english&to=japanese&text=%5B%22Hello%20world%22%5D'
```

La première commande indique le modèle utilisé. La deuxième retourne la traduction dans `"text"`, par exemple `["こんにちは世界"]`.

## Ce qu'il protège

Chaque traduction est vérifiée avant que le visiteur ne la voie :

- Les liens, les adresses e-mail, le code entre backticks, et les espaces réservés tels que `{name}` ou `%s` doivent passer sans modification.
- Les noms de produits figurant sur la liste de conservation (AIEN, Omega, Allen, etc.) ne sont jamais traduits. Utilisez `--keep-file` pour fournir votre propre liste.
- Une traduction vide, ou beaucoup plus longue que l'original, est rejetée.

Si une vérification échoue, le visiteur voit le texte original au lieu d'une traduction corrompue.

## Comment cela fonctionne

- translate.js envoie le texte de la page à `/translate.json`. Le serveur l'envoie au modèle par lots, une requête à la fois, car la GPU peut être partagée.
- Chaque traduction est sauvegardée (SQLite), donc le même texte n'est jamais traduit deux fois. Les visites répétées sont instantanées.
- translate.js découpe les adresses web aux endroits `//`. Le serveur conserve ces morceaux en place afin que les liens soient correctement reconstruits.
- Si la réponse du modèle pour un lot ne peut pas être associée aux entrées, le lot est divisé et réessayé, jusqu'à des éléments individuels.

## Options

```
--listen     address to serve on        (default 127.0.0.1:18097)
--engine     model server               (default http://127.0.0.1:18091)
--cache      saved translations         (default ~/.local/state/aien-translate/cache.sqlite)
--keep-file  names never translated, one per line
```

Points d'accès : `/translate.json`, `/language.json`, `/connectTest.json`, `/init.json` et `/health`. `/ip.json` (recherche d'emplacement) n'est pas proposé.

## Langues

Anglais, chinois simplifié et traditionnel, japonais, coréen, espagnol, français, allemand, portugais, italien, russe, ukrainien, polonais, néerlandais, suédois, danois, norvégien, finnois, tchèque, roumain, hongrois, grec, turc, arabe, persan, hindi, bengali, thaï, vietnamien, indonésien et malais. La liste est dans `src/langs.rs`.

## Tests

- `cargo test` exécute les tests unitaires, ainsi que des tests bout-en-bout contre un modèle factice (aucune GPU nécessaire).
- `demo/` contient une page qui utilise translate.js réel. Servez le dossier et exécutez `python demo/check.py URL` (nécessite Playwright) pour voir la page traduite.

## Traductions de cette page

Les versions de cette page dans d'autres langues ont été réalisées par aien-translate lui-même, avec `scripts/translate_readme.py`. Ce sont des traductions automatiques, et la version anglaise est l'originale.

## Limites

- La vitesse dépend de la carte graphique. Sur l'exécution d'AIEN, un lot prend environ une seconde lorsque la GPU est libre, et bien plus longtemps lorsque une autre tâche l'utilise.
- Il s'agit de traductions automatiques. Elles sont bonnes pour la lecture, mais aucune personne n'a vérifié leur contenu.

## Licence

AGPL-3.0. Voir [LICENSE](../../LICENSE).
