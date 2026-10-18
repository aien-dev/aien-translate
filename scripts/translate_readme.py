#!/usr/bin/env python3
"""Translate README.md into other languages with a running aien-translate.

Usage: python3 scripts/translate_readme.py [--server URL] [lang ...]

Code blocks are copied unchanged. Each other line is translated, and a line
whose markdown (links, bold, code spans) does not survive is kept in English.
Output goes to docs/i18n/README.<code>.md.
"""
import json
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "i18n"

# file code, translate.js id, name shown in the language bar
LANGS = [
    ("zh-CN", "chinese_simplified", "简体中文"),
    ("zh-TW", "chinese_traditional", "繁體中文"),
    ("ja", "japanese", "日本語"),
    ("ko", "korean", "한국어"),
    ("es", "spanish", "Español"),
    ("fr", "french", "Français"),
    ("de", "deutsch", "Deutsch"),
    ("pt", "portuguese", "Português"),
    ("ru", "russian", "Русский"),
]

NOTE = ("This page was translated automatically by aien-translate, running on AIEN's own model. "
        "The [English version](../../README.md) is the original.")

PREFIX = re.compile(r"^(\s*(?:#{1,6} |[-*] |\d+\. |> )?)(.*)$")
LINK_TARGET = re.compile(r"\]\(([^)]+)\)")


def markers(s):
    """Markdown that must survive translation unchanged."""
    return (sorted(LINK_TARGET.findall(s)), s.count("**"), s.count("`"), s.count("["))


def lang_bar(current):
    parts = []
    english = "**English**" if current is None else "[English](../../README.md)"
    parts.append(english)
    for code, _, name in LANGS:
        if code == current:
            parts.append(f"**{name}**")
        elif current is None:
            parts.append(f"[{name}](docs/i18n/README.{code}.md)")
        else:
            parts.append(f"[{name}](README.{code}.md)")
    return " · ".join(parts)


def fix_links(line):
    """Relative links in the README point from the repo root; the copies live two folders down."""
    def repl(m):
        target = m.group(1)
        if re.match(r"^(https?:|mailto:|#|\.\./|README\.)", target):
            return m.group(0)
        return f"]({'../../' + target})"
    return LINK_TARGET.sub(repl, line)


def translate(server, to, texts):
    body = urllib.parse.urlencode({"from": "english", "to": to, "text": json.dumps(texts)}).encode()
    req = urllib.request.Request(server.rstrip("/") + "/translate.json", body,
                                 {"content-type": "application/x-www-form-urlencoded"})
    with urllib.request.urlopen(req, timeout=1800) as r:
        d = json.load(r)
    if d.get("result") != 1:
        raise SystemExit(f"{to}: server refused: {d.get('info')}")
    return d["text"], d.get("aien", {})


def main():
    args = sys.argv[1:]
    server = "http://127.0.0.1:18097"
    if args[:1] == ["--server"]:
        server, args = args[1], args[2:]
    wanted = [l for l in LANGS if not args or l[0] in args]

    lines = (ROOT / "README.md").read_text(encoding="utf-8").split("\n")
    OUT.mkdir(parents=True, exist_ok=True)

    # Work out which lines get translated.
    jobs = []  # (line index, prefix, text)
    in_code = in_bar = False
    for i, line in enumerate(lines):
        if line.startswith("```"):
            in_code = not in_code
            continue
        if line.strip() == "<!-- languages -->":
            in_bar = True
            continue
        if line.strip() == "<!-- /languages -->":
            in_bar = False
            continue
        if in_code or in_bar or not line.strip():
            continue
        m = PREFIX.match(line)
        assert m
        prefix, text = m.groups()
        if text.strip():
            jobs.append((i, prefix, text))

    for code, lang_id, name in wanted:
        texts = [t for _, _, t in jobs] + [NOTE]
        out, stats = translate(server, lang_id, texts)
        note = out[-1]
        if markers(note) != markers(NOTE):
            note = NOTE
        result = list(lines)
        english = []
        for (i, prefix, src), tr in zip(jobs, out):
            if markers(tr) != markers(src):
                tr = src
            if tr == src and re.search(r"[A-Za-z]{3}", src):
                english.append(src[:60])
            result[i] = prefix + tr

        # Language bar for this copy, then the note under the title.
        final = []
        in_code = in_bar = False
        for line in result:
            if line.startswith("```"):
                in_code = not in_code
            if not in_code and line.strip() == "<!-- languages -->":
                in_bar = True
                final.append(line)
                final.append(lang_bar(code))
                continue
            if not in_code and line.strip() == "<!-- /languages -->":
                in_bar = False
                final.append(line)
                final.append("")
                final.append(f"> {note}")
                continue
            if in_bar:
                continue
            final.append(line if in_code else fix_links(line))

        path = OUT / f"README.{code}.md"
        path.write_text("\n".join(final), encoding="utf-8")
        print(f"{code}: wrote {path.relative_to(ROOT)}; server {stats}; "
              f"{len(english)} line(s) kept in English" + (f": {english}" if english else ""))


if __name__ == "__main__":
    main()
