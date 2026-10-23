//! Tests del panel de paletas con la ventana baja, dentro de la ventana
//! (specs/hito-2-panel-ventana-baja.md, tabla «Casos», filas 3, 4, 5, 6, 7, 8, 9 y 11 en su parte
//! de banco). Lo puro (`panel_layout`, `PanelView::scroll_by`, `panel_thumb`, `Scroll::reveal`,
//! `ColorEditor::confirm_hex_checked`) se prueba en `edit_layout.rs`, `panel_view.rs` y
//! `color_edit.rs`; aquí se prueba que `show_editing_view` lo une bien a `egui`.
//!
//! El banco es el de `tests/color_editor.rs`, copiado y adaptado (ese fichero no se toca): un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hará la app, se llama a `ColorEditor::sync`, se monta `status::show`, luego
//! `palette_panel::show_editing_view` con un `PanelView` que **persiste entre fotogramas** y por
//! fin el lienzo. `enabled` es `!editor.dragging()`.
//!
//! La imagen del banco sale de un `Document` real: `applies`, `undoes` y `redoes` mutan la imagen
//! con `Command::SetPaletteColor`, `AddPaletteEntry` o `RemovePaletteEntry`, como lo haría la
//! app. El banco **nunca aplica** lo que devuelve el panel (la confirmación es de `Session`, en
//! `px-app`): cada `Commit` se guarda en `bench.history`.
//!
//! Se mide a `ppp` 1,0 y 2,0 (`n` = 1 y 2). Los altos del panel son múltiplos de `n`, así que
//! `top_px = height_2 − H·n` cuando se desplaza al máximo. La rueda se manda en puntos
//! (`MouseWheelUnit::Point`): `delta.y = −px / ppp` desplaza `px` píxeles físicos hacia abajo.
//!
//! Lo pintado se lee de las formas del fotograma. «Todo lo pintado queda dentro del panel» se
//! comprueba con el `clip_rect` de cada `ClippedShape`. La barra de desplazamiento del panel son
//! los rectángulos de `x = TRACK_X_U·u` (el de `palette_grid`) y ancho `TRACK_W_U·u`: la pista
//! (alto de todo el panel, desde su borde superior) y el indicador `TEXT_DIM` de `panel_thumb`.
//! No hay capturas de pantalla.

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource, MouseWheelUnit,
    PointerButton, Pos2, RawInput, Rect, TouchPhase, Vec2, pos2, vec2,
};
use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Rgba};
use px_ui::canvas::Canvas;
use px_ui::color::{ColorChoice, SlotColors, cell_of_choice};
use px_ui::color_edit::{
    ColorEditor, Commit, Control, EditTarget, StaleReason, hex_notice, hex_text, slider_value_at,
    slider_x, stale_notice,
};
use px_ui::edit_layout::{
    COLOR_FIRST_ROW_Y_U, FIELD_H_U, FIELD_W_U, FIELD_X_U, FIELD_Y_U, PanelLayout, ROW_H_U,
    TRACK_W_U, TRACK_X_U, block_height_px, control_row_u, editor_height_u, panel_layout,
};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PANEL_WIDTH_U, PITCH_U, PanelView, panel_thumb,
    total_rows,
};
use px_ui::palette_grid::{TRACK_W_U as BAR_W_U, TRACK_X_U as BAR_X_U};
use px_ui::palette_panel::{self, PanelOutput};
use px_ui::status::bar_height_px;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::TEXT_DIM;

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

const PPPS: [f32; 2] = [1.0, 2.0];

/// Indexada de 64×48 con la paleta `[R, G, B, K]`.
fn image_f() -> Image {
    let palette = Palette::from_colors([RED, GREEN, BLUE, TEST_BLACK]).expect("paleta");
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

/// Indexada de 64×48 con 256 colores (16 filas de la rejilla).
fn image_indexed_256() -> Image {
    let colors_2: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect();
    let palette = Palette::from_colors(colors_2).expect("paleta de 256 colores");
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

fn image_rgba() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen RGBA")
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

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

fn key_ctrl_a() -> Event {
    Event::Key {
        key: Key::A,
        physical_key: Some(Key::A),
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    }
}

fn in_points(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Painted {
    text_2: String,
    pos: Pos2,
}

fn collects(shape: &Shape, texts: &mut Vec<Painted>, rects: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Text(t) => texts.push(Painted {
            text_2: t.galley.text().to_owned(),
            pos: t.pos,
        }),
        Shape::Rect(r) => rects.push((r.rect, r.fill)),
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects(shape_2, texts, rects);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

const SCREEN_HIGH: Vec2 = vec2(800.0, 600.0);

struct Bench {
    ctx: Context,
    canvas: Canvas,
    scroll: palette_grid::Scroll,
    view: PanelView,
    screen: Vec2,
    document: Document,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    /// Lo que devolvió `show_editing_view` en cada fotograma, en orden.
    history: Vec<PanelOutput>,
}

struct TestFrame {
    rect_canvas: Rect,
    output_2: PanelOutput,
    texts: Vec<Painted>,
    rects: Vec<(Rect, Color32)>,
    shapes_2: Vec<ClippedShape>,
}

impl TestFrame {
    fn paints(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2 == searched)
    }

    fn paints_something_with(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2.contains(searched))
    }

    fn text_painted(&self, searched: &str) -> Option<&Painted> {
        self.texts.iter().find(|t| t.text_2 == searched)
    }
}

/// Medidas del editor a la escala del banco, en píxeles físicos desde el borde superior del panel.
#[derive(Clone, Copy, Debug)]
struct Geo {
    n: i64,
    mode: Mode,
    top_px: i64,
    /// Top edge of the tail under the grid (bar, field and icon row), where the field now lives.
    tail_top_px: i64,
}

impl Geo {
    /// `y` del centro de la fila del deslizador `control`.
    fn y_row(&self, control: Control) -> i64 {
        let row_2 = control_row_u(self.mode, control).expect("el modo tiene ese deslizador");
        self.top_px + row_2 * self.n + ROW_H_U * self.n / 2
    }

    fn x_hint(&self) -> i64 {
        TRACK_X_U * self.n
    }

    fn width_hint(&self) -> i64 {
        TRACK_W_U * self.n
    }

    fn x_of_value(&self, value: u32, max: u32) -> i64 {
        self.x_hint() + slider_x(value, self.width_hint(), max)
    }

    fn value_in(&self, dx: i64, control: Control) -> u32 {
        slider_value_at(dx, self.width_hint(), control.max())
    }

    /// Centro del campo hexadecimal.
    fn field_2(&self) -> (i64, i64) {
        (
            FIELD_X_U * self.n + FIELD_W_U * self.n / 2,
            self.tail_top_px + FIELD_Y_U * self.n + FIELD_H_U * self.n / 2,
        )
    }
}

/// Centro de la muestra `index_2` en píxeles físicos con el bloque sin desplazar.
fn center_of_swatch(n: i64, index_2: usize, first_row: usize) -> (i64, i64) {
    let column = (index_2 % COLUMNS) as i64;
    let row_2 = (index_2 / COLUMNS) as i64 - first_row as i64;
    (
        (GRID_X_U + column * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + row_2 * PITCH_U) * n + CELL_U * n / 2,
    )
}

fn screen_for_panel(ppp: f32, panel_h_px: i64) -> Vec2 {
    let n = text_scale(ppp);
    vec2(
        800.0,
        (panel_h_px + i64::from(bar_height_px(n))) as f32 / ppp,
    )
}

impl Bench {
    /// Contexto de `screen` puntos a `ppp`, con la fuente instalada, `image` en un `Document`
    /// y, si `warm_up`, dos fotogramas de reposo del panel (sin ellos el primer fotograma del
    /// panel es el primero que se llama a `show_editing_view`).
    fn new_with(ppp: f32, screen: Vec2, image: Image, choice: ColorChoice, warm_up: bool) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut bench = Bench {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            view: PanelView::default(),
            screen,
            document: Document::new(image.clone()),
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
        if warm_up {
            bench.rest_2_2();
            bench.rest_2_2();
            bench.history.clear();
        }
        bench
    }

    fn new_2(ppp: f32, screen: Vec2, image: Image, choice: ColorChoice) -> Bench {
        Bench::new_with(ppp, screen, image, choice, true)
    }

    fn with_panel_height(ppp: f32, panel_h_px: i64, image: Image, choice: ColorChoice) -> Bench {
        Bench::new_2(ppp, screen_for_panel(ppp, panel_h_px), image, choice)
    }

    /// Cambia el alto del panel (la ventana) para el siguiente fotograma.
    fn height_of_panel(&mut self, panel_h_px: i64) {
        self.screen = screen_for_panel(self.ppp(), panel_h_px);
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

    fn panel_h_px(&self) -> i64 {
        (self.screen.y * self.ppp()).round() as i64 - i64::from(bar_height_px(self.n() as u32))
    }

    /// El panel, en puntos.
    fn rect_panel(&self) -> Rect {
        let u = self.n() as f32 / self.ppp();
        Rect::from_min_size(
            Pos2::ZERO,
            vec2(
                PANEL_WIDTH_U as f32 * u,
                self.panel_h_px() as f32 / self.ppp(),
            ),
        )
    }

    /// El diseño del panel con el desplazamiento que tiene la vista ahora.
    fn layout(&self) -> PanelLayout {
        panel_layout(
            self.image.mode(),
            self.panel_h_px(),
            self.n() as u32,
            self.view.offset_px,
        )
    }

    /// El máximo desplazamiento posible con el alto actual.
    fn max_offset(&self) -> i64 {
        panel_layout(self.image.mode(), self.panel_h_px(), self.n() as u32, 0).max_offset_px
    }

    fn geo(&self) -> Geo {
        Geo {
            n: self.n(),
            mode: self.image.mode(),
            top_px: self.layout().top_px,
            tail_top_px: self.layout().tail_top_for(
                total_rows(
                    px_ui::color::cell_count(&self.image)
                        + usize::from(self.image.palette().is_some()),
                ),
                self.n() as u32,
            ),
        }
    }

    fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let target_2 = EditTarget::of(&self.image, self.choice)?;
        let color = target_2.stored(&self.image, &self.slots)?;
        Some((target_2, color))
    }

    fn frame(&mut self, events: Vec<Event>) -> TestFrame {
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !self.editor.dragging();
        let input = self.input_2(events);
        let mut result = None;
        let Bench {
            ctx,
            canvas,
            scroll,
            view,
            image,
            choice,
            slots,
            editor,
            ..
        } = self;
        let mut output_2 = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (panel, rect_canvas) = panel_on_the_left(
                ui,
                |ui| {
                    palette_panel::show_editing_view(
                        ui, image, *choice, scroll, view, enabled, slots, editor,
                    )
                },
                |ui| {
                    let rect_canvas = ui.available_rect_before_wrap();
                    let _ = canvas
                        .interact_roles(ui)
                        .into_iter()
                        .map(|(input, _)| input)
                        .collect::<Vec<_>>();
                    let _ = canvas.paint(ui);
                    rect_canvas
                },
            );
            result = Some((rect_canvas, panel));
        });
        output_2.textures_delta.clear();
        let shapes_2: Vec<ClippedShape> = std::mem::take(&mut output_2.shapes);
        let (rect_canvas, panel) = result.expect("el lienzo se dibuja");
        let (mut texts, mut rects) = (Vec::new(), Vec::new());
        for clipped in &shapes_2 {
            collects(&clipped.shape, &mut texts, &mut rects);
        }
        self.history.push(panel.clone());
        TestFrame {
            rect_canvas,
            output_2: panel,
            texts,
            rects,
            shapes_2,
        }
    }

    fn rest_2_2(&mut self) -> TestFrame {
        self.frame(vec![])
    }

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

    fn click(&mut self, x_px: i64, y_px: i64) {
        self.moves(x_px, y_px);
        self.presses_2(x_px, y_px);
        self.releases(x_px, y_px);
    }

    fn click_on_the_canvas(&mut self) {
        let center = self.rest_2_2().rect_canvas.center();
        let (x, y) = (
            (center.x * self.ppp()).round() as i64,
            (center.y * self.ppp()).round() as i64,
        );
        self.click(x, y);
    }

    /// La rueda, `px_below` píxeles físicos hacia abajo, en el punto donde esté el puntero.
    fn wheel(&mut self, px_below: i64) -> TestFrame {
        let delta = vec2(0.0, -(px_below as f32) / self.ppp());
        self.frame(vec![Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta,
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }])
    }

    /// Mueve el puntero a `(x, y)` y gira la rueda `px_below` píxeles físicos.
    fn wheel_in(&mut self, x_px: i64, y_px: i64, px_below: i64) -> TestFrame {
        self.moves(x_px, y_px);
        self.wheel(px_below)
    }

    /// Un punto del panel dentro del editor (su última fila de píxeles) para poner el puntero.
    fn over_the_editor(&self) -> (i64, i64) {
        (100 * self.n(), self.panel_h_px() - self.n())
    }

    /// Un punto del panel sobre el texto de arriba.
    fn over_the_text(&self) -> (i64, i64) {
        (10 * self.n(), 10 * self.n())
    }

    // --- Campo hexadecimal ---

    fn focuses_field(&mut self) {
        let (x, y) = self.geo().field_2();
        self.click(x, y);
        self.rest_2_2();
        assert!(
            self.wants_keyboard(),
            "un clic en el campo le da el foco de texto"
        );
    }

    fn writes(&mut self, string: &str) -> TestFrame {
        self.frame(vec![key_ctrl_a()]);
        self.frame(vec![text_2(string)])
    }

    // --- Documento ---

    fn refreshes(&mut self) {
        let id = ImageId::from_raw(0);
        self.image = self.document.image(id).expect("la imagen").clone();
    }

    fn applies(&mut self, command: Command) {
        self.document.apply(command).expect("comando válido");
        self.refreshes();
    }

    fn undoes(&mut self) {
        self.document.undo().expect("hay algo que deshacer");
        self.refreshes();
    }

    fn redoes(&mut self) {
        self.document.redo().expect("hay algo que rehacer");
        self.refreshes();
    }

    fn set_color(&mut self, index: u8, color: Rgba) {
        let image = ImageId::from_raw(0);
        self.applies(Command::SetPaletteColor {
            image,
            index,
            color,
        });
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

/// The label of the first slider: the editor is seen when it is painted at the top of its block
/// (the editor has no header since C3: only the separator and the sliders).
fn first_slider_label(f: &TestFrame) -> Option<&Painted> {
    f.text_painted("R")
}

/// Banco con la imagen `F` y un panel de 700·n (cabe todo), elegida la entrada `input_2`.
fn bench_f(ppp: f32, input_2: u8) -> Bench {
    Bench::new_2(ppp, SCREEN_HIGH, image_f(), ColorChoice::Entry(input_2))
}

// ---------------------------------------------------------------------------------------------
// Comprobaciones de lo pintado
// ---------------------------------------------------------------------------------------------

const TOLERANCE: f32 = 0.05;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// Todo lo pintado que cae sobre el panel se recorta al panel: el `clip_rect` de cada forma que
/// asoma en él está dentro del panel. Lo de la barra de estado y el lienzo no se solapa con él.
fn checks_inside_the_panel(f: &TestFrame, panel: Rect, context: &str) {
    let panel = panel.expand(TOLERANCE);
    for shape_2 in &f.shapes_2 {
        let visible = shape_2
            .shape
            .visual_bounding_rect()
            .intersect(shape_2.clip_rect);
        let overlap = visible.intersect(panel);
        if !(visible.is_positive()
            && overlap.width() > 2.0 * TOLERANCE
            && overlap.height() > 2.0 * TOLERANCE)
        {
            continue;
        }
        assert!(
            panel.contains_rect(shape_2.clip_rect),
            "{context}: una forma que cae en el panel se recorta a {:?}, fuera del panel {panel:?}",
            shape_2.clip_rect
        );
        assert!(
            panel.contains_rect(visible),
            "{context}: se ve {visible:?} y el panel es {panel:?}"
        );
    }
}

/// Rectángulos con la `x` y el ancho de la barra de desplazamiento del panel.
fn bar_rects(f: &TestFrame, ppp: f32, n: i64) -> Vec<(Rect, Color32)> {
    let x = (BAR_X_U * n) as f32 / ppp;
    let width_2 = (BAR_W_U * n) as f32 / ppp;
    f.rects
        .iter()
        .filter(|(r, _)| near(r.min.x, x) && near(r.width(), width_2))
        .copied()
        .collect()
}

/// La pista de la barra: desde el borde superior del panel y con todo su alto.
fn bar_hints(f: &TestFrame, bench: &Bench) -> Vec<Rect> {
    let height_2 = bench.panel_h_px() as f32 / bench.ppp();
    bar_rects(f, bench.ppp(), bench.n())
        .into_iter()
        .map(|(r, _)| r)
        .filter(|r| near(r.min.y, 0.0) && near(r.height(), height_2))
        .collect()
}

/// La barra solo se pinta con desplazamiento, con el indicador de `panel_thumb`; sin él no hay
/// pista del panel. Con desplazamiento, la rejilla no pinta el suyo: solo hay pista e indicador.
fn checks_bar(f: &TestFrame, bench: &Bench, context: &str) {
    let layout = bench.layout();
    let hints = bar_hints(f, bench);
    if !layout.scrolling {
        assert!(
            hints.is_empty(),
            "{context}: sin desplazamiento no hay pista"
        );
        return;
    }
    let (ppp, n) = (bench.ppp(), bench.n());
    let block = block_height_px(bench.image.mode(), n as u32);
    let (y, height_2) = panel_thumb(layout.offset_px, bench.panel_h_px(), block, n as u32)
        .expect("con desplazamiento hay indicador");
    assert_eq!(hints.len(), 1, "{context}: una pista del alto del panel");
    let bar = bar_rects(f, ppp, n);
    assert_eq!(
        bar.len(),
        2,
        "{context}: solo la pista y el indicador (la rejilla no pinta el suyo): {bar:?}"
    );
    let indicators: Vec<&Rect> = bar
        .iter()
        .filter(|(_, fill)| *fill == TEXT_DIM)
        .map(|(r, _)| r)
        .collect();
    assert_eq!(indicators.len(), 1, "{context}: un indicador TEXT_DIM");
    let indicator = indicators[0];
    assert!(
        near(indicator.min.y, y as f32 / ppp) && near(indicator.height(), height_2 as f32 / ppp),
        "{context}: indicador en {indicator:?} y `panel_thumb` da ({y}, {height_2}) px"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 3: el editor se ve siempre y la rejilla tiene una fila
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_the_editor_is_always_seen_and_the_grid_has_one_row() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for height_2 in [308 * n, 307 * n, 200 * n, 100 * n] {
            let context = format!("ppp {ppp}, alto {height_2}");
            let mut bench =
                Bench::with_panel_height(ppp, height_2, image_indexed_256(), ColorChoice::Entry(5));
            let f = bench.rest_2_2();
            let geo = bench.geo();
            assert_eq!(geo.top_px, 106 * n, "{context}: top_px con offset 0");
            // El editor se pinta, en su sitio.
            let label = first_slider_label(&f)
                .unwrap_or_else(|| panic!("{context}: sin el primer deslizador"));
            assert!(
                near(
                    label.pos.y * ppp,
                    (geo.top_px + COLOR_FIRST_ROW_Y_U * n) as f32
                ),
                "{context}: primer deslizador en y = {}",
                label.pos.y * ppp
            );
            // Una fila de rejilla.
            assert_eq!(bench.view.last_visible, Some(1), "{context}");
            let (x, y) = center_of_swatch(n, 0, 0);
            bench.click(x, y);
            let (x, y) = center_of_swatch(n, COLUMNS, 0);
            bench.click(x, y);
            assert_eq!(
                bench.choices(),
                vec![ColorChoice::Entry(0)],
                "{context}: la fila 1 se elige y la 2 no se ve"
            );
        }
    }
}

#[test]
fn row_3_from_308n_to_307n_the_editor_and_the_grid_do_not_jump() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench =
            Bench::with_panel_height(ppp, 308 * n, image_indexed_256(), ColorChoice::Entry(5));
        let before = bench.rest_2_2();
        let header_before = first_slider_label(&before).expect("primer deslizador").pos;
        assert!(!bench.layout().scrolling);

        bench.height_of_panel(307 * n);
        let after_2 = bench.rest_2_2();
        assert!(bench.layout().scrolling, "ppp {ppp}: 240n desplaza");
        assert_eq!(bench.view.offset_px, 0);
        let header_after = first_slider_label(&after_2).expect("primer deslizador").pos;
        assert!(
            near(header_before.y, header_after.y) && near(header_before.x, header_after.x),
            "ppp {ppp}: la cabecera salta de {header_before:?} a {header_after:?}"
        );
        assert_eq!(bench.view.last_visible, Some(1));
    }
}

#[test]
fn row_3_with_maximum_offset_the_editor_stays_anchored_to_the_bottom() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for height_2 in [307 * n, 200 * n, 100 * n] {
            let mut bench =
                Bench::with_panel_height(ppp, height_2, image_indexed_256(), ColorChoice::Entry(5));
            let (x, y) = bench.over_the_editor();
            bench.wheel_in(x, y, 100_000 * n);
            let block = block_height_px(Mode::Indexed, n as u32);
            let max = ((block - height_2) / n) * n;
            assert_eq!(bench.view.offset_px, max, "ppp {ppp}, alto {height_2}");
            assert_eq!(bench.max_offset(), max);
            assert_eq!(
                bench.geo().top_px,
                height_2 - editor_height_u(Mode::Indexed) * n,
                "ppp {ppp}, alto {height_2}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: la rueda mueve el panel, no las filas
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_the_wheel_over_the_text_and_over_the_editor_moves_offset_px_and_not_the_rows() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench =
            Bench::with_panel_height(ppp, 200 * n, image_indexed_256(), ColorChoice::Entry(5));
        assert!(bench.layout().scrolling);
        assert_eq!(bench.view.offset_px, 0);

        // Sobre el texto y la rejilla.
        let (x, y) = bench.over_the_text();
        bench.wheel_in(x, y, 10 * n);
        assert_eq!(bench.view.offset_px, 10 * n, "ppp {ppp}: sobre el texto");
        // Sobre el editor.
        let (x, y) = bench.over_the_editor();
        bench.wheel_in(x, y, 5 * n);
        assert_eq!(bench.view.offset_px, 15 * n, "ppp {ppp}: sobre el editor");
        // Hacia arriba.
        bench.wheel(-(6 * n));
        assert_eq!(bench.view.offset_px, 9 * n, "ppp {ppp}: hacia arriba");

        // Las filas de la rejilla no se mueven.
        assert_eq!(bench.scroll.first_row, 0, "ppp {ppp}");
        assert_eq!(bench.scroll.rest, 0, "ppp {ppp}");
        // Sin eventos el desplazamiento se queda donde está.
        for _ in 0..3 {
            bench.rest_2_2();
        }
        assert_eq!(bench.view.offset_px, 9 * n, "ppp {ppp}");
        assert_eq!(bench.scroll.first_row, 0);
    }
}

#[test]
fn row_4_offset_px_is_always_a_multiple_of_n_and_is_clipped_to_zero_and_to_the_maximum() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench =
            Bench::with_panel_height(ppp, 200 * n, image_indexed_256(), ColorChoice::Entry(5));
        let max = bench.max_offset();
        assert_eq!(max, 108 * n);
        let (x, y) = bench.over_the_text();
        bench.moves(x, y);
        // Subir con el desplazamiento a 0: se queda en 0.
        bench.wheel(-(50 * n));
        assert_eq!(bench.view.offset_px, 0);
        // Pasos que no son múltiplo de `n` (en `ppp` 2 un píxel suelto), desde el interior para
        // no mezclarlos con el resto del extremo superior: se acumulan en el resto.
        bench.wheel(10 * n);
        assert_eq!(bench.view.offset_px, 10 * n);
        for px in [1, 1, 1, 3, 5, 7] {
            bench.wheel(px);
            assert_eq!(bench.view.offset_px % n, 0, "ppp {ppp}: tras bajar {px} px");
            assert!(bench.view.rest_px >= 0 && bench.view.rest_px < n);
        }
        // 1 + 1 + 1 + 3 + 5 + 7 = 18 px.
        assert_eq!(
            bench.view.offset_px + bench.view.rest_px,
            10 * n + 18,
            "ppp {ppp}: no se pierde nada"
        );
        // Bajar de más: el máximo, sin resto.
        bench.wheel(100_000 * n);
        assert_eq!(bench.view.offset_px, max);
        assert_eq!(bench.view.rest_px, 0);
        bench.wheel(n);
        assert_eq!(bench.view.offset_px, max);
        // Y subir de más: el cero.
        bench.wheel(-(100_000 * n));
        assert_eq!(bench.view.offset_px, 0);
    }
}

#[test]
fn row_4_the_wheel_outside_the_panel_does_not_move_the_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench =
            Bench::with_panel_height(ppp, 200 * n, image_indexed_256(), ColorChoice::Entry(5));
        let center = bench.rest_2_2().rect_canvas.center();
        let (x, y) = (
            (center.x * ppp).round() as i64,
            (center.y * ppp).round() as i64,
        );
        bench.wheel_in(x, y, 10 * n);
        assert_eq!(bench.view.offset_px, 0, "ppp {ppp}: sobre el lienzo");
        // Ni sobre la barra de estado.
        bench.wheel_in(50 * n, 210 * n, 10 * n);
        assert_eq!(
            bench.view.offset_px, 0,
            "ppp {ppp}: sobre la barra de estado"
        );
    }
}

#[test]
fn row_4_with_a_drag_in_progress_the_wheel_does_not_move_the_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 270·n: el panel desplaza (el bloque mide 308·n) y la fila de R (251·n) queda dentro.
        let mut bench = Bench::with_panel_height(ppp, 270 * n, image_f(), ColorChoice::Entry(1));
        let geo = bench.geo();
        let y = geo.y_row(Control::Red);
        let x = geo.x_of_value(128, Control::Red.max());
        bench.moves(x, y);
        bench.presses_2(x, y);
        assert!(bench.editor.dragging(), "ppp {ppp}: el arrastre empieza");
        bench.wheel(10 * n);
        assert_eq!(
            bench.view.offset_px, 0,
            "ppp {ppp}: la rueda no mueve el panel"
        );
        assert!(
            bench.editor.dragging(),
            "ppp {ppp}: y no cancela el arrastre"
        );
        // Al soltar, la rueda vuelve a valer.
        bench.releases(x, y);
        bench.wheel(10 * n);
        assert_eq!(bench.view.offset_px, 10 * n, "ppp {ppp}");
    }
}

#[test]
fn row_4_when_enlarging_up_to_the_block_offset_px_is_0_in_the_same_frame() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench =
            Bench::with_panel_height(ppp, 200 * n, image_indexed_256(), ColorChoice::Entry(5));
        let (x, y) = bench.over_the_text();
        bench.wheel_in(x, y, 100_000 * n);
        assert_eq!(bench.view.offset_px, 108 * n);

        // Agrandar un poco: el máximo baja y el desplazamiento se recorta en el mismo fotograma.
        bench.height_of_panel(220 * n);
        bench.rest_2_2();
        assert_eq!(bench.view.offset_px, 88 * n, "ppp {ppp}: 220n");
        assert_eq!(bench.max_offset(), 88 * n);

        // Hasta el bloque: 0 en el mismo fotograma, sin pista y con el editor en su sitio.
        bench.height_of_panel(308 * n);
        let f = bench.rest_2_2();
        assert_eq!(bench.view.offset_px, 0, "ppp {ppp}: 308n");
        assert!(!bench.layout().scrolling);
        checks_bar(&f, &bench, &format!("ppp {ppp}, 308n"));
        let label = first_slider_label(&f).expect("primer deslizador");
        assert!(near(
            label.pos.y * ppp,
            (106 * n + COLOR_FIRST_ROW_Y_U * n) as f32
        ));

        // Y mucho más alto: el diseño normal, sin desplazamiento posible.
        bench.height_of_panel(700 * n);
        bench.rest_2_2();
        assert_eq!(bench.view.offset_px, 0);
        assert_eq!(bench.view.rest_px, 0);
    }
}

#[test]
fn row_4_the_bar_is_only_painted_with_scroll_and_the_grid_does_not_paint_its_own() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for (height_2, pans) in [
            (700 * n, false),
            (330 * n, false),
            (308 * n, false),
            (307 * n, true),
            (200 * n, true),
            (100 * n, true),
        ] {
            let context = format!("ppp {ppp}, alto {height_2}");
            let mut bench =
                Bench::with_panel_height(ppp, height_2, image_indexed_256(), ColorChoice::Entry(5));
            let f = bench.rest_2_2();
            assert_eq!(bench.layout().scrolling, pans, "{context}");
            checks_bar(&f, &bench, &context);
            if !pans {
                continue;
            }
            // En reposo, a media altura y abajo del todo.
            let max = bench.max_offset();
            let (x, y) = bench.over_the_text();
            bench.moves(x, y);
            for goal in [max / 2 / n * n, max] {
                bench.wheel(-100_000 * n);
                bench.wheel(goal);
                assert_eq!(bench.view.offset_px, goal, "{context}");
                let f = bench.rest_2_2();
                checks_bar(&f, &bench, &format!("{context}, offset {goal}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: clics, recorte y arrastre con el panel desplazado
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_a_click_on_a_slider_finds_the_control_with_y_minus_offset() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 270·n: la fila de R (244·n a 258·n con el panel sin desplazar) se ve con 20·n de desplazamiento.
        let mut bench = Bench::with_panel_height(ppp, 270 * n, image_f(), ColorChoice::Entry(1));
        let (x, y) = bench.over_the_text();
        bench.wheel_in(x, y, 20 * n);
        assert_eq!(bench.view.offset_px, 20 * n);
        let geo = bench.geo();
        assert_eq!(geo.top_px, 86 * n);

        // El centro de la fila de R, ya desplazada, es R.
        let y_red = geo.y_row(Control::Red);
        let x_a = geo.x_of_value(128, Control::Red.max());
        bench.moves(x_a, y_red);
        bench.presses_2(x_a, y_red);
        assert_eq!(
            bench.editor.dragging_control(),
            Some(Control::Red),
            "ppp {ppp}: el control se halla con y − offset"
        );
        let value = geo.value_in(x_a - geo.x_hint(), Control::Red);
        assert_eq!(
            bench.editor.preview(),
            Some((EditTarget::Entry(1), Rgba::opaque(value as u8, 255, 0)))
        );
        // Soltar aplica a la entrada 1.
        let f = bench.releases(x_a, y_red);
        assert_eq!(
            f.output_2.commit,
            Some(Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(value as u8, 255, 0)
            })
        );

        // El sitio donde estaría R sin desplazar (y = 138·n) es ahora la fila de G.
        let y_without_pan = (106 + COLOR_FIRST_ROW_Y_U + ROW_H_U / 2) * n;
        bench.moves(x_a, y_without_pan);
        bench.presses_2(x_a, y_without_pan);
        assert_eq!(
            bench.editor.dragging_control(),
            Some(Control::Green),
            "ppp {ppp}: y = {y_without_pan} cae ya en G"
        );
        bench.releases(x_a, y_without_pan);
    }
}

#[test]
fn row_5_a_click_on_the_field_with_offset_gives_it_the_focus() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = Bench::with_panel_height(ppp, 200 * n, image_f(), ColorChoice::Entry(1));
        let (x, y) = bench.over_the_text();
        bench.wheel_in(x, y, 36 * n);
        assert_eq!(bench.view.offset_px, 36 * n);
        // El campo está en la parte visible (y = 18·n − 36·n + 42·n + 9·n = 33·n < 200·n).
        let (x, y) = bench.geo().field_2();
        assert!(y < bench.panel_h_px());
        bench.click(x, y);
        bench.rest_2_2();
        assert!(bench.wants_keyboard(), "ppp {ppp}: el clic enfoca el campo");
        // Tras teclear algo y salir, el texto va a la entrada 1.
        bench.writes("#ff00ff");
        bench.click_on_the_canvas();
        bench.rest_2_2();
        assert_eq!(
            bench.commits(),
            vec![Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(255, 0, 255)
            }]
        );
    }
}

#[test]
fn row_5_outside_the_panel_rectangle_nothing_is_hit() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // Panel de 266·n y sin desplazar: la fila de B (272..286·n) queda por debajo del panel,
        // bajo la barra de estado (266..286·n), y la de G (258..272·n) asoma por arriba.
        let mut bench = Bench::with_panel_height(ppp, 266 * n, image_f(), ColorChoice::Entry(1));
        let geo = bench.geo();
        assert_eq!(geo.top_px, 106 * n);
        let x = geo.x_of_value(128, 255);
        let y_blue = geo.y_row(Control::Blue);
        assert!(y_blue >= 266 * n, "B está fuera del panel");
        bench.moves(x, y_blue);
        bench.presses_2(x, y_blue);
        assert!(
            !bench.editor.dragging(),
            "ppp {ppp}: un clic fuera del panel no empieza un arrastre"
        );
        bench.releases(x, y_blue);
        assert!(bench.commits().is_empty());

        // Contraste: la fila de G, dentro del panel (258..266·n, y = 262·n), sí.
        let y_green = 262 * n;
        bench.moves(x, y_green);
        bench.presses_2(x, y_green);
        assert_eq!(bench.editor.dragging_control(), Some(Control::Green));
        bench.releases(x, y_green);
    }
}

#[test]
fn row_5_a_click_on_the_part_of_the_field_that_falls_outside_the_panel_does_not_focus_it() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 66·n sin desplazar: el campo va de 60 a 78·n y el panel acaba en 66·n. Egui acepta
        // clics hasta 5 puntos fuera de un widget (`interact_radius`): se pulsa a más de eso (en
        // 76·n), sobre la barra de estado.
        let mut bench = Bench::with_panel_height(ppp, 66 * n, image_f(), ColorChoice::Entry(1));
        let geo = bench.geo();
        assert_eq!(geo.top_px, 106 * n);
        let (x, _) = geo.field_2();
        bench.click(x, 76 * n);
        bench.rest_2_2();
        assert!(
            !bench.wants_keyboard(),
            "ppp {ppp}: la parte del campo bajo el panel no recibe el clic"
        );
        // La parte de dentro (y = 63·n, de 60 a 66·n) sí.
        bench.click(x, 63 * n);
        bench.rest_2_2();
        assert!(bench.wants_keyboard(), "ppp {ppp}: la de dentro sí");
    }
}

#[test]
fn row_5_everything_painted_stays_inside_the_panel_rectangle() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for height_2 in [330 * n, 308 * n, 307 * n, 200 * n, 100 * n] {
            let mut bench =
                Bench::with_panel_height(ppp, height_2, image_indexed_256(), ColorChoice::Entry(5));
            let panel = bench.rect_panel();
            let context = format!("ppp {ppp}, alto {height_2}, offset 0");
            let f = bench.rest_2_2();
            checks_inside_the_panel(&f, panel, &context);
            if !bench.layout().scrolling {
                continue;
            }
            let (x, y) = bench.over_the_text();
            bench.moves(x, y);
            let max = bench.max_offset();
            for goal in [n, max / 2 / n * n, max] {
                bench.wheel(-100_000 * n);
                bench.wheel(goal);
                let context = format!(
                    "ppp {ppp}, alto {height_2}, offset {}",
                    bench.view.offset_px
                );
                let f = bench.rest_2_2();
                checks_inside_the_panel(&f, panel, &context);
            }
        }
    }
}

#[test]
fn row_5_the_focused_field_and_the_drag_are_also_clipped_to_the_panel() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = Bench::with_panel_height(ppp, 236 * n, image_f(), ColorChoice::Entry(1));
        let panel = bench.rect_panel();
        bench.focuses_field();
        bench.writes("#ff00ff");
        let f = bench.rest_2_2();
        checks_inside_the_panel(&f, panel, &format!("ppp {ppp}: campo enfocado"));
        bench.click_on_the_canvas();

        let geo = bench.geo();
        let (x, y) = (geo.x_of_value(100, 255), geo.y_row(Control::Red));
        bench.moves(x, y);
        bench.presses_2(x, y);
        let f = bench.rest_2_2();
        checks_inside_the_panel(&f, panel, &format!("ppp {ppp}: arrastrando"));
    }
}

#[test]
fn row_5_lowering_the_window_does_not_cancel_a_drag() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = Bench::with_panel_height(ppp, 300 * n, image_f(), ColorChoice::Entry(1));
        let geo = bench.geo();
        let y = geo.y_row(Control::Red);
        let x_a = geo.x_of_value(128, Control::Red.max());
        bench.moves(x_a, y);
        bench.presses_2(x_a, y);
        assert!(bench.editor.dragging(), "ppp {ppp}: el arrastre empieza");
        let provisional = bench.editor.preview();
        assert!(provisional.is_some());

        // La ventana baja hasta el bloque, por debajo (se desplaza) y hasta el mínimo.
        for height_2 in [308 * n, 307 * n, 200 * n, 100 * n] {
            bench.height_of_panel(height_2);
            let f = bench.rest_2_2();
            assert!(
                bench.editor.dragging(),
                "ppp {ppp}: el arrastre sigue con el panel de {height_2} px"
            );
            assert_eq!(bench.editor.preview(), provisional, "ppp {ppp}, {height_2}");
            assert_eq!(f.output_2.commit, None);
        }

        // Seguir moviendo y soltar da el paso, con la geometría nueva.
        let geo = bench.geo();
        let x_b = geo.x_of_value(200, Control::Red.max());
        let y_b = geo.y_row(Control::Green);
        bench.moves(x_b, y_b);
        bench.releases(x_b, y_b);
        let value = geo.value_in(x_b - geo.x_hint(), Control::Red);
        assert_eq!(
            bench.commits(),
            vec![Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(value as u8, 255, 0)
            }],
            "ppp {ppp}: soltar aplica el valor de la `x` final"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: sin aviso de ventana baja
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6_neither_when_lowering_nor_when_raising_the_window_are_there_notices() {
    let ppp = 1.0;
    let mut bench = bench_f(ppp, 1);
    for height_2 in [330, 308, 307, 100, 308, 700] {
        bench.height_of_panel(height_2);
        let f = bench.rest_2_2();
        assert_eq!(f.output_2.notice, None, "alto {height_2}");
        assert!(f.paints("Idx-1"), "alto {height_2}");
    }
    assert!(bench.notices().is_empty());
    assert!(bench.commits().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Filas 7, 8 y 9: texto pendiente del campo hexadecimal
// ---------------------------------------------------------------------------------------------

/// Banco a 700·n con el campo enfocado y `written` pendiente en la entrada `input_2` de `F`
/// (o en la ranura de la imagen RGBA si `image` lo es).
fn bench_with_pending(image: Image, choice: ColorChoice, written: &str) -> Bench {
    let mut bench = Bench::new_2(1.0, SCREEN_HIGH, image, choice);
    bench.focuses_field();
    bench.writes(written);
    bench.rest_2_2();
    assert!(bench.editor.has_pending(), "se tecleó {written:?}");
    bench.history.clear();
    bench
}

fn only_commit(bench: &Bench, target: EditTarget, color: Rgba) {
    assert_eq!(bench.commits(), vec![Commit { target, color }]);
}

fn without_commit_and_with_a_notice(bench: &Bench, reason_2: StaleReason) {
    assert!(bench.commits().is_empty(), "no hay commit");
    assert_eq!(bench.notices(), vec![stale_notice(reason_2).to_owned()]);
    assert!(!bench.editor.has_pending());
    assert_eq!(bench.editor.hex_display(), None);
}

// Fila 7.

/// **[DEPENDE DE EGUI]** Un clic fuera quita el foco al soltar.
#[test]
fn row_7_losing_focus_applies_the_text_to_the_original_entry() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    bench.click_on_the_canvas();
    bench.rest_2_2();
    only_commit(&bench, EditTarget::Entry(2), GREEN);
    assert!(bench.notices().is_empty());
    assert!(!bench.editor.has_pending());
    assert!(!bench.wants_keyboard());
    // No se repite.
    for _ in 0..3 {
        bench.rest_2_2();
    }
    assert_eq!(bench.commits().len(), 1);
}

#[test]
fn row_7_being_left_without_a_target_applies_the_text_to_the_original_entry() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    // La elección pasa a una entrada que no existe: no hay editor.
    bench.choice = ColorChoice::Entry(200);
    let f = bench.rest_2_2();
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Entry(2),
            color: GREEN
        }),
        "el panel resuelve el texto en el mismo fotograma en que se queda sin destino"
    );
    assert_eq!(f.output_2.notice, None);
    assert!(
        !f.paints_something_with("Idx-"),
        "ya no hay barra ni editor"
    );
    assert!(!bench.editor.has_pending());
    assert_eq!(bench.editor.hex_display(), None);
    for _ in 0..3 {
        bench.rest_2_2();
    }
    only_commit(&bench, EditTarget::Entry(2), GREEN);
    assert!(bench.notices().is_empty());
}

#[test]
fn row_7_the_text_does_not_reappear_when_the_editor_returns_or_with_another_entry() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    bench.choice = ColorChoice::Entry(200);
    bench.rest_2_2();
    bench.rest_2_2();
    assert_eq!(bench.commits().len(), 1);

    // Vuelve el editor con la misma entrada: muestra el color guardado, no el texto viejo.
    bench.choice = ColorChoice::Entry(2);
    let f = bench.rest_2_2();
    assert!(f.paints(&hex_text(BLUE)), "muestra el guardado");
    assert!(!f.paints("#00ff00"));
    // Y con otra entrada.
    bench.choice = ColorChoice::Entry(0);
    let f = bench.rest_2_2();
    assert!(f.paints(&hex_text(RED)));
    assert!(!f.paints("#00ff00"));
    assert_eq!(bench.editor.hex_display(), None);

    // Un clic dentro y fuera del campo no aplica nada a la entrada original.
    bench.focuses_field();
    bench.click_on_the_canvas();
    bench.rest_2_2();
    assert_eq!(bench.commits().len(), 1, "sin commits nuevos");
    assert!(bench.notices().is_empty());
}

#[test]
fn row_7_without_a_target_with_invalid_text_it_warns_about_the_hexadecimal() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "zzz");
    bench.choice = ColorChoice::Entry(200);
    let f = bench.rest_2_2();
    assert_eq!(f.output_2.commit, None);
    assert_eq!(
        f.output_2.notice.as_deref(),
        Some(hex_notice(&px_ui::color_edit::HexError::Digit)),
        "aviso del hexadecimal inválido"
    );
    assert!(!bench.editor.has_pending());
    for _ in 0..3 {
        bench.rest_2_2();
    }
    assert_eq!(bench.notices().len(), 1);
    assert!(bench.commits().is_empty());
}

#[test]
fn row_7_with_a_slot_and_the_valid_text_it_is_applied_to_the_original_slot() {
    // La ranura 3 de RGBA es roja por defecto: teclear verde cambia el color.
    let mut bench = bench_with_pending(image_rgba(), ColorChoice::Slot(3), "#00ff00");
    bench.choice = ColorChoice::Slot(10);
    let f = bench.rest_2_2();
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Slot(3),
            color: GREEN
        })
    );
    assert!(!bench.editor.has_pending());
}

// Fila 8: la entrada ya no existe.

/// **[DEPENDE DE EGUI]** La imagen tiene una paleta más corta y la elección se movió a otra
/// entrada: el campo sigue pintado y pierde el foco con un clic fuera.
#[test]
fn row_8_the_entry_no_longer_exists_and_the_focus_is_lost() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(3), "#00ff00");
    let image = ImageId::from_raw(0);
    bench.applies(Command::RemovePaletteEntry {
        image,
        index: 3,
        into: 0,
    });
    assert_eq!(bench.image.palette().expect("paleta").len(), 3);
    bench.choice = ColorChoice::Entry(0);
    bench.rest_2_2();
    assert!(bench.editor.has_pending(), "el texto sigue pendiente");
    bench.click_on_the_canvas();
    bench.rest_2_2();
    without_commit_and_with_a_notice(&bench, StaleReason::Gone);
    // Una vez: no se repite.
    for _ in 0..3 {
        bench.rest_2_2();
    }
    assert_eq!(bench.notices().len(), 1);
}

#[test]
fn row_8_the_entry_no_longer_exists_and_it_is_left_without_a_target() {
    // Una imagen con la paleta más corta, con la misma elección: no hay destino.
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(3), "#00ff00");
    let short = Image::new_indexed(64, 48, Palette::from_colors([RED, GREEN]).expect("paleta"))
        .expect("imagen");
    bench.image = short;
    assert!(bench.actual().is_none());
    let f = bench.rest_2_2();
    assert_eq!(f.output_2.commit, None, "sin Commit");
    assert_eq!(
        f.output_2.notice.as_deref(),
        Some(stale_notice(StaleReason::Gone)),
        "texto descartado: la entrada ya no existe"
    );
    without_commit_and_with_a_notice(&bench, StaleReason::Gone);
    for _ in 0..3 {
        bench.rest_2_2();
    }
    assert_eq!(bench.notices().len(), 1);
}

#[test]
fn row_8_undoing_an_add_leaves_the_entry_non_existent() {
    let mut bench = bench_f(1.0, 0);
    let image = ImageId::from_raw(0);
    bench.applies(Command::AddPaletteEntry {
        image,
        color: YELLOW,
    });
    bench.choice = ColorChoice::Entry(4);
    bench.rest_2_2();
    bench.focuses_field();
    bench.writes("#00ff00");
    bench.rest_2_2();
    assert!(bench.editor.has_pending());
    bench.history.clear();

    bench.undoes();
    assert_eq!(bench.image.palette().expect("paleta").len(), 4);
    let f = bench.rest_2_2();
    assert_eq!(f.output_2.commit, None);
    without_commit_and_with_a_notice(&bench, StaleReason::Gone);
    assert!(
        !f.paints_something_with("Idx-"),
        "sin destino no hay barra ni editor"
    );
}

// Fila 9: el color guardado cambió.

#[test]
fn row_9_the_color_changed_and_the_focus_is_lost() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    bench.set_color(2, RED);
    bench.rest_2_2();
    assert!(bench.editor.has_pending());
    bench.click_on_the_canvas();
    bench.rest_2_2();
    without_commit_and_with_a_notice(&bench, StaleReason::Changed);
    for _ in 0..3 {
        bench.rest_2_2();
    }
    assert_eq!(bench.notices().len(), 1, "el aviso no se repite");
}

#[test]
fn row_9_the_color_changed_and_it_is_left_without_a_target() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    bench.set_color(2, RED);
    bench.choice = ColorChoice::Entry(200);
    let f = bench.rest_2_2();
    assert_eq!(f.output_2.commit, None);
    assert_eq!(
        f.output_2.notice.as_deref(),
        Some(stale_notice(StaleReason::Changed)),
        "texto descartado: el color cambió"
    );
    without_commit_and_with_a_notice(&bench, StaleReason::Changed);
}

#[test]
fn row_9_undoing_a_set_palette_color_with_pending_text_discards() {
    let mut bench = bench_f(1.0, 2);
    bench.set_color(2, RED);
    bench.rest_2_2();
    bench.focuses_field();
    bench.writes("#00ff00");
    bench.rest_2_2();
    bench.history.clear();

    bench.undoes();
    bench.rest_2_2();
    bench.click_on_the_canvas();
    bench.rest_2_2();
    without_commit_and_with_a_notice(&bench, StaleReason::Changed);
}

#[test]
fn row_9_redoing_a_set_palette_color_with_pending_text_discards() {
    let mut bench = bench_f(1.0, 2);
    bench.set_color(2, RED);
    bench.undoes();
    bench.rest_2_2();
    bench.focuses_field();
    bench.writes("#00ff00");
    bench.rest_2_2();
    bench.history.clear();

    bench.redoes();
    bench.rest_2_2();
    bench.choice = ColorChoice::Entry(200);
    bench.rest_2_2();
    without_commit_and_with_a_notice(&bench, StaleReason::Changed);
}

#[test]
fn row_9_if_the_color_goes_back_to_the_base_one_it_is_applied() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    bench.set_color(2, RED);
    bench.rest_2_2();
    bench.undoes();
    assert_eq!(bench.actual().map(|(_, c)| c), Some(BLUE));
    bench.rest_2_2();
    assert!(bench.editor.has_pending());
    bench.click_on_the_canvas();
    bench.rest_2_2();
    only_commit(&bench, EditTarget::Entry(2), GREEN);
    assert!(bench.notices().is_empty());
    assert!(!bench.editor.has_pending());

    // Y con rehacer: cambia, vuelve a lo que era al rehacer el cambio y deshacerlo otra vez.
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(2), "#00ff00");
    bench.set_color(2, RED);
    bench.undoes();
    bench.redoes();
    bench.undoes();
    bench.choice = ColorChoice::Entry(200);
    bench.rest_2_2();
    only_commit(&bench, EditTarget::Entry(2), GREEN);
}

/// **[DEPENDE DE EGUI]**
#[test]
fn row_9_the_slot_changes() {
    // La ranura 3 de RGBA es roja por defecto.
    let mut bench = bench_with_pending(image_rgba(), ColorChoice::Slot(3), "#00ff00");
    assert!(bench.slots.set(3, BLUE), "la ranura 3 existe");
    bench.rest_2_2();
    bench.click_on_the_canvas();
    bench.rest_2_2();
    without_commit_and_with_a_notice(&bench, StaleReason::Changed);

    let mut bench = bench_with_pending(image_rgba(), ColorChoice::Slot(3), "#00ff00");
    assert!(bench.slots.set(3, BLUE));
    bench.choice = ColorChoice::Slot(10);
    bench.rest_2_2();
    without_commit_and_with_a_notice(&bench, StaleReason::Changed);
}

// Texto pendiente y ventana baja: el campo no desaparece.

#[test]
fn shrinking_and_enlarging_the_window_does_not_lose_the_pending_text() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_f(), ColorChoice::Entry(2));
        bench.focuses_field();
        bench.writes("#00ff00");
        bench.rest_2_2();
        bench.history.clear();
        for height_2 in [
            300 * n,
            308 * n,
            200 * n,
            100 * n,
            200 * n,
            308 * n,
            500 * n,
        ] {
            bench.height_of_panel(height_2);
            let f = bench.rest_2_2();
            assert!(bench.editor.has_pending(), "ppp {ppp}, alto {height_2}");
            assert_eq!(
                bench.editor.hex_display().as_deref(),
                Some("#00ff00"),
                "ppp {ppp}, alto {height_2}"
            );
            assert_eq!(f.output_2.commit, None, "ppp {ppp}, alto {height_2}");
            assert_eq!(f.output_2.notice, None, "ppp {ppp}, alto {height_2}");
        }
        assert!(bench.commits().is_empty());
        assert!(bench.notices().is_empty());
        let f = bench.rest_2_2();
        assert!(
            f.paints("#00ff00"),
            "ppp {ppp}: el campo muestra lo tecleado"
        );

        // Al salir del campo, se aplica a la entrada original.
        bench.click_on_the_canvas();
        bench.rest_2_2();
        only_commit(&bench, EditTarget::Entry(2), GREEN);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: mostrar la elegida
// ---------------------------------------------------------------------------------------------

fn row_of(input_2: usize) -> usize {
    input_2 / COLUMNS
}

/// La fila de la elegida está entre las `visible_2` que empiezan en la primera del banco.
fn chosen_visible(bench: &Bench, visible_2: usize) -> bool {
    let row_2 =
        row_of(cell_of_choice(&bench.image, bench.choice).expect("la elegida tiene muestra"));
    bench.scroll.first_row <= row_2 && row_2 < bench.scroll.first_row + visible_2
}

fn bench_256(ppp: f32, panel_h_px: i64, input_2: u8, warm_up: bool) -> Bench {
    Bench::new_with(
        ppp,
        screen_for_panel(ppp, panel_h_px),
        image_indexed_256(),
        ColorChoice::Entry(input_2),
        warm_up,
    )
}

#[test]
fn row_11_the_first_frame_does_not_show_the_chosen_one() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 467·n (400·n de antes, más lo que ocupan ahora la cola y el editor): 12 filas visibles de 17 y la elegida (200 → fila 12) fuera de la vista.
        let mut bench = bench_256(ppp, 467 * n, 200, false);
        assert_eq!(bench.view, PanelView::default());
        bench.rest_2_2();
        assert_eq!(bench.view.last_visible, Some(12), "ppp {ppp}");
        assert_eq!(bench.scroll.first_row, 0, "ppp {ppp}: primer fotograma");
        assert!(!bench.view.reveal);
        // Ni en los siguientes mientras nada cambie.
        for _ in 0..3 {
            bench.rest_2_2();
        }
        assert_eq!(
            bench.scroll.first_row, 0,
            "ppp {ppp}: sin cambios no hay reveal"
        );
        assert!(!chosen_visible(&bench, 12));
    }
}

#[test]
fn row_11_changing_the_number_of_visible_rows_shows_the_chosen_one_with_the_minimum_movement() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = bench_256(ppp, 467 * n, 200, true);
        assert_eq!(bench.scroll.first_row, 0);
        assert_eq!(bench.view.last_visible, Some(12));

        // 367·n: 5 filas visibles. La fila 12 pasa a ser la última visible (primera = 8).
        bench.height_of_panel(367 * n);
        bench.rest_2_2();
        assert_eq!(bench.view.last_visible, Some(5), "ppp {ppp}");
        assert_eq!(bench.scroll.first_row, 8, "ppp {ppp}: el mínimo movimiento");
        assert_eq!(bench.scroll.rest, 0);
        assert!(chosen_visible(&bench, 5));
        // Sin más cambios no se vuelve a llamar: lo que se desplace a mano se queda.
        bench.rest_2_2();
        assert_eq!(bench.scroll.first_row, 8);

        // Agrandar: 12 filas visibles; la elegida sigue a la vista.
        bench.height_of_panel(467 * n);
        bench.rest_2_2();
        assert_eq!(bench.view.last_visible, Some(12));
        assert!(chosen_visible(&bench, 12), "ppp {ppp}");
        assert_eq!(
            bench.scroll.first_row, 5,
            "ppp {ppp}: recortado al máximo (17 − 12)"
        );
    }
}

#[test]
fn row_11_when_lowering_to_the_fallback_the_chosen_one_stays_in_view() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // 700·n: caben las 17 filas.
        let mut bench = bench_256(ppp, 700 * n, 200, true);
        assert_eq!(bench.scroll.first_row, 0);
        assert!(bench.view.last_visible.is_some_and(|v| v >= 17));

        // 200·n: una sola fila visible (respaldo): tiene que ser la de la elegida.
        bench.height_of_panel(200 * n);
        bench.rest_2_2();
        assert_eq!(bench.view.last_visible, Some(1), "ppp {ppp}");
        assert_eq!(bench.scroll.first_row, 12, "ppp {ppp}");
        assert!(chosen_visible(&bench, 1));
    }
}

#[test]
fn row_11_view_reveal_shows_the_chosen_one_and_the_panel_turns_it_off() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = bench_256(ppp, 467 * n, 200, true);
        assert_eq!(bench.scroll.first_row, 0);
        // Como las teclas 1 a 9, deshacer o rehacer: la sesión lo pone y el panel lo apaga.
        bench.view.reveal = true;
        bench.rest_2_2();
        assert!(!bench.view.reveal, "ppp {ppp}: el panel lo apaga");
        assert_eq!(
            bench.scroll.first_row, 1,
            "ppp {ppp}: la fila 12 pasa a ser la última visible"
        );
        assert!(chosen_visible(&bench, 12));

        // Ya visible: no se mueve, pero el aviso se apaga igual.
        bench.view.reveal = true;
        bench.rest_2_2();
        assert!(!bench.view.reveal);
        assert_eq!(bench.scroll.first_row, 1, "ppp {ppp}");

        // Por encima: sube lo justo.
        bench.choice = ColorChoice::Entry(0);
        bench.rest_2_2();
        assert_eq!(
            bench.scroll.first_row, 1,
            "cambiar la elección sin `reveal` no mueve"
        );
        bench.view.reveal = true;
        bench.rest_2_2();
        assert_eq!(bench.scroll.first_row, 0, "ppp {ppp}");
    }
}

#[test]
fn row_11_view_reveal_with_a_choice_without_a_swatch_moves_nothing_and_turns_off() {
    let mut bench = bench_f(1.0, 1);
    bench.choice = ColorChoice::Entry(200);
    bench.scroll.first_row = 0;
    bench.view.reveal = true;
    bench.rest_2_2();
    assert!(!bench.view.reveal);
    assert_eq!(bench.scroll.first_row, 0);
}

#[test]
fn row_11_view_reveal_in_the_first_frame_does_show_the_chosen_one() {
    let n = 1;
    let mut bench = bench_256(1.0, 467 * n, 200, false);
    bench.view.reveal = true;
    bench.rest_2_2();
    assert!(!bench.view.reveal);
    assert_eq!(bench.scroll.first_row, 1);
}

#[test]
fn row_11_the_wheel_does_not_show_the_chosen_one() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = bench_256(ppp, 367 * n, 0, true);
        assert_eq!(bench.view.last_visible, Some(5));
        let (x, y) = center_of_swatch(n, 0, 0);
        bench.moves(x, y);
        // Tres filas hacia abajo: la elegida (fila 0) sale de la vista.
        bench.wheel(3 * PITCH_U * n);
        assert_eq!(bench.scroll.first_row, 3, "ppp {ppp}");
        for _ in 0..3 {
            bench.rest_2_2();
        }
        assert_eq!(bench.scroll.first_row, 3, "ppp {ppp}: la rueda manda");
        bench.wheel(PITCH_U * n);
        assert_eq!(bench.scroll.first_row, 4, "ppp {ppp}");
        assert!(!chosen_visible(&bench, 5));
    }
}

#[test]
fn row_11_a_click_on_a_swatch_does_not_show_the_chosen_one() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = bench_256(ppp, 387 * n, 0, true);
        let (x, y) = center_of_swatch(n, 0, 0);
        bench.moves(x, y);
        bench.wheel(3 * PITCH_U * n);
        assert_eq!(bench.scroll.first_row, 3);
        bench.rest_2_2();

        // Un clic en una muestra de la fila 3 (la primera visible): la elección es de la
        // sesión y el panel no recoloca la rejilla, ni con la elección vieja (fila 0) ni con la
        // nueva.
        let (x, y) = center_of_swatch(n, 3 * COLUMNS + 2, 3);
        bench.moves(x, y);
        bench.presses_2(x, y);
        bench.releases(x, y);
        assert_eq!(
            bench.choices(),
            vec![ColorChoice::Entry((3 * COLUMNS + 2) as u8)],
            "ppp {ppp}"
        );
        assert_eq!(bench.scroll.first_row, 3, "ppp {ppp}: tras el clic");
        bench.choice = ColorChoice::Entry((3 * COLUMNS + 2) as u8);
        for _ in 0..3 {
            bench.rest_2_2();
        }
        assert_eq!(
            bench.scroll.first_row, 3,
            "ppp {ppp}: tras aplicar la elección"
        );
        assert!(!bench.view.reveal);
    }
}

#[test]
fn row_11_dragging_a_slider_does_not_show_the_chosen_one() {
    let ppp = 1.0;
    let n = 1;
    let mut bench = bench_256(ppp, 467 * n, 0, true);
    let (x, y) = center_of_swatch(n, 0, 0);
    bench.moves(x, y);
    bench.wheel(3 * PITCH_U * n);
    assert_eq!(bench.scroll.first_row, 3);
    let geo = bench.geo();
    let (x, y) = (geo.x_of_value(100, 255), geo.y_row(Control::Green));
    bench.moves(x, y);
    bench.presses_2(x, y);
    bench.moves(x + 10, y);
    bench.releases(x + 10, y);
    bench.rest_2_2();
    assert_eq!(bench.scroll.first_row, 3);
}
