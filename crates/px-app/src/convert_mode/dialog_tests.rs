//! Tests de `convert_mode` con el diálogo real (specs/hito-2-convertir-modo.md, unidad 20): la fila 11,
//! el banco de egui del formulario de `convert_form`. En ROJO hasta la implementación.
//!
//! El banco reproduce el orden de fotograma de `App::ui` (como `new_image/dialog_tests.rs` y
//! `px-ui/tests/dialog_open_focus.rs`): primero el menú (`show_menu`), luego las acciones del
//! fotograma (`ConvertMode` abre el `Modal` con `open_convert_mode`, los `Locks` del fotograma y un
//! `ConvertSync` nuevo) y, el último, `show_dialog` seguido de `sync_convert` (con el analizador real
//! y un contador de llamadas). Sin ventana ni capturas: `Context::run_ui` con `RawInput` sintético.
//! El diálogo se abre con un clic real en `TestImage → Convert mode_2…`.
//!
//! - Foco inicial en la primera opción de `target_2` desde el fotograma siguiente al del clic (con el
//!   clic del menú).
//! - El informe cambia al cambiar de destino y **solo** entonces se vuelve a calcular.
//! - Con un destino que necesita permiso, `Accept` se pinta atenuado y el motivo del pie es visible
//!   (`falta: permitir perder el color`, sin `error:`); marcar la casilla lo activa.
//! - El grupo del alfa y el hexadecimal solo se ven con destino Gris (y el hexadecimal solo con su
//!   opción); los errores del hexadecimal (`#12`, `zz`, `#ff000080`).
//! - `Accept` con y sin motivo, `Intro`, `Escape`, `Cancel` y `Tab`.
//! - A la ventana mínima el cuerpo se desplaza solo con destino Gris y solo si no cabe, con título,
//!   pie y botones fijos; a 960 × 720 no.
//! - Contraste ≥ 4,5 en todo texto pintado y sin glifo ausente (`MissingGlyphPolicy::Panic`).
//!
//! **Decisiones y suposiciones:**
//! - Las cinco escalas (`PPPS`) y las dos ventanas (468n × 333n px y 960 × 720 pt) se recorren en las
//!   pruebas **salvo** las que necesitan ver todo el formulario a la vez (qué grupo se pinta, el
//!   grupo del alfa y el hexadecimal), que usan 960 × 720 porque a la mínima el cuerpo se desplaza.
//! - Rótulos de las opciones de `target_2`: `Indexado`, `RGBA`, `Gris with alpha_2`, `Gris` (suposición: los
//!   de la 19; la spec solo da el orden). El resto de rótulos son los de D2.
//! - Se identifica cada pieza por su texto pintado visible. Los literales que se buscan caben en una
//!   fila de 42 caracteres (la zona del informe envuelve a esa anchura).
//! - Se escribe con `Ctrl+A`, doce `Retroceso` y `Event::Text`, con el foco que tenga el campo. Del
//!   hexadecimal no se comprueba el informe con el texto pendiente (la spec no dice si el fondo se
//!   lee del texto pendiente o del confirmado), solo el estado del formulario y `Accept`.
//! - `Intro` convierte desde el foco inicial (en la primera opción); los permisos se dejan con
//!   eventos directos sobre el `Dialog` cuando el foco no debe moverse.
//! - Una muesca de la rueda es de 42 u en el cuerpo y se mide con el puntero en el relleno izquierdo
//!   del cuerpo (`x = 3 u` desde el borde), como en `dialog_view.rs`.
//! - Para el contraste solo se miran los textos dentro del rectángulo del diálogo (el menú de detrás es
//!   de otra unidad); el relleno bajo un texto es el último opaco anterior que contiene su recorte.

use super::*;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{
    Color32, Context, Event as InputEvent, Id, Key, MissingGlyphPolicy, Modifiers,
    MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, Vec2, pos2,
    vec2,
};
use px_core::{Allowed, Image, Mode, Palette, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::color_edit::{HexError, hex_notice};
use px_ui::dialog::{Dialog, DialogKind, Event, Modal, Reaction, Values};
use px_ui::dialog_keys::{FocusTarget, focus_order};
use px_ui::dialog_layout::{DialogLayout, dialog_layout};
use px_ui::dialog_view::{ERROR_PREFIX, show_dialog};
use px_ui::menu::show_menu;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::{TEXT_DISABLED, contrast_ratio, visuals};
use px_ui::tool::Tool;

// ---------------------------------------------------------------------------------------------
// Ayudantes del banco
// ---------------------------------------------------------------------------------------------

/// Las cinco escalas de pantalla de la spec.
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

const TEST_MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];

/// La ventana mínima real de la escala (`468n × 333n` px) o 960 × 720 pt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    Minimum,
    Large,
}

const WINDOWS: [Window; 2] = [Window::Minimum, Window::Large];

fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn screen(ppp: f32, window: Window) -> Vec2 {
    match window {
        Window::Minimum => {
            let n = n_of(ppp) as f32;
            vec2(468.0 * n / ppp, 333.0 * n / ppp)
        }
        Window::Large => vec2(960.0, 720.0),
    }
}

/// El tamaño de la ventana en píxeles físicos.
fn pixels_2(ppp: f32, window: Window) -> (i64, i64) {
    match window {
        Window::Minimum => (468 * n_of(ppp), 333 * n_of(ppp)),
        Window::Large => (
            (960.0 * f64::from(ppp)).round() as i64,
            (720.0 * f64::from(ppp)).round() as i64,
        ),
    }
}

/// Los destinos de `from` en el orden de D2.
fn targets_2(from: Mode) -> [Mode; 3] {
    let v: Vec<Mode> = TEST_MODES.into_iter().filter(|m| *m != from).collect();
    [v[0], v[1], v[2]]
}

fn name_of(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "Indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "Gris con alfa",
        Mode::Gray => "Gris",
    }
}

fn in_the_set(text_2: &str) -> bool {
    text_2
        .chars()
        .all(|c| px_ui::text::INTERFACE_CHARSET.contains(c))
}

// --- Imágenes del banco (con las pérdidas que dice cada comentario) ---

fn rgba_of(pixels: &[Rgba]) -> Image {
    let mut image = Image::new(pixels.len() as u32, 1, Mode::Rgba).expect("imagen RGBA");
    for (x, p) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Rgba(*p))
            .expect("píxel");
    }
    image
}

/// Una imagen pequeña de cada modo:
///
/// - RGBA: a Indexado, 4 colores; a Gris con alfa, color en 3 píxeles; a Gris, color en 3 y alfa en 2.
/// - Indexada (paleta de 3, la 2 transparente, `[0,1,1,2,2]`): a RGBA exacta; a Gris con alfa, color
///   en 2; a Gris, color en 2 y alfa en 2.
/// - Gris con alfa (`[(10,0),(10,255),(200,128),(30,0)]`): a Indexado, 3 entradas con el transparente;
///   a RGBA exacta; a Gris, alfa en 3.
/// - Gris (`[0,127,255]`): exacta a los tres.
fn image_of_bench(mode: Mode) -> Image {
    match mode {
        Mode::Rgba => rgba_of(&[
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 128),
            Rgba::opaque(7, 7, 7),
            Rgba::new(0, 0, 255, 128),
        ]),
        Mode::Indexed => {
            let mut palette = Palette::from_colors([
                Rgba::opaque(10, 10, 10),
                Rgba::opaque(200, 0, 0),
                Rgba::opaque(5, 5, 5),
            ])
            .expect("paleta");
            palette.set_transparent_index(Some(2)).expect("índice");
            let mut image = Image::new_indexed(5, 1, palette).expect("indexada");
            for (x, i) in [0u8, 1, 1, 2, 2].into_iter().enumerate() {
                image
                    .set_pixel(x as u32, 0, Pixel::Indexed(i))
                    .expect("píxel");
            }
            image
        }
        Mode::GrayAlpha => {
            let mut image = Image::new(4, 1, Mode::GrayAlpha).expect("imagen");
            for (x, (gray, alpha)) in [(10u8, 0u8), (10, 255), (200, 128), (30, 0)]
                .into_iter()
                .enumerate()
            {
                image
                    .set_pixel(x as u32, 0, Pixel::GrayAlpha { gray, alpha })
                    .expect("píxel");
            }
            image
        }
        Mode::Gray => {
            let mut image = Image::new(3, 1, Mode::Gray).expect("imagen");
            for (x, v) in [0u8, 127, 255].into_iter().enumerate() {
                image.set_pixel(x as u32, 0, Pixel::Gray(v)).expect("píxel");
            }
            image
        }
    }
}

/// 257 × 1 con 256 colores y el transparente: a indexado no cabe.
fn rgba_of_257_colors() -> Image {
    let mut colors: Vec<Rgba> = (0..256u32)
        .map(|i| Rgba::opaque((i % 256) as u8, (i / 256) as u8, 0))
        .collect();
    colors.push(Rgba::TRANSPARENT);
    rgba_of(&colors)
}

/// El color activo por defecto del banco: opaco.
const ACTIVE: Rgba = Rgba::opaque(0x12, 0x34, 0x56);

// --- Eventos sintéticos ---

fn move_pointer(pos: Pos2) -> InputEvent {
    InputEvent::PointerMoved(pos)
}

fn button_2(pos: Pos2, pressed: bool) -> InputEvent {
    InputEvent::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn key_2(key: Key, modifiers: Modifiers) -> InputEvent {
    InputEvent::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn key_releases(key: Key) -> InputEvent {
    InputEvent::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn wheel_line(top: bool) -> InputEvent {
    InputEvent::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if top { 1.0 } else { -1.0 }),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

fn ctrl_a() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

// --- Contexto y fotogramas ---

fn context(ppp: f32) -> Context {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
    install_fonts(&ctx);
    for _ in 0..2 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..RawInput::default()
        };
        let output = ctx.run_ui(input, |ui| apply_scale(ui.ctx()));
        output.drop_without_applying_deltas();
    }
    ctx
}

fn runs(
    ctx: &Context,
    screen: Vec2,
    events: Vec<InputEvent>,
    draws: impl FnMut(&mut eframe::egui::Ui),
) -> Vec<ClippedShape> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
        events,
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, draws);
    output.textures_delta.clear();
    std::mem::take(&mut output.shapes)
}

// --- Lo pintado, en orden de pintado ---

#[derive(Clone, Debug)]
struct Text {
    text_2: String,
    pos: Pos2,
    size: Vec2,
    clip: Rect,
    colors: Vec<Color32>,
    opacity_2: f32,
}

impl Text {
    fn rect(&self) -> Rect {
        Rect::from_min_size(self.pos, self.size)
    }

    /// Algo del texto cae dentro de su recorte.
    fn visible(&self) -> bool {
        self.rect().intersect(self.clip).is_positive()
    }

    fn disabled(&self) -> bool {
        self.opacity_2 < 1.0
            || self
                .colors
                .iter()
                .any(|c| *c == TEXT_DISABLED || c.a() < 255)
    }
}

#[derive(Clone, Copy, Debug)]
struct Fill {
    rect: Rect,
    fill: Color32,
    clip: Rect,
}

#[derive(Clone, Debug)]
enum Shape2 {
    Text(Text),
    Fill(Fill),
}

#[derive(Clone, Debug, Default)]
struct Painted {
    shapes: Vec<Shape2>,
}

impl Painted {
    fn visible(&self) -> impl Iterator<Item = &Text> + '_ {
        self.shapes.iter().filter_map(|f| match f {
            Shape2::Text(t) if t.visible() => Some(t),
            _ => None,
        })
    }

    /// Un texto visible que, recortado, es exactamente `searched`.
    fn exact(&self, searched: &str) -> Option<&Text> {
        self.visible().find(|t| t.text_2.trim() == searched)
    }

    /// Algún texto visible contiene `searched`.
    fn contains(&self, searched: &str) -> bool {
        self.visible().any(|t| t.text_2.contains(searched))
    }

    fn list(&self) -> Vec<&str> {
        self.visible().map(|t| t.text_2.as_str()).collect()
    }
}

fn collects(shape: &Shape, clip: Rect, outside: &mut Vec<Shape2>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colors = job
                .sections
                .iter()
                .map(|s| {
                    t.override_text_color
                        .unwrap_or(if s.format.color == Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            s.format.color
                        })
                })
                .collect();
            outside.push(Shape2::Text(Text {
                text_2: t.galley.text().to_owned(),
                pos: t.pos,
                size: t.galley.size(),
                clip,
                colors,
                opacity_2: t.opacity_factor,
            }));
        }
        Shape::Rect(r) => {
            if r.fill != Color32::TRANSPARENT {
                outside.push(Shape2::Fill(Fill {
                    rect: r.rect,
                    fill: r.fill,
                    clip,
                }));
            }
        }
        Shape::Vec(shapes) => {
            for shape_2 in shapes {
                collects(shape_2, clip, outside);
            }
        }
        _ => {}
    }
}

fn painted(shapes: &[ClippedShape]) -> Painted {
    let mut outside = Vec::new();
    for clipped in shapes {
        collects(&clipped.shape, clipped.clip_rect, &mut outside);
    }
    Painted { shapes: outside }
}

// --- El banco: el orden de fotograma de `App` ---

/// El analizador real, con un contador de llamadas.
#[derive(Default)]
struct Count {
    calls: usize,
}

impl Analyzer for Count {
    fn analyze(&mut self, image: &Image, target: Mode) -> Analysis {
        self.calls += 1;
        analyze(image, target)
    }
}

struct Bench {
    ctx: Context,
    ppp: f32,
    window: Window,
    from: Mode,
    image: Image,
    active_2: Option<Rgba>,
    modal: Modal,
    focus: Option<FocusTarget>,
    convert: Option<ConvertSync>,
    count: Count,
    /// Los `Values` de cada `Accepted` desde el principio.
    accepted: Vec<Values>,
    /// Los `Cancelled` desde el principio.
    cancelled: usize,
    /// Qué `Id` tenía el foco al final de cada fotograma, y si el diálogo estaba abierto.
    trace: Vec<(bool, Option<Id>)>,
    /// El fotograma (índice en `trace`) en que `show_dialog` pintó el diálogo por primera vez.
    open_in: Option<usize>,
    last_2_2: Painted,
}

impl Bench {
    fn new_2(ppp: f32, window: Window, from: Mode, image: Image, active_2: Option<Rgba>) -> Bench {
        let ctx = context(ppp);
        ctx.set_visuals_of(eframe::egui::Theme::Dark, visuals(ppp));
        Bench {
            ctx,
            ppp,
            window,
            from,
            image,
            active_2,
            modal: Modal::Closed,
            focus: None,
            convert: None,
            count: Count::default(),
            accepted: Vec::new(),
            cancelled: 0,
            trace: Vec::new(),
            open_in: None,
            last_2_2: Painted::default(),
        }
    }

    /// Con el diálogo abierto por un clic en `TestImage → Convert mode_2…` y unos fotogramas de reposo.
    fn open(ppp: f32, window: Window, from: Mode) -> Bench {
        Bench::open_with(ppp, window, from, image_of_bench(from), Some(ACTIVE))
    }

    fn open_with(
        ppp: f32,
        window: Window,
        from: Mode,
        image: Image,
        active_2: Option<Rgba>,
    ) -> Bench {
        let mut b = Bench::new_2(ppp, window, from, image, active_2);
        b.opens_with_the_mouse();
        assert!(
            b.modal.is_open(),
            "ppp {ppp} {window:?} {from:?}: no se abrió"
        );
        b
    }

    fn dialog(&self) -> &Dialog {
        match &self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el modal está cerrado"),
        }
    }

    fn dialog_mut(&mut self) -> &mut Dialog {
        match &mut self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el modal está cerrado"),
        }
    }

    fn frame(&mut self, events: Vec<InputEvent>) {
        let size = screen(self.ppp, self.window);
        let mut reaction = Reaction::None;
        let mut paint_the_dialog = false;
        let shapes = {
            let Bench {
                ctx,
                modal,
                focus,
                convert,
                count,
                image,
                from,
                active_2,
                ..
            } = &mut *self;
            runs(ctx, size, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                let locks = Locks {
                    text_focus: c.text_edit_focused(),
                    modal: modal.is_open(),
                    ..Locks::default()
                };
                let state = UiState {
                    can_undo: false,
                    can_redo: false,
                    zoom: None,
                    tool: Tool::Pencil,
                    locks,
                };
                // Menú y acciones del fotograma, como `App::ui`.
                let mut actions = Vec::new();
                if let Some(action) = show_menu(ui, &state, Platform::Linux) {
                    actions.push(action);
                }
                for action in actions {
                    if action == AppAction::ConvertMode && open_convert_mode(modal, *from, locks) {
                        *focus = None;
                        *convert = Some(ConvertSync::new(*from, 0, *active_2));
                    }
                }
                // El diálogo, el último, y `sync_convert` tras él.
                if let Modal::Open(dialog) = modal {
                    paint_the_dialog = true;
                    reaction = show_dialog(&c, dialog, focus);
                    if let Some(sync) = convert.as_mut() {
                        let history_2 = History {
                            limit: 64 << 20,
                            used: 0,
                        };
                        sync_convert(dialog, sync, image, 0, history_2, count);
                    }
                }
            })
        };
        match reaction {
            Reaction::Accepted(values) => {
                self.accepted.push(values);
                self.modal.abandon();
                self.convert = None;
                self.focus = None;
            }
            Reaction::Cancelled => {
                self.cancelled += 1;
                self.modal.abandon();
                self.convert = None;
                self.focus = None;
            }
            Reaction::Rejected(_) | Reaction::None | Reaction::Pressed { .. } => {}
        }
        if paint_the_dialog && self.open_in.is_none() {
            self.open_in = Some(self.trace.len());
        }
        self.trace
            .push((self.modal.is_open(), self.ctx.memory(|m| m.focused())));
        self.last_2_2 = painted(&shapes);
    }

    fn rest(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn click(&mut self, pos: Pos2) {
        self.frame(vec![move_pointer(pos)]);
        self.frame(vec![button_2(pos, true)]);
        self.frame(vec![button_2(pos, false)]);
    }

    /// Pulsa y suelta una tecla (sin el evento de soltar, egui tomaría una segunda pulsación de la
    /// misma tecla por una repetición).
    fn presses(&mut self, key: Key) {
        self.frame(vec![key_2(key, Modifiers::NONE)]);
        self.frame(vec![key_releases(key)]);
        self.rest();
    }

    /// El centro del texto visible `searched`: el exacto y, si no hay, el primero que lo contiene.
    fn center_of(&self, searched: &str) -> Pos2 {
        self.last_2_2
            .exact(searched)
            .or_else(|| {
                self.last_2_2
                    .visible()
                    .find(|t| t.text_2.contains(searched))
            })
            .unwrap_or_else(|| panic!("no se pinta {searched:?}; hay {:?}", self.last_2_2.list()))
            .rect()
            .center()
    }

    /// Clic en el texto visible `searched` y unos fotogramas de reposo.
    fn click_in(&mut self, searched: &str) {
        let center = self.center_of(searched);
        self.click(center);
        self.rest();
    }

    /// `TestImage → Convert mode_2…` con el ratón, como lo hace el usuario.
    fn opens_with_the_mouse(&mut self) {
        self.rest();
        self.click_in("Imagen");
        assert!(
            self.last_2_2.exact("Convertir modo…").is_some(),
            "el menú Imagen muestra la entrada; hay {:?}",
            self.last_2_2.list()
        );
        assert!(
            !self.last_2_2.contains("Ctrl+") && !self.last_2_2.contains("Cmd+"),
            "la entrada no tiene atajo; hay {:?}",
            self.last_2_2.list()
        );
        let input = self.center_of("Convertir modo…");
        self.click(input);
        self.rest();
    }

    /// Escribe `text_2` en el campo con el foco: `Ctrl+A`, borrar y teclear.
    fn writes(&mut self, text_2: &str) {
        let mut delete = vec![key_2(Key::A, ctrl_a())];
        for _ in 0..12 {
            delete.push(key_2(Key::Backspace, Modifiers::NONE));
        }
        self.frame(delete);
        self.frame(vec![InputEvent::Text(text_2.to_owned())]);
        self.rest();
    }

    /// `Tab` hasta que el foco es `goal` (como mucho una vuelta entera).
    fn focus_until(&mut self, goal: FocusTarget) {
        let ret = focus_order(self.dialog()).len() + 1;
        for _ in 0..ret {
            if self.focus == Some(goal) {
                return;
            }
            self.presses(Key::Tab);
        }
        assert_eq!(self.focus, Some(goal), "el Tab no llega a {goal:?}");
    }

    fn focus_of_egui(&self) -> Option<Id> {
        self.ctx.memory(|m| m.focused())
    }

    /// El `Id` de egui de un campo (el que se enfoca al darle el foco).
    fn id_of(field_2: &'static str) -> Id {
        Id::unique(("pixoko-dialog", field_2))
    }

    /// El `Id` de egui de la opción `i` de un grupo.
    fn id_of_option(field_2: &'static str, i: usize) -> Id {
        Id::unique(("pixoko-dialog-option", field_2, i))
    }

    fn layout(&self) -> DialogLayout {
        let d = self.dialog();
        dialog_layout(
            pixels_2(self.ppp, self.window),
            n_of(self.ppp) as u32,
            d.content_h_u(),
            d.is_fill(),
            d.spec().cancel.is_some(),
        )
    }

    fn u(&self) -> f32 {
        n_of(self.ppp) as f32 / self.ppp
    }

    fn accept_disabled(&self) -> bool {
        self.last_2_2
            .exact("Aceptar")
            .unwrap_or_else(|| panic!("no se pinta Aceptar; hay {:?}", self.last_2_2.list()))
            .disabled()
    }

    /// Elige un destino con un clic real en su opción.
    fn chooses_target(&mut self, mode: Mode) {
        self.click_in(name_of(mode));
    }

    /// Elige una opción de un grupo con eventos directos y un par de fotogramas (sin mover el foco).
    fn chooses(&mut self, field_2: &'static str, index: usize) {
        assert_eq!(
            self.dialog_mut().event(Event::Choice {
                field: field_2,
                index
            }),
            Reaction::None
        );
        self.rest();
    }

    fn mark(&mut self, field_2: &'static str, on: bool) {
        assert_eq!(
            self.dialog_mut().event(Event::Check { field: field_2, on }),
            Reaction::None
        );
        self.rest();
    }

    fn describe(&self) -> String {
        format!(
            "ppp {} {:?} {:?}; aceptados {}; cancelados {}; foco {:?}; pintado {:?}",
            self.ppp,
            self.window,
            self.from,
            self.accepted.len(),
            self.cancelled,
            self.focus,
            self.last_2_2.list()
        )
    }
}

const BOX: &str = "Permitir perder el color (pasar a gris)";
const ALPHA_LABEL: &str = "Si hay alfa, componer sobre:";
const OPTIONS_OF_THE_ALPHA: [&str; 5] = [
    "No perder el alfa",
    "Fondo blanco",
    "Fondo negro",
    "Fondo: el color activo",
    "Fondo: un color hexadecimal",
];

fn plan(target: Mode, luma: bool, drop_alpha: Option<[u8; 3]>) -> Plan {
    Plan {
        target,
        allowed: Allowed { luma, drop_alpha },
    }
}

/// Los fotogramas posteriores al de apertura, con el `Id` que tenía el foco.
fn focus_after_opening(b: &Bench) -> Vec<(usize, Option<Id>)> {
    let from_2 = b.open_in.map_or(b.trace.len(), |i| i + 1);
    b.trace
        .iter()
        .enumerate()
        .skip(from_2)
        .map(|(i, (_, id))| (i, *id))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Fila 11: abrir
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_opened_with_a_click_on_the_menu_the_initial_focus_is_the_first_option_of_the_target() {
    for ppp in PPPS {
        for window in WINDOWS {
            for from in TEST_MODES {
                let b = Bench::open(ppp, window, from);
                assert_eq!(b.modal.kind(), Some(DialogKind::ConvertMode));
                assert!(
                    b.open_in.is_some(),
                    "ppp {ppp} {window:?} {from:?}: nunca se pintó"
                );
                let focus = focus_after_opening(&b);
                assert!(
                    focus.len() >= 3,
                    "ppp {ppp} {window:?} {from:?}: faltan fotogramas tras abrir"
                );
                for (frame, id) in focus {
                    assert_eq!(
                        id,
                        Some(Bench::id_of_option("destino", 0)),
                        "el foco de egui en el fotograma {frame} (se abrió en {:?}) no es la \
                         primera opción del destino; {}",
                        b.open_in,
                        b.describe()
                    );
                }
                assert_eq!(
                    b.focus,
                    Some(FocusTarget::Option("destino", 0)),
                    "ppp {ppp} {window:?} {from:?}"
                );
            }
        }
    }
}

#[test]
fn row_11_the_dialog_paints_title_targets_footer_and_buttons_at_both_windows() {
    for ppp in PPPS {
        for window in WINDOWS {
            for from in TEST_MODES {
                let b = Bench::open(ppp, window, from);
                let mut expected_2_2_2: Vec<&str> =
                    vec!["Convertir modo", "Aceptar", "Cancelar", "Modo de destino"];
                expected_2_2_2.extend(targets_2(from).into_iter().map(name_of));
                for text_2 in expected_2_2_2 {
                    assert!(
                        b.last_2_2.exact(text_2).is_some(),
                        "ppp {ppp} {window:?} {from:?}: falta {text_2:?}; hay {:?}",
                        b.last_2_2.list()
                    );
                }
                // El modo actual no se ofrece: ni siquiera como opción.
                assert!(
                    b.last_2_2.exact(name_of(from)).is_none(),
                    "ppp {ppp} {window:?} {from:?}: se ofrece el modo actual"
                );
            }
        }
    }
}

#[test]
fn row_11_with_the_default_target_all_sources_have_accept_active_and_their_report() {
    // El destino por defecto (el primero de la lista) es exacto desde los cuatro modos de la prueba.
    for ppp in PPPS {
        for (from, expected_2) in [
            (Mode::Indexed, "Sin pérdidas: la conversión es exacta."),
            (Mode::Rgba, "Sin pérdidas. Paleta nueva de 4 colores."),
            (Mode::GrayAlpha, "Sin pérdidas. Paleta nueva de 3 colores,"),
            (Mode::Gray, "Sin pérdidas. Paleta nueva de 3 colores."),
        ] {
            let b = Bench::open(ppp, Window::Large, from);
            assert!(
                b.last_2_2.contains(expected_2),
                "ppp {ppp} {from:?}: falta {expected_2:?}; hay {:?}",
                b.last_2_2.list()
            );
            assert!(!b.accept_disabled(), "ppp {ppp} {from:?}");
            assert!(
                !b.last_2_2.contains(ERROR_PREFIX),
                "ppp {ppp} {from:?}: no hay motivo; hay {:?}",
                b.last_2_2.list()
            );
            assert_eq!(b.count.calls, 1, "ppp {ppp} {from:?}: una llamada al abrir");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el informe, el destino y el permiso del color
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_report_changes_when_the_target_changes_and_only_then_recomputes() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        let context = format!("ppp {ppp}");
        assert_eq!(b.count.calls, 1, "{context}: al abrir");
        assert!(
            b.last_2_2
                .contains("Sin pérdidas. Paleta nueva de 4 colores."),
            "{context}"
        );

        b.chooses_target(Mode::GrayAlpha);
        assert_eq!(b.count.calls, 2, "{context}: un destino nuevo");
        assert!(
            b.last_2_2.contains("Pierde el color en 3 píxeles."),
            "{context}: hay {:?}",
            b.last_2_2.list()
        );
        assert!(!b.last_2_2.contains("Paleta nueva"), "{context}");

        // Reposo, casilla y clics en los demás campos: ningún cálculo más.
        b.rest();
        b.rest();
        assert_eq!(b.count.calls, 2, "{context}: el reposo no calcula");
        b.click_in(BOX);
        b.click_in(BOX);
        assert_eq!(b.count.calls, 2, "{context}: la casilla no calcula");

        b.chooses_target(Mode::Gray);
        assert_eq!(b.count.calls, 3, "{context}");
        assert!(
            b.last_2_2.contains("Pierde el color en 3 píxeles."),
            "{context}"
        );
        assert!(
            b.last_2_2.contains("Pierde el alfa en 2 píxeles."),
            "{context}"
        );

        b.chooses_target(Mode::Indexed);
        assert_eq!(b.count.calls, 4, "{context}");
        assert!(
            b.last_2_2
                .contains("Sin pérdidas. Paleta nueva de 4 colores."),
            "{context}"
        );
    }
}

#[test]
fn row_11_the_report_of_each_source_and_target_says_what_it_would_lose() {
    // (origen, destino, líneas que deben pintarse, líneas que no)
    let cases: [(Mode, Mode, &[&str], &[&str]); 11] = [
        (
            Mode::Indexed,
            Mode::Rgba,
            &["Sin pérdidas: la conversión es exacta."],
            &["Pierde"],
        ),
        (
            Mode::Indexed,
            Mode::GrayAlpha,
            &["Pierde el color en 2 píxeles."],
            &["Pierde el alfa"],
        ),
        (
            Mode::Indexed,
            Mode::Gray,
            &[
                "Pierde el color en 2 píxeles.",
                "Pierde el alfa en 2 píxeles.",
            ],
            &[],
        ),
        (
            Mode::Rgba,
            Mode::GrayAlpha,
            &["Pierde el color en 3 píxeles."],
            &["Pierde el alfa"],
        ),
        (
            Mode::Rgba,
            Mode::Gray,
            &[
                "Pierde el color en 3 píxeles.",
                "Pierde el alfa en 2 píxeles.",
            ],
            &[],
        ),
        (
            Mode::GrayAlpha,
            Mode::Rgba,
            &["Sin pérdidas: la conversión es exacta."],
            &["Pierde"],
        ),
        (
            Mode::GrayAlpha,
            Mode::Gray,
            &["Pierde el alfa en 3 píxeles."],
            &["Pierde el color"],
        ),
        (
            Mode::Gray,
            Mode::Indexed,
            &["Sin pérdidas. Paleta nueva de 3 colores."],
            &["Pierde"],
        ),
        (
            Mode::Gray,
            Mode::Rgba,
            &["Sin pérdidas: la conversión es exacta."],
            &["Pierde"],
        ),
        (
            Mode::Gray,
            Mode::GrayAlpha,
            &["Sin pérdidas: la conversión es exacta."],
            &["Pierde"],
        ),
        (
            Mode::Rgba,
            Mode::Indexed,
            &["Sin pérdidas. Paleta nueva de 4 colores."],
            &["Pierde"],
        ),
    ];
    for ppp in PPPS {
        for (from, target_2, must, not_must) in cases {
            let mut b = Bench::open(ppp, Window::Large, from);
            if target_2 != targets_2(from)[0] {
                b.chooses_target(target_2);
            }
            for line in must {
                assert!(
                    b.last_2_2.contains(line),
                    "ppp {ppp} {from:?} a {target_2:?}: falta {line:?}; hay {:?}",
                    b.last_2_2.list()
                );
            }
            for line in not_must {
                assert!(
                    !b.last_2_2.contains(line),
                    "ppp {ppp} {from:?} a {target_2:?}: sobra {line:?}"
                );
            }
        }
    }
}

#[test]
fn row_11_with_a_target_that_needs_permission_accept_is_dimmed_with_the_reason_visible() {
    for ppp in PPPS {
        for window in WINDOWS {
            for from in [Mode::Rgba, Mode::Indexed] {
                let context = format!("ppp {ppp} {window:?} {from:?}");
                let mut b = Bench::open(ppp, window, from);
                assert!(
                    !b.accept_disabled(),
                    "{context}: a Indexado/RGBA, nada que permitir"
                );

                b.chooses_target(Mode::GrayAlpha);
                assert!(b.accept_disabled(), "{context}: falta el permiso");
                assert!(
                    b.last_2_2.contains("falta: permitir perder el color"),
                    "{context}: el motivo del pie es visible; hay {:?}",
                    b.last_2_2.list()
                );
                assert_eq!(
                    b.dialog().block(),
                    Some("falta: permitir perder el color"),
                    "{context}"
                );

                // Marcar la casilla con un clic real lo activa.
                b.click_in(BOX);
                assert_eq!(
                    b.dialog().checked("luma_gris_alfa"),
                    Some(true),
                    "{context}"
                );
                assert!(!b.accept_disabled(), "{context}: con la casilla marcada");
                assert!(
                    !b.last_2_2.contains(ERROR_PREFIX),
                    "{context}: ya no hay motivo; hay {:?}",
                    b.last_2_2.list()
                );

                // Y desmarcarla lo vuelve a atenuar.
                b.click_in(BOX);
                assert!(b.accept_disabled(), "{context}");
                assert!(
                    b.last_2_2.contains("falta: permitir perder el color"),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn row_11_gray_asks_for_the_box_and_the_background_and_each_alpha_option_covers_it() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        let context = |p: &str| format!("ppp {ppp}: {p}");
        b.chooses_target(Mode::Gray);
        assert!(b.accept_disabled(), "{}", context("sin nada"));
        assert!(
            b.last_2_2
                .contains("falta: perder el color; fondo del alfa"),
            "{}: {:?}",
            context("faltan la casilla y el fondo"),
            b.last_2_2.list()
        );

        b.click_in(BOX);
        assert!(
            b.accept_disabled(),
            "{}",
            context("con la casilla y sin fondo")
        );
        assert!(
            b.last_2_2.contains("falta: elegir el fondo del alfa"),
            "{}: {:?}",
            context("falta el fondo"),
            b.last_2_2.list()
        );

        for (option, background_2) in [
            ("Fondo blanco", "Fondo del alfa: #ffffff"),
            ("Fondo negro", "Fondo del alfa: #000000"),
            ("Fondo: el color activo", "Fondo del alfa: #123456"),
        ] {
            b.click_in(option);
            assert!(!b.accept_disabled(), "{}", context(option));
            assert!(!b.last_2_2.contains(ERROR_PREFIX), "{}", context(option));
            assert!(
                b.last_2_2.contains(background_2),
                "{}: falta {background_2:?}; hay {:?}",
                context(option),
                b.last_2_2.list()
            );
        }
        assert_eq!(
            b.count.calls,
            2,
            "{}",
            context("solo abrir y cambiar de destino")
        );

        // «No perder el alfa» vuelve a pedir el fondo.
        b.click_in("No perder el alfa");
        assert!(b.accept_disabled());
        assert!(b.last_2_2.contains("falta: elegir el fondo del alfa"));
    }
}

#[test]
fn row_11_an_active_color_with_alpha_or_without_color_dims_accept_with_its_reason() {
    for ppp in PPPS {
        for (active_2, reason_2) in [
            (
                Some(Rgba::new(0xff, 0, 0, 0x80)),
                "error: el color activo #ff000080 tiene alfa",
            ),
            (None, "error: no hay color activo"),
        ] {
            let mut b = Bench::open_with(
                ppp,
                Window::Large,
                Mode::Rgba,
                image_of_bench(Mode::Rgba),
                active_2,
            );
            b.chooses_target(Mode::Gray);
            b.click_in(BOX);
            b.click_in("Fondo: el color activo");
            assert!(b.accept_disabled(), "ppp {ppp} {active_2:?}");
            assert!(
                b.last_2_2.contains(reason_2),
                "ppp {ppp} {active_2:?}: falta {reason_2:?}; hay {:?}",
                b.last_2_2.list()
            );
            assert!(
                !b.last_2_2.contains("Fondo del alfa"),
                "ppp {ppp} {active_2:?}: no hay fondo válido"
            );
            // Otro fondo lo resuelve.
            b.click_in("Fondo blanco");
            assert!(!b.accept_disabled(), "ppp {ppp} {active_2:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el grupo del alfa y el hexadecimal
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_alpha_group_shows_only_with_a_gray_target_and_the_hexadecimal_only_with_its_option() {
    for ppp in PPPS {
        for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
            let mut b = Bench::open(ppp, Window::Large, from);
            let context = |p: &str| format!("ppp {ppp} {from:?}: {p}");
            // Destino por defecto: ni casilla, ni grupo del alfa, ni hexadecimal.
            for text_2 in [BOX, ALPHA_LABEL, "Color"] {
                assert!(
                    b.last_2_2.exact(text_2).is_none(),
                    "{}: {text_2}",
                    context("por defecto")
                );
            }
            assert!(!b.dialog().is_shown("alfa_gris"));

            // A Gris con alfa (solo desde RGBA e indexado): la casilla y ningún grupo del alfa.
            if from != Mode::GrayAlpha {
                b.chooses_target(Mode::GrayAlpha);
                assert!(
                    b.last_2_2.exact(BOX).is_some(),
                    "{}",
                    context("Gris con alfa")
                );
                assert!(
                    b.last_2_2.exact(ALPHA_LABEL).is_none(),
                    "{}",
                    context("Gris con alfa")
                );
                assert!(!b.dialog().is_shown("alfa_gris"));
            }

            // A Gris: el grupo del alfa con sus cinco opciones y, desde RGBA e indexado, la casilla.
            b.chooses_target(Mode::Gray);
            assert!(
                b.last_2_2.exact(ALPHA_LABEL).is_some(),
                "{}",
                context("Gris")
            );
            for option in OPTIONS_OF_THE_ALPHA {
                assert!(
                    b.last_2_2.exact(option).is_some(),
                    "{}: falta {option:?}; hay {:?}",
                    context("Gris"),
                    b.last_2_2.list()
                );
            }
            assert_eq!(
                b.last_2_2.exact(BOX).is_some(),
                from != Mode::GrayAlpha,
                "{}",
                context("la casilla solo desde RGBA e indexado")
            );
            assert!(
                b.last_2_2.exact("Color").is_none(),
                "{}",
                context("sin la opción del hexadecimal")
            );
            assert!(!b.dialog().is_shown("fondo_hex"));

            // Con la opción del hexadecimal sale «Color» con su valor de nacimiento.
            b.click_in("Fondo: un color hexadecimal");
            assert!(
                b.dialog().is_shown("fondo_hex"),
                "{}",
                context("hexadecimal")
            );
            assert!(
                b.last_2_2.exact("Color").is_some(),
                "{}",
                context("hexadecimal")
            );
            assert!(
                b.last_2_2.contains("#808080"),
                "{}: nace en gris medio; hay {:?}",
                context("hexadecimal"),
                b.last_2_2.list()
            );

            // Otra opción lo oculta.
            b.click_in("Fondo blanco");
            assert!(
                !b.dialog().is_shown("fondo_hex"),
                "{}",
                context("otra opción")
            );
            assert!(
                b.last_2_2.exact("Color").is_none(),
                "{}",
                context("otra opción")
            );

            // Volver a otro destino oculta el grupo entero (y con él el hexadecimal).
            b.click_in("Fondo: un color hexadecimal");
            b.chooses_target(targets_2(from)[0]);
            assert!(
                b.last_2_2.exact(ALPHA_LABEL).is_none(),
                "{}",
                context("destino 0")
            );
            assert!(
                b.last_2_2.exact("Color").is_none(),
                "{}",
                context("destino 0")
            );
            assert!(!b.dialog().is_shown("fondo_hex"));
        }
    }
}

#[test]
fn row_11_from_gray_there_is_no_box_group_or_hexadecimal_with_any_target() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Gray);
        for target_2 in targets_2(Mode::Gray) {
            b.chooses_target(target_2);
            for text_2 in [BOX, ALPHA_LABEL, "Color"] {
                assert!(
                    b.last_2_2.exact(text_2).is_none(),
                    "ppp {ppp} a {target_2:?}: sobra {text_2:?}"
                );
            }
            assert!(!b.accept_disabled(), "ppp {ppp} a {target_2:?}: es exacto");
            assert!(
                !b.last_2_2.contains(ERROR_PREFIX),
                "ppp {ppp} a {target_2:?}"
            );
        }
    }
}

#[test]
fn row_11_unreadable_hexadecimal_with_alpha_and_valid_each_give_their_error_or_nothing() {
    for ppp in PPPS {
        for from in [Mode::Rgba, Mode::GrayAlpha] {
            let mut b = Bench::open(ppp, Window::Large, from);
            let context = |p: &str| format!("ppp {ppp} {from:?}: {p}");
            b.chooses_target(Mode::Gray);
            if from == Mode::Rgba {
                b.click_in(BOX);
            }
            b.click_in("Fondo: un color hexadecimal");
            b.focus_until(FocusTarget::Field("fondo_hex"));

            // `#12` y `zz`: demasiado cortos.
            let short = hex_notice(&HexError::Length(2));
            for text_2 in ["#12", "zz"] {
                b.writes(text_2);
                assert!(
                    b.last_2_2.contains(&format!("{ERROR_PREFIX}{short}")),
                    "{}: falta el error en línea; hay {:?}",
                    context(text_2),
                    b.last_2_2.list()
                );
                assert!(
                    b.last_2_2
                        .contains(&format!("{ERROR_PREFIX}Color: {short}")),
                    "{}: falta el motivo del pie; hay {:?}",
                    context(text_2),
                    b.last_2_2.list()
                );
                assert!(b.accept_disabled(), "{}", context(text_2));
            }

            // `#ff000080`: legible, pero un fondo no lleva alfa.
            b.writes("#ff000080");
            assert_eq!(
                b.dialog().reason(),
                Some("Color: el fondo no lleva alfa".to_string()),
                "{}",
                context("#ff000080")
            );
            assert!(
                b.last_2_2
                    .contains(&format!("{ERROR_PREFIX}Color: el fondo no lleva alfa")),
                "{}: falta el motivo del pie; hay {:?}",
                context("#ff000080"),
                b.last_2_2.list()
            );
            assert!(b.accept_disabled(), "{}", context("#ff000080"));

            // `#ff0000`: vale.
            b.writes("#ff0000");
            assert_eq!(b.dialog().reason(), None, "{}", context("#ff0000"));
            assert!(!b.accept_disabled(), "{}", context("#ff0000"));
            assert!(!b.last_2_2.contains(ERROR_PREFIX), "{}", context("#ff0000"));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Aceptar, Intro, Escape y Cancelar
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_accept_without_a_reason_converts_and_dimmed_does_nothing() {
    for ppp in PPPS {
        for window in WINDOWS {
            // Sin nada que cubrir: Aceptar convierte con el plan por defecto.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            b.click_in("Aceptar");
            assert_eq!(b.accepted.len(), 1, "{}", b.describe());
            assert!(!b.modal.is_open());
            assert_eq!(
                plan_from_values(&b.accepted[0], Mode::Rgba, Some(ACTIVE)),
                Some(plan(Mode::Indexed, false, None)),
                "ppp {ppp} {window:?}"
            );

            // Con un permiso que falta: Aceptar atenuado no convierte ni cierra.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            b.chooses_target(Mode::GrayAlpha);
            assert!(b.accept_disabled());
            b.click_in("Aceptar");
            assert!(
                b.accepted.is_empty(),
                "ppp {ppp} {window:?}: {}",
                b.describe()
            );
            assert!(b.modal.is_open());
            b.click_in(BOX);
            b.click_in("Aceptar");
            assert_eq!(b.accepted.len(), 1, "{}", b.describe());
            assert_eq!(
                plan_from_values(&b.accepted[0], Mode::Rgba, Some(ACTIVE)),
                Some(plan(Mode::GrayAlpha, true, None)),
                "ppp {ppp} {window:?}"
            );
        }
    }
}

#[test]
fn row_11_accept_with_the_gray_target_delivers_the_chosen_background() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.chooses_target(Mode::Gray);
        b.click_in(BOX);
        b.click_in("Fondo negro");
        b.click_in("Aceptar");
        assert_eq!(b.accepted.len(), 1, "{}", b.describe());
        assert_eq!(
            plan_from_values(&b.accepted[0], Mode::Rgba, Some(ACTIVE)),
            Some(plan(Mode::Gray, true, Some([0, 0, 0]))),
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_11_enter_converts_only_with_everything_covered() {
    for ppp in PPPS {
        for window in WINDOWS {
            // Con todo cubierto desde el principio: convierte.
            for from in TEST_MODES {
                let mut b = Bench::open(ppp, window, from);
                b.presses(Key::Enter);
                assert_eq!(b.accepted.len(), 1, "{from:?}: {}", b.describe());
                assert!(!b.modal.is_open());
                assert_eq!(
                    plan_from_values(&b.accepted[0], from, Some(ACTIVE)),
                    Some(plan(targets_2(from)[0], false, None)),
                    "ppp {ppp} {window:?} {from:?}"
                );
                assert_eq!(b.cancelled, 0);
            }

            // Con un permiso por cubrir: Intro no convierte; con él cubierto, sí.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            // Con un clic real en la opción: el foco queda en ella, como lo deja el usuario.
            b.chooses_target(Mode::GrayAlpha);
            assert!(b.accept_disabled());
            b.presses(Key::Enter);
            assert!(
                b.accepted.is_empty(),
                "ppp {ppp} {window:?}: {}",
                b.describe()
            );
            assert!(b.modal.is_open());
            b.mark("luma_gris_alfa", true);
            b.presses(Key::Enter);
            assert_eq!(
                b.accepted.len(),
                1,
                "ppp {ppp} {window:?}: {}",
                b.describe()
            );
            assert_eq!(
                plan_from_values(&b.accepted[0], Mode::Rgba, Some(ACTIVE)),
                Some(plan(Mode::GrayAlpha, true, None))
            );
        }
    }
}

#[test]
fn row_11_with_more_than_256_colors_accept_is_not_activated_by_anything_and_explains_the_quantizer()
{
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b =
                Bench::open_with(ppp, window, Mode::Rgba, rgba_of_257_colors(), Some(ACTIVE));
            let context = format!("ppp {ppp} {window:?}");
            assert!(b.accept_disabled(), "{context}");
            assert!(
                b.last_2_2
                    .contains("error: más de 256 colores: reduce colores"),
                "{context}: el motivo del pie; hay {:?}",
                b.last_2_2.list()
            );
            assert!(
                b.last_2_2.contains("Reducir colores"),
                "{context}: el informe lo explica; hay {:?}",
                b.last_2_2.list()
            );
            assert_eq!(
                b.dialog().block(),
                Some("más de 256 colores: reduce colores"),
                "{context}"
            );
            b.presses(Key::Enter);
            assert!(b.accepted.is_empty(), "{context}");
            b.click_in("Aceptar");
            assert!(b.accepted.is_empty(), "{context}");
            assert!(b.modal.is_open());
            // Cualquier permiso marcado, en cualquier destino de salida, no lo activa a Indexado.
            b.chooses("destino", 1);
            b.mark("luma_gris_alfa", true);
            b.chooses("destino", 0);
            assert!(b.accept_disabled(), "{context}: sigue siendo Indexado");
            b.presses(Key::Enter);
            assert!(b.accepted.is_empty(), "{context}");
        }
    }
}

#[test]
fn row_11_cancel_and_escape_close_without_converting() {
    for ppp in PPPS {
        for window in WINDOWS {
            for from in TEST_MODES {
                let mut b = Bench::open(ppp, window, from);
                b.click_in("Cancelar");
                assert_eq!(b.cancelled, 1, "{}", b.describe());
                assert!(b.accepted.is_empty());
                assert!(!b.modal.is_open());

                let mut b = Bench::open(ppp, window, from);
                b.presses(Key::Escape);
                assert_eq!(b.cancelled, 1, "{}", b.describe());
                assert!(b.accepted.is_empty());
                assert!(!b.modal.is_open());
            }
        }
    }
}

#[test]
fn row_11_cancel_with_a_pending_permission_and_with_accept_dimmed_also_closes() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.chooses_target(Mode::GrayAlpha);
        assert!(b.accept_disabled());
        b.presses(Key::Escape);
        assert_eq!(b.cancelled, 1, "{}", b.describe());
        assert!(!b.modal.is_open());
        assert!(b.accepted.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Tab
// ---------------------------------------------------------------------------------------------

fn checks_tab(b: &mut Bench, context: &str) {
    let order = focus_order(b.dialog());
    // `Accept` solo está en el orden si el conjunto es válido.
    assert_eq!(
        order.contains(&FocusTarget::Accept),
        b.dialog().is_valid(),
        "{context}: Aceptar y la validez"
    );
    assert!(order.contains(&FocusTarget::Cancel), "{context}");
    // Solo lo visible.
    for &goal in &order {
        match goal {
            FocusTarget::Field(id)
            | FocusTarget::Option(id, _)
            | FocusTarget::Minus(id)
            | FocusTarget::Plus(id)
            | FocusTarget::Zone(id)
            | FocusTarget::Button(id)
            | FocusTarget::ButtonAside(id) => assert!(
                b.dialog().is_shown(id),
                "{context}: {goal:?} está oculto y está en el orden"
            ),
            FocusTarget::Accept | FocusTarget::Cancel => {}
        }
    }
    // Un ciclo entero y una vuelta más: cada paso es el siguiente del orden, y da la vuelta.
    let mut actual = b
        .focus
        .unwrap_or_else(|| panic!("{context}: no hay foco para empezar"));
    for step in 0..=order.len() {
        let position = order
            .iter()
            .position(|t| *t == actual)
            .unwrap_or_else(|| panic!("{context}: {actual:?} no está en {order:?}"));
        let expected_2 = order[(position + 1) % order.len()];
        b.presses(Key::Tab);
        assert_eq!(
            b.focus,
            Some(expected_2),
            "{context}: paso {step} de Tab desde {actual:?}; orden {order:?}"
        );
        actual = expected_2;
    }
}

#[test]
fn row_11_tab_walks_only_what_is_visible_and_wraps_around_in_each_state() {
    for ppp in PPPS {
        for window in WINDOWS {
            // (origen, destino, opción del alfa si procede)
            for (from, target_2, alpha_2) in [
                (Mode::Rgba, Mode::Indexed, None),
                (Mode::Rgba, Mode::GrayAlpha, None),
                (Mode::Rgba, Mode::Gray, None),
                (Mode::Rgba, Mode::Gray, Some(4usize)),
                (Mode::Indexed, Mode::Gray, Some(4)),
                (Mode::GrayAlpha, Mode::Gray, None),
                (Mode::GrayAlpha, Mode::Gray, Some(4)),
                (Mode::Gray, Mode::Indexed, None),
            ] {
                let mut b = Bench::open(ppp, window, from);
                let index_2 = targets_2(from)
                    .iter()
                    .position(|m| *m == target_2)
                    .expect("destino");
                b.chooses("destino", index_2);
                if let Some(option) = alpha_2 {
                    b.chooses("alfa_gris", option);
                }
                let order = focus_order(b.dialog());
                // El grupo del destino lleva siempre sus tres opciones y, solo con Gris, el del alfa.
                for i in 0..3 {
                    assert!(order.contains(&FocusTarget::Option("destino", i)));
                }
                assert_eq!(
                    order.contains(&FocusTarget::Option("alfa_gris", 0)),
                    target_2 == Mode::Gray && from != Mode::Gray
                );
                assert_eq!(
                    order.contains(&FocusTarget::Field("fondo_hex")),
                    alpha_2 == Some(4) && target_2 == Mode::Gray
                );
                checks_tab(
                    &mut b,
                    &format!("ppp {ppp} {window:?} {from:?} a {target_2:?} alfa {alpha_2:?}"),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: desplazamiento a la ventana mínima y a 960 × 720
// ---------------------------------------------------------------------------------------------

/// Pone el origen RGBA con destino Gris y, según `background_2`, «No perder el alfa» (0), «Fondo blanco» (1)
/// o el hexadecimal (4), con eventos directos y un fotograma entre medias.
fn put_gray(b: &mut Bench, background_2: usize) {
    let gray = targets_2(b.from)
        .iter()
        .position(|m| *m == Mode::Gray)
        .expect("destino Gris");
    b.chooses("destino", gray);
    b.chooses("alfa_gris", background_2);
}

/// Mueve el puntero al relleno izquierdo del cuerpo (`x = 3 u`) y da `notches` de rueda hacia abajo.
fn wheel_over_the_body(b: &mut Bench, notches: usize) {
    let layout = b.layout();
    let n = n_of(b.ppp);
    let pos = pos2(
        (layout.body.x + 3 * n) as f32 / b.ppp,
        (layout.body.y + layout.body.height / 2) as f32 / b.ppp,
    );
    b.frame(vec![move_pointer(pos)]);
    b.rest();
    for _ in 0..notches {
        b.frame(vec![wheel_line(false)]);
    }
    b.rest();
}

/// La `y` (en puntos) del texto pintado exacto, aunque quede recortado.
fn y_of(b: &Bench, searched: &str) -> Option<f32> {
    b.last_2_2.shapes.iter().find_map(|f| match f {
        Shape2::Text(t) if t.text_2.trim() == searched => Some(t.pos.y),
        _ => None,
    })
}

#[test]
fn row_11_at_the_minimum_the_body_scrolls_only_with_a_gray_target_and_only_if_it_does_not_fit() {
    for ppp in PPPS {
        // Destinos que no son Gris: nunca se desplaza.
        for target_2 in [Mode::Indexed, Mode::GrayAlpha] {
            let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
            if target_2 != Mode::Indexed {
                b.chooses_target(target_2);
            }
            assert!(b.dialog().content_h_u() <= 252, "ppp {ppp} {target_2:?}");
            assert_eq!(b.layout().max_scroll_u, 0, "ppp {ppp} {target_2:?}");
        }

        // Gris con el informe de dos líneas (color y alfa): cabe en 252 u (246).
        let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
        put_gray(&mut b, 0);
        assert_eq!(
            b.dialog().content_h_u(),
            246,
            "ppp {ppp}: RGBA a Gris sin fondo"
        );
        assert_eq!(
            b.layout().max_scroll_u,
            0,
            "ppp {ppp}: cabe: no se desplaza"
        );

        // Con «Fondo blanco» el informe gana una línea (fondo): 260 u, no cabe.
        let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
        put_gray(&mut b, 1);
        assert_eq!(
            b.dialog().content_h_u(),
            260,
            "ppp {ppp}: RGBA a Gris con fondo"
        );
        assert_eq!(b.layout().max_scroll_u, 260 - 252, "ppp {ppp}");

        // Con el hexadecimal: 296 u.
        let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
        put_gray(&mut b, 4);
        assert_eq!(
            b.dialog().content_h_u(),
            296,
            "ppp {ppp}: con el hexadecimal"
        );
        assert_eq!(b.layout().max_scroll_u, 296 - 252, "ppp {ppp}");

        // Desde Gris con alfa (sin casilla) cabe salvo con el hexadecimal.
        for (background_2, pans) in [(0usize, false), (1, false), (4, true)] {
            let mut b = Bench::open(ppp, Window::Minimum, Mode::GrayAlpha);
            put_gray(&mut b, background_2);
            let content = b.dialog().content_h_u();
            assert_eq!(
                b.layout().max_scroll_u,
                (content - 252).max(0),
                "ppp {ppp}: GrayAlpha a Gris, alfa {background_2}"
            );
            assert_eq!(
                b.layout().max_scroll_u > 0,
                pans,
                "ppp {ppp}: GrayAlpha a Gris, alfa {background_2}, contenido {content}"
            );
        }
    }
}

#[test]
fn row_11_with_the_seven_line_report_the_body_measures_316_352_296_and_332_u() {
    let seven = (1..=7)
        .map(|n| format!("linea {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    for (from, target_2, alpha_2, expected_2) in [
        (Mode::Rgba, Mode::Gray, 0usize, 316i64),
        (Mode::Rgba, Mode::Gray, 4, 352),
        (Mode::Indexed, Mode::Gray, 0, 316),
        (Mode::Indexed, Mode::Gray, 4, 352),
        (Mode::GrayAlpha, Mode::Gray, 0, 296),
        (Mode::GrayAlpha, Mode::Gray, 4, 332),
    ] {
        let mut d = Dialog::new(DialogKind::ConvertMode, convert_form(from)).expect("formulario");
        let gray = targets_2(from)
            .iter()
            .position(|m| *m == target_2)
            .expect("destino");
        d.event(Event::Choice {
            field: "destino",
            index: gray,
        });
        d.event(Event::Choice {
            field: "alfa_gris",
            index: alpha_2,
        });
        assert!(d.set_report("informe", seven.clone()));
        assert_eq!(
            d.content_h_u(),
            expected_2,
            "{from:?} a Gris, alfa {alpha_2}"
        );
        for ppp in PPPS {
            // A la mínima, el cuerpo mide 252 u y se desplaza lo que falta; a 960 × 720, nada.
            let n = n_of(ppp) as u32;
            let minimum = dialog_layout(pixels_2(ppp, Window::Minimum), n, expected_2, false, true);
            assert_eq!(minimum.max_scroll_u, expected_2 - 252, "{from:?} ppp {ppp}");
            let large = dialog_layout(pixels_2(ppp, Window::Large), n, expected_2, false, true);
            assert_eq!(large.max_scroll_u, 0, "{from:?} ppp {ppp}");
        }
    }
    // Con cualquier otro destino y siete líneas, cabe a la mínima.
    for (from, target_2) in [
        (Mode::Rgba, Mode::Indexed),
        (Mode::Rgba, Mode::GrayAlpha),
        (Mode::Indexed, Mode::Rgba),
        (Mode::GrayAlpha, Mode::Indexed),
        (Mode::Gray, Mode::Rgba),
    ] {
        let mut d = Dialog::new(DialogKind::ConvertMode, convert_form(from)).expect("formulario");
        let index_2 = targets_2(from)
            .iter()
            .position(|m| *m == target_2)
            .expect("destino");
        d.event(Event::Choice {
            field: "destino",
            index: index_2,
        });
        assert!(d.set_report("informe", seven.clone()));
        assert!(
            d.content_h_u() <= 252,
            "{from:?} a {target_2:?}: {}",
            d.content_h_u()
        );
    }
}

#[test]
fn row_11_at_the_minimum_the_wheel_scrolls_the_body_and_title_footer_and_buttons_do_not_move() {
    for ppp in PPPS {
        for background_2 in [1usize, 4] {
            let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
            put_gray(&mut b, background_2);
            let context = format!("ppp {ppp}, alfa {background_2}");
            let maximum = b.layout().max_scroll_u;
            assert!(maximum > 0, "{context}");
            let fixed_2 = ["Convertir modo", "Aceptar", "Cancelar"];
            let before_fixed: Vec<Option<f32>> = fixed_2.iter().map(|t| y_of(&b, t)).collect();
            let label_before = y_of(&b, "Modo de destino").expect("Modo de destino");
            assert_eq!(b.dialog().scroll_u(), 0, "{context}");

            wheel_over_the_body(&mut b, 2);
            assert_eq!(b.dialog().scroll_u(), maximum, "{context}: hasta el final");
            let shifted = label_before - y_of(&b, "Modo de destino").expect("Modo de destino");
            assert!(
                (shifted - maximum as f32 * b.u()).abs() <= 0.1,
                "{context}: el cuerpo se movió {shifted} pt y debía {}",
                maximum as f32 * b.u()
            );
            let after_fixed: Vec<Option<f32>> = fixed_2.iter().map(|t| y_of(&b, t)).collect();
            assert_eq!(
                after_fixed, before_fixed,
                "{context}: título, pie y botones fijos"
            );
            assert!(b.last_2_2.exact("Aceptar").is_some(), "{context}");
            assert!(b.last_2_2.exact("Cancelar").is_some(), "{context}");
            // El motivo del pie sigue visible con el cuerpo desplazado.
            assert!(
                b.last_2_2.contains("error: ")
                    || b.last_2_2.contains("falta: ")
                    || !b.accept_disabled(),
                "{context}: si Aceptar está atenuado, el motivo se ve"
            );
        }
    }
}

#[test]
fn row_11_at_the_minimum_the_reason_and_the_buttons_are_visible_with_the_body_at_the_end() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
        put_gray(&mut b, 4);
        assert!(b.accept_disabled());
        wheel_over_the_body(&mut b, 3);
        assert!(b.dialog().scroll_u() > 0, "ppp {ppp}");
        assert!(
            b.last_2_2.contains("falta: permitir perder el color"),
            "ppp {ppp}: el motivo del pie; hay {:?}",
            b.last_2_2.list()
        );
        assert!(b.last_2_2.exact("Aceptar").is_some());
        assert!(b.last_2_2.exact("Cancelar").is_some());
    }
}

#[test]
fn row_11_at_the_minimum_tab_brings_into_view_the_hexadecimal_that_was_outside() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Minimum, Mode::Rgba);
        put_gray(&mut b, 4);
        assert_eq!(b.dialog().scroll_u(), 0, "ppp {ppp}");
        b.focus_until(FocusTarget::Field("fondo_hex"));
        assert!(
            b.dialog().scroll_u() > 0,
            "ppp {ppp}: el foco en el hexadecimal desplaza el cuerpo"
        );
        assert!(b.last_2_2.exact("Cancelar").is_some());
        assert_eq!(
            b.focus_of_egui(),
            Some(Bench::id_of("fondo_hex")),
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_11_at_960_by_720_the_form_does_not_scroll_in_any_state() {
    for ppp in PPPS {
        for (from, background_2) in [(Mode::Rgba, 0usize), (Mode::Rgba, 4), (Mode::GrayAlpha, 4)] {
            let mut b = Bench::open(ppp, Window::Large, from);
            put_gray(&mut b, background_2);
            let context = format!("ppp {ppp} {from:?} alfa {background_2}");
            assert_eq!(b.layout().max_scroll_u, 0, "{context}");
            let fixed_2 = ["Convertir modo", "Modo de destino", "Aceptar", "Cancelar"];
            let before: Vec<Option<f32>> = fixed_2.iter().map(|t| y_of(&b, t)).collect();
            wheel_over_the_body(&mut b, 3);
            assert_eq!(b.dialog().scroll_u(), 0, "{context}");
            let after_2: Vec<Option<f32>> = fixed_2.iter().map(|t| y_of(&b, t)).collect();
            assert_eq!(after_2, before, "{context}: nada se mueve");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: contraste y glifos
// ---------------------------------------------------------------------------------------------

const MIN_TEXT: f64 = 4.5;

/// El último relleno opaco anterior a `until` que contiene `probe` (con el recorte aplicado).
fn fill_under(shapes: &[Shape2], until: usize, probe: Rect) -> Option<Fill> {
    shapes[..until].iter().rev().find_map(|f| match f {
        Shape2::Fill(r)
            if r.fill.a() == 255 && r.rect.intersect(r.clip).expand(0.01).contains_rect(probe) =>
        {
            Some(*r)
        }
        _ => None,
    })
}

/// Los fallos de contraste de los textos del diálogo (no los de detrás, ni los atenuados) y
/// cuántos textos se comprobaron.
fn failures_of_contrast(b: &Bench) -> (Vec<String>, usize) {
    let area = {
        let l = b.layout().dialog;
        let ppp = b.ppp;
        Rect::from_min_max(
            pos2(l.x as f32 / ppp, l.y as f32 / ppp),
            pos2((l.x + l.width) as f32 / ppp, (l.y + l.height) as f32 / ppp),
        )
        .expand(0.5)
    };
    let mut failures = Vec::new();
    let mut checked = 0;
    for (i, shape_2) in b.last_2_2.shapes.iter().enumerate() {
        let Shape2::Text(t) = shape_2 else { continue };
        let visible = t.rect().intersect(t.clip);
        if t.text_2.trim().is_empty() || !visible.is_positive() || !area.contains_rect(visible) {
            continue;
        }
        if t.disabled() {
            continue;
        }
        let probe = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let Some(background_2) = fill_under(&b.last_2_2.shapes, i, probe) else {
            failures.push(format!("el texto {:?} no tiene relleno debajo", t.text_2));
            continue;
        };
        for color in &t.colors {
            checked += 1;
            let ratio = contrast_ratio(*color, background_2.fill);
            if ratio < MIN_TEXT {
                failures.push(format!(
                    "el texto {:?} ({color:?}) sobre {:?} da {ratio:.2} (< {MIN_TEXT})",
                    t.text_2, background_2.fill
                ));
            }
        }
    }
    (failures, checked)
}

fn checks_contrast(b: &Bench, context: &str) {
    let (failures, checked) = failures_of_contrast(b);
    assert!(
        checked >= 5,
        "{context}: solo se comprobaron {checked} textos"
    );
    assert!(failures.is_empty(), "{context}: {failures:#?}");
}

#[test]
fn row_11_all_painted_text_of_the_dialog_has_contrast_of_at_least_4_5() {
    for ppp in PPPS {
        for window in WINDOWS {
            let context = |state_2: &str| format!("ppp {ppp} {window:?}: {state_2}");

            // Por defecto, desde cada origen.
            for from in TEST_MODES {
                let b = Bench::open(ppp, window, from);
                checks_contrast(&b, &context(&format!("por defecto desde {from:?}")));
            }

            // Con un permiso por cubrir: el motivo del pie y Aceptar atenuado (exento).
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            b.chooses_target(Mode::GrayAlpha);
            checks_contrast(&b, &context("falta el permiso del color"));
            b.click_in(BOX);
            checks_contrast(&b, &context("casilla marcada"));

            // RGBA a Gris con el informe de color, alfa y fondo.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            put_gray(&mut b, 1);
            checks_contrast(&b, &context("Gris con fondo blanco"));
            put_gray(&mut b, 0);
            checks_contrast(&b, &context("Gris sin fondo: falta el color"));
            b.mark("luma_gris", true);
            checks_contrast(&b, &context("Gris sin fondo: falta el fondo"));

            // Con el hexadecimal: inválido y con alfa.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            put_gray(&mut b, 4);
            b.focus_until(FocusTarget::Field("fondo_hex"));
            b.writes("zz");
            checks_contrast(&b, &context("hexadecimal inválido"));
            b.writes("#ff000080");
            checks_contrast(&b, &context("hexadecimal con alfa"));
            b.writes("#ff0000");
            checks_contrast(&b, &context("hexadecimal válido"));

            // Más de 256 colores: el informe que manda a Reducir colores y su motivo.
            let b = Bench::open_with(ppp, window, Mode::Rgba, rgba_of_257_colors(), Some(ACTIVE));
            checks_contrast(&b, &context("más de 256 colores"));

            // Con el color activo con alfa: el motivo con su valor resuelto.
            let mut b = Bench::open_with(
                ppp,
                window,
                Mode::Rgba,
                image_of_bench(Mode::Rgba),
                Some(Rgba::new(0xff, 0, 0, 0x80)),
            );
            put_gray(&mut b, 3);
            b.mark("luma_gris", true);
            checks_contrast(&b, &context("color activo con alfa"));
        }
    }
}

#[test]
fn row_11_no_dialog_state_paints_a_missing_glyph() {
    // `MissingGlyphPolicy::Panic` hace fallar cualquier texto con un carácter que la fuente no tenga.
    // Se recorren los textos del formulario (rótulos, opciones, motivos y el informe más largo).
    for ppp in PPPS {
        let mut b = Bench::open_with(
            ppp,
            Window::Large,
            Mode::Rgba,
            rgba_of_257_colors(),
            Some(Rgba::new(0xff, 0, 0, 0x80)),
        );
        for target_2 in targets_2(Mode::Rgba) {
            b.chooses_target(target_2);
        }
        put_gray(&mut b, 3);
        put_gray(&mut b, 4);
        b.focus_until(FocusTarget::Field("fondo_hex"));
        b.writes("#12");
        b.writes("#ff000080");
        let mut b = Bench::open_with(ppp, Window::Large, Mode::Rgba, rgba_of_257_colors(), None);
        assert!(
            b.last_2_2.contains("Reducir colores"),
            "ppp {ppp}: {:?}",
            b.last_2_2.list()
        );
        put_gray(&mut b, 3);
        // Los textos pintados están en el juego.
        for text_2 in b.last_2_2.visible() {
            assert!(in_the_set(&text_2.text_2), "ppp {ppp}: {:?}", text_2.text_2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20b, parte 1 (specs/hito-2-motivo-y-marcadores.md): filas 7 y 8. En ROJO hasta que
// existan `Dialog::problems`, `Dialog::set_summary`, `Dialog::summary`, `footer_text` y el
// resumen de `sync_convert`.
//
// - Fila 7: el caso de la prueba manual con clics reales. El pie se lee del texto pintado en el
//   rectángulo del motivo (`layout.reason`), no de `Dialog::reason()`. El cambio tras un clic se ve
//   **en el fotograma siguiente** al del clic (`click_and_next`: mover, pulsar, soltar y un
//   fotograma más).
// - Fila 8: ancho medido del pie a las cinco escalas, en la ventana mínima y en 960 × 720. Los pies
//   fijados se pintan quitando `ConvertSync` del banco (`convert = None`) y poniendo el motivo con
//   `set_block` o `set_summary`; los reales salen de `sync_convert` con eventos sobre el diálogo.
//
// **Suposiciones y ambigüedades:**
// - Un clic en la casilla de la fila 7 con el hexadecimal con foco hace perder el foco al campo y
//   revierte el texto ilegible (regla de 11c); por eso el paso «con la casilla marcada y `zz`» se
//   hace con eventos directos sobre la casilla (sin mover el foco) o marcando la casilla antes de
//   escribir.
// - El pie «más largo posible» se prueba con tres textos: un motivo de 37 con `error: ` (44 en
//   total), la lista más larga de dos con `falta: ` (la de color y fondo, 38) y el límite físico
//   de `falta: ` + 37 caracteres (44), aunque ningún texto real lo alcance.
// - `faltan 3: color, background_2, value` no se pinta (en el diálogo no es alcanzable y `footer_text` le
//   pondría `error: `, ver `tests.rs`).
// ---------------------------------------------------------------------------------------------

const MISSING_BOTH_20B: &str = "falta: perder el color; fondo del alfa";
const MISSING_COLOR_20B: &str = "falta: permitir perder el color";
const CHOOSE_BACKGROUND_20B: &str = "falta: elegir el fondo del alfa";
const MISSING_COLOR_AND_VALUE_20B: &str = "falta: perder el color; fondo válido";

/// El texto pintado en el rectángulo del motivo (el pie), si lo hay.
fn footer_of(b: &Bench) -> Option<Text> {
    let r = b.layout().reason;
    let ppp = b.ppp;
    b.last_2_2
        .visible()
        .find(|t| {
            (t.pos.x * ppp - r.x as f32).abs() < 1.5 && (t.pos.y * ppp - r.y as f32).abs() < 1.5
        })
        .cloned()
}

fn footer_text(b: &Bench) -> Option<String> {
    footer_of(b).map(|t| t.text_2.trim().to_string())
}

/// Un clic real sobre el texto `searched` y **un** fotograma más: el siguiente al del evento.
fn click_and_next(b: &mut Bench, searched: &str) {
    let center = b.center_of(searched);
    b.click(center);
    b.frame(vec![]);
}

fn error_of_the_field(short: &str) -> String {
    format!("{ERROR_PREFIX}{short}")
}

#[test]
fn row_7_20b_rgba_with_transparency_to_gray_follows_the_manual_test_case() {
    let short = hex_notice(&HexError::Length(2));
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        let ctx = |step: &str| format!("ppp {ppp}: {step}");

        // Gris, sin marcar nada: faltan el color y el fondo, sin `error:`.
        b.chooses_target(Mode::Gray);
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_BOTH_20B),
            "{}: {:?}",
            ctx("sin nada"),
            b.last_2_2.list()
        );
        assert!(b.accept_disabled(), "{}", ctx("sin nada"));
        assert!(!b.last_2_2.contains(ERROR_PREFIX), "{}", ctx("sin nada"));

        // La casilla: queda el fondo, en el fotograma siguiente al clic.
        click_and_next(&mut b, BOX);
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(CHOOSE_BACKGROUND_20B),
            "{}: {:?}",
            ctx("casilla"),
            b.last_2_2.list()
        );
        assert!(b.accept_disabled(), "{}", ctx("casilla"));

        // Un fondo: sin motivo y Aceptar activo, también en el fotograma siguiente.
        click_and_next(&mut b, "Fondo blanco");
        assert_eq!(
            footer_text(&b),
            None,
            "{}: {:?}",
            ctx("blanco"),
            b.last_2_2.list()
        );
        assert!(!b.accept_disabled(), "{}", ctx("blanco"));
        assert_eq!(b.dialog().reason(), None, "{}", ctx("blanco"));
        assert!(!b.last_2_2.contains("falta:") && !b.last_2_2.contains(ERROR_PREFIX));

        // Volver a «No perder el alfa»: otra vez el fondo.
        click_and_next(&mut b, "No perder el alfa");
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(CHOOSE_BACKGROUND_20B),
            "{}",
            ctx("sin fondo")
        );
        assert!(b.accept_disabled(), "{}", ctx("sin fondo"));

        // Desmarcar la casilla con el fondo ya cubierto: solo el color.
        click_and_next(&mut b, "Fondo negro");
        assert_eq!(footer_text(&b), None, "{}", ctx("negro"));
        click_and_next(&mut b, BOX);
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_20B),
            "{}",
            ctx("sin casilla")
        );

        // Hexadecimal ilegible con la casilla sin marcar: color y fondo válido; el error del campo
        // sigue junto al campo.
        click_and_next(&mut b, "Fondo: un color hexadecimal");
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_20B),
            "{}: el hexadecimal por defecto vale",
            ctx("hexadecimal")
        );
        b.focus_until(FocusTarget::Field("fondo_hex"));
        b.writes("zz");
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_AND_VALUE_20B),
            "{}: {:?}",
            ctx("zz sin casilla"),
            b.last_2_2.list()
        );
        assert!(
            b.last_2_2.exact(&error_of_the_field(short)).is_some(),
            "{}: el error del campo sigue junto al campo; hay {:?}",
            ctx("zz sin casilla"),
            b.last_2_2.list()
        );
        assert!(
            !b.last_2_2.contains(&format!("{ERROR_PREFIX}Color:")),
            "{}: el detalle no va en el pie",
            ctx("zz sin casilla")
        );
        assert!(b.accept_disabled(), "{}", ctx("zz sin casilla"));
        assert_eq!(b.dialog().summary(), Some(MISSING_COLOR_AND_VALUE_20B));

        // La casilla marcada con eventos directos (sin mover el foco): solo el valor, con `error:`.
        b.mark("luma_gris", true);
        assert_eq!(
            footer_text(&b),
            Some(format!("{ERROR_PREFIX}Color: {short}")),
            "{}: {:?}",
            ctx("zz con casilla"),
            b.last_2_2.list()
        );
        assert!(
            b.last_2_2.exact(&error_of_the_field(short)).is_some(),
            "{}",
            ctx("zz con casilla")
        );
        assert_eq!(b.dialog().summary(), None, "{}", ctx("zz con casilla"));
        assert!(b.accept_disabled());

        // Y desmarcarla vuelve al resumen.
        b.mark("luma_gris", false);
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_AND_VALUE_20B),
            "{}",
            ctx("otra vez")
        );

        // Arreglar el hexadecimal deja el color; con la casilla, nada.
        b.writes("#ff0000");
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_20B),
            "{}",
            ctx("arreglado")
        );
        b.mark("luma_gris", true);
        assert_eq!(footer_text(&b), None, "{}", ctx("todo cubierto"));
        assert!(!b.accept_disabled(), "{}", ctx("todo cubierto"));
    }
}

#[test]
fn row_7_20b_with_the_box_checked_before_writing_zz_the_footer_is_only_the_value_error() {
    let short = hex_notice(&HexError::Length(2));
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.chooses_target(Mode::Gray);
        click_and_next(&mut b, BOX);
        click_and_next(&mut b, "Fondo: un color hexadecimal");
        b.focus_until(FocusTarget::Field("fondo_hex"));
        b.writes("zz");
        assert_eq!(
            footer_text(&b),
            Some(format!("{ERROR_PREFIX}Color: {short}")),
            "ppp {ppp}: {:?}",
            b.last_2_2.list()
        );
        assert!(!b.last_2_2.contains("falta:"), "ppp {ppp}");
        assert!(
            b.last_2_2.exact(&error_of_the_field(short)).is_some(),
            "ppp {ppp}"
        );
        assert_eq!(b.dialog().summary(), None, "ppp {ppp}");
        assert!(b.accept_disabled());
    }
}

#[test]
fn row_7_20b_the_hexadecimal_with_alpha_without_the_box_also_gives_color_and_valid_background() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.chooses_target(Mode::Gray);
        click_and_next(&mut b, "Fondo: un color hexadecimal");
        b.focus_until(FocusTarget::Field("fondo_hex"));
        b.writes("#ff000080");
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_AND_VALUE_20B),
            "ppp {ppp}: {:?}",
            b.last_2_2.list()
        );
        assert!(b.accept_disabled(), "ppp {ppp}");
        b.mark("luma_gris", true);
        assert_eq!(
            footer_text(&b).as_deref(),
            Some("error: Color: el fondo no lleva alfa"),
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_7_20b_gray_alpha_from_rgba_only_asks_for_the_color_and_there_is_no_summary() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.chooses_target(Mode::GrayAlpha);
        assert_eq!(
            footer_text(&b).as_deref(),
            Some(MISSING_COLOR_20B),
            "ppp {ppp}"
        );
        assert_eq!(b.dialog().summary(), None, "ppp {ppp}");
        click_and_next(&mut b, BOX);
        assert_eq!(footer_text(&b), None, "ppp {ppp}");
        assert!(!b.accept_disabled(), "ppp {ppp}");
    }
}

// --- Fila 8: encaje del texto ---

/// Un rectángulo en píxeles físicos pasado a puntos.
fn rect_in_points(r: px_ui::view::ScreenRect, ppp: f32) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32 / ppp, r.y as f32 / ppp),
        vec2(r.width as f32 / ppp, r.height as f32 / ppp),
    )
}

/// Comprueba el pie pintado de `b`: es `footer_text(reason_2)`, mide como mucho 44 columnas, cabe en
/// `layout.reason.width`, no se recorta ni toca los botones y lleva su pista (`falta:` o `error:`).
fn checks_footer(b: &Bench, reason_2: &str, ctx: &str) {
    let expected_2 = px_ui::dialog_view::footer_text(reason_2);
    assert!(expected_2.chars().count() <= 44, "{ctx}: {expected_2:?}");
    assert!(
        expected_2.starts_with("falta: ") || expected_2.starts_with(ERROR_PREFIX),
        "{ctx}: la pista textual falta en {expected_2:?}"
    );
    assert!(in_the_set(&expected_2), "{ctx}: {expected_2:?}");
    let t = footer_of(b).unwrap_or_else(|| {
        panic!(
            "{ctx}: no se pinta el pie {expected_2:?}; hay {:?}",
            b.last_2_2.list()
        )
    });
    assert_eq!(t.text_2.trim(), expected_2, "{ctx}");

    let layout = b.layout();
    let n = n_of(b.ppp);
    assert_eq!(
        layout.reason.width,
        308 * n,
        "{ctx}: el pie mide 44 columnas de 7 u"
    );
    let width_px = t.size.x * b.ppp;
    assert!(
        width_px <= layout.reason.width as f32 + 0.5,
        "{ctx}: {expected_2:?} mide {width_px} px y el pie {} px",
        layout.reason.width
    );
    // Sin recorte.
    assert!(
        t.clip.expand(0.5).contains_rect(t.rect()),
        "{ctx}: {expected_2:?} queda recortado ({:?} en {:?})",
        t.rect(),
        t.clip
    );
    // Sin solapamiento con los botones.
    let zone = t.rect();
    for button_2 in [Some(layout.accept), layout.cancel].into_iter().flatten() {
        assert!(
            !zone
                .intersect(rect_in_points(button_2, b.ppp))
                .is_positive(),
            "{ctx}: {expected_2:?} toca un botón"
        );
    }
    // Contraste.
    assert!(
        contrast_ratio(
            px_ui::dialog_view::ERROR_FG,
            px_ui::dialog_view::DIALOG_FILL
        ) >= MIN_TEXT
    );
    checks_contrast(b, ctx);
}

#[derive(Clone, Copy, Debug)]
enum Via {
    Block,
    Summary,
}

/// Los pies de D2 y los más largos posibles, y por dónde llegan al diálogo.
fn pies_set() -> Vec<(String, Via)> {
    vec![
        (MISSING_COLOR_20B.to_string(), Via::Block),
        (CHOOSE_BACKGROUND_20B.to_string(), Via::Block),
        (MISSING_BOTH_20B.to_string(), Via::Summary),
        (MISSING_COLOR_AND_VALUE_20B.to_string(), Via::Summary),
        (
            "falta: fondo del alfa; fondo válido".to_string(),
            Via::Summary,
        ),
        ("no hay color activo".to_string(), Via::Block),
        (
            "el color activo #ff000080 tiene alfa".to_string(),
            Via::Block,
        ),
        ("más de 256 colores: reduce colores".to_string(), Via::Block),
        ("la imagen convertida no cabe".to_string(), Via::Block),
        ("calculando el informe".to_string(), Via::Block),
        // El motivo más largo con `error: `: 37 caracteres, 44 en total.
        ("M".repeat(37), Via::Block),
        // El límite físico de `falta: `: 37 tras el prefijo, 44 en total.
        (format!("falta: {}", "M".repeat(37)), Via::Block),
        (format!("falta: {}", "M".repeat(37)), Via::Summary),
    ]
}

#[test]
fn row_8_20b_the_fixed_footers_fit_at_the_five_scales_in_the_two_windows() {
    for ppp in PPPS {
        for window in WINDOWS {
            // Un solo banco por escala y ventana: sin `ConvertSync` el motivo no se recalcula.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            b.convert = None;
            for (reason_2, via) in pies_set() {
                let ctx = format!("ppp {ppp} {window:?} {via:?} {reason_2:?}");
                b.dialog_mut().set_block(None);
                b.dialog_mut().set_summary(None);
                match via {
                    Via::Block => b.dialog_mut().set_block(Some(reason_2.clone())),
                    Via::Summary => b.dialog_mut().set_summary(Some(reason_2.clone())),
                }
                b.rest();
                assert!(b.accept_disabled(), "{ctx}");
                let painted = b.dialog().reason().expect("hay motivo");
                checks_footer(&b, &painted, &ctx);
            }
        }
    }
}

#[test]
fn row_8_20b_the_longest_footer_with_error_and_the_longest_with_missing_fill_the_44_columns() {
    // 37 caracteres con `error: ` y `falta: ` + 37: 44 columnas de 7 u cada uno.
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.convert = None;
        for reason_2 in ["M".repeat(37), format!("falta: {}", "M".repeat(37))] {
            b.dialog_mut().set_block(Some(reason_2.clone()));
            b.rest();
            let footer = footer_of(&b).expect("pie");
            assert_eq!(
                footer.text_2.trim().chars().count(),
                44,
                "ppp {ppp}: {reason_2:?}"
            );
            let layout = b.layout();
            // Fuente monoespaciada: 44 columnas ocupan como mucho el ancho del pie.
            assert!(
                footer.size.x * ppp <= layout.reason.width as f32 + 0.5,
                "ppp {ppp}: {reason_2:?} mide {} px de {}",
                footer.size.x * ppp,
                layout.reason.width
            );
        }
    }
}

#[test]
fn row_8_20b_the_real_footers_of_each_state_fit_at_the_five_scales_in_the_two_windows() {
    let short = hex_notice(&HexError::Length(2));
    for ppp in PPPS {
        for window in WINDOWS {
            let ctx = |state_2: &str| format!("ppp {ppp} {window:?}: {state_2}");

            // Gris sin nada: color y fondo; con la casilla: el fondo.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            put_gray(&mut b, 0);
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(reason_2, MISSING_BOTH_20B, "{}", ctx("sin nada"));
            checks_footer(&b, &reason_2, &ctx("sin nada"));
            b.mark("luma_gris", true);
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(reason_2, CHOOSE_BACKGROUND_20B, "{}", ctx("casilla"));
            checks_footer(&b, &reason_2, &ctx("casilla"));

            // Gris con alfa sin la casilla.
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            b.chooses_target(Mode::GrayAlpha);
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(reason_2, MISSING_COLOR_20B, "{}", ctx("gris con alfa"));
            checks_footer(&b, &reason_2, &ctx("gris con alfa"));

            // Hexadecimal ilegible: con la casilla sin marcar (resumen) y con ella (valor solo).
            let mut b = Bench::open(ppp, window, Mode::Rgba);
            put_gray(&mut b, 4);
            b.focus_until(FocusTarget::Field("fondo_hex"));
            b.writes("zz");
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(
                reason_2,
                MISSING_COLOR_AND_VALUE_20B,
                "{}",
                ctx("zz sin casilla")
            );
            checks_footer(&b, &reason_2, &ctx("zz sin casilla"));
            b.mark("luma_gris", true);
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(
                reason_2,
                format!("Color: {short}"),
                "{}",
                ctx("zz con casilla")
            );
            checks_footer(&b, &reason_2, &ctx("zz con casilla"));
            b.writes("#ff000080");
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(reason_2, "Color: el fondo no lleva alfa", "{}", ctx("alfa"));
            checks_footer(&b, &reason_2, &ctx("alfa"));

            // Color activo con alfa: color y fondo válido (sin resumen).
            let mut b = Bench::open_with(
                ppp,
                window,
                Mode::Rgba,
                image_of_bench(Mode::Rgba),
                Some(Rgba::new(0xff, 0, 0, 0x80)),
            );
            put_gray(&mut b, 3);
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(
                reason_2,
                MISSING_COLOR_AND_VALUE_20B,
                "{}",
                ctx("activo con alfa")
            );
            checks_footer(&b, &reason_2, &ctx("activo con alfa"));
            b.mark("luma_gris", true);
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(
                reason_2,
                "el color activo #ff000080 tiene alfa",
                "{}",
                ctx("activo")
            );
            checks_footer(&b, &reason_2, &ctx("activo"));

            // Más de 256 colores.
            let b = Bench::open_with(ppp, window, Mode::Rgba, rgba_of_257_colors(), Some(ACTIVE));
            let reason_2 = b.dialog().reason().expect("hay motivo");
            assert_eq!(
                reason_2,
                "más de 256 colores: reduce colores",
                "{}",
                ctx("257")
            );
            checks_footer(&b, &reason_2, &ctx("257"));
        }
    }
}

#[test]
fn row_8_20b_the_hint_is_in_the_text_and_not_only_in_the_color() {
    let mut b = Bench::open(2.0, Window::Large, Mode::Rgba);
    put_gray(&mut b, 0);
    let t = footer_of(&b).expect("pie");
    assert!(t.text_2.trim().starts_with("falta:"), "{:?}", t.text_2);
    b.mark("luma_gris", true);
    b.chooses("alfa_gris", 3);
    b.dialog_mut()
        .set_block(Some("no hay color activo".to_string()));
    b.convert = None;
    b.rest();
    let t = footer_of(&b).expect("pie");
    assert!(t.text_2.trim().starts_with("error:"), "{:?}", t.text_2);
}

// ---------------------------------------------------------------------------------------------
// Unidad 20b, parte 2: marcador redondo de las opciones y borde de la casilla
// (specs/hito-2-motivo-y-marcadores.md, D5; filas 9 a 11)
// ---------------------------------------------------------------------------------------------
//
// En ROJO hasta la implementación: `px_ui::dialog_view::TOGGLE_EDGE` todavía no existe, y
// `draw_toggle` pinta las opciones como cuadrados con `EDGE`.
//
// Se lee lo pintado en el último fotograma con las formas crudas (`Shape::Circle` y `Shape::Rect`),
// que es donde están el centro, el radio y el trazo de un círculo (el `Painted` de este banco solo
// guarda textos y rellenos). El banco no guarda las formas, así que `raw` ejecuta un fotograma
// más de reposo sin eventos (menú aparte: solo el diálogo y su `sync`), con el mismo estado.
//
// Lo que fija D5, medido en 960 × 720 pt (a la mínima el cuerpo se desplaza y parte de las filas
// queda recortada) y a las cinco escalas:
// - Opción: un aro (círculo con trazo `TOGGLE_EDGE` de 1 u y relleno `FIELD_FILL`, diámetro exterior
//   de 10 u = 2 · radio + trazo, centro en `x = PAD_U + 2 + 5 u` desde el borde del diálogo); la
//   elegida lleva además un punto `ACCENT` de 6 u, concéntrico; **un** punto por grupo.
// - Casilla: rectángulo de 10 u con trazo `TOGGLE_EDGE` de 1 u y relleno `FIELD_FILL`; marcada, un
//   cuadrado `ACCENT` de 6 u centrado en la caja. Ningún círculo en su fila ni caja en la de un aro.
// - Los grupos y las casillas que se esperan salen del propio diálogo (`is_shown`, `chosen`,
//   `checked`), en el orden del formulario, que es el vertical.
// - Contraste de aro, punto, borde y marca frente a `FIELD_FILL` y `DIALOG_FILL`: ≥ 3:1.
// - Anillo de foco: trazo `ACCENT` de 2 u (el de siempre) alrededor de la fila entera (`x = PAD_U`,
//   `y = rótulo − 1 u`, 16 u de alto, `18 u + 7 u · letters` de ancho), el mismo en opción y casilla.

use px_ui::dialog_view::{DIALOG_FILL, FIELD_FILL, TOGGLE_EDGE};

const ACCENT: Color32 = px_ui::theme::ACCENT;
const MIN_MARK_20B: f64 = 3.0;

#[derive(Clone, Copy, Debug)]
struct Disk {
    center: Pos2,
    radius: f32,
    fill: Color32,
    /// `TRANSPARENT` y `0.0` si no tiene trazo visible.
    stroke_color: Color32,
    stroke_width: f32,
    order: usize,
}

#[derive(Clone, Copy, Debug)]
struct Toolbox {
    rect: Rect,
    fill: Color32,
    stroke_color: Color32,
    stroke_width: f32,
}

#[derive(Debug, Default)]
struct Marks {
    disks: Vec<Disk>,
    boxes: Vec<Toolbox>,
}

fn stroke_visible(s: eframe::egui::Stroke) -> (Color32, f32) {
    if s.width > 0.0 && s.color != Color32::TRANSPARENT {
        (s.color, s.width)
    } else {
        (Color32::TRANSPARENT, 0.0)
    }
}

fn collects_marks(shape: &Shape, outside: &mut Marks, order: &mut usize) {
    match shape {
        Shape::Circle(c) => {
            let (stroke_color, stroke_width) = stroke_visible(c.stroke);
            outside.disks.push(Disk {
                center: c.center,
                radius: c.radius,
                fill: c.fill,
                stroke_color,
                stroke_width,
                order: *order,
            });
            *order += 1;
        }
        Shape::Rect(r) => {
            let (stroke_color, stroke_width) = stroke_visible(r.stroke);
            outside.boxes.push(Toolbox {
                rect: r.rect,
                fill: r.fill,
                stroke_color,
                stroke_width,
            });
            *order += 1;
        }
        Shape::Vec(shapes) => {
            for shape_2 in shapes {
                collects_marks(shape_2, outside, order);
            }
        }
        _ => {}
    }
}

fn marks_of(shapes: &[ClippedShape]) -> Marks {
    let mut outside = Marks::default();
    let mut order = 0;
    for clipped in shapes {
        collects_marks(&clipped.shape, &mut outside, &mut order);
    }
    outside
}

fn near_u(a: f32, b: f32, u: f32) -> bool {
    (a - b).abs() <= 0.05 * u
}

fn same_center(a: Pos2, b: Pos2, u: f32) -> bool {
    near_u(a.x, b.x, u) && near_u(a.y, b.y, u)
}

/// Los aros (círculos con trazo `TOGGLE_EDGE`), de arriba abajo.
fn rings(m: &Marks) -> Vec<Disk> {
    let mut v: Vec<Disk> = m
        .disks
        .iter()
        .copied()
        .filter(|d| d.stroke_color == TOGGLE_EDGE)
        .collect();
    v.sort_by(|a, b| a.center.y.total_cmp(&b.center.y));
    v
}

/// Los puntos (círculos rellenos de `ACCENT`), de arriba abajo.
fn points(m: &Marks) -> Vec<Disk> {
    let mut v: Vec<Disk> = m
        .disks
        .iter()
        .copied()
        .filter(|d| d.fill == ACCENT)
        .collect();
    v.sort_by(|a, b| a.center.y.total_cmp(&b.center.y));
    v
}

/// Las cajas de casilla: rectángulos con trazo `TOGGLE_EDGE` de hasta 20 u de lado (`TOGGLE_EDGE`
/// es `TEXT_DIM` y otros trazos grandes, como el marco del diálogo, pueden tener ese color).
fn boxes_2(m: &Marks, u: f32) -> Vec<Toolbox> {
    let mut v: Vec<Toolbox> = m
        .boxes
        .iter()
        .copied()
        .filter(|c| c.stroke_color == TOGGLE_EDGE)
        .filter(|c| c.rect.width() <= 20.0 * u && c.rect.height() <= 20.0 * u)
        .collect();
    v.sort_by(|a, b| a.rect.center().y.total_cmp(&b.rect.center().y));
    v
}

/// Las marcas cuadradas: rellenos `ACCENT` sin trazo de hasta 10 u con el centro a menos de 5 u en
/// vertical de `y`.
fn marks_square(m: &Marks, u: f32, y: f32) -> Vec<Toolbox> {
    m.boxes
        .iter()
        .copied()
        .filter(|c| c.fill == ACCENT && c.stroke_width == 0.0)
        .filter(|c| c.rect.width() <= 10.0 * u + 0.05 && c.rect.height() <= 10.0 * u + 0.05)
        .filter(|c| (c.rect.center().y - y).abs() <= 5.0 * u)
        .collect()
}

/// El relleno de un aro: el suyo o, si no tiene, el del último círculo relleno anterior y concéntrico.
fn fill_of_ring(m: &Marks, ring: &Disk) -> Color32 {
    if ring.fill != Color32::TRANSPARENT {
        return ring.fill;
    }
    m.disks
        .iter()
        .filter(|d| d.order < ring.order && d.fill != Color32::TRANSPARENT)
        .rfind(|d| (d.center - ring.center).length() < 0.01)
        .map_or(Color32::TRANSPARENT, |d| d.fill)
}

/// Lo que se espera de los marcadores según el estado del diálogo, comparado con lo pintado en
/// `shapes` (ver la cabecera de la sección). Devuelve lo leído, para seguir con ello.
fn asserts_markers(
    dialog: &Dialog,
    shapes: &[ClippedShape],
    ppp: f32,
    dialog_x_px: i64,
    location: &str,
) -> Marks {
    let n = i64::from(text_scale(ppp));
    let u = n as f32 / ppp;
    let x = (dialog_x_px + 15 * n) as f32 / ppp;
    let m = marks_of(shapes);
    let rings = rings(&m);
    let points = points(&m);
    let boxes = boxes_2(&m, u);

    // Lo que hay que ver: grupos (ids, opciones, elegida) y casillas (marcada), en orden.
    let mut groups: Vec<(&str, usize, usize)> = Vec::new();
    let mut checks: Vec<bool> = Vec::new();
    for f in &dialog.spec().fields {
        if !dialog.is_shown(f.id) {
            continue;
        }
        match &f.kind {
            px_ui::dialog::FieldKind::Choice { options, .. } => {
                groups.push((f.id, options.len(), dialog.chosen(f.id).expect("grupo")));
            }
            px_ui::dialog::FieldKind::Check { .. } => {
                checks.push(dialog.checked(f.id).expect("casilla"));
            }
            _ => {}
        }
    }

    // Opciones: un aro por opción, en su sitio y con las medidas de D5.
    let total: usize = groups.iter().map(|g| g.1).sum();
    assert_eq!(
        rings.len(),
        total,
        "{location}: un aro por opción de los grupos {groups:?}; círculos pintados {:?}",
        m.disks
    );
    let mut first_2 = 0;
    for &(id, options_2, chosen) in &groups {
        for i in 0..options_2 {
            let ring = rings[first_2 + i];
            assert!(
                near_u(ring.center.x, x, u),
                "{location}: el aro {id}/{i} está en x = {} y debía estar en {x}",
                ring.center.x
            );
            assert!(
                near_u(2.0 * ring.radius + ring.stroke_width, 10.0 * u, u),
                "{location}: el aro {id}/{i} mide {} de diámetro exterior y debía medir {}",
                2.0 * ring.radius + ring.stroke_width,
                10.0 * u
            );
            assert!(
                near_u(ring.stroke_width, u, u),
                "{location}: el trazo del aro {id}/{i} mide {} y debía medir {u}",
                ring.stroke_width
            );
            assert_eq!(
                fill_of_ring(&m, &ring),
                FIELD_FILL,
                "{location}: el aro {id}/{i} se rellena con el color del campo"
            );
            let here: Vec<&Disk> = points
                .iter()
                .filter(|p| same_center(p.center, ring.center, u))
                .collect();
            assert_eq!(
                here.len(),
                usize::from(i == chosen),
                "{location}: la opción {id}/{i} (elegida: {chosen}) lleva {} puntos",
                here.len()
            );
            if let Some(point) = here.first() {
                assert!(
                    near_u(2.0 * point.radius, 6.0 * u, u),
                    "{location}: el punto de {id} mide {} y debía medir {}",
                    2.0 * point.radius,
                    6.0 * u
                );
                assert_eq!(
                    point.stroke_width, 0.0,
                    "{location}: el punto no lleva trazo"
                );
            }
        }
        first_2 += options_2;
    }
    assert_eq!(
        points.len(),
        groups.len(),
        "{location}: exactamente un punto por grupo ({groups:?}): {points:?}"
    );

    // Casillas: una caja de 10 u por casilla, con su marca si está marcada.
    assert_eq!(
        boxes.len(),
        checks.len(),
        "{location}: una caja con borde TOGGLE_EDGE por casilla; rectángulos de caja {:?}",
        m.boxes
            .iter()
            .filter(|c| c.rect.width() <= 20.0 * u)
            .collect::<Vec<_>>()
    );
    for (toolbox, marked) in boxes.iter().zip(&checks) {
        let center = toolbox.rect.center();
        assert!(
            near_u(toolbox.rect.width(), 10.0 * u, u) && near_u(toolbox.rect.height(), 10.0 * u, u),
            "{location}: la caja mide {:?} y debía ser un cuadrado de {}",
            toolbox.rect.size(),
            10.0 * u
        );
        assert!(
            near_u(center.x, x, u),
            "{location}: la caja está centrada en x = {}, no en {x}",
            center.x
        );
        assert!(
            near_u(toolbox.stroke_width, u, u),
            "{location}: el borde de la caja mide {} y debía medir {u}",
            toolbox.stroke_width
        );
        assert_eq!(toolbox.fill, FIELD_FILL, "{location}: relleno de la caja");
        let marks = marks_square(&m, u, center.y);
        assert_eq!(
            marks.len(),
            usize::from(*marked),
            "{location}: casilla marcada = {marked}; marcas {marks:?}"
        );
        if let Some(mark) = marks.first() {
            assert!(
                near_u(mark.rect.width(), 6.0 * u, u) && near_u(mark.rect.height(), 6.0 * u, u),
                "{location}: la marca mide {:?} y debía ser un cuadrado de {}",
                mark.rect.size(),
                6.0 * u
            );
            assert!(
                same_center(mark.rect.center(), center, u),
                "{location}: la marca está centrada en la caja"
            );
        }
        // Opción y casilla se distinguen por la forma: ningún círculo en la fila de la caja.
        assert!(
            m.disks
                .iter()
                .all(|d| (d.center.y - center.y).abs() > 3.0 * u),
            "{location}: hay un círculo en la fila de una casilla: {:?}",
            m.disks
        );
    }
    for ring in &rings {
        for toolbox in &boxes {
            assert!(
                (ring.center.y - toolbox.rect.center().y).abs() > 3.0 * u,
                "{location}: un aro y una caja comparten fila"
            );
        }
    }

    // Contraste de lo pintado frente al campo y al diálogo.
    let mut colors: Vec<(&str, Color32)> = Vec::new();
    colors.extend(rings.iter().map(|d| ("aro", d.stroke_color)));
    colors.extend(points.iter().map(|d| ("punto", d.fill)));
    colors.extend(boxes.iter().map(|c| ("borde de casilla", c.stroke_color)));
    for toolbox in &boxes {
        colors.extend(
            marks_square(&m, u, toolbox.rect.center().y)
                .iter()
                .map(|c| ("marca de casilla", c.fill)),
        );
    }
    for (name_2, color) in colors {
        for background_2 in [FIELD_FILL, DIALOG_FILL] {
            let ratio = contrast_ratio(color, background_2);
            assert!(
                ratio >= MIN_MARK_20B,
                "{location}: {name_2} {color:?} sobre {background_2:?} da {ratio:.2} (< {MIN_MARK_20B})"
            );
        }
    }
    m
}

/// El anillo de foco de la fila de `label` con el foco en `goal`: tras comprobar que es el de
/// siempre (un trazo `ACCENT` de 2 u que rodea la fila entera, con el marcador dentro y con 3:1
/// sobre el campo y el diálogo), devuelve sus medidas en u: `x` desde `PAD_U`, `y` desde el rótulo,
/// alto, ancho sin las letras y trazo.
fn asserts_ring(
    text_2: &Painted,
    shapes: &[ClippedShape],
    ppp: f32,
    dialog_x_px: i64,
    goal: FocusTarget,
    label: &str,
    location: &str,
) -> [f32; 5] {
    let n = i64::from(text_scale(ppp));
    let u = n as f32 / ppp;
    let m = marks_of(shapes);
    let rings_2: Vec<Toolbox> = m
        .boxes
        .iter()
        .copied()
        .filter(|c| {
            c.stroke_color == ACCENT
                && near_u(c.stroke_width, 2.0 * u, u)
                && c.fill == Color32::TRANSPARENT
        })
        .collect();
    assert_eq!(
        rings_2.len(),
        1,
        "{location}: el foco en {goal:?} pinta {} anillos ACCENT de 2 u: {rings_2:?}",
        rings_2.len()
    );
    let ring_2 = rings_2[0];
    let label_2 = text_2
        .exact(label)
        .unwrap_or_else(|| panic!("{location}: no se pinta {label:?}; hay {:?}", text_2.list()));
    let letters = label.chars().count() as f32;
    let x0 = (dialog_x_px + 8 * n) as f32 / ppp;
    assert!(
        near_u(ring_2.rect.min.x, x0, u)
            && near_u(ring_2.rect.min.y, label_2.pos.y - u, u)
            && near_u(ring_2.rect.height(), 16.0 * u, u)
            && near_u(ring_2.rect.width(), (18.0 + 7.0 * letters) * u, u),
        "{location}: el anillo de {goal:?} es {:?} y debía rodear la fila entera de {label:?}",
        ring_2.rect
    );
    for background_2 in [FIELD_FILL, DIALOG_FILL] {
        assert!(
            contrast_ratio(ACCENT, background_2) >= MIN_MARK_20B,
            "{location}: anillo sobre {background_2:?}"
        );
    }
    let marker = Rect::from_center_size(
        pos2((dialog_x_px + 15 * n) as f32 / ppp, label_2.pos.y + 7.0 * u),
        Vec2::splat(10.0 * u),
    );
    assert!(
        ring_2.rect.contains_rect(marker.shrink(0.01)),
        "{location}: el marcador {marker:?} queda fuera del anillo {:?}",
        ring_2.rect
    );
    [
        (ring_2.rect.min.x - x0) / u,
        (ring_2.rect.min.y - label_2.pos.y) / u,
        ring_2.rect.height() / u,
        ring_2.rect.width() / u - 7.0 * letters,
        ring_2.stroke_width / u,
    ]
}

/// Medidas de anillo esperadas: `x` en `PAD_U` (0 desde `PAD_U`), `y` a 1 u sobre el rótulo, 16 u de
/// alto, 18 u de ancho más 7 u por letra y 2 u de trazo.
fn asserts_ring_measures(measures: &[[f32; 5]], location: &str) {
    for m in measures {
        assert!(
            m[0].abs() <= 0.05
                && (m[1] + 1.0).abs() <= 0.05
                && (m[2] - 16.0).abs() <= 0.05
                && (m[3] - 18.0).abs() <= 0.05
                && (m[4] - 2.0).abs() <= 0.05,
            "{location}: {m:?}"
        );
    }
    for m in &measures[1..] {
        for (a, b) in m.iter().zip(&measures[0]) {
            assert!(
                (a - b).abs() <= 0.05,
                "{location}: el anillo no es el mismo: {measures:?}"
            );
        }
    }
}

impl Bench {
    /// Un fotograma más de reposo sin eventos, solo con el diálogo y su `sync`, y las formas crudas.
    fn raw(&mut self) -> Vec<ClippedShape> {
        let size = screen(self.ppp, self.window);
        let Bench {
            ctx,
            modal,
            focus,
            convert,
            count,
            image,
            ..
        } = &mut *self;
        runs(ctx, size, vec![], |ui| {
            let c = ui.ctx().clone();
            apply_scale(&c);
            if let Modal::Open(dialog) = modal {
                let _ = show_dialog(&c, dialog, focus);
                if let Some(sync) = convert.as_mut() {
                    let history_2 = History {
                        limit: 64 << 20,
                        used: 0,
                    };
                    sync_convert(dialog, sync, image, 0, history_2, count);
                }
            }
        })
    }

    /// Reposo y las comprobaciones de marcadores sobre lo pintado.
    fn markers(&mut self, location: &str) -> Marks {
        self.rest();
        let shapes = self.raw();
        let x = self.layout().dialog.x;
        asserts_markers(self.dialog(), &shapes, self.ppp, x, location)
    }

    /// La casilla que se ve, si hay.
    fn box_visible(&self) -> Option<&'static str> {
        let d = self.dialog();
        d.spec()
            .fields
            .iter()
            .find(|f| matches!(f.kind, px_ui::dialog::FieldKind::Check { .. }) && d.is_shown(f.id))
            .map(|f| f.id)
    }
}

#[test]
fn row_9_convert_mode_paints_rings_and_square_boxes_at_each_target_and_with_each_click() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        let targets_2 = targets_2(Mode::Rgba);
        b.markers(&format!("ppp {ppp} al abrir"));
        for (k, target_2) in targets_2.into_iter().enumerate() {
            // Clic real en el aro del destino (el primer grupo, de arriba abajo).
            let shapes = b.raw();
            let center = rings(&marks_of(&shapes))[k].center;
            b.click(center);
            b.rest();
            assert_eq!(
                b.dialog().chosen(FIELD_TARGET),
                Some(k),
                "ppp {ppp}: clic en el aro del destino {k}"
            );
            let location = format!("ppp {ppp} destino {target_2:?}");
            b.markers(&location);

            // La casilla, si la hay, con clic real en su caja: marcada, marca cuadrada, y desmarcada.
            if let Some(box_value) = b.box_visible() {
                let u = b.u();
                for expected_2 in [true, false] {
                    let shapes = b.raw();
                    let toolbox = boxes_2(&marks_of(&shapes), u)[0];
                    b.click(toolbox.rect.center());
                    b.rest();
                    assert_eq!(
                        b.dialog().checked(box_value),
                        Some(expected_2),
                        "{location}: clic en la caja de {box_value}"
                    );
                    b.markers(&format!("{location} casilla {expected_2}"));
                }
            }

            // El alfa (con destino Gris): un clic real en el aro de cada opción.
            if target_2 == Mode::Gray {
                for j in [4usize, 1, 0, 3, 2] {
                    let shapes = b.raw();
                    let center = rings(&marks_of(&shapes))[3 + j].center;
                    b.click(center);
                    b.rest();
                    assert_eq!(
                        b.dialog().chosen(FIELD_ALPHA),
                        Some(j),
                        "{location}: clic en el aro {j} del alfa"
                    );
                    b.markers(&format!("{location} alfa {j}"));
                }
            }
        }
    }
}

#[test]
fn row_11_convert_mode_the_focus_ring_is_the_same_on_option_and_on_box() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        b.chooses_target(Mode::Gray);
        let location = format!("ppp {ppp}");
        let targets_2_2 = [
            (FocusTarget::Option(FIELD_TARGET, 0), "Indexado"),
            (FocusTarget::Option(FIELD_TARGET, 1), "Gris con alfa"),
            (FocusTarget::Option(FIELD_TARGET, 2), "Gris"),
            (FocusTarget::Field(FIELD_LUMA_GRAY), BOX),
            (FocusTarget::Option(FIELD_ALPHA, 0), OPTIONS_OF_THE_ALPHA[0]),
            (FocusTarget::Option(FIELD_ALPHA, 4), OPTIONS_OF_THE_ALPHA[4]),
        ];
        let mut measures = Vec::new();
        for (goal, label) in targets_2_2 {
            b.focus_until(goal);
            let shapes = b.raw();
            let x = b.layout().dialog.x;
            measures.push(asserts_ring(
                &b.last_2_2,
                &shapes,
                ppp,
                x,
                goal,
                label,
                &location,
            ));
        }
        asserts_ring_measures(&measures, &location);
        // Enfocar no cambia el marcador.
        b.markers(&location);
    }
}

#[test]
fn row_11_convert_mode_tab_and_space_choose_and_toggle_as_today_with_the_new_marker() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, Mode::Rgba);
        for k in [1usize, 0, 2] {
            b.focus_until(FocusTarget::Option(FIELD_TARGET, k));
            b.presses(Key::Space);
            assert_eq!(
                b.dialog().chosen(FIELD_TARGET),
                Some(k),
                "ppp {ppp}: Espacio en el destino {k}"
            );
            b.markers(&format!("ppp {ppp} Espacio en el destino {k}"));
        }
        // Con destino Gris: la casilla y una opción del alfa.
        for expected_2 in [true, false] {
            b.focus_until(FocusTarget::Field(FIELD_LUMA_GRAY));
            b.presses(Key::Space);
            assert_eq!(
                b.dialog().checked(FIELD_LUMA_GRAY),
                Some(expected_2),
                "ppp {ppp}: Espacio en la casilla"
            );
            b.markers(&format!("ppp {ppp} Espacio en la casilla {expected_2}"));
        }
        b.focus_until(FocusTarget::Option(FIELD_ALPHA, 3));
        b.presses(Key::Space);
        assert_eq!(b.dialog().chosen(FIELD_ALPHA), Some(3), "ppp {ppp}");
        b.markers(&format!("ppp {ppp} Espacio en el alfa 3"));
        assert!(
            b.accepted.is_empty() && b.cancelled == 0,
            "{}",
            b.describe()
        );
    }
}
