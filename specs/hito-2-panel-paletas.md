# Hito 2 · Unidad 11b: panel de paletas (ver y elegir colores)

Estado: **aprobada** (2026-10-18; decisiones 1 a 8 como se recomendaban, con estos matices, que **prevalecen sobre el resto de la página**: (1) si los tests de las unidades 6 y 7 no compilan sin cambios con `ColorChoice`, se para y se avisa, sin editarlos; (2) `u` es un número entero de píxeles físicos (`n = text_scale(ppp)`, el múltiplo de la escala del texto de 11a) y un test comprueba que toda la geometría cae en píxeles físicos enteros a 1,0, 1,25, 1,5, 2,0 y 3,0; (3) las medidas viven en constantes en un solo sitio (`palette_grid.rs`) y los anillos contrastan al menos 3:1 con el fondo del panel, con un test; (6) para la entrada transparente el texto muestra el hexadecimal **resuelto** (`#rrggbb00`) y la palabra «transparente»; (7) se actualiza `specs/hito-2-abrir-guardar.md`, los dos tests de `adjust_slot` se sustituyen por uno y hay una fila nueva (guardar y guardar como no reinician); (9) la lista de tests de otras unidades afectados por el formato de la barra está en «Tests de otras unidades»; las teclas 1 a 9 siguen eligiendo las entradas 0 a 8 en indexado, sincronizadas con el panel; (10) ya no se mide ni se anota el coste: no se escribe en `docs/COSTES.md`). Editar la paleta (RGB, HSL, añadir, quitar, mover, rotar) es la **unidad 11c**, aparte. Cambios: `px-ui` y `px-app`. **No se toca `px-core` ni `px-raster`**: todo lo que hace falta ya existe (`Image::mode`, `Image::palette`, `Palette::len`, `get`, `resolved`, `transparent_index`, `texture::pixel_rgba`). Sin dependencias nuevas, sin `unsafe`, sin cambios en `deny.toml`, `docs/DEPENDENCIES.md` ni `docs/THIRD-PARTY.md`.

**Actualización de la unidad I.1 (2026-10-20), que prevalece:** las teclas `1` a `9` ya no eligen entrada ni ranura; `]` y `[` recorren las entradas (indexado) o las nueve ranuras (resto de modos) y dan la vuelta. Clic en una muestra y teclas siguen siendo el mismo mecanismo (`ColorChoice`). Ver [`interfaz-teclado.md`](interfaz-teclado.md).

Texto, escala, tema, `fit_text`, `INTERFACE_CHARSET` y la barra de estado: `specs/hito-2-texto.md` (11a). Atajos 1 a 9, `pixel_for_slot` y ranuras: `specs/hito-2-lapiz-deshacer.md` §4. Conversión pantalla-imagen y ajuste: `specs/hito-2-ventana.md`. Paleta y color transparente: `specs/hito-1-palette.md`.

## Objetivo

Un panel a la izquierda de la ventana que muestra los colores disponibles y deja elegir el activo con el ratón, sin modificar el documento ni la historia. Elegir con el panel y con las teclas 1 a 9 es **el mismo mecanismo** y se mantienen sincronizados.

## Lo comprobado

- `Panel::left` y `exact_size` existen en `egui` 0.37.0 (mismo uso que `Panel::bottom` en `status.rs`).
- `fn f(c: impl Into<ColorChoice>)` con `From<u8>` acepta `f(3)` (literal sin sufijo): compilado en un fichero suelto. Con esto, los tests de las unidades 6 y 7 (`canvas_input(.., 3, ..)`) deberían compilar sin cambios; se confirma al compilar los tests reales.
- Los números de la fila 5 los calculé a mano con la regla de `View::fit` y un script; **no medí nada con `egui`**: los fija el banco de pruebas, como en la fila 13 de 11a.

## Decisiones (a confirmar)

1. **Qué se elige: `ColorChoice`.** Con 256 entradas, la «ranura 1 a 9» (`u8`) no basta. En `px-ui::color`: `enum ColorChoice { Slot(u8), Entry(u8) }`. `Slot(n)` es lo de hoy (1 a 9; en imágenes no indexadas). `Entry(i)` es el índice de paleta (0 a 255, solo indexadas). Forma normal: **indexadas usan `Entry`, las demás `Slot`**. `From<u8> for ColorChoice` da `Slot`, y `ToolState::canvas_event`/`canvas_input` pasan de `slot: u8` a `choice: impl Into<ColorChoice>`: los tests anteriores no cambian. `pixel_for_choice(image, Slot(n))` = `pixel_for_slot` (también en indexadas, como hoy); `Entry(i)` da `Indexed(i)` si la imagen es indexada e `i < len`, y `None` si no.
2. **Panel a la izquierda, siempre visible y de ancho fijo** (233 u, ver abajo), en todos los modos. A la izquierda y arriba su origen es (0, 0) de la ventana, y así todas las medidas caen en píxeles físicos enteros; a la derecha dependería del ancho de la ventana. Al ser fijo, abrir un documento de otro modo no mueve el lienzo.
3. **Medidas en «píxeles de interfaz» `u`** (`u` = `n` píxeles físicos, `n = text_scale(ppp)`, como la barra de 11a): muestra de 10 u, hueco de 4 u, **paso de 14 u = el alto de una línea de texto**, 16 columnas. La elegida lleva un anillo de 2 u en el hueco; la del cursor, uno de 1 u.
4. **Se elige al pulsar** (botón izquierdo, `PointerButton` en los eventos del fotograma, como el lienzo), no al soltar. Otros botones, el hueco y lo de fuera no hacen nada.
5. **Desplazamiento por filas enteras** (nunca una fila cortada) con acumulador de píxeles, e indicador (barra de 2 u) solo si hace falta.
6. **Alfa parcial:** el color se pinta con su alfa **sobre el damero** (mezcla normal de `egui`, como la superposición del lápiz sobre el fondo); el hexadecimal con alfa del texto da el valor exacto. La entrada transparente se pinta **solo como damero** (se ignora su RGB), aunque su color guardado sea opaco.
7. **Abrir un documento reinicia siempre** (también si el modo y el tamaño de la paleta coinciden): selección inicial y desplazamiento a 0. Sustituye la regla de 15 («conservar la ranura si vale»), así que **se elimina `adjust_slot` y sus dos tests** (`fila_8_adjust_slot` y `fila_8_una_ranura_fuera_de_rango_vuelve_a_la_1`, en `px-app/src/session/tests.rs`) y se **sustituyen por uno** que fija la regla nueva (fila 8). Es la única edición de tests anteriores, y la pide el reinicio. La selección y el desplazamiento viven en `Session` (campos `choice` y `scroll`), no en `App`, para poder probarlos sin ventana: `Session::open` los reinicia solo en la rama `Ok`, y `save` y `save_as` no los tocan (fila 13). `specs/hito-2-abrir-guardar.md` se actualiza (firma de `Session`, paso 4 de «abrir» y su fila 8).
8. **Texto arriba, rejilla debajo.** Así el origen de la rejilla no depende del alto de la ventana.

## Qué muestra

Medidas en `u` desde la esquina superior izquierda del panel (la **rejilla** empieza en (4, 41)):

| Zona | Posición y tamaño |
| --- | --- |
| Fondo | `SURFACE_1`, ancho **233 u** = 4 (margen) + 224 (16 pasos) + 2 + 2 (pista) + 1 (trazo derecho, `SURFACE_3`) |
| Línea 1 | texto en (4, 4), 14 u de alto, color `TEXT` |
| Línea 2 | texto en (4, 18), 14 u de alto, color `TEXT_DIM` |
| Separador | franja de 1 u en y = 36, `SURFACE_3` |
| Rejilla | origen (4, 41); la muestra `i` está en columna `i % 16`, fila `i / 16`; muestra de 10×10, paso 14 |
| Pista de desplazamiento | x = 230..232, de y = 41 a 41 + `filas_visibles`·14; solo si hay más filas que las visibles |

Texto con la fuente de 11a, **sin color como única señal** (la elegida lleva anillo y la línea 2 dice «activa»). Todo texto que viene de la imagen pasa por `replace_missing`; aquí solo hay números y hexadecimales.

- **Indexadas:** una muestra por entrada de la paleta (1 a 256).
- **RGBA, Gray y GrayAlpha:** las 9 ranuras de hoy, en una fila (`pixel_for_slot` + `pixel_rgba`). Sin editor todavía.
- **Muestra:** `swatch_parts` (fila 10). **Elegida:** anillo `ACCENT` de 2 u. **Cursor** (si no es la elegida): anillo `TEXT` de 1 u. Los anillos van en el hueco: nunca tapan color y no los recorta nada (el hueco del borde de la rejilla mide 4 u).
- **Información** (de la muestra bajo el cursor; si no hay ninguna, de la elegida):
  - Línea 1, `cell_label`: indexada `índice 12 #ff8800` (color **resuelto** (`Palette::resolved`, el que se pinta); con alfa distinto de 255, `#rrggbbaa`, como `Rgba::fmt`; la entrada transparente da `#rrggbb00`); RGBA `color 3 #ff0000` (`color 9 #00000000`); Gray `color 4 gris 96`; GrayAlpha `color 4 gris 96`, o `color 9 gris 0 alfa 0` si el alfa no es 255.
  - Línea 2, `info_flags`: las que apliquen, unidas con ` · ` y en este orden: `cursor` (si la mostrada es la del cursor), `activa` (si es la elegida) y `transparente` (indexada: es el índice transparente; RGBA y GrayAlpha: alfa 0; Gray: nunca). Puede quedar vacía.
- **Barra de estado:** el color activo pasa a `choice_label(image, choice)`: `Slot` como hoy (`slot_label`), y `Entry(i)` = `índice 12 #ff8800` (la transparente, `índice 2 #00ff0000 transparente`). Es un cambio de texto visible solo en indexadas: antes decía `color 3 índice 2`. `slot_label` no cambia.

## Interacción

- **Clic en la muestra `i`:** `choice_for_cell(image, i)` = `Entry(i)` en indexadas y `Slot(i + 1)` en las demás. **Teclas 1 a 9:** `key_choice(image, n)` = `Entry(n − 1)` o `Slot(n)`, o `None` si la imagen no tiene esa muestra (se ignora, como hoy). La muestra `k` por clic y la tecla `k + 1` dan **la misma** `ColorChoice`, y de ella sale el mismo `Pixel`.
- Con un gesto en curso (`ToolState::busy`) los clics del panel se ignoran, como las teclas. El panel **no toca el documento**: `state_id`, deshacer y rehacer son los mismos antes y después de cualquier interacción con él.
- **Rueda sobre el panel:** desplaza (hacia abajo, filas posteriores); no hace zoom del lienzo (el lienzo solo mira el puntero dentro de **su** rectángulo). Una «línea» de rueda son 3 filas; en puntos, los píxeles físicos de verdad (`delta · ppp`). La elegida no se trae a la vista sola: la línea 2 y la información dicen cuál es.
- **Orden del fotograma en `px-app`:** `status::show`, `palette_panel::show`, y luego el lienzo (que ya usa `available_rect_before_wrap`). Un clic o una rueda en el panel no llegan al lienzo: queda fuera de su rectángulo.

## Nitidez y lienzo

- Todo es entero de `u`, así que cada medida es `k·n` píxeles físicos para cualquier `ppp`, igual que en la barra. Las posiciones salen de **(0, 0)** más enteros de `u`; el panel va junto al borde.
- **Filas visibles:** `floor((alto_px − 41·n) / (14·n))`, 0 si es negativo; el alto es el del panel (la ventana menos la barra). Cada fila cuenta su hueco final (como las columnas), y el último anillo cabe.
- **Lienzo:** el panel quita **233 u de ancho** (233·n px) al rectángulo del lienzo, y la barra sigue quitando `20·n` de alto. El ajuste (`Home`, y el primero al abrir, porque la vista se calcula en el primer `interact`) y la conversión pantalla-imagen usan ese rectángulo, como con la barra (fila 5). Al cambiar de monitor o de `ppp` el panel cambia de ancho en puntos y la vista solo se reencuadra (`clamp`), como ya hace con la barra; `Home` reajusta.
- Con una ventana más estrecha que el panel, el lienzo queda con ancho 0 (ya tratado como lienzo vacío) y el panel se recorta.

## Piezas (firmas)

**`px-ui/src/color.rs`** (se amplía; `slot_label`, `pixel_for_slot`, `SLOTS` no cambian):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChoice { Slot(u8), Entry(u8) }
impl From<u8> for ColorChoice { /* Slot(n) */ }
pub fn pixel_for_choice(image: &Image, choice: ColorChoice) -> Option<Pixel>;
pub fn initial_choice(image: &Image) -> ColorChoice;          // Entry(0) si es indexada; si no, Slot(1)
pub fn cell_count(image: &Image) -> usize;                    // paleta.len() si es indexada; si no, 9
pub fn choice_for_cell(image: &Image, cell: usize) -> Option<ColorChoice>;  // None si cell >= cell_count
pub fn cell_of_choice(image: &Image, choice: ColorChoice) -> Option<usize>; // None si no vale para la imagen
pub fn key_choice(image: &Image, n: u8) -> Option<ColorChoice>;             // n en 1..=9; None si no existe esa muestra
pub fn cell_color(image: &Image, cell: usize) -> Option<Rgba>;             // el color con que se ve (resuelto)
pub fn is_transparent_cell(image: &Image, cell: usize) -> bool;
pub fn cell_label(image: &Image, cell: usize) -> Option<String>;           // línea 1
pub fn info_flags(hover: bool, active: bool, transparent: bool) -> String; // línea 2
pub fn choice_label(image: &Image, choice: ColorChoice) -> Option<String>; // barra de estado
```

`ToolState::canvas_event` y `canvas_input` (en `tool.rs`) pasan de `slot: u8` a `choice: impl Into<ColorChoice>` y usan `pixel_for_choice`.

**`px-ui/src/palette_grid.rs`** (nuevo, **sin tipos de egui**; píxeles físicos `i64`, `view::ScreenRect` y `px_core::Rgba`). Constantes `pub const`, en `u` y de tipo `i64` salvo `COLUMNS: usize`: `COLUMNS = 16`, `CELL_U = 10`, `GAP_U = 4`, `PITCH_U = 14`, `CHECK_U = 5`, `PANEL_WIDTH_U = 233`, `GRID_X_U = 4`, `GRID_Y_U = 41`, `TEXT_X_U = 4`, `TEXT_WIDTH_U = 225`, `LINE_H_U = 14`, `LINE1_Y_U = 4`, `LINE2_Y_U = 18`, `SEPARATOR_Y_U = 36`, `TRACK_X_U = 230`, `TRACK_W_U = 2`, `RIGHT_LINE_X_U = 232`, `RING_ACTIVE_U = 2`, `RING_HOVER_U = 1`; y `CHECK_A`, `CHECK_B: Rgba` (`#4a4a4a`, `#6e6e6e`).

```rust
pub fn edges_u() -> Vec<i64>;   // todas las coordenadas x e y, en u, que usa la maqueta (bordes del panel, textos, separador, pista, y de la primera fila y columna y de la última de cada muestra con sus anillos)
pub fn total_rows(count: usize) -> usize;
pub fn visible_rows(panel_h_px: i64, n: u32) -> usize;
pub fn max_first_row(total: usize, visible: usize) -> usize;   // total − max(visible, 1), saturado
pub struct Grid { /* n, count, visible_rows, first_row */ }
impl Grid {
    pub fn new(n: u32, count: usize, visible_rows: usize, first_row: usize) -> Grid;
    pub fn cell_rect(&self, index: usize) -> Option<ScreenRect>;   // relativo al origen de la rejilla
    pub fn index_at(&self, x: i64, y: i64) -> Option<usize>;
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll { pub first_row: usize, pub rest: i64 }
impl Scroll {
    pub fn by(&mut self, delta_px: i64, n: u32, total: usize, visible: usize);
    pub fn clamp(&mut self, total: usize, visible: usize);
}
pub fn thumb(first_row: usize, visible: usize, total: usize, n: u32) -> Option<(i64, i64)>;  // (y, alto) relativos a la pista
pub fn swatch_parts(n: u32, color: Rgba) -> Vec<(ScreenRect, Rgba)>;
pub fn ring_rects(cell: ScreenRect, thickness: i64) -> [ScreenRect; 4];   // arriba, abajo, izquierda, derecha
```

`Grid::cell_rect` e `index_at`: `None` si la fila no se ve, `i ≥ count`, en un hueco o fuera. `Scroll::by` suma `delta` (saturando) a `rest`, avanza `div_euclid(14n)` filas, deja el resto en `0..14n` y limita a `0..=max_first_row`; `rest` queda en 0 si se limitó o si `first_row` es el máximo (no hay nada más que acumular). `thumb`: `T = visible·14n`; `alto = max(2n, floor(T·visible/total))`, `y = min(floor(T·first_row/total), T − alto)`; `None` si `total ≤ visible` o `visible = 0`.

**`px-ui/src/palette_panel.rs`** (nuevo, `egui`): `pub fn show(ui: &mut Ui, image: &Image, choice: ColorChoice, scroll: &mut Scroll, enabled: bool) -> Option<ColorChoice>` (`enabled` falso con un gesto en curso: sigue mostrando y desplazando, pero no elige). Es un `Panel::left("paleta")` de `PANEL_WIDTH_U · ui_unit(ppp)` puntos, sin separador propio, con marco `SURFACE_1`; se monta después de `status::show` y antes del lienzo. Solo lee las constantes de `palette_grid`.

**`px-ui/src/theme.rs`:** `RING_ACTIVE: Color32` (= `ACCENT`) y `RING_HOVER: Color32` (= `TEXT`), que contrastan ≥ 3:1 con `SURFACE_1` (`contrast_ratio` de 11a; fila 10). El damero no lleva texto encima, así que no entra en la tabla de contraste del texto.

**Constantes en un solo sitio:** todas las medidas son constantes de `palette_grid.rs`, en `u`; `palette_panel.rs` y los tests las leen de ahí y no repiten ningún número. **`u` = `n` = `text_scale(ppp)` píxeles físicos**, siempre un entero (`ui_unit(ppp)` de 11a, en puntos).

**`px-app/src/session.rs`:** `Session` gana `pub choice: ColorChoice` y `pub scroll: Scroll`; `sample` y un `open` con éxito ponen `initial_choice(imagen)` y `Scroll::default()`; un error de `open`, `save` y `save_as` no los tocan; se elimina `adjust_slot`. **`px-app/src/main.rs`:** `slot` desaparece; `handle_keys` usa `key_choice` y `session.choice`; el panel recibe `session.choice` y `session.scroll`; la barra usa `choice_label`.

**Damero y marcas** (`swatch_parts`, relativo a la esquina de la muestra, orden de pintado): alfa 255, un rectángulo de 10n×10n con el color; alfa 0, cuatro cuadrados de 5n (`CHECK_A` en (0,0) y (5n,5n); `CHECK_B` en los otros dos), sin color; alfa de 1 a 254, el damero de cuatro y encima el rectángulo con su alfa.

## Casos límite

Medidas relativas al origen de la rejilla, `n` = 1, 2 y 3 (los números se escalan con `n`).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `cell_rect(i)` con 256 entradas, 16 filas visibles, `first_row` 0: `i` = 0, 1, 15, 16, 255, 256. Después `first_row` 3 con 5 filas visibles: `i` = 47, 48, 111, 112, 128 | (0,0,10n,10n); (14n,0,…); (210n,0,…); (0,14n,…); (210n,210n,…); `None`. Con `first_row` 3: 47 `None` (fila 2, arriba); 48 en (0,0); 111 en (210n,42n); 112 en (0,56n); 128 `None` (fila 8, abajo) |
| 2 | `index_at` con 256 entradas y 16 filas: (0,0); (10n−1, 10n−1); (10n, 0); (14n−1, 0); (14n, 0); (10n, 10n); (−1, 0); (0, −1); (220n, 0); (224n, 0); (0, 224n). Con 9 entradas: (126n, 0) y (0, 14n). Con 1: (0,0), (14n, 0) | `Some(0)`; `Some(0)`; `None` (hueco); `None`; `Some(1)`; `None` (esquina de huecos); `None`; `None`; `None` (la columna 15 acaba en 220n); `None` (no hay columna 16); `None` (no hay fila 16). Con 9: `None` (índice 9 ≥ 9) y `None` (fila 1). Con 1: `Some(0)`, `None` |
| 3 | `visible_rows(alto, n)` con n = 1: 0, −5, 54, 55, 68, 69, 264, 265; con n = 2: 109, 110; con n = 3: 164, 165. `total_rows` de 0, 1, 9, 16, 17, 256. `max_first_row` de (16,5), (16,16), (16,20), (16,0), (0,5) | 0, 0, 0, 1, 1, 2, 15, 16; 0, 1; 0, 1. Filas: 0, 1, 1, 1, 2, 16. Máximos: 11, 0, 0, 15, 0 |
| 4 | **Desplazamiento.** `Scroll` (0,0), n = 1, 16 filas, 5 visibles: `by` +7, +7, +21, −8, −100; después +10¹², +1, 0, y con `i64::MIN` y `i64::MAX`. Con n = 2: +28. `clamp(16, 12)` desde la fila 11. `thumb(first_row, visibles, total, n)` con (0, 5, 16, 1) y (11, 5, 16, 1); con (0, 5, 16, 2) y (11, 5, 16, 2); con (0, 16, 16, 1), (0, 5, 4, 1) y (0, 0, 16, 1); y con (15, 1, 16, 1) | (`first_row`, `rest`): (0,7); (1,0); (2,7); (1,13); (0,0). Luego (11,0); (11,0); sin cambio; con `i64::MIN` y luego `i64::MAX`: (0,0) y (11,0), sin pánico ni desbordamiento. n = 2: (1,0). `clamp`: fila 4. `thumb`: (0,21) y (48,21); (0,43) y (96,43); `None`; `None`; `None`; (12,2) (alto mínimo 2n) |
| 5 | **Lienzo desplazado por el panel.** Imagen RGBA 64×48, pantalla de 800×600 pt, panel y barra; `Home`, y el puntero sobre el píxel (2, 3), sobre el panel (100, 100) pt y sobre la barra. Y con una pantalla de 200×600 pt | ppp 1: lienzo (233, 0)–(800, 580) pt, 567×580 px; `Home` da **×8**; la imagen ocupa 512×384 px desde (260, 98) y el puntero en (280, 126) da (2, 3). ppp 2: panel de 233 pt (466 px), barra de 20 pt; lienzo de 1134×1160 px; **×17**; la imagen, 1088×816 px desde (489, 172) px; puntero (265,75; 115,75) pt da (2, 3). ppp 1,5: panel de 310,667 pt (466 px); lienzo de 734×860 px; **×11**; imagen de 704×528 desde (481, 166) px; puntero (339; 136,33) pt da (2, 3) (±0,05 px: `egui` cuantiza el panel a 1/32 pt). En los tres, sobre el panel o la barra: `hover` ninguno y ningún evento de trazo, aunque se pulse y se mueva; la rueda sobre el panel no cambia el zoom del lienzo. Con 200 pt de ancho: lienzo de ancho 0, sin pánico, `Home` sin efecto, `hover` ninguno |
| 6 | **Elegir.** Indexada de 256 (las muestras se numeran desde 0): clic en la muestra 200 y en la 8; teclas 1 y 9. RGBA: clic en la muestra 2 y tecla 3. Gray: clic en la muestra 8. Indexada de 3 colores: tecla 9. Después `cell_of_choice` y `choice_label` | `Entry(200)` → `Indexed(200)`; la 8 y la tecla 9 dan `Entry(8)`; la tecla 1, `Entry(0)`. RGBA: clic 2 y tecla 3 dan `Slot(3)` → `Rgba(#ff0000)`. Gray: `Slot(9)` → `Gray(255)`. Con 3 colores: `key_choice` da `None` y la selección **no cambia**. `cell_of_choice` da la muestra pulsada (o la de la tecla menos 1). `choice_label`: `índice 200 #…`; RGBA `color 3 #ff0000`. Un clic en la muestra `k` y la tecla `k + 1` dan siempre la misma `ColorChoice` |
| 7 | **Clics que no eligen:** en un hueco, fuera de la rejilla, en la fila 5 con solo 3 visibles, en `i ≥ count`, con el botón derecho o central, y con un gesto en curso (`enabled` falso) | La selección, `state_id`, `can_undo` y `can_redo` del documento no cambian; `show` devuelve `None` |
| 8 | **Abrir un documento.** `initial_choice`: indexada de 256 con `Entry(200)` y desplazamiento 7, y se abre una RGBA; RGBA con `Slot(9)`, y se abre una indexada de 3; indexada con `Entry(2)`, y se abre otra indexada de 3; Gray; GrayAlpha. Lo mismo con `Session::open` (px-app, ficheros PNG del corpus o escritos en el test): se elige una muestra, se desplaza `scroll` y se abre otro fichero; y un `open` con error (fichero inexistente; y con cambios sin guardar sin `discard`) | `initial_choice`: `Slot(1)`; `Entry(0)`; `Entry(0)` (mismo modo y mismo tamaño: igual se reinicia); `Slot(1)`; `Slot(1)`. `Session::open` con éxito: `choice == initial_choice(nueva)` y `scroll == Scroll::default()`, en todos los casos anteriores. Con error: `choice` y `scroll` **no cambian** |
| 9 | **Textos.** Indexada `[#000000, #ff0000, #00ff00 (índice transparente), #0000ff80, …, #ffffff (255)]`: `cell_label` de 1, 2, 3 y 255. RGBA y GrayAlpha: `cell_label` de las muestras 2 y 8 (desde 0: las ranuras 3 y 9). Gray: la muestra 3 (ranura 4). `info_flags` (hover, activa, transparente) en las ocho combinaciones. `choice_label` con `Entry(2)` y `Slot(9)` en RGBA | `índice 1 #ff0000`; `índice 2 #00ff0000` (resuelto: alfa 0); `índice 3 #0000ff80`; `índice 255 #ffffff`. RGBA `color 3 #ff0000` y `color 9 #00000000`; GrayAlpha `color 3 gris 64` y `color 9 gris 0 alfa 0`; Gray `color 4 gris 96`. Flags: `""`, `activa`, `transparente`, `activa · transparente`, `cursor`, `cursor · activa`, `cursor · transparente`, `cursor · activa · transparente`. `choice_label`: `índice 2 #00ff0000 transparente` y `color 9 transparente` |
| 10 | **Muestras.** `swatch_parts(n, c)` con `#ff0000` (a 255), `#00000000`, `#ff000001`, `#ff000080`, `#ff0000fe`. Una indexada con el índice transparente guardado como `#ff0000` opaco. `ring_rects` de la muestra en (0,0) con grosor 2n y con grosor n; `contrast_ratio(RING_ACTIVE, SURFACE_1)` y `contrast_ratio(RING_HOVER, SURFACE_1)` | Un rectángulo (0,0,10n,10n) rojo. Cuatro cuadrados de 5n (`CHECK_A`, `CHECK_B`, `CHECK_B`, `CHECK_A`) sin color. Los otros tres, el damero y encima el rojo con alfa 1, 128 y 254. La entrada transparente guardada opaca se pinta como la segunda (su `pixel_rgba` ya da alfa 0). Anillo de 2n: (−2n,−2n,14n,2n), (−2n,10n,14n,2n), (−2n,0,2n,10n), (10n,0,2n,10n); con grosor n: (−n,−n,12n,n), (−n,10n,12n,n), (−n,0,n,10n), (10n,0,n,10n). Los dos contrastes son ≥ 3,0 |
| 11 | **Paletas límite.** Una de 1 entrada; una de 256; las 9 ranuras de RGBA, Gray y GrayAlpha; el índice transparente en 0, 17 y 255 (y en `None`); `[A, A, A]` (iguales) y `[A, A con alfa 128]` | 1 entrada: una muestra, 1 fila, sin pista (`thumb` `None`), teclas 2 a 9 ignoradas. 256: 16 filas, la 255 en columna 15 y fila 15. Modos sin paleta: 9 muestras en una fila, las ranuras 9+ no existen (`index_at` `None`); la 9 es damero en RGBA y GrayAlpha y blanco opaco en Gray. Transparente: damero y la bandera `transparente` solo en esa entrada, y lápiz con ella activa escribe `Indexed(t)`. Duplicadas: las tres se pueden elegir (clic en la 1 da `Entry(1)`, `cell_of_choice` 1) y solo la 1 lleva anillo |
| 12 | Con `MissingGlyphPolicy::Panic` y el banco de 11a: maquetar `cell_label` (todos los modos, el más largo `color 9 gris 0 alfa 0`, 21 caracteres) e `info_flags` (el más largo `cursor · activa · transparente`, 30), a ppp 1, 1,25, 1,5, 2 y 3; y los 225 u útiles | Sin pánico; todo sale de `INTERFACE_CHARSET`; ninguna línea supera las 32 columnas que caben (`columns(225·n, ppp)` = 32 con cualquier `n`), así que `fit_text` no recorta |
| 13 | **Guardar no reinicia.** `Session` con un PNG abierto (indexado de 256), `choice` = `Entry(200)` y `scroll` desplazado; `apply` de un píxel; `save`; `save_as`; y `save` de una sesión de muestra sin ruta | Tras cada una, con éxito, `choice` y `scroll` son los mismos que antes. Si el guardado falla (directorio inexistente), tampoco cambian |

## Propiedades (proptest)

1. **Geometría.** Para `n` de 1 a 8, `count` de 0 a 256, `visible_rows` de 0 a 20, `first_row ≤ max_first_row` y cualquier punto `(x, y)` (en un rango que incluye los bordes y los negativos): `index_at(x, y) == Some(i)` si y solo si `(x, y)` está en `cell_rect(i)`; ningún punto está en dos muestras; un punto en un hueco (`x mod 14n ≥ 10n` o `y mod 14n ≥ 10n`) da `None`.
2. **Desplazamiento.** Para cualquier secuencia de `by` (`i64` cualesquiera, también los extremos), `total` y `visible`: sin pánico ni desbordamiento; `first_row ≤ max_first_row` y `0 ≤ rest < 14n`; un delta ≥ 0 no reduce `first_row` y uno ≤ 0 no lo aumenta; `thumb` queda siempre dentro de la pista (`y + alto ≤ visible · 14n`).
3. **Nitidez.** Para `ppp` en 1,0, 1,25, 1,5, 2,0 y 3,0 (probados siempre) y para cualquier múltiplo de 0,05 entre 1 y 4: con las medidas de «Qué muestra» (panel, separador, rejilla, muestras, anillos, pista, líneas de texto), cada borde en puntos × `ppp` es entero (±0,001) y múltiplo de `n`; el panel empieza en (0, 0).
4. **Selección.** Para cualquier modo, de 1 a 256 entradas y cualquier secuencia de pulsaciones (en cualquier punto, también huecos y fuera), ruedas, teclas 0 a 9, con y sin gesto en curso: la selección activa siempre cumple `pixel_for_choice(..).is_some()`; el documento no cambia (`state_id`, `can_undo`, `can_redo`); y un clic en una muestra visible `i` deja `cell_of_choice == Some(i)`.

## Dónde van las pruebas

`px-ui/tests/palette_grid.rs` (filas 1 a 4, 10 y la parte geométrica de la 11; propiedades 1 y 2); `px-ui/tests/color_choice.rs` (filas 6 a 9 en su parte de `px-ui`, la parte de selección de la 11, y la propiedad 4); `px-app/src/session/tests.rs` (fila 8 en su parte de `Session::open`, que sustituye a los dos tests de `adjust_slot`, y fila 13); `px-ui/tests/palette_panel.rs` (filas 5 y 12 y propiedad 3; banco de `egui` copiado de `tests/status.rs`, sin modificarlo). Los tests de las unidades anteriores **no se tocan**, salvo los dos de `adjust_slot` (decisión 7; véase la sección siguiente). Sin dorados: el panel es de diseño propio. Orden de trabajo, en commits pequeños: spec aprobada; tests (agente `test-writer`); `ColorChoice` y `ToolState`; `palette_grid`; tema y `palette_panel`; `px-app`. Al final: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y `license-auditor`.

## Tests de otras unidades que cambian por el formato de la barra (y por la selección)

Revisado con `grep` en `px-ui/tests`, `px-ui/src` y `px-app/src` (`slot_label`, `adjust_slot`, `índice N`, `color N`):

| Test | ¿Cambia? | Cambio mínimo |
| --- | --- | --- |
| `px-ui/tests/color.rs` (`slot_label_*`, incluidos `color 2 índice 1` y `color 1 índice 0`) | No | Ninguno: `slot_label` no se modifica y sigue dando `color 2 índice 1` para `Slot(2)` en una indexada. La app ya no lo usa con indexadas (usa `Entry`), pero la función y su contrato quedan. |
| `px-app/src/title.rs` (filas 17 de la unidad 6, 10 de 11a y propiedad 5, incluido el generador de `color {slot} índice {i}`) | No | Ninguno: `title_text`, `window_title` y `status_line` reciben el texto del color ya hecho; el generador es de cadenas arbitrarias. |
| `px-ui/tests/tool.rs` (`status_text`), `px-ui/tests/text.rs` fila 5, `px-ui/tests/status.rs` | No | Ninguno: usan textos literales. |
| `px-ui/tests/pencil.rs`, `tool_canvas.rs`, `tool.rs`, `px-app/src/demo.rs` (`fill_tests`) que llaman a `canvas_event`/`canvas_input` con `slot: u8` | No, si compilan | Ninguno: `impl Into<ColorChoice>` con `From<u8>`. **Si alguno no compila, se para y se avisa** (matiz 1). |
| `px-app/src/session/tests.rs`: `fila_8_adjust_slot` y `fila_8_una_ranura_fuera_de_rango_vuelve_a_la_1` | **Sí** | Se eliminan y se sustituyen por `fila_8_abrir_reinicia_la_seleccion_y_el_desplazamiento` (fila 8) y `fila_13_guardar_no_reinicia_la_seleccion` (fila 13). Son los únicos tests anteriores que se tocan. |

Cambia, pues, un solo fichero de tests de otras unidades, y por la regla de reinicio, no por el formato de la barra: el formato nuevo solo lo produce `choice_label`, que es código nuevo con tests propios (fila 9).

## Prueba manual

`cargo run --release -p px-app`, a 1,0, 1,5 y 2,0 (como en 11a). Sin capturas de pantalla (`CLAUDE.md`): la comprobación visual es tuya, con una lupa si hace falta.

1. Al abrir la muestra RGBA hay un panel a la izquierda con **9 muestras** en una fila y la primera (negra) con el anillo verde. Debajo de ellas, el vacío. El texto de arriba dice `color 1 #000000` y `activa`.
2. Pulsar la 3: la roja pasa a tener el anillo, el texto dice `color 3 #ff0000`, la barra de estado también, y el lápiz pinta rojo. Pulsar la tecla 5: se mueve el anillo y el color. Pulsar la 9 (damero): el lápiz borra a transparente.
3. Pasar el cursor por las muestras: el texto cambia y dice `cursor`; al salir vuelve a la elegida. Pulsar en el hueco entre muestras o en el panel vacío no cambia nada. **Nitidez:** muestras de tamaño igual, huecos iguales, anillos del mismo grosor y sin bordes grises, a todas las escalas.
4. Abrir (`arrastrar`) un PNG indexado con paleta de 256 colores y con transparencia: 16 filas de 16 muestras; la transparente en damero, con `transparente` en la línea 2 al pasar por encima; pulsar la muestra de índice 200 (fila 12, columna 8) da `índice 200 #…` y las teclas 1 a 9 mueven el anillo por las nueve primeras. Un PNG con colores repetidos: cada muestra se puede elegir por separado.
5. Reducir la altura de la ventana: la rejilla muestra filas enteras, la rueda las desplaza y la barra de 2 u lo indica; con una altura mínima desaparecen las muestras sin romper nada. Estrechar la ventana por debajo de 233 u: el panel se recorta y la app no se cae.
6. Con la herramienta de línea a medio gesto (botón pulsado), pasar por el panel y soltar: no cambia el color ni se queda la línea a medias.
7. Abrir un PNG nuevo tras elegir la muestra 200 o desplazar: la selección vuelve a la primera y el panel a la fila 0. Un fichero que no se puede abrir no cambia nada. `Home` reencuadra con el lienzo recortado por el panel.
8. Cambiar de monitor con otra escala (si hay): el panel y el lienzo se reajustan sin reiniciar.

## Fuera de alcance

Editar la paleta y sus controles (RGB, HSL, añadir, quitar, mover, rotar: unidad **11c**), segundo color o botón derecho, ocultar o redimensionar el panel, arrastrar muestras, nombres y consejos emergentes, recorrer las muestras con el teclado, traer la elegida a la vista, ordenar o fusionar duplicados, tema claro, más de 9 ranuras en los modos sin paleta, y un tablero de transparencia en el lienzo (sigue siendo el fondo liso de la unidad 2).
