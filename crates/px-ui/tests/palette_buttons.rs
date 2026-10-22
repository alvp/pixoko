//! Tests del panel de paletas con los botones y las operaciones de la unidad 11d, dentro de la
//! ventana (specs/hito-2-paleta-operaciones.md, tabla «Casos», filas 2, 3 a 6, 7, 9, 10, 13 y 14
//! en lo que toca al panel y al banco). Lo puro (`palette_ops`, `Grid::drop_index`, `MoveDrag`,
//! `button_hit`, `edges_u`) se prueba en `palette_ops.rs`, `palette_grid.rs` y `edit_layout.rs`;
//! `Session` y `apply_palette_op`, en `px-app`. Aquí se prueba que `show_editing_ops` lo une bien
//! a `egui`: qué se pinta, dónde se pulsa y qué `PanelOutput` sale.
//!
//! El banco es el de `tests/panel_baja.rs`, copiado (ese fichero no se toca) y ampliado: un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hace `main`, se llama a `PaletteOps::sync(doc_state, len)` con un `doc_state`
//! que controla el test, a `ColorEditor::sync`, se monta `status::show`, luego
//! `palette_panel::show_editing_ops` con un `PanelView` y un `PaletteOps` que **persisten entre
//! fotogramas** y por fin el lienzo. `enabled` es `!busy`, con `busy` = un gesto de dibujo
//! (`gesture_drawing`, lo pone el test) o un deslizador o un arrastre de muestra en curso. A
//! diferencia de `panel_baja.rs`, el banco **sí aplica la elección** que devuelve el panel (como
//! `main`), para poder encadenar gestos; **nunca aplica** una `op` ni un `commit` (eso es de
//! `Session`): cada salida del panel se guarda en `bench.history`.
//!
//! Se mide a los cinco `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 (`n` = 1, 1, 2, 2 y 3). Los clics se dan en
//! píxeles físicos enteros. Lo pintado se lee de las formas del fotograma. No hay capturas de
//! pantalla.

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource, MouseWheelUnit,
    PointerButton, Pos2, RawInput, Rect, TextureId, TouchPhase, Vec2, pos2, vec2,
};
use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::Canvas;
use px_ui::color::{ColorChoice, SlotColors, cell_count, initial_choice};
use px_ui::color_edit::{ColorEditor, Commit, Control, EditTarget, slider_x};
use px_ui::edit_layout::{
    BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, FIELD_H_U, FIELD_W_U, FIELD_X_U, PaletteButton, PanelLayout,
    ROW_H_U, TRACK_W_U, TRACK_X_U, button_rect_u, control_row_u, field_y_u, panel_layout,
};
use px_ui::icons::{Icon, IconSet};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, Grid, PANEL_WIDTH_U, PITCH_U, PanelView,
    drop_mark_rect, ring_rects,
};
use px_ui::palette_ops::{PaletteOp, PaletteOps, Removal};
use px_ui::palette_panel::{self, PanelInfo, PanelOutput, panel_info};
use px_ui::status::bar_height_px;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::{
    ACCENT, DROP_MARK, RING_DEST, RING_RANGE, SURFACE_0, SURFACE_2, SURFACE_3, TEXT, TEXT_DISABLED,
};
use px_ui::view::ScreenRect;

#[path = "common/icons.rs"]
mod icons;
#[path = "common/zones.rs"]
mod zones;
use icons::IconsOfTest;
use zones::panel_on_the_left;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BLACK: Rgba = Rgba::opaque(0, 0, 0);
/// `X` y `Y` de la paleta `P5` (fila «colores intermedios» de `hito-2-color-cercano.md`).
const X: Rgba = Rgba::opaque(200, 30, 30);
const Y: Rgba = Rgba::opaque(10, 10, 240);

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Indexada con los píxeles `pixels_2` (una fila) y la paleta `colors_2`, con el transparente dado.
fn image_with(colors_2: &[Rgba], transparent: Option<u8>, pixels_2: &[u8]) -> Image {
    let mut palette = Palette::from_colors(colors_2.iter().copied()).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("transparente");
    let mut image_2 =
        Image::new_indexed(pixels_2.len() as u32, 1, palette).expect("imagen indexada");
    for (x, index_2) in pixels_2.iter().enumerate() {
        image_2
            .set_pixel(x as u32, 0, Pixel::Indexed(*index_2))
            .expect("píxel");
    }
    image_2
}

/// `F`: `[R, G, B, K]` 4×1 con los píxeles `[0, 1, 2, 3]` (todas las entradas se usan).
fn image_f() -> Image {
    image_with(&[RED, GREEN, BLUE, TEST_BLACK], None, &[0, 1, 2, 3])
}

/// `P5`: `[R, G, B, X, Y]` 5×1 con los píxeles `[0, 1, 2, 3, 3]` (la 4 no la usa nadie).
fn image_p_5() -> Image {
    image_with(&[RED, GREEN, BLUE, X, Y], None, &[0, 1, 2, 3, 3])
}

/// `P5` con la entrada `transparent` como transparente.
fn transparent_p5_image(transparent: u8) -> Image {
    image_with(
        &[RED, GREEN, BLUE, X, Y],
        Some(transparent),
        &[0, 1, 2, 3, 3],
    )
}

/// Paleta de una sola entrada.
fn image_a_input() -> Image {
    image_with(&[RED], None, &[0, 0])
}

/// Indexada de 64×48 con 256 colores (16 filas de la rejilla); todos los píxeles en la entrada 0.
fn image_indexed_256() -> Image {
    let colors_2: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect();
    let palette = Palette::from_colors(colors_2).expect("paleta de 256 colores");
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

fn button_2(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    }
}

fn text_2(s: &str) -> Event {
    Event::Text(s.to_owned())
}

fn key_2(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
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
    color: Color32,
}

/// Una malla con textura (un icono): la textura, el rectángulo que cubre y el color de sus vértices
/// (la tinta).
#[derive(Clone, Copy, Debug)]
struct TestMesh {
    texture_2: TextureId,
    rect: Rect,
    /// Rectángulo que cubren las coordenadas de textura de sus vértices.
    uv: Rect,
    color: Color32,
}

fn collects(
    shape: &Shape,
    texts: &mut Vec<Painted>,
    rects: &mut Vec<(Rect, Color32)>,
    meshes: &mut Vec<TestMesh>,
) {
    match shape {
        Shape::Mesh(mesh) => {
            let points: Vec<Pos2> = mesh.vertices.iter().map(|v| v.pos).collect();
            let uvs: Vec<Pos2> = mesh.vertices.iter().map(|v| v.uv).collect();
            meshes.push(TestMesh {
                texture_2: mesh.texture_id,
                rect: Rect::from_points(&points),
                uv: Rect::from_points(&uvs),
                color: mesh
                    .vertices
                    .first()
                    .map_or(Color32::PLACEHOLDER, |v| v.color),
            });
        }
        Shape::Text(t) => texts.push(Painted {
            text_2: t.galley.text().to_owned(),
            pos: t.pos,
            color: t.fallback_color,
        }),
        Shape::Rect(r) => rects.push((r.rect, r.fill)),
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects(shape_2, texts, rects, meshes);
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
    /// Estado de las operaciones de paleta, entre fotogramas, como el de `Session`.
    ops: PaletteOps,
    /// Los iconos de prueba (una textura blanca por icono).
    icons_2: IconsOfTest,
    /// «Estado del documento» que se pasa a `ops.sync`; lo cambia el test (y `applies`, `undoes`
    /// y `redoes`).
    doc_state: u64,
    /// Veces que `ops.sync` canceló una quita pendiente.
    cancellations: usize,
    /// Hay un gesto de dibujo en curso (`enabled` falso).
    gesture_drawing: bool,
    /// Usa `show_editing_view` (sin estado de operaciones) en lugar de `show_editing_ops`.
    without_ops: bool,
    /// Lo que devolvió el panel en cada fotograma, en orden.
    history: Vec<PanelOutput>,
    /// Tiempo del fotograma para los tooltips: sin él (`None`) `RawInput::time` queda vacío, como
    /// en todas las pruebas anteriores; `con_tiempo` lo enciende y avanza 0,1 s por fotograma.
    time_2: std::cell::Cell<Option<f64>>,
}

struct TestFrame {
    output_2: PanelOutput,
    /// What the panel said about the swatch (tooltip and status line) in this frame.
    info: PanelInfo,
    texts: Vec<Painted>,
    rects: Vec<(Rect, Color32)>,
    meshes: Vec<TestMesh>,
    shapes_2: Vec<ClippedShape>,
}

impl TestFrame {
    /// One of the two lines of the panel's information is exactly `searched` (they used to be
    /// painted as fixed text; now they are the tooltip's).
    fn says(&self, searched: &str) -> bool {
        self.info.lines.iter().any(|line| line == searched)
    }

    fn says_something_with(&self, searched: &str) -> bool {
        self.info.lines.iter().any(|line| line.contains(searched))
    }

    fn paints(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2 == searched)
    }

    fn paints_something_with(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2.contains(searched))
    }

    fn text_painted(&self, searched: &str) -> Option<&Painted> {
        self.texts.iter().find(|t| t.text_2 == searched)
    }

    /// La malla del icono, si se pintó.
    fn mesh_of(&self, icons_2: &IconSet, icon: Icon) -> Option<&TestMesh> {
        let id = icons_2.get(icon)?;
        self.meshes.iter().find(|m| m.texture_2 == id)
    }

    fn paints_icon(&self, icons_2: &IconSet, icon: Icon) -> bool {
        self.mesh_of(icons_2, icon).is_some()
    }

    /// La tinta con que se pintó el icono.
    fn ink_of(&self, icons_2: &IconSet, icon: Icon) -> Color32 {
        self.mesh_of(icons_2, icon)
            .unwrap_or_else(|| panic!("no se pinta el icono {icon:?}"))
            .color
    }

    fn color_of(&self, searched: &str) -> Color32 {
        self.text_painted(searched)
            .unwrap_or_else(|| panic!("no se pinta {searched:?}"))
            .color
    }
}

/// Medidas del editor a la escala del banco, en píxeles físicos desde el borde superior del panel.
#[derive(Clone, Copy, Debug)]
struct Geo {
    n: i64,
    mode: Mode,
    top_px: i64,
    /// Top edge of the icon row (the buttons) from the top edge of the panel.
    icons_top_px: i64,
}

impl Geo {
    /// `y` del centro de la fila del deslizador `control`.
    fn y_row(&self, control: Control) -> i64 {
        let row_2 = control_row_u(self.mode, control).expect("el modo tiene ese deslizador");
        self.top_px + row_2 * self.n + ROW_H_U * self.n / 2
    }

    fn x_of_value(&self, value: u32, max: u32) -> i64 {
        TRACK_X_U * self.n + slider_x(value, TRACK_W_U * self.n, max)
    }

    /// Centro del campo hexadecimal.
    fn field_2(&self) -> (i64, i64) {
        (
            FIELD_X_U * self.n + FIELD_W_U * self.n / 2,
            self.top_px + field_y_u(self.mode) * self.n + FIELD_H_U * self.n / 2,
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

/// Rectángulo en `u` `(x, y, width_2, height_2)` de un botón de indexadas.
fn rect_button_u(button: PaletteButton) -> (i64, i64, i64, i64) {
    button_rect_u(Mode::Indexed, button).expect("las indexadas tienen botones")
}

impl Bench {
    fn new_2(ppp: f32, screen: Vec2, image: Image, choice: ColorChoice) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let icons_2 = IconsOfTest::new_2_2(&ctx);
        let mut bench = Bench {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            view: PanelView {
                icons: icons_2.set,
                ..PanelView::default()
            },
            screen,
            document: Document::new(image.clone()),
            slots: SlotColors::defaults(image.mode()),
            image,
            choice,
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
            icons_2,
            doc_state: 0,
            cancellations: 0,
            gesture_drawing: false,
            without_ops: false,
            history: Vec::new(),
            time_2: std::cell::Cell::new(None),
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

    fn with_panel_height(ppp: f32, panel_h_px: i64, image: Image, choice: ColorChoice) -> Bench {
        Bench::new_2(ppp, screen_for_panel(ppp, panel_h_px), image, choice)
    }

    fn input_2(&self, events: Vec<Event>) -> RawInput {
        let time = self.time_2.get().map(|t| {
            self.time_2.set(Some(t + 0.1));
            t + 0.1
        });
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            time,
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
            icons_top_px: self.layout().icons_top_px,
        }
    }

    fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let target_2 = EditTarget::of(&self.image, self.choice)?;
        let color = target_2.stored(&self.image, &self.slots)?;
        Some((target_2, color))
    }

    fn frame(&mut self, events: Vec<Event>) -> TestFrame {
        let len = self.image.palette().map_or(0, Palette::len);
        if self.ops.sync(self.doc_state, len) {
            self.cancellations += 1;
        }
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !(self.gesture_drawing || self.editor.dragging() || self.ops.busy());
        let without_ops = self.without_ops;
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
            ops,
            ..
        } = self;
        let mut output_2 = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (panel, ()) = panel_on_the_left(
                ui,
                |ui| {
                    if without_ops {
                        palette_panel::show_editing_view(
                            ui, image, *choice, scroll, view, enabled, slots, editor,
                        )
                    } else {
                        palette_panel::show_editing_ops(
                            ui, image, *choice, scroll, view, enabled, slots, editor, ops,
                        )
                    }
                },
                |ui| {
                    let _ = canvas
                        .interact_roles(ui)
                        .into_iter()
                        .map(|(input, _)| input)
                        .collect::<Vec<_>>();
                    let _ = canvas.paint(ui);
                },
            );
            result = Some(panel);
        });
        output_2.textures_delta.clear();
        let shapes_2: Vec<ClippedShape> = std::mem::take(&mut output_2.shapes);
        let panel = result.expect("el panel se dibuja");
        let (mut texts, mut rects, mut meshes) = (Vec::new(), Vec::new(), Vec::new());
        for clipped in &shapes_2 {
            collects(&clipped.shape, &mut texts, &mut rects, &mut meshes);
        }
        if let Some(chosen_2) = panel.choice {
            self.choice = chosen_2;
        }
        self.history.push(panel.clone());
        TestFrame {
            output_2: panel,
            info: panel_info(&self.ctx),
            texts,
            rects,
            meshes,
            shapes_2,
        }
    }

    /// Un fotograma en el que egui recibe los eventos pero el panel no se monta (para perder un
    /// soltar sin que el panel lo vea).
    fn frame_without_panel(&mut self, events: Vec<Event>) {
        let input = self.input_2(events);
        let output_2 = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
        });
        output_2.drop_without_applying_deltas();
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

    fn presses_with(&mut self, x_px: i64, y_px: i64, modifiers: Modifiers) -> TestFrame {
        let pos = in_points(self.ppp(), x_px, y_px);
        self.frame(vec![button_2(pos, true, modifiers)])
    }

    fn presses_2(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        self.presses_with(x_px, y_px, Modifiers::NONE)
    }

    fn releases_with(&mut self, x_px: i64, y_px: i64, modifiers: Modifiers) -> TestFrame {
        let pos = in_points(self.ppp(), x_px, y_px);
        self.frame(vec![button_2(pos, false, modifiers)])
    }

    fn releases(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        self.releases_with(x_px, y_px, Modifiers::NONE)
    }

    /// Mover, pulsar y soltar. Devuelve el fotograma de pulsar (donde actúan botones y muestras).
    fn click_with(&mut self, x_px: i64, y_px: i64, modifiers: Modifiers) -> TestFrame {
        self.moves(x_px, y_px);
        let pressed_2 = self.presses_with(x_px, y_px, modifiers);
        self.releases_with(x_px, y_px, modifiers);
        pressed_2
    }

    fn click(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        self.click_with(x_px, y_px, Modifiers::NONE)
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

    fn wheel_in(&mut self, x_px: i64, y_px: i64, px_below: i64) -> TestFrame {
        self.moves(x_px, y_px);
        self.wheel(px_below)
    }

    /// Un punto del panel sobre el texto de arriba.
    fn over_the_text(&self) -> (i64, i64) {
        (10 * self.n(), 10 * self.n())
    }

    // --- Botones y muestras ---

    /// Centro del botón en píxeles físicos, con el desplazamiento del panel de ahora.
    fn center_of_button(&self, button: PaletteButton) -> (i64, i64) {
        let (x, y, w, h) = rect_button_u(button);
        let n = self.n();
        ((x + w / 2) * n, self.geo().icons_top_px + (y + h / 2) * n)
    }

    /// Rectángulo del botón, en puntos.
    fn rect_of_button(&self, button: PaletteButton) -> Rect {
        let (x, y, w, h) = rect_button_u(button);
        let (n, ppp) = (self.n(), self.ppp());
        let top = self.geo().icons_top_px;
        Rect::from_min_size(
            pos2((x * n) as f32 / ppp, (top + y * n) as f32 / ppp),
            vec2((w * n) as f32 / ppp, (h * n) as f32 / ppp),
        )
    }

    /// La tinta del icono en lo pintado en `f`.
    fn ink(&self, f: &TestFrame, icon: Icon) -> Color32 {
        f.ink_of(&self.icons_2.set, icon)
    }

    /// Clic completo en el botón. Devuelve el fotograma de pulsar.
    fn click_button(&mut self, button: PaletteButton) -> TestFrame {
        let (x, y) = self.center_of_button(button);
        self.click(x, y)
    }

    fn swatch_center(&self, index_2: usize) -> (i64, i64) {
        center_of_swatch(self.n(), index_2, self.scroll.first_row)
    }

    /// Clic en la muestra. Devuelve el fotograma de pulsar.
    fn click_swatch(&mut self, index_2: usize) -> TestFrame {
        let (x, y) = self.swatch_center(index_2);
        self.click(x, y)
    }

    fn click_swatch_with(&mut self, index_2: usize, modifiers: Modifiers) -> TestFrame {
        let (x, y) = self.swatch_center(index_2);
        self.click_with(x, y, modifiers)
    }

    /// Pulsa en el centro de `from_2` y mueve el puntero al centro de `until`, sin soltar.
    fn drags(&mut self, from_2: usize, until: usize) -> TestFrame {
        let (x, y) = self.swatch_center(from_2);
        self.moves(x, y);
        self.presses_2(x, y);
        let (x, y) = self.swatch_center(until);
        self.moves(x, y)
    }

    /// Rectángulo en píxeles físicos desde el origen de la rejilla, en puntos.
    fn rect_of_grid(&self, r: ScreenRect) -> Rect {
        let (n, ppp) = (self.n(), self.ppp());
        let (x, y) = (GRID_X_U * n + r.x, GRID_Y_U * n - self.view.offset_px + r.y);
        Rect::from_min_size(
            pos2(x as f32 / ppp, y as f32 / ppp),
            vec2(r.width as f32 / ppp, r.height as f32 / ppp),
        )
    }

    fn grid(&self) -> Grid {
        Grid::new(
            self.n() as u32,
            cell_count(&self.image),
            self.layout().visible_rows,
            self.scroll.first_row,
        )
    }

    fn cel(&self, index_2: usize) -> ScreenRect {
        self.grid()
            .cell_rect(index_2)
            .unwrap_or_else(|| panic!("la muestra {index_2} se ve"))
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
        self.doc_state += 1;
    }

    fn applies(&mut self, command: Command) {
        self.document.apply(command).expect("comando válido");
        self.refreshes();
    }

    fn undoes(&mut self) {
        self.document.undo().expect("hay algo que deshacer");
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

    fn operations(&self) -> Vec<PaletteOp> {
        self.history.iter().filter_map(|o| o.op).collect()
    }

    /// Todo el historial sin una sola petición: ni elección, ni `commit`, ni aviso, ni `op`.
    fn history_empty(&self) -> bool {
        self.history.iter().all(|o| *o == PanelOutput::default())
    }

    /// Like [`Bench::history_empty`] but a request to close the editor (`Escape` last in the
    /// chain) does not count as something the panel did to the document.
    fn history_empty_but_close(&self) -> bool {
        self.history.iter().all(|o| {
            *o == PanelOutput {
                close_editor: o.close_editor,
                ..PanelOutput::default()
            }
        })
    }
}

fn bench_f(ppp: f32, input_2: u8) -> Bench {
    Bench::new_2(ppp, SCREEN_HIGH, image_f(), ColorChoice::Entry(input_2))
}

fn bench_p_5(ppp: f32, input_2: u8) -> Bench {
    Bench::new_2(ppp, SCREEN_HIGH, image_p_5(), ColorChoice::Entry(input_2))
}

/// Banco a 1,0 con el campo hexadecimal enfocado y `written` pendiente en la entrada `choice`.
fn bench_with_pending(image: Image, choice: ColorChoice, written: &str) -> Bench {
    let mut bench = Bench::new_2(1.0, SCREEN_HIGH, image, choice);
    bench.focuses_field();
    bench.writes(written);
    bench.rest_2_2();
    assert!(bench.editor.has_pending(), "se tecleó {written:?}");
    bench.history.clear();
    bench
}

// ---------------------------------------------------------------------------------------------
// Comprobaciones de lo pintado
// ---------------------------------------------------------------------------------------------

const TOLERANCE: f32 = 0.05;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCE
}

fn near_rect(a: Rect, b: Rect) -> bool {
    near(a.min.x, b.min.x)
        && near(a.min.y, b.min.y)
        && near(a.max.x, b.max.x)
        && near(a.max.y, b.max.y)
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

/// Los cuatro rectángulos del anillo de `thickness_u` alrededor de la muestra `index_2`, con `color`.
fn there_is_ring(
    f: &TestFrame,
    bench: &Bench,
    index_2: usize,
    thickness_u: i64,
    color: Color32,
) -> bool {
    let ring_2 = ring_rects(bench.cel(index_2), thickness_u * bench.n());
    ring_2.into_iter().all(|part| {
        let expected_2 = bench.rect_of_grid(part);
        f.rects
            .iter()
            .any(|(r, fill)| *fill == color && near_rect(*r, expected_2))
    })
}

/// Hay una forma rellena de `color` en el rectángulo `expected_2`.
fn there_is_rect(f: &TestFrame, expected_2: Rect, color: Color32) -> bool {
    f.rects
        .iter()
        .any(|(r, fill)| *fill == color && near_rect(*r, expected_2))
}

/// Los botones de texto: solo `Confirmar` y `Cancel` (specs/interfaz-iconos.md, decisión 8).
const TEST_LABELS: [(PaletteButton, &str); 2] = [
    (PaletteButton::Confirm, "Confirmar"),
    (PaletteButton::Cancel, "Cancelar"),
];

/// Los botones de icono de la fila de la paleta.
const ICONS_OF_BUTTON: [(PaletteButton, Icon); 5] = [
    (PaletteButton::Add, Icon::Add),
    (PaletteButton::Remove, Icon::Remove),
    (PaletteButton::RotateLeft, Icon::RotateLeft),
    (PaletteButton::RotateRight, Icon::RotateRight),
    (PaletteButton::Transparent, Icon::Transparent),
];

/// Los botones de `visible_2` se pintan en su sitio: los de icono, con la malla de 16 u a 4 u del
/// borde del botón (o 1 u más abajo y a la derecha si está hundido) sobre una cara `SURFACE_2`
/// (`SURFACE_3` con el cursor encima, `SURFACE_0` si está hundido); `Confirmar` y `Cancel`, con
/// su rótulo en `(6, 5)` dentro del botón y la misma cara. Los demás, no.
fn checks_buttons(f: &TestFrame, bench: &Bench, visible_2: &[PaletteButton], context: &str) {
    let (ppp, n) = (bench.ppp(), bench.n());
    let top = bench.geo().icons_top_px;
    let icons_2 = bench.icons_2.set;
    for (button, icon) in ICONS_OF_BUTTON {
        // `Transparent` lo comprueba `checks_button` (su estado depende de la entrada).
        if button == PaletteButton::Transparent {
            continue;
        }
        if !visible_2.contains(&button) {
            assert!(
                !f.paints_icon(&icons_2, icon),
                "{context}: no debe pintarse el icono {icon:?}"
            );
            continue;
        }
        let rect = bench.rect_of_button(button);
        assert!(
            there_is_rect(f, rect, SURFACE_2) || there_is_rect(f, rect, SURFACE_3),
            "{context}: no hay cara del botón {button:?} en {rect:?}"
        );
        let mesh = f
            .mesh_of(&icons_2, icon)
            .unwrap_or_else(|| panic!("{context}: no se pinta el icono {icon:?}"));
        let (x, y, _, _) = rect_button_u(button);
        let expected_2 = Rect::from_min_size(
            pos2(((x + 4) * n) as f32 / ppp, (top + (y + 4) * n) as f32 / ppp),
            vec2((16 * n) as f32 / ppp, (16 * n) as f32 / ppp),
        );
        assert!(
            near_rect(mesh.rect, expected_2),
            "{context}: el icono {icon:?} está en {:?} y la maqueta dice {expected_2:?}",
            mesh.rect
        );
    }
    for (button, name_2) in TEST_LABELS {
        if !visible_2.contains(&button) {
            assert!(!f.paints(name_2), "{context}: no debe pintarse {name_2:?}");
            continue;
        }
        let (x, y, _, _) = rect_button_u(button);
        let painted = f
            .text_painted(name_2)
            .unwrap_or_else(|| panic!("{context}: no se pinta {name_2:?}"));
        let expected_2 = (
            ((x + BUTTON_TEXT_X_U) * n) as f32,
            (top + (y + BUTTON_TEXT_Y_U) * n) as f32,
        );
        assert!(
            near(painted.pos.x * ppp, expected_2.0) && near(painted.pos.y * ppp, expected_2.1),
            "{context}: {name_2:?} en ({}, {}) px y la maqueta dice {expected_2:?}",
            painted.pos.x * ppp,
            painted.pos.y * ppp
        );
        let rect = bench.rect_of_button(button);
        assert!(
            there_is_rect(f, rect, SURFACE_2) || there_is_rect(f, rect, SURFACE_3),
            "{context}: no hay cara del botón {name_2:?} en {rect:?}"
        );
    }
}

const NORMAL: [PaletteButton; 4] = [
    PaletteButton::Add,
    PaletteButton::Remove,
    PaletteButton::RotateLeft,
    PaletteButton::RotateRight,
];

/// Con una quita pendiente la fila de iconos desaparece: solo `Confirmar` y `Cancel`.
const WITH_REMOVES: [PaletteButton; 2] = [PaletteButton::Confirm, PaletteButton::Cancel];

// ---------------------------------------------------------------------------------------------
// Fila 2: los botones se pintan en su sitio y se pulsan donde la maqueta dice
// ---------------------------------------------------------------------------------------------

#[test]
fn row_2_the_buttons_are_painted_with_their_icons_in_their_place_at_the_five_ppp() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        let f = bench.rest_2_2();
        let context = format!("ppp {ppp}");
        checks_buttons(&f, &bench, &NORMAL, &context);
        // Dentro del panel, y con los cuatro botones cabiendo (sin desplazamiento).
        assert!(!bench.layout().scrolling, "{context}");
        let panel = bench.rect_panel();
        for button in NORMAL {
            assert!(
                panel
                    .expand(TOLERANCE)
                    .contains_rect(bench.rect_of_button(button)),
                "{context}: {button:?} dentro del panel"
            );
        }
        checks_inside_the_panel(&f, panel, &context);
    }
}

#[test]
fn row_2_with_a_pending_removal_confirm_and_cancel_are_painted_instead_of_the_icons() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        let context = format!("ppp {ppp}");
        checks_buttons(&bench.rest_2_2(), &bench, &NORMAL, &context);
        bench.click_button(PaletteButton::Remove);
        assert!(bench.ops.removal().is_some(), "{context}: quita pendiente");
        let f = bench.rest_2_2();
        checks_buttons(&f, &bench, &WITH_REMOVES, &context);
        checks_inside_the_panel(&f, bench.rect_panel(), &context);
        // Al cancelar vuelven los de siempre.
        bench.click_button(PaletteButton::Cancel);
        checks_buttons(&bench.rest_2_2(), &bench, &NORMAL, &context);
    }
}

/// Modo B (spec de la ventana baja): alto 200·n, el panel entero se desplaza con la rueda y los
/// botones van con él (`y − offset`).
#[test]
fn row_2_with_the_panel_scrolled_the_buttons_are_painted_and_pressed_with_y_minus_offset() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        let mut bench = Bench::with_panel_height(ppp, 200 * n, image_f(), ColorChoice::Entry(1));
        assert!(bench.layout().scrolling, "ppp {ppp}: el panel se desplaza");
        let (x, y) = bench.over_the_text();
        let max = bench.max_offset();
        assert!(max >= 77 * n, "ppp {ppp}: there is room to scroll ({max})");
        for offset in [28 * n, 70 * n] {
            bench.wheel_in(x, y, -100_000 * n);
            bench.wheel(offset);
            assert_eq!(bench.view.offset_px, offset, "ppp {ppp}");
            let context = format!("ppp {ppp}, offset {offset}");
            let f = bench.rest_2_2();
            assert_eq!(bench.geo().top_px, 91 * n - offset, "{context}");
            // With 70·n the buttons peek 5·n above the panel: they are painted (egui does not
            // discard shapes) and clipped to the panel, which the next line checks.
            checks_buttons(&f, &bench, &NORMAL, &context);
            checks_inside_the_panel(&f, bench.rect_panel(), &context);

            // Pulsar donde está ahora cada botón.
            let f = bench.click_button(PaletteButton::Add);
            assert_eq!(f.output_2.op, Some(PaletteOp::Add), "{context}");
            let f = bench.click_button(PaletteButton::RotateRight);
            assert_eq!(
                f.output_2.op,
                Some(PaletteOp::Rotate {
                    start: 0,
                    len: 4,
                    steps: 1
                }),
                "{context}"
            );
            let f = bench.click_button(PaletteButton::RotateLeft);
            assert_eq!(
                f.output_2.op,
                Some(PaletteOp::Rotate {
                    start: 0,
                    len: 4,
                    steps: -1
                }),
                "{context}"
            );
            // Donde estaría `Añadir` sin desplazar no hay nada.
            let (x_a, _) = bench.center_of_button(PaletteButton::Add);
            let without_pan = (49 + 14 + 2 + 8) * n;
            if without_pan < bench.panel_h_px() {
                let f = bench.click(x_a, without_pan);
                assert_eq!(f.output_2.op, None, "{context}");
            }
        }
        assert_eq!(bench.view.offset_px, 70 * n);
        assert!(bench.notices().is_empty(), "ppp {ppp}");
    }
}

#[test]
fn row_2_what_falls_outside_the_panel_is_not_hit_with_the_panel_scrolled() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        let mut bench = Bench::with_panel_height(ppp, 200 * n, image_f(), ColorChoice::Entry(1));
        // With 70·n of scroll, `Añadir` goes from −5·n to 19·n and the panel starts at 0.
        let (x, y) = bench.over_the_text();
        bench.wheel_in(x, y, 70 * n);
        assert_eq!(bench.view.offset_px, 70 * n, "ppp {ppp}");
        let (x, _) = bench.center_of_button(PaletteButton::Add);
        assert_eq!(bench.geo().top_px, 21 * n);
        assert_eq!(bench.geo().icons_top_px, -7 * n);

        // Above the panel (over the menu), though inside the rectangle of the button.
        let f = bench.click(x, -n);
        assert_eq!(f.output_2.op, None, "ppp {ppp}: fuera del panel");
        assert!(bench.notices().is_empty());
        // The part inside does.
        let f = bench.click(x, n);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Add),
            "ppp {ppp}: dentro del panel"
        );
    }
}

#[test]
fn row_2_in_rgba_gray_and_grayalpha_there_are_no_buttons_or_icons_or_button_text() {
    for ppp in PPPS {
        for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let image = Image::new(64, 48, mode_2).expect("imagen");
            let choice = initial_choice(&image);
            let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image, choice);
            let context = format!("ppp {ppp}, {mode_2:?}");
            let f = bench.rest_2_2();
            for (_, name_2) in TEST_LABELS {
                assert!(
                    !f.paints_something_with(name_2),
                    "{context}: pinta {name_2:?}"
                );
            }
            for (_, icon) in ICONS_OF_BUTTON {
                assert!(
                    !f.paints_icon(&bench.icons_2.set, icon),
                    "{context}: pinta el icono {icon:?}"
                );
            }
            assert!(
                !f.paints_something_with("Rotar") && !f.paints_something_with("quitar"),
                "{context}"
            );
            checks_inside_the_panel(&f, bench.rect_panel(), &context);

            // Barrido de clics por el editor, donde estarían los botones y por debajo: nada pide
            // una operación ni avisa ni deja una quita pendiente.
            let n = bench.n();
            let top = bench.geo().top_px;
            let mut y = top + 4 * n;
            while y < bench.panel_h_px() {
                for x_u in [10, 60, 120, 200] {
                    let f = bench.click(x_u * n, y);
                    assert_eq!(f.output_2.op, None, "{context}: clic en ({x_u}·n, {y})");
                    assert_eq!(f.output_2.notice, None, "{context}");
                    assert!(!f.paints_something_with("Confirmar"), "{context}");
                }
                y += 8 * n;
            }
            for button in NORMAL {
                let (x, y) = bench.center_of_button(button);
                bench.click(x, y);
            }
            assert!(bench.operations().is_empty(), "{context}");
            assert!(bench.ops.removal().is_none(), "{context}");
            assert_eq!(bench.ops.range_end(), None, "{context}");
            assert!(bench.notices().is_empty(), "{context}");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    Add,
    Remove,
    Left,
    Right,
    Nothing,
}

fn effect(bench: &Bench, f: &TestFrame) -> Effect {
    match f.output_2.op {
        Some(PaletteOp::Add) => Effect::Add,
        Some(PaletteOp::Rotate { steps, .. }) if steps < 0 => Effect::Left,
        Some(PaletteOp::Rotate { .. }) => Effect::Right,
        Some(other_2) => panic!("operación inesperada {other_2:?}"),
        None if bench.ops.removal().is_some() => Effect::Remove,
        None => Effect::Nothing,
    }
}

/// Los puntos de la fila 2 de la spec, en píxeles desde la esquina del editor (`n` = escala).
fn mockup_points(n: i64) -> [(i64, i64, Effect); 11] {
    [
        (4 * n, 2 * n, Effect::Add),
        (28 * n - 1, 2 * n, Effect::Add),
        (28 * n, 2 * n, Effect::Nothing),
        (29 * n, 2 * n, Effect::Remove),
        (53 * n - 1, 26 * n - 1, Effect::Remove),
        (60 * n, 14 * n, Effect::Nothing),
        (61 * n, 2 * n, Effect::Left),
        (86 * n, 14 * n, Effect::Right),
        (110 * n, 14 * n, Effect::Nothing),
        (4 * n, 26 * n, Effect::Nothing),
        (229 * n, 14 * n, Effect::Nothing),
    ]
}

#[test]
fn row_2_pressing_where_the_mockup_says_at_the_five_ppp() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        let n = bench.n();
        let top = bench.geo().icons_top_px;
        for (x, y, expected_2) in mockup_points(n) {
            let f = bench.click(x, top + y);
            assert_eq!(
                effect(&bench, &f),
                expected_2,
                "ppp {ppp}: pulsar ({x}, {y}) desde el editor"
            );
            bench.ops.cancel_removal();
            bench.rest_2_2();
        }
        assert!(bench.notices().is_empty(), "ppp {ppp}");
    }
}

#[test]
fn row_2_with_a_pending_removal_row_1_is_confirm_and_cancel_at_their_points() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        let n = bench.n();
        let top = bench.geo().icons_top_px;
        // El hueco de 5 u no pulsa nada.
        bench.click_button(PaletteButton::Remove);
        let f = bench.click(114 * n, top + 2 * n);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert_eq!(
            bench.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(0)
            })
        );
        // (119n, 186n) es Cancelar.
        bench.click(119 * n, top + 2 * n);
        assert_eq!(bench.ops.removal(), None, "ppp {ppp}: Cancelar");
        // (4n, 186n) es Confirmar.
        bench.click_button(PaletteButton::Remove);
        let f = bench.click(4 * n, top + 2 * n);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Remove { index: 3, into: 0 }),
            "ppp {ppp}: Confirmar"
        );
        // Los iconos no golpean con la quita pendiente (decisión 8): donde estaba el de rotar a la
        // derecha cae `Confirmar`.
        bench.click_button(PaletteButton::Remove);
        let f = bench.click(98 * n, top + 14 * n);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Remove { index: 3, into: 0 }),
            "ppp {ppp}: {:?}",
            f.output_2.op
        );
    }
}

#[test]
fn row_2_everything_painted_with_buttons_and_states_stays_inside_the_panel() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        // Normal, con quita pendiente (con y sin destino), con rango y arrastrando.
        let mut bench = bench_p_5(ppp, 3);
        let panel = bench.rect_panel();
        checks_inside_the_panel(&bench.rest_2_2(), panel, &format!("ppp {ppp}: reposo"));
        bench.click_button(PaletteButton::Remove);
        checks_inside_the_panel(&bench.rest_2_2(), panel, &format!("ppp {ppp}: quita"));
        bench.click_swatch(2);
        checks_inside_the_panel(&bench.rest_2_2(), panel, &format!("ppp {ppp}: destino"));
        bench.ops.cancel_removal();
        bench.click_swatch_with(1, SHIFT);
        checks_inside_the_panel(&bench.rest_2_2(), panel, &format!("ppp {ppp}: rango"));
        bench.ops.clear_range();
        // Hacia atrás y hacia delante, con la barra a izquierda y derecha de la muestra.
        for (from_2, until) in [(3, 0), (0, 4)] {
            let f = bench.drags(from_2, until);
            assert!(bench.ops.busy());
            checks_inside_the_panel(&f, panel, &format!("ppp {ppp}: {from_2} a {until}"));
            bench.frame(vec![key_2(Key::Escape)]);
            let (x, y) = bench.swatch_center(until);
            bench.releases(x, y);
        }
        // Con el panel desplazado y una quita pendiente puesta a mano.
        let mut bench = Bench::with_panel_height(ppp, 200 * n, image_p_5(), ColorChoice::Entry(3));
        let panel = bench.rect_panel();
        bench.ops.begin_removal(3, Some(0));
        let (x, y) = bench.over_the_text();
        bench.moves(x, y);
        let max = bench.max_offset();
        for goal in [0, n, 84 * n, max] {
            bench.wheel(-100_000 * n);
            bench.wheel(goal);
            let context = format!("ppp {ppp}, offset {}", bench.view.offset_px);
            checks_inside_the_panel(&bench.rest_2_2(), panel, &context);
        }
    }
}

#[test]
fn row_2_show_editing_view_without_ops_paints_the_buttons_but_without_state() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        bench.without_ops = true;
        let f = bench.rest_2_2();
        checks_buttons(&f, &bench, &NORMAL, &format!("ppp {ppp}"));
        // Quitar una entrada usada no deja pendiente nada: el estado se pierde cada fotograma.
        let f = bench.click_button(PaletteButton::Remove);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        let f = bench.rest_2_2();
        assert!(
            f.paints_icon(&bench.icons_2.set, Icon::Remove) && !f.paints("Confirmar"),
            "ppp {ppp}"
        );
        assert!(bench.ops.removal().is_none());
        // `Mayús` + clic no deja rango.
        bench.click_swatch_with(1, SHIFT);
        let f = bench.rest_2_2();
        assert_eq!(bench.ops.range_end(), None, "ppp {ppp}");
        assert!(!there_is_ring(&f, &bench, 2, 1, RING_RANGE), "ppp {ppp}");
        // Un arrastre no se arma.
        bench.drags(0, 2);
        assert!(!bench.ops.busy(), "ppp {ppp}");
        // Los botones que no necesitan estado siguen dando su operación.
        bench.without_ops = true;
        let f = bench.click_button(PaletteButton::Add);
        assert_eq!(f.output_2.op, Some(PaletteOp::Add), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: Añadir
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_pressing_add_gives_op_add_and_nothing_else() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        let f = bench.click_button(PaletteButton::Add);
        assert_eq!(
            f.output_2,
            PanelOutput {
                op: Some(PaletteOp::Add),
                ..PanelOutput::default()
            },
            "ppp {ppp}"
        );
        bench.rest_2_2();
        bench.rest_2_2();
        assert_eq!(
            bench.operations(),
            vec![PaletteOp::Add],
            "ppp {ppp}: una sola vez"
        );
        assert!(bench.ops.removal().is_none());
        assert!(bench.notices().is_empty());
        assert!(bench.choices().is_empty());
    }
}

#[test]
fn row_3_with_256_entries_add_gives_the_notice_and_no_op() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(5));
        let f = bench.rest_2_2();
        assert_eq!(
            bench.ink(&f, Icon::Add),
            TEXT_DISABLED,
            "ppp {ppp}: atenuado"
        );
        assert_eq!(bench.ink(&f, Icon::Remove), TEXT, "ppp {ppp}");
        assert_eq!(bench.ink(&f, Icon::RotateLeft), TEXT);
        let f = bench.click_button(PaletteButton::Add);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert_eq!(
            f.output_2.notice.as_deref(),
            Some("paleta llena: 256 entradas"),
            "ppp {ppp}"
        );
        assert!(bench.operations().is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: Quitar
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_removing_an_entry_without_pixels_gives_remove_with_the_default_target() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 4);
        let f = bench.click_button(PaletteButton::Remove);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Remove { index: 4, into: 2 }),
            "ppp {ppp}: el más cercano a Y es B"
        );
        assert!(f.output_2.op_unused, "ppp {ppp}");
        assert_eq!(f.output_2.notice, None);
        assert!(bench.ops.removal().is_none(), "ppp {ppp}: nada pendiente");
        bench.rest_2_2();
        assert_eq!(bench.operations().len(), 1);
    }
}

#[test]
fn row_4_removing_a_used_one_leaves_the_removal_pending_with_the_notice_lines() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        let f = bench.click_button(PaletteButton::Remove);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert!(!f.output_2.op_unused);
        assert_eq!(
            bench.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(0)
            }),
            "ppp {ppp}"
        );
        let f = bench.rest_2_2();
        assert!(f.paints("quitar 3; destino 0"), "ppp {ppp}");
        assert!(f.paints("elige otra muestra o confirma"), "ppp {ppp}");
        // La quitada lleva el anillo activo (2 u) y el destino el anillo de aviso (2 u).
        assert!(there_is_ring(&f, &bench, 3, 2, ACCENT), "ppp {ppp}: activo");
        assert!(
            there_is_ring(&f, &bench, 0, 2, RING_DEST),
            "ppp {ppp}: destino"
        );
        assert!(!there_is_ring(&f, &bench, 2, 2, RING_DEST), "ppp {ppp}");
        checks_inside_the_panel(&f, bench.rect_panel(), &format!("ppp {ppp}"));
    }
}

#[test]
fn row_4_pressing_another_swatch_changes_the_target_and_not_the_selection() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        bench.click_button(PaletteButton::Remove);
        bench.history.clear();

        let f = bench.click_swatch(2);
        assert_eq!(f.output_2.choice, None, "ppp {ppp}: no elige");
        assert_eq!(f.output_2.op, None);
        assert_eq!(
            bench.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(2)
            }),
            "ppp {ppp}"
        );
        assert!(f.paints("quitar 3; destino 2"), "ppp {ppp}");
        assert!(there_is_ring(&f, &bench, 2, 2, RING_DEST), "ppp {ppp}");
        assert!(!there_is_ring(&f, &bench, 0, 2, RING_DEST), "ppp {ppp}");
        assert_eq!(bench.choice, ColorChoice::Entry(3));

        // La propia entrada se ignora.
        bench.click_swatch(3);
        assert_eq!(
            bench.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(2)
            }),
            "ppp {ppp}: la propia se ignora"
        );
        // Y no se arma ningún arrastre mientras hay quita pendiente.
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}");
        assert!(
            bench.choices().is_empty(),
            "ppp {ppp}: la selección no cambia"
        );
        assert_eq!(bench.choice, ColorChoice::Entry(3));
        assert!(bench.operations().is_empty());
    }
}

#[test]
fn row_4_confirm_gives_the_remove_and_closes_the_removal() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        bench.click_button(PaletteButton::Remove);
        bench.click_swatch(2);
        let f = bench.click_button(PaletteButton::Confirm);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Remove { index: 3, into: 2 }),
            "ppp {ppp}"
        );
        assert!(!f.output_2.op_unused, "ppp {ppp}: con píxeles");
        assert_eq!(f.output_2.notice, None);
        assert!(bench.ops.removal().is_none(), "ppp {ppp}");
        // Un solo `Remove`, y los botones vuelven a ser Añadir y Quitar.
        let f = bench.rest_2_2();
        assert_eq!(bench.operations().len(), 1, "ppp {ppp}");
        checks_buttons(&f, &bench, &NORMAL, &format!("ppp {ppp}"));
    }
}

#[test]
fn row_4_confirm_with_the_default_target() {
    let mut bench = bench_p_5(1.0, 3);
    bench.click_button(PaletteButton::Remove);
    let f = bench.click_button(PaletteButton::Confirm);
    assert_eq!(f.output_2.op, Some(PaletteOp::Remove { index: 3, into: 0 }));
}

#[test]
fn row_4_cancel_and_escape_cancel_without_op_or_notice() {
    for ppp in PPPS {
        for as_word in ["Cancelar", "Escape"] {
            let mut bench = bench_p_5(ppp, 3);
            bench.click_button(PaletteButton::Remove);
            assert!(bench.ops.removal().is_some());
            bench.history.clear();
            if as_word == "Cancelar" {
                bench.click_button(PaletteButton::Cancel);
            } else {
                bench.frame(vec![key_2(Key::Escape)]);
            }
            let f = bench.rest_2_2();
            let context = format!("ppp {ppp}, {as_word}");
            assert!(bench.ops.removal().is_none(), "{context}");
            assert!(bench.history_empty(), "{context}: ni op ni aviso");
            checks_buttons(&f, &bench, &NORMAL, &context);
            assert_eq!(bench.choice, ColorChoice::Entry(3), "{context}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: Quitar con transparente, con una entrada, destino transparente
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_the_transparent_has_no_target_and_confirming_warns_until_one_is_chosen() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(
            ppp,
            SCREEN_HIGH,
            transparent_p5_image(3),
            ColorChoice::Entry(3),
        );
        let f = bench.click_button(PaletteButton::Remove);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert_eq!(
            bench.ops.removal(),
            Some(Removal {
                index: 3,
                into: None
            }),
            "ppp {ppp}"
        );
        let f = bench.rest_2_2();
        assert!(f.paints("quitar 3: pulsa un destino"), "ppp {ppp}");
        assert!(f.paints("aviso: sus píxeles quedan opacos"), "ppp {ppp}");
        assert_eq!(
            f.color_of("Confirmar"),
            TEXT_DISABLED,
            "ppp {ppp}: desactivado"
        );
        assert_eq!(f.color_of("Cancelar"), TEXT, "ppp {ppp}");
        assert!(!there_is_ring(&f, &bench, 4, 2, RING_DEST), "ppp {ppp}");

        // Confirmar sin destino: aviso, ningún op y la quita sigue.
        let f = bench.click_button(PaletteButton::Confirm);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert_eq!(
            f.output_2.notice.as_deref(),
            Some("elige un destino pulsando una muestra"),
            "ppp {ppp}"
        );
        assert_eq!(bench.ops.removal().map(|r| r.index), Some(3), "ppp {ppp}");

        // Elegir el 4 activa Confirmar.
        let f = bench.click_swatch(4);
        assert!(f.paints("quitar 3; destino 4"), "ppp {ppp}");
        assert!(f.paints("aviso: sus píxeles quedan opacos"), "ppp {ppp}");
        assert_eq!(f.color_of("Confirmar"), TEXT, "ppp {ppp}: activo");
        assert!(there_is_ring(&f, &bench, 4, 2, RING_DEST), "ppp {ppp}");
        let f = bench.click_button(PaletteButton::Confirm);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Remove { index: 3, into: 4 }),
            "ppp {ppp}"
        );
        assert!(bench.ops.removal().is_none());
        assert_eq!(bench.notices().len(), 1, "ppp {ppp}: solo el de antes");
    }
}

#[test]
fn row_5_with_the_transparent_as_target_the_transparent_target_notice_is_painted() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(
            ppp,
            SCREEN_HIGH,
            transparent_p5_image(1),
            ColorChoice::Entry(3),
        );
        bench.click_button(PaletteButton::Remove);
        let f = bench.click_swatch(1);
        assert!(f.paints("quitar 3; destino 1"), "ppp {ppp}");
        assert!(f.paints("aviso: destino transparente"), "ppp {ppp}");
        assert!(!f.paints("elige otra muestra o confirma"), "ppp {ppp}");
        // Con otro destino vuelve la ayuda.
        let f = bench.click_swatch(2);
        assert!(f.paints("elige otra muestra o confirma"), "ppp {ppp}");
        assert!(!f.paints_something_with("aviso:"), "ppp {ppp}");
    }
}

#[test]
fn row_5_with_one_entry_remove_warns_and_leaves_nothing_pending() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_a_input(), ColorChoice::Entry(0));
        let f = bench.rest_2_2();
        assert_eq!(
            bench.ink(&f, Icon::Remove),
            TEXT_DISABLED,
            "ppp {ppp}: atenuado"
        );
        assert_eq!(bench.ink(&f, Icon::Add), TEXT, "ppp {ppp}");
        assert_eq!(
            bench.ink(&f, Icon::RotateRight),
            TEXT,
            "ppp {ppp}: rotar nunca se atenúa"
        );
        let f = bench.click_button(PaletteButton::Remove);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert_eq!(
            f.output_2.notice.as_deref(),
            Some("no se puede quitar la única entrada"),
            "ppp {ppp}"
        );
        assert!(bench.ops.removal().is_none(), "ppp {ppp}");
        // Añadir sí se puede.
        let f = bench.click_button(PaletteButton::Add);
        assert_eq!(f.output_2.op, Some(PaletteOp::Add), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: Rotar y rango
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6_rotating_without_a_range_turns_the_whole_palette_and_does_not_touch_the_selection() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 2);
        let f = bench.click_button(PaletteButton::RotateRight);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Rotate {
                start: 0,
                len: 4,
                steps: 1
            }),
            "ppp {ppp}: derecha = +1"
        );
        let f = bench.click_button(PaletteButton::RotateLeft);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Rotate {
                start: 0,
                len: 4,
                steps: -1
            }),
            "ppp {ppp}: izquierda = −1"
        );
        assert_eq!(bench.choice, ColorChoice::Entry(2), "ppp {ppp}");
        assert!(bench.notices().is_empty(), "ppp {ppp}");
        assert!(bench.choices().is_empty());
    }
}

#[test]
fn row_6_with_256_entries_rotate_turns_the_256() {
    let mut bench = Bench::new_2(1.0, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(5));
    let f = bench.click_button(PaletteButton::RotateRight);
    assert_eq!(
        f.output_2.op,
        Some(PaletteOp::Rotate {
            start: 0,
            len: 256,
            steps: 1
        })
    );
}

#[test]
fn row_6_shift_click_sets_the_range_and_rotate_uses_it_and_keeps_it() {
    for ppp in PPPS {
        // Desde Entry(1) con el otro extremo en la 3, y al revés desde Entry(3) con la 1.
        for (active_2, other_2) in [(1u8, 3usize), (3, 1)] {
            let mut bench = bench_f(ppp, active_2);
            let f = bench.click_swatch_with(other_2, SHIFT);
            assert_eq!(f.output_2.choice, None, "ppp {ppp}: no cambia la selección");
            assert_eq!(bench.ops.range_end(), Some(other_2 as u8), "ppp {ppp}");
            assert_eq!(bench.choice, ColorChoice::Entry(active_2));
            assert!(bench.choices().is_empty());

            let f = bench.click_button(PaletteButton::RotateRight);
            assert_eq!(
                f.output_2.op,
                Some(PaletteOp::Rotate {
                    start: 1,
                    len: 3,
                    steps: 1
                }),
                "ppp {ppp}, activa {active_2}"
            );
            let f = bench.click_button(PaletteButton::RotateLeft);
            assert_eq!(
                f.output_2.op,
                Some(PaletteOp::Rotate {
                    start: 1,
                    len: 3,
                    steps: -1
                }),
                "ppp {ppp}, activa {active_2}"
            );
            // El panel no borra el rango al rotar (se puede repetir) ni cambia la selección.
            assert_eq!(bench.ops.range_end(), Some(other_2 as u8), "ppp {ppp}");
            assert_eq!(bench.choice, ColorChoice::Entry(active_2));
        }
    }
}

#[test]
fn row_6_the_range_rings_and_the_range_flag_are_seen() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        let before = bench.rest_2_2();
        assert!(
            !there_is_ring(&before, &bench, 2, 1, RING_RANGE),
            "ppp {ppp}"
        );
        let f = bench.click_swatch_with(3, SHIFT);
        // Anillos finos (1 u) en las muestras del rango que no son la activa; la activa lleva el
        // suyo de 2 u y la 0 (fuera del rango) ninguno.
        assert!(
            there_is_ring(&f, &bench, 2, 1, RING_RANGE),
            "ppp {ppp}: la 2"
        );
        assert!(
            there_is_ring(&f, &bench, 3, 1, RING_RANGE),
            "ppp {ppp}: la 3"
        );
        assert!(
            there_is_ring(&f, &bench, 1, 2, ACCENT),
            "ppp {ppp}: la activa"
        );
        assert!(
            !there_is_ring(&f, &bench, 0, 1, RING_RANGE),
            "ppp {ppp}: la 0"
        );
        // El cursor está sobre la 3, que está en el rango y no es la activa.
        assert!(f.says("cursor · rango"), "ppp {ppp}");
        checks_inside_the_panel(&f, bench.rect_panel(), &format!("ppp {ppp}"));
        // Con el cursor en la 2 y luego en la activa.
        let (x, y) = bench.swatch_center(2);
        assert!(bench.moves(x, y).says("cursor · rango"), "ppp {ppp}");
        let (x, y) = bench.swatch_center(1);
        let f = bench.moves(x, y);
        assert!(
            f.says("cursor · activa"),
            "ppp {ppp}: la activa no lleva `rango`"
        );
        assert!(!f.paints_something_with("rango"), "ppp {ppp}");
        // Sin el cursor en la rejilla no sale la bandera en la información de la elegida.
        let f = bench.moves(300 * bench.n(), 100 * bench.n());
        assert!(f.says("activa"), "ppp {ppp}");
    }
}

#[test]
fn row_6_shift_click_does_not_arm_a_drag_even_if_the_pointer_moves_20_px() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        let (x, y) = bench.swatch_center(3);
        bench.moves(x, y);
        bench.presses_with(x, y, SHIFT);
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}: no se arma");
        assert_eq!(bench.ops.range_end(), Some(3));
        // 20 px a la izquierda (a la muestra 2 o su separación) y luego a la muestra 0.
        let f = bench.moves(x - 20, y);
        assert!(!bench.ops.busy(), "ppp {ppp}");
        assert!(!f.paints_something_with("mover"), "ppp {ppp}");
        let (x0, y0) = bench.swatch_center(0);
        bench.moves(x0, y0);
        let f = bench.releases_with(x0, y0, SHIFT);
        assert_eq!(f.output_2.op, None, "ppp {ppp}: ningún Move");
        assert!(bench.operations().is_empty());
        assert_eq!(bench.ops.range_end(), Some(3), "ppp {ppp}");
        assert_eq!(bench.choice, ColorChoice::Entry(1));
        assert!(bench.choices().is_empty());
    }
}

#[test]
fn row_6_a_normal_click_clears_the_range_and_chooses() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        bench.click_swatch_with(3, SHIFT);
        assert_eq!(bench.ops.range_end(), Some(3));
        let f = bench.click_swatch(0);
        assert_eq!(f.output_2.choice, Some(ColorChoice::Entry(0)), "ppp {ppp}");
        assert_eq!(bench.ops.range_end(), None, "ppp {ppp}");
        let f = bench.click_button(PaletteButton::RotateRight);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Rotate {
                start: 0,
                len: 4,
                steps: 1
            }),
            "ppp {ppp}: sin rango otra vez"
        );
        let f = bench.rest_2_2();
        assert!(!there_is_ring(&f, &bench, 2, 1, RING_RANGE), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: arrastrar muestras
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_pressing_and_moving_less_than_4n_and_releasing_only_selects() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 3);
        let n = bench.n();
        let (x, y) = bench.swatch_center(0);
        bench.moves(x, y);
        let f = bench.presses_2(x, y);
        assert_eq!(
            f.output_2.choice,
            Some(ColorChoice::Entry(0)),
            "ppp {ppp}: se elige al pulsar"
        );
        assert!(bench.ops.drag_state().is_some(), "ppp {ppp}: armado");
        assert!(!bench.ops.busy(), "ppp {ppp}: armado no es ocupado");
        // 4n − 1 px: aún un clic.
        let f = bench.moves(x + 4 * n - 1, y);
        assert!(!bench.ops.busy(), "ppp {ppp}");
        assert!(!f.paints_something_with("mover"), "ppp {ppp}");
        let f = bench.releases(x + 4 * n - 1, y);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}");
        assert_eq!(bench.choice, ColorChoice::Entry(0));
        assert!(bench.operations().is_empty());
        assert!(bench.notices().is_empty());
    }
}

#[test]
fn row_7_from_4n_the_drag_starts_with_the_bar_and_the_lines() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 3);
        let n = bench.n();
        let (x, y) = bench.swatch_center(0);
        bench.moves(x, y);
        bench.presses_2(x, y);
        // Justo 4n px (distancia euclídea 4n): empieza.
        bench.moves(x + 4 * n, y);
        assert!(bench.ops.busy(), "ppp {ppp}: con 4n empieza");

        // Hacia la muestra 2: barra a la derecha de la muestra destino y las dos líneas.
        let (x2, y2) = bench.swatch_center(2);
        let f = bench.moves(x2, y2);
        assert!(bench.ops.busy(), "ppp {ppp}");
        assert!(f.says("mover 0 a 2"), "ppp {ppp}");
        assert!(f.says("Escape cancela"), "ppp {ppp}");
        let bar = drop_mark_rect(bench.cel(2), true, n as u32);
        assert!(
            there_is_rect(&f, bench.rect_of_grid(bar), DROP_MARK),
            "ppp {ppp}: barra a la derecha de la 2"
        );
        checks_inside_the_panel(&f, bench.rect_panel(), &format!("ppp {ppp}"));
        // El documento no cambia ni hay ninguna petición mientras dura.
        assert!(bench.operations().is_empty(), "ppp {ppp}");
        assert_eq!(bench.doc_state, 0);

        // Hacia la 0 (la misma): sin barra y con `move_pointer 0 a 0`... la línea de destino no es un
        // movimiento: no se pinta barra.
        let (x0, y0) = bench.swatch_center(0);
        let f = bench.moves(x0, y0);
        let bar_0 = drop_mark_rect(bench.cel(0), false, n as u32);
        assert!(
            !there_is_rect(&f, bench.rect_of_grid(bar_0), DROP_MARK),
            "ppp {ppp}"
        );
        bench.releases(x0, y0);
    }
}

#[test]
fn row_7_going_backwards_the_bar_goes_to_the_left_of_the_target_swatch() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 0);
        let n = bench.n();
        let f = bench.drags(3, 1);
        assert!(bench.ops.busy(), "ppp {ppp}");
        assert!(f.says("mover 3 a 1"), "ppp {ppp}");
        let left = drop_mark_rect(bench.cel(1), false, n as u32);
        let right = drop_mark_rect(bench.cel(1), true, n as u32);
        assert!(
            there_is_rect(&f, bench.rect_of_grid(left), DROP_MARK),
            "ppp {ppp}"
        );
        assert!(
            !there_is_rect(&f, bench.rect_of_grid(right), DROP_MARK),
            "ppp {ppp}"
        );
        let (x, y) = bench.swatch_center(1);
        bench.releases(x, y);
    }
}

#[test]
fn row_7_releasing_on_another_swatch_gives_a_single_move() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 3);
        bench.drags(0, 2);
        let (x, y) = bench.swatch_center(2);
        let f = bench.releases(x, y);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Move { from: 0, to: 2 }),
            "ppp {ppp}"
        );
        assert!(!bench.ops.busy(), "ppp {ppp}: el arrastre termina");
        assert!(bench.ops.drag_state().is_none());
        // La elección del pulsar (la 0) es lo único que pidió el panel además del `Move`; la
        // selección final (`to`) la fija `Session` al aplicar.
        assert_eq!(bench.choices(), vec![ColorChoice::Entry(0)], "ppp {ppp}");
        for _ in 0..3 {
            bench.rest_2_2();
        }
        let (x1, y1) = bench.swatch_center(1);
        bench.moves(x1, y1);
        bench.rest_2_2();
        assert_eq!(
            bench.operations(),
            vec![PaletteOp::Move { from: 0, to: 2 }],
            "ppp {ppp}: un solo op"
        );
        assert!(bench.notices().is_empty());
    }
}

#[test]
fn row_7_with_16_or_more_entries_the_target_is_the_final_index() {
    // 256 entradas, 0 a 17 (segunda fila, segunda columna) y de 17 a 255 (la última).
    let mut bench = Bench::new_2(1.0, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(9));
    bench.drags(0, 17);
    let (x, y) = bench.swatch_center(17);
    let f = bench.releases(x, y);
    assert_eq!(f.output_2.op, Some(PaletteOp::Move { from: 0, to: 17 }));
    bench.drags(17, 255);
    let (x, y) = bench.swatch_center(255);
    let f = bench.releases(x, y);
    assert_eq!(f.output_2.op, Some(PaletteOp::Move { from: 17, to: 255 }));
}

#[test]
fn row_7_releasing_on_the_same_one_outside_the_grid_or_outside_the_panel_does_nothing() {
    for ppp in PPPS {
        let n_ppp = (ppp + 0.5).floor().max(1.0) as i64;
        // Puntos de suelta, en píxeles: (descripción, x, y) con el bloque sin desplazar.
        let right_of_the_grid = (228 * n_ppp, (GRID_Y_U + 5) * n_ppp);
        let below_the_last_row = (10 * n_ppp, (GRID_Y_U + 30) * n_ppp);
        let over_the_text = (10 * n_ppp, 10 * n_ppp);
        let over_the_canvas = (400 * n_ppp, (GRID_Y_U + 5) * n_ppp);
        let x_negative = (-5, (GRID_Y_U + 5) * n_ppp);
        let cases = [
            ("la misma", None),
            ("a la derecha de la rejilla", Some(right_of_the_grid)),
            ("debajo de la última fila", Some(below_the_last_row)),
            ("sobre el texto", Some(over_the_text)),
            ("sobre el lienzo", Some(over_the_canvas)),
            ("fuera de la ventana por la izquierda", Some(x_negative)),
        ];
        for (name_2, point) in cases {
            let mut bench = bench_f(ppp, 3);
            let (x0, y0) = bench.swatch_center(0);
            bench.moves(x0, y0);
            bench.presses_2(x0, y0);
            // Pasa el umbral con un destino válido (la muestra 2) y luego va al punto de suelta.
            let (x2, y2) = bench.swatch_center(2);
            bench.moves(x2, y2);
            assert!(bench.ops.busy(), "ppp {ppp}, {name_2}");
            let (xs, ys) = point.unwrap_or((x0, y0));
            let f = bench.moves(xs, ys);
            if point.is_some() {
                assert!(
                    f.says_something_with("suelta en la rejilla"),
                    "ppp {ppp}, {name_2}"
                );
            }
            let f = bench.releases(xs, ys);
            assert_eq!(f.output_2.op, None, "ppp {ppp}, {name_2}: no mueve");
            assert!(bench.operations().is_empty(), "ppp {ppp}, {name_2}");
            assert!(!bench.ops.busy(), "ppp {ppp}, {name_2}");
            assert!(bench.notices().is_empty(), "ppp {ppp}, {name_2}");
            bench.rest_2_2();
            assert!(bench.operations().is_empty(), "ppp {ppp}, {name_2}");
        }
    }
}

#[test]
fn row_7_escape_cancels_the_drag_and_the_pressed_one_stays_chosen() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        bench.drags(3, 0);
        assert!(bench.ops.busy(), "ppp {ppp}");
        let f = bench.frame(vec![key_2(Key::Escape)]);
        assert!(!bench.ops.busy(), "ppp {ppp}");
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}");
        assert!(
            !f.paints_something_with("mover"),
            "ppp {ppp}: ya no se pintan las líneas"
        );
        let (x, y) = bench.swatch_center(0);
        let f = bench.releases(x, y);
        assert_eq!(
            f.output_2.op, None,
            "ppp {ppp}: soltar tras cancelar no mueve"
        );
        assert!(bench.operations().is_empty(), "ppp {ppp}");
        assert_eq!(
            bench.choice,
            ColorChoice::Entry(3),
            "ppp {ppp}: la pulsada sigue elegida"
        );
        assert!(bench.notices().is_empty(), "ppp {ppp}: cancelar no avisa");
    }
}

#[test]
fn row_7_losing_the_window_focus_cancels_the_drag() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        bench.drags(3, 0);
        assert!(bench.ops.busy(), "ppp {ppp}");
        bench.frame(vec![Event::WindowFocused(false)]);
        assert!(!bench.ops.busy(), "ppp {ppp}");
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}");
        let (x, y) = bench.swatch_center(0);
        let f = bench.releases(x, y);
        assert_eq!(f.output_2.op, None, "ppp {ppp}");
        assert!(bench.operations().is_empty(), "ppp {ppp}");
        assert_eq!(bench.choice, ColorChoice::Entry(3), "ppp {ppp}");
        assert!(bench.notices().is_empty(), "ppp {ppp}");
    }
}

/// Con 256 entradas y la rejilla con filas ocultas, la rueda durante el arrastre desplaza las
/// filas y el destino sigue a la muestra que queda bajo el puntero.
#[test]
fn row_7_the_wheel_during_the_drag_scrolls_the_rows() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        let mut bench =
            Bench::with_panel_height(ppp, 400 * n, image_indexed_256(), ColorChoice::Entry(0));
        assert!(!bench.layout().scrolling, "ppp {ppp}");
        let visible_2 = bench.layout().visible_rows;
        assert!(
            visible_2 < 16,
            "ppp {ppp}: faltan filas por ver ({visible_2})"
        );
        assert_eq!(bench.scroll.first_row, 0);

        bench.drags(0, 17);
        assert!(bench.ops.busy(), "ppp {ppp}");
        let f = bench.wheel(2 * PITCH_U * n);
        assert_eq!(
            bench.scroll.first_row, 2,
            "ppp {ppp}: la rueda desplaza las filas"
        );
        assert_eq!(bench.view.offset_px, 0, "ppp {ppp}");
        assert!(bench.ops.busy(), "ppp {ppp}: el arrastre sigue");
        // El puntero sigue en la fila 1 y columna 1 de lo que se ve: la muestra (2 + 1)·16 + 1.
        assert!(f.says("mover 0 a 49"), "ppp {ppp}");
        let (x, y) = center_of_swatch(n, 17, 0);
        let f = bench.releases(x, y);
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Move { from: 0, to: 49 }),
            "ppp {ppp}"
        );
    }
}

/// **[DEPENDE DE EGUI]** Si egui ya no ve el botón abajo (se perdió el soltar), es un soltar.
#[test]
fn row_7_if_egui_no_longer_sees_the_button_down_it_is_a_release() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 3);
        bench.drags(0, 2);
        assert!(bench.ops.busy(), "ppp {ppp}");
        // El soltar llega a egui pero el panel no lo ve.
        let (x, y) = bench.swatch_center(2);
        let pos = in_points(ppp, x, y);
        bench.frame_without_panel(vec![button_2(pos, false, Modifiers::NONE)]);
        assert!(bench.ops.busy(), "ppp {ppp}: el panel aún no lo sabe");
        let f = bench.rest_2_2();
        assert_eq!(
            f.output_2.op,
            Some(PaletteOp::Move { from: 0, to: 2 }),
            "ppp {ppp}: el fotograma siguiente lo trata como un soltar"
        );
        assert!(!bench.ops.busy(), "ppp {ppp}");
        bench.rest_2_2();
        assert_eq!(bench.operations().len(), 1, "ppp {ppp}: un solo op");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: gestos en curso, `enabled` falso y teclado
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_with_enabled_false_neither_buttons_nor_swatches_nor_shift_click_do_anything() {
    for ppp in PPPS {
        let images: [(&str, Image, u8, PaletteButton); 3] = [
            ("F", image_f(), 1, PaletteButton::Remove),
            // Con 256 entradas, `Añadir` avisaría si estuviera activo.
            ("256", image_indexed_256(), 5, PaletteButton::Add),
            // Con una sola entrada, `Remove` avisaría.
            ("1", image_a_input(), 0, PaletteButton::Remove),
        ];
        for (name_2, image, input_2, notice) in images {
            let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image, ColorChoice::Entry(input_2));
            bench.gesture_drawing = true;
            let context = format!("ppp {ppp}, {name_2}");
            for button in NORMAL {
                bench.click_button(button);
            }
            bench.click_button(notice);
            bench.click_swatch(0);
            bench.click_swatch_with(0, SHIFT);
            bench.drags(0, 0);
            assert!(!bench.ops.busy(), "{context}");
            let (x, y) = bench.swatch_center(0);
            bench.releases(x, y);
            let f = bench.rest_2_2();
            assert!(bench.history_empty(), "{context}: {:?}", bench.history);
            assert!(bench.ops.removal().is_none(), "{context}");
            assert_eq!(bench.ops.range_end(), None, "{context}");
            assert!(bench.ops.drag_state().is_none(), "{context}");
            assert_eq!(bench.choice, ColorChoice::Entry(input_2), "{context}");
            // Se ven igual.
            checks_buttons(&f, &bench, &NORMAL, &context);
        }
    }
}

#[test]
fn row_9_with_enabled_false_and_a_pending_removal_nothing_changes_it() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        bench.click_button(PaletteButton::Remove);
        let pending = bench.ops.removal();
        assert!(pending.is_some());
        bench.history.clear();
        bench.gesture_drawing = true;
        for button in [PaletteButton::Confirm, PaletteButton::Cancel] {
            bench.click_button(button);
        }
        bench.click_swatch(2);
        bench.click_swatch_with(1, SHIFT);
        assert_eq!(bench.ops.removal(), pending, "ppp {ppp}");
        assert!(bench.history_empty(), "ppp {ppp}");
        let f = bench.rest_2_2();
        checks_buttons(&f, &bench, &WITH_REMOVES, &format!("ppp {ppp}"));
    }
}

#[test]
fn row_9_with_a_slider_in_progress_the_buttons_and_the_swatches_do_nothing() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        let geo = bench.geo();
        let (x, y) = (
            geo.x_of_value(128, Control::Red.max()),
            geo.y_row(Control::Red),
        );
        bench.moves(x, y);
        bench.presses_2(x, y);
        assert!(
            bench.editor.dragging(),
            "ppp {ppp}: el deslizador está en curso"
        );
        bench.history.clear();
        // Nuevas pulsaciones (sin soltar el deslizador) sobre los controles nuevos.
        for button in NORMAL {
            let (bx, by) = bench.center_of_button(button);
            bench.moves(bx, by);
            bench.presses_2(bx, by);
        }
        for (index_2, mods) in [(1, Modifiers::NONE), (2, SHIFT)] {
            let (cx, cy) = bench.swatch_center(index_2);
            bench.moves(cx, cy);
            bench.presses_with(cx, cy, mods);
        }
        assert!(bench.editor.dragging(), "ppp {ppp}");
        assert!(bench.operations().is_empty(), "ppp {ppp}");
        assert!(bench.notices().is_empty(), "ppp {ppp}");
        assert!(bench.choices().is_empty(), "ppp {ppp}");
        assert!(bench.ops.removal().is_none(), "ppp {ppp}");
        assert_eq!(bench.ops.range_end(), None, "ppp {ppp}");
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}");
    }
}

#[test]
fn row_9_with_a_swatch_drag_in_progress_the_buttons_do_nothing() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 1);
        bench.drags(0, 2);
        assert!(bench.ops.busy(), "ppp {ppp}");
        bench.history.clear();
        // Una pulsación más (sin soltar) sobre los botones y sobre una muestra con `Mayús`.
        for button in NORMAL {
            let (x, y) = bench.center_of_button(button);
            bench.moves(x, y);
            let f = bench.presses_2(x, y);
            assert_eq!(f.output_2.op, None, "ppp {ppp}: {button:?}");
            assert!(bench.ops.busy(), "ppp {ppp}: {button:?}");
        }
        let (x, y) = bench.swatch_center(3);
        bench.moves(x, y);
        bench.presses_with(x, y, SHIFT);
        assert_eq!(bench.ops.range_end(), None, "ppp {ppp}");
        assert!(bench.ops.removal().is_none(), "ppp {ppp}");
        assert!(bench.notices().is_empty(), "ppp {ppp}");
        assert!(bench.operations().is_empty(), "ppp {ppp}");
    }
}

#[test]
fn row_9_with_a_focused_field_escape_does_not_cancel_the_pending_removal() {
    for written in [None, Some("#00ff00")] {
        let mut bench = bench_p_5(1.0, 3);
        bench.click_button(PaletteButton::Remove);
        let pending = bench.ops.removal();
        assert!(pending.is_some());
        bench.focuses_field();
        if let Some(text_2) = written {
            bench.writes(text_2);
        }
        bench.history.clear();
        bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(
            bench.ops.removal(),
            pending,
            "{written:?}: Escape es del campo"
        );
        assert!(
            !bench.wants_keyboard(),
            "{written:?}: egui saca el foco del campo"
        );
        assert!(bench.notices().is_empty(), "{written:?}");
        assert!(
            bench.commits().is_empty(),
            "{written:?}: Escape no confirma"
        );
        assert_eq!(bench.editor.hex_display(), None, "{written:?}");
        let f = bench.rest_2_2();
        assert!(
            f.paints("Confirmar"),
            "{written:?}: la quita sigue a la vista"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: el texto pendiente se resuelve antes de operar
// ---------------------------------------------------------------------------------------------

/// **[DEPENDE DE EGUI]** Con texto válido en el campo y el clic entero (pulsar y soltar) en un
/// solo fotograma, el panel devuelve el `Commit` y la operación juntos: `main` aplica primero el
/// commit y luego la operación.
#[test]
fn row_10_with_the_click_on_a_frame_the_commit_and_the_operation_come_out_together() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(1), "#ff00ff");
    let (x, y) = bench.center_of_button(PaletteButton::Add);
    bench.moves(x, y);
    let pos = in_points(bench.ppp(), x, y);
    let f = bench.frame(vec![
        button_2(pos, true, Modifiers::NONE),
        button_2(pos, false, Modifiers::NONE),
    ]);
    assert_eq!(
        f.output_2.commit,
        Some(Commit {
            target: EditTarget::Entry(1),
            color: Rgba::opaque(255, 0, 255)
        })
    );
    assert_eq!(f.output_2.op, Some(PaletteOp::Add));
    assert!(!bench.editor.has_pending(), "no queda texto");
    assert!(bench.notices().is_empty());
}

/// **[DEPENDE DE EGUI]** Con texto válido en el campo, el clic en un botón (pulsar y soltar en
/// fotogramas distintos, como en una ventana real) da el `Commit` antes de la operación (spec,
/// fila 10: «el `Commit` llega antes de la operación»). egui pierde el foco al **soltar**, y el
/// botón actúa al **pulsar**, así que la operación sale un fotograma antes que el commit.
#[test]
fn row_10_the_commit_of_the_field_arrives_before_the_operation_of_the_button() {
    let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(1), "#ff00ff");
    bench.click_button(PaletteButton::Add);
    bench.rest_2_2();
    bench.rest_2_2();
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Entry(1),
            color: Rgba::opaque(255, 0, 255)
        }]
    );
    assert_eq!(bench.operations(), vec![PaletteOp::Add]);
    let of_the_commit = bench.history.iter().position(|o| o.commit.is_some());
    let op_index = bench.history.iter().position(|o| o.op.is_some());
    assert!(
        of_the_commit <= op_index,
        "el commit ({of_the_commit:?}) llega antes que la op ({op_index:?})"
    );
    assert!(!bench.editor.has_pending(), "no queda texto");
    assert!(bench.notices().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Fila 13: rotar sin nada que rotar
// ---------------------------------------------------------------------------------------------

#[test]
fn row_13_with_one_entry_the_rotate_button_emits_the_rotate_and_the_panel_does_not_warn() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_a_input(), ColorChoice::Entry(0));
        for steps in [1, -1] {
            let button_2 = if steps > 0 {
                PaletteButton::RotateRight
            } else {
                PaletteButton::RotateLeft
            };
            let f = bench.click_button(button_2);
            assert_eq!(
                f.output_2,
                PanelOutput {
                    op: Some(PaletteOp::Rotate {
                        start: 0,
                        len: 1,
                        steps
                    }),
                    ..PanelOutput::default()
                },
                "ppp {ppp}: el aviso lo da la sesión"
            );
        }
        assert!(bench.notices().is_empty(), "ppp {ppp}");
        assert_eq!(bench.choice, ColorChoice::Entry(0));
    }
}

#[test]
fn row_13_a_one_entry_range_emits_the_rotate_of_len_1() {
    for ppp in PPPS {
        let mut bench = bench_f(ppp, 2);
        // `Mayús` + clic en la propia activa: rango de una sola entrada.
        let f = bench.click_swatch_with(2, SHIFT);
        assert_eq!(f.output_2.choice, None, "ppp {ppp}");
        assert_eq!(bench.ops.range_end(), Some(2), "ppp {ppp}");
        for steps in [1, -1] {
            let button_2 = if steps > 0 {
                PaletteButton::RotateRight
            } else {
                PaletteButton::RotateLeft
            };
            let f = bench.click_button(button_2);
            assert_eq!(
                f.output_2.op,
                Some(PaletteOp::Rotate {
                    start: 2,
                    len: 1,
                    steps
                }),
                "ppp {ppp}"
            );
            assert_eq!(f.output_2.notice, None, "ppp {ppp}");
        }
        assert_eq!(
            bench.ops.range_end(),
            Some(2),
            "ppp {ppp}: el rango se queda"
        );
        assert_eq!(bench.choice, ColorChoice::Entry(2));
        // El botón no está atenuado.
        let f = bench.rest_2_2();
        assert_eq!(bench.ink(&f, Icon::RotateRight), TEXT, "ppp {ppp}");
        assert_eq!(bench.ink(&f, Icon::RotateLeft), TEXT, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 14: prioridad de Escape
// ---------------------------------------------------------------------------------------------

/// (a) Quita pendiente con el campo hexadecimal enfocado y texto escrito: el primer `Escape` solo
/// revierte el texto y la quita sigue; el segundo la cancela.
#[test]
fn row_14_a_escape_with_a_focused_field_and_a_pending_removal_consumes_one_level_each_time() {
    for ppp in [1.0, 2.0] {
        let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_p_5(), ColorChoice::Entry(3));
        bench.click_button(PaletteButton::Remove);
        let pending = bench.ops.removal();
        assert!(pending.is_some());
        bench.focuses_field();
        bench.writes("#00ff00");
        assert!(bench.editor.has_pending(), "ppp {ppp}");
        bench.history.clear();

        bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(
            bench.editor.hex_display(),
            None,
            "ppp {ppp}: el texto se revierte"
        );
        assert!(!bench.editor.has_pending(), "ppp {ppp}");
        assert_eq!(bench.ops.removal(), pending, "ppp {ppp}: la quita sigue");
        assert!(!bench.wants_keyboard(), "ppp {ppp}");

        bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(
            bench.ops.removal(),
            None,
            "ppp {ppp}: el segundo la cancela"
        );
        assert!(bench.history_empty(), "ppp {ppp}: sin commit, op ni aviso");
        let f = bench.rest_2_2();
        checks_buttons(&f, &bench, &NORMAL, &format!("ppp {ppp}"));
    }
}

/// (b) Quita pendiente con un gesto de dibujo en curso (`enabled` falso): `Escape` no cancela la
/// quita (el gesto lo cancela quien lo lleva); sin gesto, sí.
#[test]
fn row_14_b_with_a_drawing_gesture_in_progress_escape_does_not_cancel_the_removal() {
    for ppp in [1.0, 2.0] {
        let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_p_5(), ColorChoice::Entry(3));
        bench.click_button(PaletteButton::Remove);
        let pending = bench.ops.removal();
        assert!(pending.is_some());
        bench.history.clear();
        bench.gesture_drawing = true;
        bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(bench.ops.removal(), pending, "ppp {ppp}: la quita sigue");
        // El gesto se cancela (lo hace el lienzo) y entonces el siguiente `Escape` la cancela.
        bench.gesture_drawing = false;
        bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(bench.ops.removal(), None, "ppp {ppp}");
        assert!(bench.history_empty(), "ppp {ppp}");
    }
}

/// (c) Un arrastre de muestra en curso: `Escape` cancela solo el arrastre; la muestra pulsada
/// sigue elegida y no hay paso.
#[test]
fn row_14_c_escape_with_a_swatch_drag_cancels_only_the_drag() {
    for ppp in [1.0, 2.0] {
        let mut bench = bench_f(ppp, 1);
        bench.drags(3, 0);
        assert!(bench.ops.busy(), "ppp {ppp}");
        bench.history.clear();
        bench.frame(vec![key_2(Key::Escape)]);
        assert!(bench.ops.drag_state().is_none(), "ppp {ppp}");
        assert_eq!(
            bench.choice,
            ColorChoice::Entry(3),
            "ppp {ppp}: la pulsada sigue elegida"
        );
        let (x, y) = bench.swatch_center(0);
        bench.releases(x, y);
        assert!(bench.history_empty(), "ppp {ppp}: ni paso ni aviso");

        // También con el botón solo armado (antes del umbral).
        let (x, y) = bench.swatch_center(2);
        bench.moves(x, y);
        bench.presses_2(x, y);
        assert!(
            bench.ops.drag_state().is_some() && !bench.ops.busy(),
            "ppp {ppp}: armado"
        );
        bench.frame(vec![key_2(Key::Escape)]);
        assert!(
            bench.ops.drag_state().is_none(),
            "ppp {ppp}: armado también se cancela"
        );
        bench.releases(x, y);
        assert!(bench.operations().is_empty(), "ppp {ppp}");
    }
}

/// (d) Un deslizador en curso con una quita pendiente: el primer `Escape` cancela el deslizador
/// y restaura el color; la quita sigue; el segundo la cancela.
#[test]
fn row_14_d_escape_with_a_slider_in_progress_cancels_only_the_slider() {
    for ppp in [1.0, 2.0] {
        let mut bench = bench_p_5(ppp, 3);
        bench.click_button(PaletteButton::Remove);
        let pending = bench.ops.removal();
        let geo = bench.geo();
        let (x, y) = (
            geo.x_of_value(10, Control::Red.max()),
            geo.y_row(Control::Red),
        );
        bench.moves(x, y);
        bench.presses_2(x, y);
        assert!(bench.editor.dragging(), "ppp {ppp}");
        assert!(
            bench.editor.preview().is_some(),
            "ppp {ppp}: hay color provisional"
        );
        bench.history.clear();

        bench.frame(vec![key_2(Key::Escape)]);
        assert!(
            !bench.editor.dragging(),
            "ppp {ppp}: se cancela el deslizador"
        );
        assert_eq!(
            bench.editor.preview(),
            None,
            "ppp {ppp}: el color se restaura"
        );
        assert_eq!(bench.ops.removal(), pending, "ppp {ppp}: la quita sigue");
        bench.releases(x, y);
        assert!(bench.commits().is_empty(), "ppp {ppp}: no hay paso");

        bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(
            bench.ops.removal(),
            None,
            "ppp {ppp}: el segundo cancela la quita"
        );
        assert!(bench.history_empty(), "ppp {ppp}");
    }
}

/// (e) Nothing in progress: `Escape` does not touch the range, the history or the choice and does
/// not warn; the only thing it does (C1) is to ask to close the color editor, last in the chain.
#[test]
fn row_14_e_escape_with_nothing_in_progress_only_asks_to_close_the_editor() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        bench.click_swatch_with(1, SHIFT);
        let range = bench.ops.range_end();
        bench.history.clear();
        let f = bench.frame(vec![key_2(Key::Escape)]);
        assert_eq!(
            f.output_2,
            PanelOutput {
                close_editor: true,
                ..PanelOutput::default()
            },
            "ppp {ppp}"
        );
        assert!(bench.history_empty_but_close(), "ppp {ppp}");
        assert_eq!(bench.choice, ColorChoice::Entry(3), "ppp {ppp}");
        // `Escape` no consume el rango: queda como estaba.
        assert_eq!(bench.ops.range_end(), range, "ppp {ppp}");
        assert!(bench.ops.removal().is_none());
        assert!(!bench.ops.busy());
    }
}

// ---------------------------------------------------------------------------------------------
// El estado y el documento
// ---------------------------------------------------------------------------------------------

#[test]
fn a_document_change_cancels_the_pending_removal_and_the_buttons_come_back() {
    for ppp in PPPS {
        let mut bench = bench_p_5(ppp, 3);
        bench.click_button(PaletteButton::Remove);
        let f = bench.rest_2_2();
        checks_buttons(&f, &bench, &WITH_REMOVES, &format!("ppp {ppp}"));
        assert_eq!(bench.cancellations, 0);
        // Editar un color de otra entrada cambia el estado del documento.
        bench.set_color(0, Rgba::opaque(1, 2, 3));
        let f = bench.rest_2_2();
        assert_eq!(bench.cancellations, 1, "ppp {ppp}: `sync` avisa una vez");
        assert!(bench.ops.removal().is_none(), "ppp {ppp}");
        checks_buttons(&f, &bench, &NORMAL, &format!("ppp {ppp}"));
        assert!(!f.paints_something_with("quitar 3"), "ppp {ppp}");
        bench.rest_2_2();
        assert_eq!(bench.cancellations, 1, "ppp {ppp}: no se repite");

        // Y deshacer también cancela una quita nueva.
        bench.click_button(PaletteButton::Remove);
        assert!(bench.ops.removal().is_some());
        bench.undoes();
        bench.rest_2_2();
        assert_eq!(bench.cancellations, 2, "ppp {ppp}");
        assert!(bench.ops.removal().is_none(), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 21: botón «Fijar/Quitar transparente» (specs/hito-2-fijar-transparente.md, filas 2, 3, 5 y
// 9 en su parte de ventana). Lo puro (rótulo, `transparent_toggle`, resúmenes) está en
// `palette_ops.rs`; `button_hit` y `edges_u`, en `edit_layout.rs`; `info_flags_alpha0` e
// `is_alpha0_cell_with`, en `color.rs`; `Session`, en `px-app`. El banco no aplica las `op`: las
// pruebas que necesitan el documento nuevo lo aplican con `Bench::applies`.
// ---------------------------------------------------------------------------------------------

mod set_transparent {
    use super::*;
    use px_core::ImageId;
    use px_ui::edit_layout::min_panel_height_px;
    use px_ui::text::{INTERFACE_CHARSET, fit_text};

    const SET: &str = "Fijar transparente";
    const REMOVE: &str = "Quitar transparente";

    const ALPHA_0: Rgba = Rgba::new(0, 255, 0, 0);

    /// `[R, G with alpha saved 0, B, K]`, píxeles `[0, 1, 2, 3]`, con `mark` como transparente.
    fn image_alpha_0(mark: Option<u8>) -> Image {
        image_with(&[RED, ALPHA_0, BLUE, TEST_BLACK], mark, &[0, 1, 2, 3])
    }

    fn op_of(index: Option<u8>) -> Option<PaletteOp> {
        Some(PaletteOp::SetTransparent { index })
    }

    /// El botón `Transparent` se pinta en su sitio como icono y con el estado del rótulo: hundido
    /// (cara `SURFACE_0`, icono `ACCENT`, 1 u abajo y a la derecha) si el rótulo es `REMOVE`; si no,
    /// normal (cara `SURFACE_2` o `SURFACE_3`, icono `TEXT`). El rótulo mismo es del tooltip.
    fn checks_button(f: &TestFrame, bench: &Bench, label: &str, context: &str) {
        let (ppp, n) = (bench.ppp(), bench.n());
        let top = bench.geo().icons_top_px;
        let (x, y, _, _) = rect_button_u(PaletteButton::Transparent);
        let pressed = label == REMOVE;
        let mesh = f
            .mesh_of(&bench.icons_2.set, Icon::Transparent)
            .unwrap_or_else(|| panic!("{context}: no se pinta el icono de {label:?}"));
        let pan = if pressed { 1 } else { 0 };
        let expected_2 = Rect::from_min_size(
            pos2(
                ((x + 4 + pan) * n) as f32 / ppp,
                (top + (y + 4 + pan) * n) as f32 / ppp,
            ),
            vec2((16 * n) as f32 / ppp, (16 * n) as f32 / ppp),
        );
        assert!(
            near_rect(mesh.rect, expected_2),
            "{context}: {label:?}: el icono está en {:?} y la maqueta dice {expected_2:?}",
            mesh.rect
        );
        assert_eq!(
            mesh.color,
            if pressed { ACCENT } else { TEXT },
            "{context}: {label:?}"
        );
        let rect = bench.rect_of_button(PaletteButton::Transparent);
        if pressed {
            assert!(there_is_rect(f, rect, SURFACE_0), "{context}: cara hundida");
        } else {
            assert!(
                there_is_rect(f, rect, SURFACE_2) || there_is_rect(f, rect, SURFACE_3),
                "{context}: no hay cara del botón en {rect:?}"
            );
        }
    }

    /// Algún texto de la línea 2 (en `TEXT_DIM`) dice `transparent` y `alpha 0` a la vez.
    fn line_2_mixes_the_two(f: &TestFrame) -> bool {
        f.info.lines[1].contains("transparente") && f.info.lines[1].contains("alfa 0")
    }

    // Fila 2: se pinta y se pulsa donde la maqueta dice.

    #[test]
    fn row_2_the_icon_is_painted_with_the_state_of_the_label_at_the_five_ppp() {
        for ppp in PPPS {
            let context = format!("ppp {ppp}");
            // Sin marca: `Fijar`; con la activa marcada: `Remove`; con otra marcada: `Fijar`.
            for (mark, active_2, expected_2, other_2) in [
                (None, 1u8, SET, REMOVE),
                (Some(1u8), 1, REMOVE, SET),
                (Some(2), 1, SET, REMOVE),
            ] {
                let mut bench = Bench::new_2(
                    ppp,
                    SCREEN_HIGH,
                    image_alpha_0(mark),
                    ColorChoice::Entry(active_2),
                );
                let f = bench.rest_2_2();
                let context = format!("{context}, marca {mark:?}");
                checks_button(&f, &bench, expected_2, &context);
                // El estado contrario se ve distinto (hundido o no): `other_2` no se cumple.
                assert_eq!(
                    bench.ink(&f, Icon::Transparent) == ACCENT,
                    other_2 == SET,
                    "{context}: solo un estado"
                );
                // Las tres primeras filas siguen donde estaban.
                checks_buttons(&f, &bench, &NORMAL, &context);
                checks_inside_the_panel(&f, bench.rect_panel(), &context);
            }
        }
    }

    #[test]
    fn row_2_with_the_panel_at_its_minimum_height_the_button_fits_without_scrolling() {
        for ppp in PPPS {
            let n = i64::from(text_scale(ppp));
            let mut bench = Bench::with_panel_height(
                ppp,
                min_panel_height_px(n as u32),
                image_f(),
                ColorChoice::Entry(1),
            );
            let context = format!("ppp {ppp}");
            assert!(!bench.layout().scrolling, "{context}");
            assert_eq!(bench.panel_h_px(), 277 * n, "{context}");
            let f = bench.rest_2_2();
            checks_button(&f, &bench, SET, &context);
            checks_buttons(&f, &bench, &NORMAL, &context);
            checks_inside_the_panel(&f, bench.rect_panel(), &context);
            // The button ends 2 u before the top edge of the editor.
            let rect = bench.rect_of_button(PaletteButton::Transparent);
            let editor_top = bench.rect_panel().min.y + bench.geo().top_px as f32 / ppp;
            assert!(
                near(rect.max.y + 2.0 * n as f32 / ppp, editor_top),
                "{context}"
            );
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(Some(1)), "{context}");
        }
    }

    #[test]
    fn row_2_pressing_at_the_mockup_points_of_the_new_button() {
        for ppp in PPPS {
            let mut bench = bench_f(ppp, 2);
            let n = bench.n();
            let top = bench.geo().icons_top_px;
            // Dentro: la esquina y el último píxel físico; fuera: izquierda, derecha, 1 u sobre
            // el botón y bajo él.
            for (x, y, inside) in [
                (118 * n, 2 * n, true),
                (142 * n - 1, 26 * n - 1, true),
                (130 * n, 14 * n, true),
                (117 * n, 14 * n, false),
                (142 * n, 14 * n, false),
                (130 * n, 185 * n, false),
                (130 * n, 26 * n, false),
            ] {
                bench.history.clear();
                let f = bench.click(x, top + y);
                let context = format!("ppp {ppp}, ({x}, {y})");
                if inside {
                    assert_eq!(f.output_2.op, op_of(Some(2)), "{context}");
                } else {
                    assert_eq!(f.output_2.op, None, "{context}");
                }
                assert_eq!(f.output_2.notice, None, "{context}");
                assert_eq!(bench.choices(), vec![], "{context}");
            }
            // Y las filas 1 a 3 siguen en sus puntos.
            let f = bench.click(4 * n, top + 2 * n);
            assert_eq!(f.output_2.op, Some(PaletteOp::Add));
        }
    }

    #[test]
    fn row_2_with_the_panel_scrolled_the_button_is_pressed_with_y_minus_offset() {
        for ppp in PPPS {
            let n = i64::from(text_scale(ppp));
            let mut bench =
                Bench::with_panel_height(ppp, 200 * n, image_f(), ColorChoice::Entry(1));
            assert!(bench.layout().scrolling, "ppp {ppp}");
            // Sin desplazar el botón queda por debajo del panel: no se golpea.
            let (x, _) = bench.center_of_button(PaletteButton::Transparent);
            let f = bench.click(x, (55 + 248) * n);
            assert_eq!(f.output_2.op, None, "ppp {ppp}: fuera del panel");
            // Con el máximo desplazamiento entero cae dentro.
            let (tx, ty) = bench.over_the_text();
            bench.wheel_in(tx, ty, -100_000 * n);
            bench.wheel(bench.max_offset());
            assert_eq!(bench.view.offset_px, bench.max_offset(), "ppp {ppp}");
            let f = bench.rest_2_2();
            checks_button(&f, &bench, SET, &format!("ppp {ppp}"));
            checks_inside_the_panel(&f, bench.rect_panel(), &format!("ppp {ppp}"));
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(Some(1)), "ppp {ppp}");
        }
    }

    // Fila 3: pulsar emite la op y el rótulo sale del estado.

    #[test]
    fn row_3_pressing_set_emits_set_transparent_and_nothing_else() {
        for ppp in PPPS {
            let mut bench = bench_f(ppp, 2);
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(
                f.output_2,
                PanelOutput {
                    op: op_of(Some(2)),
                    ..PanelOutput::default()
                },
                "ppp {ppp}"
            );
            bench.rest_2_2();
            bench.rest_2_2();
            assert_eq!(
                bench.operations(),
                vec![PaletteOp::SetTransparent { index: Some(2) }]
            );
            assert!(bench.ops.removal().is_none());
            assert!(bench.notices().is_empty());
            assert!(bench.choices().is_empty(), "no cambia la selección");
            assert_eq!(bench.choice, ColorChoice::Entry(2));
        }
    }

    #[test]
    fn row_3_once_the_op_is_applied_the_label_and_line_2_change_and_pressing_again_removes() {
        for ppp in PPPS {
            let mut bench = bench_f(ppp, 2);
            let id = ImageId::from_raw(0);
            bench.click_button(PaletteButton::Transparent);
            bench.applies(Command::SetTransparentIndex {
                image: id,
                index: Some(2),
            });
            let f = bench.rest_2_2();
            let context = format!("ppp {ppp}");
            checks_button(&f, &bench, REMOVE, &context);
            assert!(f.says("activa · transparente"), "{context}");

            bench.history.clear();
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(
                f.output_2.op,
                op_of(None),
                "{context}: la activa es la marcada"
            );
            bench.applies(Command::SetTransparentIndex {
                image: id,
                index: None,
            });
            let f = bench.rest_2_2();
            checks_button(&f, &bench, SET, &context);
            assert!(f.says("activa"), "{context}");

            // Deshacer: vuelve a la marca y el rótulo cambia solo.
            bench.undoes();
            let f = bench.rest_2_2();
            checks_button(&f, &bench, REMOVE, &context);
            bench.undoes();
            let f = bench.rest_2_2();
            checks_button(&f, &bench, SET, &context);
        }
    }

    #[test]
    fn row_3_with_another_entry_marked_the_set_button_sets_the_active_one() {
        for ppp in PPPS {
            let mut bench = Bench::new_2(
                ppp,
                SCREEN_HIGH,
                image_with(&[RED, GREEN, BLUE, TEST_BLACK], Some(1), &[0, 1, 2, 3]),
                ColorChoice::Entry(3),
            );
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(Some(3)), "ppp {ppp}");
        }
    }

    // Fila 5: la línea 2 de la muestra.

    #[test]
    fn row_5_line_2_of_a_marked_one_of_alpha_0_unmarked_of_an_opaque_one_and_empty() {
        for ppp in PPPS {
            // Marcada la 2; la 1 tiene alfa guardado 0; la 0 y la 3 son opacas.
            for (active_2, expected_2) in [
                (2u8, "activa · transparente"),
                (1, "activa · alfa 0"),
                (3, "activa"),
                (0, "activa"),
            ] {
                let mut bench = Bench::new_2(
                    ppp,
                    SCREEN_HIGH,
                    image_alpha_0(Some(2)),
                    ColorChoice::Entry(active_2),
                );
                let f = bench.rest_2_2();
                let context = format!("ppp {ppp}, activa {active_2}");
                assert!(
                    f.says(expected_2),
                    "{context}: the panel does not say {expected_2:?}: {:?}",
                    f.info
                );
                // `transparent` y `alpha 0` nunca juntas.
                assert!(!line_2_mixes_the_two(&f), "{context}");
            }
            // Sin cursor ni elegida en la rejilla (elegida fuera de rango no hay): vacía no se
            // puede probar aquí; una opaca solo dice `active_2`.
            let mut bench = Bench::new_2(
                ppp,
                SCREEN_HIGH,
                image_alpha_0(Some(2)),
                ColorChoice::Entry(0),
            );
            let f = bench.rest_2_2();
            assert!(!f.says("activa · alfa 0"), "ppp {ppp}");
            assert!(!f.says("activa · transparente"), "ppp {ppp}");
        }
    }

    #[test]
    fn row_5_with_the_cursor_over_it_the_longest_line_and_the_others() {
        for ppp in PPPS {
            let mut bench = Bench::new_2(
                ppp,
                SCREEN_HIGH,
                image_alpha_0(Some(2)),
                ColorChoice::Entry(2),
            );
            let context = format!("ppp {ppp}");
            // Cursor sobre la activa marcada: la línea más larga, 30 columnas, sin recortar.
            let (x, y) = bench.swatch_center(2);
            let f = bench.moves(x, y);
            let line = "cursor · activa · transparente";
            assert_eq!(line.chars().count(), 30);
            assert!(f.says(line), "{context}");
            // Cursor sobre la de alfa 0 (no es la activa).
            let (x, y) = bench.swatch_center(1);
            assert!(bench.moves(x, y).says("cursor · alfa 0"), "{context}");
            // Cursor sobre una opaca.
            let (x, y) = bench.swatch_center(3);
            assert!(bench.moves(x, y).says("cursor"), "{context}");
            // Con la activa de alfa 0 y el cursor encima.
            bench.choice = ColorChoice::Entry(1);
            let (x, y) = bench.swatch_center(1);
            let f = bench.moves(x, y);
            assert!(f.says("cursor · activa · alfa 0"), "{context}");
            assert!(!line_2_mixes_the_two(&f), "{context}");
            // Y sobre la marcada, con la de alfa 0 activa.
            let (x, y) = bench.swatch_center(2);
            assert!(bench.moves(x, y).says("cursor · transparente"), "{context}");
        }
    }

    #[test]
    fn row_5_the_mark_of_an_entry_with_stored_alpha_0_says_transparent_not_alpha_0() {
        for ppp in PPPS {
            // La 1 tiene alfa guardado 0 y está marcada: ya resuelve alfa 0 y dice `transparent`.
            let mut bench = Bench::new_2(
                ppp,
                SCREEN_HIGH,
                image_alpha_0(Some(1)),
                ColorChoice::Entry(1),
            );
            let f = bench.rest_2_2();
            assert!(f.says("activa · transparente"), "ppp {ppp}");
            assert!(!f.says("activa · alfa 0"), "ppp {ppp}");
            // Tras `Remove` (aplicado en el documento), pasa a decir `alpha 0`.
            let id = ImageId::from_raw(0);
            bench.applies(Command::SetTransparentIndex {
                image: id,
                index: None,
            });
            let f = bench.rest_2_2();
            assert!(f.says("activa · alfa 0"), "ppp {ppp}");
            assert!(!f.says("activa · transparente"), "ppp {ppp}");
            assert!(!line_2_mixes_the_two(&f), "ppp {ppp}");
        }
    }

    #[test]
    fn row_5_with_three_digits_the_labels_and_lines_are_painted_without_clipping() {
        for ppp in PPPS {
            let colors_2: Vec<Rgba> = (0..256usize)
                .map(|i| Rgba::opaque(i as u8, 255 - i as u8, 7))
                .collect();
            let image_2 = image_with(&colors_2, Some(254), &[0, 255]);
            let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_2, ColorChoice::Entry(255));
            let context = format!("ppp {ppp}");
            let f = bench.rest_2_2();
            checks_button(&f, &bench, SET, &context);
            assert!(f.says("activa"), "{context}");
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(Some(255)), "{context}");
            bench.choice = ColorChoice::Entry(254);
            let f = bench.rest_2_2();
            checks_button(&f, &bench, REMOVE, &context);
            assert!(f.says("activa · transparente"), "{context}");
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(None), "{context}");
        }
    }

    #[test]
    fn row_5_the_labels_come_from_the_character_set_and_fit_in_225_u() {
        assert_eq!(6 + 7 * REMOVE.chars().count(), 139);
        for label in [SET, REMOVE, "alfa 0", "transparente"] {
            assert!(
                label.chars().all(|c| INTERFACE_CHARSET.contains(c)),
                "{label}"
            );
            let used = 6 + 7 * label.chars().count() as i64;
            assert!(used <= 225, "{label}: {used} u");
            assert_eq!(
                fit_text(label, 32, |c| INTERFACE_CHARSET.contains(c)),
                label
            );
        }
    }

    #[test]
    fn row_5_the_button_is_never_dimmed() {
        for ppp in PPPS {
            let cases: [(&str, Image, u8, &str); 4] = [
                ("F sin marca", image_f(), 1, SET),
                ("una entrada", image_a_input(), 0, SET),
                ("256", image_indexed_256(), 5, SET),
                (
                    "marcada",
                    image_with(&[RED, GREEN], Some(0), &[0, 1]),
                    0,
                    REMOVE,
                ),
            ];
            for (name_2, image_2, active_2, label) in cases {
                let mut bench =
                    Bench::new_2(ppp, SCREEN_HIGH, image_2, ColorChoice::Entry(active_2));
                let f = bench.rest_2_2();
                // Nunca atenuado: normal con `TEXT`, o hundido con `ACCENT` si es la marcada.
                let expected_2 = if label == REMOVE { ACCENT } else { TEXT };
                assert_eq!(
                    bench.ink(&f, Icon::Transparent),
                    expected_2,
                    "ppp {ppp}, {name_2}"
                );
            }
        }
    }

    #[test]
    fn row_5_with_one_entry_and_with_256_pressing_emits_the_op_without_a_notice() {
        for ppp in PPPS {
            let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_a_input(), ColorChoice::Entry(0));
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(Some(0)), "ppp {ppp}");
            assert_eq!(f.output_2.notice, None, "ppp {ppp}");

            let mut bench = Bench::new_2(
                ppp,
                SCREEN_HIGH,
                image_indexed_256(),
                ColorChoice::Entry(255),
            );
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, op_of(Some(255)), "ppp {ppp}");
            assert_eq!(f.output_2.notice, None, "ppp {ppp}");
        }
    }

    // Fila 9: bloqueos.

    /// `enabled` falso (un gesto de dibujo en curso; un diálogo abierto da el mismo `enabled`).
    #[test]
    fn row_9_with_a_gesture_or_an_open_dialog_the_button_is_seen_and_does_nothing() {
        for ppp in PPPS {
            for (mark, label) in [(None, SET), (Some(1u8), REMOVE)] {
                let mut bench =
                    Bench::new_2(ppp, SCREEN_HIGH, image_alpha_0(mark), ColorChoice::Entry(1));
                bench.gesture_drawing = true;
                let context = format!("ppp {ppp}, marca {mark:?}");
                bench.click_button(PaletteButton::Transparent);
                let (x, y) = bench.center_of_button(PaletteButton::Transparent);
                bench.presses_2(x, y);
                bench.rest_2_2();
                bench.releases(x, y);
                let f = bench.rest_2_2();
                assert!(bench.history_empty(), "{context}: {:?}", bench.history);
                assert_eq!(bench.doc_state, 0, "{context}");
                assert_eq!(bench.choice, ColorChoice::Entry(1), "{context}");
                assert!(bench.ops.removal().is_none(), "{context}");
                // Se ve igual.
                checks_button(&f, &bench, label, &context);
            }
        }
    }

    #[test]
    fn row_9_with_a_slider_or_a_swatch_drag_in_progress_it_does_nothing() {
        for ppp in PPPS {
            // Deslizador en curso.
            let mut bench = bench_p_5(ppp, 3);
            let geo = bench.geo();
            let (x, y) = (
                geo.x_of_value(128, Control::Red.max()),
                geo.y_row(Control::Red),
            );
            bench.moves(x, y);
            bench.presses_2(x, y);
            assert!(bench.editor.dragging(), "ppp {ppp}");
            bench.history.clear();
            let (bx, by) = bench.center_of_button(PaletteButton::Transparent);
            bench.moves(bx, by);
            let f = bench.presses_2(bx, by);
            assert_eq!(f.output_2.op, None, "ppp {ppp}: deslizador");
            assert!(bench.operations().is_empty(), "ppp {ppp}");
            assert!(bench.notices().is_empty(), "ppp {ppp}");
            assert!(bench.editor.dragging(), "ppp {ppp}");

            // Arrastre de muestra en curso.
            let mut bench = bench_f(ppp, 1);
            bench.drags(0, 2);
            assert!(bench.ops.busy(), "ppp {ppp}");
            bench.history.clear();
            let (bx, by) = bench.center_of_button(PaletteButton::Transparent);
            bench.moves(bx, by);
            let f = bench.presses_2(bx, by);
            assert_eq!(f.output_2.op, None, "ppp {ppp}: arrastre");
            assert!(bench.ops.busy(), "ppp {ppp}");
            assert!(bench.operations().is_empty(), "ppp {ppp}");
            assert!(bench.notices().is_empty(), "ppp {ppp}");
        }
    }

    #[test]
    fn row_9_with_valid_text_in_the_field_the_commit_arrives_before_the_op() {
        let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(1), "#ff00ff");
        bench.click_button(PaletteButton::Transparent);
        bench.rest_2_2();
        bench.rest_2_2();
        assert_eq!(
            bench.commits(),
            vec![Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(255, 0, 255)
            }]
        );
        assert_eq!(
            bench.operations(),
            vec![PaletteOp::SetTransparent { index: Some(1) }]
        );
        let of_the_commit = bench.history.iter().position(|o| o.commit.is_some());
        let op_index = bench.history.iter().position(|o| o.op.is_some());
        assert!(
            of_the_commit.is_some() && of_the_commit <= op_index,
            "el commit ({of_the_commit:?}) llega antes que la op ({op_index:?})"
        );
        assert!(!bench.editor.has_pending(), "no queda texto");
        assert!(bench.notices().is_empty());
    }

    #[test]
    fn row_9_with_the_click_in_a_single_frame_the_commit_and_the_op_come_out_together() {
        let mut bench = bench_with_pending(image_f(), ColorChoice::Entry(1), "#ff00ff");
        let (x, y) = bench.center_of_button(PaletteButton::Transparent);
        bench.moves(x, y);
        let pos = in_points(bench.ppp(), x, y);
        let f = bench.frame(vec![
            button_2(pos, true, Modifiers::NONE),
            button_2(pos, false, Modifiers::NONE),
        ]);
        assert_eq!(
            f.output_2.commit,
            Some(Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(255, 0, 255)
            })
        );
        assert_eq!(f.output_2.op, op_of(Some(1)));
        assert!(!bench.editor.has_pending(), "no queda texto");
    }

    /// Con una quita pendiente el icono no se ve ni golpea (specs/interfaz-iconos.md, decisión 8):
    /// donde estaba cae `Cancel`, y la quita se cancela además al cambiar el documento.
    #[test]
    fn row_9_with_a_pending_removal_the_icon_is_neither_seen_nor_hit_and_the_removal_is_cancelled_when_the_document_changes()
     {
        for ppp in PPPS {
            let mut bench = bench_p_5(ppp, 3);
            let id = ImageId::from_raw(0);
            // Rango de rotación 3..=4 con Mayús, y la quita pendiente de la 3.
            bench.click_swatch_with(4, SHIFT);
            assert_eq!(bench.ops.range_end(), Some(4), "ppp {ppp}");
            bench.click_button(PaletteButton::Remove);
            assert!(bench.ops.removal().is_some(), "ppp {ppp}");
            let f = bench.rest_2_2();
            checks_buttons(&f, &bench, &WITH_REMOVES, &format!("ppp {ppp}"));
            assert!(
                !f.paints_icon(&bench.icons_2.set, Icon::Transparent),
                "ppp {ppp}: el icono no se ve con una quita pendiente"
            );
            bench.history.clear();

            // Donde estaba el icono ahora está `Cancel`: cancela la quita y no pide ninguna op.
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(f.output_2.op, None, "ppp {ppp}: no golpea");
            assert_eq!(f.output_2.notice, None, "ppp {ppp}");
            assert!(
                bench.ops.removal().is_none(),
                "ppp {ppp}: `Cancelar` cancela"
            );
            assert_eq!(bench.choice, ColorChoice::Entry(3), "ppp {ppp}");
            assert_eq!(bench.ops.range_end(), Some(4), "ppp {ppp}: rango intacto");

            // De nuevo con la quita pendiente: si la sesión cambia el documento (aquí con la op
            // que antes pedía el icono), en el siguiente fotograma la quita se cancela con aviso.
            bench.click_button(PaletteButton::Remove);
            assert!(bench.ops.removal().is_some(), "ppp {ppp}");
            bench.applies(Command::SetTransparentIndex {
                image: id,
                index: Some(3),
            });
            let f = bench.rest_2_2();
            assert_eq!(bench.cancellations, 1, "ppp {ppp}");
            assert!(bench.ops.removal().is_none(), "ppp {ppp}");
            assert_eq!(bench.ops.range_end(), Some(4), "ppp {ppp}");
            assert_eq!(bench.choice, ColorChoice::Entry(3), "ppp {ppp}");
            checks_buttons(&f, &bench, &NORMAL, &format!("ppp {ppp}"));
            checks_button(&f, &bench, REMOVE, &format!("ppp {ppp}"));
        }
    }

    #[test]
    fn row_9_in_rgba_gray_and_grayalpha_there_is_no_button_or_op() {
        for ppp in PPPS {
            for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
                let image = Image::new(64, 48, mode_2).expect("imagen");
                let choice = initial_choice(&image);
                let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image, choice);
                let context = format!("ppp {ppp}, {mode_2:?}");
                let f = bench.rest_2_2();
                assert!(!f.paints_something_with(SET), "{context}");
                assert!(!f.paints_something_with(REMOVE), "{context}");
                let n = bench.n();
                let top = bench.geo().top_px;
                let mut y = top + 4 * n;
                while y < bench.panel_h_px() {
                    for x_u in [10, 60, 120, 200] {
                        let f = bench.click(x_u * n, y);
                        assert_eq!(f.output_2.op, None, "{context}: ({x_u}·n, {y})");
                        assert_eq!(f.output_2.notice, None, "{context}");
                    }
                    y += 8 * n;
                }
                // Donde estaría el botón de indexadas, medido desde el borde superior del editor.
                let (x, y) = (10 * n, top + 248 * n);
                if y < bench.panel_h_px() {
                    let f = bench.click(x, y);
                    assert_eq!(f.output_2.op, None, "{context}");
                }
                assert!(bench.operations().is_empty(), "{context}");
                assert!(bench.notices().is_empty(), "{context}");
            }
        }
    }

    #[test]
    fn row_2_show_editing_view_without_ops_also_paints_the_button() {
        // La spec no dice si el botón emite `op` sin estado de operaciones (como `Añadir`, no lo
        // necesita); aquí solo se fija que se pinta.
        for ppp in PPPS {
            let mut bench = bench_f(ppp, 2);
            bench.without_ops = true;
            let f = bench.rest_2_2();
            checks_button(&f, &bench, SET, &format!("ppp {ppp}"));
        }
    }
}

// Unidad 21, ampliación: clic real con el rótulo del estado y avisos en la barra de estado.

mod set_transparent_click_and_bar {
    use super::*;
    use px_ui::palette_ops::{summary, transparent_summary, transparent_toggle};
    use px_ui::status::{bar_text, columns};
    use px_ui::text::fit_text;
    use px_ui::theme::NOTICE_PREFIX;

    /// Con un clic real (mover, pulsar y soltar en la ventana del banco), la op que sale es
    /// `SetTransparent { index: transparent_toggle(palette, active_2) }`, en los dos estados.
    #[test]
    fn the_real_click_emits_the_toggle_op_with_the_active_one_unmarked_and_with_the_active_one_marked()
     {
        for ppp in PPPS {
            for mark in [None, Some(0u8), Some(1), Some(3)] {
                for active_2 in 0..4u8 {
                    let image_2 = image_with(&[RED, GREEN, BLUE, TEST_BLACK], mark, &[0, 1, 2, 3]);
                    let expected_2 = transparent_toggle(image_2.palette().unwrap(), active_2);
                    assert_eq!(
                        expected_2,
                        if mark == Some(active_2) {
                            None
                        } else {
                            Some(active_2)
                        }
                    );
                    let mut bench =
                        Bench::new_2(ppp, SCREEN_HIGH, image_2, ColorChoice::Entry(active_2));
                    let context = format!("ppp {ppp}, marca {mark:?}, activa {active_2}");
                    let f = bench.click_button(PaletteButton::Transparent);
                    assert_eq!(
                        f.output_2.op,
                        Some(PaletteOp::SetTransparent { index: expected_2 }),
                        "{context}"
                    );
                    // Tras pulsar y soltar, la op salió una sola vez y no hay nada más.
                    bench.rest_2_2();
                    assert_eq!(bench.operations().len(), 1, "{context}");
                    assert!(bench.notices().is_empty(), "{context}");
                    assert!(bench.choices().is_empty(), "{context}");
                }
            }
        }
    }

    /// Los avisos caben en la barra de estado a las cinco escalas: `bar_text` con las columnas
    /// de 361 u − 6 u, `MissingGlyphPolicy::Panic` y sin recortar.
    #[test]
    fn the_transparent_notices_fit_in_the_bar_at_the_five_ppp_without_clipping() {
        for ppp in PPPS {
            let bench = bench_f(ppp, 1);
            let n = text_scale(ppp);
            let columns_2 = columns((361 - 6) as f32 * n as f32, ppp);
            assert_eq!(columns_2, 50, "ppp {ppp}");
            let mut notices: Vec<String> = vec![
                transparent_summary(Some(255), 255, Some(254)),
                transparent_summary(Some(255), 255, None),
                transparent_summary(None, 255, None),
                transparent_summary(Some(3), 3, Some(1)),
                transparent_summary(Some(0), 0, Some(255)),
            ];
            for (index, active_2) in [(Some(255u8), 255u8), (None, 255), (Some(2), 2), (None, 0)] {
                notices.push(summary(
                    &PaletteOp::SetTransparent { index },
                    active_2,
                    256,
                    false,
                    false,
                ));
            }
            for i in [0u8, 9, 10, 99, 100, 255] {
                for p in [0u8, 9, 10, 99, 100, 254] {
                    if p != i {
                        notices.push(transparent_summary(Some(i), i, Some(p)));
                    }
                }
            }
            for text_2 in &notices {
                assert!(text_2.chars().count() <= 43, "{text_2:?}");
                let expected_2 = format!("{NOTICE_PREFIX}{text_2}");
                assert_eq!(
                    bar_text(&bench.ctx, text_2, true, columns_2),
                    expected_2,
                    "ppp {ppp}: se recortó o se sustituyó {text_2:?}"
                );
                assert_eq!(
                    fit_text(&expected_2, columns_2, |_| true),
                    expected_2,
                    "ppp {ppp}: fit_text recorta {text_2:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.4 (specs/interfaz-iconos.md): los iconos del editor, sus estados, tooltips y el icono
// de fijar o quitar la transparente con dos estados
// ---------------------------------------------------------------------------------------------

mod icons_editor {
    use super::*;
    use px_core::ImageId;
    use px_ui::icon_button::{Look, look};
    use px_ui::palette_ops::palette_button_tooltip;
    use px_ui::text::INTERFACE_CHARSET;

    const PPPS_FINE: [f32; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

    fn icon_of(button: PaletteButton) -> Icon {
        ICONS_OF_BUTTON
            .iter()
            .find(|(b, _)| *b == button)
            .map(|(_, i)| *i)
            .expect("botón de icono")
    }

    /// El botón se pinta con la cara, el relieve y el icono del aspecto `l` (spec, decisión 3): la
    /// cara llena el rectángulo, las franjas de 1 u del relieve van arriba y a la izquierda
    /// (`top_left`) y abajo y a la derecha (`bottom_right`), y el icono de `16n` píxeles va a 4 u
    /// del borde más el desplazamiento, con las esquinas en píxeles enteros y el UV completo.
    fn checks(f: &TestFrame, bench: &Bench, button: PaletteButton, l: &Look, context: &str) {
        let (ppp, n) = (bench.ppp(), bench.n());
        let u = n as f32 / ppp;
        let rect = bench.rect_of_button(button);
        assert!(
            there_is_rect(f, rect, l.face),
            "{context}: falta la cara {:?}",
            l.face
        );
        let bands = [
            (
                Rect::from_min_size(rect.min, vec2(rect.width(), u)),
                l.top_left,
            ),
            (
                Rect::from_min_size(rect.min, vec2(u, rect.height())),
                l.top_left,
            ),
            (
                Rect::from_min_size(pos2(rect.min.x, rect.max.y - u), vec2(rect.width(), u)),
                l.bottom_right,
            ),
            (
                Rect::from_min_size(pos2(rect.max.x - u, rect.min.y), vec2(u, rect.height())),
                l.bottom_right,
            ),
        ];
        for (band, color) in bands {
            assert!(
                there_is_rect(f, band, color),
                "{context}: falta la franja del relieve {band:?} de {color:?}"
            );
        }
        let mesh = f
            .mesh_of(&bench.icons_2.set, icon_of(button))
            .unwrap_or_else(|| panic!("{context}: no se pinta el icono de {button:?}"));
        let pan = (4 + l.offset_u) as f32 * u;
        let expected_2 = Rect::from_min_size(rect.min + vec2(pan, pan), vec2(16.0 * u, 16.0 * u));
        assert!(
            near_rect(mesh.rect, expected_2),
            "{context}: el icono está en {:?} y debería estar en {expected_2:?}",
            mesh.rect
        );
        for corner in [mesh.rect.min, mesh.rect.max] {
            for v in [corner.x * ppp, corner.y * ppp] {
                assert!(
                    (v - v.round()).abs() <= 0.05,
                    "{context}: esquina fuera de píxel: {v}"
                );
            }
        }
        assert!(
            ((mesh.rect.width() * ppp) - (16 * n) as f32).abs() <= 0.05,
            "{context}: el icono mide {} px",
            mesh.rect.width() * ppp
        );
        assert_eq!(mesh.color, l.tint, "{context}: tinta");
        assert!(
            mesh.uv.min.distance(pos2(0.0, 0.0)) < 1e-4
                && mesh.uv.max.distance(pos2(1.0, 1.0)) < 1e-4,
            "{context}: UV incompleto: {:?}",
            mesh.uv
        );
    }

    #[test]
    fn row_6_the_icons_are_painted_with_face_relief_and_icon_of_each_state_at_ppp_from_1_to_3() {
        for ppp in PPPS_FINE {
            let context = format!("ppp {ppp}");
            // Normales: los cuatro de la fila con una paleta de 4 entradas (todos habilitados).
            let mut bench = bench_f(ppp, 1);
            let f = bench.rest_2_2();
            for button in [
                PaletteButton::Add,
                PaletteButton::Remove,
                PaletteButton::RotateLeft,
                PaletteButton::RotateRight,
                PaletteButton::Transparent,
            ] {
                checks(
                    &f,
                    &bench,
                    button,
                    &look(false, true, false),
                    &format!("{context} {button:?}"),
                );
            }
            // Con 256 entradas, `Añadir` está desactivado; con una sola, `Remove`.
            let mut bench =
                Bench::new_2(ppp, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(5));
            let f = bench.rest_2_2();
            checks(
                &f,
                &bench,
                PaletteButton::Add,
                &look(false, false, false),
                &context,
            );
            checks(
                &f,
                &bench,
                PaletteButton::Remove,
                &look(false, true, false),
                &context,
            );
            let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_a_input(), ColorChoice::Entry(0));
            let f = bench.rest_2_2();
            checks(
                &f,
                &bench,
                PaletteButton::Remove,
                &look(false, false, false),
                &context,
            );
            checks(
                &f,
                &bench,
                PaletteButton::RotateRight,
                &look(false, true, false),
                &context,
            );
            // Hundido: la entrada elegida es la transparente.
            let mut bench = Bench::new_2(
                ppp,
                SCREEN_HIGH,
                image_with(&[RED, GREEN, BLUE, TEST_BLACK], Some(2), &[0, 1, 2, 3]),
                ColorChoice::Entry(2),
            );
            let f = bench.rest_2_2();
            checks(
                &f,
                &bench,
                PaletteButton::Transparent,
                &look(true, true, false),
                &context,
            );
        }
    }

    #[test]
    fn row_6_with_the_mouse_over_it_the_face_lightens_and_disabled_does_not_respond_to_the_mouse() {
        for ppp in PPPS {
            let mut bench = bench_f(ppp, 1);
            let (x, y) = bench.center_of_button(PaletteButton::RotateLeft);
            bench.moves(x, y);
            let f = bench.rest_2_2();
            checks(
                &f,
                &bench,
                PaletteButton::RotateLeft,
                &look(false, true, true),
                &format!("ppp {ppp}"),
            );
            checks(
                &f,
                &bench,
                PaletteButton::Add,
                &look(false, true, false),
                &format!("ppp {ppp}"),
            );
            // Un botón desactivado con el ratón encima sigue con la cara normal.
            let mut bench =
                Bench::new_2(ppp, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(5));
            let (x, y) = bench.center_of_button(PaletteButton::Add);
            bench.moves(x, y);
            let f = bench.rest_2_2();
            checks(
                &f,
                &bench,
                PaletteButton::Add,
                &look(false, false, true),
                &format!("ppp {ppp}"),
            );
            // Con un gesto en curso todo se ve normal (los controles no responden).
            assert!(f.paints_icon(&bench.icons_2.set, Icon::Add));
        }
    }

    #[test]
    fn row_8_the_editor_tooltips_are_those_of_the_spec_and_fit_in_the_character_set() {
        for (button_2, active_2_2, text_2) in [
            (PaletteButton::Add, false, "Añadir entrada"),
            (PaletteButton::Remove, false, "Quitar entrada"),
            (PaletteButton::RotateLeft, false, "Rotar a la izquierda"),
            (PaletteButton::RotateRight, false, "Rotar a la derecha"),
            (PaletteButton::Transparent, false, "Fijar transparente"),
            (PaletteButton::Transparent, true, "Quitar transparente"),
            (PaletteButton::Confirm, false, "Confirmar"),
            (PaletteButton::Cancel, false, "Cancelar"),
        ] {
            let real = palette_button_tooltip(button_2, active_2_2);
            assert_eq!(real, text_2, "{button_2:?} {active_2_2}");
            assert!(
                real.chars().all(|c| INTERFACE_CHARSET.contains(c)),
                "{real}"
            );
            assert!(!real.contains('\n'), "sin segunda línea ni atajo");
        }
    }

    #[test]
    fn row_8_the_hover_shows_the_tooltip_of_each_icon_with_the_pixel_font_and_without_hover_it_does_not()
     {
        for ppp in [1.0, 1.5, 2.0] {
            for (button, expected_2) in [
                (PaletteButton::Add, "Añadir entrada"),
                (PaletteButton::Remove, "Quitar entrada"),
                (PaletteButton::RotateLeft, "Rotar a la izquierda"),
                (PaletteButton::RotateRight, "Rotar a la derecha"),
                (PaletteButton::Transparent, "Fijar transparente"),
            ] {
                let mut bench = bench_f(ppp, 1);
                bench.time_2.set(Some(1.0));
                let without = bench.rest_2_2();
                assert!(
                    !without.paints(expected_2),
                    "ppp {ppp}: {expected_2} sin hover"
                );
                let (x, y) = bench.center_of_button(button);
                bench.moves(x, y);
                let mut f = bench.rest_2_2();
                for _ in 0..8 {
                    f = bench.rest_2_2();
                }
                assert!(
                    f.paints(expected_2),
                    "ppp {ppp}: falta el tooltip {expected_2:?}"
                );
                bench.moves(3000, 3000);
                let far = bench.rest_2_2();
                assert!(
                    !far.paints(expected_2),
                    "ppp {ppp}: {expected_2} sin el ratón"
                );
            }
        }
    }

    #[test]
    fn row_8_pressing_an_enabled_icon_emits_widget_info_of_type_button_with_its_label() {
        use egui::Role;
        use egui::output::OutputEvent;
        for (button, expected_2) in [
            (PaletteButton::Add, "Añadir entrada"),
            (PaletteButton::RotateRight, "Rotar a la derecha"),
            (PaletteButton::Transparent, "Fijar transparente"),
        ] {
            let mut bench = bench_f(1.0, 1);
            let (x, y) = bench.center_of_button(button);
            let pos = in_points(1.0, x, y);
            bench.moves(x, y);
            let actual = bench.actual();
            bench.editor.sync(actual);
            let mut events_2 = Vec::new();
            for events in [
                vec![button_2(pos, true, Modifiers::NONE)],
                vec![button_2(pos, false, Modifiers::NONE)],
            ] {
                let input = bench.input_2(events);
                let Bench {
                    ctx,
                    scroll,
                    view,
                    image,
                    choice,
                    slots,
                    editor,
                    ops,
                    ..
                } = &mut bench;
                let output_2 = ctx.run_ui(input, |ui| {
                    apply_scale(ui.ctx());
                    px_ui::status::show(ui, "lápiz · color 1 #000000", false);
                    panel_on_the_left(
                        ui,
                        |ui| {
                            palette_panel::show_editing_ops(
                                ui, image, *choice, scroll, view, true, slots, editor, ops,
                            )
                        },
                        |_| (),
                    );
                });
                events_2.extend(output_2.platform_output.events.clone());
                output_2.drop_without_applying_deltas();
            }
            assert!(
                events_2
                    .iter()
                    .any(|e| matches!(e, OutputEvent::Clicked(info)
                    if info.role == Role::Button
                        && info.label.as_deref() == Some(expected_2)
                        && info.enabled)),
                "{button:?}: no hay WidgetInfo de botón con el rótulo {expected_2:?}; hay {events_2:?}"
            );
        }
    }

    #[test]
    fn row_9_the_five_icons_give_the_same_as_before_and_the_disabled_ones_warn() {
        use PaletteOp::*;
        for ppp in PPPS {
            let mut bench = bench_f(ppp, 1);
            let f = bench.click_button(PaletteButton::Add);
            assert_eq!(f.output_2.op, Some(Add), "ppp {ppp}");
            let f = bench.click_button(PaletteButton::RotateLeft);
            assert_eq!(
                f.output_2.op,
                Some(Rotate {
                    start: 0,
                    len: 4,
                    steps: -1
                }),
                "ppp {ppp}"
            );
            let f = bench.click_button(PaletteButton::RotateRight);
            assert_eq!(
                f.output_2.op,
                Some(Rotate {
                    start: 0,
                    len: 4,
                    steps: 1
                }),
                "ppp {ppp}"
            );
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(
                f.output_2.op,
                Some(SetTransparent { index: Some(1) }),
                "ppp {ppp}"
            );
            // `Remove` una entrada usada deja la quita pendiente: aparecen `Confirmar` y `Cancel`.
            bench.click_button(PaletteButton::Remove);
            assert!(bench.ops.removal().is_some(), "ppp {ppp}");
            let f = bench.rest_2_2();
            checks_buttons(&f, &bench, &WITH_REMOVES, &format!("ppp {ppp}"));
            // Desactivados: 256 entradas y una sola entrada avisan, sin op.
            let mut bench =
                Bench::new_2(ppp, SCREEN_HIGH, image_indexed_256(), ColorChoice::Entry(5));
            let f = bench.click_button(PaletteButton::Add);
            assert_eq!(
                (f.output_2.op, f.output_2.notice.as_deref()),
                (None, Some("paleta llena: 256 entradas"))
            );
            let mut bench = Bench::new_2(ppp, SCREEN_HIGH, image_a_input(), ColorChoice::Entry(0));
            let f = bench.click_button(PaletteButton::Remove);
            assert_eq!(f.output_2.op, None);
            assert_eq!(
                f.output_2.notice.as_deref(),
                Some("no se puede quitar la única entrada")
            );
            // Con un gesto en curso no responde ninguno y nada cambia.
            let mut bench = bench_f(ppp, 1);
            bench.gesture_drawing = true;
            for (button, _) in ICONS_OF_BUTTON {
                bench.click_button(button);
            }
            assert!(bench.history_empty(), "ppp {ppp}");
        }
    }

    #[test]
    fn row_10_the_transparent_icon_has_two_states_in_the_same_rectangle_and_alternating_tooltip() {
        for ppp in PPPS {
            let image_2 = |mark| image_with(&[RED, GREEN, BLUE, TEST_BLACK], mark, &[0, 1, 2, 3]);
            let mut rectangles = Vec::new();
            for (mark, active_2, pressed) in [
                (Some(3u8), 3u8, true),
                (Some(3), 1, false),
                (None, 1, false),
                (Some(1), 1, true),
            ] {
                let mut bench = Bench::new_2(
                    ppp,
                    SCREEN_HIGH,
                    image_2(mark),
                    ColorChoice::Entry(active_2),
                );
                let f = bench.rest_2_2();
                let context = format!("ppp {ppp}, marca {mark:?}, activa {active_2}");
                checks(
                    &f,
                    &bench,
                    PaletteButton::Transparent,
                    &look(pressed, true, false),
                    &context,
                );
                let palette = bench.image.palette().expect("indexada");
                let text_2 = px_ui::palette_ops::transparent_button_label(palette, active_2);
                assert_eq!(
                    palette_button_tooltip(PaletteButton::Transparent, pressed),
                    text_2,
                    "{context}: el tooltip sale del estado"
                );
                assert_eq!(text_2 == "Quitar transparente", pressed, "{context}");
                rectangles.push(bench.rect_of_button(PaletteButton::Transparent));
            }
            assert!(
                rectangles.windows(2).all(|p| p[0] == p[1]),
                "ppp {ppp}: mismo rectángulo"
            );
            // Pulsar aplica `SetTransparentIndex` (un paso) y el icono cambia en el siguiente
            // fotograma, con la op aplicada.
            let mut bench = bench_f(ppp, 2);
            let id = ImageId::from_raw(0);
            let f = bench.click_button(PaletteButton::Transparent);
            assert_eq!(
                f.output_2.op,
                Some(PaletteOp::SetTransparent { index: Some(2) }),
                "ppp {ppp}"
            );
            bench.applies(Command::SetTransparentIndex {
                image: id,
                index: Some(2),
            });
            let f = bench.rest_2_2();
            checks(
                &f,
                &bench,
                PaletteButton::Transparent,
                &look(true, true, false),
                &format!("ppp {ppp}: tras fijar"),
            );
        }
    }
}
