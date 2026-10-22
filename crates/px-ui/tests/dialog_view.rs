//! Banco de egui de `px_ui::dialog_view::show_dialog` (specs/hito-2-dialogos-modales.md, «Casos»
//! filas 9 (solo la parte de banco), 10 y 11), sin ventana ni capturas: `Context::run_ui` con
//! `RawInput` sintético. En ROJO hasta la implementación: `dialog_view`, `dialog_layout` y el resto
//! de «Piezas» no existen todavía.
//!
//! - **Fila 9 (banco).** Cada texto y cada marca pintados, salvo el deshabilitado, frente al relleno
//!   que tiene debajo (el último relleno opaco, anterior en el orden de pintado, que contiene el
//!   texto): ≥ 4,5:1 el texto (también el de los errores), ≥ 3:1 las marcas (la opción elegida y la
//!   casilla, rellenos `ACCENT`) y el anillo de foco (trazo `ACCENT`). Estados: por defecto, con
//!   errores (vacío, no dígito, rango, 11 cifras, `check`), con el hexadecimal (válido, pendiente
//!   válido e inválido), con opciones y casilla marcadas, con el ratón sobre `Accept`, `Cancel`
//!   y `−`, y con el foco en cada elemento de `focus_order`. Los colores pintados tienen que estar en
//!   `CONTRAST_PAIRS` (no una copia).
//! - **Fila 10.** Piezas y puntero a las cinco escalas en la ventana mínima real (`468n × 333n` px,
//!   el banco usa puntos: `px / ppp`): etiquetas, rango, `−`/`+`, opciones, casilla, teclear en el
//!   numérico, `Intro`, pérdida de foco, `Escape`, `Accept`, `Cancel`, zona de solo lectura, y el
//!   velo (clic, arrastre y rueda no cierran ni cambian nada y no llegan a un botón, un campo ni un
//!   lienzo simulado dibujados detrás, en una capa inferior; con el diálogo oculto sí llegan).
//! - **Fila 11.** Rectángulos pintados frente a `dialog_layout` (ventana mínima y 960 × 720 pt),
//!   nada sale del diálogo salvo el velo, rueda de 42 u por muesca con contenido enorme, foco inicial,
//!   `Tab` y `Mayús`+`Tab` un ciclo entero (sin llegar nunca a lo de detrás), `Intro` y `Espacio` en
//!   los botones, `Intro` y `Escape` en un campo, y cambio de `ppp` y de tamaño de pantalla con el
//!   diálogo abierto.
//!
//! Suposiciones sobre las firmas de «Piezas» (derives no listados): `Dialog`, `Values`, `Reaction`
//! y `Event`: `Clone + Debug + PartialEq`; `FocusTarget`: `Clone + Copy + Debug + PartialEq + Eq`.
//! `dialog_layout` no tiene los tipos exactos en la spec: se llama **solo** desde `layout_of` (un único
//! punto que ajustar) como `dialog_layout((width_px, alto_px): (i64, i64), n: u32, content_h_u: i64,
//! fill: bool, has_cancel: bool)`, with `DialogLayout { dialog, title, body, footer, reason, accept:
//! ScreenRect, cancel: Option<ScreenRect>, max_scroll_u: i64 }`.
//!
//! Decisiones donde la spec no fija un detalle:
//! - Se identifica cada pieza por su texto pintado (`Shape::Text`): el rótulo del botón es
//!   `spec().accept`/`cancel`, las opciones son sus textos y el error en línea es exactamente
//!   `ERROR_PREFIX` + motivo, en una sola forma de texto (o varios tramos, todos `WARNING`). El
//!   motivo del pie es `ERROR_PREFIX` + `reason()`. Un texto cuenta como «el rótulo `−`» si es `−`
//!   (U+2212) o `-`, porque la spec no dice cuál de los dos glifos usa el botón.
//! - Texto deshabilitado = color `TEXT_DISABLED`, alfa < 255 u opacidad < 1; solo pueden estarlo
//!   `Accept` (con errores) y `−`/`+` (en su límite).
//! - El relleno bajo un texto se busca con el recorte aplicado y quitando un cuarto del alto por
//!   arriba y por abajo (el interlineado de la galería puede sobresalir un poco del campo).
//! - Una muesca de la rueda es un `MouseWheel` de una línea (`wheel_line`): 42 u en el cuerpo, 3
//!   líneas en la zona. La rueda se mide con el puntero en el relleno izquierdo del cuerpo
//!   (`x = 3 u` desde el borde), que no es ninguna pieza ni la zona.
//! - Teclear: `Ctrl+A`, doce `Retroceso` y el texto con `Event::Text`, con el foco inicial (el primer
//!   campo) o el pedido con `Tab`.
//! - Las muescas de la rueda y los clics se aplican en `show_dialog` sobre el `Dialog` recibido (no
//!   hay `Modal`): «se cierra» es que la `Reaction` sea `Accepted` o `Cancelled`.
//! - `AvPág`, `RePág`, `Fin`, `Start` y las flechas con el foco en la zona: se exige la dirección y
//!   el recorte (`Fin` va al máximo, `Start` a 0), no cuántas líneas avanza `AvPág`.

use egui::{
    Area, Color32, Context, Event, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Vec2, pos2, vec2,
};
use px_ui::dialog::{Dialog, Event as TestEvent, FieldKind, Reaction, Validity, Values};
use px_ui::dialog_keys::{FocusTarget, focus_order, initial_focus};
use px_ui::dialog_layout::dialog_layout;
use px_ui::dialog_view::{CONTRAST_PAIRS, ERROR_PREFIX, show_dialog};
use px_ui::licenses_view::{clamp_first_line, wrap_text};
use px_ui::text::apply_scale;
use px_ui::theme::{ACCENT, SURFACE_1, TEXT_DISABLED, WARNING, contrast_ratio, visuals};
use px_ui::view::ScreenRect;

mod common;
#[path = "common/dialog_forms.rs"]
mod dialog_forms;

use common::{
    PPPS, TestFill, Text, button_2, context, frame, in_points, key_2, modifiers_command,
    move_pointer, n_of, tolerance_of_egui, wheel_line,
};
use dialog_forms::{all_kinds, huge, open};

const TEST_MIN_TEXT: f64 = 4.5;
const TEST_MIN_MARK: f64 = 3.0;

// ---------------------------------------------------------------------------------------------
// Lo pintado, en orden de pintado
// ---------------------------------------------------------------------------------------------

mod order {
    //! Como `common::painted`, pero conserva el orden de pintado entre textos, rellenos y trazos
    //! (hace falta para «el último relleno que hay debajo») y recoge los trazos de los rectángulos
    //! (el anillo de foco) y los círculos rellenos (una marca redonda).

    use egui::epaint::{ClippedShape, Shape};
    use egui::{Color32, Rect, Vec2};

    use crate::common::{TestFill, TestSpan, Text};

    #[derive(Clone, Copy, Debug)]
    pub struct TestStroke {
        pub rect: Rect,
        pub color: Color32,
        pub width_2: f32,
        pub clip: Rect,
    }

    #[derive(Clone, Debug)]
    pub enum Shape2 {
        Text(Text),
        TestFill(TestFill),
        TestStroke(TestStroke),
    }

    #[derive(Clone, Debug, Default)]
    pub struct Paint {
        pub shapes_2: Vec<Shape2>,
    }

    impl Paint {
        pub fn texts(&self) -> impl Iterator<Item = (usize, &Text)> + '_ {
            self.shapes_2
                .iter()
                .enumerate()
                .filter_map(|(i, f)| match f {
                    Shape2::Text(t) => Some((i, t)),
                    _ => None,
                })
        }

        pub fn fills(&self) -> impl Iterator<Item = (usize, &TestFill)> + '_ {
            self.shapes_2
                .iter()
                .enumerate()
                .filter_map(|(i, f)| match f {
                    Shape2::TestFill(r) => Some((i, r)),
                    _ => None,
                })
        }

        pub fn strokes(&self) -> impl Iterator<Item = (usize, &TestStroke)> + '_ {
            self.shapes_2
                .iter()
                .enumerate()
                .filter_map(|(i, f)| match f {
                    Shape2::TestStroke(t) => Some((i, t)),
                    _ => None,
                })
        }

        /// La primera forma de texto que, recortada, es exactamente `searched`.
        pub fn exact(&self, searched: &str) -> Option<&Text> {
            self.texts()
                .map(|(_, t)| t)
                .find(|t| t.text_2.trim() == searched)
        }

        /// Todas las formas de texto que son exactamente `searched`, de arriba abajo.
        pub fn all_exact(&self, searched: &str) -> Vec<&Text> {
            let mut v: Vec<&Text> = self
                .texts()
                .map(|(_, t)| t)
                .filter(|t| t.text_2.trim() == searched)
                .collect();
            v.sort_by(|a, b| {
                a.pos
                    .y
                    .total_cmp(&b.pos.y)
                    .then(a.pos.x.total_cmp(&b.pos.x))
            });
            v
        }

        /// Todo el texto pintado, junto.
        pub fn all_the_text(&self) -> String {
            self.texts().map(|(_, t)| t.text_2.as_str()).collect()
        }

        pub fn list_of_texts(&self) -> Vec<&str> {
            self.texts().map(|(_, t)| t.text_2.as_str()).collect()
        }
    }

    fn collects(shape: &Shape, clip: Rect, outside: &mut Vec<Shape2>) {
        match shape {
            Shape::Text(t) => {
                let job = &t.galley.job;
                let spans_2 = job
                    .sections
                    .iter()
                    .map(|s| TestSpan {
                        text_2: job.text[s.byte_range.start.0..s.byte_range.end.0].to_owned(),
                        color: t.override_text_color.unwrap_or(
                            if s.format.color == Color32::PLACEHOLDER {
                                t.fallback_color
                            } else {
                                s.format.color
                            },
                        ),
                    })
                    .collect();
                outside.push(Shape2::Text(Text {
                    text_2: t.galley.text().to_owned(),
                    pos: t.pos,
                    size_2: t.galley.size(),
                    clip,
                    spans_2,
                    opacity_2: t.opacity_factor,
                }));
            }
            Shape::Rect(r) => {
                if r.fill != Color32::TRANSPARENT {
                    outside.push(Shape2::TestFill(TestFill {
                        rect: r.rect,
                        fill: r.fill,
                        clip,
                    }));
                }
                if r.stroke.width > 0.0 && r.stroke.color != Color32::TRANSPARENT {
                    outside.push(Shape2::TestStroke(TestStroke {
                        rect: r.rect,
                        color: r.stroke.color,
                        width_2: r.stroke.width,
                        clip,
                    }));
                }
            }
            Shape::Circle(c) => {
                if c.fill != Color32::TRANSPARENT {
                    outside.push(Shape2::TestFill(TestFill {
                        rect: Rect::from_center_size(c.center, Vec2::splat(2.0 * c.radius)),
                        fill: c.fill,
                        clip,
                    }));
                }
            }
            Shape::Vec(shapes_2) => {
                for shape_2 in shapes_2 {
                    collects(shape_2, clip, outside);
                }
            }
            _ => {}
        }
    }

    pub fn paint_2(shapes_2: &[ClippedShape]) -> Paint {
        let mut outside = Vec::new();
        for clipped in shapes_2 {
            collects(&clipped.shape, clipped.clip_rect, &mut outside);
        }
        Paint { shapes_2: outside }
    }
}

use order::{Paint, Shape2, TestStroke, paint_2};

// ---------------------------------------------------------------------------------------------
// Ventanas y geometría
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    /// `468n × 333n` px (la ventana mínima real de la escala).
    Minimum,
    /// 960 × 720 pt.
    Large,
}

/// Tamaño en píxeles y en puntos.
fn measures(ppp: f32, window: Window) -> ((i64, i64), Vec2) {
    match window {
        Window::Minimum => {
            let n = n_of(ppp);
            let (w, h) = (468 * n, 333 * n);
            ((w, h), vec2(w as f32 / ppp, h as f32 / ppp))
        }
        Window::Large => (
            (
                (960.0 * f64::from(ppp)).round() as i64,
                (720.0 * f64::from(ppp)).round() as i64,
            ),
            vec2(960.0, 720.0),
        ),
    }
}

const WINDOWS: [Window; 2] = [Window::Minimum, Window::Large];

/// Lo que se usa de `DialogLayout`.
#[derive(Clone, Copy, Debug)]
struct Rects {
    dialog: ScreenRect,
    title: ScreenRect,
    body: ScreenRect,
    footer: ScreenRect,
    reason: ScreenRect,
    accept: ScreenRect,
    cancel: Option<ScreenRect>,
    max_scroll_u: i64,
}

/// El único sitio donde se llama a `dialog_layout` (ver la cabecera para los tipos supuestos).
fn layout_of(
    window_px: (i64, i64),
    n: i64,
    content_u: i64,
    fill: bool,
    with_cancel: bool,
) -> Rects {
    let l = dialog_layout(
        window_px,
        u32::try_from(n).expect("n cabe en u32"),
        content_u,
        fill,
        with_cancel,
    );
    Rects {
        dialog: l.dialog,
        title: l.title,
        body: l.body,
        footer: l.footer,
        reason: l.reason,
        accept: l.accept,
        cancel: l.cancel,
        max_scroll_u: l.max_scroll_u,
    }
}

fn rect_pt(r: ScreenRect, ppp: f32) -> Rect {
    Rect::from_min_max(
        in_points(ppp, r.x, r.y),
        in_points(ppp, r.x + r.width, r.y + r.height),
    )
}

/// El rectángulo pintado (puntos) es el de `expected_2` (píxeles), con el redondeo de egui.
fn matches(painted: Rect, expected_2: ScreenRect, ppp: f32) -> bool {
    let tol = tolerance_of_egui(ppp);
    let near = |pt: f32, px: i64| (pt * ppp - px as f32).abs() <= tol;
    near(painted.min.x, expected_2.x)
        && near(painted.min.y, expected_2.y)
        && near(painted.max.x, expected_2.x + expected_2.width)
        && near(painted.max.y, expected_2.y + expected_2.height)
}

/// El rectángulo pintado (puntos) cae dentro de `zone` (píxeles), con el redondeo de egui.
fn inside(painted: Rect, zone: ScreenRect, ppp: f32) -> bool {
    let tol = tolerance_of_egui(ppp);
    painted.min.x * ppp >= zone.x as f32 - tol
        && painted.min.y * ppp >= zone.y as f32 - tol
        && painted.max.x * ppp <= (zone.x + zone.width) as f32 + tol
        && painted.max.y * ppp <= (zone.y + zone.height) as f32 + tol
}

// ---------------------------------------------------------------------------------------------
// Lo que hay detrás del diálogo
// ---------------------------------------------------------------------------------------------

/// Un botón, un campo de texto y un lienzo simulado, cada uno en un `Area` de `Order::Middle`
/// (una capa **inferior** a la del diálogo), con lo que han recibido.
#[derive(Clone, Debug)]
struct Behind {
    button_2: Rect,
    button_id: Id,
    text_id: Id,
    text_2: String,
    clicks_button: usize,
    hovers_button: usize,
    clicks_canvas: usize,
    hovers_canvas: usize,
    wheels_canvas: usize,
}

impl Behind {
    fn new_2() -> Behind {
        Behind {
            button_2: Rect::NOTHING,
            button_id: Id::NULL,
            text_id: Id::NULL,
            text_2: String::new(),
            clicks_button: 0,
            hovers_button: 0,
            clicks_canvas: 0,
            hovers_canvas: 0,
            wheels_canvas: 0,
        }
    }

    fn a_zero(&mut self) {
        self.clicks_button = 0;
        self.hovers_button = 0;
        self.clicks_canvas = 0;
        self.hovers_canvas = 0;
        self.wheels_canvas = 0;
    }

    fn nothing_received(&self) -> bool {
        self.clicks_button == 0
            && self.hovers_button == 0
            && self.clicks_canvas == 0
            && self.hovers_canvas == 0
            && self.wheels_canvas == 0
    }
}

fn draws_behind(ctx: &Context, d: &mut Behind, screen: Vec2) {
    // El lienzo simulado: toda la pantalla, con clic, arrastre y rueda.
    let canvas_2 = Area::new(Id::unique("detras_lienzo"))
        .order(Order::Middle)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            ui.allocate_exact_size(screen, Sense::click_and_drag()).1
        });
    let r = canvas_2.inner;
    if r.hovered() {
        d.hovers_canvas += 1;
        d.wheels_canvas += ctx.input(|i| {
            i.events
                .iter()
                .filter(|e| matches!(e, Event::MouseWheel { .. }))
                .count()
        });
    }
    if r.clicked() || r.is_pointer_button_down_on() || r.dragged() {
        d.clicks_canvas += 1;
    }

    // Un campo de texto detrás (para la tabulación).
    let text_2 = Area::new(Id::unique("detras_texto"))
        .order(Order::Middle)
        .fixed_pos(pos2(2.0, 120.0))
        .show(ctx, |ui| ui.text_edit_singleline(&mut d.text_2));
    d.text_id = text_2.inner.id;

    // El botón, encima del lienzo y en la esquina de la ventana (sobre el velo, fuera del diálogo).
    let button_2 = Area::new(Id::unique("detras_boton"))
        .order(Order::Middle)
        .fixed_pos(pos2(2.0, 2.0))
        .show(ctx, |ui| ui.button("Detras"));
    d.button_2 = button_2.inner.rect;
    d.button_id = button_2.inner.id;
    if button_2.inner.hovered() {
        d.hovers_button += 1;
    }
    if button_2.inner.clicked() || button_2.inner.is_pointer_button_down_on() {
        d.clicks_button += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

struct Bench {
    ctx: Context,
    ppp: f32,
    window: Window,
    dialog: Dialog,
    focus: Option<FocusTarget>,
    behind: Option<Behind>,
    swatch_dialog: bool,
    /// Toda `Reaction` que no es `None`, de todos los fotogramas desde el último `clear`.
    reactions: Vec<Reaction>,
}

fn configures(ctx: &Context, ppp: f32) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
}

impl Bench {
    fn new_2(ppp: f32, window: Window, dialog: Dialog) -> Bench {
        let ctx = context(ppp);
        configures(&ctx, ppp);
        Bench {
            ctx,
            ppp,
            window,
            dialog,
            focus: None,
            behind: None,
            swatch_dialog: true,
            reactions: Vec::new(),
        }
    }

    /// Ya con unos fotogramas de reposo (el diálogo colocado y con su foco inicial).
    fn ready(ppp: f32, window: Window, dialog: Dialog) -> Bench {
        let mut bench = Bench::new_2(ppp, window, dialog);
        bench.rest_2_2();
        bench
    }

    fn with_behind(ppp: f32, window: Window, dialog: Dialog) -> Bench {
        let mut bench = Bench::new_2(ppp, window, dialog);
        bench.behind = Some(Behind::new_2());
        bench.rest_2_2();
        bench
    }

    /// Solo lo de detrás, con el diálogo sin mostrar (el control de las pruebas del velo y de `Tab`).
    fn without_dialog(ppp: f32, window: Window, dialog: Dialog) -> Bench {
        let mut bench = Bench::new_2(ppp, window, dialog);
        bench.behind = Some(Behind::new_2());
        bench.swatch_dialog = false;
        bench.rest_2_2();
        bench
    }

    fn screen(&self) -> Vec2 {
        measures(self.ppp, self.window).1
    }

    fn n(&self) -> i64 {
        n_of(self.ppp)
    }

    /// Una unidad `u` en puntos.
    fn u(&self) -> f32 {
        self.n() as f32 / self.ppp
    }

    fn rects(&self) -> Rects {
        layout_of(
            measures(self.ppp, self.window).0,
            self.n(),
            self.dialog.content_h_u(),
            self.dialog.is_fill(),
            self.dialog.spec().cancel.is_some(),
        )
    }

    fn changes(&mut self, ppp: f32, window: Window) {
        self.ppp = ppp;
        self.window = window;
        self.ctx.set_pixels_per_point(ppp);
        configures(&self.ctx, ppp);
    }

    fn frame(&mut self, events: Vec<Event>) -> Paint {
        let screen = self.screen();
        let mut reaction = Reaction::None;
        let shapes_2 = {
            let Bench {
                ctx,
                dialog,
                focus,
                behind,
                swatch_dialog,
                ..
            } = &mut *self;
            let swatch = *swatch_dialog;
            frame(ctx, screen, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                if let Some(d) = behind.as_mut() {
                    draws_behind(&c, d, screen);
                }
                if swatch {
                    reaction = show_dialog(&c, dialog, focus);
                }
            })
        };
        if !matches!(reaction, Reaction::None) {
            self.reactions.push(reaction);
        }
        paint_2(&shapes_2)
    }

    /// Tres fotogramas sin eventos: lo bastante para que egui reparta el espacio y el diálogo
    /// asiente su posición, su foco y su desplazamiento.
    fn rest_2_2(&mut self) -> Paint {
        self.frame(vec![]);
        self.frame(vec![]);
        self.frame(vec![])
    }

    fn move_to(&mut self, pos: Pos2) -> Paint {
        self.frame(vec![move_pointer(pos)]);
        self.rest_2_2()
    }

    /// Mueve, pulsa y suelta en `pos`, y deja un fotograma de reposo.
    fn click(&mut self, pos: Pos2) -> Paint {
        self.frame(vec![move_pointer(pos)]);
        self.frame(vec![button_2(pos, true)]);
        self.frame(vec![button_2(pos, false)]);
        self.rest_2_2()
    }

    fn wheel(&mut self, below: bool, notches: usize) -> Paint {
        let events = (0..notches).map(|_| wheel_line(!below)).collect();
        self.frame(events)
    }

    fn presses_2(&mut self, key: Key, modifiers: Modifiers) -> Paint {
        self.frame(vec![key_2(key, modifiers)]);
        self.rest_2_2()
    }

    fn tab(&mut self, back: bool) -> Paint {
        let modifiers = Modifiers {
            shift: back,
            ..Modifiers::NONE
        };
        self.presses_2(Key::Tab, modifiers)
    }

    /// `Tab` hasta que el foco es `goal` (como mucho una vuelta entera).
    fn focus_until(&mut self, goal: FocusTarget) {
        let ret = focus_order(&self.dialog).len() + 1;
        for _ in 0..ret {
            if self.focus == Some(goal) {
                return;
            }
            self.tab(false);
        }
        assert_eq!(self.focus, Some(goal), "el Tab no llega a {goal:?}");
    }

    /// Escribe `text_2` en el campo con el foco: `Ctrl+A`, borrar y teclear.
    fn writes(&mut self, text_2: &str) -> Paint {
        let mut delete = vec![key_2(Key::A, modifiers_command(false))];
        for _ in 0..12 {
            delete.push(key_2(Key::Backspace, Modifiers::NONE));
        }
        self.frame(delete);
        self.frame(vec![Event::Text(text_2.to_owned())]);
        self.rest_2_2()
    }

    fn event(&mut self, event: TestEvent) {
        self.dialog.event(event);
    }

    fn focus_of_egui(&self) -> Option<Id> {
        self.ctx.memory(|m| m.focused())
    }
}

// ---------------------------------------------------------------------------------------------
// Ayudantes de lectura
// ---------------------------------------------------------------------------------------------

fn accepted(reactions: &[Reaction]) -> Vec<&Values> {
    reactions
        .iter()
        .filter_map(|r| match r {
            Reaction::Accepted(v) => Some(v),
            _ => None,
        })
        .collect()
}

fn cancelled(reactions: &[Reaction]) -> usize {
    reactions
        .iter()
        .filter(|r| matches!(r, Reaction::Cancelled))
        .count()
}

fn closes(reactions: &[Reaction]) -> bool {
    !accepted(reactions).is_empty() || cancelled(reactions) > 0
}

/// El diálogo sin su desplazamiento (que se recorta según la ventana).
fn state_2(dialog: &Dialog) -> Dialog {
    let mut d = dialog.clone();
    d.set_scroll_u(0);
    d
}

fn text_of<'a>(p: &'a Paint, searched: &str) -> &'a Text {
    p.exact(searched)
        .unwrap_or_else(|| panic!("no se pinta {searched:?}; hay {:?}", p.list_of_texts()))
}

/// La primera forma de texto que contiene `searched` (una fila de la zona, o la zona entera si se
/// pinta de una vez).
fn text_that_contains<'a>(p: &'a Paint, searched: &str) -> &'a Text {
    p.texts()
        .map(|(_, t)| t)
        .find(|t| t.text_2.contains(searched))
        .unwrap_or_else(|| panic!("no se pinta {searched:?}; hay {:?}", p.list_of_texts()))
}

fn center_of(p: &Paint, searched: &str) -> Pos2 {
    text_of(p, searched).rect().center()
}

fn is_disabled(t: &Text) -> bool {
    t.opacity_2 < 1.0
        || t.spans_2
            .iter()
            .any(|x| x.color == TEXT_DISABLED || x.color.a() < 255)
}

fn in_warning(t: &Text) -> bool {
    !t.spans_2.is_empty() && t.spans_2.iter().all(|x| x.color == WARNING)
}

fn accept_label(dialog: &Dialog) -> &'static str {
    dialog.spec().accept
}

fn cancel_label(dialog: &Dialog) -> &'static str {
    dialog.spec().cancel.expect("el formulario tiene Cancelar")
}

fn accept_disabled(p: &Paint, dialog: &Dialog) -> bool {
    is_disabled(text_of(p, accept_label(dialog)))
}

fn options_of(dialog: &Dialog, id: &str) -> &'static [&'static str] {
    for f in &dialog.spec().fields {
        if f.id == id
            && let FieldKind::Choice { options, .. } = &f.kind
        {
            return options;
        }
    }
    panic!("no hay un grupo de opciones {id:?}");
}

fn text_of_the_report(dialog: &Dialog) -> String {
    for f in &dialog.spec().fields {
        if let FieldKind::ReadOnly { text, .. } = &f.kind {
            return text.clone();
        }
    }
    panic!("no hay informe");
}

/// El formulario de la fila 3 con el texto del informe cambiado por `changes(texto_original)`.
fn with_report(changes: impl Fn(&str) -> String) -> px_ui::dialog::FormSpec {
    let mut spec = all_kinds();
    for f in spec.fields.iter_mut() {
        if let FieldKind::ReadOnly { text, .. } = &mut f.kind {
            *text = changes(text);
        }
    }
    spec
}

/// El formulario de la fila 3 con `日✓` (fuera del juego de caracteres) en el informe: en la
/// primera línea, para no cambiar el número de líneas.
fn form() -> px_ui::dialog::FormSpec {
    with_report(|original| {
        let (first, rest_2) = original
            .split_once('\n')
            .expect("el informe tiene varias líneas");
        format!("{first} 日✓\n{rest_2}")
    })
}

/// Números de las líneas «linea N» que se pintan, ordenados (el número son los dígitos tras el
/// prefijo; lo que venga detrás, como el `日✓` sustituido, se ignora).
fn numbers_of_line(p: &Paint) -> Vec<usize> {
    let mut v: Vec<usize> = p
        .texts()
        .flat_map(|(_, t)| t.text_2.lines())
        .filter_map(|l| {
            let rest_2 = l.trim().strip_prefix("linea ")?;
            let digits: String = rest_2.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Rectángulo de cada texto cuyo texto recortado es `-` o `−`, de arriba abajo.
fn less(p: &Paint) -> Vec<Rect> {
    let mut v: Vec<Rect> = p
        .texts()
        .map(|(_, t)| t)
        .filter(|t| matches!(t.text_2.trim(), "−" | "-"))
        .map(|t| t.rect())
        .collect();
    v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    v
}

fn more(p: &Paint) -> Vec<Rect> {
    p.all_exact("+").iter().map(|t| t.rect()).collect()
}

/// Rellenos `ACCENT` de como mucho `10 u × 10 u` (marcas) con el centro entre `y0` y `y1` (puntos).
fn marks_between(p: &Paint, u: f32, y0: f32, y1: f32) -> Vec<Rect> {
    p.fills()
        .map(|(_, r)| r)
        .filter(|r| r.fill == ACCENT)
        .map(|r| r.rect)
        .filter(|r| r.width() <= 10.0 * u + 0.05 && r.height() <= 10.0 * u + 0.05)
        .filter(|r| r.width() > 0.0 && (y0..=y1).contains(&r.center().y))
        .collect()
}

/// El anillo de foco: un trazo `ACCENT` de `2 u` con parte visible.
fn rings_2(p: &Paint, u: f32) -> Vec<&TestStroke> {
    p.strokes()
        .map(|(_, t)| t)
        .filter(|t| t.color == ACCENT && (t.width_2 - 2.0 * u).abs() <= 0.01 * u)
        .filter(|t| t.rect.intersect(t.clip).is_positive())
        .collect()
}

fn glaze(p: &Paint) -> Vec<(usize, TestFill)> {
    p.fills()
        .filter(|(_, r)| r.fill == Color32::from_black_alpha(128))
        .map(|(i, r)| (i, *r))
        .collect()
}

/// El relleno del diálogo: el `SURFACE_1` de mayor área.
fn fill_of_the_dialog(p: &Paint) -> Option<(usize, TestFill)> {
    p.fills()
        .filter(|(_, r)| r.fill == SURFACE_1)
        .max_by(|a, b| a.1.rect.area().total_cmp(&b.1.rect.area()))
        .map(|(i, r)| (i, *r))
}

// ---------------------------------------------------------------------------------------------
// Fila 9: contraste de lo pintado
// ---------------------------------------------------------------------------------------------

/// El último relleno opaco anterior a `until` que contiene `probe` (con el recorte aplicado).
fn fill_under(shapes_2: &[Shape2], until: usize, probe: Rect) -> Option<TestFill> {
    shapes_2[..until].iter().rev().find_map(|f| match f {
        Shape2::TestFill(r)
            if r.fill.a() == 255 && r.rect.intersect(r.clip).expand(0.01).contains_rect(probe) =>
        {
            Some(*r)
        }
        _ => None,
    })
}

/// Lo que se comprueba en un fotograma.
#[derive(Debug, Default)]
struct Report {
    failures: Vec<String>,
    /// Elementos comprobados (textos, marcas y anillos).
    checked_2: usize,
    /// Textos deshabilitados, exentos.
    exempt_2: Vec<String>,
    /// `(descripción, foreground, background_2, mínimo required)` de cada comprobación.
    pairs: Vec<(String, Color32, Color32, f64)>,
}

fn report_of(p: &Paint) -> Report {
    let mut inf = Report::default();
    for (i, f) in p.shapes_2.iter().enumerate() {
        match f {
            Shape2::Text(t) => {
                let visible = t.rect().intersect(t.clip);
                if t.text_2.trim().is_empty() || !visible.is_positive() {
                    continue;
                }
                if is_disabled(t) {
                    inf.exempt_2.push(t.text_2.trim().to_owned());
                    continue;
                }
                let probe = visible.shrink2(vec2(0.0, visible.height() * 0.25));
                let Some(background_2) = fill_under(&p.shapes_2, i, probe) else {
                    inf.failures
                        .push(format!("el texto {:?} no tiene relleno debajo", t.text_2));
                    continue;
                };
                for span in &t.spans_2 {
                    if span.text_2.trim().is_empty() {
                        continue;
                    }
                    inf.checked_2 += 1;
                    let ratio = contrast_ratio(span.color, background_2.fill);
                    inf.pairs.push((
                        format!("texto {:?}", span.text_2),
                        span.color,
                        background_2.fill,
                        TEST_MIN_TEXT,
                    ));
                    if ratio < TEST_MIN_TEXT {
                        inf.failures.push(format!(
                            "el texto {:?} ({:?}) sobre {:?} da {ratio:.2} (< {TEST_MIN_TEXT})",
                            span.text_2, span.color, background_2.fill
                        ));
                    }
                }
            }
            Shape2::TestFill(r) if r.fill == ACCENT => {
                let visible = r.rect.intersect(r.clip);
                if !visible.is_positive() {
                    continue;
                }
                let Some(background_2) = fill_under(&p.shapes_2, i, visible) else {
                    inf.failures
                        .push(format!("la marca {:?} no tiene relleno debajo", r.rect));
                    continue;
                };
                inf.checked_2 += 1;
                inf.pairs
                    .push(("marca".into(), ACCENT, background_2.fill, TEST_MIN_MARK));
                let ratio = contrast_ratio(ACCENT, background_2.fill);
                if ratio < TEST_MIN_MARK {
                    inf.failures.push(format!(
                        "la marca {:?} sobre {:?} da {ratio:.2} (< {TEST_MIN_MARK})",
                        r.rect, background_2.fill
                    ));
                }
            }
            Shape2::TestStroke(t) if t.color == ACCENT => {
                if !t.rect.intersect(t.clip).is_positive() {
                    continue;
                }
                let probe = Rect::from_center_size(t.rect.center(), Vec2::splat(0.1));
                let Some(background_2) = fill_under(&p.shapes_2, i, probe) else {
                    inf.failures
                        .push(format!("el anillo {:?} no tiene relleno debajo", t.rect));
                    continue;
                };
                inf.checked_2 += 1;
                inf.pairs
                    .push(("anillo".into(), ACCENT, background_2.fill, TEST_MIN_MARK));
                let ratio = contrast_ratio(ACCENT, background_2.fill);
                if ratio < TEST_MIN_MARK {
                    inf.failures.push(format!(
                        "el anillo {:?} sobre {:?} da {ratio:.2} (< {TEST_MIN_MARK})",
                        t.rect, background_2.fill
                    ));
                }
            }
            _ => {}
        }
    }
    inf
}

/// Los estados de la fila 9.
const SCENARIOS: [&str; 15] = [
    "defecto",
    "vacio_y_no_digito",
    "rango",
    "once_cifras",
    "check_falla",
    "hex",
    "hex_invalido",
    "hex_pendiente_valido",
    "opciones_y_casilla",
    "hover_aceptar",
    "hover_cancelar",
    "hover_menos",
    "hover_aceptar_deshabilitado",
    "limite_inferior",
    "limite_superior",
];

fn number(b: &mut Bench, field_2: &'static str, text_2: &str) {
    b.event(TestEvent::Number {
        field: field_2,
        text: text_2.to_owned(),
    });
}

fn prepares(b: &mut Bench, scenario: &str) {
    match scenario {
        "defecto" => {}
        "vacio_y_no_digito" => {
            number(b, "ancho", "abc");
            number(b, "alto", "");
        }
        "rango" => {
            number(b, "ancho", "0");
            number(b, "alto", "8193");
        }
        "once_cifras" => number(b, "ancho", "12345678901"),
        "check_falla" => {
            number(b, "ancho", "8192");
            number(b, "alto", "8192");
        }
        "hex" => b.event(TestEvent::Choice {
            field: "modo",
            index: 2,
        }),
        "hex_invalido" => {
            b.event(TestEvent::Choice {
                field: "modo",
                index: 2,
            });
            b.event(TestEvent::Hex {
                field: "color",
                text: "zz".into(),
            });
        }
        "hex_pendiente_valido" => {
            b.event(TestEvent::Choice {
                field: "modo",
                index: 2,
            });
            b.event(TestEvent::Hex {
                field: "color",
                text: "#12f".into(),
            });
        }
        "opciones_y_casilla" => {
            b.event(TestEvent::Choice {
                field: "modo",
                index: 1,
            });
            b.event(TestEvent::Check {
                field: "opaco",
                on: true,
            });
        }
        "hover_aceptar" => {
            let p = b.rest_2_2();
            let c = center_of(&p, accept_label(&b.dialog));
            b.frame(vec![move_pointer(c)]);
        }
        "hover_cancelar" => {
            let p = b.rest_2_2();
            let c = center_of(&p, cancel_label(&b.dialog));
            b.frame(vec![move_pointer(c)]);
        }
        "hover_menos" => {
            let p = b.rest_2_2();
            let c = less(&p).first().expect("hay un −").center();
            b.frame(vec![move_pointer(c)]);
        }
        "hover_aceptar_deshabilitado" => {
            number(b, "ancho", "abc");
            let p = b.rest_2_2();
            let c = center_of(&p, accept_label(&b.dialog));
            b.frame(vec![move_pointer(c)]);
        }
        "limite_inferior" => number(b, "ancho", "1"),
        "limite_superior" => number(b, "ancho", "8192"),
        other_2 => panic!("escenario desconocido: {other_2}"),
    }
}

fn checks_scenarios(window: Window) {
    for ppp in PPPS {
        for scenario in SCENARIOS {
            let mut bench = Bench::new_2(ppp, window, open(form()));
            bench.rest_2_2();
            prepares(&mut bench, scenario);
            let p = bench.rest_2_2();
            let inf = report_of(&p);
            assert!(
                inf.failures.is_empty(),
                "ppp {ppp} {window:?} {scenario}: {:#?}",
                inf.failures
            );
            assert!(
                inf.checked_2 >= 8,
                "ppp {ppp} {window:?} {scenario}: solo se comprueban {} elementos; hay {:?}",
                inf.checked_2,
                p.list_of_texts()
            );
            // Solo lo deshabilitado queda exento: `Accept` y los botones `−` y `+`.
            for exempt in &inf.exempt_2 {
                assert!(
                    [accept_label(&bench.dialog), "−", "-", "+"].contains(&exempt.as_str()),
                    "ppp {ppp} {window:?} {scenario}: texto exento que no es un botón: {exempt:?}"
                );
            }
        }
    }
}

#[test]
fn row_9_all_painted_text_and_marks_exceed_their_minimum_over_their_fill_in_the_minimum_window() {
    checks_scenarios(Window::Minimum);
}

#[test]
fn row_9_all_painted_text_and_marks_exceed_their_minimum_over_their_fill_at_960x720() {
    checks_scenarios(Window::Large);
}

#[test]
fn row_9_with_errors_and_the_hexadecimal_the_marks_and_the_errors_are_painted_and_checked() {
    // Que la prueba de arriba no sea vacua en lo importante: en estos estados hay marcas de
    // opción y casilla, errores en `WARNING` y el hexadecimal.
    for ppp in PPPS {
        let u = n_of(ppp) as f32 / ppp;
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "opciones_y_casilla");
        let p = bench.rest_2_2();
        let options_2 = options_of(&bench.dialog, "modo");
        let first = center_of(&p, options_2[0]).y;
        let box_value = center_of(&p, "Opaco").y;
        let marks = marks_between(&p, u, first - 8.0 * u, box_value + 8.0 * u);
        assert!(
            marks.len() >= 2,
            "ppp {ppp}: la opción elegida y la casilla se marcan en ACCENT: {marks:?}"
        );

        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "hex_invalido");
        let p = bench.rest_2_2();
        let reason = bench.dialog.reason().expect("hay un motivo");
        let footer = format!("{ERROR_PREFIX}{reason}");
        let t = text_of(&p, &footer);
        assert!(in_warning(t), "ppp {ppp}: el motivo del pie va en WARNING");
        assert!(
            report_of(&p)
                .pairs
                .iter()
                .any(|(_, fg, bg, min)| *fg == WARNING
                    && *min >= TEST_MIN_TEXT
                    && contrast_ratio(*fg, *bg) >= TEST_MIN_TEXT),
            "ppp {ppp}: el error se compara con su fondo (≥ 4,5)"
        );
    }
}

#[test]
fn row_9_the_focus_ring_of_each_element_is_painted_and_exceeds_3_to_1() {
    for ppp in PPPS {
        for window in WINDOWS {
            for scenario in ["defecto", "hex_pendiente_valido", "vacio_y_no_digito"] {
                let u = n_of(ppp) as f32 / ppp;
                let mut bench = Bench::new_2(ppp, window, open(form()));
                bench.rest_2_2();
                prepares(&mut bench, scenario);
                let mut p = bench.rest_2_2();
                let order = focus_order(&bench.dialog);
                assert!(order.len() >= 8, "{scenario}: {order:?}");
                for (k, goal) in order.iter().enumerate() {
                    if k > 0 {
                        p = bench.tab(false);
                    }
                    assert_eq!(
                        bench.focus,
                        Some(*goal),
                        "ppp {ppp} {window:?} {scenario}: paso {k}"
                    );
                    let inf = report_of(&p);
                    assert!(
                        inf.failures.is_empty(),
                        "ppp {ppp} {window:?} {scenario} foco {goal:?}: {:#?}",
                        inf.failures
                    );
                    // El contraste del anillo frente a lo que tiene debajo lo comprueba `report_of`.
                    assert!(
                        !rings_2(&p, u).is_empty(),
                        "ppp {ppp} {window:?} {scenario}: el foco en {goal:?} no pinta un \
                         anillo ACCENT de 2 u a la vista"
                    );
                }
            }
        }
    }
}

#[test]
fn row_9_the_painted_colors_are_in_the_pairs_table_with_their_minimum() {
    // «Los colores de la tabla son los que se pintan (no una copia)»: cada par frente/fondo que se
    // pinta está en `CONTRAST_PAIRS`, con un mínimo de al menos 4,5 (texto) o 3,0 (marcas y anillo)
    // y el par lo cumple.
    for ppp in PPPS {
        for window in WINDOWS {
            for scenario in [
                "defecto",
                "vacio_y_no_digito",
                "check_falla",
                "hex_invalido",
                "opciones_y_casilla",
                "hover_aceptar",
                "hover_menos",
            ] {
                let mut bench = Bench::new_2(ppp, window, open(form()));
                bench.rest_2_2();
                prepares(&mut bench, scenario);
                let p = bench.rest_2_2();
                for (desc, fg, bg, required) in report_of(&p).pairs {
                    let in_table = CONTRAST_PAIRS
                        .iter()
                        .find(|&&(_, f, b, _)| f == fg && b == bg);
                    let Some(&(name_2, _, _, minimum_2)) = in_table else {
                        panic!(
                            "ppp {ppp} {window:?} {scenario}: {desc} se pinta con {fg:?} sobre \
                             {bg:?}, que no está en CONTRAST_PAIRS"
                        );
                    };
                    assert!(
                        minimum_2 >= required,
                        "{name_2}: la tabla pide {minimum_2} y se necesita {required}"
                    );
                    assert!(
                        contrast_ratio(fg, bg) >= minimum_2,
                        "{name_2}: {fg:?} sobre {bg:?} no llega a {minimum_2}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_9_only_the_disabled_text_accept_with_errors_and_the_limits_are_exempt() {
    for ppp in PPPS {
        // Con errores, `Accept` se pinta deshabilitado (y exento); con todo válido, no.
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let valid = bench.rest_2_2();
        assert!(!accept_disabled(&valid, &bench.dialog), "ppp {ppp}");
        assert!(report_of(&valid).exempt_2.is_empty(), "ppp {ppp}");

        prepares(&mut bench, "vacio_y_no_digito");
        let invalid = bench.rest_2_2();
        assert!(
            accept_disabled(&invalid, &bench.dialog),
            "ppp {ppp}: con errores, Aceptar se pinta deshabilitado"
        );
        let inf = report_of(&invalid);
        assert_eq!(
            inf.exempt_2,
            [accept_label(&bench.dialog)],
            "ppp {ppp}: lo único exento es Aceptar"
        );

        // `−` en el mínimo y `+` en el máximo.
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "limite_inferior");
        let p = bench.rest_2_2();
        assert!(
            report_of(&p).exempt_2.iter().any(|t| t == "−" || t == "-"),
            "ppp {ppp}: el `−` en el mínimo se pinta deshabilitado"
        );
        prepares(&mut bench, "limite_superior");
        let p = bench.rest_2_2();
        assert!(
            report_of(&p).exempt_2.iter().any(|t| t == "+"),
            "ppp {ppp}: el `+` en el máximo se pinta deshabilitado"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: piezas y puntero
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_title_labels_range_options_box_and_buttons_are_painted() {
    for ppp in PPPS {
        let bench_dialog = open(form());
        let options_2 = options_of(&bench_dialog, "modo");
        let mut bench = Bench::ready(ppp, Window::Minimum, bench_dialog);
        let p = bench.rest_2_2();
        let title = bench.dialog.spec().title;
        for expected_2 in [title, "Ancho", "Alto", "Modo", "Opaco"] {
            assert!(
                p.texts().any(|(_, t)| t.text_2.contains(expected_2)),
                "ppp {ppp}: falta {expected_2:?}; hay {:?}",
                p.list_of_texts()
            );
        }
        // El rango visible, una vez por numérico.
        let ranges = p
            .texts()
            .filter(|(_, t)| t.text_2.contains("1-8192"))
            .count();
        assert_eq!(ranges, 2, "ppp {ppp}: `1-8192` en cada numérico");
        for option in options_2 {
            assert!(p.exact(option).is_some(), "ppp {ppp}: falta {option:?}");
        }
        assert!(p.exact(accept_label(&bench.dialog)).is_some());
        assert!(p.exact(cancel_label(&bench.dialog)).is_some());
        // Con todo válido no hay ningún error.
        assert!(
            p.texts()
                .all(|(_, t)| !t.text_2.trim_start().starts_with(ERROR_PREFIX)),
            "ppp {ppp}: sin errores no se pinta `{ERROR_PREFIX}`"
        );
        // El hexadecimal está oculto con la opción 0.
        assert!(!bench.dialog.is_shown("color"));
        assert!(p.exact("Color").is_none(), "ppp {ppp}");
    }
}

#[test]
fn row_10_the_report_with_text_outside_the_set_paints_no_missing_glyphs() {
    // El contexto del banco entra en pánico ante un glifo ausente: todo texto externo pasa por
    // `printable`.
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        let all = p.all_the_text();
        assert!(!all.contains('日'), "ppp {ppp}: {all:?}");
        assert!(!all.contains('✓'), "ppp {ppp}: {all:?}");
        assert!(all.contains("linea 1"), "ppp {ppp}");
        // También al recorrer el informe con la rueda (entran filas nuevas).
        let center = text_that_contains(&p, "linea 2").rect().center();
        bench.move_to(center);
        bench.wheel(true, 3);
        let p = bench.rest_2_2();
        let all = p.all_the_text();
        assert!(
            !all.contains('日') && !all.contains('✓'),
            "ppp {ppp}: {all:?}"
        );
        assert!(
            all.contains("linea 10"),
            "ppp {ppp}: se ven las filas de más abajo"
        );
    }
}

#[test]
fn row_10_click_on_minus_and_plus_changes_the_text_and_does_not_pass_the_limit() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        assert_eq!(bench.dialog.text("ancho"), Some("64".into()));

        // `+` y `−` del primer numérico (el de arriba).
        let more_0 = more(&p)[0].center();
        let p = bench.click(more_0);
        assert_eq!(bench.dialog.text("ancho"), Some("65".into()), "ppp {ppp}");
        assert_eq!(bench.dialog.text("alto"), Some("64".into()), "ppp {ppp}");
        assert!(p.exact("65").is_some(), "ppp {ppp}: se pinta el 65");
        let less_0 = less(&p)[0].center();
        bench.click(less_0);
        bench.click(less_0);
        assert_eq!(bench.dialog.text("ancho"), Some("63".into()), "ppp {ppp}");

        // El segundo numérico tiene los suyos.
        let p = bench.rest_2_2();
        let more_1 = more(&p)[1].center();
        bench.click(more_1);
        assert_eq!(bench.dialog.text("alto"), Some("65".into()), "ppp {ppp}");
        assert_eq!(bench.dialog.text("ancho"), Some("63".into()), "ppp {ppp}");

        // Los límites: no pasan.
        number(&mut bench, "ancho", "1");
        let p = bench.rest_2_2();
        bench.click(less(&p)[0].center());
        assert_eq!(
            bench.dialog.text("ancho"),
            Some("1".into()),
            "ppp {ppp}: min"
        );
        number(&mut bench, "ancho", "8192");
        let p = bench.rest_2_2();
        bench.click(more(&p)[0].center());
        assert_eq!(
            bench.dialog.text("ancho"),
            Some("8192".into()),
            "ppp {ppp}: max"
        );
        // Con el texto inválido, `+` pone el valor por defecto.
        number(&mut bench, "ancho", "abc");
        let p = bench.rest_2_2();
        bench.click(more(&p)[0].center());
        assert_eq!(
            bench.dialog.text("ancho"),
            Some("64".into()),
            "ppp {ppp}: inválido → por defecto"
        );
    }
}

#[test]
fn row_10_click_on_an_option_marks_it_and_leaves_a_single_one_marked() {
    for ppp in PPPS {
        let u = n_of(ppp) as f32 / ppp;
        let dialog = open(form());
        let options_2 = options_of(&dialog, "modo");
        let mut bench = Bench::ready(ppp, Window::Minimum, dialog);
        for index_2 in [2usize, 1, 0, 2] {
            let p = bench.rest_2_2();
            let center = center_of(&p, options_2[index_2]);
            bench.click(center);
            let values = bench.dialog.values().expect("sigue válido");
            assert_eq!(values.choice("modo"), Some(index_2), "ppp {ppp}");
            assert_eq!(
                bench.dialog.is_shown("color"),
                index_2 == 2,
                "ppp {ppp}: el hexadecimal solo se muestra con la opción 2"
            );

            let p = bench.rest_2_2();
            let ys: Vec<f32> = options_2.iter().map(|o| center_of(&p, o).y).collect();
            let marks = marks_between(&p, u, ys[0] - 7.0 * u, ys[ys.len() - 1] + 7.0 * u);
            assert_eq!(
                marks.len(),
                1,
                "ppp {ppp}: una sola opción marcada, elegida {index_2}: {marks:?}"
            );
            assert!(
                (marks[0].center().y - ys[index_2]).abs() <= 7.0 * u,
                "ppp {ppp}: la marca está en la opción {index_2}"
            );
        }
    }
}

#[test]
fn row_10_click_on_the_box_toggles_it_and_paints_its_mark() {
    for ppp in PPPS {
        let u = n_of(ppp) as f32 / ppp;
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        for expected_2 in [true, false, true] {
            let p = bench.rest_2_2();
            let t = text_of(&p, "Opaco");
            let center = t.rect().center();
            bench.click(center);
            let values = bench.dialog.values().expect("sigue válido");
            assert_eq!(values.check("opaco"), Some(expected_2), "ppp {ppp}");

            let p = bench.rest_2_2();
            let y = text_of(&p, "Opaco").rect().center().y;
            let marks = marks_between(&p, u, y - 8.0 * u, y + 8.0 * u);
            assert_eq!(
                !marks.is_empty(),
                expected_2,
                "ppp {ppp}: la marca de la casilla solo se pinta con la casilla marcada: {marks:?}"
            );
        }
    }
}

/// Lo tecleado, y el error en línea y el del pie que se esperan (sin prefijo).
const TYPED: [(&str, Option<&str>); 9] = [
    ("abc", Some("solo dígitos 0-9")),
    ("-5", Some("solo dígitos 0-9")),
    ("12345678901", Some("máximo 10 cifras")),
    ("8193", Some("máximo 8192")),
    ("0", Some("mínimo 1")),
    ("", Some("escribe un número")),
    ("500", None),
    ("007", None),
    ("0000000064", None),
];

#[test]
fn row_10_typing_in_the_numeric_shows_the_inline_error_with_prefix_and_disables_accept() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        // El foco inicial es el primer campo (`width_2`).
        assert_eq!(bench.focus, Some(FocusTarget::Field("ancho")), "ppp {ppp}");
        for (typed, error) in TYPED {
            let p = bench.writes(typed);
            assert_eq!(
                bench.dialog.text("ancho"),
                Some(typed.to_owned()),
                "ppp {ppp}: {typed:?}"
            );
            match error {
                Some(reason_2) => {
                    let in_line = format!("{ERROR_PREFIX}{reason_2}");
                    let t = text_of(&p, &in_line);
                    assert!(in_warning(t), "ppp {ppp}: {in_line:?} va en WARNING");
                    // El motivo del pie: `error: Ancho: reason_2`.
                    let footer = format!("{ERROR_PREFIX}Ancho: {reason_2}");
                    assert_eq!(
                        bench.dialog.reason().as_deref(),
                        Some(format!("Ancho: {reason_2}").as_str()),
                        "ppp {ppp}"
                    );
                    let t = text_of(&p, &footer);
                    assert!(in_warning(t), "ppp {ppp}: {footer:?} va en WARNING");
                    assert!(
                        accept_disabled(&p, &bench.dialog),
                        "ppp {ppp}: {typed:?}: Aceptar deshabilitado"
                    );
                    assert!(bench.dialog.values().is_none());
                }
                None => {
                    assert!(
                        p.texts()
                            .all(|(_, t)| !t.text_2.trim_start().starts_with(ERROR_PREFIX)),
                        "ppp {ppp}: {typed:?} es válido: sin errores"
                    );
                    assert!(!accept_disabled(&p, &bench.dialog));
                    assert!(bench.dialog.values().is_some());
                }
            }
        }
        // Una cadena larguísima se acota a 11 caracteres (y es `TooLong`).
        bench.writes(&"9".repeat(40));
        let text_2 = bench.dialog.text("ancho").expect("el campo existe");
        assert!(text_2.chars().count() <= 11, "ppp {ppp}: {text_2:?}");
        assert!(bench.dialog.reason().is_some());
    }
}

#[test]
fn row_10_typing_in_the_second_numeric_changes_only_that_field() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        // El segundo «64» es el del alto: clic en él le da el foco.
        let values = p.all_exact("64");
        assert_eq!(values.len(), 2, "ppp {ppp}: {:?}", p.list_of_texts());
        let center = values[1].rect().center();
        bench.click(center);
        assert_eq!(bench.focus, Some(FocusTarget::Field("alto")), "ppp {ppp}");
        bench.writes("12");
        assert_eq!(bench.dialog.text("alto"), Some("12".into()), "ppp {ppp}");
        assert_eq!(bench.dialog.text("ancho"), Some("64".into()), "ppp {ppp}");
    }
}

#[test]
fn row_10_enter_accepts_with_everything_valid_and_with_errors_it_does_not_and_the_repeat_does_not_accept()
 {
    for ppp in PPPS {
        // Todo válido: `Intro` en el campo acepta con los valores.
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.writes("500");
        bench.reactions.clear();
        bench.presses_2(Key::Enter, Modifiers::NONE);
        let accepted = accepted(&bench.reactions);
        assert_eq!(accepted.len(), 1, "ppp {ppp}: {:?}", bench.reactions);
        assert_eq!(accepted[0].number("ancho"), Some(500), "ppp {ppp}");
        assert_eq!(accepted[0].number("alto"), Some(64), "ppp {ppp}");
        assert_eq!(cancelled(&bench.reactions), 0);

        // Con errores no hace nada: ni acepta, ni cancela, ni cambia el texto.
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.writes("abc");
        bench.reactions.clear();
        bench.presses_2(Key::Enter, Modifiers::NONE);
        assert!(
            !closes(&bench.reactions),
            "ppp {ppp}: {:?}",
            bench.reactions
        );
        assert_eq!(bench.dialog.text("ancho"), Some("abc".into()), "ppp {ppp}");

        // Una repetición de teclado (`Intro` mantenida) no acepta. egui decide qué es una
        // repetición por si la tecla ya estaba pulsada (ignora el `repeat` que se le da), así que
        // se reproduce de verdad: la tecla se pulsa antes de que exista el diálogo (en el menú) y
        // sigue pulsada cuando este sale.
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.swatch_dialog = false;
        bench.frame(vec![key_2(Key::Enter, Modifiers::NONE)]);
        bench.swatch_dialog = true;
        bench.rest_2_2();
        bench.reactions.clear();
        let repetition = Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        bench.frame(vec![repetition]);
        bench.rest_2_2();
        assert!(
            !closes(&bench.reactions),
            "ppp {ppp}: {:?}",
            bench.reactions
        );
    }
}

#[test]
fn row_10_losing_focus_keeps_the_text_of_the_numeric_and_its_error() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.writes("abc");
        bench.tab(false);
        assert_ne!(bench.focus, Some(FocusTarget::Field("ancho")), "ppp {ppp}");
        assert_eq!(bench.dialog.text("ancho"), Some("abc".into()), "ppp {ppp}");
        let p = bench.rest_2_2();
        assert!(
            p.exact(&format!("{ERROR_PREFIX}solo dígitos 0-9"))
                .is_some(),
            "ppp {ppp}: el error sigue en línea"
        );
        assert!(accept_disabled(&p, &bench.dialog), "ppp {ppp}");
        assert!(!closes(&bench.reactions), "ppp {ppp}");
    }
}

#[test]
fn row_10_escape_cancels_only_once() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.reactions.clear();
        bench.presses_2(Key::Escape, Modifiers::NONE);
        assert_eq!(cancelled(&bench.reactions), 1, "ppp {ppp}");
        assert!(accepted(&bench.reactions).is_empty(), "ppp {ppp}");
        // También con errores (no hace falta que sea válido para cancelar).
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.writes("abc");
        bench.reactions.clear();
        bench.presses_2(Key::Escape, Modifiers::NONE);
        assert_eq!(cancelled(&bench.reactions), 1, "ppp {ppp}: con errores");
    }
}

#[test]
fn row_10_click_on_accept_with_errors_does_not_accept_and_the_reason_is_visible() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "vacio_y_no_digito");
        let p = bench.rest_2_2();
        let reason_2 = bench.dialog.reason().expect("hay un motivo");
        assert_eq!(
            reason_2, "Ancho: solo dígitos 0-9",
            "el primero en orden de campos"
        );
        assert!(
            p.exact(&format!("{ERROR_PREFIX}{reason_2}")).is_some(),
            "ppp {ppp}: el motivo es visible en el pie; hay {:?}",
            p.list_of_texts()
        );
        assert!(accept_disabled(&p, &bench.dialog), "ppp {ppp}");
        bench.reactions.clear();
        bench.click(center_of(&p, accept_label(&bench.dialog)));
        assert!(
            !closes(&bench.reactions),
            "ppp {ppp}: {:?}",
            bench.reactions
        );
        let p = bench.rest_2_2();
        assert!(
            p.exact(&format!("{ERROR_PREFIX}{reason_2}")).is_some(),
            "ppp {ppp}: el motivo sigue visible tras el clic"
        );
    }
}

#[test]
fn row_10_the_reason_of_the_cross_check_is_also_visible() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "check_falla");
        let p = bench.rest_2_2();
        let reason_2 = bench.dialog.reason().expect("el check falla");
        assert_eq!(reason_2, "demasiado grande");
        let t = text_of(&p, &format!("{ERROR_PREFIX}{reason_2}"));
        assert!(in_warning(t), "ppp {ppp}");
        assert!(accept_disabled(&p, &bench.dialog), "ppp {ppp}");
    }
}

#[test]
fn row_10_click_on_accept_with_everything_valid_gives_accepted_with_the_values() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        bench.reactions.clear();
        bench.click(center_of(&p, accept_label(&bench.dialog)));
        let accepted = accepted(&bench.reactions);
        assert_eq!(accepted.len(), 1, "ppp {ppp}: {:?}", bench.reactions);
        assert_eq!(accepted[0].number("ancho"), Some(64), "ppp {ppp}");
        assert_eq!(accepted[0].number("alto"), Some(64), "ppp {ppp}");
        assert_eq!(accepted[0].choice("modo"), Some(0), "ppp {ppp}");
        assert_eq!(accepted[0].check("opaco"), Some(false), "ppp {ppp}");
        assert!(accepted[0].color("color").is_none(), "ppp {ppp}: oculto");
        assert_eq!(cancelled(&bench.reactions), 0);
    }
}

#[test]
fn row_10_click_on_cancel_gives_cancelled_and_never_delivers_values() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        bench.reactions.clear();
        bench.click(center_of(&p, cancel_label(&bench.dialog)));
        assert_eq!(cancelled(&bench.reactions), 1, "ppp {ppp}");
        assert!(accepted(&bench.reactions).is_empty(), "ppp {ppp}");
        // `Cancel` también con errores.
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "vacio_y_no_digito");
        let p = bench.rest_2_2();
        bench.reactions.clear();
        bench.click(center_of(&p, cancel_label(&bench.dialog)));
        assert_eq!(cancelled(&bench.reactions), 1, "ppp {ppp}: con errores");
    }
}

#[test]
fn row_10_the_hexadecimal_shows_its_error_inline_and_live() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        prepares(&mut bench, "hex");
        bench.rest_2_2();
        bench.focus_until(FocusTarget::Field("color"));
        let p = bench.writes("zz");
        assert!(bench.dialog.hex_pending("color"), "ppp {ppp}");
        let Some(Validity::Invalid(reason_2)) = bench.dialog.validity("color") else {
            panic!("ppp {ppp}: `zz` no es un color");
        };
        let t = text_of(&p, &format!("{ERROR_PREFIX}{reason_2}"));
        assert!(in_warning(t), "ppp {ppp}: error en línea del hexadecimal");
        assert!(accept_disabled(&p, &bench.dialog), "ppp {ppp}");
        // Uno válido, aún sin confirmar: sin error y `Accept` habilitado.
        let p = bench.writes("#12f");
        assert!(bench.dialog.hex_pending("color"), "ppp {ppp}");
        assert!(
            p.texts()
                .all(|(_, t)| !t.text_2.trim_start().starts_with(ERROR_PREFIX)),
            "ppp {ppp}: #12f es válido"
        );
    }
}

#[test]
fn row_10_the_area_fits_to_42_columns_advances_3_lines_per_notch_and_is_clipped() {
    for ppp in PPPS {
        let total = wrap_text(&text_of_the_report(&open(form())), 42).len();
        assert_eq!(total, 30);
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        // 4 filas, la primera línea al principio.
        let numbers = numbers_of_line(&p);
        assert_eq!(
            numbers,
            [1, 2, 3, 4],
            "ppp {ppp}: 4 filas desde el principio"
        );
        assert_eq!(bench.dialog.first_line("informe"), 0);

        // El puntero sobre la zona.
        let center = text_that_contains(&p, "linea 2").rect().center();
        bench.move_to(center);
        bench.wheel(true, 1);
        assert_eq!(
            bench.dialog.first_line("informe"),
            3,
            "ppp {ppp}: 3 por muesca"
        );
        bench.wheel(true, 2);
        assert_eq!(bench.dialog.first_line("informe"), 9, "ppp {ppp}");
        bench.wheel(false, 1);
        assert_eq!(bench.dialog.first_line("informe"), 6, "ppp {ppp}");
        let p = bench.rest_2_2();
        assert_eq!(numbers_of_line(&p), [7, 8, 9, 10], "ppp {ppp}");

        // Se recorta a `total − rows_2` y la última línea queda a la vista.
        bench.wheel(true, 1000);
        let maximum_2 = clamp_first_line(usize::MAX / 2, total, 4);
        assert_eq!(maximum_2, 26);
        assert_eq!(bench.dialog.first_line("informe"), maximum_2, "ppp {ppp}");
        let p = bench.rest_2_2();
        assert_eq!(numbers_of_line(&p), [27, 28, 29, 30], "ppp {ppp}");
        // Sigue sobre la zona aunque el cuerpo se hubiera movido: se vuelve a poner el puntero.
        let center = text_that_contains(&p, "linea 28").rect().center();
        bench.move_to(center);
        bench.wheel(false, 1000);
        assert_eq!(bench.dialog.first_line("informe"), 0, "ppp {ppp}");
    }
}

#[test]
fn row_10_a_long_report_line_is_cut_at_42_columns() {
    for ppp in PPPS {
        let spec = with_report(|_| format!("{}\nlinea 1\nlinea 2", "a".repeat(100)));
        let mut bench = Bench::ready(ppp, Window::Minimum, open(spec));
        let p = bench.rest_2_2();
        // Las filas de solo `a`, en el orden en que se pintan (de arriba abajo).
        let lengths: Vec<usize> = p
            .texts()
            .flat_map(|(_, t)| t.text_2.lines())
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.chars().all(|c| c == 'a'))
            .map(|l| l.chars().count())
            .collect();
        assert_eq!(
            lengths,
            [42, 42, 16],
            "ppp {ppp}: 100 letras en 42 columnas"
        );
    }
}

#[test]
fn row_10_the_keys_with_the_focus_in_the_area_scroll_it() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        bench.focus_until(FocusTarget::Zone("informe"));
        let maximum_2 = 26;
        assert_eq!(bench.dialog.first_line("informe"), 0);

        bench.presses_2(Key::ArrowDown, Modifiers::NONE);
        let after_arrow = bench.dialog.first_line("informe");
        assert!(after_arrow >= 1, "ppp {ppp}: la flecha abajo avanza");
        bench.presses_2(Key::ArrowUp, Modifiers::NONE);
        assert!(
            bench.dialog.first_line("informe") < after_arrow,
            "ppp {ppp}"
        );

        bench.presses_2(Key::PageDown, Modifiers::NONE);
        let after_page = bench.dialog.first_line("informe");
        assert!(
            after_page > 0 && after_page <= maximum_2,
            "ppp {ppp}: AvPág"
        );
        bench.presses_2(Key::End, Modifiers::NONE);
        assert_eq!(
            bench.dialog.first_line("informe"),
            maximum_2,
            "ppp {ppp}: Fin"
        );
        bench.presses_2(Key::PageUp, Modifiers::NONE);
        let after_re = bench.dialog.first_line("informe");
        assert!(after_re < maximum_2, "ppp {ppp}: RePág");
        bench.presses_2(Key::Home, Modifiers::NONE);
        assert_eq!(bench.dialog.first_line("informe"), 0, "ppp {ppp}: Inicio");
        // `Intro` en la zona no hace nada.
        bench.reactions.clear();
        bench.presses_2(Key::Enter, Modifiers::NONE);
        assert!(!closes(&bench.reactions), "ppp {ppp}: Intro en la zona");
    }
}

/// Un punto del velo (fuera del diálogo): dentro del botón de detrás, cerca de su esquina.
fn over_the_button(bench: &Bench) -> Pos2 {
    let button_2 = bench.behind.as_ref().expect("hay lo de detrás").button_2;
    assert!(
        button_2.is_positive(),
        "el botón de detrás no se ha pintado"
    );
    button_2.min + vec2(4.0, button_2.height() / 2.0)
}

/// Un punto del velo que solo toca el lienzo simulado (la esquina inferior derecha).
fn over_the_canvas(bench: &Bench) -> Pos2 {
    let p = bench.screen();
    pos2(p.x - 3.0, p.y - 3.0)
}

#[test]
fn row_10_the_veil_consumes_click_drag_and_wheel_without_closing_or_reaching_what_is_behind() {
    for ppp in PPPS {
        let mut bench = Bench::with_behind(ppp, Window::Minimum, open(form()));
        let rects = bench.rects();
        let dialog_2 = rect_pt(rects.dialog, ppp);
        let points = [over_the_button(&bench), over_the_canvas(&bench)];
        for p in points {
            assert!(!dialog_2.contains(p), "ppp {ppp}: {p:?} cae en el diálogo");
        }
        let before = bench.dialog.clone();
        bench.reactions.clear();
        bench.behind.as_mut().expect("detrás").a_zero();

        for p in points {
            // Clic.
            bench.frame(vec![move_pointer(p)]);
            bench.rest_2_2();
            bench.frame(vec![button_2(p, true)]);
            bench.frame(vec![button_2(p, false)]);
            bench.rest_2_2();
            // Arrastre.
            let q = p + vec2(3.0, 2.0);
            bench.frame(vec![button_2(p, true)]);
            bench.frame(vec![move_pointer(q)]);
            bench.frame(vec![button_2(q, false)]);
            bench.rest_2_2();
            // Rueda, hacia los dos lados.
            bench.wheel(true, 2);
            bench.wheel(false, 3);
            bench.rest_2_2();
        }

        assert!(
            bench.reactions.is_empty(),
            "ppp {ppp}: el velo no cierra ni cambia nada: {:?}",
            bench.reactions
        );
        assert_eq!(bench.dialog, before, "ppp {ppp}: el diálogo no cambia");
        let behind = bench.behind.as_ref().expect("detrás");
        assert!(
            behind.nothing_received(),
            "ppp {ppp}: lo de detrás recibió entrada: {behind:?}"
        );
        // Y el diálogo sigue ahí.
        let p = bench.rest_2_2();
        assert!(p.exact(accept_label(&bench.dialog)).is_some(), "ppp {ppp}");
    }
}

#[test]
fn row_10_without_the_dialog_what_is_behind_does_receive_the_click_and_the_wheel() {
    // El control de la prueba anterior: el mismo banco con el diálogo sin mostrar.
    for ppp in PPPS {
        let mut bench = Bench::without_dialog(ppp, Window::Minimum, open(form()));
        let over_button = over_the_button(&bench);
        let over_canvas = over_the_canvas(&bench);
        bench.behind.as_mut().expect("detrás").a_zero();

        bench.click(over_button);
        let d = bench.behind.as_ref().expect("detrás");
        assert!(
            d.clicks_button >= 1,
            "ppp {ppp}: el botón recibe el clic: {d:?}"
        );
        assert!(d.hovers_button >= 1, "ppp {ppp}");

        bench.move_to(over_canvas);
        bench.wheel(true, 2);
        bench.click(over_canvas);
        let d = bench.behind.as_ref().expect("detrás");
        assert!(
            d.wheels_canvas >= 1,
            "ppp {ppp}: el lienzo recibe la rueda: {d:?}"
        );
        assert!(
            d.clicks_canvas >= 1,
            "ppp {ppp}: el lienzo recibe el clic: {d:?}"
        );
    }
}

#[test]
fn row_10_the_veil_does_not_close_with_a_click_outside_and_a_click_inside_on_blank_does_not_either()
{
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let rects = bench.rects();
        let before = bench.dialog.clone();
        bench.reactions.clear();
        // Fuera del diálogo, en las cuatro esquinas de la ventana.
        let screen = bench.screen();
        for p in [
            pos2(1.0, 1.0),
            pos2(screen.x - 1.0, 1.0),
            pos2(1.0, screen.y - 1.0),
            pos2(screen.x - 1.0, screen.y - 1.0),
        ] {
            bench.click(p);
        }
        // Dentro, en el relleno izquierdo del cuerpo (ninguna pieza).
        let body = rect_pt(rects.body, ppp);
        bench.click(pos2(body.min.x + 3.0 * bench.u(), body.center().y));
        assert!(
            bench.reactions.is_empty(),
            "ppp {ppp}: {:?}",
            bench.reactions
        );
        assert_eq!(state_2(&bench.dialog), state_2(&before), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: geometría
// ---------------------------------------------------------------------------------------------

fn checks_geometry(bench: &mut Bench, label_2: &str) {
    let ppp = bench.ppp;
    let window = bench.window;
    let p = bench.rest_2_2();
    let rects = bench.rects();
    let location = format!("ppp {ppp} {window:?} {label_2}");
    let (_, background_2) =
        fill_of_the_dialog(&p).unwrap_or_else(|| panic!("{location}: sin fondo SURFACE_1"));
    assert!(
        matches(background_2.rect, rects.dialog, ppp),
        "{location}: el diálogo pintado es {:?} y el de dialog_layout {:?}",
        background_2.rect,
        rects.dialog
    );

    // Título, botones y motivo, en sus rectángulos.
    let title = text_of(&p, bench.dialog.spec().title).rect();
    assert!(
        inside(title, rects.title, ppp),
        "{location}: título {title:?} fuera de {:?}",
        rects.title
    );
    let accept = text_of(&p, accept_label(&bench.dialog)).rect();
    assert!(
        inside(accept, rects.accept, ppp),
        "{location}: Aceptar {accept:?}"
    );
    let cancel = text_of(&p, cancel_label(&bench.dialog)).rect();
    let rect_cancel = rects.cancel.expect("hay Cancelar");
    assert!(
        inside(cancel, rect_cancel, ppp),
        "{location}: Cancelar {cancel:?}"
    );
    for (expected_2, name_2) in [(rects.accept, "Aceptar"), (rect_cancel, "Cancelar")] {
        assert!(
            p.fills().any(|(_, r)| matches(r.rect, expected_2, ppp)),
            "{location}: ningún relleno es el botón {name_2} ({expected_2:?})"
        );
    }
    if let Some(reason) = bench.dialog.reason() {
        let reason_2 = text_of(&p, &format!("{ERROR_PREFIX}{reason}")).rect();
        assert!(
            inside(reason_2, rects.reason, ppp),
            "{location}: motivo {reason_2:?} fuera de {:?}",
            rects.reason
        );
    }

    // Los botones de 68n × 16n con 4n de hueco, a la derecha del pie y dentro de él.
    let n = bench.n();
    assert_eq!(
        (rects.accept.width, rects.accept.height),
        (68 * n, 16 * n),
        "{location}"
    );
    assert_eq!(
        (rect_cancel.width, rect_cancel.height),
        (68 * n, 16 * n),
        "{location}"
    );
    assert_eq!(
        rect_cancel.x - (rects.accept.x + rects.accept.width),
        4 * n,
        "{location}"
    );

    // Ningún texto ni forma sale del diálogo salvo el velo; el cuerpo no invade título ni pie.
    for (_, t) in p.texts() {
        let visible = t.rect().intersect(t.clip);
        if t.text_2.trim().is_empty() || !visible.is_positive() {
            continue;
        }
        assert!(
            inside(visible, rects.title, ppp)
                || inside(visible, rects.body, ppp)
                || inside(visible, rects.footer, ppp),
            "{location}: el texto {:?} ({visible:?}) no cabe en el título, el cuerpo ni el pie",
            t.text_2
        );
    }
    for (_, r) in p.fills() {
        if r.fill == Color32::from_black_alpha(128) {
            continue;
        }
        let visible = r.rect.intersect(r.clip);
        if !visible.is_positive() {
            continue;
        }
        assert!(
            inside(visible, rects.dialog, ppp),
            "{location}: un relleno {:?} sale del diálogo {:?}",
            visible,
            rects.dialog
        );
    }
    for (_, t) in p.strokes() {
        let visible = t.rect.expand(t.width_2 / 2.0).intersect(t.clip);
        if !visible.is_positive() {
            continue;
        }
        assert!(
            inside(visible, rects.dialog, ppp),
            "{location}: un trazo {visible:?} sale del diálogo"
        );
    }
}

#[test]
fn row_11_the_painted_rectangles_are_those_of_dialog_layout() {
    for ppp in PPPS {
        for window in WINDOWS {
            for scenario in ["defecto", "vacio_y_no_digito", "hex", "check_falla"] {
                let mut bench = Bench::new_2(ppp, window, open(form()));
                bench.rest_2_2();
                prepares(&mut bench, scenario);
                checks_geometry(&mut bench, scenario);
            }
        }
    }
}

#[test]
fn row_11_with_huge_content_the_geometry_also_adds_up() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::new_2(ppp, window, open(huge(60)));
            bench.rest_2_2();
            checks_geometry(&mut bench, "huge");
            bench.dialog.set_scroll_u(100);
            checks_geometry(&mut bench, "huge con scroll 100");
        }
    }
}

#[test]
fn row_11_the_veil_covers_the_whole_window_with_alpha_128_below_the_dialog() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::ready(ppp, window, open(form()));
            let p = bench.rest_2_2();
            let screen = Rect::from_min_size(Pos2::ZERO, bench.screen());
            let veils = glaze(&p);
            assert!(
                !veils.is_empty(),
                "ppp {ppp} {window:?}: no hay velo (negro de alfa 128)"
            );
            assert!(
                veils
                    .iter()
                    .any(|(_, r)| r.rect.expand(0.01).contains_rect(screen)),
                "ppp {ppp} {window:?}: el velo no cubre la ventana: {veils:?}"
            );
            let (i_dialog, _) = fill_of_the_dialog(&p).expect("el diálogo se pinta");
            assert!(
                veils.iter().all(|(i, _)| *i < i_dialog),
                "ppp {ppp} {window:?}: el velo va debajo del diálogo"
            );
        }
    }
}

#[test]
fn row_11_the_dialog_stays_centered_with_a_margin_of_8_u_in_the_minimum_window() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
        let p = bench.rest_2_2();
        let n = bench.n();
        let (_, background_2) = fill_of_the_dialog(&p).expect("el diálogo se pinta");
        let (width_2, height_2) = measures(ppp, Window::Minimum).0;
        let rect = background_2.rect;
        let (left, right) = (rect.min.x * ppp, width_2 as f32 - rect.max.x * ppp);
        let (arr, aba) = (rect.min.y * ppp, height_2 as f32 - rect.max.y * ppp);
        let tol = tolerance_of_egui(ppp) + 1.0;
        assert!((left - right).abs() <= tol, "ppp {ppp}: {left} y {right}");
        assert!((arr - aba).abs() <= tol, "ppp {ppp}: {arr} y {aba}");
        assert!(
            left >= 8.0 * n as f32 - tol && arr >= 8.0 * n as f32 - tol,
            "ppp {ppp}: margen"
        );
        assert!(
            (rect.width() * ppp - 330.0 * n as f32).abs() <= tol,
            "ppp {ppp}: ancho 330 u"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: contenido enorme y rueda
// ---------------------------------------------------------------------------------------------

/// El puntero en el relleno izquierdo del cuerpo (sin pieza debajo).
fn pointer_on_the_body(bench: &mut Bench) {
    let body = rect_pt(bench.rects().body, bench.ppp);
    let pos = pos2(body.min.x + 3.0 * bench.u(), body.center().y);
    bench.move_to(pos);
}

/// Posiciones (puntos) de los textos de título y pie.
fn fixed(p: &Paint, dialog: &Dialog) -> Vec<(String, Pos2)> {
    let mut v = Vec::new();
    for label_2 in [
        dialog.spec().title,
        accept_label(dialog),
        cancel_label(dialog),
    ] {
        let t = text_of(p, label_2);
        v.push((label_2.to_owned(), t.pos));
    }
    v
}

/// Cada texto de `after_2` (visible, en el cuerpo) estaba `delta` puntos más abajo antes, salvo los
/// que acaban de entrar por abajo. Devuelve cuántos se han podido contrastar.
fn shifted(before: &Paint, after_2: &Paint, body: Rect, delta: f32, tol: f32) -> usize {
    let visible_2 = |p: &Paint| -> Vec<(String, Pos2)> {
        p.texts()
            .map(|(_, t)| t)
            .filter(|t| {
                let v = t.rect().intersect(t.clip);
                v.is_positive() && body.expand(tol).contains_rect(v)
            })
            .map(|t| (t.text_2.clone(), t.pos))
            .collect()
    };
    let (before, after_2) = (visible_2(before), visible_2(after_2));
    let mut contrasted = 0;
    for (text_2, pos) in &after_2 {
        let was = before.iter().any(|(t, q)| {
            t == text_2 && (q.x - pos.x).abs() <= tol && (q.y - (pos.y + delta)).abs() <= tol
        });
        if was {
            contrasted += 1;
        } else {
            // Solo puede ser algo que entra por abajo.
            assert!(
                pos.y + delta > body.max.y - tol,
                "el texto {text_2:?} en {pos:?} no estaba {delta} pt más abajo antes ni entra por abajo"
            );
        }
    }
    contrasted
}

#[test]
fn row_11_the_wheel_scrolls_the_body_42_u_per_notch_without_passing_max_scroll() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::ready(ppp, window, open(huge(60)));
            let rects = bench.rects();
            let maximum_2 = rects.max_scroll_u;
            assert!(
                maximum_2 > 5 * 42,
                "ppp {ppp} {window:?}: contenido enorme: {maximum_2}"
            );
            assert_eq!(bench.dialog.scroll_u(), 0);
            pointer_on_the_body(&mut bench);

            let mut expected_2 = 0;
            for (below, notches) in [(true, 1), (true, 3), (false, 1), (true, 2)] {
                bench.wheel(below, notches);
                let delta = 42 * notches as i64;
                expected_2 = if below {
                    (expected_2 + delta).min(maximum_2)
                } else {
                    (expected_2 - delta).max(0)
                };
                assert_eq!(
                    bench.dialog.scroll_u(),
                    expected_2,
                    "ppp {ppp} {window:?}: abajo={below} muescas={notches}"
                );
                bench.rest_2_2();
                assert_eq!(
                    bench.dialog.scroll_u(),
                    expected_2,
                    "ppp {ppp} {window:?}: se mantiene"
                );
            }

            // Recorte: ni por abajo ni por arriba.
            bench.wheel(true, 10_000);
            assert_eq!(
                bench.dialog.scroll_u(),
                maximum_2,
                "ppp {ppp} {window:?}: tope"
            );
            bench.wheel(true, 1);
            assert_eq!(
                bench.dialog.scroll_u(),
                maximum_2,
                "ppp {ppp} {window:?}: no pasa"
            );
            bench.wheel(false, 10_000);
            assert_eq!(bench.dialog.scroll_u(), 0, "ppp {ppp} {window:?}: suelo");
        }
    }
}

#[test]
fn row_11_when_scrolling_the_body_what_is_visible_moves_the_same_and_title_and_footer_do_not_move()
{
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::ready(ppp, window, open(huge(60)));
            pointer_on_the_body(&mut bench);
            let body = rect_pt(bench.rects().body, ppp);
            let u = bench.u();
            let tol = tolerance_of_egui(ppp) / ppp + 0.01;
            for notches in [1usize, 2] {
                let before = bench.rest_2_2();
                let fixed_before = fixed(&before, &bench.dialog);
                let scroll_before = bench.dialog.scroll_u();
                bench.wheel(true, notches);
                let after_2 = bench.rest_2_2();
                let moved = bench.dialog.scroll_u() - scroll_before;
                assert_eq!(moved, 42 * notches as i64, "ppp {ppp} {window:?}");
                assert_eq!(
                    fixed(&after_2, &bench.dialog),
                    fixed_before,
                    "ppp {ppp} {window:?}: título y pie fijos"
                );
                let contrasted = shifted(&before, &after_2, body, moved as f32 * u, tol);
                assert!(
                    contrasted >= 2,
                    "ppp {ppp} {window:?}: solo se contrastan {contrasted} textos del cuerpo"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: foco y teclado
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_initial_focus_is_initial_focus() {
    for ppp in PPPS {
        for window in WINDOWS {
            for spec in [form(), huge(20)] {
                let dialog = open(spec);
                let expected_2 = initial_focus(&dialog);
                let mut bench = Bench::new_2(ppp, window, dialog);
                assert_eq!(bench.focus, None);
                bench.rest_2_2();
                assert_eq!(
                    bench.focus,
                    Some(expected_2),
                    "ppp {ppp} {window:?}: el foco inicial"
                );
                // El foco se pide una sola vez: otro fotograma no lo cambia.
                bench.rest_2_2();
                assert_eq!(bench.focus, Some(expected_2), "ppp {ppp} {window:?}");
            }
        }
    }
}

/// Un ciclo entero de `Tab` (o `Mayús`+`Tab`) más una vuelta extra: cada paso es el siguiente (o el
/// anterior) de `focus_order`.
fn walks(bench: &mut Bench, back: bool, with_ring: bool) {
    let ppp = bench.ppp;
    let window = bench.window;
    let order = focus_order(&bench.dialog);
    assert!(order.len() >= 4, "{order:?}");
    let start_2 = order
        .iter()
        .position(|t| Some(*t) == bench.focus)
        .expect("el foco inicial está en el orden");
    let u = bench.u();
    let length = order.len();
    for step in 1..=length + 2 {
        let p = bench.tab(back);
        let expected_2 = if back {
            order[(start_2 + length * 3 - step) % length]
        } else {
            order[(start_2 + step) % length]
        };
        assert_eq!(
            bench.focus,
            Some(expected_2),
            "ppp {ppp} {window:?} atras={back}: paso {step}"
        );
        if with_ring {
            assert!(
                !rings_2(&p, u).is_empty(),
                "ppp {ppp} {window:?}: el anillo de {expected_2:?} no está a la vista"
            );
        }
        if let Some(d) = bench.behind.as_ref() {
            let focus = bench.focus_of_egui();
            assert!(
                focus != Some(d.button_id) && focus != Some(d.text_id),
                "ppp {ppp} {window:?}: el Tab llegó a lo de detrás en el paso {step}"
            );
        }
        let rects = bench.rects();
        assert!(
            (0..=rects.max_scroll_u).contains(&bench.dialog.scroll_u()),
            "ppp {ppp} {window:?}: scroll {} fuera de 0..={}",
            bench.dialog.scroll_u(),
            rects.max_scroll_u
        );
    }
}

#[test]
fn row_11_tab_walks_focus_order_in_order_wraps_around_and_the_ring_is_seen() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::ready(ppp, window, open(form()));
            walks(&mut bench, false, true);
        }
    }
}

#[test]
fn row_11_shift_tab_walks_the_order_backwards_and_wraps_around() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::ready(ppp, window, open(form()));
            walks(&mut bench, true, true);
        }
    }
}

#[test]
fn row_11_tab_with_the_hexadecimal_visible_and_with_errors_follows_the_order_without_accepting() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, Window::Minimum, open(form()));
        bench.rest_2_2();
        prepares(&mut bench, "hex_invalido");
        bench.rest_2_2();
        let order = focus_order(&bench.dialog);
        assert!(order.contains(&FocusTarget::Field("color")), "ppp {ppp}");
        assert!(
            !order.contains(&FocusTarget::Accept),
            "ppp {ppp}: con errores, Aceptar no se enfoca"
        );
        walks(&mut bench, false, true);
        walks(&mut bench, true, true);
    }
}

#[test]
fn row_11_tab_with_huge_content_keeps_the_focus_in_view_and_the_scroll_in_range() {
    for ppp in PPPS {
        for window in WINDOWS {
            let mut bench = Bench::ready(ppp, window, open(huge(60)));
            let start_2 = bench.focus;
            let mut maximum_seen = 0;
            let order = focus_order(&bench.dialog);
            for _ in 0..order.len() {
                bench.tab(false);
                maximum_seen = maximum_seen.max(bench.dialog.scroll_u());
            }
            assert_eq!(
                bench.focus, start_2,
                "ppp {ppp} {window:?}: un ciclo entero vuelve al inicio"
            );
            assert!(
                maximum_seen > 0 || bench.rects().max_scroll_u == 0,
                "ppp {ppp} {window:?}: con 60 casillas el Tab tiene que desplazar el cuerpo"
            );
            walks(&mut bench, false, true);
        }
    }
}

#[test]
fn row_11_tab_and_shift_tab_never_reach_what_is_behind_and_without_a_dialog_they_do() {
    for ppp in PPPS {
        let mut bench = Bench::with_behind(ppp, Window::Minimum, open(form()));
        walks(&mut bench, false, false);
        walks(&mut bench, true, false);

        // Control: sin el diálogo, la tabulación sí llega al botón de detrás.
        let mut bench = Bench::without_dialog(ppp, Window::Minimum, open(form()));
        let button_2 = bench.behind.as_ref().expect("detrás").button_id;
        let mut arrive = false;
        for _ in 0..6 {
            bench.tab(false);
            arrive |= bench.focus_of_egui() == Some(button_2);
        }
        assert!(arrive, "ppp {ppp}: el control no llega al botón con Tab");
    }
}

#[test]
fn row_11_enter_and_space_on_cancel_cancel_and_on_accept_accept() {
    for ppp in PPPS {
        for key_button in [Key::Enter, Key::Space] {
            let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
            bench.focus_until(FocusTarget::Cancel);
            bench.reactions.clear();
            bench.presses_2(key_button, Modifiers::NONE);
            assert_eq!(
                cancelled(&bench.reactions),
                1,
                "ppp {ppp} {key_button:?}: con el foco en Cancelar cancela una vez: {:?}",
                bench.reactions
            );
            assert!(
                accepted(&bench.reactions).is_empty(),
                "ppp {ppp} {key_button:?}"
            );

            let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
            bench.focus_until(FocusTarget::Accept);
            bench.reactions.clear();
            bench.presses_2(key_button, Modifiers::NONE);
            let accepted = accepted(&bench.reactions);
            assert_eq!(
                accepted.len(),
                1,
                "ppp {ppp} {key_button:?}: con el foco en Aceptar acepta una vez: {:?}",
                bench.reactions
            );
            assert_eq!(accepted[0].number("ancho"), Some(64), "ppp {ppp}");
            assert_eq!(cancelled(&bench.reactions), 0, "ppp {ppp} {key_button:?}");
        }
    }
}

#[test]
fn row_11_escape_cancels_with_the_focus_on_any_element() {
    for ppp in PPPS {
        let order = focus_order(&open(form()));
        for goal in order {
            let mut bench = Bench::ready(ppp, Window::Minimum, open(form()));
            bench.focus_until(goal);
            bench.reactions.clear();
            bench.presses_2(Key::Escape, Modifiers::NONE);
            assert_eq!(
                cancelled(&bench.reactions),
                1,
                "ppp {ppp}: Escape con el foco en {goal:?}"
            );
            assert!(accepted(&bench.reactions).is_empty(), "ppp {ppp} {goal:?}");
        }
    }
}

#[test]
fn row_11_escape_with_a_pending_hexadecimal_reverts_it_and_the_next_one_cancels() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, Window::Minimum, open(form()));
        bench.rest_2_2();
        prepares(&mut bench, "hex");
        bench.rest_2_2();
        bench.focus_until(FocusTarget::Field("color"));
        let confirmed = bench.dialog.text("color").expect("el campo se muestra");
        bench.writes("#12");
        assert!(bench.dialog.hex_pending("color"), "ppp {ppp}");
        assert_eq!(bench.dialog.text("color"), Some("#12".into()), "ppp {ppp}");

        bench.reactions.clear();
        bench.presses_2(Key::Escape, Modifiers::NONE);
        assert!(
            !closes(&bench.reactions),
            "ppp {ppp}: el primer Escape solo revierte"
        );
        assert!(
            !bench.dialog.hex_pending("color"),
            "ppp {ppp}: ya no hay pendiente"
        );
        assert_eq!(
            bench.dialog.text("color"),
            Some(confirmed),
            "ppp {ppp}: revertido"
        );

        // Se suelta la tecla (si no, egui toma la siguiente pulsación por una repetición).
        bench.frame(vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        bench.presses_2(Key::Escape, Modifiers::NONE);
        assert_eq!(
            cancelled(&bench.reactions),
            1,
            "ppp {ppp}: el segundo cancela"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: cambio de ppp y de tamaño con el diálogo abierto
// ---------------------------------------------------------------------------------------------

/// Valores escritos, un hexadecimal pendiente y el foco puesto.
fn with_state(ppp: f32, window: Window) -> Bench {
    let mut bench = Bench::new_2(ppp, window, open(form()));
    bench.rest_2_2();
    number(&mut bench, "ancho", "300");
    bench.event(TestEvent::Choice {
        field: "modo",
        index: 2,
    });
    bench.event(TestEvent::Hex {
        field: "color",
        text: "#12".into(),
    });
    bench.event(TestEvent::Check {
        field: "opaco",
        on: true,
    });
    bench.rest_2_2();
    bench
}

#[test]
fn row_11_a_ppp_and_window_change_repositions_without_losing_values_or_focus() {
    let changes_2 = [
        (1.0, 2.0),
        (2.0, 1.25),
        (1.5, 3.0),
        (3.0, 1.0),
        (1.25, 1.5),
        (2.0, 3.0),
    ];
    let windows = [
        (Window::Minimum, Window::Minimum),
        (Window::Minimum, Window::Large),
        (Window::Large, Window::Minimum),
    ];
    for (before, after_2) in changes_2 {
        for (window_before, window_after) in windows {
            let mut bench = with_state(before, window_before);
            let saved = state_2(&bench.dialog);
            let focus = bench.focus;
            assert!(focus.is_some());
            bench.reactions.clear();

            bench.changes(after_2, window_after);
            bench.rest_2_2();
            let location = format!("{before} → {after_2}, {window_before:?} → {window_after:?}");
            assert_eq!(
                state_2(&bench.dialog),
                saved,
                "{location}: los valores no cambian"
            );
            assert_eq!(bench.focus, focus, "{location}: el foco no cambia");
            assert!(
                bench.dialog.hex_pending("color"),
                "{location}: el pendiente sigue"
            );
            assert!(
                bench.reactions.is_empty(),
                "{location}: {:?}",
                bench.reactions
            );
            // Sigue centrado y con la geometría de la escala nueva.
            checks_geometry(&mut bench, &location);
        }
    }
}

#[test]
fn row_11_the_scroll_is_kept_in_u_when_changing_scale_or_size() {
    let walks_2 = [
        (1.0, Window::Minimum),
        (2.0, Window::Minimum),
        (3.0, Window::Minimum),
        (1.0, Window::Large),
        (1.5, Window::Minimum),
        (1.25, Window::Minimum),
    ];
    let mut bench = Bench::new_2(walks_2[0].0, walks_2[0].1, open(huge(60)));
    bench.rest_2_2();
    bench.dialog.set_scroll_u(100);
    bench.rest_2_2();
    assert_eq!(bench.dialog.scroll_u(), 100);
    for (ppp, window) in &walks_2[1..] {
        bench.changes(*ppp, *window);
        bench.rest_2_2();
        let maximum_2 = bench.rects().max_scroll_u;
        assert_eq!(
            bench.dialog.scroll_u(),
            100.min(maximum_2),
            "ppp {ppp} {window:?}: el desplazamiento se guarda en u (máximo {maximum_2})"
        );
    }
}

#[test]
fn row_11_a_ppp_change_moves_neither_the_focus_nor_the_text_of_a_focused_field() {
    // Un campo con el foco y texto escrito: el cambio de escala no lo suelta ni lo toca.
    let mut bench = Bench::ready(1.0, Window::Minimum, open(form()));
    bench.writes("512");
    assert_eq!(bench.focus, Some(FocusTarget::Field("ancho")));
    for ppp in [2.0, 1.25, 3.0, 1.5, 1.0] {
        bench.changes(ppp, Window::Minimum);
        bench.rest_2_2();
        assert_eq!(bench.focus, Some(FocusTarget::Field("ancho")), "ppp {ppp}");
        assert_eq!(bench.dialog.text("ancho"), Some("512".into()), "ppp {ppp}");
        // Y se puede seguir tecleando.
        bench.writes("77");
        assert_eq!(bench.dialog.text("ancho"), Some("77".into()), "ppp {ppp}");
        bench.writes("512");
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 20b, parte 2: marcador redondo de las opciones y borde de la casilla
// (specs/hito-2-motivo-y-marcadores.md, D5; filas 9, 10 y 11; propiedad 4)
// ---------------------------------------------------------------------------------------------
//
// En ROJO hasta la implementación: `px_ui::dialog_view::TOGGLE_EDGE` todavía no existe (este
// fichero no compila hasta entonces), `draw_toggle` pinta las opciones como cuadrados con `EDGE` y
// `CONTRAST_PAIRS` tiene 10 pares.
//
// Lo que se lee del pintado: los círculos (`Shape::Circle`: centro, radio, relleno y trazo) y los
// rectángulos (`Shape::Rect`) tal como los pinta `show_dialog`, sin comparar píxeles. El helper
// `order::paint_2` solo trata un círculo como «relleno de caja» y descarta su trazo y su centro,
// así que aquí hay un lector aparte (`marks`) que sale del mismo `Vec<ClippedShape>`.
//
// Lo que fija D5 y se comprueba, a las cinco escalas y en 960 × 720 (a la mínima el cuerpo se
// desplaza y parte de las filas queda recortada):
// - Opción: un aro (círculo con trazo `TOGGLE_EDGE` de 1 u, relleno `FIELD_FILL`, diámetro exterior
//   de 10 u = 2 · radio + trazo, centro en `x = PAD_U + 2 + 5 u` desde el borde del diálogo y en
//   `y = rótulo + 7 u`); la elegida lleva además un punto `ACCENT` de 6 u de diámetro, concéntrico.
//   Un punto por grupo, nunca más.
// - Casilla: rectángulo de 10 u con trazo `TOGGLE_EDGE` de 1 u y relleno `FIELD_FILL`; marcada,
//   un cuadrado `ACCENT` de 6 u centrado en la caja. Nunca un círculo en su fila.
// - Anillo de foco: el de siempre (trazo `ACCENT` de 2 u, `Inside`, alrededor de la fila entera:
//   `x = PAD_U`, `y = rótulo − 1 u`, `16 u` de alto y `18 u + 7 u · letters` de ancho), igual en
//   opción y en casilla.
// - Se admite que el aro y su relleno sean dos círculos (relleno debajo, aro encima, concéntricos)
//   en vez de uno con relleno y trazo; no se fija el radio por separado, sino el diámetro exterior.

use px_ui::dialog_view::{DIALOG_FILL, FIELD_FILL, TOGGLE_EDGE};

mod marks {
    //! Círculos y rectángulos pintados, en orden de pintado, con su relleno y su trazo.

    use egui::epaint::{ClippedShape, Shape};
    use egui::{Color32, Pos2, Rect};

    #[derive(Clone, Copy, Debug)]
    pub struct Disk {
        pub center: Pos2,
        pub radius: f32,
        pub fill: Color32,
        /// `TRANSPARENT` y `0.0` si no tiene trazo visible.
        pub stroke_color: Color32,
        pub stroke_width: f32,
        pub order: usize,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Toolbox {
        pub rect: Rect,
        pub fill: Color32,
        pub stroke_color: Color32,
        pub stroke_width: f32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct TestMarks {
        pub disks: Vec<Disk>,
        pub boxes: Vec<Toolbox>,
        next: usize,
    }

    fn stroke_2(s: egui::Stroke) -> (Color32, f32) {
        if s.width > 0.0 && s.color != Color32::TRANSPARENT {
            (s.color, s.width)
        } else {
            (Color32::TRANSPARENT, 0.0)
        }
    }

    fn collects(shape: &Shape, outside: &mut TestMarks) {
        match shape {
            Shape::Circle(c) => {
                let (stroke_color, stroke_width) = stroke_2(c.stroke);
                outside.disks.push(Disk {
                    center: c.center,
                    radius: c.radius,
                    fill: c.fill,
                    stroke_color,
                    stroke_width,
                    order: outside.next,
                });
                outside.next += 1;
            }
            Shape::Rect(r) => {
                let (stroke_color, stroke_width) = stroke_2(r.stroke);
                outside.boxes.push(Toolbox {
                    rect: r.rect,
                    fill: r.fill,
                    stroke_color,
                    stroke_width,
                });
                outside.next += 1;
            }
            Shape::Vec(shapes_2) => {
                for shape_2 in shapes_2 {
                    collects(shape_2, outside);
                }
            }
            _ => {}
        }
    }

    pub fn marks_of(shapes_2: &[ClippedShape]) -> TestMarks {
        let mut outside = TestMarks::default();
        for clipped in shapes_2 {
            collects(&clipped.shape, &mut outside);
        }
        outside
    }
}

use marks::{Disk, TestMarks, Toolbox, marks_of};

/// Un fotograma: lo pintado (textos, trazos y rellenos) y los círculos y rectángulos crudos.
struct Scene {
    p: Paint,
    m: TestMarks,
}

impl Bench {
    /// Como `frame`, pero devuelve las formas sin procesar (para leer círculos).
    fn frame_raw(&mut self, events: Vec<Event>) -> Vec<egui::epaint::ClippedShape> {
        let screen = self.screen();
        let mut reaction = Reaction::None;
        let shapes_2 = {
            let Bench {
                ctx,
                dialog,
                focus,
                behind,
                swatch_dialog,
                ..
            } = &mut *self;
            let swatch = *swatch_dialog;
            frame(ctx, screen, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                if let Some(d) = behind.as_mut() {
                    draws_behind(&c, d, screen);
                }
                if swatch {
                    reaction = show_dialog(&c, dialog, focus);
                }
            })
        };
        if !matches!(reaction, Reaction::None) {
            self.reactions.push(reaction);
        }
        shapes_2
    }

    /// Tres fotogramas de reposo y lo pintado en el último, con los círculos.
    fn scene(&mut self) -> Scene {
        self.frame_raw(vec![]);
        self.frame_raw(vec![]);
        let shapes_2 = self.frame_raw(vec![]);
        Scene {
            p: paint_2(&shapes_2),
            m: marks_of(&shapes_2),
        }
    }
}

/// Tolerancia de las medidas: 5 % de una `u` (el redondeo de píxeles de egui es mucho menor).
fn near_u(a: f32, b: f32, u: f32) -> bool {
    (a - b).abs() <= 0.05 * u
}

fn same_center(a: Pos2, b: Pos2, u: f32) -> bool {
    near_u(a.x, b.x, u) && near_u(a.y, b.y, u)
}

/// `x` (puntos) del centro de los marcadores: `PAD_U + 2 + 5 u` desde el borde del diálogo.
fn x_marks(b: &Bench) -> f32 {
    in_points(
        b.ppp,
        b.rects().dialog.x + (px_ui::dialog_layout::PAD_U + 2 + 5) * b.n(),
        0,
    )
    .x
}

/// Los aros (círculos con trazo `TOGGLE_EDGE`), de arriba abajo.
fn rings(m: &TestMarks) -> Vec<Disk> {
    let mut v: Vec<Disk> = m
        .disks
        .iter()
        .copied()
        .filter(|d| d.stroke_color == TOGGLE_EDGE)
        .collect();
    v.sort_by(|a, b| a.center.y.total_cmp(&b.center.y));
    v
}

/// Los puntos: círculos rellenos de `ACCENT`, de arriba abajo.
fn points(m: &TestMarks) -> Vec<Disk> {
    let mut v: Vec<Disk> = m
        .disks
        .iter()
        .copied()
        .filter(|d| d.fill == ACCENT)
        .collect();
    v.sort_by(|a, b| a.center.y.total_cmp(&b.center.y));
    v
}

/// Las cajas de casilla: rectángulos con trazo `TOGGLE_EDGE` y de aspecto de caja de marca (hasta
/// 20 u de lado; `TOGGLE_EDGE` es `TEXT_DIM` y otros trazos grandes, como el marco del diálogo,
/// pueden tener ese color), de arriba abajo.
fn boxes_2(m: &TestMarks, u: f32) -> Vec<Toolbox> {
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

/// Las marcas cuadradas: rectángulos de relleno `ACCENT` y sin trazo, de como mucho 10 u, con el
/// centro a menos de 5 u en vertical de `y`.
fn marks_square(m: &TestMarks, u: f32, y: f32) -> Vec<Toolbox> {
    m.boxes
        .iter()
        .copied()
        .filter(|c| c.fill == ACCENT && c.stroke_width == 0.0)
        .filter(|c| c.rect.width() <= 10.0 * u + 0.05 && c.rect.height() <= 10.0 * u + 0.05)
        .filter(|c| (c.rect.center().y - y).abs() <= 5.0 * u)
        .collect()
}

/// El relleno de un aro: el suyo o, si no tiene, el del último círculo relleno anterior y concéntrico.
fn fill_of_ring(m: &TestMarks, ring: &Disk) -> Color32 {
    if ring.fill != Color32::TRANSPARENT {
        return ring.fill;
    }
    m.disks
        .iter()
        .filter(|d| d.order < ring.order && d.fill != Color32::TRANSPARENT)
        .rfind(|d| (d.center - ring.center).length() < 0.01)
        .map_or(Color32::TRANSPARENT, |d| d.fill)
}

/// Opciones del grupo `mode_2` de `form()`: un aro por opción en su sitio, un único punto y solo
/// en la elegida, ninguna opción cuadrada.
fn asserts_options(b: &Bench, esc: &Scene, location: &str) {
    let u = b.u();
    let options_2 = options_of(&b.dialog, "modo");
    let chosen_2 = b.dialog.chosen("modo").expect("el grupo modo");
    let rings = rings(&esc.m);
    assert_eq!(
        rings.len(),
        options_2.len(),
        "{location}: un aro por opción; círculos pintados {:?}",
        esc.m.disks
    );
    let x = x_marks(b);
    let all_the_points = points(&esc.m);
    for (i, option) in options_2.iter().enumerate() {
        let ring = rings[i];
        let y = text_of(&esc.p, option).pos.y + 7.0 * u;
        assert!(
            near_u(ring.center.x, x, u) && near_u(ring.center.y, y, u),
            "{location}: el aro de {option:?} está en {:?} y debía estar en ({x}, {y})",
            ring.center
        );
        assert!(
            near_u(2.0 * ring.radius + ring.stroke_width, 10.0 * u, u),
            "{location}: el aro de {option:?} mide {} de diámetro exterior y debía medir {}",
            2.0 * ring.radius + ring.stroke_width,
            10.0 * u
        );
        assert!(
            near_u(ring.stroke_width, u, u),
            "{location}: el trazo del aro de {option:?} mide {} y debía medir 1 u = {u}",
            ring.stroke_width
        );
        assert_eq!(
            fill_of_ring(&esc.m, &ring),
            FIELD_FILL,
            "{location}: el aro de {option:?} se rellena con el color del campo"
        );
        let here: Vec<&Disk> = all_the_points
            .iter()
            .filter(|p| same_center(p.center, ring.center, u))
            .collect();
        assert_eq!(
            here.len(),
            usize::from(i == chosen_2),
            "{location}: la opción {i} (elegida: {chosen_2}) lleva {} puntos",
            here.len()
        );
        if let Some(point) = here.first() {
            assert!(
                near_u(2.0 * point.radius, 6.0 * u, u),
                "{location}: el punto mide {} y debía medir {}",
                2.0 * point.radius,
                6.0 * u
            );
            assert_eq!(
                point.stroke_width, 0.0,
                "{location}: el punto no lleva trazo"
            );
        }
    }
    assert_eq!(
        all_the_points.len(),
        1,
        "{location}: exactamente un punto por grupo (el formulario tiene un grupo): {all_the_points:?}"
    );
    // Ninguna opción se pinta con una caja cuadrada de borde.
    for toolbox_2 in boxes_2(&esc.m, u) {
        for option in options_2 {
            let y = text_of(&esc.p, option).pos.y + 7.0 * u;
            assert!(
                (toolbox_2.rect.center().y - y).abs() > 3.0 * u,
                "{location}: la opción {option:?} se pinta con un cuadrado de borde {:?}",
                toolbox_2.rect
            );
        }
    }
}

/// La casilla `opaque`: una caja de 10 u con borde `TOGGLE_EDGE`, marca cuadrada de 6 u solo si
/// está marcada y ningún círculo en su fila.
fn asserts_box(b: &Bench, esc: &Scene, location: &str) {
    let u = b.u();
    let marked = b.dialog.checked("opaco").expect("la casilla opaco");
    let boxes = boxes_2(&esc.m, u);
    assert_eq!(
        boxes.len(),
        1,
        "{location}: una caja con borde TOGGLE_EDGE; rectángulos de caja {:?}",
        esc.m
            .boxes
            .iter()
            .filter(|c| c.rect.width() <= 20.0 * u)
            .collect::<Vec<_>>()
    );
    let toolbox_2 = boxes[0];
    let y = text_of(&esc.p, "Opaco").pos.y + 7.0 * u;
    let center = toolbox_2.rect.center();
    assert!(
        near_u(toolbox_2.rect.width(), 10.0 * u, u) && near_u(toolbox_2.rect.height(), 10.0 * u, u),
        "{location}: la caja mide {:?} y debía ser un cuadrado de {}",
        toolbox_2.rect.size(),
        10.0 * u
    );
    assert!(
        near_u(center.x, x_marks(b), u) && near_u(center.y, y, u),
        "{location}: la caja está centrada en {center:?}"
    );
    assert!(
        near_u(toolbox_2.stroke_width, u, u),
        "{location}: el borde de la caja mide {} y debía medir {u}",
        toolbox_2.stroke_width
    );
    assert_eq!(toolbox_2.fill, FIELD_FILL, "{location}: relleno de la caja");
    let marks = marks_square(&esc.m, u, center.y);
    assert_eq!(
        marks.len(),
        usize::from(marked),
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
    assert!(
        esc.m
            .disks
            .iter()
            .all(|d| (d.center.y - center.y).abs() > 3.0 * u),
        "{location}: hay un círculo en la fila de la casilla: {:?}",
        esc.m.disks
    );
}

/// Todo lo pintado como marca (aro, punto, borde de la casilla y su marca) frente al relleno del
/// campo y al del diálogo: al menos 3:1, con los colores que de verdad se pintan.
fn asserts_contrast(esc: &Scene, u: f32, location: &str) {
    let mut colors_2: Vec<(&str, Color32)> = Vec::new();
    colors_2.extend(rings(&esc.m).iter().map(|d| ("aro", d.stroke_color)));
    colors_2.extend(points(&esc.m).iter().map(|d| ("punto", d.fill)));
    colors_2.extend(
        boxes_2(&esc.m, u)
            .iter()
            .map(|c| ("borde de casilla", c.stroke_color)),
    );
    let y = boxes_2(&esc.m, u).first().map(|c| c.rect.center().y);
    if let Some(y) = y {
        colors_2.extend(
            marks_square(&esc.m, u, y)
                .iter()
                .map(|c| ("marca de casilla", c.fill)),
        );
    }
    assert!(
        colors_2.iter().any(|(n, _)| *n == "aro")
            && colors_2.iter().any(|(n, _)| *n == "borde de casilla"),
        "{location}: no se pinta ningún aro o ningún borde de casilla con TOGGLE_EDGE: {colors_2:?}"
    );
    for (name_2, color) in colors_2 {
        for background_2 in [FIELD_FILL, DIALOG_FILL] {
            let ratio = contrast_ratio(color, background_2);
            assert!(
                ratio >= TEST_MIN_MARK,
                "{location}: {name_2} {color:?} sobre {background_2:?} da {ratio:.2} (< {TEST_MIN_MARK})"
            );
        }
    }
}

/// Medidas del anillo de foco de la fila `label` con el foco en `goal`, relativas al rótulo
/// (`x` desde `PAD_U`, `y` desde el rótulo, alto, ancho sin las letras y trazo, en u), tras comprobar que es
/// el anillo de siempre y que contrasta.
fn asserts_ring(b: &mut Bench, goal: FocusTarget, label: &str, location: &str) -> [f32; 5] {
    b.focus_until(goal);
    let esc = b.scene();
    let u = b.u();
    let there_is = rings_2(&esc.p, u);
    assert_eq!(
        there_is.len(),
        1,
        "{location}: el foco en {goal:?} pinta {} anillos ACCENT de 2 u",
        there_is.len()
    );
    let ring_2 = there_is[0];
    let label_2 = text_of(&esc.p, label);
    let letters = label.chars().count() as f32;
    let x0 = in_points(
        b.ppp,
        b.rects().dialog.x + px_ui::dialog_layout::PAD_U * b.n(),
        0,
    )
    .x;
    assert!(
        near_u(ring_2.rect.min.x, x0, u)
            && near_u(ring_2.rect.min.y, label_2.pos.y - u, u)
            && near_u(ring_2.rect.height(), 16.0 * u, u)
            && near_u(ring_2.rect.width(), (18.0 + 7.0 * letters) * u, u),
        "{location}: el anillo de {goal:?} es {:?} y debía rodear la fila entera de {label:?}",
        ring_2.rect
    );
    assert_eq!(ring_2.color, ACCENT, "{location}");
    for background_2 in [FIELD_FILL, DIALOG_FILL] {
        assert!(
            contrast_ratio(ring_2.color, background_2) >= TEST_MIN_MARK,
            "{location}: anillo sobre {background_2:?}"
        );
    }
    // El marcador de la fila queda dentro del anillo.
    let marker = Rect::from_center_size(
        pos2(x_marks(b), label_2.pos.y + 7.0 * u),
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
        ring_2.width_2 / u,
    ]
}

fn with_option_and_box(ppp: f32, window: Window, chosen_2: usize, marked: bool) -> Bench {
    let mut bench = Bench::ready(ppp, window, open(form()));
    bench.event(TestEvent::Choice {
        field: "modo",
        index: chosen_2,
    });
    bench.event(TestEvent::Check {
        field: "opaco",
        on: marked,
    });
    bench.rest_2_2();
    bench
}

const CHOSEN: [usize; 3] = [0, 1, 2];

// --- Fila 9 -----------------------------------------------------------------------------------

#[test]
fn row_9_the_unchosen_option_is_a_ring_of_10_u_without_a_dot_and_with_the_fill_of_the_field() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Large, open(form()));
        let esc = bench.scene();
        let u = bench.u();
        // Por defecto es la primera: «Dos» y «Tres» son solo un aro.
        assert_eq!(bench.dialog.chosen("modo"), Some(0));
        let rings = rings(&esc.m);
        assert_eq!(rings.len(), 3, "ppp {ppp}: tres aros; {:?}", esc.m.disks);
        for ring in &rings[1..] {
            assert!(
                points(&esc.m)
                    .iter()
                    .all(|p| !same_center(p.center, ring.center, u)),
                "ppp {ppp}: una opción no elegida lleva punto"
            );
        }
        asserts_options(&bench, &esc, &format!("ppp {ppp}"));
    }
}

#[test]
fn row_9_the_chosen_option_carries_its_ring_and_exactly_one_accent_dot_of_6_u() {
    for ppp in PPPS {
        for chosen_2 in CHOSEN {
            let mut bench = with_option_and_box(ppp, Window::Large, chosen_2, false);
            let esc = bench.scene();
            let location = format!("ppp {ppp} elegida {chosen_2}");
            asserts_options(&bench, &esc, &location);
            let u = bench.u();
            let point = points(&esc.m);
            assert_eq!(point.len(), 1, "{location}");
            assert!(
                same_center(point[0].center, rings(&esc.m)[chosen_2].center, u),
                "{location}: el punto está sobre el aro de la opción elegida"
            );
            assert_eq!(point[0].fill, ACCENT, "{location}");
        }
    }
}

#[test]
fn row_9_the_dot_follows_the_chosen_option_with_events_one_after_another() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Large, open(form()));
        for chosen_2 in [2usize, 0, 1, 1, 2, 0] {
            bench.event(TestEvent::Choice {
                field: "modo",
                index: chosen_2,
            });
            let esc = bench.scene();
            asserts_options(&bench, &esc, &format!("ppp {ppp} → {chosen_2}"));
        }
    }
}

#[test]
fn row_9_the_box_is_still_square_of_10_u_with_toggle_edge_border_and_a_square_mark_of_6_u() {
    for ppp in PPPS {
        for marked in [false, true, false] {
            let mut bench = with_option_and_box(ppp, Window::Large, 0, marked);
            let esc = bench.scene();
            asserts_box(&bench, &esc, &format!("ppp {ppp} marcada {marked}"));
        }
    }
}

#[test]
fn row_9_option_and_box_are_distinguished_by_shape_not_only_by_color() {
    for ppp in PPPS {
        for (chosen_2, marked) in [(0, false), (1, true), (2, true), (0, true)] {
            let mut bench = with_option_and_box(ppp, Window::Large, chosen_2, marked);
            let esc = bench.scene();
            let u = bench.u();
            let location = format!("ppp {ppp} elegida {chosen_2} marcada {marked}");
            // Tres opciones = tres círculos con aro y ninguna caja en sus filas; una casilla = una
            // caja con aro de rectángulo y ningún círculo en su fila.
            assert_eq!(
                rings(&esc.m).len(),
                3,
                "{location}: las opciones son círculos"
            );
            assert_eq!(
                boxes_2(&esc.m, u).len(),
                1,
                "{location}: la casilla es un rectángulo"
            );
            let toolbox_2 = boxes_2(&esc.m, u)[0];
            for ring in rings(&esc.m) {
                assert!(
                    (ring.center.y - toolbox_2.rect.center().y).abs() > 8.0 * u,
                    "{location}: el aro y la caja comparten fila"
                );
                // El aro no es un cuadrado: su diámetro exterior es el lado de la caja, pero su
                // marcador es un círculo (no hay ningún rectángulo con su centro y su tamaño).
                assert!(
                    boxes_2(&esc.m, u).iter().all(|c| !same_center(
                        c.rect.center(),
                        ring.center,
                        u
                    )),
                    "{location}: la opción lleva además un rectángulo de borde"
                );
            }
            // Y no hay opciones ni casilla «a medias»: sin ningún otro círculo con trazo.
            let with_stroke: Vec<_> = esc
                .m
                .disks
                .iter()
                .filter(|d| d.stroke_width > 0.0)
                .collect();
            assert_eq!(with_stroke.len(), 3, "{location}: {with_stroke:?}");
        }
    }
}

// --- Fila 10 ----------------------------------------------------------------------------------

#[test]
fn row_10_the_ring_the_dot_the_border_and_the_mark_exceed_3_to_1_over_the_field_and_over_the_dialog()
 {
    for ppp in PPPS {
        for (chosen_2, marked) in [(0, false), (1, true), (2, true)] {
            let mut bench = with_option_and_box(ppp, Window::Large, chosen_2, marked);
            let esc = bench.scene();
            asserts_contrast(
                &esc,
                bench.u(),
                &format!("ppp {ppp} elegida {chosen_2} marcada {marked}"),
            );
        }
    }
}

#[test]
fn row_10_the_focus_ring_of_an_option_and_of_a_box_exceeds_3_to_1_over_field_and_dialog() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Large, open(form()));
        // `asserts_ring` comprueba el color y el contraste en cada parada.
        asserts_ring(
            &mut bench,
            FocusTarget::Option("modo", 1),
            "Dos",
            &format!("ppp {ppp}"),
        );
        asserts_ring(
            &mut bench,
            FocusTarget::Field("opaco"),
            "Opaco",
            &format!("ppp {ppp}"),
        );
    }
}

#[test]
fn row_10_toggle_edge_is_text_dim_and_is_in_contrast_pairs_with_minimum_3_and_they_are_11_pairs() {
    assert_eq!(TOGGLE_EDGE, px_ui::theme::TEXT_DIM);
    assert_eq!(CONTRAST_PAIRS.len(), 11, "los 10 de antes y el del aro");
    let pairs: Vec<_> = CONTRAST_PAIRS
        .iter()
        .filter(|&&(_, f, b, _)| f == TOGGLE_EDGE && b == FIELD_FILL)
        .collect();
    // `TEXT_DIM` sobre `FIELD_FILL` no estaba (solo sobre el diálogo y con 4,5): el par es nuevo.
    assert_eq!(
        pairs.len(),
        1,
        "un único par TOGGLE_EDGE sobre FIELD_FILL: {pairs:?}"
    );
    let &(name_2, _, _, minimum_2) = pairs[0];
    assert!(
        minimum_2 >= TEST_MIN_MARK,
        "{name_2}: la tabla pide {minimum_2}"
    );
    assert!(
        contrast_ratio(TOGGLE_EDGE, FIELD_FILL) >= minimum_2,
        "{name_2}: {} < {minimum_2}",
        contrast_ratio(TOGGLE_EDGE, FIELD_FILL)
    );
    assert!(contrast_ratio(TOGGLE_EDGE, DIALOG_FILL) >= TEST_MIN_MARK);
    // Los diez de antes siguen: cada par de la tabla cumple su mínimo y el anillo y el texto están.
    for &(name_2, f, b, minimum_2) in &CONTRAST_PAIRS {
        assert!(contrast_ratio(f, b) >= minimum_2, "{name_2}");
    }
    for expected_2 in [
        (ACCENT, FIELD_FILL, TEST_MIN_MARK),
        (ACCENT, DIALOG_FILL, TEST_MIN_MARK),
        (px_ui::theme::TEXT, FIELD_FILL, TEST_MIN_TEXT),
        (px_ui::theme::TEXT, DIALOG_FILL, TEST_MIN_TEXT),
    ] {
        assert!(
            CONTRAST_PAIRS
                .iter()
                .any(|&(_, f, b, m)| (f, b) == (expected_2.0, expected_2.1) && m >= expected_2.2),
            "falta el par {expected_2:?}"
        );
    }
}

#[test]
fn row_10_the_painted_border_of_options_and_box_is_in_the_pairs_table_with_its_background() {
    for ppp in PPPS {
        let mut bench = with_option_and_box(ppp, Window::Large, 1, true);
        let esc = bench.scene();
        let u = bench.u();
        let mut painted_2: Vec<(Color32, Color32)> = Vec::new();
        for ring in rings(&esc.m) {
            painted_2.push((ring.stroke_color, fill_of_ring(&esc.m, &ring)));
        }
        for toolbox_2 in boxes_2(&esc.m, u) {
            painted_2.push((toolbox_2.stroke_color, toolbox_2.fill));
        }
        assert_eq!(painted_2.len(), 4, "ppp {ppp}: tres aros y una caja");
        for (foreground, background_2) in painted_2 {
            let in_table = CONTRAST_PAIRS
                .iter()
                .find(|&&(_, f, b, _)| f == foreground && b == background_2);
            let Some(&(name_2, _, _, minimum_2)) = in_table else {
                panic!(
                    "ppp {ppp}: {foreground:?} sobre {background_2:?} no está en CONTRAST_PAIRS"
                );
            };
            assert!(
                minimum_2 >= TEST_MIN_MARK,
                "{name_2}: la tabla pide {minimum_2}"
            );
            assert!(
                contrast_ratio(foreground, background_2) >= minimum_2,
                "{name_2}"
            );
        }
    }
}

#[test]
fn row_10_the_bench_of_the_9_stays_without_failures_with_circles_and_measures_the_dot_and_the_mark()
{
    for ppp in PPPS {
        for window in WINDOWS {
            for (chosen_2, marked) in [(0, false), (1, true), (2, true)] {
                let mut bench = with_option_and_box(ppp, window, chosen_2, marked);
                let p = bench.rest_2_2();
                let inf = report_of(&p);
                let location = format!("ppp {ppp} {window:?} elegida {chosen_2} marcada {marked}");
                assert!(inf.failures.is_empty(), "{location}: {:#?}", inf.failures);
                assert!(inf.checked_2 >= 8, "{location}: {}", inf.checked_2);
                // El punto (círculo) y la marca (cuadrado) entran como «marca» sobre el campo.
                assert!(
                    inf.pairs.iter().any(|(d, f, b, m)| d == "marca"
                        && *f == ACCENT
                        && *b == FIELD_FILL
                        && *m == TEST_MIN_MARK),
                    "{location}: no se comprueba ninguna marca sobre FIELD_FILL"
                );
            }
        }
    }
}

// --- Fila 11 ----------------------------------------------------------------------------------

#[test]
fn row_11_the_focus_ring_is_the_same_rectangle_of_2_u_accent_on_option_and_box() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Large, open(form()));
        let location = format!("ppp {ppp}");
        let targets = [
            (FocusTarget::Option("modo", 0), "Uno"),
            (FocusTarget::Option("modo", 1), "Dos"),
            (FocusTarget::Option("modo", 2), "Tres"),
            (FocusTarget::Field("opaco"), "Opaco"),
        ];
        let measures: Vec<[f32; 5]> = targets
            .iter()
            .map(|&(goal, label)| asserts_ring(&mut bench, goal, label, &location))
            .collect();
        for m in &measures {
            // x = PAD_U (0 desde PAD_U), y = rótulo − 1 u, alto 16 u, ancho 18 u + 7 u por letra y
            // trazo de 2 u.
            assert!(
                m[0].abs() <= 0.05
                    && (m[1] + 1.0).abs() <= 0.05
                    && (m[2] - 16.0).abs() <= 0.05
                    && (m[3] - 18.0).abs() <= 0.05
                    && (m[4] - 2.0).abs() <= 0.05,
                "{location}: {m:?}"
            );
        }
    }
}

#[test]
fn row_11_click_on_the_marker_chooses_the_option_and_toggles_the_box_as_today() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Large, open(form()));
        for k in [2usize, 1, 0, 2] {
            let esc = bench.scene();
            let center = rings(&esc.m).get(k).expect("hay tres aros").center;
            bench.click(center);
            assert_eq!(
                bench.dialog.chosen("modo"),
                Some(k),
                "ppp {ppp}: clic en el aro {k}"
            );
            let esc = bench.scene();
            asserts_options(&bench, &esc, &format!("ppp {ppp} tras clic en {k}"));
        }
        for expected_2 in [true, false, true] {
            let esc = bench.scene();
            let u = bench.u();
            let center = boxes_2(&esc.m, u)
                .first()
                .expect("hay una caja")
                .rect
                .center();
            bench.click(center);
            assert_eq!(bench.dialog.checked("opaco"), Some(expected_2), "ppp {ppp}");
            let esc = bench.scene();
            asserts_box(
                &bench,
                &esc,
                &format!("ppp {ppp} tras clic, marcada {expected_2}"),
            );
        }
        // Un clic en el rótulo hace lo mismo que en el marcador.
        let p = bench.rest_2_2();
        bench.click(center_of(&p, "Dos"));
        assert_eq!(bench.dialog.chosen("modo"), Some(1), "ppp {ppp}");
        assert!(
            bench.reactions.is_empty(),
            "ppp {ppp}: {:?}",
            bench.reactions
        );
    }
}

#[test]
fn row_11_tab_and_space_walk_and_choose_as_today_with_the_new_marker() {
    for ppp in PPPS {
        let mut bench = Bench::ready(ppp, Window::Large, open(form()));
        // El orden de foco: opción 0, 1, 2 y la casilla (el hexadecimal está oculto).
        bench.focus_until(FocusTarget::Option("modo", 0));
        for next in [
            FocusTarget::Option("modo", 1),
            FocusTarget::Option("modo", 2),
            FocusTarget::Field("opaco"),
        ] {
            bench.tab(false);
            assert_eq!(bench.focus, Some(next), "ppp {ppp}");
        }
        bench.tab(true);
        assert_eq!(
            bench.focus,
            Some(FocusTarget::Option("modo", 2)),
            "ppp {ppp}"
        );

        for k in [1usize, 2, 0] {
            bench.focus_until(FocusTarget::Option("modo", k));
            bench.presses_2(Key::Space, Modifiers::NONE);
            assert_eq!(
                bench.dialog.chosen("modo"),
                Some(k),
                "ppp {ppp}: Espacio en la opción {k}"
            );
            let esc = bench.scene();
            asserts_options(&bench, &esc, &format!("ppp {ppp} tras Espacio en {k}"));
        }
        for expected_2 in [true, false] {
            bench.focus_until(FocusTarget::Field("opaco"));
            bench.presses_2(Key::Space, Modifiers::NONE);
            assert_eq!(bench.dialog.checked("opaco"), Some(expected_2), "ppp {ppp}");
            let esc = bench.scene();
            asserts_box(
                &bench,
                &esc,
                &format!("ppp {ppp} tras Espacio, marcada {expected_2}"),
            );
        }
        assert!(
            bench.reactions.is_empty(),
            "ppp {ppp}: {:?}",
            bench.reactions
        );
    }
}

// --- Propiedad 4 ------------------------------------------------------------------------------

#[test]
fn property_4_one_marker_per_state_exhaustive_in_the_four_states_and_the_five_scales() {
    // Opción elegida o no × casilla marcada o no: `chosen_2` recorre las tres opciones, así que cada
    // opción se ve elegida y no elegida, con la casilla en cada estado (30 casos por ventana).
    for ppp in PPPS {
        for chosen_2 in CHOSEN {
            for marked in [false, true] {
                let mut bench = with_option_and_box(ppp, Window::Large, chosen_2, marked);
                let location = format!("ppp {ppp} elegida {chosen_2} marcada {marked}");
                let esc = bench.scene();
                // Cada opción: un aro y, como mucho, un punto (uno en total); la casilla: un cuadrado.
                asserts_options(&bench, &esc, &location);
                asserts_box(&bench, &esc, &location);
                assert!(points(&esc.m).len() <= 1, "{location}");
                // Contraste de lo pintado.
                asserts_contrast(&esc, bench.u(), &location);
                // El anillo de foco, el mismo en las tres opciones y en la casilla.
                let measures: Vec<[f32; 5]> = [
                    (FocusTarget::Option("modo", 0), "Uno"),
                    (FocusTarget::Option("modo", 1), "Dos"),
                    (FocusTarget::Option("modo", 2), "Tres"),
                    (FocusTarget::Field("opaco"), "Opaco"),
                ]
                .iter()
                .map(|&(goal, label)| asserts_ring(&mut bench, goal, label, &location))
                .collect();
                for m in &measures[1..] {
                    for (a, b) in m.iter().zip(&measures[0]) {
                        assert!((a - b).abs() <= 0.05, "{location}: {measures:?}");
                    }
                }
                // Enfocar no cambia el marcador: mismo aro, mismo punto, misma caja.
                let after_2 = bench.scene();
                asserts_options(&bench, &after_2, &location);
                asserts_box(&bench, &after_2, &location);
            }
        }
    }
}
