# Hito 4 · Unidad 4.6: operaciones de paleta

Estado: **aprobada de antemano** (2026-10-27: decisiones delegadas al implementador; solo los cambios aditivos de `px-core` de la sección «px-core» están autorizados). Referencias: [`hito-2-paleta-editar.md`](hito-2-paleta-editar.md) (comandos y pasos de paleta), [`hito-2-paleta-operaciones.md`](hito-2-paleta-operaciones.md) (panel, rango con `Mayús`, quita con destino) y D13 del mapa (formatos de paleta de texto sencillos). `G2` (Grafx2) es solo referencia manual de la lista de 4.14; no hay dorados.

## Objetivo

Que la paleta de una imagen indexada se pueda copiar, intercambiar, degradar, recortar por varias entradas a la vez, restaurar a la de por defecto, cargar y guardar de fichero, y que un color se pueda cambiar o intercambiar en toda la imagen (todos los modos). Cada operación es **un paso** de deshacer; reordenar no cambia la imagen.

## px-core (aditivo, autorizado)

Dos comandos nuevos, un paso nuevo por comando (`PaletteStep::Colors` y `PaletteStep::RemoveSet`, con el mismo mecanismo de reserva previa, `growth_on_*` y coste de la historia que `Grow` y `Shrink`) y una variante nueva de `PaletteEditError` (`BadSelection`). Ninguna firma pública existente cambia.

| Comando | Qué hace |
| --- | --- |
| `SetPaletteRange { image, start: u8, colors: Vec<Rgba> }` | Escribe `colors` en las entradas `start..start + n`. Si pasan del final, la paleta crece (con esas entradas, hasta 256). Los píxeles y el índice transparente no cambian. `start > len`: `IndexOutOfRange`; `start + n > 256`: `RangeOutOfPalette`; vacío o sin cambios: `Ok(false)`. |
| `RemovePaletteEntries { image, indices: Vec<u8>, into: u8 }` | Quita varias entradas (`indices` estrictamente creciente, todas `< len`, si no `BadSelection` o `IndexOutOfRange`). Los píxeles que las usan pasan a `into` (índice antes de quitar, fuera de `indices`; si no, `RemoveIntoSelf`); las demás bajan lo que toque. Transparente dentro del conjunto: se quita; fuera: baja con las demás. Quitarlas todas: `LastEntry`. Deshacer guarda una máscara por celda afectada (como `Shrink`) y los colores quitados. |

Ambos solo como comando suelto (`NotBatchable` en un `Batch`, como el resto de comandos de paleta). Nada más de `px-core` ni de `px-raster` cambia.

## Piezas de px-ui y px-app

- **Selección** (`palette_ops.rs`): `Mayús`+clic fija el rango (ya existe) y `Ctrl`+clic **conmuta** una entrada suelta (nueva: `extras`). Entrada seleccionada = (en el rango XOR en `extras`) o la activa; la activa no se puede desmarcar. Un clic normal, las teclas 1 a 9 y las operaciones que cambian la paleta borran rango y extras; rotar los conserva (rota el rango, como hoy). El anillo del rango se pinta en todas las seleccionadas.
- **Funciones puras** (nuevo `palette_tools.rs`, sin egui): `selected_entries`, `gradient_colors(a, b, n)`, `default_palette()`, `swap_order(len, a, b)`, `replace_color_images(doc, from, to)` y `swap_colors_images(doc, a, b)` (calculan las imágenes nuevas; el comando es `ReplaceImages`, un paso, ya existente).
- **Formatos** (nuevo `palette_file.rs`, sin egui ni dependencias): lee y escribe `.pal` (JASC-PAL), `.gpl` (texto de paleta de GIMP) y `.hex` (un `RRGGBB` por línea). Salida con alfa 255; solo se escribe el RGB y el índice transparente no se guarda. Al leer: hasta 256 colores (los de sobra se ignoran y se avisa) y mínimo uno.
- **Menú** `Imagen → Paleta` (submenú, once acciones, sin teclas por defecto, editables, con ayuda): `Copiar entradas`, `Pegar entradas`, `Intercambiar entradas`, `Degradado`, `Borrar entradas`, `Paleta por defecto`, `Cargar paleta…`, `Cargar paleta en la activa…`, `Guardar paleta…`, `Cambiar color en la imagen`, `Intercambiar colores en la imagen`.

## Comportamiento

1. **Copiar** guarda en la sesión los colores (RGBA guardados) de la selección, en orden de índice; no toca el documento. **Pegar** los escribe con `SetPaletteRange` desde la entrada activa (crece si no caben, hasta 256; si pasan de 256 se recortan y se avisa).
2. **Intercambiar entradas** necesita exactamente dos seleccionadas: `PermutePalette` con la transposición (la imagen resuelta no cambia). Otro número: aviso, sin paso.
3. **Degradado**: entre la primera y la última entrada seleccionada (necesita al menos 3 posiciones entre los extremos, es decir `max − min ≥ 2`); las de en medio, seleccionadas o no, reciben la interpolación lineal por canal en enteros, `(a·(n−i) + b·i + n/2) / n` con `n = max − min` (redondeo a la entrada más cercana, mitades hacia arriba), alfa incluido. `SetPaletteRange`. Los extremos no cambian.
4. **Borrar entradas**: `RemovePaletteEntries` con la selección. Destino por defecto: el color más cercano de entre las no seleccionadas (`Palette::nearest` sobre el resto); el flujo de destino pendiente de la 11d (`Confirmar`, `Cancelar`, `Esc`, anillo `RING_DEST`) se reutiliza tal cual; sin píxeles que las usen se aplica directo. Con una sola seleccionada es el `Remove` de siempre. Quitarlas todas: aviso.
5. **Paleta por defecto**: escribe `default_palette()[0..len]` en las entradas existentes con `SetPaletteRange` (la longitud no cambia). La paleta por defecto es la de 256 entradas: 0 a 15 los 16 colores clásicos de texto (negro, azul, verde, cian, rojo, magenta, marrón, gris claro, gris oscuro, y las ocho versiones vivas), 16 a 231 el cubo 6×6×6 (niveles 0, 51, 102, 153, 204, 255; `16 + 36·r + 6·g + b`) y 232 a 255 24 grises (`8 + 10·k`).
6. **Cargar paleta…**: abre el diálogo de archivo, lee, y escribe los colores desde la entrada 0 (`Cargar en la activa…`: desde la activa) con `SetPaletteRange`; la paleta crece si hace falta, **nunca encoge** y los píxeles no cambian de índice. **Guardar paleta…** escribe la paleta entera con el formato de la extensión (`.pal` por defecto). Ninguna toca la ruta del documento ni su estado de «modificado» salvo el paso de deshacer de cargar.
7. **Cambiar color en la imagen**: sustituye en **todas las celdas con imagen** el color resuelto del primer plano por el del fondo; **Intercambiar colores en la imagen** los intercambia. Indexado: se reasignan los índices que resuelven a ese color (si el color es el mismo, nada); RGBA, gris y gris con alfa: coincidencia exacta de píxel. Con ambos colores iguales: aviso, sin paso. Un `ReplaceImages`.
8. Todos los avisos en español caben en 43 columnas; ninguna operación funciona con un gesto en curso ni con un modal abierto (como el resto del menú). En modos no indexados solo están activas las dos últimas.

## Casos (12) y propiedades (4)

| N.º | Caso |
| --- | --- |
| PC-01 | `SetPaletteRange` dentro de la paleta: cambian los colores y no los píxeles; deshacer y rehacer exactos |
| PC-02 | `SetPaletteRange` que crece (`start + n > len`): la paleta crece; deshacer vuelve a la longitud de antes |
| PC-03 | `SetPaletteRange`: `start > len`, más de 256, y vacío o sin cambios (`Ok(false)`, sin paso) |
| PC-04 | `RemovePaletteEntries {1,3}` en 5 entradas con `into 0`: píxeles `[0,1,2,3,4] → [0,0,1,0,2]`; deshacer los devuelve |
| PC-05 | `RemovePaletteEntries`: el transparente dentro del conjunto desaparece y, fuera, baja; con varias capas, todas las celdas |
| PC-06 | `RemovePaletteEntries`: `BadSelection` (vacío, desordenado, repetido), `IndexOutOfRange`, `RemoveIntoSelf` y `LastEntry` |
| PC-07 | `Ctrl`/`Mayús`: `selected_entries` (rango, extras, activa siempre dentro, conmutar dos veces vuelve al estado) |
| PC-08 | `gradient_colors` de (0,0,0,255) a (255,255,255,255) en 5 posiciones: 0, 64, 128, 191, 255; y con alfa |
| PC-09 | `default_palette`: longitud 256 y entradas 16, 21, 196, 231, 232 y 255 |
| PC-10 | Formatos: leer JASC, GPL y hex; escribir y leer da lo mismo; errores (cabecera, número, vacío) y más de 256 |
| PC-11 | `replace_color_images` y `swap_colors_images` en indexado y RGBA, en dos capas; sin celdas vacías materializadas |
| PC-12 | Sesión: `Intercambiar entradas` deja la imagen resuelta igual, y cada operación se deshace de un paso |

Propiedades (`proptest`, 500 casos): **(A)** `SetPaletteRange` aplicar y deshacer es la identidad (imagen, paleta, `history_bytes`); **(B)** `RemovePaletteEntries` nunca deja un píxel `>= len` ni un transparente fuera, y deshacer es exacto; **(C)** reordenar con la transposición no cambia el color resuelto de ningún píxel; **(D)** escribir y leer cualquier paleta en cada formato la devuelve igual (salvo el alfa, que se fija en 255).

## Decisiones tomadas (delegadas)

1. Cargar y pegar **nunca encogen** la paleta (encoger es del asa y de `Tamaño de la paleta…`).
2. Formatos: `.pal` (JASC), `.gpl` y `.hex`; sin dependencias nuevas. `.aco`, `.ase` y PNG de paleta quedan fuera.
3. Cambiar e intercambiar un color en la imagen usan los **dos colores** (primer plano y fondo), sin diálogo propio.
4. Borrar varias admite entradas no contiguas (`Ctrl`) en un solo comando.
5. Paleta por defecto: la de 256 entradas de arriba, aplicada sobre las existentes sin cambiar la longitud.
6. Las once acciones van en el submenú `Imagen → Paleta` y no tienen tecla por defecto.

## Fuera de alcance

Ordenar por tono o luminancia, fusionar duplicados, rangos con cuentas (4.7), paleta por fotograma (hito 5), importar paleta desde una imagen (usa el cuantizador).
