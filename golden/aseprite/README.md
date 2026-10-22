# golden/aseprite

Corpus del hito 3 (`specs/corpus-hito-3.md`): capas, opacidad, modos de mezcla, celdas con
desplazamiento y con recorte, capas ocultas, paleta y canal alfa, en 29 casos.

- **Programa:** Aseprite 1.3.18.6-dev
- **Fecha:** 2026-10-22
- **Cómo se generó:** `tools/corpus-h3/generate.sh` (MIT, en este repositorio): un script de Lua
  propio crea cada caso desde cero y guarda el `.aseprite`; el PNG aplanado sale de abrir ese
  fichero y exportarlo con el modo por lotes del programa.
- **Origen de las imágenes:** son dibujos propios de prueba (rectángulos, discos y diagonales con
  colores definidos en el script). No se copió ningún recurso del programa: ni la paleta por
  defecto, ni imágenes, ni ficheros de ejemplo. Se publican bajo la licencia MIT del repositorio.
- **Hash:** cada `X.png` lleva al lado `X.png.sha256`: SHA-256 de la imagen expandida a RGBA de
  8 bits (alfa directo, sin cabecera), calculado con Pillow, como en `golden/png/`.

| Fichero | PNG aplanado |
| --- | --- |
| layer-locked.aseprite | layer-locked.png |
| layers-alpha-background.aseprite | layers-alpha-background.png |
| layers-alpha-source.aseprite | layers-alpha-source.png |
| layers-normal-three.aseprite | layers-normal-three.png |
| layers-hidden.aseprite | layers-hidden.png |
| layers-opacity.aseprite | layers-opacity.png |
| cel-shifted.aseprite | cel-shifted.png |
| cel-clipped.aseprite | cel-clipped.png |
| cel-empty.aseprite | cel-empty.png |
| background-with-opacity.aseprite | background-with-opacity.png |
| indexed-layers.aseprite | indexed-layers.png |
| indexed-cels.aseprite | indexed-cels.png |
| indexed-palette-256.aseprite | indexed-palette-256.png |
| indexed-transparent-7.aseprite | indexed-transparent-7.png |
| blend-lighten.aseprite | blend-lighten.png |
| blend-alpha-grid-multiply.aseprite | blend-alpha-grid-multiply.png |
| blend-alpha-grid-overlay.aseprite | blend-alpha-grid-overlay.png |
| blend-difference.aseprite | blend-difference.png |
| blend-multiply.aseprite | blend-multiply.png |
| blend-normal.aseprite | blend-normal.png |
| blend-eight-alpha.aseprite | blend-eight-alpha.png |
| blend-eight-background-translucent.aseprite | blend-eight-background-translucent.png |
| blend-eight-opacity.aseprite | blend-eight-opacity.png |
| blend-darken.aseprite | blend-darken.png |
| blend-add.aseprite | blend-add.png |
| blend-overlay.aseprite | blend-overlay.png |
| blend-pattern.aseprite | blend-pattern.png |
| mixed-rgba.aseprite | mixed-rgba.png |
| rgba-one-layer.aseprite | rgba-one-layer.png |
