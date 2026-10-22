#!/usr/bin/env python3
"""Renombrado a inglés (specs/limpieza-1.md, apartado 1 y etapa L1).

    rename.py dry    lista de colisiones (nombres nuevos que ocultarían a otro del mismo fichero) y de cambios
    rename.py apply  aplica el mapa (map.tsv): código, literales de formato, variables de entorno, nombres
                     de caso del corpus y referencias, y mueve los ficheros con `git mv`

Tipos de fila de map.tsv (`kind old new scope`):
  id    identificador, solo en los ficheros del ámbito (rutas separadas por comas)
  mod   nombre de módulo: solo donde hace de módulo (tras `mod`, antes de `::`, dentro de un `use`),
        en los `#[path = "…"]` y en los comentarios entre comillas invertidas junto a `::` o `/`
  file  fichero que se mueve (ruta vieja, ruta nueva)
  ref   nombre de fichero citado en texto (comentarios, cadenas, scripts)
  env   variable de entorno, en literales y comentarios
  case  nombre de caso del corpus, en literales, comentarios, scripts y el README del corpus
"""
from __future__ import annotations

import collections
import pathlib
import re
import subprocess
import sys

from regions import ROOT, rs_files
from rslex import Tok, ident_name, lex

HERE = pathlib.Path(__file__).resolve().parent
FORMAT_MACROS = {
    "format": 0, "print": 0, "println": 0, "eprint": 0, "eprintln": 0, "panic": 0, "unreachable": 0,
    "todo": 0, "unimplemented": 0, "format_args": 0, "write": 1, "writeln": 1, "assert": 1,
    "debug_assert": 1, "assert_eq": 2, "assert_ne": 2, "debug_assert_eq": 2, "debug_assert_ne": 2,
}


def load_map():
    m = collections.defaultdict(list)
    for name in ("map.tsv", "map-l2-applied.tsv", "map-l2.tsv"):
        f = HERE / name
        if not f.exists():
            continue
        for line in f.read_text(encoding="utf-8").splitlines():
            if not line.strip() or line.startswith("#"):
                continue
            kind, old, new, scope = line.split("\t")
            m[kind].append((old, new, scope))
    return m


MAP = load_map()
IDS = {}  # old -> (new, scopes)
for old, new, scope in MAP["id"]:
    IDS.setdefault(old, []).append((new, scope.split(",")))
MODS = {old: new for old, new, _ in MAP["mod"]}
ENVS = {old: new for old, new, _ in MAP["env"]}
CASES = sorted(((o, n) for o, n, _ in MAP["case"]), key=lambda t: -len(t[0]))
REFS = {old: new for old, new, _ in MAP["ref"]}
FILES = {old: new for old, new, _ in MAP["file"]}
# L2: los `id` con ámbito de carpeta se comprueban por función con l2collide.py, no por fichero
L2_IDS = {old for old, _, scope in MAP["id"] if scope.endswith("/")}


def id_target(old: str, rel: str):
    for new, scopes in IDS.get(old, []):
        if "*" in scopes or rel in scopes or any(s.endswith("/") and rel.startswith(s) for s in scopes):
            return new
    return None


def sig_prev(toks, i):
    i -= 1
    while i >= 0 and toks[i].kind in ("ws", "comment"):
        i -= 1
    return i


def sig_next(toks, i):
    i += 1
    while i < len(toks) and toks[i].kind in ("ws", "comment"):
        i += 1
    return i


def rename_format_args(s: str, rel: str):
    """`{viejo}`, `{viejo:?}` y `{:>viejo$}` dentro de un literal de formato."""
    changed = []

    def fix_ident(m):
        n = id_target(m.group(0), rel)
        if n:
            changed.append((m.group(0), n))
            return n
        return m.group(0)

    out, i = "", 0
    while i < len(s):
        if s.startswith("{{", i) or s.startswith("}}", i):
            out += s[i : i + 2]
            i += 2
        elif s[i] == "{":
            j = s.find("}", i)
            if j < 0:
                out += s[i:]
                break
            body = s[i + 1 : j]
            arg, colon, spec = body.partition(":")
            if re.fullmatch(r"[A-Za-z_]\w*", arg):
                arg = fix_ident(re.match(r"[A-Za-z_]\w*", arg))
            spec = re.sub(r"[A-Za-z_]\w*(?=\$)", fix_ident, spec)
            out += "{" + arg + colon + spec + "}"
            i = j + 1
        else:
            out += s[i]
            i += 1
    return out, changed


def text_renames(s: str, in_comment: bool):
    """Variables de entorno, nombres de caso y referencias, en un literal o comentario."""
    for old, new in ENVS.items():
        s = re.sub(r"(?<![A-Za-z0-9_])" + re.escape(old) + r"(?![A-Za-z0-9_])", new, s)
    for old, new in CASES:
        s = re.sub(r"(?<![\w-])" + re.escape(old) + r"(?![\w-])", new, s)
    for old, new in REFS.items():
        s = re.sub(r"(?<![\w-])" + re.escape(old) + r"(?![\w-])", new, s)
    return s


def comment_spans(s: str, rel: str, used_mods=frozenset()):
    """Entre comillas invertidas: identificadores `id` del fichero y módulos junto a `::` o `/`."""
    parts = s.split("`")
    for k in range(1, len(parts), 2):
        span = parts[k]

        def sub(m):
            w = m.group(0)
            n = id_target(w, rel)
            if n:
                return n
            if w in MODS:
                before = span[: m.start()]
                after = span[m.end() :]
                if before.endswith(("::", "/")) or after.startswith(("::", "/", ".rs")) or (span == w and w in used_mods):
                    return MODS[w]
            return w

        parts[k] = re.sub(r"[A-Za-z_]\w*", sub, span)
    return "`".join(parts)


# ficheros que no se renombran aunque lleven palabras del glosario (`ci.rs`: solo sus comentarios, en L3)
PROTECTED = {"crates/px-golden/tests/ci.rs"}


def rename_rs(rel: str, src: str, used_mods=None):
    if rel in PROTECTED:
        return src, collections.Counter()
    if used_mods is None:  # primera pasada: qué módulos renombra el código de este fichero
        _, first = rename_rs(rel, src, frozenset())
        used_mods = frozenset(n for (k, n) in first if k == "mod")
    toks = lex(src)
    out: list[str] = []
    report = collections.Counter()
    in_use = False
    use_parents: list[str] = []  # el segmento que precede a cada `{` de un `use`
    stack: list[tuple[str | None, int]] = []  # (macro de formato o None, comas)
    attr_path = False
    for i, t in enumerate(toks):
        text = t.text
        if t.kind == "ident":
            name = ident_name(t)
            new = id_target(name, rel)
            if new:
                report[("id", name)] += 1
                text = new if not t.text.startswith("r#") else "r#" + new
            elif name in MODS:
                p, n = sig_prev(toks, i), sig_next(toks, i)
                prev_t = toks[p].text if p >= 0 else ""
                next_t = toks[n].text + (toks[n + 1].text if n + 1 < len(toks) else "") if n < len(toks) else ""
                after_dot = prev_t == "."
                # en un `use`, una hoja igual que el segmento anterior (`caja::caja`) es el elemento, no el módulo
                sep = p >= 1 and toks[p].text == ":" and toks[p - 1].text == ":"  # `::` son dos signos
                twin = in_use and sep and toks[sig_prev(toks, p - 1)].text == name
                if in_use and prev_t in ("{", ",") and use_parents and use_parents[-1] == name:
                    twin = True
                if not after_dot and not twin and (prev_t == "mod" or next_t.startswith("::") or in_use):
                    report[("mod", name)] += 1
                    text = MODS[name]
            if name == "use" and t.kind == "ident":
                in_use = True
        elif t.kind == "punct":
            if text == ";" and in_use:
                in_use = False
                use_parents.clear()
            if in_use and text == "{":
                p = sig_prev(toks, i)
                sep = p >= 1 and toks[p].text == ":" and toks[p - 1].text == ":"
                q = sig_prev(toks, p - 1) if sep else -1
                use_parents.append(toks[q].text if q >= 0 else "")
            elif in_use and text == "}" and use_parents:
                use_parents.pop()
            if text in "([{":
                p = sig_prev(toks, i)
                macro = None
                if text in "([" and p >= 1 and toks[p].text == "!" and toks[sig_prev(toks, p)].kind == "ident":
                    mname = toks[sig_prev(toks, p)].text
                    macro = mname  # cualquier macro: los envoltorios de prueba (`requires!`…) reciben formatos
                stack.append((macro, 0))
            elif text in ")]}" and stack:
                stack.pop()
            elif text == "," and stack:
                m, c = stack[-1]
                stack[-1] = (m, c + 1)
        elif t.kind == "string":
            body = text
            # #[path = "…"]
            p = sig_prev(toks, i)
            if p >= 1 and toks[p].text == "=" and toks[sig_prev(toks, p)].text == "path":
                def comp(m):
                    w = m.group(0)
                    return MODS.get(w, w)
                body = re.sub(r"[A-Za-z_]\w*", comp, body)
                if body != text:
                    report[("path", text)] += 1
            # argumentos en línea de las macros de formato
            if stack and stack[-1][0] is not None and (stack[-1][0] not in FORMAT_MACROS or stack[-1][1] == FORMAT_MACROS[stack[-1][0]]):
                new_body, ch = rename_format_args(body, rel)
                if ch:
                    report[("format", text)] += 1
                    body = new_body
            body2 = text_renames(body, False)
            if body2 != body:
                report[("text", text)] += 1
            text = body2
        elif t.kind == "comment":
            c2 = comment_spans(text, rel, used_mods)
            c2 = text_renames(c2, True)
            if c2 != text:
                report[("comment", text[:40])] += 1
            text = c2
        out.append(text)
    return "".join(out), report


def reviewed():
    out = set()
    for line in (HERE / "reviewed-collisions.tsv").read_text(encoding="utf-8").splitlines():
        if line.strip() and not line.startswith("#"):
            f = line.split("\t")
            out.add(tuple(f[:4]))
    return out


def collisions():
    """Nombres nuevos que ya existen como identificador en el mismo fichero (sin los revisados)."""
    found = []
    ok = reviewed()
    for p in rs_files():
        rel = str(p.relative_to(ROOT))
        if not in_crate(rel):
            continue
        src = p.read_text(encoding="utf-8")
        toks = lex(src)
        idents = collections.Counter(ident_name(t) for t in toks if t.kind == "ident")
        _, report = rename_rs(rel, src)
        for (kind, name), n in report.items():
            if kind in ("id", "mod"):
                new = IDS[name][0][0] if kind == "id" else MODS[name]
                if kind == "id":
                    new = id_target(name, rel)
                if kind == "id" and name in L2_IDS:
                    continue
                if idents.get(new) and (rel, kind, name, new) not in ok:
                    found.append((rel, kind, name, new, idents[new]))
    return found


def other_files():
    out = []
    for p in sorted((ROOT / "tools" / "corpus-h3").rglob("*")):
        if p.is_file():
            out.append(p)
    out.append(ROOT / "golden" / "aseprite" / "README.md")
    return out


def tool_text(s: str):
    for old, new in CASES:
        s = re.sub(r"(?<![\w-])" + re.escape(old) + r"(?![\w-])", new, s)
    for old, new in REFS.items():
        s = re.sub(r"(?<![\w-])" + re.escape(old) + r"(?![\w-])", new, s)
    return s


def tracked_files():
    return subprocess.check_output(["git", "ls-files", "crates", "fuzz", "tools", "golden"], cwd=ROOT, text=True).split("\n")


def move_targets(tracked=None):
    """(vieja, nueva) para todo fichero versionado que cambia de nombre."""
    tracked = tracked_files() if tracked is None else tracked
    case_new = dict(CASES)
    moves = []
    for f in tracked:
        if not f:
            continue
        new = FILES.get(f)
        if new is None and f.startswith("golden/aseprite/"):
            name = f.split("/")[-1]
            for old, nw in CASES:
                if name.startswith(old + "."):
                    new = "golden/aseprite/" + nw + name[len(old) :]
                    break
        if new is None and f.startswith("crates/") and not f.endswith("/Cargo.toml"):
            parts = f.split("/")
            changed = False
            for k in range(2, len(parts)):
                stem, dot, ext = parts[k].partition(".")
                if k == len(parts) - 1:
                    if dot and stem in MODS and ext == "rs" and f not in FILES:
                        parts[k] = MODS[stem] + ".rs"
                        changed = True
                elif parts[k] in MODS:
                    parts[k] = MODS[parts[k]]
                    changed = True
            if changed:
                new = "/".join(parts)
        if new and new != f:
            moves.append((f, new))
    return moves


def configure(argv):
    """`--crate crates/px-ui` limita los ficheros; `--kinds id,mod,file,env,case,ref` limita el mapa."""
    global IDS, MODS, ENVS, CASES, REFS, FILES, CRATE
    kinds = None
    CRATE = None
    for i, a in enumerate(argv):
        if a == "--kinds":
            kinds = set(argv[i + 1].split(","))
        if a == "--crate":
            CRATE = argv[i + 1].rstrip("/") + "/"
    if kinds is not None:
        if "id" not in kinds:
            IDS = {}
        if "mod" not in kinds:
            MODS = {}
        if "env" not in kinds:
            ENVS = {}
        if "case" not in kinds:
            CASES = []
        if "ref" not in kinds:
            REFS = {}
        if "file" not in kinds:
            FILES = {}


CRATE = None


def in_crate(rel: str) -> bool:
    return CRATE is None or rel.startswith(CRATE)


def main():
    configure(sys.argv)
    cmd = sys.argv[1] if len(sys.argv) > 1 else "dry"
    if cmd == "dry":
        found = collisions()
        total = collections.Counter()
        for p in rs_files():
            if not in_crate(str(p.relative_to(ROOT))):
                continue
            _, rep = rename_rs(str(p.relative_to(ROOT)), p.read_text(encoding="utf-8"))
            for k, n in rep.items():
                total[k[0]] += n
        print("cambios por clase:", dict(total))
        print("colisiones (el nombre nuevo ya existe en el fichero):", len(found))
        for rel, kind, old, new, n in found:
            print(f"  {rel}: {kind} {old} -> {new} (ya hay {n})")
        print("movimientos:", len([m for m in move_targets() if in_crate(m[0])]))
        return 1 if found else 0
    if cmd == "apply":
        found = collisions()
        if L2_IDS and "id" in (sys.argv[sys.argv.index("--kinds") + 1].split(",") if "--kinds" in sys.argv else ["id"]):
            import l2collide
            found += l2collide.find(CRATE.rstrip("/") + "/" if CRATE else None)
        if found:
            print("hay colisiones sin resolver; ejecuta `rename.py dry`")
            return 1
        for p in rs_files():
            rel = str(p.relative_to(ROOT))
            if not in_crate(rel):
                continue
            src = p.read_text(encoding="utf-8")
            new, rep = rename_rs(rel, src)
            if new != src:
                p.write_text(new, encoding="utf-8")
        for p in other_files():
            if p.exists() and CRATE is None and (CASES or REFS):
                s = p.read_text(encoding="utf-8")
                n = tool_text(s)
                if n != s:
                    p.write_text(n, encoding="utf-8")
        for old, new in move_targets():
            if not in_crate(old):
                continue
            (ROOT / new).parent.mkdir(parents=True, exist_ok=True)
            subprocess.check_call(["git", "mv", old, new], cwd=ROOT)
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
