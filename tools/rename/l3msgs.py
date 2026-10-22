#!/usr/bin/env python3
"""Mensajes de aserción de las pruebas (specs/limpieza-1.md, etapa L3a): extrae los que están en español y aplica la traducción.

    l3msgs.py extract <crate> <salida.jsonl>        un objeto JSON por mensaje: id, fichero, desplazamiento, texto del literal
    l3msgs.py show <salida.jsonl> [desde] [hasta]   id y texto interior (⏎ = salto de línea), para traducir
    l3msgs.py apply <crate> <salida.jsonl> <traducciones.tsv>   `id<TAB>texto nuevo` (con ⏎ por cada salto de línea); sustituye el interior del literal

Solo el argumento de mensaje de assert!/assert_eq!/assert_ne!/debug_assert*!/prop_assert*!/panic!/unreachable!/todo!
(msgpos.py), solo en código de pruebas; el literal nuevo debe llevar los mismos huecos `{…}` (en cualquier orden).
Los valores esperados y los literales de producción no se tocan.
"""
from __future__ import annotations

import json
import re
import sys

from msgpos import message_indices
from regions import ROOT, classify
from rename import PROTECTED
from rslex import lex
from spanish_left import looks_spanish


def holes(lit: str) -> list[str]:
    s = lit.replace("{{", "").replace("}}", "")
    return sorted(re.findall(r"\{[^{}]*\}", s))


def messages(crate: str):
    cls = classify()
    for p, v in sorted(cls.items()):
        rel = str(p.relative_to(ROOT))
        if not rel.startswith(f"crates/{crate}/") or rel in PROTECTED:
            continue
        toks = lex(p.read_text(encoding="utf-8"))
        sig = [(i, t) for i, t in enumerate(toks) if t.kind not in ("ws", "comment")]
        items = [(t.kind, t.text) for _, t in sig]
        for j in message_indices(items):
            i, t = sig[j]
            if v != "test" and not any(a <= i < b for a, b in v):
                continue
            if t.text.startswith(("r", "b")):
                continue
            if looks_spanish(t.text):
                yield rel, t.start, t.text


def main(argv):
    cmd = argv[1]
    if cmd == "extract":
        rows = [{"id": n, "file": f, "start": s, "text": x} for n, (f, s, x) in enumerate(messages(argv[2]), 1)]
        with open(argv[3], "w", encoding="utf-8") as fh:
            for r in rows:
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")
        print(len(rows), "mensajes")
        return 0
    if cmd == "show":
        rows = [json.loads(l) for l in open(argv[2], encoding="utf-8")]
        lo = int(argv[3]) if len(argv) > 3 else 1
        hi = int(argv[4]) if len(argv) > 4 else len(rows)
        for r in rows:
            if lo <= r["id"] <= hi:
                print(f'{r["id"]}\t{r["text"][1:-1].replace(chr(10), "⏎")}')
        return 0
    if cmd == "apply":
        rows = {r["id"]: r for r in map(json.loads, open(argv[3], encoding="utf-8"))}
        tr = {}
        for l in open(argv[4], encoding="utf-8"):
            if l.strip():
                i, lit = l.rstrip("\n").split("\t", 1)
                tr[int(i)] = lit.replace("⏎", "\n")
        by_file: dict[str, list] = {}
        bad = 0
        for i, r in rows.items():
            if i not in tr:
                continue
            new = '"' + tr[i] + '"'
            if holes(new) != holes(r["text"]):
                print("huecos distintos en", i, holes(r["text"]), holes(new)); bad += 1; continue
            by_file.setdefault(r["file"], []).append((r["start"], r["text"], new))
        if bad:
            return 1
        for rel, lst in by_file.items():
            p = ROOT / rel
            src = p.read_text(encoding="utf-8")
            for start, old, new in sorted(lst, reverse=True):
                assert src[start:start + len(old)] == old, (rel, start)
                src = src[:start] + new + src[start + len(old):]
            p.write_text(src, encoding="utf-8")
        print(len(tr), "mensajes traducidos en", len(by_file), "ficheros")
        return 0
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
