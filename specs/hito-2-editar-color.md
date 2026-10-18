# Hito 2 · Unidad 11c: editar un color (RGB, HSL, hexadecimal y alfa)

Estado: **aprobada** (2026-10-18; decisiones 1, 2, 5, 6, 7, 8, 9 y 12 con los cambios del visto bueno incorporados al texto, que **prevalecen sobre cualquier otra frase de la página**: (3) las ranuras editadas se **conservan durante la sesión, por modo**; (4) hexadecimal de 3 o 6 dígitos conserva el alfa actual y de 4 u 8 lo fija, con una fila de casos cada uno; (5) con valor inválido, Intro mantiene el foco y el aviso, perder el foco revierte el texto y avisa 5 s en la barra de estado, y `Escape` revierte; (7) campos numéricos editables y pasos con el teclado quedan como pulido posterior; (8) filas nuevas de teclado con el campo enfocado y cambio **aditivo** en `canvas.rs`; (9) la vista previa regenera solo cuando cambia el valor, soltar y cancelar la restauran y se anota el límite con lienzos enormes; (10) el editor se **ancla al fondo del panel** y la rejilla usa la altura restante (el aviso de «editor oculto» se retiró el 2026-10-18: ver `hito-2-panel-ventana-baja.md`); aviso de la entrada transparente: «aviso: entrada transparente» y «se guarda con alfa 0»). Cambios: `px-ui` y `px-app`. **No se toca `px-core` ni `px-raster`**: todo lo que hace falta existe (`Command::SetPaletteColor`, `Hsl`, `Rgba::from_hex`, `Rgba: Display`, `Palette::get`, `Document::apply`). Sin dependencias nuevas, sin `unsafe`, sin cambios en `deny.toml`, `docs/DEPENDENCIES.md` ni `docs/THIRD-PARTY.md`. Añadir, quitar, mover y rotar entradas, y fijar el índice transparente, es la **unidad 11d**, aparte.

Referencias: `SetPaletteColor` (`Ok(false)` con el mismo color; `Ok(true)` también sobre la transparente), `Remove` y avisos: `specs/hito-2-paleta-editar.md`. `Hsl` y la nota «Para el panel de paletas»: `specs/hito-2-hsl.md`. Texto, `columns`, `NOTICE_PREFIX`, `INTERFACE_CHARSET`, escala `n` y `u`: `specs/hito-2-texto.md` (11a). Panel, `ColorChoice`, geometría en píxeles físicos enteros, `Session.choice` y reinicio al abrir: `specs/hito-2-panel-paletas.md` (11b). Gesto en curso, `Escape`, `busy` y textura: `specs/hito-2-lapiz-deshacer.md` §3 y §6 y `specs/hito-2-linea-relleno-app.md` §1 y §2. Ranuras 1 a 9 y sus colores: `specs/hito-2-lapiz-deshacer.md` §4.

## Objetivo

Debajo de la rejilla del panel, un editor del color elegido: deslizadores RGB y HSL, alfa y un campo hexadecimal. Un arrastre o una confirmación es **un solo paso** de deshacer, y mientras se arrastra el documento no se toca.

## Lo comprobado (y lo que no)

- `egui` 0.37.0: `Escape` pone `focused_widget = None` (`memory/mod.rs:598`), así que un `TextEdit` **pierde el foco también con `Escape`**: se distingue por el evento del mismo fotograma. `Context::egui_wants_keyboard_input()` existe. `Margin` es `i8` en **puntos enteros** y `TextEdit::frame` recibe un `Frame`: por eso el campo va con `Frame::NONE` y `Margin::ZERO` en un rectángulo interior que calculamos nosotros (a 1,5× un margen de 3 u no es un entero de puntos).
- **Comprobado después, en el banco de egui sin ventana** (al implementar): un clic fuera del campo le quita el foco (al soltar el botón); `Tab` lo saca, pero el foco pasa al lienzo (`Sense::click_and_drag`), por eso la guarda es `text_edit_focused()`; `Intro` y `Escape` le quitan el foco. egui redondea el diseño a 1/32 de punto: la forma del texto del campo queda a menos de 0,03 px de un píxel físico y el teselado redondea el texto al píxel, así que los tests aceptan 0,05 px.
- **No comprobado** (sigue siendo de la prueba manual): la nitidez real del campo en pantalla a 1,25× y 1,5×; el coste de subir la textura entera por fotograma con un lienzo grande; que soltar el botón fuera de la ventana llegue siempre (red de seguridad).

## Decisiones (todas a confirmar; las que cambian lo que pediste, marcadas con ⚠)

1. **Qué se edita.** `EditTarget::of(image, choice)`: en indexadas, la **entrada** de la paleta (`Entry(i)`, y `Slot(n)` vale como la entrada `n − 1`); en RGBA, Gray y GrayAlpha, la **ranura** `Slot(1..=9)`. Si no vale (`Entry` fuera de la paleta, `Slot(0)` o `Slot(10)`, `Entry` en un modo sin paleta) no hay destino y el editor no se muestra. Se edita el color **guardado** (`Palette::get`), no el resuelto.
2. **Indexadas: `SetPaletteColor`** al confirmar, con deshacer. **Resto de modos: estado de la sesión**, no del documento: sin paso de deshacer, sin marcar el documento como modificado y sin que `Ctrl+Z` lo toque. La sesión guarda un `SlotBank` con **una tabla `SlotColors` (9 `Rgba`) por modo** (RGBA, Gray y GrayAlpha; en indexadas no se usan), cada una con los valores por defecto de la tabla de `pixel_for_slot` (`SlotColors::defaults(mode)`). **Las ranuras editadas se conservan durante toda la sesión**: abrir otro documento reinicia solo `choice` y `scroll` (como en 11b) y el editor (`reset`), no el banco; abrir uno del mismo modo o de otro y volver al primero las encuentra como se dejaron. No se guardan en disco. El panel pinta y rotula cada ranura con su color actual (`cell_color`, `cell_label` y `choice_label` pasan a leer `SlotColors`, con envoltorios que usan los valores por defecto para no tocar los tests del 11b). El lápiz, la línea y el relleno usan el color editado.
3. ⚠ **Gray y GrayAlpha: un solo deslizador «G» (gris 0 a 255), sin RGB ni HSL**, porque un color cromático no se puede dibujar en esos modos y convertirlo (luma) sería una pérdida silenciosa. El hexadecimal en esos modos **debe ser gris** (`r = g = b`; si no, `NotGray`) y en Gray **sin alfa** (alfa ≠ 255: `NoAlpha`). **Alfa:** el deslizador «A» (0 a 255) está en indexadas y RGBA, como pediste, **y también en GrayAlpha** (su ranura 9 es `{gray 0, alpha 0}`: sin alfa no se podría hacer visible); en Gray no.
4. **Controles.** Siete en indexadas y RGBA (`R G B | H S L | A`), dos en GrayAlpha (`G | A`), uno en Gray (`G`). Valores enteros **siempre visibles** a la derecha de cada uno, con la fuente de 11a, alineados a la derecha en 3 columnas: R, G, B, A y gris 0 a 255; H 0 a 359 (`round(h) mod 360`); S y L 0 a 100 (`round(100·x)`). Alfa en 0 a 255 (no en %), como el `aa` del hexadecimal. **Precisión:** la pista mide 184 u, así que a 1,0× un deslizador de 256 valores avanza de 1,4 en 1,4 valores; el valor exacto se escribe en hexadecimal. **Pulido posterior** (fuera de esta unidad): campos numéricos editables y pasos con el teclado (egui usa las flechas para mover el foco entre widgets).
5. **Hexadecimal.** `Rgba::from_hex` (3, 4, 6 u 8 dígitos, con o sin `#`, mayúsculas o minúsculas) tras **recortar espacios** a los lados. **Alfa:** con 3 o 6 dígitos se **conserva el alfa actual** del color que se edita (`#f80` sobre `#ff000080` da alfa 128); con 4 u 8 dígitos **se fija el alfa escrito**. Texto vacío o solo `#` es `Empty`. **Se confirma con Intro o al perder el foco** (clic fuera, `Tab`), no letra a letra. **Con un valor inválido** (`HexOutcome::Invalid`, nada cambia): **Intro** mantiene el foco, el texto escrito y el aviso de dos líneas del editor (`aviso: ` + motivo) hasta que se teclee, se pulse `Escape`, cambie el destino o el color o se confirme uno válido; **perder el foco** revierte el texto al color actual y muestra el motivo como aviso de 5 s en la barra de estado (`Notice`, `PanelOutput.notice`); **`Escape`** revierte el texto sin aviso (egui le quita el foco también con `Escape`: se distingue por el evento del mismo fotograma). Un texto confirmado igual al color actual no da paso. El texto pendiente recuerda **su destino y el color base** del momento de teclear: si al perder el foco ya se eligió otra muestra, se aplica al destino original (con su alfa original); si la entrada ya no existe o su color cambió, el texto se descarta con aviso (**actualizado** por `hito-2-panel-ventana-baja.md`: `confirm_hex_checked`). En reposo el campo muestra `hex_text(color)` = `Rgba: Display` (`#rrggbb`, o `#rrggbbaa` si el alfa no es 255).
6. **Arrastre = un paso.** Pulsar en la pista **ya fija el valor bajo el puntero** (`begin`); mover con el botón pulsado lo cambia (`drag`; la `x` se limita a la pista aunque el puntero salga de ella o de la fila); soltar confirma (`release`). Solo se calcula `apply_control(base, control, valor)` desde el color **del inicio del arrastre**: el color provisional no depende del camino. **Mientras dura, el documento no se modifica** (ni `state_id`, ni historia): la imagen se previsualiza (decisión 7) y la muestra elegida del panel se pinta con el color provisional. Al soltar, `Session::commit_edit` aplica **un** `SetPaletteColor` (indexadas) o escribe la ranura. **Si el color provisional es igual al del inicio no hay paso** (también un clic sin mover sobre el valor actual). **`Escape` cancela** y restaura color, `Hsl` y textura; los movimientos siguientes se ignoran hasta soltar el botón (sin reanudar). ⚠ **Perder el foco la ventana cancela** (el lápiz, en cambio, aplica: aquí no se aplica un color a medias sin soltar). La red de seguridad de `hito-2-lapiz-deshacer.md` §3 vale: si egui ya no ve el botón primario abajo, es `release`. Mientras hay un arrastre, `busy` es cierto: se ignoran clics en la rejilla, teclas 1 a 9, `Ctrl+Z`/`Y`, guardar, cambiar de herramienta y abrir ficheros (como con un trazo). Si el editor deja de mostrarse (ventana demasiado baja) o el destino desaparece, se cancela.
7. **Vista previa sin copiar la imagen.** Función pura nueva `texture::to_rgba8_with_entry(image, index, color)`: los bytes que daría `to_rgba8` si la entrada `index` valiera `color` (alfa resuelto: el índice transparente sigue con alfa 0), leyendo la imagen sin clonarla. `Canvas::preview_entry(ctx, image, index, color)` regenera la textura **entera** con ella **solo cuando cambia el color resuelto** respecto a lo último subido (como mucho una subida por fotograma y ninguna si el valor se repite; `Canvas::preview_uploads()` cuenta las subidas para probarlo). **Soltar** (`refresh` tras el `Ok(true)`, o `end_preview` si no hubo paso) y **cancelar** (`end_preview`) la devuelven al documento; `set_image` también. Se sustituye **por índice**, no por color: las entradas duplicadas no se tocan. En ranuras no hay nada que previsualizar en el lienzo. **Límite anotado hasta la unidad 14 (regiones):** la subida es `O(ancho·alto)` por cambio de valor, como un trazo; con lienzos de decenas de millones de píxeles el arrastre irá a trompicones (en 16384² son 1 GiB por subida). Se anota allí.
8. **`Hsl` propio del editor** (`ColorEditor.hsl`), nunca recalculado desde el `Rgba` salvo en estos casos. **Se recalcula (`Hsl::from_rgba`)** cuando `sync(destino, color)` ve **otro destino**, o el mismo con **otro color guardado** (deshacer o rehacer de ese color, abrir, cualquier cambio externo), y siempre tras `reset` (abrir). **No se recalcula** tras su propia confirmación, ni tras un deshacer o rehacer que deja ese color igual (por ejemplo, un trazo). Al editar RGB o gris, el `Hsl` nuevo es `hsl_after_rgb(anterior, nuevo)`: el de `from_rgba`, salvo que el color sea gris (entonces conserva el **matiz** anterior) o negro o blanco (conserva también la **saturación** anterior), y sigue cumpliendo `hsl.to_rgba(a) == rgba` (comportamiento 6 de `hito-2-hsl.md`). Al editar H, S o L solo cambia ese campo y el `Rgba` sale de `Hsl::to_rgba(alfa)`. Así mover S o L por 0 (o L por 1) y volver recupera el color.
9. **Entrada transparente.** Se edita su color guardado (`Ok(true)`, un paso). El editor y el campo muestran el **guardado** (`#0000ff`), no el resuelto (`#0000ff00`). Aviso de dos líneas en el editor: `aviso: entrada transparente` y `se guarda con alfa 0`. El aviso del hexadecimal inválido tras Intro (`aviso: ` + motivo, una línea) lo tapa mientras dure. Los textos son fijos, **no repiten lo que escribió el usuario** (no hace falta `replace_missing`), y todos caben en las 32 columnas de 11b.
10. **Deshacer y rehacer con el editor abierto.** `choice` no cambia (11b: lo reinicia solo abrir), el fotograma siguiente `sync` actualiza valores y texto, y el `Hsl` se recalcula si el color de ese destino cambió (decisión 8). Con arrastre en curso se ignoran (decisión 6).
11. **Teclado con el campo enfocado.** Mientras `Context::text_edit_focused()` (no `egui_wants_keyboard_input()`, que también es cierto si `Tab` pasa el foco al lienzo, comprobado en el banco): `px_ui::shortcuts::gate_keys(events, wants_keyboard)` (pura) devuelve los eventos para los atajos de `px-app` (todos si no hay foco de texto; ninguno si lo hay), así teclear `ff00ab` no cambia de herramienta (`f`), de color (`0` a `9`) ni de vista, y `Ctrl+Z` o `Ctrl+Y` con el campo enfocado no tocan el documento (el `TextEdit` tiene el suyo). `Canvas::interact_ordered` ignora entonces `Home`, `+`, `-` y `Escape`. **El cambio en `canvas.rs` es aditivo** (una guarda con `ctx.text_edit_focused()`; sin foco de texto no cambia nada) y los tests de la unidad 2 pasan sin tocarse; si alguno no pasara, se para y se avisa. Al salir del campo (Intro, `Escape`, clic fuera, `Tab`) los atajos vuelven.
12. **Colocación.** El editor se **ancla al fondo del panel**, en posición estable: `top_px = ((alto_px − H·n) div n)·n` (múltiplo de `n` y calculado con el alto **redondeado** a píxeles físicos, no con el borde en puntos de egui), de modo que no se mueve al cambiar el número de entradas ni de filas. La **rejilla usa la altura restante**: `visible_rows_reserved(alto_px, reservado_px, n)`, función nueva con la altura reservada como parámetro; `visible_rows(alto, n)` se conserva con su firma y es `visible_rows_reserved(alto, 0, n)` (así los tests de 11b, que la llaman con dos argumentos, no cambian). **Actualizada por [`hito-2-panel-ventana-baja.md`](hito-2-panel-ventana-baja.md) (2026-10-18), que prevalece:** con `alto_px < (41 + 14 + H)·n` el editor **ya no se oculta ni hay aviso en la barra**; el bloque `[texto · una fila de rejilla · editor]` se desplaza entero con un único desplazamiento y una barra de desplazamiento visible (`panel_layout`, `PanelView`), y la ventana tiene una altura mínima (`min_window_height_pt`). Un arrastre en curso no se cancela por bajar la ventana. Sin interruptor, salvo la variable de entorno de pruebas `PIXOKO_SIN_MINIMO` de esa página.

## Geometría (en `u`, `n = text_scale(ppp)` píxeles físicos por `u`; constantes `pub const` en `edit_layout.rs`, que es el único sitio)

Desde la esquina del panel. El editor empieza en `top_px` (decisión 12, múltiplo de `n`); las medidas de dentro del editor son `u` desde ahí. Alturas `H`: indexadas y RGBA **186**, GrayAlpha **112**, Gray **94**. Las filas se compactan sin huecos (Gray y GrayAlpha son «Full» sin las filas que faltan).

| Zona | Posición y tamaño (desde `top_px`) |
| --- | --- |
| Separador | franja de 1 u en y = 0, ancho del panel, `SURFACE_3` |
| Cabecera | texto en (4, 4), 14 u: `editar índice 12`, `editar color 3`, `editar color 4 gris` |
| Deslizadores (Full) | R, G, B en y = 22, 36, 50; H, S, L en 68, 82, 96; A en 114 (huecos de 4 u entre grupos); cada fila de 14 u |
| Dentro de la fila | letra en x = 4 (7 u); pista en x = 15, **184 u** de largo y 4 u de alto, centrada (`SURFACE_3`; parte llena `TEXT_DIM`); marca de 3 u × 12 u en `ACCENT`, centrada en `slider_x`; valor en x = 203 (21 u, a la derecha) |
| Zona activa | x de 13 u (incluida) a 201 u (excluida), toda la fila (14 u); durante un arrastre solo cuenta la `x` |
| Campo hexadecimal | (4, fin de controles + 4), **69 × 18 u** (`#` + 8 dígitos = 63 u + 3 de relleno a cada lado), fondo `SURFACE_0`, borde `SURFACE_3` (`ACCENT` con foco); el `TextEdit` va dentro, sin marco (`Frame::NONE`, `Margin::ZERO`) |
| Aviso | dos líneas de 14 u en x = 4, tras el campo + 4 u; `WARNING` con el prefijo de texto `NOTICE_PREFIX` |

## Piezas (firmas)

**`px-ui/src/color.rs`** (se amplía; lo existente no cambia de firma ni de resultado):

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotColors { /* [Rgba; 9] */ }
impl SlotColors {
    pub fn defaults(mode: Mode) -> SlotColors;
    pub fn get(&self, slot: u8) -> Option<Rgba>;           // 1..=9
    pub fn set(&mut self, slot: u8, color: Rgba) -> bool;  // false si slot fuera de 1..=9
}
/// Una tabla por modo; Indexed comparte la de RGBA (no se usa).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotBank { /* rgba, gray, gray_alpha */ }
impl SlotBank {
    pub fn new() -> SlotBank;                               // todas por defecto
    pub fn get(&self, mode: Mode) -> &SlotColors;
    pub fn get_mut(&mut self, mode: Mode) -> &mut SlotColors;
}
pub fn pixel_for_choice_with(image: &Image, choice: ColorChoice, slots: &SlotColors) -> Option<Pixel>;
// y las variantes `_with` de cell_color, cell_label, is_transparent_cell y choice_label;
// las actuales llaman a las `_with` con SlotColors::defaults(image.mode()).
```

En Gray, el `Pixel` es `Gray(color.r)`; en GrayAlpha, `GrayAlpha { gray: color.r, alpha: color.a }`. El editor garantiza `r = g = b` en esos modos.

**`px-ui/src/tool.rs`:** `ToolState::canvas_event_with(doc, image, pixel: Option<Pixel>, event)` y `canvas_input_with(..)`; `canvas_event` y `canvas_input` calculan el `Pixel` con `pixel_for_choice` y delegan en ellas (sus tests no cambian).

**`px-ui/src/color_edit.rs`** (nuevo, **sin tipos de egui**):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditTarget { Entry(u8), Slot(u8) }
impl EditTarget {
    pub fn of(image: &Image, choice: ColorChoice) -> Option<EditTarget>;
    pub fn stored(self, image: &Image, slots: &SlotColors) -> Option<Rgba>;   // Palette::get o la ranura
    pub fn header(self, mode: Mode) -> String;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control { Red, Green, Blue, Hue, Saturation, Lightness, Alpha, Gray }
pub fn controls(mode: Mode) -> &'static [Control];       // en orden de arriba abajo
impl Control { pub fn label(self) -> char; pub fn max(self) -> u32; }   // 255, 359, 100
pub fn control_value(rgba: Rgba, hsl: Hsl, control: Control) -> u32;
pub fn value_text(value: u32) -> String;                 // 3 columnas, a la derecha
pub fn slider_value_at(x_px: i64, width_px: i64, max: u32) -> u32;   // redondea; x fuera: se limita
pub fn slider_x(value: u32, width_px: i64, max: u32) -> i64;
pub fn hsl_after_rgb(previous: Hsl, new: Rgba) -> Hsl;
pub fn apply_control(base: (Rgba, Hsl), control: Control, value: u32) -> (Rgba, Hsl);  // value se limita a max
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexError { Empty, Length(usize), Digit, NotGray, NoAlpha }
/// `current` aporta el alfa cuando el texto tiene 3 o 6 dígitos.
pub fn parse_hex(text: &str, mode: Mode, current: Rgba) -> Result<Rgba, HexError>;
pub fn hex_text(color: Rgba) -> String;
pub fn hex_notice(error: &HexError) -> &'static str;     // ≤ 25 caracteres, de INTERFACE_CHARSET
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commit { pub target: EditTarget, pub color: Rgba }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HexOutcome { Nothing, Unchanged, Commit(Commit), Invalid(HexError) }
#[derive(Default)]
pub struct ColorEditor { /* destino, rgba, hsl, arrastre, texto pendiente (destino, base, texto, error) */ }
impl ColorEditor {
    pub fn sync(&mut self, current: Option<(EditTarget, Rgba)>);   // no hace nada con arrastre en curso
    pub fn reset(&mut self);
    pub fn begin(&mut self, control: Control, value: u32) -> bool; // false sin destino o con arrastre en curso
    pub fn drag(&mut self, value: u32);
    pub fn release(&mut self) -> Option<Commit>;           // Some solo si el color cambió respecto al inicio
    pub fn cancel(&mut self);                              // termina el arrastre y restaura; después drag/release no hacen nada
    pub fn dragging(&self) -> bool;
    pub fn dragging_control(&self) -> Option<Control>;     // el deslizador que se arrastra
    pub fn preview(&self) -> Option<(EditTarget, Rgba)>;   // provisional mientras dure y no esté cancelado
    pub fn shown(&self) -> Option<(Rgba, Hsl)>;            // provisional o guardado; None sin destino (tras `reset`)
    pub fn type_hex(&mut self, target: EditTarget, base: Rgba, text: String);
    pub fn hex_display(&self) -> Option<String>;           // el texto pendiente, si lo hay
    /// Intro (`blur` falso) o pérdida del foco (`blur` verdadero). Nada pendiente: `Nothing`.
    /// Válido: borra lo pendiente (`Unchanged` si es el color base, si no `Commit`).
    /// Inválido: con Intro conserva texto y aviso; con `blur` borra el texto (revierte).
    pub fn confirm_hex(&mut self, mode: Mode, blur: bool) -> HexOutcome;
    pub fn escape_hex(&mut self);                          // borra lo pendiente, sin aviso
    /// Las dos líneas del aviso, **con** el prefijo `aviso: ` ya puesto (vacías si no hay aviso).
    pub fn notice(&self, transparent: bool) -> [String; 2];
}
```

Textos del aviso de hexadecimal (todos tras `aviso: `): `Empty` «escribe un color», `Length` «usa 3, 4, 6 u 8 dígitos», `Digit` «solo dígitos 0-9 y a-f», `NotGray` «en modo gris: r = g = b», `NoAlpha` «este modo no tiene alfa». Orden: `Empty`, `Length`, `Digit` (el de `from_hex`: la longitud antes que los dígitos), después `NoAlpha` y `NotGray`. En Gray, `NoAlpha` solo si el texto escribe un alfa distinto de 255 (4 u 8 dígitos).

**`px-ui/src/edit_layout.rs`** (nuevo, sin egui): las constantes de la sección anterior y

```rust
pub fn editor_height_u(mode: Mode) -> i64;                          // 186, 112, 94
pub struct EditorLayout { pub top_px: i64, pub reserved_px: i64 }   // reservado = alto − top
pub fn panel_layout(mode: Mode, panel_h_px: i64, n: u32, offset_px: i64) -> PanelLayout;  // sustituye a editor_layout (hito-2-panel-ventana-baja.md)
pub fn control_row_u(mode: Mode, control: Control) -> Option<i64>;  // y desde top_px
pub fn slider_hit(mode: Mode, x_px: i64, y_px: i64, n: u32) -> Option<Control>;     // relativo a top_px y al panel
pub fn field_y_u(mode: Mode) -> i64;                                // campo hexadecimal, desde top_px
pub fn notice_y_u(mode: Mode) -> i64;                               // primera línea del aviso
pub fn edges_u(mode: Mode) -> Vec<i64>;                             // todas las x e y de la maqueta, en u
```

**`px-ui/src/palette_grid.rs`:** `visible_rows_reserved(panel_h_px, reserved_px, n)`; `visible_rows` la llama con 0. **`px-ui/src/shortcuts.rs`:** `gate_keys(events: &[Event], wants_keyboard: bool) -> Vec<Event>`. **`px-ui/src/texture.rs`:** `to_rgba8_with_entry`. **`px-ui/src/canvas.rs`:** `preview_entry`, `end_preview`, `preview_uploads` y la guarda del teclado (decisión 11). **`px-ui/src/palette_panel.rs`:** `show` se conserva tal cual (equivale a no tener editor) y se añade

```rust
pub struct PanelOutput {
    pub choice: Option<ColorChoice>,
    pub commit: Option<Commit>,
    /// Aviso de 5 s para la barra de estado (hexadecimal inválido al perder el foco).
    pub notice: Option<String>,
    /// Hay destino pero el panel es demasiado bajo para el editor.
}
pub fn show_editing(ui, image, choice, scroll, enabled, slots: &SlotColors, editor: &mut ColorEditor) -> PanelOutput;
```

que comparten; el editor se pinta en una función aparte (`color_editor.rs`, egui). **`px-ui/src/theme.rs`:** sin colores nuevos.

**`px-app`:** `Session` gana `slots: SlotBank` y `editor: ColorEditor`; `sample` los crea; un `open` con éxito reinicia `editor` (**no** `slots`); `active_pixel()`, `sync_editor()` y `commit_edit(Commit) -> Result<bool, DocumentError>` (entrada: `apply(SetPaletteColor)`; ranura: `slots.get_mut(modo).set`, `Ok(false)`). `px-app/src/status_error.rs` (retirado el 2026-10-18 con el aviso de «editor oculto»: el error del lienzo gana al aviso con un simple `or`). `main.rs`: `busy = tools.busy() || editor.dragging()`; los atajos reciben `gate_keys(..)`; el lienzo y las herramientas usan `canvas_input_with(.., session.active_pixel(), ..)`; tras el panel, la confirmación pasa por `commit_edit` y `finish`; si no, `preview_entry` con `editor.preview()` o `end_preview`; `PanelOutput.notice` va a `notify`.

## Casos límite

`F`: indexada 4×1, paleta `[R, G, B, K]`, píxeles `[0,1,2,3]`, sin transparente (la de `hito-2-paleta-editar.md`). Medidas con `n` = 1, 2 y 3 salvo que se diga. Son 19 filas (más de las 12 de partida: el visto bueno pidió una por cada caso de alfa del hexadecimal y las de teclado y ventana baja).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | **Destino.** `EditTarget::of` sobre `F`: `Entry(1)`, `Slot(3)`, `Entry(4)`, `Slot(5)`; sobre RGBA: `Slot(3)`, `Slot(0)`, `Slot(10)`, `Entry(0)`; sobre Gray: `Slot(9)`. Y `commit_edit(Commit { Entry(9), c })` sobre `F` | `Entry(1)`; `Entry(2)`; `None`; `None`; `Slot(3)`; `None`; `None`; `None`; `Slot(9)`. El `commit` da `Err(PaletteEdit(IndexOutOfRange { index: 9, len: 4 }))`, documento intacto |
| 2 | **Valores y textos.** `(255,128,0,255)`; `(0,0,0,0)`; `(255,255,255,255)`; gris 96 en Gray; `(255,128,0,128)`; cabeceras de `Entry(12)`, `Slot(3)` en RGBA y `Slot(4)` en Gray; los textos más largos con `MissingGlyphPolicy::Panic` a 1, 1,25, 1,5, 2 y 3 | R 255, G 128, B 0, A 255, H 30, S 100, L 50 (`  0` con relleno); todo 0 con S 0 y L 0; L 100; G 96 y `#606060`; hex `#ff800080`; `editar índice 12`, `editar color 3`, `editar color 4 gris`. Sin pánico y ninguna línea supera las 32 columnas (`aviso: entrada transparente` son 27, `editar color 4 gris` 19) |
| 3 | **Deslizadores y maqueta.** `slider_value_at` con ancho `184n` y máx. 255: `x` = −5, 0, 92n, `184n − 1`, `184n + 9`; con máx. 359 y 100 en `x = 92n`; `slider_x` de 0 y de `max`; ida y vuelta con máx. 100. `editor_layout` con alto 700n (todos los modos), 240n, 241n y 0; `visible_rows_reserved(h, 0, n) == visible_rows(h, n)`. Bordes a ppp 1,0, 1,25, 1,5, 2,0 y 3,0 | 0; 0; 128 (n = 1; `92·255/183`); 255; 255. Máx. 359: 180; máx. 100: 50. `slider_x(0) = 0`, `slider_x(max) = 184n − 1`, monótonas, y con máx. 100 `value_at(slider_x(v)) == v`. Alto 700n: `top_px` = 514n (indexadas y RGBA), 588n (GrayAlpha), 606n (Gray), reservado 186n, 112n y 94n; con alto 700n + 3 (n = 3: no múltiplo) `top_px` sigue siendo múltiplo de `n`. 240n: `None`; 241n: `Some` y `visible_rows_reserved` da 1; 0: `None`. Todo borde en puntos × `ppp` es entero (±0,001), sin números repetidos fuera de `edit_layout` |
| 4 | **Un arrastre, un paso.** Sobre `F` con la entrada 1 (G): `begin(Red, 255)`, `drag(128)`, `drag(255)`, `release`; antes del `release`, `state_id`, `can_undo`, `history_bytes` y la imagen; después `commit_edit`; `undo`; `redo`. Los bytes de `to_rgba8_with_entry(F, 1, amarillo)` | El provisional va de `(255,255,0,255)` a `(128,255,0,255)` y otra vez a amarillo; el documento **no cambia** hasta soltar. `release` da `Commit { Entry(1), (255,255,0,255) }`; `Ok(true)`, paleta `[R,Y,B,K]` y píxeles iguales (la fila 1 de la unidad 10); un solo `undo` restaura `F` exacta y `redo` deja `[R,Y,B,K]`. Los bytes son los de `to_rgba8(F)` salvo el píxel 1, amarillo |
| 5 | **Sin paso.** Sobre `F`, entrada 0 (R): `begin(Red, 255)` y `release`; `begin(Red, 0)`, `drag(200)`, `drag(255)`, `release`; `begin(Green, 90)`, `drag(10)`, `cancel`, luego `drag(200)` y `release`; igual con foco de la ventana perdido; `begin` sin destino; `begin` con otro arrastre en curso; `drag` y `release` sin `begin`; y `begin(Hue, 100)` sobre un gris `(128,128,128,255)` y `release` | `release` da `None` en el primero (mismo valor) y en el segundo (vuelve a 255); tras `cancel` el editor queda como al empezar (`Rgba` y `Hsl` exactos, `preview()` `None`) y `drag`/`release` posteriores no hacen nada; `begin` devuelve `false` en los dos casos; `drag` y `release` sueltos no hacen nada. El gris con matiz 100: `release` da `None` (el `Rgba` no cambió) pero `hsl.h()` queda en 100 y se muestra H 100. En todos, ningún `apply`: `state_id`, `can_undo`, `can_redo` y la textura (`end_preview`) como estaban |
| 6 | **`Hsl` propio.** `(0,128,255,255)` (H 208, S 100, L 50): arrastrar S a 0 y soltar; después otro arrastre de S a 100. Igual con L a 0 y a 100 y de vuelta a 50. Desde el mismo color: arrastrar R a 128 y B a 128 (dos arrastres). Seleccionar otra entrada y volver. Deshacer el cambio de color. Un trazo del lápiz y su `undo` | S a 0: `(128,128,128,255)` con H 208 mostrado; S a 100: `(0,128,255,255)` otra vez. L a 0: negro y L a 50: el original; L a 100: blanco y de vuelta, el original. R y B a 128: gris `(128,128,128,255)` con `hsl.h() == 240` (el matiz de `(128,128,255)`, el color intermedio tras el primer arrastre, no el 208 original) y S 0. Al volver a la entrada, `Hsl::from_rgba` (un gris da H 0: pérdida esperada, la decisión 8). Tras deshacer el color, `from_rgba` del color restaurado. Tras el trazo y su `undo`, el `Hsl` **no** se recalcula |
| 7 | **Hexadecimal de 3 o 6 dígitos: conserva el alfa.** `parse_hex` con `current = (1,2,3,128)` de `#f80`, `f80`, `#ff8800`, `ff8800`, `  #FF8800  `, `#fF8800`; con `current` de alfa 255; en Gray `#808080` con `current` gris de alfa 255; en GrayAlpha `#808080` con `current` alfa 64. Sobre `F` con la entrada 0 puesta a `(255,0,0,128)`, teclear `#00ff00` y confirmar | `(255,136,0,128)` los seis primeros; con alfa 255: `(255,136,0,255)`; Gray `(128,128,128,255)`; GrayAlpha `(128,128,128,64)`. El `Commit` es `Entry(0)`, `(0,255,0,128)` y un paso |
| 8 | **Hexadecimal de 4 u 8 dígitos: fija el alfa escrito.** `parse_hex` con `current = (1,2,3,128)` de `#f808`, `f80f`, `#ff880080`, `ff8800ff`, `#ff880000`; en GrayAlpha `#80808010`; en Gray `#808080ff` y `#80808080`. Sobre `F` con la entrada 0 en `(255,0,0,128)`, teclear `#00ff00ff` y confirmar | `(255,136,0,136)`, `(255,136,0,255)`, `(255,136,0,128)`, `(255,136,0,255)`, `(255,136,0,0)`; GrayAlpha `(128,128,128,16)`; Gray `(128,128,128,255)` y `Err(NoAlpha)`. El `Commit` es `(0,255,0,255)`: el alfa pasa de 128 a 255 |
| 9 | **Confirmar.** Sobre `F`, entrada 2 (B): teclear `#00ff00` e Intro; teclear `#0000ff` (igual al actual) e Intro; teclear y pulsar `Escape`; teclear `#00ff00` y elegir la muestra 3 en el mismo fotograma, perdiendo el foco; sin teclear nada, enfocar y salir | `HexOutcome::Commit(Entry(2), (0,255,0,255))` y un paso; `Unchanged` (sin paso); con `Escape` el texto vuelve a `#0000ff`, sin paso ni aviso; con el cambio de muestra se aplica a la entrada 2 (el destino original), no a la 3; la última: `Nothing`, sin paso |
| 10 | **Hexadecimal inválido (análisis).** `parse_hex` de ``, `   `, `#`; `#12345`; `#12g45f`; `ñññ`; `#ff0000ff0`; `ff 00 00`; en Gray `#ff0000` y `#80808080`; en GrayAlpha `#ff0000`; en indexadas `#ff0000` | `Empty` (las tres); `Length(5)`; `Digit`; `Digit`; `Length(9)`; `Digit` (el espacio); `NotGray`; `NoAlpha`; `NotGray`; `Ok`. Los `hex_notice` caben en 25 caracteres y están en `INTERFACE_CHARSET` |
| 11 | **Inválido: Intro, foco y `Escape`.** Con el campo enfocado y `zzz` escrito: Intro; luego teclear otra letra; luego Intro y perder el foco con un clic fuera; en otro caso, `zzz` y `Escape`; y `zzz` con perder el foco (`Tab`) | Con Intro: `Invalid(Digit)`, el campo **mantiene el foco y el texto**, el editor muestra `aviso: solo dígitos 0-9 y a-f` y el documento no cambia; al teclear, el aviso desaparece. Perder el foco: el texto vuelve a `hex_text(color actual)`, `PanelOutput.notice` es `Some("solo dígitos 0-9 y a-f")` (la barra lo muestra 5 s con `aviso: `) y no hay paso. `Escape`: texto revertido, `notice` `None`, sin aviso en el editor. En ningún caso se aplica nada |
| 12 | **Alfa y extremos.** Sobre `F`, entrada 0: `begin(Alpha, 0)`; `begin(Alpha, 255)`; `begin(Alpha, 128)`; valores por encima del máximo (`apply_control` con 999 en cada control); cada control en 0 y en su máximo desde `(255,0,0,255)`. GrayAlpha: `Gray` 0 y 255, `Alpha` 0 | A 0: `(255,0,0,0)` y hex `#ff000000`, se confirma con `SetPaletteColor` (el píxel 0 pasa a verse con alfa 0, no es la transparente); 255: `Commit` ninguno (es el valor de partida); 128: `#ff000080`. 999 se limita al máximo de cada control (255, 359, 100). Extremos: R 0 `(0,0,0)`, G 255 `(255,255,0)`, B 255 `(255,0,255)`, H 0 y 359, S 0 `(128,128,128)` y 100, L 0 y 100 (negro y blanco), siempre con `hsl.to_rgba(a) == rgba`. GrayAlpha: `(0,0,0,255)` y `(255,255,255,255)`; A 0 no cambia el gris |
| 13 | **Transparente y duplicadas.** (a) `[A, A, A]`, píxeles `[0,1,2]`, elegida la 1: arrastrar R; (b) `[R, G, (0,0,255,255)]` con transparente `Some(2)` y la 2 elegida: teclear `#090909` y confirmar | (a) `Ok(true)` y la paleta `[A, A', A]`; en la vista previa y tras confirmar, los píxeles 0 y 2 conservan los bytes de `A` (se sustituye por índice); `undo` la restaura. (b) `Ok(true)`: guardado `(9,9,9,255)`, **resuelto `(9,9,9,0)`**; el campo muestra `#090909`; el aviso es `aviso: entrada transparente` / `se guarda con alfa 0`; `to_rgba8_with_entry` da alfa 0 en el píxel 2 |
| 14 | **Deshacer y rehacer con el editor abierto.** Sobre `F`, elegida la entrada 1 (G, H 120): confirmar amarillo; `undo`; `redo`; con un arrastre en curso, `Ctrl+Z` | Tras cada uno `choice == Entry(1)`. Tras el `undo`: valores R 0, G 255, B 0, H 120 (recalculado, color cambiado); tras el `redo`: R 255, G 255, B 0, H 60. Con el arrastre en curso el `undo` se ignora (`busy`) y el documento y el arrastre siguen como estaban |
| 15 | **Ranuras por modo.** RGBA: elegir `Slot(3)`, arrastrar G a 255 y soltar; un trazo del lápiz; `modified()`, `can_undo` y `state_id`; `cell_label(2)`. `Slot(9)` (por defecto `(0,0,0,0)`): A a 255. Gray: `Slot(5)` (gris 128) con G a 200. Después: abrir un PNG de otro modo, volver a abrir uno RGBA; guardar; `save_as` | `bank.get(Rgba).get(3) == (255,255,0,255)`, **ningún** paso ni cambio de `state_id`, y `modified()` queda como estaba; el trazo escribe `Pixel::Rgba((255,255,0,255))`; `color 3 #ffff00`. La 9: `Pixel::Rgba((0,0,0,255))`. Gray: `Pixel::Gray(200)`. Tras abrir otros ficheros, guardar y guardar como, **las tres tablas siguen como se dejaron** (la de RGBA con la 3 amarilla y la 9 opaca, la de Gray con la 5 en 200), `choice` y `scroll` se reinician y `editor` queda reiniciado |
| 16 | **Teclear `ff00ab` en el campo.** Con el campo enfocado (herramienta lápiz, `Entry(1)`, vista con zoom ×8): eventos `Text` y `Key` de `f`, `f`, `0`, `0`, `a`, `b`; y `Home`, `+`, `-` | `gate_keys(eventos, true)` da vacío: ni `tool_action_for_key` (la `f` no elige relleno) ni `action_for_key` (el `0` y los números no cambian `choice`) ven nada; `Canvas::interact_ordered` no cambia la vista con `Home`, `+` ni `-`; el texto del campo es `#ff00ab` (con `#`) o `ff00ab` según lo escrito, sin tocar el color hasta confirmar |
| 17 | **`Ctrl+Z` con el campo enfocado y volver.** Documento con un trazo deshacible; campo enfocado: `Ctrl+Z`, `Ctrl+Y`, `Ctrl+S`; Intro; luego `Ctrl+Z`, la tecla 3 y `Home` con el campo ya sin foco | Con foco: `gate_keys` los deja fuera, `can_undo`, `can_redo`, `state_id` y el fichero no cambian. Sin foco (tras Intro, `Escape`, clic fuera o `Tab`) `gate_keys(eventos, false)` los devuelve **todos, en el mismo orden** y `Ctrl+Z` deshace el trazo, la 3 elige la ranura y `Home` reencuadra |
| 18 | **Ventana baja.** Sustituida por los casos de [`hito-2-panel-ventana-baja.md`](hito-2-panel-ventana-baja.md) (B sin ocultar el editor, mínimo de ventana, texto pendiente y mostrar la elegida) | Ver esa página. Ya no existen `status_error`, `editor_hidden` ni `EDITOR_HIDDEN_NOTICE` |
| 19 | **Vista previa: solo con cambios y restaurar.** Sobre `F`, entrada 1: `preview_entry` con amarillo tres fotogramas seguidos, luego con `(128,255,0,255)`, de nuevo amarillo; en una entrada transparente un color que solo cambia el RGB; soltar con `Ok(true)`; en otro caso, cancelar con `Escape`; arrastre cuyo valor final es el inicial | `preview_uploads` sube 1 con los tres fotogramas iguales, +1 con el color nuevo y +1 al volver; tras soltar, la textura es la del documento (`refresh`); tras cancelar o terminar sin paso, `end_preview` la restaura (bytes iguales a `to_rgba8` del documento); sin `preview_entry` previo, `end_preview` no sube nada. En la transparente, el color resuelto no cambia de alfa: sigue alfa 0 en el píxel |

## Propiedades (proptest)

1. **Conversión de controles.** Para cualquier modo, color base con su `Hsl` coherente (`hsl.to_rgba(a) == rgba`), control y valor `u32` cualquiera (también `u32::MAX`): sin pánico; el resultado solo cambia lo que dice el control (R, G, B o gris: ese canal, o los tres con el gris; alfa: solo `a`; H, S y L: el alfa no cambia); `hsl'.to_rgba(rgba'.a) == rgba'` siempre; en Gray y GrayAlpha `r == g == b`; el resultado depende solo de `(base, control, valor)`, no del camino; y `slider_value_at` es monótona, llega a 0 y al máximo, y `slider_x(slider_value_at(x))` queda a no más de `⌈(w − 1)/max⌉` píxeles de `x`. **Estabilidad:** bajar S a 0 (o L a 0, o L a 100), soltar y volver a su valor inicial devuelve el `Rgba` de partida.
2. **Máquina del arrastre.** Para cualquier secuencia de `begin`, `drag`, `release`, `cancel` y `sync` (con destinos y colores aleatorios, también inexistentes): `release` solo da `Some` tras un `begin` aceptado y no cancelado, **como mucho una vez por arrastre**, con `color` igual al último provisional y distinto del inicial; `cancel` restaura `Rgba` y `Hsl` exactos del inicio y deja `preview() == None`; tras `cancel` y hasta el `release` no hay `Commit` ni provisional; `preview()` es `Some` exactamente con arrastre vivo; `sync` durante un arrastre no cambia nada; y el `Hsl` solo se recalcula en los casos de la decisión 8.
3. **Hexadecimal.** Para cualquier `Rgba` `c` y modo que lo admita: `parse_hex(hex_text(c), modo, c)` es `Ok(c)` (en Gray y GrayAlpha, para los grises que caben). Para cualquier texto (Unicode, hasta 12 caracteres, con o sin `#` y con espacios) y cualquier `current`: sin pánico; `Ok(c)` si y solo si lo da un oráculo escrito en el test, **que no llama a `Rgba::from_hex`** (recorta, quita `#`, comprueba 3, 4, 6 u 8 dígitos ASCII, los lee a mano, usa el alfa de `current` con 3 o 6 dígitos y el escrito con 4 u 8) y cumple las reglas del modo; todo `Err` tiene un aviso de como mucho 25 caracteres y todos de `INTERFACE_CHARSET`.
4. **Vista previa y pasos.** (a) Para cualquier imagen indexada de hasta 8×8, paleta de 1 a 16 entradas (con duplicadas y con o sin transparente), `index` y `color`: `to_rgba8_with_entry(image, index, color)` es **igual** a `to_rgba8` de una copia con `SetPaletteColor(index, color)` aplicado; la imagen usada no cambia; y los píxeles de otro índice son iguales a `to_rgba8(image)`. (b) Para cualquier secuencia de arrastres (confirmados o cancelados), confirmaciones de hexadecimal (válidas o no) y cambios de selección sobre una `Session` indexada: `state_id` solo cambia en las confirmaciones con `Ok(true)`, el número de `undo` posibles es el de esas confirmaciones, y deshacerlas todas deja la paleta inicial exacta.

## Dónde van las pruebas

`px-ui/tests/color_edit.rs` (filas 1 a 3 en su parte pura, 5 a 10 y 12, y propiedades 1 a 3); `px-ui/tests/edit_layout.rs` (fila 3, maqueta y nitidez, y `visible_rows_reserved`); `px-ui/tests/color_editor.rs` (banco de `egui` copiado de `tests/palette_panel.rs` sin modificarlo: ratón sobre la pista, teclado, foco, Intro, `Escape` y las filas 2, 9, 11, 16, 17 y 18 en su parte de ventana); `px-ui/tests/texture_preview.rs` (filas 4, 13 y 19 y propiedad 4a; `Canvas::preview_entry`); `px-ui/tests/shortcuts_gate.rs` (`gate_keys`, filas 16 y 17 en su parte pura); `px-app/src/session/edit_tests.rs` (filas 1, 4, 13, 14 y 15 y propiedad 4b); `px-app/src/status_error.rs` (fila 18, en su módulo de tests). **Los tests de las unidades anteriores no se tocan**: las funciones de `color.rs`, `ToolState`, `show`, `visible_rows` y `Canvas` conservan firma y resultado. Si alguno no compila o falla, se para y se avisa, sin editarlo. Sin dorados: la interfaz es de diseño propio. Orden, en commits pequeños: spec aprobada; `SlotColors`/`SlotBank`, `_with` y `ToolState`; `color_edit`; `edit_layout`, `visible_rows_reserved` y `gate_keys`; `texture` y `Canvas`; `color_editor` y `show_editing`; `px-app`. Al final: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y `license-auditor`.

## Prueba manual

`cargo run --release -p px-app`, a 1,0, 1,5 y 2,0. Sin capturas de pantalla (`CLAUDE.md`): la comprobación visual es tuya.

1. Con la imagen de muestra (RGBA): al fondo del panel, el editor con `editar color 1`, siete deslizadores con su valor visible y el campo `#000000`; la rejilla ocupa la parte de arriba. Subir R a la mitad: el valor cambia, la muestra 1 cambia mientras arrastras, el lápiz pinta con ese color y la barra de estado dice `color 1 #800000`. No aparece «modificado» en el título ni `Ctrl+Z` lo deshace. Abrir otro PNG y volver a uno RGBA: la ranura 1 sigue en `#800000`.
2. Abrir un PNG indexado: arrastrar G de una entrada; **la imagen cambia mientras arrastras** y, al soltar, un `Ctrl+Z` la devuelve entera (un solo paso) y `Ctrl+Y` la rehace. `Escape` a mitad de un arrastre la deja como estaba.
3. Arrastrar S a 0 y volverla a subir: vuelve el mismo color. Igual con L en los extremos. Llevar un color a gris con R, G y B y mover H: no salta.
4. Escribir en el campo `#f80` y `ff8800` sobre un color con alfa 128: el alfa se conserva; `#ff880080` y `#f80f`: el alfa es el escrito. Confirmar con Intro y con un clic fuera. Escribir `#12` y Intro: aviso en el editor y el campo sigue enfocado; un clic fuera: el texto vuelve al color y la barra avisa 5 s. `Escape` restaura.
5. Teclear `ff00ab` con el campo enfocado: no cambia de herramienta, de color ni de vista. `Ctrl+Z` en el campo no deshace el documento. Al salir del campo, vuelven los atajos.
6. Entrada transparente de un PNG indexado: cambiar su color; el editor dice `aviso: entrada transparente` / `se guarda con alfa 0`. Un PNG con colores repetidos: editar una entrada no cambia las demás.
7. Con un arrastre a medias, probar `Ctrl+Z`, las teclas 1 a 9 y soltar el ratón fuera de la ventana. Cambiar de ventana a medio arrastre cancela.
8. Reducir la altura de la ventana: la rejilla cede sitio al editor, que se queda en el mismo sitio del fondo; la ventana no baja del mínimo y, sin él (`PIXOKO_SIN_MINIMO=1`), todo el panel se desplaza con una barra visible (ver `hito-2-panel-ventana-baja.md`, «Prueba manual»).
9. **Lo no verificado en la spec:** (a) clic fuera del campo (rejilla o lienzo) le quita el foco y confirma o revierte; (b) nitidez del campo a 1,25× y 1,5× (letras sin bordes grises, cursor de 1 píxel); (c) vista previa con un lienzo grande: probar un PNG indexado de 4096×4096 o mayor y ver si el arrastre va a trompicones (límite anotado hasta la unidad 14).

## Fuera de alcance

Añadir, quitar, mover y rotar entradas, fijar el índice transparente y avisar al quitarlo (unidad **11d**); degradados en las pistas, selector de color en 2D, cuentagotas, copiar y pegar colores; **pulido posterior: campos numéricos editables y pasos con el teclado en los deslizadores**; editar varias entradas a la vez; guardar las ranuras en disco; actualizar la textura por regiones (unidad 14); tema claro.
