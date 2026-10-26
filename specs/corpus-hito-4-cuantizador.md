# Corpus del cuantizador (unidad 4.2, fila 12)

Qué es: 6 imágenes RGBA propias y, de cada una, 6 versiones indexadas hechas por el cuantizador de Aseprite (16 y 64 colores × tramado ninguno, de difusión y ordenado). Con ellas la prueba `px-golden/tests/quantize.rs` comprueba el criterio de calidad de [`hito-4-cuantizador.md`](hito-4-cuantizador.md) (D8): el error medio por canal de Pixoko es como mucho el de la referencia por 1,10 más medio nivel. Aseprite es la única referencia automática de estos hitos; los datos de la API de Lua están **sin comprobar** (el script no se ha ejecutado nunca).

## Los 6 casos (todos creados desde cero por `tools/corpus-h4/quantize.lua`, con enteros)

| Caso | Tamaño | Contenido |
| --- | --- | --- |
| `degradado` | 64 × 64 | R = 4x, G = 4y, B = 2(x + y); 4096 colores |
| `anillos` | 64 × 64 | colores periódicos en la distancia al centro |
| `manchas` | 60 × 20 | tres manchas planas (rojo, verde, azul) con una variación de hasta 4 niveles |
| `alfa` | 64 × 16 | colores con alfa creciente de 0 a 252 (prueba el umbral) |
| `grises` | 64 × 16 | rampa de gris de 64 pasos |
| `mixto` | 64 × 64 | un disco, diagonales y un fondo con textura |

Ficheros resultantes en `golden/aseprite-quant/`: `<caso>.src.png` (original) y `<caso>-n<16|64>-<none|floyd|ordered>.png` (resultado indexado): 6 + 36 = 42 ficheros.

## Pasos exactos para quien tiene Aseprite (no los puedo hacer yo)

1. En la raíz del repositorio: `tools/corpus-h4/generate.sh --prueba`. Debe imprimir la versión de Aseprite y «modo por lotes: ok». Si el programa no se llama `aseprite`, `ASEPRITE=/ruta/al/programa tools/corpus-h4/generate.sh --prueba`.
2. `tools/corpus-h4/generate.sh`. Escribe los 42 ficheros en `golden/aseprite-quant/` (usa `~/.cache` como carpeta de prueba, no `/tmp`). Dice `NUEVO` por cada uno; al repetirlo, `IGUAL` o `DISTINTO`.
3. Si falla, el fallo estará en las líneas marcadas con «comprobar» de `tools/corpus-h4/quantize.lua` (nombres de `app.command.ColorQuantization` y `app.command.ChangePixelFormat`, y de la imagen de la celda). Se corrigen en la función `cuantiza` (o en `crea`) y se vuelve a lanzar; **pásame el mensaje de error exacto** si no sale.
4. Revisar a ojo 2 o 3 resultados (que sean indexados y se parezcan al original) y commitear `golden/aseprite-quant/`.
5. Quitar el `#[ignore]` de `row_12_…` en `crates/px-golden/tests/quantize.rs` y ejecutar `cargo test -p px-golden --test quantize`. Con `-- --nocapture` imprime, por caso, el error propio, el de la referencia y el límite.
6. Si algún caso supera el límite, **no se toca el margen**: se anota el caso, el error propio y el de la referencia, y se decide en una unidad aparte (la spec fija el margen una vez).

## Qué no cubre

La paleta ajena, el umbral de alfa y la exactitud cuando los colores caben se prueban sin referencia (`px-raster/tests/quantize.rs`); aquí solo cuenta el error medio frente a Aseprite.
