# Pixoko: pixel art en Rust, licencia MIT

## Contexto: qué leer y qué no
- Resumen del plan (hitos, crates, decisiones vigentes): `docs/PLAN-RESUMEN.md`. No leas `docs/PLAN.md` al empezar una unidad.
- Specs: lee `specs/INDEX.md` y SOLO las specs que la unidad cite expresamente.

## Reglas permanentes
- Rust estable. `unsafe` prohibido salvo en px-render, con comentario.
- Proyecto MIT: nada de código GPL, LGPL ni AGPL.
- Nunca copies ni traduzcas código de Grafx2, LibreSprite, Aseprite ni DPaint.
  Para ver su comportamiento, ejecuta el programa y guarda la salida en golden/.
  `.claude/hooks/guard.sh` bloquea las búsquedas por nombre de esos programas: no se esquiva.
- Toda dependencia nueva pasa `cargo deny check` y se anota en docs/DEPENDENCIES.md.
- No toques px-core ni px-raster sin mi aprobación expresa.
- No hagas capturas de pantalla del escritorio (ni de la ventana ni de nada que se vea en pantalla).
  La prueba manual la hago yo.
- Las compilaciones desechables y las copias de prueba NO van en `/tmp` (es un tmpfs, memoria): usa `CARGO_TARGET_DIR=~/.cache/pixoko-scratch` o una carpeta en el disco, y bórrala al terminar.

## Unidad de trabajo (una función o un panel; commits pequeños)
1. **Spec primero**, en `specs/` y con su fila en `specs/INDEX.md`:
   máximo 12 filas de casos y 4 propiedades, y la lista de decisiones que debo confirmar.
   Espera mi visto bueno.
2. **Pruebas en rojo**, escritas por el subagente `test-writer` antes del código.
   Mientras tanto, no escribas código.
3. **Lógica** mínima que haga pasar las pruebas; después, `px-app`.
4. Si una prueba anterior necesita más que cambiar cifras: párate y avísame.
5. **Commits separados, en este orden:** spec; adaptación de pruebas anteriores
   (el mensaje explica qué comprobaba cada una); pruebas; lógica; px-app.
6. **Cierre:** `cargo fmt`, `cargo clippy --workspace -- -D warnings`,
   `cargo test --workspace --locked` y `cargo deny check`; después, el `license-auditor`
   una sola vez, sobre el rango de commits de la unidad.
7. **Prueba manual:** la hago yo. Al terminar me das una lista de pasos.

## Flujo de comprobaciones (salida compacta)
- `cargo test`: siempre con `-q` y recortado, p. ej. `cargo test -q --workspace --locked 2>&1 | tail -40`.
- `clippy` y `cargo deny`: solo errores y avisos nuevos; no pegues el log entero.
- No pegues listas de tests ni logs completos; si algo falla, solo el test y el error.
- Durante la unidad: `cargo check -p` y `cargo test -p -q` solo de los crates tocados. Workspace completo, clippy `-D warnings` y `cargo deny`, solo al cerrar.
- Los commits de pruebas en rojo no tienen que compilar el workspace entero.
- Las propiedades de proptest usan los casos que fije la spec; se puede bajar con `PROPTEST_CASES` al desarrollar, no al cerrar.
- Los tests nuevos de un crate se añaden como módulos de un ejecutable de pruebas existente si es razonable (menos enlazados).
- Tras `/clear`, la sesión se reanuda leyendo la spec de la unidad y `git log -6`.
- Informe final, solo esto: commits, qué cambia respecto a la spec, riesgos y la lista de prueba manual.
