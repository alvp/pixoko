#!/usr/bin/env python3
"""Comprobaciones P1 a P4 y P3b de specs/limpieza-1.md (apartado 3).

    verify.py baseline [--release]   guarda el estado de ahora en ~/.cache/pixoko-l1/base
    verify.py check [--release]      lo compara con el estado de ahora, aplicando map.tsv a lo de antes

  P1  lista de pruebas (`cargo test --workspace -- --list`): idéntica salvo el mapa
  P2  literales de cadena: producción idéntica salvo las variables de entorno y los argumentos en
      línea; pruebas: informe de diferencias
  P3  `px-app --licencias` y errores de uso (mismos bytes y códigos), SHA-256 de `golden/`
      (mismos contenidos; los nombres, salvo el mapa) y, con --release, las cadenas del ejecutable
  P4  equivalencia alfa: los tokens de cada fichero solo difieren en pares viejo -> nuevo del mapa

El léxico de aquí (expresiones regulares) es independiente del de `rslex.py`, que usa `rename.py`.
Las compilaciones desechables van a ~/.cache/pixoko-scratch (CLAUDE.md: nunca en /tmp).
"""
from __future__ import annotations

import collections
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import rename as R  # solo el mapa y la lista de ficheros
from regions import ROOT, classify

CACHE = pathlib.Path.home() / ".cache" / "pixoko-l1"
SCRATCH = pathlib.Path.home() / ".cache" / "pixoko-scratch"
TOK = re.compile(
    r"""(?P<lc>//[^\n]*)|(?P<bc>/\*.*?\*/)|(?P<rs>b?r(?P<h>\#*)".*?"(?P=h))|(?P<s>[bc]?"(?:\\.|[^"\\])*")|(?P<ch>b?'(?:\\.|[^'\\])')
    |(?P<lt>'[A-Za-z_]\w*)|(?P<id>[A-Za-z_]\w*)|(?P<num>\d[\w]*(?:\.\d[\w]*)?)|(?P<p>\S)""",
    re.S | re.X,
)


def tokens(src: str):
    out = []
    for m in TOK.finditer(src):
        k = m.lastgroup
        if k == "h":
            k = "rs"
        if k in ("lc", "bc"):
            continue
        out.append(("s" if k in ("rs", "s") else k, m.group(0)))
    return out


def sh(cmd, **kw):
    return subprocess.run(cmd, shell=isinstance(cmd, str), cwd=ROOT, capture_output=True, text=True, **kw)


# ---------------------------------------------------------------------------------------------
def snapshot(release: bool):
    snap = {}
    # P4 y P2: tokens y literales por fichero
    cls = classify()
    toks, lits = {}, {}
    for p, v in cls.items():
        rel = str(p.relative_to(ROOT))
        t = tokens(p.read_text(encoding="utf-8"))
        toks[rel] = [x for _, x in t]
        kinds = [k for k, _ in t]
        toks[rel] = [[k, x] for k, x in t]
        lits[rel] = {"role": "test" if v == "test" else "mixed", "lits": [x for k, x in t if k == "s"]}
    snap["tokens"] = toks
    snap["tracked"] = R.tracked_files()
    # producción: literales fuera de #[cfg(test)] (misma clasificación que regions.py, pero por texto)
    prod = collections.Counter()
    for p, v in cls.items():
        if v == "test":
            continue
        from rslex import lex
        lt = lex(p.read_text(encoding="utf-8"))
        skip = [False] * len(lt)
        for a, b in v:
            for k in range(a, min(b, len(lt))):
                skip[k] = True
        for k, t in enumerate(lt):
            if t.kind == "string" and not skip[k]:
                prod[(str(p.relative_to(ROOT)), t.text)] += 1
    snap["prod_literals"] = [[f, s, n] for (f, s), n in sorted(prod.items())]
    # golden
    gold = {}
    for p in sorted((ROOT / "golden").rglob("*")):
        if p.is_file():
            gold[str(p.relative_to(ROOT))] = hashlib.sha256(p.read_bytes()).hexdigest()
    snap["golden"] = gold
    # P1
    r = sh("cargo test --workspace --locked -q -- --list 2>&1")
    lines = [l for l in r.stdout.splitlines() if l.endswith(": test") or l.endswith(": benchmark") or l.startswith("     Running") or l.startswith("   Doc-tests")]
    snap["tests"] = lines
    # P3a
    env = dict(os.environ, CARGO_TARGET_DIR=str(SCRATCH))
    subprocess.run("cargo build -q -p px-app --locked", shell=True, cwd=ROOT, env=env, check=True)
    exe = SCRATCH / "debug" / "px-app"
    cases = {"licencias": ["--licencias"], "desconocido": ["--no-existe"], "dos": ["a.png", "b.png"], "tres": ["a", "b", "c"]}
    outs = {}
    for name, args in cases.items():
        q = subprocess.run([str(exe), *args], capture_output=True, timeout=60, env=dict(os.environ, DISPLAY="", WAYLAND_DISPLAY=""))
        outs[name] = {"code": q.returncode, "out": hashlib.sha256(q.stdout).hexdigest(), "err": hashlib.sha256(q.stderr).hexdigest(), "outlen": len(q.stdout), "errlen": len(q.stderr)}
    snap["binary"] = outs
    if release:
        subprocess.run("cargo build -q --release -p px-app --locked", shell=True, cwd=ROOT, env=env, check=True)
        exe = SCRATCH / "release" / "px-app"
        stripped = SCRATCH / "px-app.stripped"
        subprocess.run(["strip", "-o", str(stripped), str(exe)], check=True)
        s = subprocess.run(["strings", "-n", "4", str(stripped)], capture_output=True, text=True).stdout.splitlines()
        snap["strings"] = sorted(set(s))
    return snap


# ---------------------------------------------------------------------------------------------
def name_pairs():
    ids = {}
    for old, new, _ in R.MAP["id"]:
        ids[old] = new
    for old, new, _ in R.MAP["mod"]:
        ids[old] = new
    return ids


def inverse_pairs():
    """nuevo -> viejo, con todos los ámbitos (los nombres con sufijo de colisión incluidos)."""
    inv = {}
    for old, new, _ in R.MAP["id"] + R.MAP["mod"]:
        inv.setdefault(new, old)
    return inv


def norm_literal(s: str) -> str:
    """Quita lo que el mapa puede cambiar: argumentos en línea, variables de entorno, casos y referencias."""
    if re.fullmatch(r'"[\w./-]+\.rs"', s):  # #[path = "…"]: los componentes de módulo cambian con el mapa
        s = re.sub(r"[A-Za-z_]\w*", lambda m: R.MODS.get(m.group(0), m.group(0)), s)
    s = re.sub(r"\{[A-Za-z_]\w*", "{", s)
    s = re.sub(r"[A-Za-z_]\w*\$", "$", s)
    for old, new in list(R.ENVS.items()) + list(R.CASES) + list(R.REFS.items()):
        s = s.replace(old, "<X>").replace(new, "<X>")
    return s


_MOVES = {}


def mapped_path(old: str) -> str:
    return _MOVES.get(old, old)


# Ficheros con una edición a mano (no mecánica) declarada en el commit correspondiente
MANUAL = {
    "crates/px-formats/tests/aseprite.rs": "case_word: el sufijo inglés del caso blend-<modo>; el nombre español sigue siendo el de la capa",
    "crates/px-golden/tests/blend.rs": "case_word: igual",
}


def check(base, now):
    ok = True
    _MOVES.clear()
    _MOVES.update(dict(R.move_targets(base["tracked"])))
    pairs = name_pairs()
    inv = {}
    # P1
    def map_test_line(l):
        def tr(m):
            return pairs.get(m.group(0), m.group(0))
        return re.sub(r"[A-Za-z_]\w*", tr, l)
    moves = {}
    moves = dict(_MOVES)
    inv = inverse_pairs()
    def unmap(l):
        return re.sub(r"[A-Za-z_]\w*", lambda m: inv.get(m.group(0), m.group(0)), l)
    exp = sorted(unmap(map_test_line(l)) for l in base["tests"])
    got = sorted(unmap(l) for l in now["tests"])
    # los ejecutables de tests/ se renombran con su fichero (mapa `file`)
    def mapfile(l):
        for o, n in R.FILES.items():
            if o.endswith(".rs"):
                stem_o, stem_n = pathlib.PurePosixPath(o).stem, pathlib.PurePosixPath(n).stem
                l = l.replace(f"tests/{stem_o}.rs", f"tests/{stem_n}.rs")
        return l
    exp = sorted(mapfile(l) for l in exp)
    # los nombres de módulo que aparecen en la ruta de una prueba también salen del mapa; las líneas «Running»
    # llevan la ruta del ejecutable con un hash que cambia: se comparan sin él
    strip_hash = lambda l: re.sub(r"\(target/[^)]*\)", "", l).strip()
    exp = [strip_hash(l) for l in exp]
    got = [strip_hash(l) for l in got]
    print(f"P1 pruebas: antes {len(base['tests'])}, ahora {len(now['tests'])}", "OK" if exp == got else "DIFIERE")
    if exp != got:
        ok = False
        a, b = collections.Counter(exp), collections.Counter(got)
        for l in list((a - b))[:10]: print("   solo esperado:", l)
        for l in list((b - a))[:10]: print("   solo ahora:   ", l)
    # P2
    b_lit = collections.Counter()
    for f, s, n in base["prod_literals"]:
        b_lit[(mapped_path(f), s)] += n
    n_lit = collections.Counter()
    for f, s, n in now["prod_literals"]:
        n_lit[(f, s)] += n
    diff_prod = []
    only_b = b_lit - n_lit
    only_n = n_lit - b_lit
    nb = collections.Counter((f, norm_literal(s)) for (f, s), n in only_b.items() for _ in range(n))
    nn = collections.Counter((f, norm_literal(s)) for (f, s), n in only_n.items() for _ in range(n))
    changed_env = [(f, s) for (f, s) in only_b if any(o in s for o in R.ENVS)]
    unexplained = (nb - nn) + (nn - nb)
    print(f"P2 literales de producción: {sum(b_lit.values())} antes, {sum(n_lit.values())} ahora;",
          f"cambian {sum(only_b.values())} (de ellos {len(changed_env)} con variable de entorno)",
          "OK" if not unexplained else "DIFIERE")
    for (f, s) in list(only_b)[:20]:
        print("   antes:", f, s[:90])
    for (f, s) in list(only_n)[:20]:
        print("   ahora:", f, s[:90])
    if unexplained:
        ok = False
        for x in list(unexplained)[:10]: print("   sin explicar:", x)
    # P2 pruebas: informe resumido
    tb = collections.Counter(); tn = collections.Counter()
    for f, v in base["tokens"].items():
        for k, x in v:
            if k == "s": tb[(mapped_path(f), x)] += 1
    for f, v in now["tokens"].items():
        for k, x in v:
            if k == "s": tn[(f, x)] += 1
    d_b, d_n = tb - tn, tn - tb
    print(f"P2 literales en total (producción y pruebas): cambian {sum(d_b.values())}")
    json.dump({"antes": [[f, s] for (f, s) in d_b], "ahora": [[f, s] for (f, s) in d_n]}, open(CACHE / "p2-diff.json", "w"), ensure_ascii=False, indent=0)
    # P3
    same_bin = base["binary"] == now["binary"]
    print("P3a binario (--licencias y errores de uso):", "OK" if same_bin else "DIFIERE")
    if not same_bin:
        ok = False
        for k in base["binary"]:
            if base["binary"][k] != now["binary"].get(k): print("  ", k, base["binary"][k], now["binary"].get(k))
    g_b = collections.Counter(base["golden"].values())
    g_n = collections.Counter(now["golden"].values())
    # los nombres, salvo el mapa
    # el README de golden/aseprite/ lista los nombres de caso: cambia con ellos (documentación, no corpus)
    readme = "golden/aseprite/README.md"
    names_b = {mapped_path(f): h for f, h in base["golden"].items() if f != readme}
    names_n = {f: h for f, h in now["golden"].items() if f != readme}
    same_gold = names_b == names_n
    print(f"P3c golden: {len(now['golden'])} ficheros,", "OK" if same_gold else "DIFIERE")
    if not same_gold:
        ok = False
    if "strings" in base and "strings" in now:
        sb, sn = set(base["strings"]), set(now["strings"])
        only_b, only_n = sb - sn, sn - sb
        def mapstr(s):
            for o, n in list(R.ENVS.items()):  # primero las variables de entorno (pueden ir pegadas a otros literales)
                s = s.replace(o, n)
            s = re.sub(r"[A-Za-z_]\w*", lambda m: pairs.get(m.group(0), m.group(0)), s)
            for o, n in _MOVES.items():
                s = s.replace(o, n)
            return s
        mapped = {mapstr(s) for s in only_b}
        unexplained = mapped ^ only_n
        # `strings` parte la sección de solo lectura por bytes no imprimibles: un literal pegado al borde de
        # un bloque puede ganar o perder 1 o 2 caracteres (o dejar 4 a 6 bytes sueltos) sin que cambie ningún literal
        residual = set()
        for s in list(unexplained):
            for u in unexplained:
                if u is not s and u != s and (u.startswith(s) or s.startswith(u)) and abs(len(u) - len(s)) <= 3 and min(len(u), len(s)) > 20:
                    residual.update((s, u))
        junk = {s for s in unexplained if len(s) <= 6}
        unexplained -= residual | junk
        print(f"P3b cadenas del ejecutable release: {len(sb)} antes, {len(sn)} ahora; difieren {len(only_b)}/{len(only_n)}"
              f" (explicadas por el mapa, {len(residual)} de borde de bloque y {len(junk)} bytes sueltos);",
              "OK" if not unexplained else "SIN EXPLICAR")
        for s in sorted(residual | junk): print("    (borde)", repr(s[:100]))
        for s in list(unexplained)[:15]: print("   ", s[:120])
        if unexplained: ok = False
    # P4: por fichero, los tokens de ahora son los de antes salvo los renombrados que dice el mapa (con su ámbito);
    # se compara con una unión de clases (viejo ~ nuevo) construida con el propio mapa para ese fichero
    bad = 0
    reordered = []
    wrapped = []
    for f, v in base["tokens"].items():
        nf = mapped_path(f)
        w = now["tokens"].get(nf)
        if w is None:
            print("P4 falta el fichero", nf); bad += 1; continue
        if nf in MANUAL:
            print("P4 edición a mano declarada:", nf, "-", MANUAL[nf]); continue
        parent = {}
        def find(x):
            while parent.get(x, x) != x:
                parent[x] = parent.get(parent[x], parent[x])
                x = parent[x]
            return x
        def union(a_, b_):
            ra, rb = find(a_), find(b_)
            if ra != rb:
                parent[max(ra, rb)] = min(ra, rb)
        for k, x in v:
            if k != "id":
                continue
            for path_ in (f, nf):
                y = R.id_target(x, path_)
                if y:
                    union(x, y)
            if x in R.MODS:
                union(x, R.MODS[x])
        canon = lambda k, x: (k, find(x) if k == "id" else norm_literal(x) if k == "s" else x)
        mapped_v = [canon(k, x) for k, x in v]
        mapped_w = [canon(k, x) for k, x in w]
        def tidy(seq):
            """Sin las comas finales que rustfmt añade o quita al reajustar líneas."""
            out = []
            for i, (k, x) in enumerate(seq):
                if x == "," and i + 1 < len(seq) and seq[i + 1][1] in (")", "]", "}"):
                    continue
                out.append((k, x))
            return out
        def weak(seq):
            """Además sin `{`, `}` ni `,`: rustfmt envuelve en bloque los brazos largos de `match`."""
            return [(k, x) for k, x in seq if x not in ("{", "}", ",", ";")]
        if mapped_v == mapped_w or tidy(mapped_v) == tidy(mapped_w):
            continue
        if sorted(tidy(mapped_v)) == sorted(tidy(mapped_w)):
            reordered.append(f)  # rustfmt reordena `mod` y `use`
            continue
        if weak(mapped_v) == weak(mapped_w):
            wrapped.append(f)
            continue
        if sorted(weak(mapped_v)) == sorted(weak(mapped_w)):
            reordered.append(f)
            continue
        for (k1, a), (k2, b) in zip(mapped_v, mapped_w):
            if (k1, a) != (k2, b):
                print("P4 difiere:", f, a[:60], "->", b[:60]); break
        bad += 1
    print(f"P4 equivalencia alfa: {len(base['tokens'])} ficheros;", f"{len(reordered)} con `mod`/`use` reordenados y {len(wrapped)} con llaves o comas reajustadas por rustfmt (mismos tokens);", "OK" if not bad else f"{bad} ficheros con diferencias")
    if bad: ok = False
    return ok


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    release = "--release" in sys.argv
    if "--identity" in sys.argv:  # autocomprobación: sin cambios, todo tiene que salir OK
        R.MAP = collections.defaultdict(list)
        R.IDS, R.MODS, R.ENVS, R.CASES, R.REFS, R.FILES = {}, {}, {}, [], {}, {}
        R.move_targets = lambda: []
    CACHE.mkdir(parents=True, exist_ok=True)
    base_file = CACHE / "base.json"
    if cmd == "baseline":
        snap = snapshot(release)
        base_file.write_text(json.dumps(snap), encoding="utf-8")
        print("línea base guardada:", base_file, {k: (len(v) if hasattr(v, "__len__") else v) for k, v in snap.items()})
        return 0
    if cmd == "check":
        base = json.loads(base_file.read_text(encoding="utf-8"))
        now = snapshot(release)
        (CACHE / "now.json").write_text(json.dumps(now), encoding="utf-8")
        return 0 if check(base, now) else 1
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
