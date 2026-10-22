#!/usr/bin/env python3
"""Restos de español en el código (specs/limpieza-1.md, etapa L3a): recuento por crate y listados.

    spanish_left.py                      tabla por crate: identificadores, comentarios y mensajes de aserción
    spanish_left.py --list ids|comments|messages [--crate px-ui] [--limit N]

- identificadores: los ficheros .rs de `crates/` y `fuzz/` (producción y pruebas); se parte cada nombre en palabras
  (snake_case y CamelCase) y cuenta si alguna es una palabra española de los diccionarios (glossary*.tsv) que no
  figura en ENGLISH_OK. Cuenta nombres distintos por crate y, entre paréntesis, apariciones.
- comentarios: líneas de comentario (`//`, `///`, `//!`, `/* */` en .rs y `#` en Cargo.toml/deny.toml) que parecen
  español: llevan una letra propia del español (á é í ó ú ñ ü ¿ ¡) o palabras vacías españolas (ver SP_STRONG, SP_WEAK).
- mensajes: cadenas que son el mensaje de `assert*!`, `debug_assert*!`, `panic!`, `unreachable!`, `todo!`, `expect(..)`
  y `unwrap_or_else(|| panic!..)` y parecen español, en código de pruebas o de producción. Los textos que ve el
  usuario (`*_es.rs`, etiquetas de la interfaz) no cuentan: no son mensajes de aserción.
La heurística es la misma en todas las ejecuciones: sirve para ver el avance, no para demostrar nada.
"""
from __future__ import annotations

import collections
import pathlib
import re
import sys

from regions import ROOT, rs_files
from rslex import ident_name, lex
from vocab import KW, split_words

HERE = pathlib.Path(__file__).resolve().parent
ACCENT = re.compile("[áéíóúñüÁÉÍÓÚÑ¿¡]")
SP_STRONG = set("el la los las del que con para por una sin como cuando pero también cada entre sobre desde hasta".split())
SP_WEAK = set("de se es un lo en al su si y no".split())
# palabras que están en los diccionarios pero valen en inglés (o son el sufijo de idioma `_es` de los textos visibles)
ENGLISH_OK = set("a o y no es la lo le os describe modulo normal actual error color texto cargo".split())
ASSERT_MACROS = {
    "assert": 1, "assert_eq": 2, "assert_ne": 2, "debug_assert": 1, "debug_assert_eq": 2, "debug_assert_ne": 2,
    "panic": 0, "unreachable": 0, "todo": 0, "unimplemented": 0,
}


def spanish_words() -> set[str]:
    out = set()
    for f in ("glossary.tsv", "glossary-l2.tsv"):
        for line in (HERE / f).read_text(encoding="utf-8").splitlines():
            if line.startswith("#") or "\t" not in line:
                continue
            a, b = line.split("\t")[:2]
            if a != b and a not in ENGLISH_OK:
                out.add(a)
    return out


SP = spanish_words()


def looks_spanish(text: str) -> bool:
    if ACCENT.search(text):
        return True
    words = re.findall(r"[a-záéíóúñü]+", text.lower())
    return sum(w in SP_STRONG for w in words) >= 1 or sum(w in SP_WEAK for w in words) >= 2


def crate_of(rel: str) -> str:
    parts = rel.split("/")
    return parts[1] if parts[0] == "crates" else parts[0]


def scan() -> dict[str, dict[str, list]]:
    res: dict[str, dict[str, list]] = collections.defaultdict(lambda: {"ids": [], "comments": [], "messages": []})
    for p in rs_files():
        rel = str(p.relative_to(ROOT))
        c = crate_of(rel)
        toks = lex(p.read_text(encoding="utf-8"))
        line = 1
        stack: list[tuple[str, int, int, int]] = []  # (macro, profundidad, argumento actual, mensaje esperado)
        depth = 0
        for i, t in enumerate(toks):
            if t.kind == "ident":
                n = ident_name(t)
                if n not in KW and any(w in SP for w in split_words(n)):
                    res[c]["ids"].append((rel, line, n))
            elif t.kind == "comment":
                for k, l in enumerate(t.text.split("\n")):
                    if looks_spanish(l):
                        res[c]["comments"].append((rel, line + k, l.strip()))
            elif t.kind == "punct":
                if t.text in "([{":
                    depth += 1
                    if t.text in "([" and i >= 2 and toks[i - 1].text == "!" and toks[i - 2].kind == "ident":
                        m = ident_name(toks[i - 2])
                        if m in ASSERT_MACROS:
                            stack.append((m, depth, 0, ASSERT_MACROS[m]))
                    elif t.text == "(" and i >= 1 and toks[i - 1].kind == "ident" and ident_name(toks[i - 1]) in ("expect", "expect_err"):
                        stack.append(("expect", depth, 0, 0))
                elif t.text in ")]}":
                    if stack and stack[-1][1] == depth:
                        stack.pop()
                    depth -= 1
                elif t.text == "," and stack and stack[-1][1] == depth:
                    m, d, a, e = stack[-1]
                    stack[-1] = (m, d, a + 1, e)
            elif t.kind == "string" and stack and stack[-1][1] == depth and stack[-1][2] == stack[-1][3]:
                if looks_spanish(t.text):
                    res[c]["messages"].append((rel, line, t.text.strip()))
            line += t.text.count("\n")
    # Cargo.toml y deny.toml
    for top in sorted((ROOT / "crates").glob("*/Cargo.toml")) + [ROOT / "Cargo.toml", ROOT / "deny.toml", ROOT / "fuzz/Cargo.toml", ROOT / "fuzz/deny.toml"]:
        rel = str(top.relative_to(ROOT))
        for k, l in enumerate(top.read_text(encoding="utf-8").splitlines(), 1):
            if l.lstrip().startswith("#") and looks_spanish(l):
                res[crate_of(rel)]["comments"].append((rel, k, l.strip()))
    return res


def main(argv: list[str]) -> int:
    crate = argv[argv.index("--crate") + 1] if "--crate" in argv else None
    limit = int(argv[argv.index("--limit") + 1]) if "--limit" in argv else 40
    res = scan()
    if "--list" in argv:
        kind = argv[argv.index("--list") + 1]
        for c in sorted(res):
            if crate and c != crate:
                continue
            rows = res[c][kind]
            if kind == "ids":
                cnt = collections.Counter(n for _, _, n in rows)
                for n, k in cnt.most_common(limit):
                    print(f"{c}\t{k}\t{n}")
            else:
                for rel, ln, tx in rows[:limit]:
                    print(f"{rel}:{ln}\t{tx[:110]}")
        return 0
    tot = [0, 0, 0, 0]
    print(f"{'crate':<12}{'identificadores':>18}{'(usos)':>9}{'comentarios':>13}{'mensajes':>10}")
    for c in sorted(res):
        r = res[c]
        d = len({n for _, _, n in r["ids"]})
        row = [d, len(r["ids"]), len(r["comments"]), len(r["messages"])]
        tot = [a + b for a, b in zip(tot, row)]
        print(f"{c:<12}{row[0]:>18}{row[1]:>9}{row[2]:>13}{row[3]:>10}")
    print(f"{'total':<12}{tot[0]:>18}{tot[1]:>9}{tot[2]:>13}{tot[3]:>10}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
