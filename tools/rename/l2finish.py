"""Cierra un crate de L2: pasa sus filas de map-l2.tsv (generado antes de aplicar) a map-l2-applied.tsv."""
import pathlib, sys
HERE = pathlib.Path(__file__).resolve().parent
crate = sys.argv[1]
cur = (HERE / "map-l2.tsv").read_text(encoding="utf-8").splitlines()
mine = [l for l in cur if l.split("\t")[-1].startswith(f"crates/{crate}/")]
rest = [l for l in cur if l not in mine]
with (HERE / "map-l2-applied.tsv").open("a", encoding="utf-8") as f:
    f.write("\n".join(mine) + "\n")
(HERE / "map-l2.tsv").write_text("\n".join(rest) + "\n", encoding="utf-8")
with (HERE / "l2-applied-crates.txt").open("a") as f:
    f.write(crate + "\n")
print(len(mine), "filas aplicadas de", crate)
