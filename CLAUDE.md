# Pixoko: pixel art en Rust, licencia MIT
- Rust estable. `unsafe` prohibido salvo en px-render, con comentario.
- Antes de codificar una función: página en specs/ y tests; después, el código.
- Proyecto MIT: nada de código GPL, LGPL ni AGPL.
- Nunca copies ni traduzcas código de Grafx2, LibreSprite, Aseprite ni DPaint.
  Para ver su comportamiento, ejecuta el programa y guarda la salida en golden/.
- Toda dependencia nueva pasa `cargo deny check` y se anota en docs/DEPENDENCIES.md.
- Antes de cada commit: cargo fmt, cargo clippy -- -D warnings, cargo test --workspace.
- Una unidad de trabajo es una función o un panel; commits pequeños.
- No hagas capturas de pantalla del escritorio (ni de la ventana ni de nada que se vea en pantalla).
  La comprobación visual la hace el usuario; tú describes cómo probarlo a mano.
