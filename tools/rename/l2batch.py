#!/usr/bin/env python3
"""Traducción manual por lotes de los nombres largos (frases) de la etapa L2.

    l2batch.py list <crate> <desde> <n>    imprime `número<TAB>nombre` (los nombres de ≥5 palabras que aún no tienen traducción)
    l2batch.py put <fichero>               lee líneas `número<TAB>nuevo` del último `list` y las añade a overrides-l2.tsv
"""
import json, pathlib, re, sys
import l2gen

HERE = pathlib.Path(__file__).resolve().parent
OV = HERE / "overrides-l2.tsv"
STATE = pathlib.Path.home() / ".cache" / "pixoko-l1" / "batch.json"
KW = {"as","break","const","continue","crate","else","enum","extern","false","fn","for","if","impl","in","let","loop","match","mod","move","mut","pub","ref","return","self","Self","static","struct","super","trait","true","type","unsafe","use","where","while","async","await","dyn"}


def nw(n):
    return sum(len(ws) for ws in l2gen.words_of(n))


def load_ov():
    d = {}
    if OV.exists():
        for line in OV.read_text(encoding="utf-8").splitlines():
            if line.strip() and not line.startswith("#"):
                a, b = line.split("\t")[:2]
                d[a] = b
    return d


def main():
    cmd = sys.argv[1]
    if cmd == "list":
        crate, start, n = sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
        uses = l2gen.collect()
        ov = load_ov()
        names = sorted(o for o in uses if l2gen.is_spanish(o) and nw(o) >= 4 and o not in ov and any(c.startswith(f"crates/{crate}/") for c in uses[o]))
        batch = names[start : start + n]
        STATE.write_text(json.dumps(batch), encoding="utf-8")
        print(f"# {len(names)} pendientes en {crate}; lote {start}..{start + len(batch)}")
        for i, name in enumerate(batch, 1):
            print(f"{i}\t{name}")
    elif cmd == "put":
        batch = json.loads(STATE.read_text(encoding="utf-8"))
        ov = load_ov()
        out = []
        bad = 0
        for line in pathlib.Path(sys.argv[2]).read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            k, new = line.split("\t")
            old = batch[int(k) - 1]
            if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", new) or new in KW or new.endswith("_") or "__" in new:
                print("INVÁLIDO", k, old, "->", new); bad += 1; continue
            out.append(f"{old}\t{new}")
        if bad:
            sys.exit(1)
        missing = [i + 1 for i in range(len(batch)) if not any(l.startswith(batch[i] + "\t") for l in out)]
        if missing:
            print("faltan los números:", missing); sys.exit(1)
        with OV.open("a", encoding="utf-8") as f:
            f.write("\n".join(out) + "\n")
        print(len(out), "añadidas")


main()
