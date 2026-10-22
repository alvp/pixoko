"""Aplica las filas de l3ids.tsv de un crate: las escribe en map-l2.tsv y las pasa a map-l2-applied.tsv al terminar.
    l3ids.py prepare px-raster   escribe map-l2.tsv
    l3ids.py done                mueve map-l2.tsv a map-l2-applied.tsv y lo vacía
"""
import pathlib, sys
HERE = pathlib.Path(__file__).resolve().parent
cmd = sys.argv[1]
if cmd == "prepare":
    crate = sys.argv[2]
    rows = []
    for l in (HERE / "l3ids.tsv").read_text(encoding="utf-8").splitlines():
        if l.startswith("#") or not l.strip():
            continue
        f = l.split("\t")
        if f[0] == crate:
            rows.append(f"id\t{f[1]}\t{f[2]}\t{f[3] if len(f) > 3 else f'crates/{crate}/'}")
    (HERE / "map-l2.tsv").write_text("# kind\told\tnew\tscope (l3ids.py)\n" + "\n".join(rows) + "\n", encoding="utf-8")
    print(len(rows), "filas")
else:
    cur = [l for l in (HERE / "map-l2.tsv").read_text(encoding="utf-8").splitlines() if l.strip() and not l.startswith("#")]
    with open(HERE / "map-l2-applied.tsv", "a", encoding="utf-8") as f:
        f.write("\n".join(cur) + "\n")
    (HERE / "map-l2.tsv").write_text("# kind\told\tnew\tscope (vacío: todo lo anterior está aplicado)\n", encoding="utf-8")
