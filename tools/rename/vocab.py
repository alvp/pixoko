"""Vocabulario de los identificadores de producción: palabras, usos y clase según los diccionarios."""
from __future__ import annotations

import collections
import re
import sys
import unicodedata

from regions import ROOT, classify
from rslex import ident_name, lex

KW = set("as break const continue crate else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while async await dyn".split())


def split_words(ident: str) -> list[str]:
    out = []
    for part in ident.split("_"):
        out += re.findall(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+", part)
    return [w.lower() for w in out if w.isalpha()]


def prod_idents() -> collections.Counter:
    cls = classify()
    cnt: collections.Counter = collections.Counter()
    for p, v in cls.items():
        if v == "test":
            continue
        toks = lex(p.read_text(encoding="utf-8"))
        skip = [False] * len(toks)
        for a, b in v:
            for k in range(a, min(b, len(toks))):
                skip[k] = True
        for k, t in enumerate(toks):
            if t.kind == "ident" and not skip[k]:
                n = ident_name(t)
                if n not in KW:
                    cnt[n] += 1
    return cnt


if __name__ == "__main__":
    cnt = prod_idents()
    words = collections.Counter()
    for i, c in cnt.items():
        for w in split_words(i):
            words[w] += c
    print("identificadores de producción", len(cnt), "usos", sum(cnt.values()), "palabras", len(words), file=sys.stderr)
    for w, c in sorted(words.items()):
        print(f"{w}\t{c}")
