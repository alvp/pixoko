# golden/aseprite

Corpus del hito 3 (`specs/corpus-hito-3.md`): capas, opacidad, modos de mezcla, celdas con
desplazamiento y con recorte, capas ocultas, paleta y canal alfa, en 25 casos.

- **Programa:** Aseprite 1.3.18.6-dev
- **Fecha:** 2026-10-21
- **Cómo se generó:** `tools/corpus-h3/generar.sh` (MIT, en este repositorio): un script de Lua
  propio crea cada caso desde cero y guarda el `.aseprite`; el PNG aplanado sale de abrir ese
  fichero y exportarlo con el modo por lotes del programa.
- **Origen de las imágenes:** son dibujos propios de prueba (rectángulos, discos y diagonales con
  colores definidos en el script). No se copió ningún recurso del programa: ni la paleta por
  defecto, ni imágenes, ni ficheros de ejemplo. Se publican bajo la licencia MIT del repositorio.
- **Hash:** cada `X.png` lleva al lado `X.png.sha256`: SHA-256 de la imagen expandida a RGBA de
  8 bits (alfa directo, sin cabecera), calculado con Pillow, como en `golden/png/`.

| Fichero | PNG aplanado |
| --- | --- |
| capas-alfa-fondo.aseprite | capas-alfa-fondo.png |
| capas-alfa-origen.aseprite | capas-alfa-origen.png |
| capas-normal-tres.aseprite | capas-normal-tres.png |
| capas-ocultas.aseprite | capas-ocultas.png |
| capas-opacidad.aseprite | capas-opacidad.png |
| celda-desplazada.aseprite | celda-desplazada.png |
| celda-recortada.aseprite | celda-recortada.png |
| celda-vacia.aseprite | celda-vacia.png |
| indexado-capas.aseprite | indexado-capas.png |
| indexado-celdas.aseprite | indexado-celdas.png |
| indexado-paleta-256.aseprite | indexado-paleta-256.png |
| indexado-transparente-7.aseprite | indexado-transparente-7.png |
| mezcla-aclarar.aseprite | mezcla-aclarar.png |
| mezcla-diferencia.aseprite | mezcla-diferencia.png |
| mezcla-multiplicar.aseprite | mezcla-multiplicar.png |
| mezcla-normal.aseprite | mezcla-normal.png |
| mezcla-ocho-alfa.aseprite | mezcla-ocho-alfa.png |
| mezcla-ocho-fondo-translucido.aseprite | mezcla-ocho-fondo-translucido.png |
| mezcla-ocho-opacidad.aseprite | mezcla-ocho-opacidad.png |
| mezcla-oscurecer.aseprite | mezcla-oscurecer.png |
| mezcla-sumar.aseprite | mezcla-sumar.png |
| mezcla-superponer.aseprite | mezcla-superponer.png |
| mezcla-trama.aseprite | mezcla-trama.png |
| mixto-rgba.aseprite | mixto-rgba.png |
| rgba-una-capa.aseprite | rgba-una-capa.png |
