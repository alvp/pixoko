# tools/rename

Herramientas del renombrado a inglés (`specs/limpieza-1.md`). Nada de esto se ejecuta en el CI.

- `glossary.tsv`: diccionario palabra a palabra, español → inglés (versionado).
- `gen_map.py`: genera `map.tsv` con el glosario (`python3 gen_map.py`).
- `map.tsv`: el mapa `viejo → nuevo` (`id`, `mod`, `file`, `ref`, `env`, `case`). **Sirve también para buscar
  los nombres viejos que citan las specs anteriores.**
- `reviewed-collisions.tsv`: colisiones de nombre revisadas a mano (el nombre nuevo ya existe en el fichero
  pero no puede ocultar nada).
- `rename.py dry|apply [--crate crates/px-ui] [--kinds id,mod,file,env,case,ref]`: aplica el mapa con un
  léxico de Rust (`rslex.py`) y mueve ficheros con `git mv`; `regions.py` separa producción y pruebas.
- `vocab.py`: vocabulario de los identificadores de producción (para revisar el glosario).
- `verify.py baseline|check [--release]`: comprobaciones P1 a P4 y P3b de la spec; la línea base se toma antes
  de aplicar y vive en `~/.cache/pixoko-l1/`.

Etapa L2 (identificadores de pruebas): `glossary-l2.tsv` (palabras), `overrides-l2.tsv` (traducciones manuales),
`overrides-l2-auto.tsv` (colisiones resueltas), `map-l2-applied.tsv` (viejo → nuevo por crate, ya aplicado);
`l2gen.py`, `l2collide.py`, `l2resolve.py`, `l2batch.py`, `l2finish.py`, `l2review.py`, `l2left.py` y `l2nonen.py`
son las herramientas (se usaron en ese orden, un crate cada vez).
