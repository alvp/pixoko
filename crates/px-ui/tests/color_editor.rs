//! Tests del editor de color dentro de la ventana (specs/hito-2-editar-color.md: filas 2, 9, 11,
//! 16 y 17 de «Casos límite» en su parte de ventana (la 18, ventana baja, la sustituyó
//! specs/hito-2-panel-ventana-baja.md: ver `panel_baja.rs`), y el arrastre de los deslizadores con el
//! ratón). Lo puro (`ColorEditor`, `edit_layout`, `gate_keys` sueltos) se prueba en
//! `color_edit.rs`, `edit_layout.rs` y `shortcuts_gate.rs`; aquí se prueba que `show_editing`
//! lo une bien a `egui`.
//!
//! El banco es el de `tests/palette_panel.rs`, copiado y ampliado (ese fichero no se toca): un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hará la app, se llama a `ColorEditor::sync` con el destino y el color guardado
//! (lo que hará `Session::sync_editor`), se monta `status::show`, luego
//! `palette_panel::show_editing` y por fin el lienzo (`Canvas::interact_ordered` y
//! `Canvas::paint`) en el mismo `Ui` raíz. `enabled` es `!editor.dragging()`, como `busy` en la
//! app.
//!
//! El banco guarda la imagen, la elección y las ranuras, y **nunca aplica** lo que devuelve el
//! panel: no hay `Document` (la confirmación es de `Session`, en `px-app`). Cada test cambia
//! `bench.choice` o `bench.image` cuando quiere simular lo que haría la app. Cada `PanelOutput`
//! se guarda en `bench.history`.
//!
//! Lo pintado se lee de las formas del fotograma (`Shape::Text` y `Shape::Rect`): así se ve qué
//! textos hay en el editor y dónde caen. No hay capturas de pantalla.
//!
//! Los cambios de fuentes, de `ppp` y de la política de glifo ausente se aplican al empezar el
//! pase siguiente: el banco hace dos pases vacíos de calentamiento antes del primer fotograma.
//!
//! # Lo que depende del comportamiento real de `egui`
//!
//! La spec lo deja «a comprobar en el banco». Los tests que lo necesitan llevan en su nombre o su
//! comentario la marca **[DEPENDE DE EGUI]**: si fallan, el fallo es información sobre `egui`, no
//! necesariamente un error de `show_editing`:
//!
//! - un clic fuera del campo (rejilla o lienzo) le quita el foco (con el `egui` por defecto,
//!   `SurrenderFocusOn::Clicks`: el foco se pierde al **soltar**, no al pulsar);
//! - `Tab` saca el foco del campo (y no se lo da al lienzo, que es `Sense::click_and_drag` y por
//!   tanto enfocable: si `egui` se lo da, `egui_wants_keyboard_input()` seguiría siendo cierto, por
//!   eso la guarda usa `text_edit_focused()`, que solo vale con un campo de texto);
//! - `Intro` con el campo enfocado le quita el foco (y con un valor inválido `show_editing` se lo
//!   devuelve);
//! - la nitidez del texto del `TextEdit` a 1,25× y 1,5×;
//! - que `Ctrl+A` selecciona el texto del campo (el banco lo usa para sustituir lo que haya).

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, PointerButton, Pos2, RawInput,
    Rect, Vec2, pos2, vec2,
};
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasInfo, CanvasInput};
use px_ui::color::{ColorChoice, SlotColors};
use px_ui::color_edit::{
    ColorEditor, Commit, Control, EditTarget, HexOutcome, control_value, controls, hex_notice,
    hex_text, slider_value_at, slider_x,
};
use px_ui::edit_layout::{
    FIELD_H_U, FIELD_W_U, FIELD_X_U, HEADER_X_U, HEADER_Y_U, HIT_X0_U, HIT_X1_U, ROW_H_U,
    TRACK_W_U, TRACK_X_U, control_row_u, field_y_u, panel_layout, slider_hit,
};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PANEL_WIDTH_U, PITCH_U, TEXT_WIDTH_U,
};
use px_ui::palette_panel::{self, PanelOutput};
use px_ui::shortcuts::gate_keys;
use px_ui::status::{bar_height_px, columns};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, install_fonts, text_scale};
use px_ui::theme::SURFACE_0;
use px_ui::view::Zoom;

#[path = "common/zones.rs"]
mod zones;
use zones::panel_on_the_left;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BLACK: Rgba = Rgba::opaque(0, 0, 0);
const YELLOW: Rgba = Rgba::opaque(255, 255, 0);

const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Indexada de 64×48 (zoom de ajuste 8 con el banco a ppp 1) con la paleta `[R, G, B, K]` de `F`.
/// Es `F` de la spec con un lienzo más grande.
fn image_f() -> Image {
    let palette = Palette::from_colors([RED, GREEN, BLUE, TEST_BLACK]).expect("paleta");
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

/// Indexada de 64×48 con 256 colores, el último `#ffffff80` y el índice transparente en el 17.
fn image_indexed_256() -> Image {
    let colors_2: Vec<Rgba> = (0..256usize)
        .map(|i| {
            if i == 255 {
                Rgba::new(255, 255, 255, 128)
            } else {
                Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8)
            }
        })
        .collect();
    let mut palette = Palette::from_colors(colors_2).expect("paleta de 256 colores");
    palette
        .set_transparent_index(Some(17))
        .expect("el índice existe");
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

/// Imagen de 64×48 de ese modo (la indexada es la de 256 colores).
fn image_of(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => image_indexed_256(),
        other => Image::new(64, 48, other).expect("imagen válida"),
    }
}

/// Tres elecciones con destino válido para cada modo: la más larga de la cabecera, la
/// transparente (solo indexadas) y una corriente.
fn choices_of(mode: Mode) -> Vec<ColorChoice> {
    match mode {
        Mode::Indexed => vec![
            ColorChoice::Entry(255),
            ColorChoice::Entry(17),
            ColorChoice::Entry(0),
        ],
        _ => vec![
            ColorChoice::Slot(9),
            ColorChoice::Slot(4),
            ColorChoice::Slot(1),
        ],
    }
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

fn key_with(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn key_2(key: Key) -> Event {
    key_with(key, Modifiers::NONE)
}

/// Ctrl en Linux y Windows (`command` es Ctrl ahí).
fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

fn button_2(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn text_2(s: &str) -> Event {
    Event::Text(s.to_owned())
}

/// Las pulsaciones de teclear `string` como las da una ventana: `Key` y `Text` por cada carácter.
fn presses_2_2(string: &str) -> Vec<Event> {
    let mut events_2 = Vec::new();
    for c in string.chars() {
        let key = match c {
            'f' => Key::F,
            'a' => Key::A,
            'b' => Key::B,
            '0' => Key::Num0,
            other => panic!("sin tecla para {other:?} en el test"),
        };
        events_2.push(key_2(key));
        events_2.push(text_2(&c.to_string()));
    }
    events_2
}

/// Punto en píxeles físicos de la ventana a puntos.
fn in_points(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

/// Un texto pintado: contenido, esquina de su `galley` en puntos y ancho en píxeles físicos.
#[derive(Clone, Debug)]
struct Painted {
    text_2: String,
    pos: Pos2,
    width_px: f32,
}

fn collects(shape: &Shape, ppp: f32, texts: &mut Vec<Painted>, rects: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Text(t) => texts.push(Painted {
            text_2: t.galley.text().to_owned(),
            pos: t.pos,
            width_px: t.galley.size().x * ppp,
        }),
        Shape::Rect(r) => rects.push((r.rect, r.fill)),
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects(shape_2, ppp, texts, rects);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

const SCREEN_HIGH: Vec2 = vec2(800.0, 600.0);
const PPP_ALWAYS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

struct Bench {
    ctx: Context,
    canvas: Canvas,
    scroll: palette_grid::Scroll,
    screen: Vec2,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    /// Lo que devolvió `show_editing` en cada fotograma, en orden.
    history: Vec<PanelOutput>,
}

struct TestFrame {
    /// Lo que quedaba libre justo antes del lienzo: pantalla menos la barra y el panel.
    rect_canvas: Rect,
    entries: Vec<CanvasInput>,
    info: CanvasInfo,
    output_2: PanelOutput,
    texts: Vec<Painted>,
    rects: Vec<(Rect, Color32)>,
    /// `text_edit_focused()` justo antes y justo después del fotograma.
    wants_before: bool,
    wants_after: bool,
}

impl TestFrame {
    /// Algún texto pintado es exactamente `searched`.
    fn paints(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2 == searched)
    }

    /// Algún texto pintado contiene `searched`.
    fn paints_something_with(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2.contains(searched))
    }

    /// El texto pintado exactamente igual a `searched`, si lo hay.
    fn text_painted(&self, searched: &str) -> Option<&Painted> {
        self.texts.iter().find(|t| t.text_2 == searched)
    }

    fn there_is_fill(&self, color: Color32) -> bool {
        self.rects.iter().any(|(_, fill)| *fill == color)
    }
}

/// Medidas del editor a la escala del banco, en píxeles físicos.
#[derive(Clone, Copy, Debug)]
struct Geo {
    ppp: f32,
    n: i64,
    mode: Mode,
    top_px: i64,
}

impl Geo {
    /// `y` del centro de la fila del deslizador `control`.
    fn y_row(&self, control: Control) -> i64 {
        let row_2 = control_row_u(self.mode, control).expect("el modo tiene ese deslizador");
        self.top_px + row_2 * self.n + ROW_H_U * self.n / 2
    }

    /// `x` del inicio de la pista y su largo.
    fn x_hint(&self) -> i64 {
        TRACK_X_U * self.n
    }

    fn width_hint(&self) -> i64 {
        TRACK_W_U * self.n
    }

    /// `x` (absoluta) de la posición de la pista que da `value` de un máximo `max`.
    fn x_of_value(&self, value: u32, max: u32) -> i64 {
        self.x_hint() + slider_x(value, self.width_hint(), max)
    }

    /// Valor que da un punto a `dx` píxeles del inicio de la pista.
    fn value_in(&self, dx: i64, control: Control) -> u32 {
        slider_value_at(dx, self.width_hint(), control.max())
    }

    /// Centro del campo hexadecimal.
    fn field_2(&self) -> (i64, i64) {
        (
            FIELD_X_U * self.n + FIELD_W_U * self.n / 2,
            self.top_px + field_y_u(self.mode) * self.n + FIELD_H_U * self.n / 2,
        )
    }
}

/// Centro de la muestra `index_2` en píxeles físicos, con la primera fila visible `first_row`.
fn center_of_swatch(n: i64, index_2: usize, first_row: usize) -> (i64, i64) {
    let column = (index_2 % COLUMNS) as i64;
    let row_2 = (index_2 / COLUMNS) as i64 - first_row as i64;
    (
        (GRID_X_U + column * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + row_2 * PITCH_U) * n + CELL_U * n / 2,
    )
}

impl Bench {
    /// Contexto de `screen` puntos a `ppp`, con la fuente instalada, la política `Panic` para
    /// el glifo ausente, `image` en el lienzo y dos fotogramas de reposo.
    fn new_2(ppp: f32, screen: Vec2, image: Image, choice: ColorChoice) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut bench = Bench {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            screen,
            slots: SlotColors::defaults(image.mode()),
            image,
            choice,
            editor: ColorEditor::default(),
            history: Vec::new(),
        };
        for _ in 0..2 {
            let output_2 = bench.ctx.run_ui(bench.input_2(vec![]), |ui| {
                apply_scale(ui.ctx());
            });
            output_2.drop_without_applying_deltas();
        }
        bench.canvas.set_image(&bench.image);
        bench.rest_2_2();
        bench.rest_2_2();
        bench.history.clear();
        bench
    }

    /// Banco con un panel de exactamente `panel_h_px` píxeles físicos de alto (la ventana menos
    /// la barra de estado).
    fn with_panel_height(ppp: f32, panel_h_px: i64, image: Image, choice: ColorChoice) -> Bench {
        Bench::new_2(ppp, screen_for_panel(ppp, panel_h_px), image, choice)
    }

    fn input_2(&self, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            ..RawInput::default()
        }
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ppp()))
    }

    /// Alto del panel en píxeles físicos, redondeado como lo hace el panel.
    fn panel_h_px(&self) -> i64 {
        (self.screen.y * self.ppp()).round() as i64 - i64::from(bar_height_px(self.n() as u32))
    }

    /// Medidas del editor con el destino actual; entra en pánico si no cabe.
    fn geo(&self) -> Geo {
        let mode = self.image.mode();
        let n = self.n();
        let layout = panel_layout(mode, self.panel_h_px(), n as u32, 0);
        assert!(!layout.scrolling, "el bloque cabe sin desplazar");
        Geo {
            ppp: self.ppp(),
            n,
            mode,
            top_px: layout.top_px,
        }
    }

    /// El destino y su color guardado, como los pasa la app a `ColorEditor::sync`.
    fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let target_2 = EditTarget::of(&self.image, self.choice)?;
        let color = target_2.stored(&self.image, &self.slots)?;
        Some((target_2, color))
    }

    /// Un fotograma con los eventos dados: sincroniza el editor, aplica la escala, monta la
    /// barra, el panel con el editor y después el lienzo, como la app. Limpia los cambios de
    /// textura (egui entra en pánico si se sueltan sin aplicar).
    fn frame(&mut self, events: Vec<Event>) -> TestFrame {
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !self.editor.dragging();
        let wants_before = self.ctx.text_edit_focused();
        let input = self.input_2(events);
        let ppp = self.ppp();
        let mut result = None;
        let Bench {
            ctx,
            canvas,
            scroll,
            image,
            choice,
            slots,
            editor,
            ..
        } = self;
        let mut output_2 = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (panel, (rect_canvas, entries, info)) = panel_on_the_left(
                ui,
                |ui| {
                    palette_panel::show_editing(ui, image, *choice, scroll, enabled, slots, editor)
                },
                |ui| {
                    let rect_canvas = ui.available_rect_before_wrap();
                    let entries = canvas
                        .interact_roles(ui)
                        .into_iter()
                        .map(|(input, _)| input)
                        .collect::<Vec<_>>();
                    let info = canvas.paint(ui);
                    (rect_canvas, entries, info)
                },
            );
            result = Some((rect_canvas, entries, info, panel));
        });
        output_2.textures_delta.clear();
        let shapes_2: Vec<ClippedShape> = std::mem::take(&mut output_2.shapes);
        let (rect_canvas, entries, info, panel) = result.expect("el lienzo se dibuja");
        let (mut texts, mut rects) = (Vec::new(), Vec::new());
        for clipped in &shapes_2 {
            collects(&clipped.shape, ppp, &mut texts, &mut rects);
        }
        self.history.push(panel.clone());
        TestFrame {
            rect_canvas,
            entries,
            info,
            output_2: panel,
            texts,
            rects,
            wants_before,
            wants_after: self.ctx.text_edit_focused(),
        }
    }

    fn rest_2_2(&mut self) -> TestFrame {
        self.frame(vec![])
    }

    /// `text_edit_focused()` ahora mismo (el estado tras el último fotograma).
    fn wants_keyboard(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    // --- Ratón ---

    fn moves(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        let pos = in_points(self.ppp(), x_px, y_px);
        self.frame(vec![Event::PointerMoved(pos)])
    }

    fn presses_2(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        let pos = in_points(self.ppp(), x_px, y_px);
        self.frame(vec![button_2(pos, true)])
    }

    fn releases(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        let pos = in_points(self.ppp(), x_px, y_px);
        self.frame(vec![button_2(pos, false)])
    }

    /// Mueve el puntero, pulsa y suelta (un fotograma cada vez).
    fn click(&mut self, x_px: i64, y_px: i64) {
        self.moves(x_px, y_px);
        self.presses_2(x_px, y_px);
        self.releases(x_px, y_px);
    }

    /// Mueve el puntero y luego pulsa y suelta **en el mismo fotograma**.
    fn click_on_a_frame(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        self.moves(x_px, y_px);
        let pos = in_points(self.ppp(), x_px, y_px);
        self.frame(vec![button_2(pos, true), button_2(pos, false)])
    }

    /// Un clic en el centro del lienzo (fuera del panel y del campo).
    fn click_on_the_canvas(&mut self) {
        let center = self.rest_2_2().rect_canvas.center();
        let (x, y) = (
            (center.x * self.ppp()).round() as i64,
            (center.y * self.ppp()).round() as i64,
        );
        self.click(x, y);
    }

    // --- Campo hexadecimal ---

    /// Un clic en el campo hexadecimal y un fotograma de reposo; comprueba que tiene el foco.
    fn focuses_field(&mut self) {
        let (x, y) = self.geo().field_2();
        self.click(x, y);
        self.rest_2_2();
        assert!(
            self.wants_keyboard(),
            "un clic en el campo le da el foco de texto"
        );
    }

    /// `Ctrl+A` (seleccionar todo) en un fotograma. **[DEPENDE DE EGUI]**: así lo que se teclee
    /// sustituye lo que hubiera en el campo.
    fn selects_all(&mut self) {
        self.frame(vec![key_with(Key::A, ctrl())]);
    }

    /// Sustituye el contenido del campo (ya enfocado) por `string`; devuelve el fotograma de
    /// las pulsaciones.
    fn writes(&mut self, string: &str) -> TestFrame {
        self.selects_all();
        self.frame(vec![text_2(string)])
    }

    // --- Lo recogido ---

    fn commits(&self) -> Vec<Commit> {
        self.history.iter().filter_map(|o| o.commit).collect()
    }

    fn notices(&self) -> Vec<String> {
        self.history
            .iter()
            .filter_map(|o| o.notice.clone())
            .collect()
    }

    fn choices(&self) -> Vec<ColorChoice> {
        self.history.iter().filter_map(|o| o.choice).collect()
    }
}

/// Pantalla en puntos para que el panel mida `panel_h_px` píxeles físicos de alto.
fn screen_for_panel(ppp: f32, panel_h_px: i64) -> Vec2 {
    let n = text_scale(ppp);
    vec2(
        800.0,
        (panel_h_px + i64::from(bar_height_px(n))) as f32 / ppp,
    )
}

/// Banco con `F` grande, elegida la entrada `input_2`, a `ppp`.
fn bench_f(ppp: f32, input_2: u8) -> Bench {
    Bench::new_2(ppp, SCREEN_HIGH, image_f(), ColorChoice::Entry(input_2))
}

/// Los colores `(r, g, b, a)` que muestra el editor, si hay.
fn shown(bench: &Bench) -> Rgba {
    bench.editor.shown().expect("el editor muestra un color").0
}

fn value_shown(bench: &Bench, control: Control) -> u32 {
    let (rgba, hsl) = bench.editor.shown().expect("el editor muestra un color");
    control_value(rgba, hsl, control)
}

// ---------------------------------------------------------------------------------------------
// Fila 2: los textos del editor caben en 32 columnas y se maquetan con `Panic`
// ---------------------------------------------------------------------------------------------

/// Textos tecleados para provocar cada aviso del hexadecimal (algunos son válidos según el modo).
const TEXTS_INVALID: [&str; 6] = ["", "#", "12345", "zzz", "#ff0000", "#80808080"];

/// Comprueba que todo lo pintado cabe: ≤ 32 columnas, de `INTERFACE_CHARSET` (salvo lo que ha
/// tecleado el usuario, que aquí es ASCII) y no más ancho que el área de texto del panel.
fn checks_that_fits(f: &TestFrame, ppp: f32, context: &str) {
    let n = text_scale(ppp);
    let maximum_2 = columns((TEXT_WIDTH_U * i64::from(n)) as f32, ppp);
    assert_eq!(maximum_2, 32, "{context}: columns del área de texto");
    for t in &f.texts {
        assert!(
            t.text_2.chars().count() <= maximum_2,
            "{context}: {:?} tiene {} caracteres y caben {maximum_2}",
            t.text_2,
            t.text_2.chars().count()
        );
        for c in t.text_2.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c) || c.is_ascii(),
                "{context}: {:?} lleva {c:?}",
                t.text_2
            );
        }
        assert!(
            t.width_px <= (TEXT_WIDTH_U * i64::from(n)) as f32 + 0.5,
            "{context}: {:?} mide {} px y caben {}",
            t.text_2,
            t.width_px,
            TEXT_WIDTH_U * i64::from(n)
        );
    }
}

#[test]
fn row_02_the_headers_are_those_of_the_spec() {
    assert_eq!(
        EditTarget::Entry(12).header(Mode::Indexed),
        "editar índice 12"
    );
    assert_eq!(EditTarget::Slot(3).header(Mode::Rgba), "editar color 3");
    assert_eq!(
        EditTarget::Slot(4).header(Mode::Gray),
        "editar color 4 gris"
    );
}

#[test]
fn row_02_the_transparent_notice_is_two_lines_and_the_first_27_columns() {
    let editor = ColorEditor::default();
    let [first, second] = editor.notice(true);
    assert_eq!(first, "aviso: entrada transparente");
    assert_eq!(first.chars().count(), 27);
    assert_eq!(second, "se guarda con alfa 0");
}

#[test]
fn row_02_the_longest_texts_are_laid_out_with_panic_and_fit_at_all_ppp() {
    for ppp in PPP_ALWAYS {
        for mode in TEST_MODES {
            let image = image_of(mode);
            for choice in choices_of(mode) {
                let context = format!("ppp {ppp}, {mode:?}, {choice:?}");
                let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image.clone(), choice);
                let target_2 = EditTarget::of(&image, choice).expect("destino válido");
                let base = target_2.stored(&image, &bench.slots).expect("guardado");

                // En reposo: la cabecera y el color del campo, todo dentro de las columnas.
                let f = bench.rest_2_2();
                checks_that_fits(&f, ppp, &context);
                assert!(
                    f.paints(&target_2.header(mode)),
                    "{context}: falta la cabecera {:?}",
                    target_2.header(mode)
                );
                assert!(
                    f.paints(&hex_text(base)),
                    "{context}: falta el campo {:?}",
                    hex_text(base)
                );

                // La transparente: el aviso de dos líneas.
                let transparent = image.palette().is_some_and(|p| {
                    p.transparent_index().map(EditTarget::Entry) == Some(target_2)
                });
                if transparent {
                    assert!(
                        f.paints("aviso: entrada transparente"),
                        "{context}: primera línea del aviso"
                    );
                    assert!(
                        f.paints("se guarda con alfa 0"),
                        "{context}: segunda línea del aviso"
                    );
                }

                // Cada aviso del hexadecimal inválido (el de una línea, `notice: ` + motivo).
                for written in TEXTS_INVALID {
                    bench.editor.type_hex(target_2, base, written.to_owned());
                    let HexOutcome::Invalid(error) = bench.editor.confirm_hex(mode, false) else {
                        bench.editor.escape_hex();
                        continue;
                    };
                    let f = bench.rest_2_2();
                    let context = format!("{context}, {written:?} → {error:?}");
                    checks_that_fits(&f, ppp, &context);
                    let line = format!("aviso: {}", hex_notice(&error));
                    assert!(f.paints(&line), "{context}: falta la línea {line:?}");
                    assert!(line.chars().count() <= 32, "{context}: {line:?} no cabe");
                    bench.editor.escape_hex();
                }
            }
        }
    }
}

#[test]
fn row_02_the_longest_header_and_the_three_digit_value_fit() {
    // `edit índice 255` y los tres valores de 3 columnas, en las dos escalas extremas.
    for ppp in [1.0_f32, 3.0] {
        let mut bench = Bench::new_2(
            ppp,
            SCREEN_HIGH,
            image_indexed_256(),
            ColorChoice::Entry(255),
        );
        let f = bench.rest_2_2();
        assert!(f.paints("editar índice 255"), "ppp {ppp}");
        checks_that_fits(&f, ppp, &format!("ppp {ppp}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 13 (parte de ventana): la transparente muestra el color guardado
// ---------------------------------------------------------------------------------------------

#[test]
fn row_13b_the_transparent_shows_the_stored_one_and_the_two_line_notice() {
    // [R, G, (0,0,255,255)] con transparente Some(2) y la 2 elegida.
    let mut palette = Palette::from_colors([RED, GREEN, BLUE]).expect("paleta");
    palette.set_transparent_index(Some(2)).expect("existe");
    let image = Image::new_indexed(64, 48, palette).expect("imagen");
    let mut bench = Bench::new_2(1.0, SCREEN_HIGH, image, ColorChoice::Entry(2));
    let f = bench.rest_2_2();
    assert!(f.paints("#0000ff"), "el campo muestra el color guardado");
    assert!(
        !f.paints("#0000ff00"),
        "el campo no muestra el resuelto (con alfa 0)"
    );
    assert!(f.paints("aviso: entrada transparente"));
    assert!(f.paints("se guarda con alfa 0"));
}

// ---------------------------------------------------------------------------------------------
// Maqueta: dónde se pinta el editor (a los cinco ppp)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_editor_anchors_to_the_bottom_and_does_not_move_with_the_number_of_entries() {
    for ppp in PPP_ALWAYS {
        let n = i64::from(text_scale(ppp));
        let mut few = bench_f(ppp, 1);
        let mut many = Bench::new_2(ppp, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(1));
        let geo = few.geo();
        let expected_2 = (
            (HEADER_X_U * n) as f32,
            (geo.top_px + HEADER_Y_U * n) as f32,
        );
        for (name_2, bench) in [("4 entradas", &mut few), ("256 entradas", &mut many)] {
            let f = bench.rest_2_2();
            let header = f
                .text_painted("editar índice 1")
                .unwrap_or_else(|| panic!("ppp {ppp}, {name_2}: sin cabecera"));
            let (x, y) = (header.pos.x * ppp, header.pos.y * ppp);
            assert!(
                (x - expected_2.0).abs() <= 0.05 && (y - expected_2.1).abs() <= 0.05,
                "ppp {ppp}, {name_2}: cabecera en ({x}, {y}) px y se esperaba {expected_2:?}"
            );
        }
    }
}

#[test]
fn the_hexadecimal_field_is_painted_where_edit_layout_says_with_the_background_surface_0() {
    for ppp in PPP_ALWAYS {
        let mut bench = bench_f(ppp, 2);
        let geo = bench.geo();
        let n = geo.n;
        let f = bench.rest_2_2();
        let expected_2 = Rect::from_min_size(
            pos2(
                (FIELD_X_U * n) as f32 / ppp,
                (geo.top_px + field_y_u(geo.mode) * n) as f32 / ppp,
            ),
            vec2((FIELD_W_U * n) as f32 / ppp, (FIELD_H_U * n) as f32 / ppp),
        );
        let tolerance = 0.05 / ppp;
        assert!(
            f.rects.iter().any(|(rect, fill)| {
                *fill == SURFACE_0
                    && (rect.min.x - expected_2.min.x).abs() <= tolerance
                    && (rect.min.y - expected_2.min.y).abs() <= tolerance
                    && (rect.width() - expected_2.width()).abs() <= tolerance
                    && (rect.height() - expected_2.height()).abs() <= tolerance
            }),
            "ppp {ppp}: ningún rectángulo SURFACE_0 en {expected_2:?}"
        );
    }
}

/// **[DEPENDE DE EGUI]** Nitidez del `TextEdit` a 1,25× y 1,5× (spec: «no comprobado»): el texto
/// del campo cae en píxeles físicos enteros, también con el campo enfocado.
#[test]
fn sharpness_the_field_text_falls_on_whole_physical_pixels() {
    for ppp in PPP_ALWAYS {
        let mut bench = bench_f(ppp, 2);
        for focused in [false, true] {
            if focused {
                bench.focuses_field();
            }
            let f = bench.rest_2_2();
            let field_2 = f
                .text_painted("#0000ff")
                .unwrap_or_else(|| panic!("ppp {ppp}: el campo no se pinta"));
            for (axis, value) in [("x", field_2.pos.x), ("y", field_2.pos.y)] {
                let physical = f64::from(value) * f64::from(ppp);
                // egui redondea el diseño a 1/32 de punto (`GUI_ROUNDING`): la forma pintada puede
                // quedar a menos de 0,03 px de un píxel físico; el teselado redondea el texto al
                // píxel (`round_text_to_pixels`), así que se pinta nítido.
                assert!(
                    (physical - physical.round()).abs() <= 0.05,
                    "ppp {ppp}, enfocado {focused}: el texto del campo cae en {axis} = {physical} px"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: flujo con ventana (teclear, Intro, Escape, cambio de muestra)
// ---------------------------------------------------------------------------------------------

#[test]
fn row_09_at_rest_the_field_shows_the_stored_color() {
    let mut bench = bench_f(1.0, 2);
    let f = bench.rest_2_2();
    assert!(f.paints(&hex_text(BLUE)), "el campo muestra #0000ff");
    assert_eq!(bench.editor.hex_display(), None);
}

/// **[DEPENDE DE EGUI]** (Intro quita el foco al `TextEdit`).
#[test]
fn row_09_typing_and_enter_confirms_a_color_on_the_target_and_the_field_loses_focus() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();

    let f = bench.writes("#00ff00");
    assert_eq!(f.output_2.commit, None, "teclear no confirma");
    assert_eq!(bench.editor.hex_display(), Some("#00ff00".to_owned()));
    assert!(bench.commits().is_empty());

    let f = bench.frame(vec![key_2(Key::Enter)]);
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Entry(2),
            color: GREEN
        }),
        "Intro confirma"
    );
    assert_eq!(f.output_2.notice, None);
    assert_eq!(bench.editor.hex_display(), None, "lo pendiente se borra");

    // Un solo commit, y el campo ya no tiene el foco.
    bench.rest_2_2();
    bench.rest_2_2();
    assert_eq!(bench.commits().len(), 1);
    assert!(
        !bench.wants_keyboard(),
        "tras Intro el campo pierde el foco"
    );
}

#[test]
fn row_09_typing_the_current_color_and_enter_gives_no_step() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("#0000ff");
    let f = bench.frame(vec![key_2(Key::Enter)]);
    assert_eq!(f.output_2.commit, None, "es el color actual: Unchanged");
    assert_eq!(f.output_2.notice, None);
    bench.rest_2_2();
    assert!(bench.commits().is_empty());
    assert_eq!(bench.editor.hex_display(), None);
}

#[test]
fn row_09_escape_reverts_the_text_without_a_step_or_notice() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("#00ff00");
    assert_eq!(bench.editor.hex_display(), Some("#00ff00".to_owned()));

    let f = bench.frame(vec![key_2(Key::Escape)]);
    assert_eq!(f.output_2.commit, None);
    assert_eq!(f.output_2.notice, None);
    assert_eq!(bench.editor.hex_display(), None);
    let f = bench.rest_2_2();
    assert!(f.paints("#0000ff"), "el texto vuelve a #0000ff");
    assert!(!f.paints("#00ff00"));
    assert!(bench.commits().is_empty());
    assert!(bench.notices().is_empty());
    assert!(
        !bench.wants_keyboard(),
        "Escape saca el foco del campo (egui lo hace él)"
    );
}

/// **[DEPENDE DE EGUI]** Perder el foco por un clic en la rejilla en el **mismo fotograma** que la
/// elección de la muestra: la confirmación va al destino original.
#[test]
fn row_09_a_swatch_change_in_the_same_frame_applies_to_the_original_target() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("#00ff00");
    let n = bench.n();
    let (x, y) = center_of_swatch(n, 3, 0);

    let f = bench.click_on_a_frame(x, y);
    assert_eq!(
        f.output_2.choice,
        Some(ColorChoice::Entry(3)),
        "el clic elige la muestra 3"
    );
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Entry(2),
            color: GREEN
        }),
        "el color se aplica a la entrada 2, no a la 3"
    );

    // La app cambia la elección; no vuelve a haber commits y el editor apunta a la entrada 3.
    bench.choice = ColorChoice::Entry(3);
    for _ in 0..3 {
        let f = bench.rest_2_2();
        assert_eq!(f.output_2.commit, None);
    }
    assert_eq!(bench.commits().len(), 1);
    assert_eq!(bench.editor.hex_display(), None);
}

/// **[DEPENDE DE EGUI]** Lo mismo con el foco perdido al **soltar** el botón (el `egui` por
/// defecto pierde el foco al hacer clic, no al pulsar): entre medias la app ya eligió la muestra 3
/// y `sync` ve otro destino. La confirmación sigue yendo al destino original.
#[test]
fn row_09_a_swatch_change_before_losing_focus_applies_to_the_original_target() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("#00ff00");
    let n = bench.n();
    let (x, y) = center_of_swatch(n, 3, 0);
    bench.history.clear();

    bench.moves(x, y);
    let f = bench.presses_2(x, y);
    assert_eq!(f.output_2.choice, Some(ColorChoice::Entry(3)));
    bench.choice = ColorChoice::Entry(3);
    bench.releases(x, y);
    bench.rest_2_2();
    bench.rest_2_2();

    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Entry(2),
            color: GREEN
        }],
        "un solo commit, a la entrada 2 con el verde tecleado"
    );
    assert!(bench.notices().is_empty());
    assert_eq!(bench.editor.hex_display(), None);
    assert_eq!(bench.choices(), vec![ColorChoice::Entry(3)]);
}

/// **[DEPENDE DE EGUI]** Un clic fuera del campo le quita el foco y confirma el texto válido.
#[test]
fn row_09_a_click_outside_confirms_the_valid_text() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("#00ff00");
    bench.history.clear();
    bench.click_on_the_canvas();
    bench.rest_2_2();
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Entry(2),
            color: GREEN
        }]
    );
    assert!(!bench.wants_keyboard());
}

/// **[DEPENDE DE EGUI]** Enfocar y salir sin teclear nada: `Nothing`, sin paso ni aviso.
#[test]
fn row_09_focusing_and_leaving_without_typing_does_nothing() {
    for output_2 in ["clic fuera", "Intro", "Escape"] {
        let mut bench = bench_f(1.0, 2);
        bench.focuses_field();
        bench.history.clear();
        match output_2 {
            "clic fuera" => bench.click_on_the_canvas(),
            "Intro" => {
                bench.frame(vec![key_2(Key::Enter)]);
            }
            _ => {
                bench.frame(vec![key_2(Key::Escape)]);
            }
        }
        bench.rest_2_2();
        bench.rest_2_2();
        assert!(bench.commits().is_empty(), "{output_2}: sin paso");
        assert!(bench.notices().is_empty(), "{output_2}: sin aviso");
        assert_eq!(bench.editor.hex_display(), None, "{output_2}");
        assert!(!bench.wants_keyboard(), "{output_2}: ya no hay foco");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: hexadecimal inválido (Intro, perder el foco, Escape)
// ---------------------------------------------------------------------------------------------

const REASON_DIGIT: &str = "solo dígitos 0-9 y a-f";

/// **[DEPENDE DE EGUI]** Intro con un valor inválido: `show_editing` le devuelve el foco al campo.
#[test]
fn row_11_enter_with_invalid_keeps_the_focus_the_text_and_the_editor_notice() {
    let mut bench = bench_f(1.0, 2);
    let before = bench.image.clone();
    bench.focuses_field();
    bench.writes("zzz");
    assert_eq!(bench.editor.hex_display(), Some("zzz".to_owned()));

    let f = bench.frame(vec![key_2(Key::Enter)]);
    assert_eq!(f.output_2.commit, None);
    assert_eq!(
        f.output_2.notice, None,
        "con Intro el aviso es del editor, no de la barra"
    );
    let f = bench.rest_2_2();
    assert!(
        bench.wants_keyboard(),
        "Intro con un valor inválido mantiene el foco"
    );
    assert_eq!(
        bench.editor.hex_display(),
        Some("zzz".to_owned()),
        "y el texto escrito"
    );
    assert!(
        f.paints(&format!("aviso: {REASON_DIGIT}")),
        "el editor muestra el aviso"
    );
    assert_eq!(bench.image, before, "el documento no cambia");
    assert!(bench.commits().is_empty());

    // Al teclear, el aviso desaparece.
    bench.frame(vec![text_2("q")]);
    let f = bench.rest_2_2();
    assert_eq!(bench.editor.hex_display(), Some("zzzq".to_owned()));
    assert!(
        !f.paints_something_with(REASON_DIGIT),
        "al teclear el aviso del editor se va"
    );
    assert!(bench.wants_keyboard());
}

/// **[DEPENDE DE EGUI]** Intro inválida y luego un clic fuera: el texto vuelve al color y la barra
/// avisa.
#[test]
fn row_11_invalid_enter_and_click_outside_reverts_the_text_and_warns_in_the_bar() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("zzz");
    bench.frame(vec![key_2(Key::Enter)]);
    bench.frame(vec![text_2("q")]);
    bench.frame(vec![key_2(Key::Enter)]);
    bench.rest_2_2();
    assert!(
        bench.wants_keyboard(),
        "sigue enfocado tras la segunda Intro"
    );
    bench.history.clear();

    bench.click_on_the_canvas();
    let f = bench.rest_2_2();
    bench.rest_2_2();
    assert_eq!(
        bench.notices(),
        vec![REASON_DIGIT.to_owned()],
        "un solo aviso para la barra: el motivo, sin el prefijo"
    );
    assert!(bench.commits().is_empty(), "no hay paso");
    assert_eq!(bench.editor.hex_display(), None, "el texto se revierte");
    assert!(f.paints(&hex_text(BLUE)), "el campo vuelve a #0000ff");
    assert!(!f.paints("zzzq"));
    assert!(
        !f.paints_something_with(REASON_DIGIT),
        "ya no hay aviso en el editor"
    );
    assert!(!bench.wants_keyboard());
}

/// **[DEPENDE DE EGUI]** Perder el foco con `Tab` sin haber pulsado Intro.
#[test]
fn row_11_losing_focus_with_tab_reverts_and_warns() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("zzz");
    bench.history.clear();

    bench.frame(vec![key_2(Key::Tab)]);
    bench.rest_2_2();
    bench.rest_2_2();
    assert_eq!(bench.notices(), vec![REASON_DIGIT.to_owned()]);
    assert!(bench.commits().is_empty());
    assert_eq!(bench.editor.hex_display(), None);
    let f = bench.rest_2_2();
    assert!(f.paints(&hex_text(BLUE)));
}

/// **[DEPENDE DE EGUI]** Perder el foco con un clic fuera sin Intro previa.
#[test]
fn row_11_losing_focus_with_a_click_reverts_and_warns() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("zzz");
    bench.history.clear();

    bench.click_on_the_canvas();
    bench.rest_2_2();
    bench.rest_2_2();
    assert_eq!(bench.notices(), vec![REASON_DIGIT.to_owned()]);
    assert!(bench.commits().is_empty());
    assert_eq!(bench.editor.hex_display(), None);
}

#[test]
fn row_11_escape_reverts_the_text_without_a_notice_or_step() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("zzz");
    bench.history.clear();

    let f = bench.frame(vec![key_2(Key::Escape)]);
    assert_eq!(f.output_2.notice, None);
    assert_eq!(f.output_2.commit, None);
    bench.rest_2_2();
    let f = bench.rest_2_2();
    assert!(bench.notices().is_empty(), "Escape no avisa");
    assert!(bench.commits().is_empty());
    assert_eq!(bench.editor.hex_display(), None);
    assert!(f.paints(&hex_text(BLUE)), "el texto vuelve al color actual");
    assert!(
        !f.paints_something_with("aviso"),
        "ni aviso en el editor: {:?}",
        f.texts.iter().map(|t| &t.text_2).collect::<Vec<_>>()
    );
}

#[test]
fn row_11_escape_after_an_invalid_enter_removes_the_editor_notice() {
    let mut bench = bench_f(1.0, 2);
    bench.focuses_field();
    bench.writes("zzz");
    bench.frame(vec![key_2(Key::Enter)]);
    let f = bench.rest_2_2();
    assert!(f.paints_something_with(REASON_DIGIT));
    bench.history.clear();

    bench.frame(vec![key_2(Key::Escape)]);
    let f = bench.rest_2_2();
    assert!(!f.paints_something_with(REASON_DIGIT));
    assert!(bench.notices().is_empty());
    assert!(bench.commits().is_empty());
    assert_eq!(bench.editor.hex_display(), None);
}

// ---------------------------------------------------------------------------------------------
// Fila 16: teclear `ff00ab` con el campo enfocado
// ---------------------------------------------------------------------------------------------

#[test]
fn row_16_with_the_field_focused_the_keyboard_belongs_to_the_field() {
    let mut bench = bench_f(1.0, 1);
    let zoom_fit = bench.rest_2_2().info.zoom;
    assert_eq!(Some(zoom_fit), Zoom::from_int(8), "la vista parte en ×8");
    bench.focuses_field();
    bench.selects_all();
    bench.history.clear();

    // Teclear ff00ab.
    let typing = presses_2_2("ff00ab");
    let f = bench.frame(typing.clone());
    assert!(bench.wants_keyboard());
    assert!(f.wants_before && f.wants_after);
    assert!(
        gate_keys(&typing, f.wants_before).is_empty(),
        "los atajos no ven lo tecleado: ni la f, ni los números"
    );
    assert!(gate_keys(&typing, f.wants_after).is_empty());
    assert_eq!(f.info.zoom, zoom_fit);
    assert_eq!(
        bench.editor.hex_display(),
        Some("ff00ab".to_owned()),
        "el campo tiene lo tecleado (sin # si no se escribió)"
    );
    assert_eq!(
        f.output_2.commit, None,
        "no se toca el color hasta confirmar"
    );
    assert_eq!(f.output_2.choice, None);
    assert!(f.entries.is_empty());

    // Home, + y -: el lienzo no cambia la vista.
    let view = f.info.image_rect;
    let keys = vec![
        key_2(Key::Home),
        key_2(Key::Plus),
        key_2(Key::Equals),
        key_2(Key::Minus),
    ];
    let f = bench.frame(keys.clone());
    assert!(gate_keys(&keys, f.wants_before).is_empty());
    assert_eq!(f.info.zoom, zoom_fit, "ni Home, ni +, ni - cambian el zoom");
    assert_eq!(f.info.image_rect, view, "ni la posición de la imagen");
    assert_eq!(bench.editor.hex_display(), Some("ff00ab".to_owned()));
    assert!(bench.commits().is_empty());
}

#[test]
fn row_16_contrast_without_the_field_focused_the_canvas_does_attend_the_keys() {
    let mut bench = bench_f(1.0, 1);
    let zoom_fit = bench.rest_2_2().info.zoom;
    assert!(!bench.wants_keyboard());
    let f = bench.frame(vec![key_2(Key::Plus)]);
    assert_eq!(
        f.info.zoom,
        zoom_fit.zoom_in(),
        "sin foco de texto, + acerca (el banco recoge las teclas)"
    );
    let f = bench.frame(vec![key_2(Key::Home)]);
    assert_eq!(f.info.zoom, zoom_fit, "y Home reencuadra");

    // `gate_keys` sin foco de texto lo deja pasar todo, en el mismo orden.
    let typing = presses_2_2("ff00ab");
    assert_eq!(gate_keys(&typing, false), typing);
}

// ---------------------------------------------------------------------------------------------
// Fila 17: al salir del campo los atajos vuelven
// ---------------------------------------------------------------------------------------------

/// Ctrl+Z, Ctrl+Y, Ctrl+S, la tecla 3 y Home.
fn shortcuts() -> Vec<Event> {
    vec![
        key_with(Key::Z, ctrl()),
        key_with(Key::Y, ctrl()),
        key_with(Key::S, ctrl()),
        key_2(Key::Num3),
        key_2(Key::Home),
    ]
}

/// Enfoca el campo, comprueba que `gate_keys` calla, sale con `exit` y comprueba que los atajos
/// vuelven (todos, en el mismo orden) y que `Home` y `+` actúan en el lienzo.
fn checks_field_output(name_2: &str, exit: impl FnOnce(&mut Bench)) {
    let mut bench = bench_f(1.0, 1);
    let zoom_fit = bench.rest_2_2().info.zoom;
    bench.focuses_field();
    let events_2 = shortcuts();
    assert!(
        gate_keys(&events_2, bench.wants_keyboard()).is_empty(),
        "{name_2}: con el campo enfocado no hay atajos"
    );

    exit(&mut bench);
    bench.rest_2_2();
    bench.rest_2_2();
    assert!(
        !bench.wants_keyboard(),
        "{name_2}: tras salir del campo no queda foco de texto"
    );
    assert_eq!(
        gate_keys(&events_2, bench.wants_keyboard()),
        events_2,
        "{name_2}: los atajos vuelven, todos y en el mismo orden"
    );

    // Y en el lienzo: `+` acerca y `Home` reencuadra.
    let f = bench.frame(vec![key_2(Key::Plus)]);
    assert_eq!(
        f.info.zoom,
        zoom_fit.zoom_in(),
        "{name_2}: + vuelve a acercar"
    );
    let f = bench.frame(vec![key_2(Key::Home)]);
    assert_eq!(f.info.zoom, zoom_fit, "{name_2}: Home vuelve a reencuadrar");
}

#[test]
fn row_17_after_enter_the_shortcuts_return() {
    checks_field_output("Intro", |b| {
        b.frame(vec![key_2(Key::Enter)]);
    });
}

#[test]
fn row_17_after_escape_the_shortcuts_return() {
    checks_field_output("Escape", |b| {
        b.frame(vec![key_2(Key::Escape)]);
    });
}

/// **[DEPENDE DE EGUI]**
#[test]
fn row_17_after_a_click_outside_the_shortcuts_return() {
    checks_field_output("clic fuera", |b| b.click_on_the_canvas());
}

/// **[DEPENDE DE EGUI]** `Tab` mueve el foco al siguiente widget enfocable: el lienzo es
/// `Sense::click_and_drag` y lo recibe, así que `egui_wants_keyboard_input()` seguiría cierto. La
/// guarda de teclado usa `text_edit_focused()` (solo un campo de texto), y entonces los atajos
/// vuelven (comprobado en el banco: decisión 11 de la spec).
#[test]
fn row_17_after_tab_the_shortcuts_return() {
    checks_field_output("Tab", |b| {
        b.frame(vec![key_2(Key::Tab)]);
    });
}

// ---------------------------------------------------------------------------------------------
// Ventana baja
// ---------------------------------------------------------------------------------------------
//
// La fila 18 de la spec del editor (ocultar el editor por debajo de `(41 + 14 + H)·n`, con su
// aviso `editor_hidden` y la cancelación del arrastre) la sustituye specs/hito-2-panel-ventana-baja.md:
// el editor no se oculta nunca por la altura. Esos casos con ventana (el diseño con 241·n, 240·n,
// 200·n y 100·n, el desplazamiento, el arrastre que no se cancela y la ausencia de avisos) están
// en `panel_baja.rs`. Aquí queda lo que sigue siendo cierto sin la altura: sin destino no hay
// editor.

#[test]
fn without_a_target_there_is_no_editor_or_commit_even_if_the_panel_is_low() {
    let n = 1;
    let cases = [
        (image_f(), ColorChoice::Entry(200)),
        (image_of(Mode::Rgba), ColorChoice::Slot(10)),
        (image_of(Mode::Rgba), ColorChoice::Slot(0)),
        (image_of(Mode::Gray), ColorChoice::Entry(0)),
    ];
    for (image, choice) in cases {
        for height_2 in [240 * n, 700 * n] {
            let mut bench = Bench::with_panel_height(1.0, height_2, image.clone(), choice);
            assert!(bench.actual().is_none(), "{choice:?} no tiene destino");
            let f = bench.rest_2_2();
            assert!(
                !f.paints_something_with("editar"),
                "{choice:?}, alto {height_2}: sin destino no hay editor"
            );
            assert_eq!(f.output_2.commit, None);
            assert_eq!(f.output_2.notice, None);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Arrastre de deslizadores con el ratón
// ---------------------------------------------------------------------------------------------

#[test]
fn slider_pressing_on_the_track_sets_the_value_moving_changes_it_and_releasing_gives_a_commit() {
    for ppp in PPP_ALWAYS {
        // Entrada 1 de F: verde (0, 255, 0).
        let mut bench = bench_f(ppp, 1);
        let before = bench.image.clone();
        let geo = bench.geo();
        let y = geo.y_row(Control::Red);
        let dx_a = geo.width_hint() / 2;
        let dx_b = geo.width_hint() - 1;
        let (x_a, x_b) = (geo.x_hint() + dx_a, geo.x_hint() + dx_b);
        let value_a = geo.value_in(dx_a, Control::Red);
        let value_b = geo.value_in(dx_b, Control::Red);
        assert_eq!(value_b, Control::Red.max());
        assert_eq!(value_shown(&bench, Control::Red), 0);

        // Pulsar ya fija el valor bajo el puntero.
        bench.moves(x_a, y);
        let f = bench.presses_2(x_a, y);
        assert!(
            bench.editor.dragging(),
            "ppp {ppp}: pulsar empieza el arrastre"
        );
        assert_eq!(value_shown(&bench, Control::Red), value_a, "ppp {ppp}");
        assert_eq!(
            bench.editor.preview(),
            Some((EditTarget::Entry(1), Rgba::opaque(value_a as u8, 255, 0))),
            "ppp {ppp}: el provisional solo cambia el canal rojo"
        );
        assert_eq!(f.output_2.commit, None, "ppp {ppp}: pulsar no confirma");

        // Mover con el botón pulsado lo cambia; el documento no se toca.
        let f = bench.moves(x_b, y);
        assert_eq!(value_shown(&bench, Control::Red), value_b, "ppp {ppp}");
        assert_eq!(bench.editor.preview(), Some((EditTarget::Entry(1), YELLOW)));
        assert_eq!(f.output_2.commit, None);
        assert_eq!(bench.image, before, "ppp {ppp}: el documento no cambia");
        assert!(f.entries.is_empty(), "ppp {ppp}: el lienzo no dibuja");

        // La muestra elegida del panel se pinta con el color provisional (y no con el guardado).
        let f = bench.rest_2_2();
        assert!(
            f.there_is_fill(Color32::from_rgb(255, 255, 0)),
            "ppp {ppp}: la muestra se pinta en amarillo"
        );
        assert!(
            !f.there_is_fill(Color32::from_rgb(0, 255, 0)),
            "ppp {ppp}: ya no se pinta el verde guardado"
        );

        // Soltar da un commit, una sola vez.
        let f = bench.releases(x_b, y);
        assert_eq!(
            f.output_2.commit,
            Some(Commit {
                target: EditTarget::Entry(1),
                color: YELLOW
            }),
            "ppp {ppp}"
        );
        assert!(!bench.editor.dragging());
        bench.rest_2_2();
        bench.rest_2_2();
        assert_eq!(bench.commits().len(), 1, "ppp {ppp}: un solo commit");
        assert_eq!(bench.image, before);
    }
}

#[test]
fn slider_the_x_is_limited_to_the_track_even_if_the_pointer_leaves_it_or_the_row() {
    let mut bench = bench_f(1.0, 1);
    let geo = bench.geo();
    let y = geo.y_row(Control::Red);
    let x_a = geo.x_of_value(100, Control::Red.max());
    bench.moves(x_a, y);
    bench.presses_2(x_a, y);
    assert!(bench.editor.dragging());

    // A la izquierda de la pista, bien arriba (fuera de la fila): el valor es 0.
    let f = bench.moves(2, 2);
    assert_eq!(value_shown(&bench, Control::Red), 0);
    assert!(f.entries.is_empty());

    // Sobre el lienzo, a la derecha del panel y bien abajo: el valor es el máximo.
    let outside_x = PANEL_WIDTH_U * geo.n + 100;
    let f = bench.moves(outside_x, geo.top_px + 20 * geo.n);
    assert_eq!(value_shown(&bench, Control::Red), Control::Red.max());
    assert!(
        f.entries.is_empty(),
        "el lienzo no ve el arrastre del deslizador"
    );

    // Soltar fuera de la ventana de la fila: confirma el último valor.
    let f = bench.releases(outside_x, geo.top_px + 20 * geo.n);
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Entry(1),
            color: YELLOW
        })
    );
    assert!(
        f.entries.is_empty(),
        "soltar sobre el lienzo no es un trazo"
    );
}

#[test]
fn slider_releasing_without_a_change_gives_none() {
    // Un clic sin mover sobre el valor actual (R = 0 en el verde).
    let mut bench = bench_f(1.0, 1);
    let geo = bench.geo();
    let y = geo.y_row(Control::Red);
    let start_2 = geo.x_hint();
    bench.moves(start_2, y);
    bench.presses_2(start_2, y);
    assert!(bench.editor.dragging());
    assert_eq!(value_shown(&bench, Control::Red), 0);
    let f = bench.releases(start_2, y);
    assert_eq!(f.output_2.commit, None, "mismo valor: sin paso");
    assert!(!bench.editor.dragging());

    // Se aleja y vuelve al valor inicial antes de soltar.
    bench.moves(start_2, y);
    bench.presses_2(start_2, y);
    let far = geo.x_of_value(200, Control::Red.max());
    bench.moves(far, y);
    assert_ne!(value_shown(&bench, Control::Red), 0);
    bench.moves(start_2, y);
    let f = bench.releases(start_2, y);
    assert_eq!(f.output_2.commit, None, "vuelve al valor inicial: sin paso");
    bench.rest_2_2();
    assert!(bench.commits().is_empty());
    assert_eq!(bench.editor.preview(), None);
}

#[test]
fn slider_escape_cancels_restores_and_ignores_the_following_moves() {
    let mut bench = bench_f(1.0, 1);
    let geo = bench.geo();
    let y = geo.y_row(Control::Red);
    let max = Control::Red.max();
    let (x_a, x_b) = (geo.x_of_value(100, max), geo.x_of_value(max, max));
    bench.moves(x_a, y);
    bench.presses_2(x_a, y);
    bench.moves(x_b, y);
    assert!(bench.editor.dragging());
    assert_ne!(shown(&bench), GREEN);

    let f = bench.frame(vec![key_2(Key::Escape)]);
    assert_eq!(f.output_2.commit, None);
    assert!(!bench.editor.dragging(), "Escape cancela el arrastre");
    assert_eq!(bench.editor.preview(), None);
    assert_eq!(shown(&bench), GREEN, "restaura el color guardado");
    let f = bench.rest_2_2();
    assert!(
        f.there_is_fill(Color32::from_rgb(0, 255, 0)),
        "la muestra vuelve a pintarse con el guardado"
    );

    // Los movimientos siguientes se ignoran hasta soltar el botón (sin reanudar).
    bench.moves(x_a, y);
    assert!(!bench.editor.dragging());
    assert_eq!(bench.editor.preview(), None);
    assert_eq!(shown(&bench), GREEN);
    let f = bench.releases(x_a, y);
    assert_eq!(f.output_2.commit, None, "soltar tras cancelar no da paso");
    bench.rest_2_2();
    assert!(bench.commits().is_empty());
}

#[test]
fn slider_losing_the_window_focus_cancels() {
    let mut bench = bench_f(1.0, 1);
    let geo = bench.geo();
    let y = geo.y_row(Control::Red);
    let max = Control::Red.max();
    let (x_a, x_b) = (geo.x_of_value(100, max), geo.x_of_value(max, max));
    bench.moves(x_a, y);
    bench.presses_2(x_a, y);
    bench.moves(x_b, y);
    assert!(bench.editor.dragging());

    bench.frame(vec![Event::WindowFocused(false)]);
    assert!(
        !bench.editor.dragging(),
        "perder el foco la ventana cancela"
    );
    assert_eq!(bench.editor.preview(), None);
    assert_eq!(shown(&bench), GREEN);
    bench.releases(x_b, y);
    bench.rest_2_2();
    assert!(bench.commits().is_empty(), "no se aplica un color a medias");
}

#[test]
fn slider_with_a_drag_in_progress_the_grid_does_not_choose() {
    // `busy`: mientras hay un arrastre, `enabled` es falso y se ignoran los clics en la rejilla.
    let mut bench = bench_f(1.0, 1);
    let geo = bench.geo();
    let y = geo.y_row(Control::Red);
    let x_a = geo.x_of_value(100, Control::Red.max());
    bench.moves(x_a, y);
    bench.presses_2(x_a, y);
    assert!(bench.editor.dragging());

    let (x, y_swatch) = center_of_swatch(geo.n, 3, 0);
    bench.moves(x, y_swatch);
    let pos = in_points(geo.ppp, x, y_swatch);
    let f = bench.frame(vec![button_2(pos, true)]);
    assert_eq!(
        f.output_2.choice, None,
        "la rejilla no elige durante el arrastre"
    );
    assert!(bench.choices().is_empty());
}

#[test]
fn active_area_of_the_row_from_x_13u_included_to_201u_excluded_and_without_the_group_gaps() {
    // Casos: (nombre, borde en u, desplazamiento en píxeles, valor que debe fijar si arrastra).
    let cases = [
        ("borde izquierdo incluido", HIT_X0_U, 0, Some(0u32)),
        ("antes del borde izquierdo", HIT_X0_U, -1, None),
        (
            "último píxel de la zona",
            HIT_X1_U,
            -1,
            Some(Control::Red.max()),
        ),
        ("borde derecho excluido", HIT_X1_U, 0, None),
    ];
    for ppp in [1.0_f32, 2.0] {
        for (name_2, border_u, delta, value_expected) in cases {
            let mut bench = bench_f(ppp, 1);
            let geo = bench.geo();
            let x = border_u * geo.n + delta;
            let y = geo.y_row(Control::Red);
            assert_eq!(
                slider_hit(geo.mode, x, y - geo.top_px, geo.n as u32).is_some(),
                value_expected.is_some(),
                "ppp {ppp}, {name_2}: edit_layout::slider_hit"
            );
            bench.moves(x, y);
            bench.presses_2(x, y);
            assert_eq!(
                bench.editor.dragging(),
                value_expected.is_some(),
                "ppp {ppp}, {name_2}: ¿empieza el arrastre?"
            );
            if let Some(value) = value_expected {
                assert_eq!(
                    value_shown(&bench, Control::Red),
                    value,
                    "ppp {ppp}, {name_2}"
                );
            }
        }
    }

    // El hueco entre el grupo RGB y el HSL, a la altura de su primer píxel, no es de ninguna fila.
    let mut bench = bench_f(1.0, 1);
    let geo = bench.geo();
    let row_blue = control_row_u(geo.mode, Control::Blue).expect("tiene B");
    let y = geo.top_px + (row_blue + ROW_H_U) * geo.n;
    let x = geo.x_of_value(100, Control::Blue.max());
    assert_eq!(slider_hit(geo.mode, x, y - geo.top_px, geo.n as u32), None);
    bench.moves(x, y);
    bench.presses_2(x, y);
    assert!(
        !bench.editor.dragging(),
        "el hueco entre grupos no arrastra"
    );
}

#[test]
fn each_row_drags_its_control() {
    // Indexadas: R G B | H S L | A; cada fila empieza el arrastre de su propio control.
    for control in controls(Mode::Indexed).to_vec() {
        let mut bench = bench_f(1.0, 1);
        let geo = bench.geo();
        let y = geo.y_row(control);
        let x = geo.x_of_value(control.max() / 2, control.max());
        bench.moves(x, y);
        bench.presses_2(x, y);
        assert!(bench.editor.dragging(), "{control:?}");
        let expected_2 = geo.value_in(x - geo.x_hint(), control);
        assert_eq!(value_shown(&bench, control), expected_2, "{control:?}");
    }
}

#[test]
fn gray_slider_of_gray_confirms_a_gray_slot() {
    // Gray: un solo deslizador «G»; la ranura 5 es un gris (128 por defecto).
    let mut bench = Bench::new_2(1.0, SCREEN_HIGH, image_of(Mode::Gray), ColorChoice::Slot(5));
    assert_eq!(controls(Mode::Gray), &[Control::Gray]);
    let geo = bench.geo();
    let y = geo.y_row(Control::Gray);
    let max = Control::Gray.max();
    let x = geo.x_of_value(max, max);
    bench.moves(x, y);
    bench.presses_2(x, y);
    assert!(bench.editor.dragging());
    assert_eq!(shown(&bench), Rgba::opaque(255, 255, 255));
    let f = bench.releases(x, y);
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Slot(5),
            color: Rgba::opaque(255, 255, 255)
        })
    );
}

#[test]
fn gray_alpha_alpha_slider_confirms_alpha_0() {
    // GrayAlpha: «G» y «A»; la ranura 1 es negro opaco por defecto.
    let mut bench = Bench::new_2(
        1.0,
        SCREEN_HIGH,
        image_of(Mode::GrayAlpha),
        ColorChoice::Slot(1),
    );
    assert_eq!(controls(Mode::GrayAlpha), &[Control::Gray, Control::Alpha]);
    let geo = bench.geo();
    let y = geo.y_row(Control::Alpha);
    let x = geo.x_hint();
    bench.moves(x, y);
    bench.presses_2(x, y);
    assert!(bench.editor.dragging());
    assert_eq!(shown(&bench), Rgba::new(0, 0, 0, 0));
    let f = bench.releases(x, y);
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Slot(1),
            color: Rgba::new(0, 0, 0, 0)
        })
    );
}

#[test]
fn the_image_the_panel_receives_does_not_change_during_the_whole_flow() {
    // Arrastre confirmado, arrastre cancelado y hexadecimal: la imagen es la misma (el panel solo
    // devuelve `commit`; aplicarlo es de la sesión).
    let mut bench = bench_f(1.0, 1);
    let before = bench.image.clone();
    let geo = bench.geo();
    let y = geo.y_row(Control::Green);
    let max = Control::Green.max();
    let x = geo.x_of_value(10, max);
    bench.moves(x, y);
    bench.presses_2(x, y);
    bench.releases(x, y);
    bench.moves(x, y);
    bench.presses_2(x, y);
    bench.frame(vec![key_2(Key::Escape)]);
    bench.releases(x, y);
    bench.focuses_field();
    bench.writes("#123456");
    bench.frame(vec![key_2(Key::Enter)]);
    assert_eq!(bench.image, before);
    // Nada tocó `Pixel` alguno: la imagen es una indexada vacía de índice 0.
    assert_eq!(bench.image.get_pixel(0, 0), Ok(Pixel::Indexed(0)));
}
