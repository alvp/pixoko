"""Resuelve automáticamente las colisiones de un crate con un sufijo numérico y lo anota en overrides-l2-auto.tsv."""
import pathlib, subprocess, sys, collections
import importlib
HERE = pathlib.Path(__file__).resolve().parent
crate = sys.argv[1]
AUTO = HERE / "overrides-l2-auto.tsv"
for rnd in range(8):
    subprocess.check_call([sys.executable, "l2gen.py", "gen"], cwd=HERE, stdout=subprocess.DEVNULL)
    import rename as R, l2collide as C
    importlib.reload(R); importlib.reload(C)
    found = C.find(f"crates/{crate}/")
    if not found:
        print("sin colisiones en", crate); break
    add = {}
    have_plural = {}
    for rel, kind, old, new, why in found:
        scope = rel if kind in ("fn", "dup") else f"crates/{crate}/"
        add[(old, scope)] = new
        if kind == "dup" and (old, scope) not in have_plural:
            have_plural[(old, scope)] = True
    lines = AUTO.read_text(encoding="utf-8").splitlines() if AUTO.exists() else []
    have = {tuple(l.split("\t")[::2]) if False else (l.split("\t")[0], l.split("\t")[2]): l.split("\t")[1] for l in lines if l.strip() and not l.startswith("#")}
    for (old, scope), new in add.items():
        cur = have.get((old, scope), new)
        # siguiente sufijo libre
        pascal = new[:1].isupper() and not new.isupper() and "_" not in new
        sep = "" if pascal else "_"
        if cur == new:
            alt = new + sep + "2"
        else:
            import re as _re
            m = _re.fullmatch(rf"(.*?){sep}(\d+)", cur)
            alt = f"{m.group(1)}{sep}{int(m.group(2)) + 1}" if m else cur + sep + "2"
        have[(old, scope)] = alt
    AUTO.write_text("# old\talt\tscope (generado por l2resolve.py: colisiones resueltas con sufijo)\n" + "\n".join(f"{o}\t{a}\t{s}" for (o, s), a in sorted(have.items())) + "\n", encoding="utf-8")
    print("ronda", rnd, len(found), "colisiones")
