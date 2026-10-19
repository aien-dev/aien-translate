#!/usr/bin/env python3
"""Translate a LaTeX paper with a running aien-translate, one language per output folder.

    translate_latex.py paper.tex OUT_DIR --langs zh-CN,ja [--server http://127.0.0.1:18097] [--doi 10.5281/...]

Writes OUT_DIR/<code>/paper.tex (and copies the files next to paper.tex), ready for
`tectonic -X compile`. The model sees plain sentences: math, citations, references, labels,
URLs and code become numbered markers (<x1/>), and formatting such as \\textit{...} or
\\section{...} becomes paired markers (<g1>...</g1>) whose contents are translated. The
guard in aien-translate requires every marker back; a piece whose markers break stays in
English. Diagram drawing code, display math and the reference list are kept as they are,
the way journals publish translations; labels inside diagrams are translated. A note under the title says the translation ran on
AIEN's own model and points to the English original.
"""
import argparse
import json
import re
import shutil
import sys
import urllib.parse
import urllib.request
from pathlib import Path

LANGS = {  # code -> (aien-translate id, babel name or None for CJK, CJK font, names)
    "zh-CN": ("chinese_simplified", None, "Noto Serif CJK SC",
              dict(definition="定义", lemma="引理", abstract="摘要", refs="参考文献", fig="图", tab="表")),
    "zh-TW": ("chinese_traditional", None, "Noto Serif CJK TC",
              dict(definition="定義", lemma="引理", abstract="摘要", refs="參考文獻", fig="圖", tab="表")),
    "ja": ("japanese", None, "Noto Serif CJK JP",
           dict(definition="定義", lemma="補題", abstract="概要", refs="参考文献", fig="図", tab="表")),
    "ko": ("korean", None, "Noto Serif CJK KR",
           dict(definition="정의", lemma="보조정리", abstract="초록", refs="참고문헌", fig="그림", tab="표")),
    "es": ("spanish", "spanish", None, dict(definition="Definición", lemma="Lema")),
    "fr": ("french", "french", None, dict(definition="Définition", lemma="Lemme")),
    "de": ("deutsch", "ngerman", None, dict(definition="Definition", lemma="Lemma")),
    "pt": ("portuguese", "portuguese", None, dict(definition="Definição", lemma="Lema")),
    "ru": ("russian", "russian", None, dict(definition="Определение", lemma="Лемма")),
}

SKIP_ENVS = {"tikzpicture", "thebibliography", "equation", "equation*", "align", "align*", "gather",
             "gather*", "multline", "multline*", "eqnarray", "eqnarray*", "displaymath", "math", "array",
             "verbatim", "lstlisting", "minted"}
THEOREM_ENVS = {"definition", "lemma", "theorem", "proof", "corollary", "proposition", "remark", "example"}
SECTION_LIKE = {"section", "section*", "subsection", "subsection*", "subsubsection", "subsubsection*",
                "paragraph", "paragraph*", "caption", "title", "date"}
PAIRED = SECTION_LIKE | {"textit", "textbf", "emph", "textsc", "textsl", "underline", "footnote", "uline"}
FONT_SWITCH = {"small", "footnotesize", "scriptsize", "tiny", "large", "Large", "normalsize", "bfseries",
               "itshape", "em", "bf", "it", "sffamily", "rmfamily"}
LETTERS = re.compile(r"[^\W\d_]{2}")
MARK = re.compile(r"(<g\d+>|</g\d+>|<x\d+/>)")
ACCENT = {'"': "\u0308", "'": "\u0301", "`": "\u0300", "^": "\u0302", "~": "\u0303", "c": "\u0327", "v": "\u030c"}
ACCENT_RE = re.compile(r"\{?\\([\"'`^~]|c |v )\s*\{?([A-Za-z])\}?\}?")


# ---------- parsing a LaTeX fragment into text, atoms (x) and groups (g) ----------

class Node:
    __slots__ = ("kind", "raw", "text", "open", "close", "kids")

    def __init__(self, kind, raw="", text="", open_="", close="", kids=None):
        self.kind, self.raw, self.text, self.open, self.close, self.kids = kind, raw, text, open_, close, kids or []


def match_brace(s, i, o="{", c="}"):
    """s[i] == o; return index just past the matching c (escapes respected)."""
    depth = 0
    j = i
    while j < len(s):
        ch = s[j]
        if ch == "\\":
            j += 2
            continue
        if ch == o:
            depth += 1
        elif ch == c:
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    raise ValueError(f"unbalanced {o}{c} near: {s[i:i+80]!r}")


def find_end_env(s, i, env):
    begin, end = "\\begin{" + env + "}", "\\end{" + env + "}"
    depth = 1
    j = i
    while depth:
        nb, ne = s.find(begin, j), s.find(end, j)
        if ne < 0:
            raise ValueError(f"no \\end{{{env}}}")
        if 0 <= nb < ne:
            depth += 1
            j = nb + len(begin)
        else:
            depth -= 1
            j = ne + len(end)
    return j


def adjacent_args(s, j):
    """Consume {...} and [...] groups directly after a command (no spaces)."""
    while j < len(s) and s[j] in "{[":
        j = match_brace(s, j, s[j], "}" if s[j] == "{" else "]")
    return j


def parse(s, i=0, stop=None):
    nodes, buf, start = [], [], i

    def flush():
        if buf:
            nodes.append(Node("t", raw="".join(r for r, _ in buf), text="".join(t for _, t in buf)))
            buf.clear()

    def atom(raw):
        flush()
        nodes.append(Node("x", raw=raw))

    while i < len(s):
        ch = s[i]
        if stop and ch == stop:
            break
        if ch == "%":
            j = s.find("\n", i)
            j = len(s) if j < 0 else j + 1
            atom(s[i:j]); i = j; continue
        if ch == "$":
            if s.startswith("$$", i):
                j = s.index("$$", i + 2) + 2
            else:
                j = i + 1
                while s[j] != "$" or s[j - 1] == "\\":
                    j += 1
                j += 1
            atom(s[i:j]); i = j; continue
        if ch == "&":
            atom("&"); i += 1; continue
        if ch == "~":
            atom("~"); i += 1; continue
        am = ACCENT_RE.match(s, i) if ch in "{\\" else None
        if am and (am.group(0).count("{") == am.group(0).count("}")):
            import unicodedata
            buf.append((am.group(0), unicodedata.normalize("NFC", am.group(2) + ACCENT[am.group(1).strip()])))
            i = am.end(); continue
        if ch == "{":
            j = match_brace(s, i)
            inner = s[i + 1:j - 1]
            m = re.match(r"\\([A-Za-z]+)\s*", inner)
            if m and m.group(1) in FONT_SWITCH:
                o = "{" + m.group(0)
                kids = parse(inner, m.end())[0]
                flush(); nodes.append(Node("g", raw=s[i:j], open_=o, close="}", kids=kids))
            elif inner.startswith("\\"):
                atom(s[i:j])
            else:
                flush(); nodes.append(Node("g", raw=s[i:j], open_="{", close="}", kids=parse(inner)[0]))
            i = j; continue
        if ch == "\\":
            if i + 1 >= len(s):
                atom("\\"); i += 1; continue
            nxt = s[i + 1]
            if nxt in "%&#_${}":
                buf.append((s[i:i + 2], nxt)); i += 2; continue
            if nxt == "(":
                j = s.index("\\)", i) + 2; atom(s[i:j]); i = j; continue
            if nxt == "[":
                j = s.index("\\]", i) + 2; atom(s[i:j]); i = j; continue
            if nxt == "\\":
                j = i + 2
                if j < len(s) and s[j] == "*":
                    j += 1
                if j < len(s) and s[j] == "[":
                    j = match_brace(s, j, "[", "]")
                atom(s[i:j]); i = j; continue
            if not nxt.isalpha():
                atom(s[i:i + 2]); i += 2; continue
            m = re.match(r"\\([A-Za-z]+\*?)", s[i:])
            name, j = m.group(1), i + m.end()
            if name in ("begin", "end"):
                k = match_brace(s, j)
                env = s[j + 1:k - 1]
                if name == "end":
                    atom(s[i:k]); i = k; continue
                if env in SKIP_ENVS:
                    e = find_end_env(s, k, env); atom(s[i:e]); i = e; continue
                if env in THEOREM_ENVS and k < len(s) and s[k] == "[":
                    e = match_brace(s, k, "[", "]")
                    atom(s[i:k]); flush()
                    nodes.append(Node("g", raw=s[k:e], open_="[", close="]", kids=parse(s[k + 1:e - 1])[0]))
                    i = e; continue
                e = adjacent_args(s, k); atom(s[i:e]); i = e; continue
            if name == "item":
                if j < len(s) and s[j] == "[":
                    e = match_brace(s, j, "[", "]")
                    atom(s[i:j]); flush()
                    nodes.append(Node("g", raw=s[j:e], open_="[", close="]", kids=parse(s[j + 1:e - 1])[0]))
                    i = e; continue
                atom(s[i:j]); i = j; continue
            if name == "href" and j < len(s) and s[j] == "{":
                k = match_brace(s, j)
                if k < len(s) and s[k] == "{":
                    e = match_brace(s, k)
                    flush()
                    nodes.append(Node("g", raw=s[i:e], open_=s[i:k + 1], close="}", kids=parse(s[k + 1:e - 1])[0]))
                    i = e; continue
            if name in PAIRED:
                k = j
                if k < len(s) and s[k] == "[":
                    k = match_brace(s, k, "[", "]")
                if k < len(s) and s[k] == "{":
                    e = match_brace(s, k)
                    flush()
                    g = Node("g", raw=s[i:e], open_=s[i:k + 1], close="}", kids=parse(s[k + 1:e - 1])[0])
                    g.text = name  # remember the command, to split headings out on their own
                    nodes.append(g); i = e; continue
            e = adjacent_args(s, j)
            atom(s[i:e]); i = e; continue
        buf.append((ch, ch)); i += 1
    flush()
    return nodes, i


def raw_of(nodes):
    return "".join(n.raw for n in nodes)


def has_words(nodes):
    for n in nodes:
        if n.kind == "t" and LETTERS.search(n.text):
            return True
        if n.kind == "g" and has_words(n.kids):
            return True
    return False


def breaking(n):
    return n.kind == "x" and re.search(r"\\\\|^&$|\\item|\\begin|\\end|\\centering|\\maketitle|\\label|^%", n.raw)


LONG = 700        # characters; longer paragraphs go to the model a few sentences at a time
CHUNK = 450
SENTENCE_END = re.compile(r"(?:(?<=[.!?])(?=\s+[A-Z(`\\])|(?<=;)(?=\s))(\s+)")


def unescape(raw):
    return re.sub(r"\\([%&#_${}])", r"\1", raw)


def sentences(nodes):
    """Split a long run at sentence ends that fall in its top-level text."""
    chunks, cur, size = [], [], 0
    for n in nodes:
        if n.kind != "t":
            cur.append(n); size += len(n.raw) if n.kind == "x" else len(raw_of(n.kids)); continue
        parts = SENTENCE_END.split(n.raw)
        for k, part in enumerate(parts):
            if k % 2:  # whitespace between sentences
                cur.append(Node("t", raw=part, text=" "))
                if size >= CHUNK:
                    chunks.append(cur); cur, size = [], 0
            elif part:
                cur.append(Node("t", raw=part, text=unescape(part))); size += len(part)
    if cur:
        chunks.append(cur)
    return chunks


def tikz(raw):
    """Translate the text of each `node {...}` in a diagram; keep the drawing code as it is."""
    out, pos = [], 0
    for m in re.finditer(r"\bnode\b", raw):
        if m.start() < pos:
            continue
        j = m.end()
        while j < len(raw):
            if raw[j].isspace():
                j += 1
            elif raw[j] == "[":
                j = match_brace(raw, j, "[", "]")
            elif raw[j] == "(":
                j = match_brace(raw, j, "(", ")")
            elif raw.startswith("at", j):
                j += 2
            else:
                break
        if j < len(raw) and raw[j] == "{":
            e = match_brace(raw, j)
            kids = parse(raw[j + 1:e - 1])[0]
            out.append(("raw", raw[pos:j + 1]))
            out.append(("piece", kids) if has_words(kids) else ("raw", raw[j + 1:e - 1]))
            out.append(("raw", "}"))
            pos = e
    out.append(("raw", raw[pos:]))
    return out


def runs(nodes):
    """Split a fragment into pieces to translate (lists of nodes) and raw LaTeX to keep."""
    out, cur = [], []

    def flush():
        if cur:
            if not has_words(cur):
                out.append(("raw", raw_of(cur)))
            elif len(raw_of(cur)) > LONG:
                for ch in sentences(cur):
                    out.append(("piece", ch) if has_words(ch) else ("raw", raw_of(ch)))
            else:
                out.append(("piece", list(cur)))
            cur.clear()

    for n in nodes:
        if n.kind == "x" and n.raw.startswith("\\begin{tikzpicture}"):
            flush(); out.extend(tikz(n.raw))
        elif breaking(n):
            flush(); out.append(("raw", n.raw))
        elif n.kind == "g" and n.text in SECTION_LIKE:
            flush(); out.append(("raw", n.open)); out.extend(runs(n.kids)); out.append(("raw", n.close))
        else:
            cur.append(n)
    flush()
    return out


# ---------- encoding pieces for the model and decoding the answer ----------

DASHES = (("---", "\u2014"), ("--", "\u2013"))


def plain(t):
    t = re.sub(r"\s+", " ", t)
    t = t.replace("``", "\u201c").replace("''", "\u201d")
    for a, b in DASHES:
        t = t.replace(a, b)
    return t


def encode(nodes):
    table, counter = {}, [0]

    def enc(ns):
        parts = []
        for n in ns:
            if n.kind == "t":
                parts.append(plain(n.text))
            else:
                counter[0] += 1
                k = counter[0]
                if n.kind == "x":
                    table[f"x{k}"] = n.raw
                    parts.append(f"<x{k}/>")
                else:
                    table[f"g{k}"] = (n.open, n.close)
                    parts.append(f"<g{k}>{enc(n.kids)}</g{k}>")
        return "".join(parts)

    return enc(nodes), table


ESC = {"\\": r"\textbackslash{}", "{": r"\{", "}": r"\}", "%": r"\%", "&": r"\&", "#": r"\#",
       "_": r"\_", "$": r"\$", "^": r"\^{}", "~": r"\textasciitilde{}"}


def escape(t):
    t = t.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
    return "".join(ESC.get(c, c) for c in t)


def no_dashes(t, code):
    """Drake's writing uses no em or en dashes; keep translations to the same rule."""
    t = re.sub(r"(\d)\s*\u2013\s*(\d)", r"\1-\2", t)
    if code in ("zh-CN", "zh-TW", "ja"):
        return re.sub(r"\s*[\u2014\u2013]+\s*", "、" if code == "ja" else "，", t)
    t = re.sub(r"\s+[\u2014\u2013]+\s+", ", ", t)
    return t.replace("\u2014", ", ").replace("\u2013", "-")


def decode(translated, table, code):
    out, stack, used = [], [], set()
    for part in MARK.split(translated):
        if not part:
            continue
        m = re.fullmatch(r"<(/?)([gx])(\d+)(/?)>", part)
        if m:
            key = f"{m.group(2)}{m.group(3)}"
            if key not in table:
                return None
            if m.group(2) == "x":
                if key in used:
                    return None
                used.add(key); out.append(table[key])
            elif m.group(1):
                if not stack or stack[-1] != key:
                    return None
                stack.pop(); out.append(table[key][1])
            else:
                if key in used:
                    return None
                used.add(key); stack.append(key); out.append(table[key][0])
            continue
        text = escape(no_dashes(part, code))
        if out and re.search(r"\\[A-Za-z]+\*?$", out[-1]) and text[:1].isalpha():
            text = "{}" + text  # \ldots followed by a word must not glue into \ldotsword
        out.append(text)
    if stack or used != set(table):
        return None
    return "".join(out)


# ---------- the document ----------

def units(body):
    """Blank-line separated blocks, never splitting inside a kept environment or a brace group."""
    blocks, cur = [], []
    for line in body.split("\n"):
        cur.append(line)
        text = "\n".join(cur)
        if line.strip() == "" and balanced(text):
            blocks.append(text + "\n"); cur = []
    if cur:
        blocks.append("\n".join(cur))
    return blocks


def balanced(text):
    try:
        parse(text)
        return True
    except (ValueError, IndexError):
        return False


def note(doi):
    src = f"the English original (DOI {doi})" if doi else "the English original"
    return ("\\begin{quote}\\small \\textbf{Translated on our own machine.} AIEN's own AI model translated this "
            "paper on a single computer in our lab, not in a data center. The translation may have mistakes; "
            f"{src} is the version of record.\\end{{quote}}\n")


def preamble_for(pre, code):
    lang_id, babel, cjk, names = LANGS[code]
    pre = re.sub(r"^\\usepackage\[utf8\]\{inputenc\}\n|^\\usepackage\[T1\]\{fontenc\}\n|^\\usepackage\{lmodern\}\n",
                 "", pre, flags=re.M)
    setup = ["\\usepackage{fontspec}"]
    if babel:
        setup.append(f"\\usepackage[shorthands=off,{babel}]{{babel}}")
        if code == "ru":
            setup.append("\\setmainfont{cmunrm.otf}[BoldFont=cmunbx.otf,ItalicFont=cmunti.otf,"
                         "BoldItalicFont=cmunbi.otf]")
    else:
        bold = cjk.replace("Serif", "Serif") + ":style=Bold"
        setup.append("\\usepackage{xeCJK}")
        setup.append(f"\\setCJKmainfont{{{cjk}}}[BoldFont={{{cjk} Bold}}]")
        setup.append(f"\\setCJKsansfont{{{cjk.replace('Serif', 'Sans')}}}")
        setup.append(f"\\setCJKmonofont{{{cjk.replace('Serif CJK', 'Sans Mono CJK')}}}")
        if code == "ko":
            setup.append("\\xeCJKsetup{CJKspace=true}")
        setup.append(f"\\AtBeginDocument{{\\renewcommand{{\\abstractname}}{{{names['abstract']}}}"
                     f"\\renewcommand{{\\refname}}{{{names['refs']}}}"
                     f"\\renewcommand{{\\figurename}}{{{names['fig']}}}"
                     f"\\renewcommand{{\\tablename}}{{{names['tab']}}}}}")
    pre = re.sub(r"(\\documentclass(\[[^\]]*\])?\{[^}]*\}\n)", lambda m: m.group(1) + "\n".join(setup) + "\n",
                 pre, count=1)
    for env, word in (("definition", names["definition"]), ("lemma", names["lemma"])):
        pre = re.sub(r"(\\newtheorem\{" + env + r"\}\{)[^}]*\}", lambda m: m.group(1) + word + "}", pre)
    return pre


def translate_all(server, lang_id, texts, chunk_items=60, chunk_chars=12000):
    out, pending, i = {}, list(dict.fromkeys(t for t in texts if t)), 0
    while i < len(pending):
        batch, chars = [], 0
        while i < len(pending) and (not batch or (len(batch) < chunk_items and chars + len(pending[i]) <= chunk_chars)):
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


def build(tex, code, server, doi, dry=False):
    lang_id = LANGS[code][0]
    head, rest = tex.split("\\begin{document}", 1)
    body, tail = rest.rsplit("\\end{document}", 1)
    body = re.sub(r"(\\maketitle[^\n]*\n)", lambda m: m.group(1) + "\n" + note(doi) + "\n", body, count=1)

    # title and date live in the preamble; translate them as their own blocks
    plan = []  # list of (where, items)
    for m in re.finditer(r"\\(title|date)\{", head):
        e = match_brace(head, m.end() - 1)
        plan.append(("head", (m.start(), e), runs(parse(head[m.start():e])[0])))
    for blk in units(body):
        plan.append(("body", blk, runs(parse(blk)[0])))

    pieces = [(encode(nodes)) for _, _, items in plan for kind, nodes in items if kind == "piece"]
    texts = [p[0] for p in pieces]
    print(f"{code}: {len(texts)} pieces, {sum(map(len, texts))} characters", flush=True)
    if dry:
        return texts
    done = translate_all(server, lang_id, texts)

    stats = {"translated": 0, "kept_english": 0}
    kept = []

    def render(items):
        out = []
        for kind, val in items:
            if kind == "raw":
                out.append(val); continue
            src, table = encode(val)
            lead = re.match(r"\s*", raw_of(val)).group(0)
            trail = re.search(r"\s*$", raw_of(val)).group(0)
            res = done.get(src)
            dec = decode(res, table, code) if res and res != src else None
            if dec is None:
                stats["kept_english"] += 1; kept.append(src); out.append(raw_of(val))
            else:
                stats["translated"] += 1; out.append(lead + dec.strip() + trail)
        return "".join(out)

    new_head, pos = [], 0
    for where, span, items in plan:
        if where == "head":
            new_head.append(head[pos:span[0]]); new_head.append(render(items)); pos = span[1]
    new_head.append(head[pos:])
    head2 = preamble_for("".join(new_head), code)
    body2 = "".join(render(items) for where, _, items in plan if where == "body")
    print(f"{code}: {stats}", flush=True)
    return head2 + "\\begin{document}" + body2 + "\\end{document}" + tail, stats, kept


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("tex", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--langs", default=",".join(LANGS))
    ap.add_argument("--server", default="http://127.0.0.1:18097")
    ap.add_argument("--doi", default="")
    ap.add_argument("--dry-run", action="store_true", help="print the pieces the model would see, translate nothing")
    a = ap.parse_args()
    tex = a.tex.read_text()
    if a.dry_run:
        for t in build(tex, "zh-CN", a.server, a.doi, dry=True):
            print(json.dumps(t, ensure_ascii=False))
        return
    for code in a.langs.split(","):
        if code not in LANGS:
            sys.exit(f"unknown language {code}; known: {', '.join(LANGS)}")
        d = a.out / code
        d.mkdir(parents=True, exist_ok=True)
        for f in a.tex.parent.iterdir():
            if f.is_file() and f.suffix not in (".tex", ".pdf", ".html") and f.name != "SHA256SUMS":
                shutil.copy2(f, d / f.name)
        result, stats, kept = build(tex, code, a.server, a.doi)
        (d / a.tex.name).write_text(result)
        (d / "kept-english.json").write_text(json.dumps(kept, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
