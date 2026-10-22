"""Clasifica cada fichero y cada tramo de código en producción o pruebas (specs/limpieza-1.md).

Producción: `crates/*/src` salvo los módulos `#[cfg(test)]` (en línea o en fichero, con todo lo que
cuelga de ellos), `tests.rs`, `*_tests.rs` y las carpetas `tests`. `tests/` de cada crate, `fuzz/` y
`tools/` cuentan como pruebas (nunca generan nombres de producción).
"""
from __future__ import annotations

import pathlib
import re

from rslex import Tok, lex

ROOT = pathlib.Path(__file__).resolve().parents[2]
CFG_TEST = re.compile(r"#\s*\[\s*cfg\s*\(\s*(?:all\s*\()?\s*test\b")


def rs_files() -> list[pathlib.Path]:
    out = []
    for top in ("crates", "fuzz"):
        for p in sorted((ROOT / top).rglob("*.rs")):
            if "target" not in p.parts:
                out.append(p)
    return out


def _sig(toks: list[Tok], i: int) -> int:
    """Índice del siguiente token que no es espacio ni comentario."""
    while i < len(toks) and toks[i].kind in ("ws", "comment"):
        i += 1
    return i


def _attr_end(toks: list[Tok], i: int) -> int:
    """`i` apunta a `#`: índice del `]` que cierra el atributo."""
    j = _sig(toks, i + 1)
    if j < len(toks) and toks[j].text == "!":
        j = _sig(toks, j + 1)
    depth = 0
    while j < len(toks):
        if toks[j].text == "[":
            depth += 1
        elif toks[j].text == "]":
            depth -= 1
            if depth == 0:
                return j
        j += 1
    return len(toks) - 1


def _item_end(toks: list[Tok], i: int) -> int:
    """Final (índice inclusivo) del elemento que empieza en `i` (tras los atributos)."""
    depth, opened = 0, False
    j = i
    while j < len(toks):
        t = toks[j].text
        if toks[j].kind == "punct":
            if t in "([{":
                depth += 1
                if t == "{" and depth == 1:
                    opened = True
            elif t in ")]}":
                depth -= 1
                if depth == 0 and t == "}" and opened:
                    # `struct S {..}` o `mod m {..}` terminan aquí; `const X = S {..};` sigue hasta `;`
                    k = _sig(toks, j + 1)
                    if not (k < len(toks) and toks[k].text == ";"):
                        return j
            elif t == ";" and depth == 0:
                return j
        j += 1
    return len(toks) - 1


def test_ranges(toks: list[Tok]) -> tuple[list[tuple[int, int]], list[tuple[str | None, str]]]:
    """(tramos [inicio, fin) en índices de token que son de pruebas, módulos `mod x;` de pruebas)."""
    ranges: list[tuple[int, int]] = []
    modfiles: list[tuple[str | None, str]] = []
    i = 0
    while i < len(toks):
        if toks[i].text == "#" and toks[i].kind == "punct":
            e = _attr_end(toks, i)
            attr = "".join(t.text for t in toks[i : e + 1])
            if CFG_TEST.match(attr) and "not(test" not in attr.replace(" ", ""):
                # saltar otros atributos (guardando `#[path]`)
                j = _sig(toks, e + 1)
                path = None
                while j < len(toks) and toks[j].text == "#":
                    e2 = _attr_end(toks, j)
                    a2 = "".join(t.text for t in toks[j : e2 + 1])
                    m = re.match(r'#\s*\[\s*path\s*=\s*"([^"]*)"', a2)
                    if m:
                        path = m.group(1)
                    j = _sig(toks, e2 + 1)
                end = _item_end(toks, j)
                ranges.append((i, end + 1))
                # `mod x;` → fichero de pruebas
                k = j
                if toks[k].text == "pub":
                    k = _sig(toks, k + 1)
                    if toks[k].text == "(":
                        while toks[k].text != ")":
                            k += 1
                        k = _sig(toks, k + 1)
                if toks[k].text == "mod":
                    name = toks[_sig(toks, k + 1)].text
                    after = _sig(toks, _sig(toks, k + 1) + 1)
                    if toks[after].text == ";":
                        modfiles.append((path, name))
                i = end + 1
                continue
            i = e + 1
        else:
            i += 1
    return ranges, modfiles


def is_test_file_by_name(p: pathlib.Path) -> bool:
    rel = p.relative_to(ROOT)
    if rel.parts[0] == "fuzz":
        return True
    if "tests" in rel.parts[:-1]:
        return True
    return rel.name == "tests.rs" or rel.name.endswith("_tests.rs")


def classify() -> dict[pathlib.Path, list[tuple[int, int]] | str]:
    """Para cada fichero: 'test' (entero) o la lista de tramos de pruebas dentro de él (puede ser vacía)."""
    files = rs_files()
    result: dict[pathlib.Path, list[tuple[int, int]] | str] = {}
    toks_of = {}
    testmods: list[pathlib.Path] = []
    for p in files:
        if is_test_file_by_name(p):
            result[p] = "test"
            continue
        toks = lex(p.read_text(encoding="utf-8"))
        toks_of[p] = toks
        ranges, modfiles = test_ranges(toks)
        result[p] = ranges
        for path, name in modfiles:
            d = p.parent if (p.name in ("lib.rs", "main.rs", "mod.rs")) else p.parent / p.stem
            if path:
                cand = [p.parent / path]
            else:
                cand = [d / f"{name}.rs", d / name / "mod.rs"]
            for c in cand:
                if c.exists():
                    testmods.append(c.resolve())
                    break
            else:
                raise SystemExit(f"no encuentro el módulo de pruebas {name} declarado en {p}")
    for c in testmods:
        # el fichero y todo lo que cuelga de su carpeta
        result[c] = "test"
        sub = c.parent / c.stem if c.name != "mod.rs" else c.parent
        for p in files:
            if sub in p.parents:
                result[p] = "test"
    return result


if __name__ == "__main__":
    r = classify()
    prod = [p for p, v in r.items() if v != "test"]
    print("ficheros", len(r), "con producción", len(prod))
    for p in prod[:0]:
        print(p)
