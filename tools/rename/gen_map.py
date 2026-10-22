"""Genera map.tsv a partir de glossary.tsv y de las listas de nombres de esta etapa (L1).

Cada nombre nuevo sale de traducir palabra a palabra con el glosario; `OVERRIDES` recoge las
decisiones que no salen así. Se ejecuta a mano y su salida se versiona (`map.tsv`).
"""
import pathlib, re

HERE = pathlib.Path(__file__).resolve().parent
GLOSS = {}
for line in (HERE / "glossary.tsv").read_text(encoding="utf-8").splitlines():
    if line.startswith("#") or not line.strip():
        continue
    a, b, *_ = line.split("\t")
    GLOSS[a] = b


def tr(name: str, sep: str) -> str:
    out = []
    for w in name.split(sep):
        if w.isdigit() or w not in GLOSS:
            out.append(w)  # ya es inglés, número o nombre propio
        else:
            out.append(GLOSS[w])
    return sep.join(out)


OVERRIDES = {
    "sonda-combinar": "merge-probe",
    "panel_baja": "low_panel",  # «ventana baja»: el panel de una ventana de poca altura
}

ids = [  # (viejo, ámbito)
    ("capas", "crates/px-app/src/merge_flow.rs"),
    ("atajo", "crates/px-ui/src/keymap_file.rs"),
    ("por_defecto", "crates/px-ui/src/shortcuts.rs"),
    ("si", "crates/px-ui/src/gestures.rs"),
]
mods = [
    "puerta", "capas", "comun", "filas", "grande", "guiones", "modelo", "mundo", "propiedades",
    "damero_guardado", "ayuda_tests", "teclado_tests", "escritor", "aseprite_puerta", "gate_oraculo",
    "caja", "iconos", "texturas", "zonas",
]
# ficheros que no son módulos (binarios de prueba y scripts): (ruta vieja)
files = [
    "crates/px-app/tests/arranque.rs",
    "crates/px-golden/tests/mezcla.rs",
    "crates/px-ui/tests/navegacion.rs",
    "crates/px-ui/tests/panel_baja.rs",
    "tools/corpus-h3/comprobar.lua",
    "tools/corpus-h3/generar.lua",
    "tools/corpus-h3/generar.sh",
    "tools/corpus-h3/sonda-combinar.lua",
    "tools/corpus-h3/verificar.lua",
]
env = {
    "PIXOKO_SIN_MINIMO": "PIXOKO_NO_MIN_SIZE",
    "PIXOKO_DAMERO_MALLA": "PIXOKO_CHECKER_MESH",
    "PIXOKO_DIALOGO_PRUEBA": "PIXOKO_TEST_DIALOG",
}
cases = """
capa-bloqueada
capas-alfa-fondo
capas-alfa-origen
capas-normal-tres
capas-ocultas
capas-opacidad
celda-desplazada
celda-recortada
celda-vacia
fondo-con-opacidad
indexado-capas
indexado-celdas
indexado-paleta-256
indexado-transparente-7
mezcla-aclarar
mezcla-alfa-rejilla-multiplicar
mezcla-alfa-rejilla-superponer
mezcla-diferencia
mezcla-multiplicar
mezcla-normal
mezcla-ocho-alfa
mezcla-ocho-fondo-translucido
mezcla-ocho-opacidad
mezcla-oscurecer
mezcla-sumar
mezcla-superponer
mezcla-trama
mixto-rgba
rgba-una-capa
""".split()  # los 29 casos de golden/aseprite/ antes del renombrado

rows = []
for old, scope in ids:
    rows.append(("id", old, tr(old, "_"), scope))
for old in mods:
    rows.append(("mod", old, tr(old, "_"), "*"))
for old in files:
    p = pathlib.PurePosixPath(old)
    stem = p.stem
    new_stem = OVERRIDES.get(stem) or (tr(stem, "-") if "-" in stem else tr(stem, "_"))
    rows.append(("file", old, str(p.with_name(new_stem + p.suffix)), "-"))
    if p.suffix in (".lua", ".sh"):
        rows.append(("ref", p.name, new_stem + p.suffix, "*"))
        if stem not in [r[1] for r in rows if r[0] == "ref"]:
            pass
for old, new in env.items():
    rows.append(("env", old, new, "*"))
for c in sorted(cases):
    rows.append(("case", c, tr(c, "-"), "*"))

with open(HERE / "map.tsv", "w", encoding="utf-8") as f:
    f.write("# kind\told\tnew\tscope  (specs/limpieza-1.md; generado por gen_map.py desde glossary.tsv)\n")
    for r in rows:
        f.write("\t".join(r) + "\n")
print(len(rows), "filas")
