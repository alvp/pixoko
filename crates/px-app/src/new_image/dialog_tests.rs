//! Tests de `new_image` con el diálogo (specs/hito-2-nueva-imagen.md, unidad 19): la fila 2 en su
//! parte de diálogo (`open_new_image` y `Dialog::set_limit_note`) y la fila 11, el banco de egui del
//! formulario real. En ROJO hasta la implementación.
//!
//! El banco reproduce el orden de fotograma de `App::ui` (como `px-ui/tests/dialog_open_focus.rs`):
//! primero el menú (`show_menu`), luego las acciones del fotograma (`NewImage` abre el `Modal` con
//! `open_new_image` y los `Locks` del fotograma) y, el último, `show_dialog` seguido de `sync_form`
//! (D8). Sin ventana ni capturas: `Context::run_ui` con `RawInput` sintético. El diálogo se abre con
//! un clic real en `Archivo → New image_2…`.
//!
//! - Fila 2 (diálogo): máximo, rango visible, motivo en línea y del pie con la nota de la tarjeta
//!   gráfica, `Crear` atenuado con el motivo visible.
//! - Fila 11: foco inicial en el ancho desde el fotograma siguiente al del clic; teclear `0`, `8193`,
//!   vacío y `500` (y `5000` con `max_side` 4096); `−` y `+`; cambiar de modo y elegir «Color…»
//!   (`#12`, `zz`, `#ff0000`, `#80808080`); cambiar de modo con un fondo que no cabe (aviso en el
//!   informe hasta el siguiente cambio); `Intro`, `Crear`, `Cancel`, `Escape`; `Tab`; el desplazamiento
//!   del cuerpo a la ventana mínima (280 u en indexado, 320 u con «Color…») y que a 960 × 720 no se
//!   desplaza; contraste ≥ 4,5 en todo texto del diálogo.
//!
//! Decisiones donde la spec no fija un detalle:
//! - Las cinco escalas (`PPPS`) y las dos ventanas (468n × 333n px y 960 × 720 pt) se recorren en todas
//!   las pruebas, **salvo** las que necesitan ver todo el formulario a la vez (qué grupo de fondo se
//!   pinta, el informe completo, los clics en las opciones de modo y de fondo): esas usan 960 × 720,
//!   porque a la mínima el cuerpo se desplaza y parte de lo que hay que clicar queda recortada. A la
//!   mínima se prueban el título, el pie, los botones, los errores del ancho, el desplazamiento y `Tab`.
//! - Se identifica cada pieza por su texto pintado visible (un texto cuyo rectángulo corta su recorte).
//!   El error en línea es `ERROR_PREFIX` + motivo y el del pie `ERROR_PREFIX` + `reason()`; el rango es
//!   un texto que contiene `1-8192` (`1-4096`…); `−` puede ser `−` o `-`.
//! - Se escribe con `Ctrl+A`, doce `Retroceso` y `Event::Text`, con el foco que tenga el campo.
//! - `Intro` en un hexadecimal confirma el texto (regla de la 18); los fondos de partida de los avisos
//!   se dejan con eventos directos sobre el `Dialog` (la preparación no es lo que se prueba).
//! - Una muesca de la rueda es de 42 u en el cuerpo y se mide con el puntero en el relleno izquierdo del
//!   cuerpo (`x = 3 u` desde el borde), como en `dialog_view.rs`.
//! - Para el contraste solo se miran los textos dentro del rectángulo del diálogo (el menú de detrás es
//!   de otra unidad); el relleno bajo un texto es el último opaco anterior que contiene su recorte.

use super::*;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{
    Color32, Context, Event as InputEvent, Id, Key, MissingGlyphPolicy, Modifiers,
    MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, Vec2, pos2,
    vec2,
};
use px_core::Mode;
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::color_edit::{HexError, hex_notice};
use px_ui::dialog::{Dialog, DialogKind, Event, FieldKind, Modal, Reaction, Validity, Values};
use px_ui::dialog_keys::{FocusTarget, focus_order};
use px_ui::dialog_layout::{DialogLayout, dialog_layout};
use px_ui::dialog_view::{ERROR_PREFIX, show_dialog};
use px_ui::menu::show_menu;
use px_ui::text::{INTERFACE_CHARSET, apply_scale, install_fonts, text_scale};
use px_ui::theme::{TEXT_DISABLED, contrast_ratio, visuals};
use px_ui::tool::Tool;

// ---------------------------------------------------------------------------------------------
// Fila 2: el diálogo que abre `open_new_image`
// ---------------------------------------------------------------------------------------------

fn maximum_expected(max_side: usize) -> u32 {
    u32::try_from(max_side.clamp(1, 8192)).expect("cabe")
}

fn open(max_side: usize) -> Modal {
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, max_side, Locks::default()));
    modal
}

fn dialog_of(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn dialog_of_mut(modal: &mut Modal) -> &mut Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn number(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Number {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn reason_of(d: &Dialog, field_2: &'static str) -> String {
    match d.validity(field_2) {
        Some(Validity::Invalid(reason_2)) => reason_2,
        other_2 => panic!("{field_2} debía ser inválido y es {other_2:?}"),
    }
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

#[test]
fn row_2_open_new_image_opens_the_new_image_dialog() {
    let modal = open(16384);
    assert_eq!(modal.kind(), Some(DialogKind::NewImage));
    let d = dialog_of(&modal);
    assert_eq!(d.spec().title, "Nueva imagen");
    assert!(d.is_valid());
}

#[test]
fn row_2_open_new_image_returns_false_with_a_lock_or_with_another_dialog_open() {
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            dialog: true,
            ..Locks::default()
        },
        Locks {
            text_focus: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
    ] {
        let mut modal = Modal::default();
        assert!(!open_new_image(&mut modal, 4096, locks), "{locks:?}");
        assert!(!modal.is_open(), "{locks:?}");
    }
    let mut modal = open(4096);
    let before = modal.clone();
    assert!(!open_new_image(&mut modal, 100, Locks::default()));
    assert_eq!(modal, before, "no cambia el diálogo abierto");
}

#[test]
fn row_2_with_the_gpu_below_8192_the_maximum_and_the_reason_say_graphics_card() {
    for max_side in [8191usize, 4096, 2048, 100, 1] {
        let max = maximum_expected(max_side);
        for (field_2, label_2) in [("ancho", "Ancho"), ("alto", "Alto")] {
            let mut modal = open(max_side);
            let d = dialog_of_mut(&mut modal);
            // Máximo del campo.
            let FieldKind::Number { max: cap, .. } = d
                .spec()
                .fields
                .iter()
                .find(|f| f.id == field_2)
                .expect("campo")
                .kind
            else {
                panic!("{field_2} no es numérico");
            };
            assert_eq!(cap, max, "max_side {max_side} {field_2}");

            // `max` vale y `max + 1` da el texto con la nota.
            number(d, field_2, &max.to_string());
            assert_eq!(
                d.validity(field_2),
                Some(Validity::Valid),
                "{max_side} {field_2}"
            );
            assert!(d.is_valid());
            number(d, field_2, &(max + 1).to_string());
            let reason_2 = format!("máximo {max} (tarjeta gráfica)");
            assert_eq!(reason_of(d, field_2), reason_2, "{max_side} {field_2}");
            // El motivo del pie: `Width: máximo 4096 (tarjeta gráfica)` (36 caracteres con 4 cifras).
            let footer = format!("{label_2}: {reason_2}");
            assert_eq!(d.reason(), Some(footer.clone()), "{max_side} {field_2}");
            assert!(footer.chars().count() <= 37, "{footer}");
            if (1000..10000).contains(&max) {
                // 36 caracteres con «Ancho:» y 35 con «Alto:» (la nota y `máximo NNNN` miden 29).
                assert_eq!(
                    footer.chars().count(),
                    label_2.chars().count() + 2 + 29,
                    "{footer}"
                );
            }
            // Cabe en la línea de 44 columnas con `error: ` y está dentro del juego.
            let line = format!("{ERROR_PREFIX}{reason_2}");
            assert!(line.chars().count() <= 44, "{line}");
            assert!(in_the_set(&line), "{line}");
            assert!(in_the_set(&footer), "{footer}");
            assert!(!d.is_valid(), "Crear queda atenuado");
        }
    }
}

#[test]
fn row_2_four_thousand_ninety_six_says_exactly_error_maximum_4096_graphics_card() {
    let mut modal = open(4096);
    let d = dialog_of_mut(&mut modal);
    number(d, "ancho", "4097");
    assert_eq!(
        format!("{ERROR_PREFIX}{}", reason_of(d, "ancho")),
        "error: máximo 4096 (tarjeta gráfica)"
    );
    assert_eq!(
        d.reason(),
        Some("Ancho: máximo 4096 (tarjeta gráfica)".to_string())
    );
    assert_eq!("Ancho: máximo 4096 (tarjeta gráfica)".chars().count(), 36);
}

#[test]
fn row_2_the_values_5000_zero_and_eleven_digits_with_the_limited_gpu() {
    for max_side in [8191usize, 4096, 2048, 100, 1] {
        let max = maximum_expected(max_side);
        let mut modal = open(max_side);
        let d = dialog_of_mut(&mut modal);
        number(d, "ancho", "5000");
        if max >= 5000 {
            assert_eq!(d.validity("ancho"), Some(Validity::Valid), "{max_side}");
        } else {
            assert_eq!(
                reason_of(d, "ancho"),
                format!("máximo {max} (tarjeta gráfica)"),
                "{max_side}"
            );
        }
        // `0` y las once cifras dicen lo de siempre, sin nota.
        number(d, "ancho", "0");
        assert_eq!(reason_of(d, "ancho"), "mínimo 1", "{max_side}");
        number(d, "ancho", "99999999999");
        assert_eq!(reason_of(d, "ancho"), "máximo 10 cifras", "{max_side}");
        number(d, "ancho", "abc");
        assert_eq!(reason_of(d, "ancho"), "solo dígitos 0-9", "{max_side}");
        number(d, "ancho", "");
        assert_eq!(reason_of(d, "ancho"), "escribe un número", "{max_side}");
    }
}

#[test]
fn row_2_with_a_texture_side_of_8192_or_more_the_text_is_the_usual_one() {
    for max_side in [8192usize, 16384, 32768] {
        for field_2 in ["ancho", "alto"] {
            let mut modal = open(max_side);
            let d = dialog_of_mut(&mut modal);
            number(d, field_2, "8193");
            assert_eq!(reason_of(d, field_2), "máximo 8192", "{max_side} {field_2}");
            number(d, field_2, "8192");
            assert_eq!(d.validity(field_2), Some(Validity::Valid));
        }
    }
}

#[test]
fn row_2_set_limit_note_on_the_real_form_only_applies_to_the_numeric_ones() {
    let mut modal = open(16384);
    let d = dialog_of_mut(&mut modal);
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert!(d.set_limit_note("alto", Some("tarjeta gráfica")));
    for id in [
        "modo",
        "fondo_indexado",
        "fondo_rgba",
        "color_rgba",
        "fondo_gris_alfa",
        "color_gris_alfa",
        "fondo_gris",
        "color_gris",
        "informe",
        "inexistente",
    ] {
        assert!(!d.set_limit_note(id, Some("tarjeta gráfica")), "{id}");
    }
    number(d, "ancho", "8193");
    assert_eq!(reason_of(d, "ancho"), "máximo 8192 (tarjeta gráfica)");
    assert!(d.set_limit_note("ancho", None));
    assert_eq!(reason_of(d, "ancho"), "máximo 8192");
}

// ---------------------------------------------------------------------------------------------
// Fila 11: ayudantes del banco
// ---------------------------------------------------------------------------------------------

/// Las cinco escalas de pantalla de la spec.
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// La ventana mínima real de la escala (`468n × 333n` px) o 960 × 720 pt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    Minimum,
    Large,
}

const WINDOWS: [Window; 2] = [Window::Minimum, Window::Large];

fn n_de(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn screen(ppp: f32, window: Window) -> Vec2 {
    match window {
        Window::Minimum => {
            let n = n_de(ppp) as f32;
            vec2(468.0 * n / ppp, 333.0 * n / ppp)
        }
        Window::Large => vec2(960.0, 720.0),
    }
}

/// El tamaño de la ventana en píxeles físicos.
fn pixels_2(ppp: f32, window: Window) -> (i64, i64) {
    match window {
        Window::Minimum => (468 * n_de(ppp), 333 * n_de(ppp)),
        Window::Large => (
            (960.0 * f64::from(ppp)).round() as i64,
            (720.0 * f64::from(ppp)).round() as i64,
        ),
    }
}

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

    /// Los textos visibles que, recortados, son `-` o `−`, de arriba abajo.
    fn less(&self) -> Vec<Rect> {
        let mut v: Vec<Rect> = self
            .visible()
            .filter(|t| matches!(t.text_2.trim(), "−" | "-"))
            .map(Text::rect)
            .collect();
        v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        v
    }

    fn more(&self) -> Vec<Rect> {
        let mut v: Vec<Rect> = self
            .visible()
            .filter(|t| t.text_2.trim() == "+")
            .map(Text::rect)
            .collect();
        v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        v
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

struct Bench {
    ctx: Context,
    ppp: f32,
    window: Window,
    max_side: usize,
    modal: Modal,
    focus: Option<FocusTarget>,
    sync: FormSync,
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
    fn new_2(ppp: f32, window: Window, max_side: usize) -> Bench {
        let ctx = context(ppp);
        ctx.set_visuals_of(eframe::egui::Theme::Dark, visuals(ppp));
        Bench {
            ctx,
            ppp,
            window,
            max_side,
            modal: Modal::Closed,
            focus: None,
            sync: FormSync::default(),
            accepted: Vec::new(),
            cancelled: 0,
            trace: Vec::new(),
            open_in: None,
            last_2_2: Painted::default(),
        }
    }

    /// Con el diálogo abierto por un clic en `Archivo → New image_2…` y unos fotogramas de reposo.
    fn open(ppp: f32, window: Window, max_side: usize) -> Bench {
        let mut b = Bench::new_2(ppp, window, max_side);
        b.opens_with_the_mouse();
        assert!(b.modal.is_open(), "ppp {ppp} {window:?}: no se abrió");
        b
    }

    fn dialog(&self) -> &Dialog {
        dialog_of(&self.modal)
    }

    fn dialog_mut(&mut self) -> &mut Dialog {
        dialog_of_mut(&mut self.modal)
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
                sync,
                max_side,
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
                    if action == AppAction::NewImage && open_new_image(modal, *max_side, locks) {
                        *focus = None;
                        *sync = FormSync::default();
                    }
                }
                // El diálogo, el último, y `sync_form` tras él (D8).
                if let Modal::Open(dialog) = modal {
                    paint_the_dialog = true;
                    reaction = show_dialog(&c, dialog, focus);
                    sync_form(dialog, sync);
                }
            })
        };
        match reaction {
            Reaction::Accepted(values) => {
                self.accepted.push(values);
                self.modal.abandon();
                self.focus = None;
            }
            Reaction::Cancelled => {
                self.cancelled += 1;
                self.modal.abandon();
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

    fn presses(&mut self, key: Key) {
        self.frame(vec![key_2(key, Modifiers::NONE)]);
        self.rest();
    }

    /// El centro del texto visible `searched`: el exacto y, si no hay, el primero que lo contiene
    /// (como `dialog_open_focus.rs`, que busca las entradas del menú por contenido).
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

    /// `Archivo → New image_2…` con el ratón, como lo hace el usuario.
    fn opens_with_the_mouse(&mut self) {
        self.rest();
        self.click_in("Archivo");
        assert!(
            self.last_2_2.contains("Ctrl+N"),
            "el menú muestra el atajo; hay {:?}",
            self.last_2_2.list()
        );
        let input = self.center_of("Nueva imagen…");
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

    /// El `Id` de egui del campo `id` (el que se enfoca al darle el foco).
    fn id_de(field_2: &'static str) -> Id {
        Id::unique(("pixoko-dialog", field_2))
    }

    fn layout(&self) -> DialogLayout {
        let d = self.dialog();
        dialog_layout(
            pixels_2(self.ppp, self.window),
            n_de(self.ppp) as u32,
            d.content_h_u(),
            d.is_fill(),
            d.spec().cancel.is_some(),
        )
    }

    fn u(&self) -> f32 {
        n_de(self.ppp) as f32 / self.ppp
    }

    fn create_disabled(&self) -> bool {
        self.last_2_2
            .exact("Crear")
            .unwrap_or_else(|| panic!("no se pinta Crear; hay {:?}", self.last_2_2.list()))
            .disabled()
    }

    fn describe(&self) -> String {
        format!(
            "ppp {} {:?}; aceptados {}; cancelados {}; foco {:?}; pintado {:?}",
            self.ppp,
            self.window,
            self.accepted.len(),
            self.cancelled,
            self.focus,
            self.last_2_2.list()
        )
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

fn text_of_the_report(d: &Dialog) -> String {
    d.spec()
        .fields
        .iter()
        .find_map(|f| match &f.kind {
            FieldKind::ReadOnly { text, .. } if f.id == "informe" => Some(text.clone()),
            _ => None,
        })
        .expect("el formulario tiene el informe")
}

fn spec_of_the(b: &Bench) -> Option<NewImageSpec> {
    spec_from_values(&b.dialog().values()?)
}

const NOTICES: [&str; 5] = [
    "Gris sin Transparente: pasa a Blanco",
    "Indexado sin Color: pasa a Transparente",
    "Color con alfa: pasa a Blanco",
    "Color no gris: pasa a Transparente",
    "Color no gris: pasa a Blanco",
];

fn there_is_a_background_notice(b: &Bench) -> bool {
    NOTICES.iter().any(|a| b.last_2_2.contains(a))
}

// ---------------------------------------------------------------------------------------------
// Fila 11: abrir
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_opened_with_a_click_on_the_menu_the_initial_focus_is_the_width_from_the_next_frame() {
    for ppp in PPPS {
        for window in WINDOWS {
            let b = Bench::open(ppp, window, 16384);
            assert_eq!(b.modal.kind(), Some(DialogKind::NewImage));
            assert!(b.open_in.is_some(), "ppp {ppp} {window:?}: nunca se pintó");
            let focus = focus_after_opening(&b);
            assert!(
                focus.len() >= 3,
                "ppp {ppp} {window:?}: faltan fotogramas tras abrir"
            );
            for (frame, id) in focus {
                assert_eq!(
                    id,
                    Some(Bench::id_de("ancho")),
                    "ppp {ppp} {window:?}: el foco de egui en el fotograma {frame} \
                     (se abrió en {:?}) no es el ancho; {}",
                    b.open_in,
                    b.describe()
                );
            }
            assert_eq!(
                b.focus,
                Some(FocusTarget::Field("ancho")),
                "ppp {ppp} {window:?}"
            );
            assert_eq!(b.dialog().text("ancho"), Some("64".to_string()));
            assert_eq!(b.dialog().text("alto"), Some("64".to_string()));
        }
    }
}

#[test]
fn row_11_the_dialog_paints_title_footer_and_buttons_at_both_windows() {
    for ppp in PPPS {
        for window in WINDOWS {
            let b = Bench::open(ppp, window, 16384);
            for text_2 in ["Nueva imagen", "Crear", "Cancelar", "Ancho", "Alto", "Modo"] {
                assert!(
                    b.last_2_2.exact(text_2).is_some(),
                    "ppp {ppp} {window:?}: falta {text_2:?}; hay {:?}",
                    b.last_2_2.list()
                );
            }
            assert!(!b.create_disabled(), "ppp {ppp} {window:?}: todo es válido");
        }
    }
}

#[test]
fn row_11_at_960_by_720_the_whole_indexed_form_is_painted_with_its_report_and_the_range() {
    for ppp in PPPS {
        let b = Bench::open(ppp, Window::Large, 16384);
        for text_2 in [
            "Indexado",
            "RGBA",
            "Gris con alfa",
            "Gris",
            "Fondo",
            "Transparente",
            "Blanco",
            "Negro",
        ] {
            assert!(
                b.last_2_2.exact(text_2).is_some(),
                "ppp {ppp}: falta {text_2:?}; hay {:?}",
                b.last_2_2.list()
            );
        }
        // Indexado no ofrece «Color…» ni el hexadecimal.
        assert!(b.last_2_2.exact("Color…").is_none(), "ppp {ppp}");
        assert!(b.last_2_2.exact("Color").is_none(), "ppp {ppp}");
        // El informe y el rango visible.
        assert!(b.last_2_2.contains("Memoria: menos de 1 MiB"), "ppp {ppp}");
        assert!(
            b.last_2_2.contains("Paleta: 32 colores, el 0 transparente"),
            "ppp {ppp}"
        );
        assert!(b.last_2_2.contains("1-8192"), "ppp {ppp}: el rango visible");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11 (y fila 2): teclear en el ancho
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_typing_0_8193_empty_and_500_in_the_width_gives_inline_errors_and_create_dimmed() {
    for ppp in PPPS {
        for window in WINDOWS {
            for (text_2, reason_2) in [
                ("0", Some("mínimo 1")),
                ("8193", Some("máximo 8192")),
                ("", Some("escribe un número")),
                ("500", None),
            ] {
                let mut b = Bench::open(ppp, window, 16384);
                b.writes(text_2);
                let context = format!("ppp {ppp} {window:?} {text_2:?}");
                match reason_2 {
                    Some(reason_2) => {
                        assert_eq!(
                            b.dialog().validity("ancho"),
                            Some(Validity::Invalid(reason_2.to_string())),
                            "{context}"
                        );
                        // En línea: `error: ` y el motivo.
                        assert!(
                            b.last_2_2.contains(&format!("{ERROR_PREFIX}{reason_2}")),
                            "{context}: falta el error en línea; hay {:?}",
                            b.last_2_2.list()
                        );
                        // En el pie: `error: ` y `Width: reason_2`, con Crear atenuado.
                        assert!(
                            b.last_2_2
                                .contains(&format!("{ERROR_PREFIX}Ancho: {reason_2}")),
                            "{context}: falta el motivo del pie; hay {:?}",
                            b.last_2_2.list()
                        );
                        assert!(b.create_disabled(), "{context}: Crear atenuado");
                    }
                    None => {
                        assert_eq!(b.dialog().validity("ancho"), Some(Validity::Valid));
                        assert!(
                            !b.last_2_2.contains(ERROR_PREFIX),
                            "{context}: no hay errores; hay {:?}",
                            b.last_2_2.list()
                        );
                        assert!(!b.create_disabled(), "{context}");
                    }
                }
            }
        }
    }
}

#[test]
fn row_11_typing_in_the_height_gives_the_height_reason() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        b.focus_until(FocusTarget::Field("alto"));
        b.writes("8193");
        assert!(b.last_2_2.contains("error: máximo 8192"), "ppp {ppp}");
        assert!(b.last_2_2.contains("error: Alto: máximo 8192"), "ppp {ppp}");
        assert!(b.create_disabled(), "ppp {ppp}");
    }
}

#[test]
fn row_2_with_the_gpu_at_4096_the_range_and_the_error_say_graphics_card_in_the_bench() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 4096);
            let context = format!("ppp {ppp} {window:?}");
            assert!(
                b.last_2_2.contains("1-4096"),
                "{context}: el rango visible; hay {:?}",
                b.last_2_2.list()
            );
            b.writes("5000");
            assert!(
                b.last_2_2.contains("error: máximo 4096 (tarjeta gráfica)"),
                "{context}: error en línea; hay {:?}",
                b.last_2_2.list()
            );
            // El pie: `error: Width: máximo 4096 (tarjeta gráfica)` (43 caracteres, caben en 44).
            assert!(
                b.last_2_2
                    .contains("error: Ancho: máximo 4096 (tarjeta gráfica)"),
                "{context}: motivo del pie; hay {:?}",
                b.last_2_2.list()
            );
            assert!(
                b.create_disabled(),
                "{context}: Crear atenuado con el motivo visible"
            );

            b.writes("4096");
            assert_eq!(
                b.dialog().validity("ancho"),
                Some(Validity::Valid),
                "{context}"
            );
            assert!(!b.create_disabled(), "{context}");

            b.focus_until(FocusTarget::Field("alto"));
            b.writes("5000");
            assert!(
                b.last_2_2
                    .contains("error: Alto: máximo 4096 (tarjeta gráfica)"),
                "{context}: el alto dice lo mismo; hay {:?}",
                b.last_2_2.list()
            );
        }
    }
}

#[test]
fn row_11_minus_and_plus_change_the_width_one_by_one() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 16384);
            let context = format!("ppp {ppp} {window:?}");
            let more = b.last_2_2.more();
            let less = b.last_2_2.less();
            assert_eq!(more.len(), 2, "{context}: un + por campo numérico");
            assert_eq!(less.len(), 2, "{context}: un − por campo numérico");

            b.click(more[0].center());
            b.rest();
            assert_eq!(
                b.dialog().text("ancho"),
                Some("65".to_string()),
                "{context}"
            );
            assert_eq!(b.dialog().text("alto"), Some("64".to_string()), "{context}");
            for _ in 0..2 {
                let less = b.last_2_2.less();
                b.click(less[0].center());
                b.rest();
            }
            assert_eq!(
                b.dialog().text("ancho"),
                Some("63".to_string()),
                "{context}"
            );
            let more = b.last_2_2.more();
            b.click(more[1].center());
            b.rest();
            assert_eq!(b.dialog().text("alto"), Some("65".to_string()), "{context}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: modo, fondo y «Color…»
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_changing_mode_with_the_mouse_shows_only_the_background_group_of_that_mode() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        let context = |step: &str| format!("ppp {ppp}: {step}");

        // Indexado: Transparente, Blanco, Negro.
        assert!(
            b.dialog().is_shown("fondo_indexado"),
            "{}",
            context("inicio")
        );
        assert!(b.last_2_2.exact("Transparente").is_some());
        assert!(b.last_2_2.exact("Color…").is_none());

        for (mode_2, background_2, with_transparent) in [
            ("RGBA", "fondo_rgba", true),
            ("Gris con alfa", "fondo_gris_alfa", true),
            ("Gris", "fondo_gris", false),
            ("Indexado", "fondo_indexado", true),
        ] {
            b.click_in(mode_2);
            for other_2 in [
                "fondo_indexado",
                "fondo_rgba",
                "fondo_gris_alfa",
                "fondo_gris",
            ] {
                assert_eq!(
                    b.dialog().is_shown(other_2),
                    other_2 == background_2,
                    "{}: {other_2}",
                    context(mode_2)
                );
            }
            assert!(b.last_2_2.exact("Blanco").is_some(), "{}", context(mode_2));
            assert!(b.last_2_2.exact("Negro").is_some(), "{}", context(mode_2));
            assert_eq!(
                b.last_2_2.exact("Transparente").is_some(),
                with_transparent,
                "{}: Gris no ofrece Transparente",
                context(mode_2)
            );
            let with_color = mode_2 != "Indexado";
            assert_eq!(
                b.last_2_2.exact("Color…").is_some(),
                with_color,
                "{}: «Color…»",
                context(mode_2)
            );
            // Aún no se eligió «Color…»: ningún hexadecimal.
            for color in ["color_rgba", "color_gris_alfa", "color_gris"] {
                assert!(!b.dialog().is_shown(color), "{}: {color}", context(mode_2));
            }
        }
    }
}

#[test]
fn row_11_color_shows_the_hexadecimal_of_that_mode_only_and_validates_the_text() {
    for ppp in PPPS {
        for (mode_2, color, hex_long_invalid) in [
            ("RGBA", "color_rgba", None),
            ("Gris con alfa", "color_gris_alfa", Some(HexError::NotGray)),
            ("Gris", "color_gris", Some(HexError::NotGray)),
        ] {
            let mut b = Bench::open(ppp, Window::Large, 16384);
            let context = format!("ppp {ppp} {mode_2}");
            b.click_in(mode_2);
            assert!(!b.dialog().is_shown(color), "{context}: antes de «Color…»");
            b.click_in("Color…");
            for other_2 in ["color_rgba", "color_gris_alfa", "color_gris"] {
                assert_eq!(
                    b.dialog().is_shown(other_2),
                    other_2 == color,
                    "{context}: {other_2}"
                );
            }
            assert!(
                b.last_2_2.exact("Color").is_some(),
                "{context}: rótulo del hexadecimal"
            );
            assert!(
                b.last_2_2.contains("#808080"),
                "{context}: nace en gris medio"
            );

            // `#12` y `zz`: demasiado cortos en cualquier modo.
            b.focus_until(FocusTarget::Field(color));
            let short = hex_notice(&HexError::Length(2));
            for text_2 in ["#12", "zz"] {
                b.writes(text_2);
                assert_eq!(
                    b.dialog().validity(color),
                    Some(Validity::Invalid(short.to_string())),
                    "{context} {text_2}"
                );
                assert!(
                    b.last_2_2.contains(&format!("{ERROR_PREFIX}{short}")),
                    "{context} {text_2}: falta el error; hay {:?}",
                    b.last_2_2.list()
                );
                assert!(b.create_disabled(), "{context} {text_2}");
            }

            // `#ff0000`: vale en RGBA y no en los grises.
            b.writes("#ff0000");
            match hex_long_invalid {
                None => {
                    assert_eq!(
                        b.dialog().validity(color),
                        Some(Validity::Valid),
                        "{context}"
                    );
                    assert!(!b.create_disabled(), "{context}");
                }
                Some(error) => {
                    assert!(
                        b.last_2_2
                            .contains(&format!("{ERROR_PREFIX}{}", hex_notice(&error))),
                        "{context}: #ff0000; hay {:?}",
                        b.last_2_2.list()
                    );
                    assert!(b.create_disabled(), "{context}");
                }
            }
            // `#80808080`: en Gris falta el alfa; en Gris con alfa y RGBA vale.
            b.writes("#80808080");
            if mode_2 == "Gris" {
                assert!(
                    b.last_2_2
                        .contains(&format!("{ERROR_PREFIX}{}", hex_notice(&HexError::NoAlpha))),
                    "{context}: #80808080; hay {:?}",
                    b.last_2_2.list()
                );
                assert!(b.create_disabled(), "{context}");
            } else {
                assert_eq!(
                    b.dialog().validity(color),
                    Some(Validity::Valid),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn row_11_the_hexadecimal_shows_only_with_a_color_of_its_mode_when_changing_mode() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        b.click_in("RGBA");
        b.click_in("Color…");
        assert!(b.dialog().is_shown("color_rgba"), "ppp {ppp}");
        assert!(
            b.last_2_2.exact("Color").is_some(),
            "ppp {ppp}: rótulo del hexadecimal"
        );

        // Un gris con alfa 255 viaja a Gris como su valor: «Color…» del grupo de Gris, con su
        // hexadecimal; el de RGBA se oculta.
        b.click_in("Gris");
        assert!(
            !b.dialog().is_shown("color_rgba"),
            "ppp {ppp}: el de RGBA se oculta"
        );
        assert!(
            b.dialog().is_shown("color_gris"),
            "ppp {ppp}: el gris viaja como su valor"
        );
        assert!(!b.dialog().is_shown("color_gris_alfa"), "ppp {ppp}");
        assert!(
            !there_is_a_background_notice(&b),
            "ppp {ppp}: viajó sin aviso"
        );

        // Indexado no ofrece «Color…»: ningún hexadecimal y ningún rótulo «Color».
        b.click_in("Indexado");
        for color in ["color_rgba", "color_gris_alfa", "color_gris"] {
            assert!(!b.dialog().is_shown(color), "ppp {ppp}: {color}");
        }
        assert!(b.last_2_2.exact("Color").is_none(), "ppp {ppp}");
        assert!(b.last_2_2.exact("Color…").is_none(), "ppp {ppp}");
        assert!(
            b.last_2_2
                .contains("Indexado sin Color: pasa a Transparente"),
            "ppp {ppp}: dice qué fondo había y a cuál vuelve; hay {:?}",
            b.last_2_2.list()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el informe y el aviso del traslado del fondo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_report_updates_after_each_size_change() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        let context = format!("ppp {ppp}");
        assert!(b.last_2_2.contains("Memoria: menos de 1 MiB"), "{context}");

        b.writes("4097");
        b.focus_until(FocusTarget::Field("alto"));
        b.writes("4096");
        for line in report_text(Some((4097, 4096)), Mode::Indexed, None).lines() {
            assert!(
                b.last_2_2.contains(line),
                "{context}: falta {line:?}; hay {:?}",
                b.last_2_2.list()
            );
        }
        assert!(
            b.last_2_2
                .contains("aviso: más de 16777216 px; puede ir lento"),
            "{context}"
        );
        // El aviso de tamaño no impide crear.
        assert!(!b.create_disabled(), "{context}");

        b.writes("64");
        assert!(
            !b.last_2_2.contains("aviso: más de"),
            "{context}: el aviso desaparece al bajar el tamaño; hay {:?}",
            b.last_2_2.list()
        );

        b.writes("abc");
        assert!(b.last_2_2.contains("Memoria: —"), "{context}: sin valor");
        assert!(b.create_disabled(), "{context}");
    }
}

#[test]
fn row_11_changing_mode_with_transparent_to_gray_warns_in_the_report_until_the_next_change() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        let context = format!("ppp {ppp}");
        b.click_in("RGBA");
        assert!(
            !there_is_a_background_notice(&b),
            "{context}: Transparente viaja a RGBA sin aviso"
        );
        b.click_in("Gris");
        assert!(
            b.last_2_2.contains("Gris sin Transparente: pasa a Blanco"),
            "{context}: falta el aviso; hay {:?}",
            b.last_2_2.list()
        );
        let spec = spec_of_the(&b).expect("spec");
        assert_eq!(
            (spec.mode, spec.background),
            (Mode::Gray, Background::White),
            "{context}"
        );

        // El aviso dura hasta que cambia otro valor del formulario.
        b.rest();
        assert!(
            b.last_2_2.contains("Gris sin Transparente: pasa a Blanco"),
            "{context}"
        );
        b.focus_until(FocusTarget::Field("ancho"));
        b.writes("65");
        assert!(
            !there_is_a_background_notice(&b),
            "{context}: el aviso no sobrevive; hay {:?}",
            b.last_2_2.list()
        );
    }
}

#[test]
fn row_11_a_color_that_does_not_fit_the_new_mode_goes_back_to_the_default_and_says_so() {
    for ppp in PPPS {
        for (target_2, notice, background_expected, mode_expected) in [
            (
                "Gris con alfa",
                "Color no gris: pasa a Transparente",
                Background::Transparent,
                Mode::GrayAlpha,
            ),
            (
                "Gris",
                "Color no gris: pasa a Blanco",
                Background::White,
                Mode::Gray,
            ),
            (
                "Indexado",
                "Indexado sin Color: pasa a Transparente",
                Background::Transparent,
                Mode::Indexed,
            ),
        ] {
            let mut b = Bench::open(ppp, Window::Large, 16384);
            let context = format!("ppp {ppp} → {target_2}");
            b.click_in("RGBA");
            b.click_in("Color…");
            // Un rojo confirmado en el hexadecimal (la preparación no es lo que se prueba).
            {
                let d = b.dialog_mut();
                assert_eq!(
                    d.event(Event::Hex {
                        field: "color_rgba",
                        text: "#ff0000".to_string()
                    }),
                    Reaction::None
                );
                assert_eq!(
                    d.event(Event::HexConfirm {
                        field: "color_rgba",
                        blur: false
                    }),
                    Reaction::None
                );
            }
            b.rest();
            assert!(
                !there_is_a_background_notice(&b),
                "{context}: aún sin aviso"
            );

            b.click_in(target_2);
            assert!(
                b.last_2_2.contains(notice),
                "{context}: falta {notice:?}; hay {:?}",
                b.last_2_2.list()
            );
            let spec = spec_of_the(&b).expect("spec");
            assert_eq!(
                (spec.mode, spec.background),
                (mode_expected, background_expected),
                "{context}"
            );
            // Dice lo que había (Color) o lo que vuelve (Transparente/Blanco), sin pasar de 42.
            assert!(notice.chars().count() <= 42);
        }
    }
}

#[test]
fn row_11_white_and_black_travel_between_modes_without_a_notice() {
    for ppp in PPPS {
        for (background_2, background_expected) in
            [("Blanco", Background::White), ("Negro", Background::Black)]
        {
            let mut b = Bench::open(ppp, Window::Large, 16384);
            let context = format!("ppp {ppp} {background_2}");
            b.click_in("RGBA");
            b.click_in(background_2);
            for (target_2, mode_2) in [
                ("Gris con alfa", Mode::GrayAlpha),
                ("Gris", Mode::Gray),
                ("Indexado", Mode::Indexed),
                ("RGBA", Mode::Rgba),
            ] {
                b.click_in(target_2);
                assert!(!there_is_a_background_notice(&b), "{context} → {target_2}");
                let spec = spec_of_the(&b).expect("spec");
                assert_eq!(
                    (spec.mode, spec.background),
                    (mode_2, background_expected),
                    "{context} → {target_2}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Intro, Crear, Cancelar, Escape y Tab
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_enter_with_everything_valid_creates() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 16384);
            b.presses(Key::Enter);
            assert_eq!(b.accepted.len(), 1, "{}", b.describe());
            assert!(!b.modal.is_open());
            let spec = spec_from_values(&b.accepted[0]).expect("spec");
            assert_eq!(
                (spec.width, spec.height, spec.mode, spec.background),
                (64, 64, Mode::Indexed, Background::Transparent),
                "ppp {ppp} {window:?}"
            );
            assert_eq!(b.cancelled, 0);
        }
    }
}

#[test]
fn row_11_enter_with_an_invalid_value_does_not_create_and_the_dialog_stays_open() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 16384);
            b.writes("0");
            b.presses(Key::Enter);
            assert!(b.accepted.is_empty(), "{}", b.describe());
            assert!(b.modal.is_open(), "ppp {ppp} {window:?}");
        }
    }
}

#[test]
fn row_11_create_accepts_and_dimmed_does_nothing() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 16384);
            b.click_in("Crear");
            assert_eq!(b.accepted.len(), 1, "{}", b.describe());
            assert!(!b.modal.is_open());

            let mut b = Bench::open(ppp, window, 16384);
            b.writes("0");
            assert!(b.create_disabled());
            b.click_in("Crear");
            assert!(
                b.accepted.is_empty(),
                "ppp {ppp} {window:?}: Crear atenuado no crea"
            );
            assert!(b.modal.is_open());
        }
    }
}

#[test]
fn row_11_cancel_and_escape_close_without_creating() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 16384);
            b.click_in("Cancelar");
            assert_eq!(b.cancelled, 1, "{}", b.describe());
            assert!(b.accepted.is_empty());
            assert!(!b.modal.is_open());

            let mut b = Bench::open(ppp, window, 16384);
            b.presses(Key::Escape);
            assert_eq!(b.cancelled, 1, "{}", b.describe());
            assert!(b.accepted.is_empty());
            assert!(!b.modal.is_open());
        }
    }
}

fn checks_tab(b: &mut Bench, context: &str) {
    let order = focus_order(b.dialog());
    assert!(order.contains(&FocusTarget::Accept), "{context}");
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
fn row_11_tab_walks_only_what_is_visible_and_wraps_around_in_indexed() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut b = Bench::open(ppp, window, 16384);
            let order = focus_order(b.dialog());
            assert_eq!(order.first(), Some(&FocusTarget::Field("ancho")));
            for hidden in ["fondo_rgba", "fondo_gris_alfa", "fondo_gris"] {
                assert!(
                    !order.contains(&FocusTarget::Option(hidden, 0)),
                    "ppp {ppp} {window:?}: {hidden} está oculto"
                );
            }
            checks_tab(&mut b, &format!("ppp {ppp} {window:?} indexado"));
        }
    }
}

#[test]
fn row_11_tab_walks_only_what_is_visible_with_color_in_rgba_and_in_gray() {
    for ppp in PPPS {
        for window in WINDOWS {
            for (index_of_mode, background_2, index_of_color, color) in [
                (1usize, "fondo_rgba", 3usize, "color_rgba"),
                (3, "fondo_gris", 2, "color_gris"),
            ] {
                let mut b = Bench::open(ppp, window, 16384);
                b.dialog_mut().event(Event::Choice {
                    field: "modo",
                    index: index_of_mode,
                });
                b.rest();
                b.dialog_mut().event(Event::Choice {
                    field: background_2,
                    index: index_of_color,
                });
                b.rest();
                let order = focus_order(b.dialog());
                assert!(order.contains(&FocusTarget::Field(color)), "{color}");
                for other_2 in ["color_rgba", "color_gris_alfa", "color_gris"] {
                    assert_eq!(
                        order.contains(&FocusTarget::Field(other_2)),
                        other_2 == color,
                        "ppp {ppp} {window:?}: {other_2}"
                    );
                }
                checks_tab(&mut b, &format!("ppp {ppp} {window:?} {color}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: desplazamiento a la ventana mínima y a 960 × 720
// ---------------------------------------------------------------------------------------------

/// Pone el modo RGBA con «Color…» (el formulario más alto: 320 u) con eventos directos.
///
/// En dos pasos con un fotograma entre medias: al cambiar de modo, `sync_form` traslada el fondo del
/// grupo anterior y pisaría la elección hecha en el mismo fotograma.
fn put_rgba_with_color(b: &mut Bench) {
    b.dialog_mut().event(Event::Choice {
        field: "modo",
        index: 1,
    });
    b.rest();
    b.dialog_mut().event(Event::Choice {
        field: "fondo_rgba",
        index: 3,
    });
    b.rest();
}

/// Mueve el puntero al relleno izquierdo del cuerpo (`x = 3 u`) y da `notches` de rueda hacia abajo.
fn wheel_over_the_body(b: &mut Bench, notches: usize) {
    let layout = b.layout();
    let n = n_de(b.ppp);
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
fn y_de(b: &Bench, searched: &str) -> Option<f32> {
    b.last_2_2.shapes.iter().find_map(|f| match f {
        Shape2::Text(t) if t.text_2.trim() == searched => Some(t.pos.y),
        _ => None,
    })
}

#[test]
fn row_11_at_the_minimum_window_the_body_measures_252_u_and_the_form_scrolls() {
    for ppp in PPPS {
        // Indexado por defecto: 280 u.
        let b = Bench::open(ppp, Window::Minimum, 16384);
        let n = n_de(ppp);
        assert_eq!(b.dialog().content_h_u(), 280, "ppp {ppp}: indexado");
        let layout = b.layout();
        assert_eq!(
            layout.body.height,
            252 * n,
            "ppp {ppp}: el cuerpo mide 252 u"
        );
        assert_eq!(layout.max_scroll_u, 280 - 252, "ppp {ppp}: indexado");

        // RGBA con «Color…»: 320 u.
        let mut b = Bench::open(ppp, Window::Minimum, 16384);
        put_rgba_with_color(&mut b);
        assert_eq!(b.dialog().content_h_u(), 320, "ppp {ppp}: RGBA con Color…");
        assert_eq!(
            b.layout().max_scroll_u,
            320 - 252,
            "ppp {ppp}: RGBA con Color…"
        );
    }
}

#[test]
fn row_11_at_the_minimum_the_wheel_scrolls_the_body_and_title_footer_and_buttons_do_not_move() {
    for ppp in PPPS {
        for with_color in [false, true] {
            let mut b = Bench::open(ppp, Window::Minimum, 16384);
            if with_color {
                put_rgba_with_color(&mut b);
            }
            let context = format!("ppp {ppp}, con Color… {with_color}");
            let maximum = b.layout().max_scroll_u;
            assert!(maximum > 0, "{context}");
            let fixed_2 = ["Nueva imagen", "Crear", "Cancelar"];
            let before_fixed: Vec<Option<f32>> = fixed_2.iter().map(|t| y_de(&b, t)).collect();
            let mode_before = y_de(&b, "Modo").expect("Modo");
            assert_eq!(b.dialog().scroll_u(), 0, "{context}");

            // Una muesca son 42 u (se recorta al máximo); con 320 u hacen falta dos.
            wheel_over_the_body(&mut b, 2);
            assert_eq!(b.dialog().scroll_u(), maximum, "{context}: hasta el final");
            let shifted = mode_before - y_de(&b, "Modo").expect("Modo");
            assert!(
                (shifted - maximum as f32 * b.u()).abs() <= 0.1,
                "{context}: el cuerpo se movió {shifted} pt y debía {}",
                maximum as f32 * b.u()
            );
            let after_fixed: Vec<Option<f32>> = fixed_2.iter().map(|t| y_de(&b, t)).collect();
            assert_eq!(
                after_fixed, before_fixed,
                "{context}: título, pie y botones fijos"
            );
            assert!(
                b.last_2_2.exact("Crear").is_some(),
                "{context}: Crear se sigue viendo"
            );
            assert!(b.last_2_2.exact("Cancelar").is_some(), "{context}");
        }
    }
}

#[test]
fn row_11_at_the_minimum_tab_brings_into_view_the_hexadecimal_that_was_outside() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Minimum, 16384);
        put_rgba_with_color(&mut b);
        assert_eq!(b.dialog().scroll_u(), 0, "ppp {ppp}");
        b.focus_until(FocusTarget::Field("color_rgba"));
        assert!(
            b.dialog().scroll_u() > 0,
            "ppp {ppp}: el foco en el hexadecimal desplaza el cuerpo"
        );
        assert!(b.last_2_2.exact("Crear").is_some());
        assert_eq!(
            b.focus_of_egui(),
            Some(Bench::id_de("color_rgba")),
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_11_at_960_by_720_the_form_does_not_scroll() {
    for ppp in PPPS {
        for with_color in [false, true] {
            let mut b = Bench::open(ppp, Window::Large, 16384);
            if with_color {
                put_rgba_with_color(&mut b);
            }
            let context = format!("ppp {ppp}, con Color… {with_color}");
            assert_eq!(b.layout().max_scroll_u, 0, "{context}");
            let before: Vec<Option<f32>> = ["Nueva imagen", "Modo", "Crear", "Cancelar"]
                .iter()
                .map(|t| y_de(&b, t))
                .collect();
            wheel_over_the_body(&mut b, 3);
            assert_eq!(b.dialog().scroll_u(), 0, "{context}");
            let after_2: Vec<Option<f32>> = ["Nueva imagen", "Modo", "Crear", "Cancelar"]
                .iter()
                .map(|t| y_de(&b, t))
                .collect();
            assert_eq!(after_2, before, "{context}: nada se mueve");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: contraste
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

            // Por defecto.
            let mut b = Bench::open(ppp, window, 16384);
            checks_contrast(&b, &context("por defecto"));

            // Con un error en línea (y Crear atenuado, exento).
            b.writes("0");
            checks_contrast(&b, &context("error en el ancho"));

            // Con la nota de la tarjeta gráfica.
            let mut b = Bench::open(ppp, window, 4096);
            b.writes("5000");
            checks_contrast(&b, &context("error con tarjeta gráfica"));

            // RGBA con «Color…» y un hexadecimal inválido.
            let mut b = Bench::open(ppp, window, 16384);
            put_rgba_with_color(&mut b);
            b.focus_until(FocusTarget::Field("color_rgba"));
            b.writes("zz");
            checks_contrast(&b, &context("hexadecimal inválido"));
            b.writes("#ff0000");
            checks_contrast(&b, &context("hexadecimal válido"));

            // Con el aviso del traslado del fondo en el informe.
            let mut b = Bench::open(ppp, window, 16384);
            {
                let d = b.dialog_mut();
                d.event(Event::Choice {
                    field: "modo",
                    index: 1,
                });
            }
            b.rest();
            {
                let d = b.dialog_mut();
                d.event(Event::Choice {
                    field: "modo",
                    index: 3,
                });
            }
            b.rest();
            assert!(
                text_of_the_report(b.dialog()).contains("Gris sin Transparente: pasa a Blanco"),
                "el estado de la prueba lleva el aviso"
            );
            checks_contrast(&b, &context("con el aviso de fondo"));
        }
    }
}

#[test]
fn row_11_the_fixed_texts_of_the_dialog_are_inside_the_character_set() {
    let spec = new_image_form(16384);
    let mut texts: Vec<&str> = vec![spec.title, spec.accept];
    texts.extend(spec.cancel);
    for f in &spec.fields {
        texts.push(f.label);
        if let FieldKind::Choice { options, .. } = &f.kind {
            texts.extend(options.iter().copied());
        }
    }
    for text_2 in texts {
        assert!(
            in_the_set(text_2),
            "{text_2:?} no está en INTERFACE_CHARSET"
        );
    }
    for notice in NOTICES {
        assert!(in_the_set(notice), "{notice:?}");
        assert!(notice.chars().count() <= 42, "{notice:?}");
    }
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
            sync,
            ..
        } = &mut *self;
        runs(ctx, size, vec![], |ui| {
            let c = ui.ctx().clone();
            apply_scale(&c);
            if let Modal::Open(dialog) = modal {
                let _ = show_dialog(&c, dialog, focus);
                sync_form(dialog, sync);
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

    /// El `Id` y las opciones del grupo de fondo que se ve (el único `Choice` visible que no es el
    /// modo).
    fn group_of_background(&self) -> (&'static str, usize) {
        let d = self.dialog();
        d.spec()
            .fields
            .iter()
            .filter(|f| f.id != FIELD_MODE && d.is_shown(f.id))
            .find_map(|f| match &f.kind {
                FieldKind::Choice { options, .. } => Some((f.id, options.len())),
                _ => None,
            })
            .expect("hay un grupo de fondo visible")
    }
}

#[test]
fn row_9_new_image_paints_rings_and_a_dot_per_group_in_each_mode_and_each_background() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        b.markers(&format!("ppp {ppp} al abrir"));
        for mode_2 in [1usize, 2, 3, 0, 2] {
            // Clic real en el aro del modo (el primer grupo, de arriba abajo).
            let shapes = b.raw();
            let center = rings(&marks_of(&shapes))[mode_2].center;
            b.click(center);
            b.rest();
            assert_eq!(
                b.dialog().chosen(FIELD_MODE),
                Some(mode_2),
                "ppp {ppp}: clic en el aro del modo {mode_2}"
            );
            b.markers(&format!("ppp {ppp} modo {mode_2}"));
            // Y en cada opción del fondo de ese modo (el segundo grupo).
            let (background_2, options_2) = b.group_of_background();
            for j in 0..options_2 {
                let shapes = b.raw();
                let center = rings(&marks_of(&shapes))[MODE_OPTIONS.len() + j].center;
                b.click(center);
                b.rest();
                assert_eq!(
                    b.dialog().chosen(background_2),
                    Some(j),
                    "ppp {ppp} modo {mode_2}: clic en el aro {j} de {background_2}"
                );
                b.markers(&format!("ppp {ppp} modo {mode_2} {background_2} {j}"));
            }
        }
    }
}

#[test]
fn row_11_new_image_the_focus_ring_surrounds_the_row_of_each_option_as_always() {
    for ppp in PPPS {
        let mut b = Bench::open(ppp, Window::Large, 16384);
        let location = format!("ppp {ppp}");
        let mut measures = Vec::new();
        for (k, label) in MODE_OPTIONS.iter().enumerate() {
            let goal = FocusTarget::Option(FIELD_MODE, k);
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

// ---------------------------------------------------------------------------------------------
// Apéndice A1 (specs/hito-2-nueva-imagen.md), filas A3 y A4: los dos caminos reales hasta el
// cambio de modo, con un clic real de dos fotogramas (pulsar y soltar por separado) y, para A4,
// también con `Tab` y el teclado. En ROJO hasta la implementación.
//
// - A3, camino (a): escribir `zz` (sin `Intro`) y hacer clic en el selector de modo. La lectura del
//   código (`dialog_view.rs`, `draw_hex`) dice que el clic quita el foco al pulsar y el campo se
//   revierte a su último confirmado **antes** del fotograma en que cambia el modo: `sync_form` ya no
//   ve texto inválido pendiente. La prueba lo **registra** (`eprintln!`, visible con `--nocapture`, y
//   en el mensaje de cada aserción) y exige esa lectura: si fuese errónea, hay que parar.
// - A4, camino (b): escribir `zz`, `Intro` (el error y el foco se quedan) y luego clic en el modo
//   (o `Tab` hasta el selector y `Espacio`): `typed` ya es falso, no hay reversión y `sync_form` sí ve
//   el texto inválido pendiente.
// - Lo que ve `sync_form` en cada fotograma se lee del estado del campo de origen al final de ese
//   fotograma: `sync_form` solo escribe el grupo del modo nuevo, nunca el de origen.
// - La preparación (el modo, «Color…» y el último confirmado) se deja con eventos directos sobre el
//   `Dialog`, como las demás pruebas de la fila 11; lo que se prueba va con ratón y teclado reales.
// - A la ventana mínima el cuerpo se desplaza: antes del clic se sube con la rueda (el selector de
//   modo queda a la vista con el desplazamiento a 0) y, para ver el informe pintado, se baja.
// ---------------------------------------------------------------------------------------------

use px_core::Rgba;
use px_ui::color_edit::hex_text;

const LIGHT_GRAY_A1: Rgba = Rgba::opaque(0xaa, 0xaa, 0xaa);
const MID_GRAY_A1: Rgba = Rgba::new(0x80, 0x80, 0x80, 0x80);
const BLUE_A_1: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const DEFAULT_GRAY_A1: Rgba = Rgba::opaque(0x80, 0x80, 0x80);
const NOT_VALID_A1: &str = "Color no válido: pasa a ";

/// Origen, último confirmado (o ninguno) y destino: con el aviso nuevo, con el de D1b de un color
/// que no viaja, con Indexado y con el alfa.
const CASES_A_1: [(Mode, Option<Rgba>, Mode); 5] = [
    (Mode::Rgba, None, Mode::GrayAlpha),
    (Mode::Gray, Some(LIGHT_GRAY_A1), Mode::Rgba),
    (Mode::Rgba, Some(BLUE_A_1), Mode::GrayAlpha),
    (Mode::GrayAlpha, None, Mode::Indexed),
    (Mode::Rgba, Some(MID_GRAY_A1), Mode::Gray),
];

fn hex8_a1(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn label_a_1(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "Indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "Gris con alfa",
        Mode::Gray => "Gris",
    }
}

fn ix_mode_a1(mode: Mode) -> usize {
    match mode {
        Mode::Indexed => 0,
        Mode::Rgba => 1,
        Mode::GrayAlpha => 2,
        Mode::Gray => 3,
    }
}

fn background_id_a1(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "fondo_indexado",
        Mode::Rgba => "fondo_rgba",
        Mode::GrayAlpha => "fondo_gris_alfa",
        Mode::Gray => "fondo_gris",
    }
}

fn color_id_a1(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Indexed => None,
        Mode::Rgba => Some("color_rgba"),
        Mode::GrayAlpha => Some("color_gris_alfa"),
        Mode::Gray => Some("color_gris"),
    }
}

/// El índice de «Color…» en el grupo de fondo del modo.
fn color_ix_a1(mode: Mode) -> usize {
    if mode == Mode::Gray { 2 } else { 3 }
}

/// El fondo y el aviso que deja descartar un texto inválido con `last_2_2` confirmado (A1-1 y A1-4).
fn after_discarding_a1(from: Mode, last_2_2: Rgba, to: Mode) -> (Background, String) {
    match carry_background(from, Background::Color(last_2_2), to) {
        Carry::Kept(background_2) => (background_2, format!("{NOT_VALID_A1}{}", hex8_a1(last_2_2))),
        Carry::Reset {
            to: background_2,
            notice,
        } => (background_2, notice.to_string()),
    }
}

/// Modo `from` con «Color…» y `confirmed` ya confirmado, con eventos directos (la preparación no es
/// lo que se prueba).
fn prepares_a_1(b: &mut Bench, from: Mode, confirmed: Option<Rgba>) {
    let id = color_id_a1(from).expect("el modo tiene «Color…»");
    b.dialog_mut().event(Event::Choice {
        field: "modo",
        index: ix_mode_a1(from),
    });
    b.rest();
    b.dialog_mut().event(Event::Choice {
        field: background_id_a1(from),
        index: color_ix_a1(from),
    });
    b.rest();
    if let Some(c) = confirmed {
        let d = b.dialog_mut();
        d.event(Event::Hex {
            field: id,
            text: hex8_a1(c),
        });
        d.event(Event::HexConfirm {
            field: id,
            blur: false,
        });
        b.rest();
    }
    assert!(b.dialog().is_shown(id), "{}", b.describe());
    assert_eq!(
        b.dialog().hex_committed(id),
        Some(confirmed.unwrap_or(DEFAULT_GRAY_A1))
    );
    let report = text_of_the_report(b.dialog());
    assert!(
        !there_is_a_background_notice(b) && !report.contains(NOT_VALID_A1),
        "la preparación no deja avisos; informe {report:?}"
    );
}

/// Lo que ve `sync_form` del campo de origen y del modo en un fotograma.
#[derive(Clone, Debug)]
struct SnapshotA1 {
    mode_2: Option<usize>,
    pending_2: bool,
    text_2: Option<String>,
    confirmed: Option<Rgba>,
}

fn snapshot_a_1(b: &Bench, id: &'static str) -> SnapshotA1 {
    let d = b.dialog();
    SnapshotA1 {
        mode_2: d.chosen("modo"),
        pending_2: d.hex_pending(id),
        text_2: d.text(id),
        confirmed: d.hex_committed(id),
    }
}

/// Sube el cuerpo del todo con la rueda (a la ventana mínima se desplaza; a 960 × 720 no hace nada).
fn wheel_up_a1(b: &mut Bench) {
    let layout = b.layout();
    let n = n_de(b.ppp);
    let pos = pos2(
        (layout.body.x + 3 * n) as f32 / b.ppp,
        (layout.body.y + layout.body.height / 2) as f32 / b.ppp,
    );
    b.frame(vec![move_pointer(pos)]);
    b.rest();
    for _ in 0..4 {
        b.frame(vec![wheel_line(true)]);
    }
    b.rest();
    assert_eq!(b.dialog().scroll_u(), 0, "{}", b.describe());
}

/// Lleva el foco al hexadecimal, escribe `zz` y comprueba que sigue enfocado y con el texto pendiente.
fn writes_zz_a1(b: &mut Bench, id: &'static str) {
    b.focus_until(FocusTarget::Field(id));
    b.writes("zz");
    assert!(b.dialog().hex_pending(id), "{}", b.describe());
    assert_eq!(b.dialog().text(id), Some("zz".to_string()));
    assert!(matches!(
        b.dialog().validity(id),
        Some(Validity::Invalid(_))
    ));
    assert_eq!(
        b.focus_of_egui(),
        Some(Bench::id_de(id)),
        "el campo tiene el foco al escribir"
    );
}

/// Clic real en el texto `searched`: mover, pulsar y soltar en tres fotogramas distintos, y tres de
/// reposo; una foto del campo `id` tras cada fotograma.
fn click_with_trace_a1(b: &mut Bench, searched: &str, id: &'static str) -> Vec<SnapshotA1> {
    let pos = b.center_of(searched);
    let mut trace = Vec::new();
    for events in [
        vec![move_pointer(pos)],
        vec![button_2(pos, true)],
        vec![button_2(pos, false)],
        vec![],
        vec![],
        vec![],
    ] {
        b.frame(events);
        trace.push(snapshot_a_1(b, id));
    }
    trace
}

/// Una tecla y tres fotogramas de reposo, con la foto del campo `id` tras cada uno.
fn key_with_trace_a1(b: &mut Bench, key: Key, id: &'static str) -> Vec<SnapshotA1> {
    let mut trace = Vec::new();
    b.frame(vec![key_2(key, Modifiers::NONE)]);
    trace.push(snapshot_a_1(b, id));
    for _ in 0..3 {
        b.frame(vec![]);
        trace.push(snapshot_a_1(b, id));
    }
    trace
}

/// El índice del fotograma en que cambió el modo (exactamente uno) respecto a `before`.
fn frame_of_the_change_a1(before: Option<usize>, trace: &[SnapshotA1], ctx: &str) -> usize {
    let mut previous = before;
    let mut changes_2 = Vec::new();
    for (i, snapshot) in trace.iter().enumerate() {
        if snapshot.mode_2 != previous {
            changes_2.push(i);
        }
        previous = snapshot.mode_2;
    }
    assert_eq!(
        changes_2.len(),
        1,
        "{ctx}: el modo cambia una vez; traza {trace:?}"
    );
    changes_2[0]
}

/// El resultado de A1 o A2 con el aviso, en el diálogo y pintado.
fn checks_result_a1(b: &mut Bench, from: Mode, last_2_2: Rgba, to: Mode, ctx: &str) {
    let (background_2, notice) = after_discarding_a1(from, last_2_2, to);
    assert_eq!(
        text_of_the_report(b.dialog()),
        report_text(Some((64, 64)), to, Some(&notice)),
        "{ctx}: informe"
    );
    assert!(notice.chars().count() <= 42 && in_the_set(&notice), "{ctx}");
    let spec = spec_of_the(b).unwrap_or_else(|| panic!("{ctx}: sin valores válidos"));
    assert_eq!(
        (spec.width, spec.height, spec.mode, spec.background),
        (64, 64, to, background_2),
        "{ctx}"
    );
    let old = color_id_a1(from).expect("color");
    assert!(!b.dialog().is_shown(old), "{ctx}: el viejo se ve");
    if let (Background::Color(color), Some(id)) = (background_2, color_id_a1(to)) {
        assert!(b.dialog().is_shown(id), "{ctx}");
        assert_eq!(b.dialog().validity(id), Some(Validity::Valid), "{ctx}");
        assert!(
            !b.dialog().hex_pending(id),
            "{ctx}: sin texto en el campo nuevo"
        );
        assert_eq!(b.dialog().text(id), Some(hex_text(color)), "{ctx}");
        assert_eq!(b.dialog().hex_committed(id), Some(color), "{ctx}");
        assert_eq!(
            b.dialog().values().expect("válido").color(id),
            Some(color),
            "{ctx}"
        );
    }
    assert!(!b.create_disabled(), "{ctx}: Crear");
    // Pintado: a la ventana mínima el informe queda abajo del cuerpo.
    if b.window == Window::Minimum {
        wheel_over_the_body(b, 3);
    }
    assert!(
        b.last_2_2.contains(&notice),
        "{ctx}: falta {notice:?} pintado; hay {:?}",
        b.last_2_2.list()
    );
}

#[test]
fn row_a3_path_to_typing_zz_and_clicking_on_the_mode_the_field_is_already_reverted_and_the_notice_comes_out()
 {
    for ppp in PPPS {
        for window in WINDOWS {
            for (from, confirmed, to) in CASES_A_1 {
                let last_2_2 = confirmed.unwrap_or(DEFAULT_GRAY_A1);
                let ctx = format!("A3 ppp {ppp} {window:?} {from:?} {confirmed:?} → {to:?}");
                let id = color_id_a1(from).expect("color");
                let mut b = Bench::open(ppp, window, 16384);
                prepares_a_1(&mut b, from, confirmed);
                writes_zz_a1(&mut b, id);
                if window == Window::Minimum {
                    wheel_up_a1(&mut b);
                    assert!(
                        b.dialog().hex_pending(id),
                        "{ctx}: la rueda no cambia el campo"
                    );
                }

                let before = b.dialog().chosen("modo");
                let trace = click_with_trace_a1(&mut b, label_a_1(to), id);
                let change = frame_of_the_change_a1(before, &trace, &ctx);
                let seen = &trace[change];
                eprintln!("{ctx}: en el fotograma {change} del clic sync_form ve {seen:?}");

                // La lectura del código: el clic quitó el foco al pulsar y el campo se revirtió a su
                // último confirmado antes (o en) el fotograma del cambio.
                assert!(
                    !seen.pending_2,
                    "{ctx}: LECTURA ERRÓNEA, el texto inválido SIGUE pendiente en el fotograma del cambio; traza {trace:?}"
                );
                assert_eq!(
                    seen.text_2,
                    Some(hex_text(last_2_2)),
                    "{ctx}: traza {trace:?}"
                );
                assert_eq!(seen.confirmed, Some(last_2_2), "{ctx}");

                // Aun así, el aviso sale y el resultado es el de A1 o A2.
                checks_result_a1(&mut b, from, last_2_2, to, &ctx);
            }
        }
    }
}

#[test]
fn row_a4_path_b_zz_enter_and_click_on_the_mode_the_text_stays_pending_and_the_notice_comes_out() {
    for ppp in PPPS {
        for window in WINDOWS {
            for (from, confirmed, to) in CASES_A_1 {
                let last_2_2 = confirmed.unwrap_or(DEFAULT_GRAY_A1);
                let ctx = format!("A4 ppp {ppp} {window:?} {from:?} {confirmed:?} → {to:?}");
                let id = color_id_a1(from).expect("color");
                let mut b = Bench::open(ppp, window, 16384);
                prepares_a_1(&mut b, from, confirmed);
                writes_zz_a1(&mut b, id);
                b.presses(Key::Enter);
                // Intro con el texto inválido: el error y el foco se quedan; no se crea nada.
                assert!(b.modal.is_open() && b.accepted.is_empty(), "{ctx}");
                assert!(b.dialog().hex_pending(id), "{ctx}: el texto se queda");
                assert_eq!(b.dialog().text(id), Some("zz".to_string()), "{ctx}");
                assert_eq!(
                    b.focus_of_egui(),
                    Some(Bench::id_de(id)),
                    "{ctx}: el foco se queda en el campo"
                );
                if window == Window::Minimum {
                    wheel_up_a1(&mut b);
                }

                let before = b.dialog().chosen("modo");
                let trace = click_with_trace_a1(&mut b, label_a_1(to), id);
                let change = frame_of_the_change_a1(before, &trace, &ctx);
                let seen = &trace[change];
                eprintln!("{ctx}: en el fotograma {change} del clic sync_form ve {seen:?}");

                assert!(
                    seen.pending_2,
                    "{ctx}: LECTURA ERRÓNEA, el texto inválido NO sigue pendiente en el fotograma del cambio; traza {trace:?}"
                );
                assert_eq!(
                    seen.text_2,
                    Some("zz".to_string()),
                    "{ctx}: traza {trace:?}"
                );
                assert_eq!(seen.confirmed, Some(last_2_2), "{ctx}");

                checks_result_a1(&mut b, from, last_2_2, to, &ctx);
                // El campo del modo nuevo no conserva el texto.
                if let Some(new_2) = color_id_a1(to) {
                    assert_ne!(b.dialog().text(new_2), Some("zz".to_string()), "{ctx}");
                }
            }
        }
    }
}

#[test]
fn row_a4_path_b_with_tab_to_the_selector_and_change_with_the_keyboard() {
    let cases = [
        (Mode::Rgba, None, Mode::GrayAlpha),
        (Mode::Rgba, Some(BLUE_A_1), Mode::Gray),
        (Mode::Gray, Some(LIGHT_GRAY_A1), Mode::Indexed),
    ];
    for ppp in PPPS {
        for window in WINDOWS {
            for (from, confirmed, to) in cases {
                let last_2_2 = confirmed.unwrap_or(DEFAULT_GRAY_A1);
                let ctx =
                    format!("A4 teclado ppp {ppp} {window:?} {from:?} {confirmed:?} → {to:?}");
                let id = color_id_a1(from).expect("color");
                let mut b = Bench::open(ppp, window, 16384);
                prepares_a_1(&mut b, from, confirmed);
                writes_zz_a1(&mut b, id);
                b.presses(Key::Enter);
                assert!(b.dialog().hex_pending(id), "{ctx}: tras Intro");

                // Tab hasta la opción del modo de destino: el texto sigue pendiente por el camino.
                b.focus_until(FocusTarget::Option("modo", ix_mode_a1(to)));
                assert!(
                    b.dialog().hex_pending(id),
                    "{ctx}: Tab no revierte el texto tras Intro"
                );
                assert_eq!(b.dialog().text(id), Some("zz".to_string()), "{ctx}");

                // Espacio elige la opción enfocada.
                let before = b.dialog().chosen("modo");
                let trace = key_with_trace_a1(&mut b, Key::Space, id);
                let change = frame_of_the_change_a1(before, &trace, &ctx);
                let seen = &trace[change];
                eprintln!("{ctx}: en el fotograma {change} sync_form ve {seen:?}");
                assert!(
                    seen.pending_2,
                    "{ctx}: LECTURA ERRÓNEA, el texto inválido NO sigue pendiente; traza {trace:?}"
                );
                assert_eq!(seen.text_2, Some("zz".to_string()), "{ctx}");

                checks_result_a1(&mut b, from, last_2_2, to, &ctx);
            }
        }
    }
}
