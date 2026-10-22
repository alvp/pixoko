"""Colisiones por función (ámbito real) para L2: un nombre nuevo que ya aparece dentro de la misma
función que el viejo, o un elemento de módulo que pisaría a otro de producción del crate."""
from __future__ import annotations

import collections
import sys

import rename as R
from regions import ROOT, rs_files, classify
from rslex import ident_name, lex

ITEM_KW = {"fn", "struct", "enum", "const", "static", "type", "trait", "mod", "union"}
# nombres del preludio de std (y de macros habituales): un nombre nuevo igual a uno de ellos los taparía
PRELUDE = {"Box", "Vec", "Option", "Some", "None", "Ok", "Err", "Result", "String", "Default", "Clone", "Copy", "Send", "Sync", "Drop", "Fn",
           "FnMut", "FnOnce", "Iterator", "IntoIterator", "From", "Into", "ToString", "ToOwned", "Eq", "PartialEq", "Ord", "PartialOrd", "AsRef",
           "AsMut", "Extend", "DoubleEndedIterator", "ExactSizeIterator", "TryFrom", "TryInto", "FromIterator", "Debug", "Hash", "Self", "Sized",
           "Unpin", "drop", "Display", "Error", "Path", "File", "Rc", "Arc", "Cell", "RefCell", "Ordering", "Duration", "Instant"}


def fn_regions(toks):
    """[(inicio, fin)) de cada `fn` (de la palabra clave al cierre de su cuerpo) y profundidad de llaves por token."""
    regs = []
    depth = 0
    stack = []  # (profundidad de apertura, inicio) de los cuerpos de fn
    pending = []  # fn vistas cuyo `{` aún no ha llegado
    for i, t in enumerate(toks):
        if t.kind == "ident" and t.text == "fn":
            pending.append(i)
        if t.kind == "punct":
            if t.text == "{":
                depth += 1
                if pending:
                    stack.append((depth, pending.pop()))
            elif t.text == "}":
                if stack and stack[-1][0] == depth:
                    _, s = stack.pop()
                    regs.append((s, i + 1))
                depth -= 1
            elif t.text == ";" and pending and depth >= 0:
                # `fn f();` sin cuerpo (trait): se descarta
                pending = [p for p in pending if p > i] if False else pending
    return regs


def prod_items(crate: str):
    """Nombres de elementos de módulo declarados en producción del crate."""
    names = set()
    cls = classify()
    for p, v in cls.items():
        rel = str(p.relative_to(ROOT))
        if not rel.startswith(crate) or v == "test":
            continue
        toks = lex(p.read_text(encoding="utf-8"))
        skip = [False] * len(toks)
        for a, b in v:
            for k in range(a, min(b, len(toks))):
                skip[k] = True
        for i, t in enumerate(toks):
            if t.kind == "ident" and t.text in ITEM_KW and not skip[i]:
                j = i + 1
                while j < len(toks) and toks[j].kind in ("ws", "comment"):
                    j += 1
                if j < len(toks) and toks[j].kind == "ident":
                    names.add(ident_name(toks[j]))
    return names


def crate_modnames(crate_dir: str):
    """Nombres de elementos y de `use` de todos los ficheros del crate (los `use mod::*` de otro fichero los ven)."""
    names = set()
    for p in rs_files():
        rel = str(p.relative_to(ROOT))
        if not rel.startswith(crate_dir):
            continue
        toks = lex(p.read_text(encoding="utf-8"))
        in_use = False
        depth = 0
        for i, tk in enumerate(toks):
            if tk.kind == "ident":
                if tk.text == "use":
                    in_use = True
                elif in_use:
                    names.add(ident_name(tk))
                elif tk.text in ITEM_KW:
                    j = i + 1
                    while j < len(toks) and toks[j].kind in ("ws", "comment"):
                        j += 1
                    if j < len(toks) and toks[j].kind == "ident":
                        names.add(ident_name(toks[j]))
            elif tk.kind == "punct" and tk.text == ";":
                in_use = False
    return names


def find(crate: str | None = None):
    found = []
    items_cache: dict[str, set[str]] = {}
    cmod_cache: dict[str, set[str]] = {}
    for p in rs_files():
        rel = str(p.relative_to(ROOT))
        if crate and not rel.startswith(crate):
            continue
        src = p.read_text(encoding="utf-8")
        toks = lex(src)
        regs = fn_regions(toks)
        # ident -> índices
        idx = collections.defaultdict(list)
        bare = collections.defaultdict(list)  # apariciones sueltas: ni `.x` ni `a::x` ni `x:` de un campo
        for i, t in enumerate(toks):
            if t.kind == "ident":
                n = ident_name(t)
                idx[n].append(i)
                j = i - 1
                while j >= 0 and toks[j].kind in ("ws", "comment"):
                    j -= 1
                prev = toks[j].text if j >= 0 else ""
                after_path = prev == "." or (prev == ":" and j >= 1 and toks[j - 1].text == ":")
                if not after_path:
                    bare[n].append(i)
        cr = "/".join(rel.split("/")[:2]) + "/"
        if cr not in items_cache:
            items_cache[cr] = prod_items(cr)
        items = items_cache[cr]
        if cr not in cmod_cache:
            cmod_cache[cr] = crate_modnames(cr)
        cmods = cmod_cache[cr]
        # fichero: en `fn_of[i]` la región más interior que contiene el token i
        def region(i):
            best = None
            for a, b in regs:
                if a <= i < b and (best is None or a >= best[0]):
                    best = (a, b)
            return best
        # nombres que el módulo del fichero ya define o importa (elementos fuera de las funciones y `use`)
        modnames = set()
        in_use = False
        for i, tk in enumerate(toks):
            if tk.kind == "ident":
                if tk.text == "use":
                    in_use = True
                elif in_use:
                    modnames.add(ident_name(tk))
                elif tk.text in ITEM_KW and region(i) is None:
                    j = i + 1
                    while j < len(toks) and toks[j].kind in ("ws", "comment"):
                        j += 1
                    if j < len(toks) and toks[j].kind == "ident":
                        modnames.add(ident_name(toks[j]))
            elif tk.kind == "punct" and tk.text == ";":
                in_use = False
        # inyectividad: dos nombres viejos del mismo fichero que dan el mismo nuevo (singular y plural, p. ej.)
        by_new = collections.defaultdict(list)
        for old in idx:
            nw_ = R.id_target(old, rel)
            if nw_:
                by_new[nw_].append(old)
        for nw_, olds in by_new.items():
            if len(olds) > 1:
                olds = sorted(olds, key=lambda o: (len(o), o))
                for o2 in olds[1:]:
                    found.append((rel, "dup", o2, nw_, f"mismo nombre nuevo que {olds[0]}"))
        done = set()
        for old, ixs in idx.items():
            new = R.id_target(old, rel)
            if not new:
                continue
            if new in PRELUDE and ("prelude", old) not in done:
                found.append((rel, "item", old, new, "tapa un nombre del preludio o de std"))
                done.add(("prelude", old))
            for i in ixs:
                r = region(i)
                if r is None:
                    # elemento de módulo: pisa a uno de producción del mismo nombre
                    if (new in items or new in modnames or new in cmods) and ("mod", old) not in done:
                        found.append((rel, "item", old, new, "pisa un elemento de producción o del módulo"))
                        done.add(("mod", old))
                    # o ya existe como elemento de módulo en este fichero
                    continue
                if (r, old) in done:
                    continue
                if any(r[0] <= j < r[1] for j in bare.get(new, [])):
                    found.append((rel, "fn", old, new, f"ya está dentro de la misma función (línea {src.count(chr(10), 0, toks[r[0]].start) + 1})"))
                    done.add((r, old))
    return found


if __name__ == "__main__":
    crate = sys.argv[1] if len(sys.argv) > 1 else None
    f = find(crate)
    c = collections.Counter(x[1] for x in f)
    print(len(f), dict(c))
    for x in f[:40]:
        print(x)
