# Hito 4 · Unidad 4.5: relleno ampliado y relleno por filas

Estado: **aprobada** (decisiones delegadas por el usuario; se confirman las de «Decisiones» tal como están, ver «Visto bueno») y **hecha, a falta de la prueba manual y de la referencia de Aseprite** (2026-10-27: ver «Medida final» y «Corrección posterior»). Cambios previstos: `px-raster` (funciones **aditivas** nuevas y la reescritura interna de `fill.rs`), `px-core` (**solo lo listado abajo**: una función `pub(crate)` nueva en `Image` y la reescritura de dos funciones privadas de `document.rs`; **ninguna firma pública cambia**), `px-ui` y `px-app`. Sin dependencias nuevas.

Referencias: fila 4.5 y D22 de [`mapa-funciones.md`](mapa-funciones.md) (T11, T12); [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md) («Rellenos»: el relleno por filas, aplazado en la 14c); [`hito-2-limites-relleno.md`](hito-2-limites-relleno.md) (los topes, que **no cambian**); [`hito-4-formas.md`](hito-4-formas.md) (patrón de una herramienta con filas en la ventana de opciones y `Batch`).

## Objetivo

1. **El relleno sigue siendo UNA herramienta** (botón `Relleno`, sin botones nuevos: la caja tiene 6 herramientas y 10 botones). Sus variantes se eligen con una fila nueva, `Modo`, de su ventana de opciones: **Sólido** (hoy), **Patrón** y **Hasta el fondo**.
2. **Reescritura por filas** del relleno (D22): acceso por filas (`Image::row`) y comparación de bytes en `fill_command` (`extend` y el escaneo de vecinos) y en `Document::apply` de un `FillSpans` (`scan_runs`, `write_run`, que también usan `undo` y `redo`), en vez de `get_pixel`/`set_pixel` por píxel. **Mismos resultados** que el relleno actual (corpus de la puerta 2.22 y suite de 2.5/14c sin tocar) y **mismos topes** de 14c.

## Lo comprobado (en el código)

1. `fill.rs` ya trabaja por tramos con un mapa de bits; lo caro es el acceso: `matches` llama a `get_pixel` (offset, comprobación de límites, construcción de `Pixel`) en cada píxel, y `scan_runs`/`write_run` de `px-core` lo hacen tres veces por píxel de la región (contar, anotar, escribir) y otra vez en `undo` y `redo`. `Image::row(y)` ya existe (unidad 12).
2. `ToolState` ya recibe el píxel de la otra ranura (`set_other_pixel`, 4.4) y comparte el `Brush` con el lápiz, la línea y las formas. Las filas de la ventana de opciones son genéricas (`OptionRow`, `option_rows`, `click_option`) y `px-app` no distingue herramientas.
3. `Brush::offsets()` da la máscara relativa al centro `(⌊n/2⌋, ⌊n/2⌋)` dentro de una caja de `n × n` (las barras, `n × 1` y `1 × n`, dentro de la misma caja): sirve de baldosa sin código nuevo de formas.
4. **Medida de partida** (release, Linux, 4 hilos, una pasada, ruido ±15 %), 8192², relleno del centro: uniforme RGBA `fill_command` 355 ms · `apply` 937 · `undo` 285 · `redo` 287 · total **1954 ms**; uniforme indexado 216 · 720 · 223 · 223 · total **1406 ms**; disperso RGBA (un `B` cada 5 en x e y; 2 692 874 tramos, el peor caso que acepta el tope) 426 · 1020 · 290 · 282 · total **2457 ms**. Pruebas `row_14_*` y `unit_4_5_scattered_rgba_8192` de `px-raster/tests/fill_memory.rs` (`#[ignore]`). Las cifras de después van en «Medida final» (las anota el cierre).

## Decisiones (⚠ = decido algo que no me diste; las confirmo abajo)

**D1. Modo del relleno.** `FillMode { Solid, Pattern, UntilBackground }` es estado de `ToolState` (de la sesión, como `ShapeKind`; por defecto `Solid`). Fila `Modo` (`OptionRow::FillMode`, textos `Modo: Sólido`, `Modo: Patrón`, `Modo: Hasta el fondo` en `fill_es.rs`); clic izquierdo, siguiente (cicla); derecho, anterior. Con `Patrón`, la ventana añade `Forma` y `Tamaño` (el pincel compartido, como en las demás herramientas); con los otros dos, solo `Modo`. ⚠ **Sin teclas nuevas**: el original usa `i`+clic para «hasta el fondo»; aquí es una fila (la tabla de atajos no cambia). `+` y `-` cambian el tamaño también con el relleno en `Patrón` (y solo entonces).
**D2. «Hasta el fondo» (T11).** La región es el conjunto de píxeles conectados (4 vecinos) a la semilla que **no valen el color de fondo**, comparados por valor almacenado como el relleno actual; el color de fondo es el de **la otra ranura** (el que `px-app` pasa con `set_other_pixel`: con el botón izquierdo, el fondo; con el derecho, el primer plano). Si la semilla vale el fondo, no hay nada que rellenar (`None`). Si la otra ranura no tiene color en el modo (`set_other_pixel(None)`), se comporta como `Sólido`. El color de relleno puede ser igual al de fondo (se calcula la región antes). Los topes de 14c valen igual (tramos de la región, `FillTooFragmented`).
**D3. Patrón (T12).** La baldosa es la máscara del pincel en su caja de `n × n` (apartado 3 de «Lo comprobado»); el píxel `(x, y)` del lienzo se pinta si la máscara tiene `((x mod n), (y mod n))`, es decir, **alineada al origen del lienzo («la página»)**, no a la semilla ni a la región: dos rellenos contiguos casan. Se pinta con el color de la herramienta y **los píxeles fuera de la máscara no se tocan**. La región es la del relleno sólido (mismo color que la semilla, 4 vecinos) y el resultado un solo `FillSpans`. ⚠ Con `n = 1` (o cualquier máscara llena) es idéntico al relleno sólido. ⚠ Patrón de dos colores (`Ambos`) queda fuera: sería otro `Batch` y no hay una petición.
**D4. Tope del patrón.** El patrón parte los tramos de la región en trozos: si salen más de `Document::MAX_FILL_SPANS` (la **misma** constante), `Err(FillTooFragmented { limit })` antes de reservar la salida (se cuenta al producirlos y se aborta al llegar al `MAX + 1`); así lo que pasa `fill_pattern_command` pasa `apply`. Ninguna constante ni error nuevos.
**D5. Celda vacía.** Como el relleno actual (`fill_in`, 3.3): la celda vacía es todo el lienzo del valor transparente; no se materializa antes de aplicar. `Hasta el fondo` en celda vacía: si el fondo vale el transparente, `None`; si no, todo el lienzo. `Patrón` en celda vacía: los tramos de fila completa pasan por `pattern_spans`.
**D6. Reescritura por filas, `px-raster`.** `region_spans` compara los **bytes** del píxel (la semilla se codifica una vez a `[u8; 4]` y su longitud) con trozos de la fila (`chunks_exact`): `extend` avanza por la fila con `take_while` en vez de `get_pixel`, y el escaneo de las filas vecinas lee la fila una vez por tramo. Misma pila, mismo mapa de visitados, mismo orden de comprobaciones y de errores, mismos tramos. Una sola implementación sirve a los dos modos de región (`Same(bytes)` y `NotEqual(bytes)`).
**D7. Reescritura por filas, `px-core` (la única petición sobre `px-core`).** (a) `Image::fill_run(&mut self, y, x, len, pixel) -> bool`, **`pub(crate)`** (no es API pública), que escribe una racha con `chunks_exact_mut`; (b) `write_run` la usa; (c) `scan_runs` compara los bytes de cada tramo con `Image::row` y solo construye el `Pixel` «anterior» al abrir un trozo. El orden de errores (`BadSpans` antes que `Span`, límites), el recuento de trozos, el tope de `apply`, el coste en bytes del paso y `undo`/`redo` **no cambian**. Si alguna prueba de `px-core` necesitara más que esto, parar.
**D8. Medida.** Se repiten las tres medidas de «Lo comprobado» tras la reescritura y se anotan en «Medida final». Objetivo: **no peor** en ningún caso y claramente más rápido en `apply`, `undo` y `redo` (sin umbral duro en las pruebas: el ruido es ±15 % y la máquina de pruebas cambia).

## Cambios exactos en `px-raster` (todos aditivos; el visto bueno vale solo para esta lista)

- **R-09** `fill_until_command(doc, image, x, y, pixel, boundary: Pixel)` y `fill_until_command_with_limit(.., max_spans)`: como `fill_command`, con la región de D2. Orden de comprobaciones: `UnknownImage`, `Pixel` (de `pixel`; el de `boundary` se comprueba igual, con el mismo error, `pixel: 1`), semilla fuera (`None`), semilla igual al fondo (`None`) y después el tope.
- **R-10** `PatternTile`: `PatternTile::from_brush(brush: &Brush) -> PatternTile` (lado `n`, máscara de `n²` bits) y `PatternTile::contains(&self, x: u32, y: u32) -> bool` (alineada al origen, D3).
- **R-11** `pattern_spans(region: &[Span], tile: &PatternTile, max_spans: usize) -> Result<Vec<Span>, DocumentError>` (D3 y D4: ordenados, sin solapes, sin tramos vacíos, contenidos en la región) y `fill_pattern_command(doc, image, x, y, pixel, tile) -> Result<Option<Command>, DocumentError>` (región sólida y `pattern_spans` con `Document::MAX_FILL_SPANS`; `None` si la región no cambia o el patrón no toca ningún píxel de ella).
- Nada no aditivo en la API pública: `fill_command`, `fill_command_with_limit` y `flood_fill` conservan firma y resultados.

## Casos (11 filas; 1 a 7 en `px-raster`, 8 en `px-core`, 9 y 10 en `px-ui`, 11 en `px-app`)

Imágenes de 12 de lado o menos salvo que se diga. `A`, `B` valores distintos, `C` el de relleno.

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Mismos tramos que antes.** `fill_command` sobre imágenes fijas (uniforme, peine 5×3, tablero, anillo, espiral, banda de 1 px, imagen de 1 px, tramo contra el borde derecho) en los cuatro modos, contra un oráculo ingenuo (cola, `get_pixel`, 4 vecinos) escrito en la prueba | Los mismos tramos, ordenados, fusionados y sin solapes; los casos `None` (fuera, igual) iguales |
| 2 | **Hasta el fondo en una fila.** `[1,2,3,B,5,6]` con semilla en 2 y fondo `B`; semilla en `B`; semilla en 5 | Tramo `0..3`; `None`; tramo `4..6`; el relleno puede ser del mismo color que el fondo |
| 3 | **Hasta el fondo en dos dimensiones.** Anillo de `B` de 1 píxel con degradado dentro y fuera; anillo con un hueco en diagonal; anillo con un hueco recto | Dentro del anillo cerrado no se sale (tampoco por la diagonal, 4 vecinos); con el hueco recto sale al exterior; el interior se rellena entero, valga lo que valga cada píxel salvo `B`; en indexado compara el índice |
| 4 | **Topes de 14c con las dos regiones.** `fill_until_command_with_limit` y `fill_command_with_limit` con `L` 7 y 6 sobre el peine 5×3 | `Ok` con 7 tramos; `Err(FillTooFragmented { limit: 6 })`; el tope va el último |
| 5 | **La baldosa.** `from_brush` de `Round 3`, `Square 2`, `Plus 3`, `Horizontal 3`, `Cross 3`, tamaño 1 y 100 | Máscaras escritas a mano (ASCII) de `n × n`; tamaño 1 = todo pintado; `contains` tiene periodo `n` en los dos ejes |
| 6 | **Alineada a la página.** Región de 7×5 en `(3, 2)` con baldosa de diagonal 2×2 (`Slash 2`) y otra con `Plus 3`; la misma baldosa sobre dos regiones contiguas | Cada píxel pintado cumple `contains(x, y)` con sus coordenadas del lienzo (no relativas a la región); las dos regiones contiguas forman el patrón continuo; ningún píxel fuera de la región |
| 7 | **`fill_pattern_command`.** Imagen 12×12 con dos zonas; clic en cada una con `Round 3`; patrón sin ningún píxel en la región (región de 1 px fuera de la máscara); todos ya del color `C`; y el mismo relleno con tope pequeño vía `pattern_spans` con `max` 5 y 4 | El `FillSpans` pinta solo los píxeles de la máscara dentro de la región; los otros quedan como estaban; `None` en los dos casos vacíos; `Err(FillTooFragmented { limit: 4 })` con 5 trozos y `Ok` con tope 5 |
| 8 | **`apply` por filas (`px-core`).** `FillSpans` sobre imágenes de 3 valores alternos en los cuatro modos, con tramos que tocan el borde izquierdo y derecho, de 1 píxel y de fila entera; `undo` y `redo` | Los trozos son las series máximas de cada tramo (`history_bytes` = 20 × trozos), los píxeles ya iguales no cuentan, `undo` devuelve los bytes originales y `redo` los rellenados; un `FillSpans` fuera del lienzo o desordenado da el mismo error que antes |
| 9 | **Herramienta y filas (`px-ui`).** Filas del relleno en cada modo; textos; clics (izquierdo, derecho, `Mayús`); `+`/`-` solo en `Patrón`; el modo no se reinicia al cambiar de herramienta | `Sólido`: `[FillMode]`; `Patrón`: `[FillMode, Shape, Size]`; `Hasta el fondo`: `[FillMode]`; cicla en los dos sentidos; el tamaño cambia solo en `Patrón`; `Dropper` y `Eraser` siguen sin filas |
| 10 | **Gesto del relleno (`px-ui`).** Un clic en cada modo sobre RGBA, indexado y celda vacía; sin color en la otra ranura; peine 16384×368 en `Patrón` y en `Hasta el fondo` | Un paso de deshacer por clic; `Hasta el fondo` usa la otra ranura (con `None`, como `Sólido`); la celda vacía no se materializa antes de aplicar; el peine da `Err(FillTooFragmented { .. })` con documento e historia iguales y `busy()` falso tras soltar |
| 11 | **Sesión (`px-app`).** Relleno en `Patrón` con el botón izquierdo y con el derecho, y `Hasta el fondo` con los colores de las dos ranuras; `Ctrl+Z`; modo indexado | El patrón usa el color del botón pulsado; el fondo es la otra ranura; un paso por clic y `modified`; deshacer deja el documento inicial |

## Propiedades (proptest; 256 casos, 500 como máximo)

1. **Relleno sólido = oráculo ingenuo.** Imágenes de 1 a 12 de lado en los cuatro modos con 2 a 3 valores, semilla y `C` arbitrarios (también fuera): `fill_command` da los mismos tramos que el recorrido ingenuo de `get_pixel` (o `None` en los mismos casos).
2. **Hasta el fondo.** Mismas imágenes: `fill_until_command` da los tramos del recorrido ingenuo con la prueba «distinto del fondo»; ningún píxel del fondo está en la región; si la semilla no es el fondo, la contiene; con tope `L` el resultado es `Err` si y solo si hay más de `L` tramos.
3. **`apply` por filas.** `FillSpans` arbitrarios válidos (anchura de 1 a 40, varios tramos por fila, también anchos de fila entera): la imagen tras `apply` es la del modelo ingenuo (`set_pixel` por píxel), `undo` restaura los bytes, `redo` los vuelve a poner, y `history_bytes` es `20 × trozos` del modelo.
4. **Patrón.** Regiones arbitrarias (conjuntos de tramos válidos) y pinceles arbitrarios (diez formas, tamaño 1 a 12): `pattern_spans` es exactamente el conjunto `{ (x, y) ∈ región : contains(x, y) }`, ordenado, sin solapes ni tramos vacíos; con tope `L` da `Err` si y solo si tiene más de `L` tramos.

## Pruebas nuevas

11 filas y 4 propiedades: 15 como mínimo, por debajo del tope orientativo de 40. Reparto: `px-raster/tests/fill.rs` (módulo nuevo `unit_4_5`: filas 1 a 7 y propiedades 1, 2 y 4), `px-core/tests/fill_spans.rs` (fila 8 y propiedad 3), `px-ui/tests/fill_modes.rs` (9 y 10) y `px-app/src/dispatch/tests/` (11). Más la prueba de tiempo `unit_4_5_scattered_rgba_8192` (`#[ignore]`) en `px-raster/tests/fill_memory.rs`. La sesión principal las escribe y commitea antes del código (modo ahorro); sin `test-writer`.

## Pruebas anteriores que cambian (por lectura; el compilador y la suite dan los números)

**A. Solo cifras, listas y literales:** las que dicen que el `Relleno` no tiene filas en su ventana (`brush_tools.rs`, «las filas de la ventana por herramienta»; el reparto de `toolbar.rs`; `options_row_count(Tool::Fill)` pasa de 0 a 1), el texto de ayuda de `tool.fill` si una prueba fija su longitud, y los `match` sobre `OptionRow`. **B. Ninguna de intención.** Las pruebas de 2.5, 14c y la puerta 2.22 **no se tocan**: son el criterio de «mismos resultados» (se ejecutan enteras al cerrar). Si alguna necesitara más que cifras, literales o firmas: parar y avisar (CLAUDE.md, regla 4).

## Orden de trabajo

Commits separados: (1) esta spec con su fila del `INDEX`; (2) adaptación de pruebas anteriores (el mensaje explica qué comprobaba cada una); (3) pruebas, en rojo; (4) lógica de `px-raster`; (5) `px-core` (reescritura de `scan_runs` y `write_run`); (6) `px-ui`; (7) `px-app`; (8) medida final y cierre. Cierre: `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; `license-auditor` una sola vez sobre el rango (hay cambios en `px-core` y `px-raster`).

## Prueba manual (la haces tú, sin capturas)

1. `Relleno` (`f`): la caja sigue con los mismos botones; clic derecho en el botón abre la ventana con una fila, `Modo: Sólido`.
2. Clic en la fila: `Modo: Patrón`; aparecen `Forma` y `Tamaño`. Con `Redondo` y tamaño 3, rellenar una zona grande de un color: puntos con periodo 3 alineados a la esquina del lienzo; rellenar la zona vecina del mismo color continúa el patrón sin saltos.
3. `+` y `-` cambian el tamaño con el patrón activo; con `Sólido` no.
4. `Forma: Diagonal /` y tamaño 4 sobre una zona: rayas.
5. `Modo: Hasta el fondo`: dibuja un recuadro del color de fondo (botón derecho en la paleta) con degradado dentro y rellena de dentro: llena todo el interior sin salirse; un hueco en el recuadro deja salir el relleno por él.
6. Botón derecho en el lienzo con `Hasta el fondo`: el color de relleno es el de la otra ranura y el de fondo el primer plano.
7. Cada clic es un solo paso de `Ctrl+Z`.
8. Capa con celda vacía: los tres modos funcionan y `Hasta el fondo` rellena todo salvo que el fondo sea transparente.
9. Lienzo de 8192² de un color: rellenar tarda claramente menos de 2 s y `Ctrl+Z`/`Ctrl+Y` son notablemente más rápidos que antes.
10. Y, por mi parte (ver «Corrección posterior»): `tools/corpus-h4/generate-fill.sh` para tener la referencia de Aseprite del relleno sólido; el patrón y «hasta el fondo» son decisión propia (D3).

## Fuera de alcance

Patrón de dos colores, patrón desde la selección (4.8), degradado (4.7), tolerancia de color y 8 vecinos, relleno en hilo aparte, tecla propia para cada modo, cambio de los topes de 14c, cualquier otro cambio en `px-core` o `px-raster`.

## Decisiones que necesito que confirmes

1. **D1:** una herramienta, fila `Modo` con tres valores, sin teclas nuevas; `+`/`-` solo con `Patrón`.
2. **D2:** «hasta el fondo» usa el color de la otra ranura; con semilla igual al fondo o sin otra ranura, nada o sólido.
3. **D3 y D4:** patrón = máscara del pincel alineada al lienzo, pinta solo la máscara; tope de la misma constante.
4. **D5:** celda vacía como hoy.
5. **D6 a D8:** reescritura por filas en `px-raster` y en `px-core` (funciones privadas y una `pub(crate)`), resultados idénticos, medida antes y después.
6. **R-09 a R-11 y D7:** tu aprobación expresa para tocar `px-raster` (aditivo) y `px-core` (solo D7), dada de antemano.

## Visto bueno (2026-10-27; el usuario delegó en mi criterio)

Confirmadas tal como están las seis decisiones del final. Aprobación expresa (de antemano) de `px-raster` solo para R-09 a R-11 y la reescritura interna de `fill.rs`, y de `px-core` solo para D7 (interna, sin API pública nueva); si hiciera falta algo más, parar y decirlo. Modo ahorro de `CLAUDE.md`: una sola corrección posterior de esta spec, al final; informe final de 20 líneas como máximo.

## Medida final (release, Linux, 4 hilos, una pasada, ruido ±15 %; mismas pruebas que la medida de partida)

| Caso a 8192² | `fill_command` | `apply` | `undo` | `redo` | Total | Antes |
| --- | --- | --- | --- | --- | --- | --- |
| uniforme RGBA | 148 ms | 503 | 108 | 113 | **970 ms** | 1954 ms |
| uniforme indexado | 64 ms | 339 | 6 | 6 | **442 ms** | 1406 ms |
| disperso RGBA (2 692 874 tramos) | 238 ms | 617 | 124 | 128 | **1711 ms** | 2457 ms |

El relleno entero baja a la mitad (RGBA) y a un tercio (indexado); `undo` y `redo` caen a un tercio (RGBA) y a casi nada (indexado, `memset` por fila); el disperso, cuyo coste es sobre todo de `apply` (un `get_pixel` por trozo y la reserva), mejora un 30 %. `VmHWM` crece lo mismo que antes (+7 MiB). Las pruebas de 14c (filas 1 a 15, `#[ignore]` incluidas las dos de memoria y tiempo) siguen igual.

## Corrección posterior (al cerrar la unidad; la única)

- **Sin cambios en `px-app`.** La otra ranura ya viaja por `set_other_pixel` desde la 4.4 y las filas de la ventana son genéricas: el commit de `px-app` previsto no existe; la fila 11 son dos pruebas del despachador.
- **Dos ajustes de rendimiento tras la primera medida** (en el mismo commit de `px-raster` y el siguiente): el escaneo de las filas vecinas salta con `next_zero` los píxeles ya visitados y `Rule::holds` compara un byte directo cuando el píxel ocupa uno. Sin ellos el indexado salía un 14 % peor en `fill_command` (216 → 246 ms).
- **`PatternTile`** guarda `Vec<bool>` de `n²` casillas (tope 10 000); `Visited::contains` desaparece (el escaneo usa `next_zero`).
- **Texto de ayuda de `tool.fill`** ampliado con los modos (la ayuda breve, la primera frase, no cambia).
- **Referencia de Aseprite (añadida, no pedida por la spec):** el relleno sólido se compara con el cubo de Aseprite (contiguo, tolerancia 0) en 6 casos de 16 × 16 (`golden/aseprite-fill/cases.txt`, único origen de los casos para el script y la prueba): `uniform`, `ring`, `diagonal` (4 vecinos), `stripes`, `gap` e `islands`. `tools/corpus-h4/fill.lua` y `generate-fill.sh` los generan en local (sin ejecutar: marcas «comprobar» donde la API puede discrepar); `px-golden/tests/fill_reference.rs` comprueba que el fichero de casos es válido (activa) y compara píxel a píxel (`#[ignore]` hasta tener los PNG). El patrón y «hasta el fondo» no tienen equivalente en Aseprite: decisión propia (D3).
- **Comentarios.** `fill.rs` (tocado a fondo) se traduce al inglés en esta unidad; en `document.rs`, `image.rs` y `tool.rs` solo se tocan unas líneas y su traducción queda para L3b.
- **Pruebas nuevas:** 4 propiedades y 11 filas (7 de `px-raster` en un solo módulo, 2 de `px-core`, 4 de `px-ui`, 2 de `px-app`), más la de tiempo `#[ignore]` y 2 de `px-golden`. **Anteriores adaptadas:** 2 (`toolbar.rs` y `brush_tools.rs`, solo una cifra y la lista de herramientas sin filas).
- **Cierre.** `fmt`, `clippy --workspace --all-targets --locked -D warnings` y `cargo test --workspace --locked` (4 492 pruebas, 0 fallos) pasan en el contenedor; `cargo deny` no está instalado aquí (sin dependencias nuevas: `Cargo.toml`, `Cargo.lock` y `deny.toml` intactos). `license-auditor` ejecutado una vez sobre el rango: sin hallazgos.
