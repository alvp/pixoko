# Hito 2 · Unidad 17b: damero de transparencia en el lienzo

Estado: **pendiente de visto bueno** (2026-10-19). Sin código ni tests escritos todavía; al aprobarla, el `test-writer` escribe y commitea los tests antes de tocar nada en `src/`.

Referencias: D11 de [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md) (unidad corta tras la 17, solo pintado) y la nota «Damero de transparencia» de [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md). Superposición del borrador y su sección «Para el damero»: [`hito-2-cuentagotas-borrador.md`](hito-2-cuentagotas-borrador.md) (decisión 3, fila 10). Muestras con damero del panel de paletas: [`hito-2-panel-paletas.md`](hito-2-panel-paletas.md) (`swatch_parts`, `CHECK_A`, `CHECK_B`). Geometría en píxeles físicos y `n = text_scale(ppp)`: [`hito-2-texto.md`](hito-2-texto.md) y [`hito-2-ventana.md`](hito-2-ventana.md). Fallo del suavizado de bordes con rectángulos de 1 píxel físico: commit `b702d8e` y `px-ui/tests/overlay_small_zoom.rs`.

Cambios: `px-ui` (módulo nuevo `checker`, y `Canvas::paint`). **`px-app` no cambia**: `main.rs` llama a `Canvas::paint(ui, overlay)` con la misma firma (ver decisión 10). **No se toca `px-core` ni `px-raster`**, ni `view.rs`. Sin dependencias nuevas ni `unsafe`; `docs/DEPENDENCIES.md` no cambia.

## Objetivo

Que un píxel transparente (o borrado) se vea como un **damero de dos grises neutros** y no como el fondo liso, dentro del rectángulo de la imagen. El resto del lienzo conserva el fondo liso `BACKGROUND` (gris 40). Es solo visualización: el documento, la textura de la imagen y el PNG guardado no cambian.

## Lo comprobado (y lo que no)

Todo medido en una copia fuera del repositorio (crate de pruebas con `egui` 0.37.0, `--release`, un hilo, sin GPU ni ventana, suavizado de bordes desactivado como en `Canvas::paint`). **Mide la CPU de egui (formas + teselado), no la GPU**; la subida de vértices por fotograma se calcula con 20 B por vértice y 4 B por índice.

- **egui 0.37 admite textura con envoltura.** `TextureOptions::NEAREST_REPEAT` existe (`epaint 0.37.0`, `textures.rs`) y `egui_glow` pasa `wrap_mode` a GL (`painter.rs`). Una textura de 2×2 con uv fuera de [0, 1] se repite. **No se ha probado en una GPU** (ni en macOS ni en Windows): lo comprueba el usuario (prueba manual).
- **Coste del damero de fondo por fotograma**, ventana grande de 3840×2160 píxeles físicos con el lienzo entero cubierto por la imagen (zoom alto), casilla de `c` píxeles físicos:

| Estrategia | `c = 8` (n = 1) | `c = 5` | `c = 16` (n = 2) | `c = 24` (n = 3) | Subida a la GPU |
| --- | --- | --- | --- | --- | --- |
| **A. Un rectángulo por casilla oscura** (`rect_filled`) | 1,61 + 4,92 = **6,5 ms**, 64 803 formas | 4,1 + 15,5 = 19,6 ms, 165 891 formas | 0,4 + 1,2 = 1,6 ms | 0,18 + 0,54 = 0,7 ms | 259 212 vértices + 388 818 índices ≈ **6,7 MB** por fotograma (con `c = 8`) |
| **B. Una sola malla** (`Mesh::add_colored_rect`) | 0,23 + 0,20 = **0,43 ms**, 4 formas | 1,3 ms | 0,11 ms | 0,05 ms | los mismos 6,7 MB |
| **C. Textura 2×2 repetida, un solo cuadrilátero** | **≈ 0 ms**, 1 cuadrilátero (4 vértices) | ≈ 0 | ≈ 0 | ≈ 0 | **ninguna** (la textura se sube una vez) |
| **D. Textura generada del tamaño visible** | 11,1 ms por cambio de vista (generar y entregar a egui) | — | — | — | **33,2 MB** por cambio de vista (2,7 ms y 8,3 MB con 1920×1080) |

  Casillas a 3840×2160: **129 600** con `c = 8` (64 800 oscuras), 331 776 con `c = 5`, 32 400 con `c = 16`. Mallas teseladas: las formas consecutivas con la misma textura y el mismo recorte se funden en una (1000 rectángulos → 1 malla; 1000 cuadriláteros de textura → 1 malla). **Límite de A:** a 4K y `c = 8` ya come el 39 % del fotograma de 16,7 ms, y crece con el cuadrado del lado de la ventana y en inverso al cuadrado de `c`. **Límite de D:** 11 ms y 33 MB en cada fotograma de arrastre de la vista. **C no tiene límite práctico** (coste constante).
- **Coste de la superposición, 50 000 píxeles pendientes** (formas + teselado, mismo banco):

| Qué se pinta por píxel | Mallas | Coste |
| --- | --- | --- |
| **Hoy**, píxel con alfa 0 (borrador): un rectángulo de `BACKGROUND` | 1 | 5,3 a 7,0 ms (tres series de medidas; 10,7 a 12,8 ms con 100 000 píxeles) |
| **Damero recortado al píxel**, alfa 0: un cuadrilátero de la textura con sus uv | 1 | 5,8 a 5,9 ms (11,9 ms con 100 000 píxeles) |
| **Hoy**, alfa parcial (fondo + color): dos rectángulos | 1 | 15,4 ms |
| Damero y color **intercalados** por píxel | **100 000** | 13,0 ms (y 100 000 llamadas de dibujo en la GPU) |
| Damero de **todos** los píxeles y luego todos los colores (**dos pasadas**) | **2** | **9,7 ms** |

  Conclusiones medidas: (1) con alfa 0 el damero **cuesta lo mismo** que el fondo liso de hoy (más formas y menos teselado; la variación entre series es del orden de ±15 %, mayor que la diferencia); (2) con alfa parcial, **intercalar** cambia de textura en cada forma y revienta la malla en una por forma: hay que pintar en **dos pasadas** (primero los cuadriláteros del damero, después los rectángulos de color); los bloques no se solapan, así que el orden entre píxeles no cambia lo que se ve; (3) el retraso medido en las notas de la unidad 14 (50 000 a 100 000 píxeles por trazo) **no empeora**: la 14a sigue siendo quien lo arregla.
- **Precisión de uv.** Con la textura repetida la posición dentro de la casilla se pasa en uv `f32`. Si el cuadrilátero cubriera la imagen entera (a ×64, una imagen de 16 384 píxeles mide más de un millón de píxeles físicos) el uv llegaría a ≈ 65 000 períodos y el error de `f32` (≈ 0,008 período) movería los bordes de casilla. Por eso el cuadrilátero se **recorta al lienzo** (nunca mide más que la ventana) y la fase se calcula con **enteros** (`rem_euclid` en `i64`), así el uv va de [0, 1) a lo sumo a unos cientos de períodos y el error queda por debajo de 0,01 píxel físico.
- **Qué no he comprobado:** nada visual (CLAUDE.md); que la GPU dé exactamente `0x4a` y `0x6e` al muestrear la textura (un nivel de diferencia con las muestras del panel, que van por color de vértice, sería invisible, pero no puedo descartarlo sin pantalla); el comportamiento de GL en macOS y Windows.

## Decisiones

1. **Colores: las mismas constantes del panel de paletas.** `CHECK_A` = `#4a4a4a` y `CHECK_B` = `#6e6e6e` de `px_ui::palette_grid` **se reutilizan, no se duplican** (ni se mueven: los tests de `palette_grid.rs` las importan de ahí). `checker` las lee de allí. `BACKGROUND` conserva su valor (gris 40), es el fondo fuera de la imagen y no es ninguno de los dos grises.
2. **Casilla de 8 u (`CELL_U = 8`), es decir `8·n` píxeles físicos:** 8, 8, 16, 16 y 24 a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0. Un entero de píxeles físicos derivado de `text_scale(ppp)`, como el resto de la maqueta. Es mayor que la del panel (`CHECK_U = 5`, porque allí caben cuatro por muestra de 10 u): 8 u es algo más que el avance de un carácter (7 u) y deja el damero a escala de la ventana y no de una muestra. Con 5 u habría 2,6 veces más casillas (afecta solo a las estrategias A y B, no a la elegida).
3. **Anclado al origen de la imagen, no a la pantalla.** La casilla `(0, 0)` empieza en la esquina superior izquierda de la imagen y es `CHECK_A`, igual que en `swatch_parts`. Motivos: (a) con el damero fijo en pantalla, arrastrar una imagen **toda transparente** (el fondo por defecto de «Nueva imagen», unidad 19) no se vería: nada se mueve; (b) el damero «es del papel»: se desplaza con la imagen y el borde superior izquierdo siempre empieza en casilla entera; (c) es una función pura del origen que ya existe (`View::image_rect`). Coste: al hacer zoom anclado en el cursor el damero «nada» bajo el cursor (se mueve la imagen, y con ella el damero). Los bordes derecho e inferior cortan casilla si la imagen no es múltiplo de `c`; es lo esperado (recorte exacto).
4. **Qué se ve con cada zoom.** La casilla mide `8·n` píxeles físicos **siempre**, con cualquier zoom. **Zoom alto:** un píxel transparente de la imagen cubre `zoom / c` casillas por lado (a ×16 con `c = 8`, 2×2; a ×64, 8×8) y el damero atraviesa el píxel sin romperse en sus bordes. **Zoom bajo:** hoy el mínimo es ×1, donde una casilla cubre 8×8 píxeles de imagen (con `n = 1`); con los zooms 1/2, 1/4 y 1/8 de la 14a una casilla cubrirá 16, 32 y 64 píxeles de imagen por lado y la función no cambia (`cell` es físico, no depende del zoom). **Coincidencia conocida:** con `n = 1` y zoom ×8 (o ×16, ×24…) la casilla coincide con el píxel de imagen y un lienzo transparente parece una imagen a cuadros; el cursor y la barra de estado dicen el color real. No se evita (cualquier tamaño de casilla coincide con algún zoom entero).
5. **Sin bordes grises, y por qué.** (a) El cuadrilátero del damero es el rectángulo de la imagen **recortado al lienzo, en enteros de píxel físico** (los mismos enteros que la malla de la imagen), así no hay hueco entre damero e imagen ni damero fuera de la imagen; (b) es una malla de textura, no un `RectShape`, así que el suavizado de bordes que dejó sin pintar los rectángulos de 1 px físico (`b702d8e`) no le afecta (y `paint` ya lo desactiva); (c) muestreo `NEAREST` sin mipmaps y el centro de cada píxel físico queda a ≥ 0,5 px de cualquier borde de casilla, es decir a ≥ 0,010 en uv con `c` hasta 24, frente a un error de `f32` por debajo de 10⁻⁴. Se prueba a las cinco escalas y con zoom ×1, ×2 y ×3 (fila 5), con el rasterizador a mano de `overlay_small_zoom.rs` ampliado para muestrear la textura 2×2.
6. **Estrategia elegida: C, textura 2×2 con `NEAREST_REPEAT`, un cuadrilátero.** Es la más barata (coste constante, sin subida por fotograma, 4 vértices frente a 259 212 de A y B a 4K) y nítida (5). B queda como **respaldo documentado** si la prueba manual diera un defecto de GL o un nivel de gris distinto (0,43 ms; mismos colores de vértice que las muestras del panel). D se descarta (11 ms y 33 MB por cambio de vista). A se descarta (6,5 ms a 4K y crece). La textura se crea **una vez**, de forma perezosa, en el primer `paint` que la necesita (`Canvas` gana el campo `checker: Option<TextureHandle>`); sobrevive a `set_image`, `refresh` y `preview_entry` y su id no cambia.
7. **La imagen se superpone con alfa** (la malla de la imagen se pinta encima, como hoy). No se toca ninguna lógica del documento. El damero no entra en `to_rgba8`, ni en la textura de la imagen, ni en `write_png`.
8. ⚠ **Superposición del borrador y del lápiz: el damero entra ahora, no en la 14a.** Para cada píxel pendiente con alfa < 255 se pinta el cuadrilátero del damero **recortado a ese píxel** (`checker::quad` del bloque, con el mismo origen y la misma `cell`, así que es idéntico por construcción al damero de fondo bajo ese píxel y al que se ve al soltar), y encima, si el alfa es mayor que 0, el rectángulo de color; **en dos pasadas** (todos los cuadriláteros, luego todos los colores; ver medida). El rectángulo de `BACKGROUND` por píxel desaparece de la superposición. Coste medido a 50 000 píxeles: **equivalente** al de hoy con alfa 0 (≈ 5,9 frente a 5,3 a 7,0 ms) y **menor** con alfa parcial en dos pasadas (9,7 frente a 15,4 ms). **Esto adelanta a la 17b lo que D11 asignaba a la 14a**; la 14a, al pasar la superposición a textura o sombreador, tendrá que resolver «borrar» de nuevo (queda anotado en su lista).
9. **Pruebas anteriores que cambian: ninguna cambia solo de valor y dos necesitan más** (sección propia). Me paro ahí y lo digo: no las toco hasta tu visto bueno.
10. **`px-app` no cambia.** El reparto decía solo `px-ui`; en tu encargo ponías `px-ui` y `px-app`. Leí `main.rs`: llama a `canvas.paint(ui, overlay)` (líneas 552 a 566), `set_image`, `refresh`, `preview_entry` y `end_preview`, y ninguna firma cambia. Si prefieres algo en `px-app` (por ejemplo un caso de prueba de integración), dímelo.
11. **`checker::cells` queda en `src/` como referencia y respaldo de la estrategia B.** La elegida (C) usa solo `quad`; `cells` sirve de oráculo para las propiedades y de base del respaldo. Es código `pub` sin llamada en producción. Alternativa: moverla a `tests/` si prefieres no tener nada sin uso en `src/`.

## Piezas (`px-ui/src/checker.rs`, sin tipos de egui; `pub mod checker` en `lib.rs`)

```rust
pub const CELL_U: i64 = 8;
pub fn cell_px(n: u32) -> i64;                       // CELL_U · n, con n >= 1 (n = 0 se trata como 1)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade { A, B }                              // A = CHECK_A, B = CHECK_B
impl Shade { pub fn color(self) -> Rgba; }           // lee palette_grid::{CHECK_A, CHECK_B}

pub fn shade_at(x: i64, y: i64, origin: (i64, i64), cell: i64) -> Shade;
//  A si floor((x-ox)/cell) + floor((y-oy)/cell) es par, B si es impar; vale con coordenadas
//  negativas (floor) y con cualquier i64 (sin desbordar: se resta en i128 o con rem_euclid).

pub fn texels() -> [Rgba; 4];                        // la textura 2×2 por filas: [A, B, B, A]

pub fn clip(image: ScreenRect, viewport: Size) -> Option<ScreenRect>;
//  image ∩ (0, 0, viewport.width, viewport.height); None si queda vacío.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckerQuad { pub rect: ScreenRect, pub uv: [f32; 4] }   // [u0, v0, u1, v1]
pub fn quad(area: ScreenRect, origin: (i64, i64), cell: i64) -> Option<CheckerQuad>;
//  rect = area. u0 = ((area.x - ox) mod 2·cell) / (2·cell) (rem_euclid, en [0, 1)), u1 = u0 +
//  area.width / (2·cell); igual para v. None si area está vacía (ancho o alto <= 0) o cell < 1.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell { pub rect: ScreenRect, pub shade: Shade }
pub fn cells(area: ScreenRect, origin: (i64, i64), cell: i64) -> impl Iterator<Item = Cell>;
//  Las casillas que cortan `area`, por filas, recortadas a `area`. Vacío si area o cell lo son.
```

`Canvas::paint` (orden de pintado): (1) rectángulo de `BACKGROUND` (como hoy); (2) si hay textura: cuadrilátero del damero de `quad(clip(image_rect, viewport), (placed.x, placed.y), cell_px(text_scale(ppp)))`, con la textura 2×2 de `texels()` y `NEAREST_REPEAT`; (3) la malla de la imagen (como hoy); (4) superposición: primera pasada, un cuadrilátero `quad(bloque, mismo origen, misma cell)` por píxel visible con alfa < 255; segunda pasada, un rectángulo de color por píxel visible con alfa > 0. Sin textura (error `TooLarge` o sin imagen) solo se pinta (1). Aditivo: `Canvas::checker_texture_id() -> Option<TextureId>` para las pruebas, como `texture_id`.

## Casos (8 filas; tests de lógica pura y de banco de egui, sin ventana)

| # | Caso | Resultado esperado |
| --- | --- | --- |
| 1 | **Constantes y tamaño.** `cell_px(n)` con `n` de 1 a 8 y con `n = 0`; `cell_px(text_scale(ppp))` a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0; `Shade::A.color()`, `Shade::B.color()`, `texels()`, `BACKGROUND` | `8·n` (y 8 para `n = 0`); 8, 8, 16, 16 y 24. `A` es `CHECK_A` (`#4a4a4a`) y `B` es `CHECK_B` (`#6e6e6e`), los **mismos valores** que prueba `palette_grid.rs`; `texels() = [A, B, B, A]`; `BACKGROUND` sigue siendo `from_gray(40)` y distinto de los dos |
| 2 | **`shade_at`.** Origen `(0,0)`, `cell` 8: `(0,0)`, `(7,7)`, `(8,0)`, `(0,8)`, `(8,8)`, `(15,15)`, `(16,0)`; con negativos `(-1,0)`, `(-1,-1)`, `(-8,-8)`, `(-9,0)`; origen `(10,20)`: `(10,20)`, `(18,20)`, `(9,20)`; origen `(-2⁴⁰+3, 2⁴⁰)` y puntos a ±2⁴⁰ | `A, A, B, B, A, A, A`; `B, A, A, A`; `A, B, B`; con los i64 enormes no hay desbordamiento ni pánico y coincide con la definición por `rem_euclid` de `(x−ox)` y `(y−oy)` con período `2·cell` |
| 3 | **`quad` y `clip`.** (a) imagen entera visible, `area = (10,20,100,50)`, origen `(10,20)`, `cell` 8; (b) imagen con origen `(-21,-35)` y `area` = la parte visible `(0,0,200,150)`; (c) un píxel de zoom ×1, `area = (37,11,1,1)`, origen `(0,0)`, `cell` 8; (d) `area` de ancho 0, de alto negativo y `cell = 0`; (e) `clip` de `(-50,-40,300,200)` en un lienzo de 100×80, de una imagen fuera del lienzo y de una que lo cubre | (a) `rect = area`, `uv = [0, 0, 6,25, 3,125]`; (b) `uv0 = (21 mod 16)/16 = 5/16`, `v0 = (35 mod 16)/16 = 3/16` y `uv1 = uv0 + 200/16`, `v1 = v0 + 150/16`; (c) `uv0 = (37 mod 16)/16 = 5/16`, `v0 = 11/16`, extensión `1/16`; (d) `None` en los tres; (e) `(0,0,100,80)`, `None`, `(0,0,100,80)` |
| 4 | **`cells`.** (a) `area = (0,0,20,12)`, origen `(0,0)`, `cell` 8; (b) `area = (5,3,20,12)`, mismo origen; (c) `area` menor que una casilla, `(3,3,2,2)`; (d) `area` de 1×1 y vacía; (e) `area = (0,0,3840,2160)`, `cell` 8, solo contando | (a) 6 casillas: anchos 8, 8, 4 y altos 8, 4; tonos `A B A / B A B`; áreas suman 240, sin solapes. (b) 8 casillas: la primera es `(5,3,3,5)` y `A`; columnas de ancho 3, 8, 8 y 1, filas de alto 5 y 7; áreas suman 240. (c) una casilla igual a `area`. (d) una y ninguna. (e) **129 600** casillas, sin reservar la lista entera |
| 5 | **Lienzo: damero sin bordes grises** (banco egui con rasterizador a mano, ampliando el de `overlay_small_zoom.rs`): imagen RGBA 64×48 con **todos los píxeles de alfa 0**; `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 × zoom ×1, ×2 y ×3; además tras arrastrar la vista con el botón central 7 y 13 píxeles físicos | **Cada** píxel físico del rectángulo de la imagen (también la primera y la última fila y columna) se ve **exactamente** `shade_at(px, py, origen_de_la_imagen, cell_px(n))`, es decir `CHECK_A` o `CHECK_B`, ni un mezclado, ni `BACKGROUND`; cada píxel del lienzo fuera de la imagen se ve **exactamente** `BACKGROUND`. Tras desplazar, el color de un píxel físico de la imagen **relativo a su esquina** es el mismo que antes (el damero va con la imagen) |
| 6 | **Capas, textura, coste y documento** (banco egui, formas y teselado): primer fotograma, un `pan`, un `zoom`, `refresh`, `set_image` y `preview_entry`; una imagen de 2048×64 a ×64; un error `TooLarge`; y `write_png` antes y después | Orden: rectángulo de `BACKGROUND`, **cuadrilátero del damero**, malla de la imagen (una sola, con la textura de la imagen). El cuadrilátero: textura distinta de la de la imagen, 4 vértices y 6 índices **siempre**, posición = `clip(image_rect, lienzo)` en puntos, uv = los de `quad`. La textura del damero: 2×2, `NEAREST_REPEAT`, píxeles `texels()`, **una** subida en el primer fotograma y ninguna después (id estable en todo lo demás). Con la imagen de 2048×64 a ×64 el rectángulo de la imagen mide 131 072 píxeles, y el del damero, a lo sumo el lienzo. Con `TooLarge`: ni damero ni textura del damero, solo `BACKGROUND`. `to_rgba8` de la imagen y la textura que sube el lienzo, intactas; `state_id` igual; `write_png` da **los mismos bytes** antes y después |
| 7 | **Superposición** (extiende la fila 10 de la unidad 17a y `overlay_small_zoom.rs`): bloques de píxeles pendientes con `(255,0,0,255)`, `(0,0,0,0)` (borrador) y `(255,0,0,128)` sobre una imagen azul; las cinco escalas × zoom ×1, ×2 y ×3; el trazo del borrador de la fila 10 (20 píxeles, varios fotogramas, `End` y `refresh`) | Por cada píxel con alfa < 255, **un** cuadrilátero del damero con `quad(bloque)`, y ninguno con alfa 255; **no** hay ningún rectángulo de `BACKGROUND` en la superposición; los cuadriláteros van **todos antes** que los rectángulos de color (2 mallas de superposición como mucho). Lo que se ve en cada píxel físico del bloque: el damero (alfa 0), el color con alfa sobre el damero (128) o el color (255). **Antes y después de soltar, igual**: el color de cada píxel físico de los píxeles borrados en el fotograma con superposición es idéntico al del fotograma tras `End` y `refresh` (textura con alfa 0 sobre el damero de fondo): sin incoherencia temporal. A ×1 siguen viéndose los de 1×1 píxel físico (`b702d8e`) |
| 8 | **Límites.** Imagen de 1×1 a ×64; vista arrastrada al extremo (quedan visibles 16 píxeles de imagen, `MIN_VISIBLE`); lienzo de 1×1 píxel físico; sin imagen; texturas máximas (`Some(1024)` con una imagen de 1025 de ancho) | Imagen 1×1: cuadrilátero de 64×64 con uv `[0, 0, 4, 4]` (a `cell` 8 y origen en la esquina). Vista en el extremo: el cuadrilátero tiene solo la franja visible, dentro del lienzo, y su uv parte de la fase correcta (cada píxel se ve como `shade_at`). Lienzo de 1×1: sin pánico, `clip` da `None` o 1×1. Sin imagen o con `TooLarge`: solo el fondo, sin fallo ni textura del damero |

## Propiedades (proptest; 3)

1. **`quad`, `shade_at` y `cells` son la misma función.** Origen en ±2⁴⁰, `cell` de 8 a 64 (múltiplo de 8), `area` de hasta 600×600 en cualquier posición (también negativa). (a) Emulando el muestreo `NEAREST` con envoltura sobre `texels()` en el **centro** de cada píxel físico del área (interpolando los uv `f32` del cuadrilátero, en `f64` para el oráculo), se obtiene `shade_at` de ese píxel. (b) Para cualquier subárea, `quad(subárea)` muestrea lo mismo que `quad(área)` en esos píxeles (el recorte no cambia el color: es lo que hace coherente la superposición). (c) `cells(area)` cubre `area` **exactamente**: sin solapes ni huecos, cada casilla dentro de una sola casilla del damero, tono igual a `shade_at` de cualquiera de sus píxeles y alternado con sus vecinas, y su número es `⌈…⌉` por eje calculado aparte.
2. **Pintar el damero no cambia la conversión pantalla-imagen.** Secuencias aleatorias de rueda, arrastre con el botón central, `+`, `-`, `Home` y movimientos del puntero a las cinco escalas: con `show` (`interact` y `paint`) frente a solo `interact`, `hover`, `zoom` e `image_rect` de `CanvasInfo` y los eventos de trazo son **idénticos**, y `hover` coincide con `floor((puntero − image_rect.min)·ppp / zoom)`. (24 casos.)
3. **Lo que se ve es el oráculo puro.** Imagen de 1 a 24 de lado con píxeles de alfa 0, de 1 a 254 y de 255 al azar, superposición al azar sobre píxeles de la imagen, una de las cinco escalas, zoom ×1, ×2 o ×3 y una vista desplazada al azar: **cada** píxel físico del rectángulo de la imagen se ve igual que `composición(shade_at(…), píxel de la imagen o de la superposición)` y el resto del lienzo, `BACKGROUND`. (24 casos; el rasterizador a mano es lento.)

## Tests de otras unidades que cambian

Revisados leyendo cada prueba que usa `Canvas` o `textures_delta`:

| Prueba | Qué pasa | ¿Solo un valor? |
| --- | --- | --- |
| `eraser_canvas.rs`: `fila_10_cada_fotograma_pinta_un_rectangulo_de_fondo_por_pixel_pendiente` y su ayudante `comprueba_fotograma_con_trazo` (con `rectangulos`) | Lee `Shape::Rect` y exige un rectángulo de `BACKGROUND` por píxel pendiente. Ahora la superposición son **cuadriláteros de textura** (mallas), no rectángulos | **No.** Cambia qué se lee de las formas y la regla (un cuadrilátero del damero por píxel, uv de `quad`), no un número |
| `overlay_small_zoom.rs`: `esperado`, `rasteriza` y los tres tests que los usan | `esperado` parte de `BACKGROUND` para alfa < 255 y ahora debe partir del damero **en ese píxel** (necesita su posición: cambia la firma); `rasteriza` trata toda malla que no es la imagen como color de vértice y ahora debe **muestrear la textura 2×2** del damero | **No.** Cambian dos ayudantes y una firma; los tres tests conservan nombre e intención |
| `eraser_canvas.rs`: `fila_10_tras_el_end_y_el_refresh_…` | Un comentario dice que el fondo bajo esos píxeles es `BACKGROUND`; sus aserciones (`rects == s.base`, `rects.first() == BACKGROUND`) siguen valiendo porque el damero no es un `Shape::Rect` y el primer rectángulo sigue siendo el fondo | Solo el comentario |
| `eraser_canvas.rs`: `el_fondo_del_lienzo_es_el_gris_40` | `BACKGROUND` sigue siendo `pub` y gris 40 | No cambia |
| `canvas.rs`, `pencil.rs`, `canvas_layers.rs`, `window_stack.rs`, `texture_preview.rs`, `tool_canvas.rs` y los tests de `px-app` | Cuentan mallas **de la textura de la imagen** (el damero tiene la suya), comparan `show` con `interact`+`paint` (igual en los dos), leen textos y `Shape::Rect` de la interfaz o miran subidas de la textura de la imagen; la del damero se sube en el primer fotograma, antes de lo que miran `eraser_canvas.rs` (líneas 300, 387, 428 y 455) | No cambian |

**Me paro aquí, como pediste:** las dos primeras filas necesitan más que cambiar un valor. Mi propuesta, si das el visto bueno, es adaptarlas **en su propio commit, antes de la lógica** (quedan en rojo hasta la implementación, como los commits de pruebas anteriores) y dejar `BACKGROUND` y las demás sin tocar.

## Dónde van las pruebas y orden de trabajo

`px-ui/tests/checker.rs` (filas 1 a 4 y propiedad 1) y `px-ui/tests/checker_canvas.rs` (filas 5 a 8 y propiedades 2 y 3; banco copiado de `overlay_small_zoom.rs`, con el rasterizador que muestrea la textura).

Orden, un commit por paso y `cargo fmt`, `cargo clippy -- -D warnings` y `cargo test --workspace` antes de cada uno (los de pruebas en rojo lo dicen en el mensaje):
1. Esta spec, con tu visto bueno.
2. Las dos pruebas anteriores que cambian (tabla de arriba), solas.
3. El `test-writer`: tests nuevos de las filas 1 a 8 y las propiedades 1 a 3, en rojo, **antes de escribir código**.
4. `checker.rs` (filas 1 a 4 y propiedad 1 en verde).
5. `Canvas::paint` y el campo `checker` (todo en verde).

## Prueba manual (la haces tú, sin capturas; yo no hago ninguna)

1. Abrir un PNG RGBA con zonas transparentes: dentro de la imagen se ve un damero de casillas de 8 píxeles (16 con la escala 2); fuera de la imagen, el gris liso de siempre. Compararlo con la muestra transparente del panel de paletas: **los mismos dos grises**.
2. Arrastrar la vista con el botón central: el damero **se mueve con la imagen**. Con una imagen toda transparente también se nota el movimiento.
3. `+` y `-`: la casilla no cambia de tamaño en pantalla; a ×8 con la escala 1 la casilla coincide con un píxel de imagen (esperado). **Ni una línea gris** entre casillas, ni en el borde de la imagen, a ×1, ×2 y ×3 y en cada escala que tengas, en Linux, macOS y Windows (envoltura de la textura en GL).
4. `E` y arrastrar sobre una zona opaca: los píxeles borrados se ven con el damero **mientras se arrastra** y **igual al soltar**, sin parpadeo. Un color con alfa 128 en el editor y el lápiz: se ve el color sobre el damero mientras se arrastra.
5. Ventana grande (4K o a pantalla completa) a zoom alto: el arrastre de la vista y el zoom van como antes. Anotar si no.
6. Guardar y reabrir: el PNG no lleva el damero (los píxeles siguen transparentes).

## Fuera de alcance

Damero o tamaño de casilla configurables, o con otros colores; zooms 1/2, 1/4 y 1/8 y la superposición como textura o sombreador (14a, que tendrá que resolver «borrar» de nuevo); cambiar las muestras del panel de paletas; `px-app`; la exportación (el damero nunca se guarda).

## Decisiones que necesito que confirmes

1. ⚠ **Las dos pruebas anteriores que necesitan más que un valor** (`eraser_canvas.rs` fila 10 y `overlay_small_zoom.rs`): ¿las adapto en un commit propio antes de la lógica, como propongo? (No he tocado ninguna.)
2. ⚠ **El damero de la superposición del borrador entra en esta unidad** y no en la 14a (decisión 8; medido: cuesta lo mismo con alfa 0, y en dos pasadas con alfa parcial cuesta menos). Cambia lo que fijó D11; la 14a lo recoge.
3. **Casilla de 8 u** (8, 8, 16, 16 y 24 px a las cinco escalas) y no 5 u como el panel.
4. **Anclado al origen de la imagen**, no a la pantalla (motivo: el desplazamiento de una imagen transparente se ve).
5. **Textura 2×2 repetida y un cuadrilátero** (estrategia C), con la malla (B) como respaldo si tu prueba manual ve un defecto o un gris distinto.
6. **Las mismas constantes `CHECK_A` y `CHECK_B`** del panel de paletas, sin moverlas ni duplicarlas.
7. **`px-app` sin cambios** (el reparto decía solo `px-ui`; tú escribiste `px-ui` y `px-app`).
8. **`checker::cells` en `src/`** como referencia y respaldo (código `pub` sin llamada en producción), o en `tests/`.
9. **Coincidencia a ×8 con `n = 1`** (casilla = píxel de imagen): aceptada y documentada, sin evitarla.
