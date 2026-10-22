#!/usr/bin/env python3
"""Etapa L2: genera el mapa de identificadores de pruebas (glossary-l2.tsv -> map-l2.tsv).

    l2gen.py stats     cifras y palabras sin traducir
    l2gen.py gen       escribe map-l2.tsv (`id old new crates/<crate>/`), con overrides-l2.tsv encima

Un identificador es español si tiene alguna palabra «inequívoca» del glosario (no válida en inglés);
entonces se traducen todas sus palabras. `y`, `a`, `o`… solas o en nombres ingleses (`max_y`) no se tocan.
"""
from __future__ import annotations

import collections
import json
import pathlib
import re
import subprocess
import sys

from regions import ROOT, rs_files
from rslex import ident_name, lex
from vocab import KW, split_words

HERE = pathlib.Path(__file__).resolve().parent
GLOSS: dict[str, str] = {}
for line in (HERE / "glossary-l2.tsv").read_text(encoding="utf-8").splitlines():
    if line.startswith("#") or not line.strip():
        continue
    a, b = line.split("\t")
    GLOSS[a] = b


def english_ok(words):
    r = subprocess.run(["hunspell", "-d", "/home/alvaro/.AffinityLinux/drive_c/Program Files/Affinity/Affinity/Resources/Dictionaries/en-US/en_US", "-l"],
                       input="\n".join(words), capture_output=True, text=True)
    bad = set(r.stdout.split())
    return {w for w in words if w not in bad}


_AMBIG: set[str] | None = None


def ambiguous() -> set[str]:
    global _AMBIG
    if _AMBIG is None:
        _AMBIG = english_ok(list(GLOSS))
        _AMBIG |= {w for w in GLOSS if len(w) <= 2}
        _AMBIG -= set(OVERRIDE_UNAMBIGUOUS)
    return _AMBIG


# palabras válidas en inglés que aquí son inequívocamente españolas
OVERRIDE_UNAMBIGUOUS = {"sin", "con", "pie", "solo", "mas", "hay", "dos", "alto", "caso", "banco", "paso", "tres", "ve", "dan", "da", "cada",
                        "fila", "modo", "mano", "mal", "mil", "nada", "cuando", "pero", "entre", "hasta", "tal", "casa", "cola", "cara",
                        "pie", "rota", "sub", "radio", "mesa", "era", "ante", "novena"}


import unicodedata


def fold(w: str) -> str:
    """Sin acentos ni eñes: las claves del glosario."""
    return "".join(c for c in unicodedata.normalize("NFD", w) if unicodedata.category(c) != "Mn").replace("ñ", "n")


LOW = "a-zñáéíóúü"
UP = "A-ZÑÁÉÍÓÚÜ"


def words_of(name: str):
    parts = []
    for p in name.split("_"):
        parts.append(re.findall(rf"[{UP}]+(?![{LOW}])|[{UP}]?[{LOW}]+|\d+", p) or ([p] if p else []))
    return parts


def is_spanish(name: str) -> bool:
    amb = ambiguous()
    for ws in words_of(name):
        for w in ws:
            lw = fold(w.lower())
            if lw in GLOSS and lw not in amb and lw != GLOSS[lw]:
                return True
    return False


# frases hechas que no salen palabra por palabra (se aplican a los nombres en minúscula, entre `_` o en los extremos)
PHRASES = {
    "ida_y_vuelta": "round_trip", "por_defecto": "default", "por_encima_de": "above", "por_debajo_de": "below",
    "a_mano": "by_hand", "al_reves": "reversed", "fuera_de_rango": "out_of_range", "fuera_de_paleta": "out_of_palette",
    "fuera_de_la_paleta": "outside_the_palette", "fuera_del_lienzo": "outside_the_canvas", "de_uno_en_uno": "one_by_one",
    "uno_a_uno": "one_by_one", "en_lote": "in_batch", "en_vivo": "live", "en_directo": "live", "tal_cual": "as_is",
    "sin_cambios": "unchanged", "sin_cambio": "unchanged",
}


def phrase_sub(name: str) -> str:
    low = name.lower()
    if low != name:  # solo nombres en minúscula
        return name
    for k, v in PHRASES.items():
        name = re.sub(rf"(?<![a-z0-9]){k}(?![a-z0-9])", v, name)
    return name


# palabras reservadas de Rust que salen de una traducción; la alternativa se aplica al nombre entero
KEYWORD_ALT = {"type": "kind", "return": "ret", "move": "moved", "match": "matching", "loop": "cycle", "use": "usage", "mod": "module",
               "ref": "reference", "self": "itself", "static": "fixed", "fn": "func", "impl": "implementation", "as": "as_word",
               "let": "bind", "in": "inside", "for": "for_each", "if": "when", "else": "otherwise", "while": "during", "break": "stop",
               "continue": "proceed", "const": "constant", "enum": "enumeration", "struct": "structure", "trait": "trait_kind",
               "where": "location", "box": "box_value", "dyn": "dynamic", "mut": "mutable", "pub": "public", "super": "parent_scope",
               "unsafe": "unchecked", "crate": "crate_root", "extern": "external", "true": "true_value", "false": "false_value"}


def translate(name: str) -> str:
    name = phrase_sub(name)
    out = _translate(name)
    return KEYWORD_ALT.get(out, KEYWORD_ALT.get(out.lower(), out)) if out in KEYWORD_ALT or out.lower() in KEYWORD_ALT and out[:1].islower() else out


def _translate(name: str) -> str:
    lead = re.match(r"_*", name).group(0)
    trail = re.search(r"_*$", name).group(0)
    core = name[len(lead) : len(name) - len(trail)] if trail else name[len(lead) :]
    upper = core.isupper() or (core.replace("_", "").isupper() and any(c.isalpha() for c in core))
    pascal = core[:1].isupper() and not upper and "_" not in core
    out: list[str] = []
    flat = [w for ws in words_of(core) for w in ws]
    for i, w in enumerate(flat):
        lw = fold(w.lower())
        if w.isdigit():
            t = w
        elif lw in ("y", "a", "o", "e", "u") and not (
            0 < i < len(flat) - 1 and len(flat[i - 1]) >= 2 and len(flat[i + 1]) >= 2 and flat[i - 1].isalpha() and flat[i + 1].isalpha()
        ):
            t = lw  # coordenada o letra suelta: no es la palabra española
        else:
            t = GLOSS.get(lw, lw)
        for piece in t.split("_") if t != "-" else []:
            out.append(piece)
    if pascal:
        s = "".join(p[:1].upper() + p[1:] for p in out)
    elif upper:
        s = "_".join(out).upper()
    else:
        s = "_".join(out)
    return lead + s + trail


def crate_of(rel: str) -> str:
    parts = rel.split("/")
    return "/".join(parts[:2]) + "/"


def collect():
    """identificador -> {crate: nº de usos}"""
    uses: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    for p in rs_files():
        rel = str(p.relative_to(ROOT))
        if rel == "crates/px-golden/tests/ci.rs":
            continue  # protegido: solo sus comentarios cambian (L3)
        c = crate_of(rel)
        for t in lex(p.read_text(encoding="utf-8")):
            if t.kind == "ident":
                n = ident_name(t)
                if n not in KW:
                    uses[n][c] += 1
    return uses


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "stats"
    uses = collect()
    spanish = {n: translate(n) for n in uses if is_spanish(n)}
    spanish = {o: n for o, n in spanish.items() if o != n}
    if cmd == "stats":
        crates = collections.Counter()
        for o in spanish:
            for c in uses[o]:
                crates[c] += 1
        print(len(spanish), "identificadores españoles;", dict(crates))
        # palabras sin traducir dentro de identificadores españoles
        en = None
        unk = collections.Counter()
        allw = {w.lower() for o in spanish for ws in words_of(o) for w in ws if not w.isdigit()}
        newwords = {w.lower() for n in spanish.values() for ws in words_of(n) for w in ws if not w.isdigit()}
        # palabras de los nombres nuevos que no son inglés
        bad = set(newwords) - english_ok(list(newwords))
        print("palabras nuevas no inglesas (revisar):", len(bad))
        print(" ".join(sorted(bad)))
        return
    if cmd == "gen":
        ov = {}
        f = HERE / "overrides-l2.tsv"
        if f.exists():
            for line in f.read_text(encoding="utf-8").splitlines():
                if line.strip() and not line.startswith("#"):
                    o, n = line.split("\t")[:2]
                    ov[o] = n
        auto = collections.defaultdict(list)  # old -> [(alt, scope)]
        fa = HERE / "overrides-l2-auto.tsv"
        if fa.exists():
            for line in fa.read_text(encoding="utf-8").splitlines():
                if line.strip() and not line.startswith("#"):
                    o, alt, scope = line.split("\t")
                    auto[o].append((alt, scope))
        applied = set((HERE / "l2-applied-crates.txt").read_text().split()) if (HERE / "l2-applied-crates.txt").exists() else set()
        rows = []
        for o, n in sorted(spanish.items()):
            n = ov.get(o, n)
            for c in sorted(uses[o]):
                if c.split("/")[1] in applied:
                    continue
                base = n
                for alt, scope in auto.get(o, []):
                    if scope == c:
                        base = alt
                for alt, scope in auto.get(o, []):
                    if scope != c and scope.startswith(c):
                        rows.append(("id", o, alt, scope))
                rows.append(("id", o, base, c))
        (HERE / "map-l2.tsv").write_text("# kind\told\tnew\tscope (generado por l2gen.py; glossary-l2.tsv + overrides-l2.tsv)\n" + "\n".join("\t".join(r) for r in rows) + "\n", encoding="utf-8")
        print(len(rows), "filas")


if __name__ == "__main__":
    main()
