#!/usr/bin/env python3
"""Translate a built static website with a running aien-translate.

Usage:
  translate_site.py DIST OUT --langs zh-CN[,ja,...] [--server URL] [--base-url URL]

For every HTML page in DIST, writes OUT/<code>/<same path> in each language.

How text is cut up: each paragraph-like element (a block holding only text and
inline tags such as links, bold or code) is translated as one piece, with its
inline tags turned into numbered markers (<g1>...</g1>, <x1/> for code) so the
model can move them with the words they belong to. If the markers do not come
back intact, that piece stays in English. Elements marked translate="no",
scripts, styles, <pre> and <code> are never translated.

Internal links are pointed at the same language (/zh-CN/...), <html lang> is
set, and hreflang alternates are added. Needs: beautifulsoup4.
"""
import argparse, copy, json, re, sys, urllib.parse, urllib.request
from pathlib import Path
from bs4 import BeautifulSoup, NavigableString, Comment, Tag

LANGS = {  # code -> (translate.js id, name, html lang)
    "zh-CN": ("chinese_simplified", "简体中文", "zh-CN"),
    "zh-TW": ("chinese_traditional", "繁體中文", "zh-TW"),
    "ja": ("japanese", "日本語", "ja"),
    "ko": ("korean", "한국어", "ko"),
    "es": ("spanish", "Español", "es"),
    "fr": ("french", "Français", "fr"),
    "de": ("deutsch", "Deutsch", "de"),
    "pt": ("portuguese", "Português", "pt"),
    "ru": ("russian", "Русский", "ru"),
}
SKIP = {"script", "style", "pre", "code", "svg", "noscript", "textarea", "math", "template", "kbd", "samp"}
ATOMIC = {"code", "kbd", "samp", "var", "br", "img", "svg", "math", "wbr", "input"}
INLINE = {"a", "strong", "em", "b", "i", "span", "small", "sup", "sub", "abbr", "mark", "u", "s",
          "q", "cite", "time", "dfn", "bdi", "bdo", "data", "label"} | ATOMIC
ATTRS = ("title", "alt", "placeholder", "aria-label")
META = {"description", "og:title", "og:description", "twitter:title", "twitter:description"}
ASSET = re.compile(r"\.(css|js|mjs|json|txt|xml|svg|png|jpe?g|gif|webp|ico|sh|pdf|woff2?|map)$", re.I)
LETTERS = re.compile(r"[A-Za-z]{2}")


def no_translate(el):
    while isinstance(el, Tag):
        if el.get("translate") == "no":
            return True
        el = el.parent
    return False


def inline_only(el):
    for c in el.children:
        if isinstance(c, Comment):
            continue
        if isinstance(c, NavigableString):
            continue
        if not isinstance(c, Tag) or c.name not in INLINE:
            return False
        if c.name not in ATOMIC and not inline_only(c):
            return False
        if c.get("translate") == "no" and c.name not in ATOMIC:
            return False
    return True


def text_of(el):
    return "".join(s for s in el.find_all(string=True) if not isinstance(s, Comment)
                   and not any(p.name in SKIP for p in s.parents if isinstance(p, Tag)))


class Unit:
    """One piece of text to translate, with what is needed to put it back."""

    def __init__(self, kind, el, source, tags=None, attr=None):
        self.kind, self.el, self.source, self.tags, self.attr = kind, el, source, tags or [], attr


def encode(el, tags):
    out = []
    for c in el.children:
        if isinstance(c, Comment):
            continue
        if isinstance(c, NavigableString):
            out.append(re.sub(r"\s+", " ", str(c)).replace("<", "&lt;"))
        elif c.name in ATOMIC:
            tags.append(c)
            out.append(f"<x{len(tags)}/>")
        else:
            tags.append(c)
            n = len(tags)
            out.append(f"<g{n}>{encode(c, tags)}</g{n}>")
    return "".join(out)


TOKEN = re.compile(r"<g(\d+)>|</g(\d+)>|<x(\d+)/>|[^<]+|<")


def decode(soup, translated, tags):
    """Rebuild child nodes from a translated string. None if markers are broken."""
    root = []
    stack = [(None, root)]
    used = set()
    for m in TOKEN.finditer(translated):
        g_open, g_close, x = m.group(1), m.group(2), m.group(3)
        if g_open:
            n = int(g_open)
            if n in used or n > len(tags) or tags[n - 1].name in ATOMIC:
                return None
            used.add(n)
            t = tags[n - 1]
            shell = soup.new_tag(t.name, attrs=dict(t.attrs))
            stack[-1][1].append(shell)
            stack.append((n, shell))
        elif g_close:
            n = int(g_close)
            if len(stack) < 2 or stack[-1][0] != n:
                return None
            stack.pop()
        elif x:
            n = int(x)
            if n in used or n > len(tags) or tags[n - 1].name not in ATOMIC:
                return None
            used.add(n)
            stack[-1][1].append(copy.copy(tags[n - 1]))
        else:
            stack[-1][1].append(NavigableString(m.group(0).replace("&lt;", "<")))
    if len(stack) != 1 or used != set(range(1, len(tags) + 1)):
        return None
    return root


def collect(soup):
    units = []

    def visit(el):
        if not isinstance(el, Tag) or el.name in SKIP or el.get("translate") == "no":
            return
        if el.name not in ("html", "head", "body") and inline_only(el) and LETTERS.search(text_of(el)):
            tags = []
            src = encode(el, tags).strip()
            units.append(Unit("block", el, src, tags))
            return
        for c in list(el.children):
            if isinstance(c, Comment):
                continue
            if isinstance(c, NavigableString):
                if LETTERS.search(str(c)) and el.name not in ("html", "head"):
                    units.append(Unit("text", c, re.sub(r"\s+", " ", str(c)).strip()))
            else:
                visit(c)

    visit(soup.html or soup)
    for el in soup.find_all(True):
        if no_translate(el) or el.name in SKIP:
            continue
        for a in ATTRS:
            v = el.get(a)
            if isinstance(v, str) and LETTERS.search(v):
                units.append(Unit("attr", el, v.strip(), attr=a))
        if el.name == "meta" and (el.get("name") in META or el.get("property") in META):
            v = el.get("content")
            if v and LETTERS.search(v):
                units.append(Unit("attr", el, v.strip(), attr="content"))
    return units


def translate_all(server, lang_id, texts, chunk_items=200, chunk_chars=20000):
    out = {}
    pending = [t for t in texts if t]
    i = 0
    while i < len(pending):
        batch, chars = [], 0
        while i < len(pending) and len(batch) < chunk_items and chars + len(pending[i]) <= chunk_chars or (i < len(pending) and not batch):
            batch.append(pending[i]); chars += len(pending[i]); i += 1
        body = urllib.parse.urlencode({"from": "english", "to": lang_id, "text": json.dumps(batch)}).encode()
        req = urllib.request.Request(server.rstrip("/") + "/translate.json", body,
                                     {"content-type": "application/x-www-form-urlencoded"})
        with urllib.request.urlopen(req, timeout=7200) as r:
            d = json.load(r)
        if d.get("result") != 1:
            raise SystemExit(f"server refused: {d.get('info')}")
        out.update(zip(batch, d["text"]))
        print(f"    {lang_id}: {i}/{len(pending)} pieces ({d.get('aien')})", flush=True)
    return out


# Shown at the top of every translated page, then translated with the page. Drake 2026-10-18: make it clear
# the translation ran on our own local model, to keep AI off data-center power.
NOTE_CLASS = "aien-translation-note"
NOTE_HTML = (
    '<p class="aien-translation-note" style="margin:0;padding:10px 16px;font-size:14px;line-height:1.5;'
    'color:var(--an-muted,#9aa4b2);border-bottom:1px solid var(--an-border,rgba(127,127,127,.25));'
    'text-align:center">'
    "<strong>Translated on our own machine.</strong> "
    "AIEN's own AI model translated this page on a single computer in our lab, not in a data center. "
    "AI data centers are putting a heavy load on power grids and adding to the emissions that warm our planet, "
    "and we would rather not add to it. "
    'The translation may have mistakes; the <a hreflang="en" href="/">English page</a> is the original.'
    "</p>"
)


def add_note(soup):
    """Put the translation note right under the site header (else first in <main> or <body>), once."""
    if soup.find(class_=NOTE_CLASS):
        return
    note = BeautifulSoup(NOTE_HTML, "html.parser").p
    header = soup.find("header", class_="aien-header")
    if header is not None:
        header.insert_after(note)
        return
    host = soup.find("main") or soup.body
    if host is not None:
        host.insert(0, note)


def no_dashes(text, code):
    """Drake's sites use no em or en dashes; keep translations to the same rule."""
    if code in ("zh-CN", "zh-TW", "ja"):
        text = re.sub(r"\s*[\u2014\u2013]+\s*", "，" if code != "ja" else "、", text)
    else:
        text = re.sub(r"\s+[\u2014\u2013]+\s+", ", ", text)
        text = text.replace("\u2014", ", ").replace("\u2013", "-")
    return text


def route_of(rel):
    """URL path for a page file: index.html -> '/', a/index.html -> '/a/'."""
    p = "/" + rel.as_posix()
    return p[: -len("index.html")] if p.endswith("index.html") else p


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dist"); ap.add_argument("out")
    ap.add_argument("--langs", required=True)
    ap.add_argument("--server", default="http://127.0.0.1:18097")
    ap.add_argument("--base-url", default="https://www.aienos.com")
    a = ap.parse_args()
    dist, out = Path(a.dist), Path(a.out)
    codes = a.langs.split(",")
    for c in codes:
        if c not in LANGS:
            sys.exit(f"unknown language {c}")

    pages = sorted(p for p in dist.rglob("*.html") if not any(part in LANGS for part in p.relative_to(dist).parts[:1]))
    routes = {route_of(p.relative_to(dist)) for p in pages}
    parsed = {p: BeautifulSoup(p.read_text(encoding="utf-8"), "html.parser") for p in pages}
    all_sources = []
    for soup in parsed.values():
        add_note(soup)
        all_sources += [u.source for u in collect(soup)]
    unique = list(dict.fromkeys(s for s in all_sources if s))
    print(f"{len(pages)} pages, {len(unique)} unique pieces, {sum(map(len, unique))} characters")

    for code in codes:
        lang_id, name, html_lang = LANGS[code]
        print(f"== {code}")
        table = translate_all(a.server, lang_id, unique)
        table = {k: no_dashes(v, code) for k, v in table.items()}
        kept = 0
        for page in pages:
            rel = page.relative_to(dist)
            soup = BeautifulSoup(page.read_text(encoding="utf-8"), "html.parser")
            add_note(soup)
            for u in collect(soup):
                tr = table.get(u.source)
                if not tr or tr == u.source:
                    continue
                if u.kind == "attr":
                    u.el[u.attr] = tr
                elif u.kind == "text":
                    s = str(u.el)
                    lead = s[: len(s) - len(s.lstrip())]
                    trail = s[len(s.rstrip()):]
                    u.el.replace_with(NavigableString(lead + tr + trail))
                else:
                    nodes = decode(soup, tr, u.tags)
                    if nodes is None:
                        kept += 1
                        continue
                    u.el.clear()
                    for n in nodes:
                        u.el.append(n)
            route = route_of(rel)
            # Language, links, alternates.
            if soup.html:
                soup.html["lang"] = html_lang
            for el in soup.find_all(["a", "link", "form"]):
                attr = "action" if el.name == "form" else "href"
                href = el.get(attr)
                if not href or el.get("hreflang") or (el.name == "link" and el.get("rel") not in (["canonical"], ["alternate"])):
                    continue
                if href.startswith("/") and not href.startswith("//") and not ASSET.search(href.split("#")[0].split("?")[0]):
                    path = href.split("#")[0].split("?")[0]
                    if path in routes or path.rstrip("/") + "/" in routes or path == "/":
                        el[attr] = f"/{code}{href}"
            for el in soup.find_all("a", hreflang=True):
                target = el["hreflang"]
                el["href"] = route if target == "en" else f"/{target}{route}"
            if soup.head:
                for old in soup.head.find_all("link", rel="canonical"):
                    old.decompose()
                canon = soup.new_tag("link", rel="canonical", href=f"{a.base_url}/{code}{route}")
                soup.head.append(canon)
                for c2, (_, _, hl) in [("en", (None, None, "en"))] + [(c, LANGS[c]) for c in codes]:
                    href = f"{a.base_url}{route}" if c2 == "en" else f"{a.base_url}/{c2}{route}"
                    soup.head.append(soup.new_tag("link", rel="alternate", hreflang=hl, href=href))
            dest = out / code / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(str(soup), encoding="utf-8")
        print(f"   wrote {len(pages)} pages to {out / code}; {kept} piece(s) kept in English because markers broke")


if __name__ == "__main__":
    main()
