//! Tests de `convert_mode` con el diálogo real (specs/hito-2-convertir-modo.md, unidad 20): la fila 11,
//! el banco de egui del formulario de `convert_form`. En ROJO hasta la implementación.
//!
//! El banco reproduce el orden de fotograma de `App::ui` (como `new_image/dialog_tests.rs` y
//! `px-ui/tests/dialog_open_focus.rs`): primero el menú (`show_menu`), luego las acciones del
//! fotograma (`ConvertMode` abre el `Modal` con `open_convert_mode`, los `Locks` del fotograma y un
//! `ConvertSync` nuevo) y, el último, `show_dialog` seguido de `sync_convert` (con el analizador real
//! y un contador de llamadas). Sin ventana ni capturas: `Context::run_ui` con `RawInput` sintético.
//! El diálogo se abre con un clic real en `Imagen → Convertir modo…`.
//!
//! - Foco inicial en la primera opción de `destino` desde el fotograma siguiente al del clic (con el
//!   clic del menú).
//! - El informe cambia al cambiar de destino y **solo** entonces se vuelve a calcular.
//! - Con un destino que necesita permiso, `Aceptar` se pinta atenuado y el motivo del pie es visible
//!   (`error: falta permitir perder el color`); marcar la casilla lo activa.
//! - El grupo del alfa y el hexadecimal solo se ven con destino Gris (y el hexadecimal solo con su
//!   opción); los errores del hexadecimal (`#12`, `zz`, `#ff000080`).
//! - `Aceptar` con y sin motivo, `Intro`, `Escape`, `Cancelar` y `Tab`.
//! - A la ventana mínima el cuerpo se desplaza solo con destino Gris y solo si no cabe, con título,
//!   pie y botones fijos; a 960 × 720 no.
//! - Contraste ≥ 4,5 en todo texto pintado y sin glifo ausente (`MissingGlyphPolicy::Panic`).
//!
//! **Decisiones y suposiciones:**
//! - Las cinco escalas (`PPPS`) y las dos ventanas (468n × 333n px y 960 × 720 pt) se recorren en las
//!   pruebas **salvo** las que necesitan ver todo el formulario a la vez (qué grupo se pinta, el
//!   grupo del alfa y el hexadecimal), que usan 960 × 720 porque a la mínima el cuerpo se desplaza.
//! - Rótulos de las opciones de `destino`: `Indexado`, `RGBA`, `Gris con alfa`, `Gris` (suposición: los
//!   de la 19; la spec solo da el orden). El resto de rótulos son los de D2.
//! - Se identifica cada pieza por su texto pintado visible. Los literales que se buscan caben en una
//!   fila de 42 caracteres (la zona del informe envuelve a esa anchura).
//! - Se escribe con `Ctrl+A`, doce `Retroceso` y `Event::Text`, con el foco que tenga el campo. Del
//!   hexadecimal no se comprueba el informe con el texto pendiente (la spec no dice si el fondo se
//!   lee del texto pendiente o del confirmado), solo el estado del formulario y `Aceptar`.
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

const MODOS: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];

/// La ventana mínima real de la escala (`468n × 333n` px) o 960 × 720 pt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ventana {
    Minima,
    Grande,
}

const VENTANAS: [Ventana; 2] = [Ventana::Minima, Ventana::Grande];

fn n_de(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn pantalla(ppp: f32, ventana: Ventana) -> Vec2 {
    match ventana {
        Ventana::Minima => {
            let n = n_de(ppp) as f32;
            vec2(468.0 * n / ppp, 333.0 * n / ppp)
        }
        Ventana::Grande => vec2(960.0, 720.0),
    }
}

/// El tamaño de la ventana en píxeles físicos.
fn pixeles(ppp: f32, ventana: Ventana) -> (i64, i64) {
    match ventana {
        Ventana::Minima => (468 * n_de(ppp), 333 * n_de(ppp)),
        Ventana::Grande => (
            (960.0 * f64::from(ppp)).round() as i64,
            (720.0 * f64::from(ppp)).round() as i64,
        ),
    }
}

/// Los destinos de `from` en el orden de D2.
fn destinos(from: Mode) -> [Mode; 3] {
    let v: Vec<Mode> = MODOS.into_iter().filter(|m| *m != from).collect();
    [v[0], v[1], v[2]]
}

fn nombre_de(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "Indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "Gris con alfa",
        Mode::Gray => "Gris",
    }
}

fn en_el_juego(texto: &str) -> bool {
    texto
        .chars()
        .all(|c| px_ui::text::INTERFACE_CHARSET.contains(c))
}

// --- Imágenes del banco (con las pérdidas que dice cada comentario) ---

fn rgba_de(pixels: &[Rgba]) -> Image {
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
fn imagen_de_banco(mode: Mode) -> Image {
    match mode {
        Mode::Rgba => rgba_de(&[
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
fn rgba_de_257_colores() -> Image {
    let mut colores: Vec<Rgba> = (0..256u32)
        .map(|i| Rgba::opaque((i % 256) as u8, (i / 256) as u8, 0))
        .collect();
    colores.push(Rgba::TRANSPARENT);
    rgba_de(&colores)
}

/// El color activo por defecto del banco: opaco.
const ACTIVO: Rgba = Rgba::opaque(0x12, 0x34, 0x56);

// --- Eventos sintéticos ---

fn mover(pos: Pos2) -> InputEvent {
    InputEvent::PointerMoved(pos)
}

fn boton(pos: Pos2, pressed: bool) -> InputEvent {
    InputEvent::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn tecla(key: Key, modifiers: Modifiers) -> InputEvent {
    InputEvent::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn tecla_suelta(key: Key) -> InputEvent {
    InputEvent::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn rueda_linea(arriba: bool) -> InputEvent {
    InputEvent::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if arriba { 1.0 } else { -1.0 }),
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

fn contexto(ppp: f32) -> Context {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
    install_fonts(&ctx);
    for _ in 0..2 {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..RawInput::default()
        };
        let salida = ctx.run_ui(entrada, |ui| apply_scale(ui.ctx()));
        salida.drop_without_applying_deltas();
    }
    ctx
}

fn ejecuta(
    ctx: &Context,
    pantalla: Vec2,
    events: Vec<InputEvent>,
    dibuja: impl FnMut(&mut eframe::egui::Ui),
) -> Vec<ClippedShape> {
    let entrada = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, pantalla)),
        events,
        ..RawInput::default()
    };
    let mut salida = ctx.run_ui(entrada, dibuja);
    salida.textures_delta.clear();
    std::mem::take(&mut salida.shapes)
}

// --- Lo pintado, en orden de pintado ---

#[derive(Clone, Debug)]
struct Texto {
    texto: String,
    pos: Pos2,
    tamano: Vec2,
    clip: Rect,
    colores: Vec<Color32>,
    opacidad: f32,
}

impl Texto {
    fn rect(&self) -> Rect {
        Rect::from_min_size(self.pos, self.tamano)
    }

    /// Algo del texto cae dentro de su recorte.
    fn visible(&self) -> bool {
        self.rect().intersect(self.clip).is_positive()
    }

    fn deshabilitado(&self) -> bool {
        self.opacidad < 1.0
            || self
                .colores
                .iter()
                .any(|c| *c == TEXT_DISABLED || c.a() < 255)
    }
}

#[derive(Clone, Copy, Debug)]
struct Relleno {
    rect: Rect,
    fill: Color32,
    clip: Rect,
}

#[derive(Clone, Debug)]
enum Forma {
    Texto(Texto),
    Relleno(Relleno),
}

#[derive(Clone, Debug, Default)]
struct Pintado {
    formas: Vec<Forma>,
}

impl Pintado {
    fn visibles(&self) -> impl Iterator<Item = &Texto> + '_ {
        self.formas.iter().filter_map(|f| match f {
            Forma::Texto(t) if t.visible() => Some(t),
            _ => None,
        })
    }

    /// Un texto visible que, recortado, es exactamente `buscado`.
    fn exacto(&self, buscado: &str) -> Option<&Texto> {
        self.visibles().find(|t| t.texto.trim() == buscado)
    }

    /// Algún texto visible contiene `buscado`.
    fn contiene(&self, buscado: &str) -> bool {
        self.visibles().any(|t| t.texto.contains(buscado))
    }

    fn lista(&self) -> Vec<&str> {
        self.visibles().map(|t| t.texto.as_str()).collect()
    }
}

fn recoge(shape: &Shape, clip: Rect, fuera: &mut Vec<Forma>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colores = job
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
            fuera.push(Forma::Texto(Texto {
                texto: t.galley.text().to_owned(),
                pos: t.pos,
                tamano: t.galley.size(),
                clip,
                colores,
                opacidad: t.opacity_factor,
            }));
        }
        Shape::Rect(r) => {
            if r.fill != Color32::TRANSPARENT {
                fuera.push(Forma::Relleno(Relleno {
                    rect: r.rect,
                    fill: r.fill,
                    clip,
                }));
            }
        }
        Shape::Vec(formas) => {
            for forma in formas {
                recoge(forma, clip, fuera);
            }
        }
        _ => {}
    }
}

fn pintado(formas: &[ClippedShape]) -> Pintado {
    let mut fuera = Vec::new();
    for clipped in formas {
        recoge(&clipped.shape, clipped.clip_rect, &mut fuera);
    }
    Pintado { formas: fuera }
}

// --- El banco: el orden de fotograma de `App` ---

/// El analizador real, con un contador de llamadas.
#[derive(Default)]
struct Cuenta {
    llamadas: usize,
}

impl Analyzer for Cuenta {
    fn analyze(&mut self, image: &Image, target: Mode) -> Analysis {
        self.llamadas += 1;
        analyze(image, target)
    }
}

struct Banco {
    ctx: Context,
    ppp: f32,
    ventana: Ventana,
    from: Mode,
    image: Image,
    activo: Option<Rgba>,
    modal: Modal,
    focus: Option<FocusTarget>,
    convert: Option<ConvertSync>,
    cuenta: Cuenta,
    /// Los `Values` de cada `Accepted` desde el principio.
    aceptados: Vec<Values>,
    /// Los `Cancelled` desde el principio.
    cancelados: usize,
    /// Qué `Id` tenía el foco al final de cada fotograma, y si el diálogo estaba abierto.
    traza: Vec<(bool, Option<Id>)>,
    /// El fotograma (índice en `traza`) en que `show_dialog` pintó el diálogo por primera vez.
    abierto_en: Option<usize>,
    ultimo: Pintado,
}

impl Banco {
    fn nuevo(ppp: f32, ventana: Ventana, from: Mode, image: Image, activo: Option<Rgba>) -> Banco {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(eframe::egui::Theme::Dark, visuals(ppp));
        Banco {
            ctx,
            ppp,
            ventana,
            from,
            image,
            activo,
            modal: Modal::Closed,
            focus: None,
            convert: None,
            cuenta: Cuenta::default(),
            aceptados: Vec::new(),
            cancelados: 0,
            traza: Vec::new(),
            abierto_en: None,
            ultimo: Pintado::default(),
        }
    }

    /// Con el diálogo abierto por un clic en `Imagen → Convertir modo…` y unos fotogramas de reposo.
    fn abierto(ppp: f32, ventana: Ventana, from: Mode) -> Banco {
        Banco::abierto_con(ppp, ventana, from, imagen_de_banco(from), Some(ACTIVO))
    }

    fn abierto_con(
        ppp: f32,
        ventana: Ventana,
        from: Mode,
        image: Image,
        activo: Option<Rgba>,
    ) -> Banco {
        let mut b = Banco::nuevo(ppp, ventana, from, image, activo);
        b.abre_con_el_raton();
        assert!(
            b.modal.is_open(),
            "ppp {ppp} {ventana:?} {from:?}: no se abrió"
        );
        b
    }

    fn dialogo(&self) -> &Dialog {
        match &self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el modal está cerrado"),
        }
    }

    fn dialogo_mut(&mut self) -> &mut Dialog {
        match &mut self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el modal está cerrado"),
        }
    }

    fn fotograma(&mut self, events: Vec<InputEvent>) {
        let size = pantalla(self.ppp, self.ventana);
        let mut reaccion = Reaction::None;
        let mut pinto_el_dialogo = false;
        let formas = {
            let Banco {
                ctx,
                modal,
                focus,
                convert,
                cuenta,
                image,
                from,
                activo,
                ..
            } = &mut *self;
            ejecuta(ctx, size, events, |ui| {
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
                        *convert = Some(ConvertSync::new(*from, 0, *activo));
                    }
                }
                // El diálogo, el último, y `sync_convert` tras él.
                if let Modal::Open(dialog) = modal {
                    pinto_el_dialogo = true;
                    reaccion = show_dialog(&c, dialog, focus);
                    if let Some(sync) = convert.as_mut() {
                        let historial = History {
                            limit: 64 << 20,
                            used: 0,
                        };
                        sync_convert(dialog, sync, image, 0, historial, cuenta);
                    }
                }
            })
        };
        match reaccion {
            Reaction::Accepted(values) => {
                self.aceptados.push(values);
                self.modal.abandon();
                self.convert = None;
                self.focus = None;
            }
            Reaction::Cancelled => {
                self.cancelados += 1;
                self.modal.abandon();
                self.convert = None;
                self.focus = None;
            }
            Reaction::Rejected(_) | Reaction::None => {}
        }
        if pinto_el_dialogo && self.abierto_en.is_none() {
            self.abierto_en = Some(self.traza.len());
        }
        self.traza
            .push((self.modal.is_open(), self.ctx.memory(|m| m.focused())));
        self.ultimo = pintado(&formas);
    }

    fn reposo(&mut self) {
        for _ in 0..3 {
            self.fotograma(vec![]);
        }
    }

    /// Mueve, pulsa y suelta en `pos`: tres fotogramas, como los entrega winit.
    fn clic(&mut self, pos: Pos2) {
        self.fotograma(vec![mover(pos)]);
        self.fotograma(vec![boton(pos, true)]);
        self.fotograma(vec![boton(pos, false)]);
    }

    /// Pulsa y suelta una tecla (sin el evento de soltar, egui tomaría una segunda pulsación de la
    /// misma tecla por una repetición).
    fn pulsa(&mut self, key: Key) {
        self.fotograma(vec![tecla(key, Modifiers::NONE)]);
        self.fotograma(vec![tecla_suelta(key)]);
        self.reposo();
    }

    /// El centro del texto visible `buscado`: el exacto y, si no hay, el primero que lo contiene.
    fn centro_de(&self, buscado: &str) -> Pos2 {
        self.ultimo
            .exacto(buscado)
            .or_else(|| self.ultimo.visibles().find(|t| t.texto.contains(buscado)))
            .unwrap_or_else(|| panic!("no se pinta {buscado:?}; hay {:?}", self.ultimo.lista()))
            .rect()
            .center()
    }

    /// Clic en el texto visible `buscado` y unos fotogramas de reposo.
    fn clic_en(&mut self, buscado: &str) {
        let centro = self.centro_de(buscado);
        self.clic(centro);
        self.reposo();
    }

    /// `Imagen → Convertir modo…` con el ratón, como lo hace el usuario.
    fn abre_con_el_raton(&mut self) {
        self.reposo();
        self.clic_en("Imagen");
        assert!(
            self.ultimo.exacto("Convertir modo…").is_some(),
            "el menú Imagen muestra la entrada; hay {:?}",
            self.ultimo.lista()
        );
        assert!(
            !self.ultimo.contiene("Ctrl+") && !self.ultimo.contiene("Cmd+"),
            "la entrada no tiene atajo; hay {:?}",
            self.ultimo.lista()
        );
        let entrada = self.centro_de("Convertir modo…");
        self.clic(entrada);
        self.reposo();
    }

    /// Escribe `texto` en el campo con el foco: `Ctrl+A`, borrar y teclear.
    fn escribe(&mut self, texto: &str) {
        let mut borrar = vec![tecla(Key::A, ctrl_a())];
        for _ in 0..12 {
            borrar.push(tecla(Key::Backspace, Modifiers::NONE));
        }
        self.fotograma(borrar);
        self.fotograma(vec![InputEvent::Text(texto.to_owned())]);
        self.reposo();
    }

    /// `Tab` hasta que el foco es `objetivo` (como mucho una vuelta entera).
    fn foco_hasta(&mut self, objetivo: FocusTarget) {
        let vuelta = focus_order(self.dialogo()).len() + 1;
        for _ in 0..vuelta {
            if self.focus == Some(objetivo) {
                return;
            }
            self.pulsa(Key::Tab);
        }
        assert_eq!(self.focus, Some(objetivo), "el Tab no llega a {objetivo:?}");
    }

    fn foco_de_egui(&self) -> Option<Id> {
        self.ctx.memory(|m| m.focused())
    }

    /// El `Id` de egui de un campo (el que se enfoca al darle el foco).
    fn id_de(campo: &'static str) -> Id {
        Id::unique(("pixoko-dialog", campo))
    }

    /// El `Id` de egui de la opción `i` de un grupo.
    fn id_de_opcion(campo: &'static str, i: usize) -> Id {
        Id::unique(("pixoko-dialog-option", campo, i))
    }

    fn layout(&self) -> DialogLayout {
        let d = self.dialogo();
        dialog_layout(
            pixeles(self.ppp, self.ventana),
            n_de(self.ppp) as u32,
            d.content_h_u(),
            d.is_fill(),
            d.spec().cancel.is_some(),
        )
    }

    fn u(&self) -> f32 {
        n_de(self.ppp) as f32 / self.ppp
    }

    fn aceptar_deshabilitado(&self) -> bool {
        self.ultimo
            .exacto("Aceptar")
            .unwrap_or_else(|| panic!("no se pinta Aceptar; hay {:?}", self.ultimo.lista()))
            .deshabilitado()
    }

    /// Elige un destino con un clic real en su opción.
    fn elige_destino(&mut self, mode: Mode) {
        self.clic_en(nombre_de(mode));
    }

    /// Elige una opción de un grupo con eventos directos y un par de fotogramas (sin mover el foco).
    fn elige(&mut self, campo: &'static str, index: usize) {
        assert_eq!(
            self.dialogo_mut().event(Event::Choice {
                field: campo,
                index
            }),
            Reaction::None
        );
        self.reposo();
    }

    fn marca(&mut self, campo: &'static str, on: bool) {
        assert_eq!(
            self.dialogo_mut().event(Event::Check { field: campo, on }),
            Reaction::None
        );
        self.reposo();
    }

    fn describe(&self) -> String {
        format!(
            "ppp {} {:?} {:?}; aceptados {}; cancelados {}; foco {:?}; pintado {:?}",
            self.ppp,
            self.ventana,
            self.from,
            self.aceptados.len(),
            self.cancelados,
            self.focus,
            self.ultimo.lista()
        )
    }
}

const CASILLA: &str = "Permitir perder el color (pasar a gris)";
const ALFA_ROTULO: &str = "Si hay alfa, componer sobre:";
const OPCIONES_DEL_ALFA: [&str; 5] = [
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
fn foco_tras_la_apertura(b: &Banco) -> Vec<(usize, Option<Id>)> {
    let desde = b.abierto_en.map_or(b.traza.len(), |i| i + 1);
    b.traza
        .iter()
        .enumerate()
        .skip(desde)
        .map(|(i, (_, id))| (i, *id))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Fila 11: abrir
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_abierto_con_un_clic_en_el_menu_el_foco_inicial_es_la_primera_opcion_del_destino() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for from in MODOS {
                let b = Banco::abierto(ppp, ventana, from);
                assert_eq!(b.modal.kind(), Some(DialogKind::ConvertMode));
                assert!(
                    b.abierto_en.is_some(),
                    "ppp {ppp} {ventana:?} {from:?}: nunca se pintó"
                );
                let foco = foco_tras_la_apertura(&b);
                assert!(
                    foco.len() >= 3,
                    "ppp {ppp} {ventana:?} {from:?}: faltan fotogramas tras abrir"
                );
                for (fotograma, id) in foco {
                    assert_eq!(
                        id,
                        Some(Banco::id_de_opcion("destino", 0)),
                        "el foco de egui en el fotograma {fotograma} (se abrió en {:?}) no es la \
                         primera opción del destino; {}",
                        b.abierto_en,
                        b.describe()
                    );
                }
                assert_eq!(
                    b.focus,
                    Some(FocusTarget::Option("destino", 0)),
                    "ppp {ppp} {ventana:?} {from:?}"
                );
            }
        }
    }
}

#[test]
fn fila_11_el_dialogo_pinta_titulo_destinos_pie_y_botones_a_las_dos_ventanas() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for from in MODOS {
                let b = Banco::abierto(ppp, ventana, from);
                let mut esperados: Vec<&str> =
                    vec!["Convertir modo", "Aceptar", "Cancelar", "Modo de destino"];
                esperados.extend(destinos(from).into_iter().map(nombre_de));
                for texto in esperados {
                    assert!(
                        b.ultimo.exacto(texto).is_some(),
                        "ppp {ppp} {ventana:?} {from:?}: falta {texto:?}; hay {:?}",
                        b.ultimo.lista()
                    );
                }
                // El modo actual no se ofrece: ni siquiera como opción.
                assert!(
                    b.ultimo.exacto(nombre_de(from)).is_none(),
                    "ppp {ppp} {ventana:?} {from:?}: se ofrece el modo actual"
                );
            }
        }
    }
}

#[test]
fn fila_11_con_el_destino_por_defecto_todos_los_origenes_tienen_aceptar_activo_y_su_informe() {
    // El destino por defecto (el primero de la lista) es exacto desde los cuatro modos de la prueba.
    for ppp in PPPS {
        for (from, esperado) in [
            (Mode::Indexed, "Sin pérdidas: la conversión es exacta."),
            (Mode::Rgba, "Sin pérdidas. Paleta nueva de 4 colores."),
            (Mode::GrayAlpha, "Sin pérdidas. Paleta nueva de 3 colores,"),
            (Mode::Gray, "Sin pérdidas. Paleta nueva de 3 colores."),
        ] {
            let b = Banco::abierto(ppp, Ventana::Grande, from);
            assert!(
                b.ultimo.contiene(esperado),
                "ppp {ppp} {from:?}: falta {esperado:?}; hay {:?}",
                b.ultimo.lista()
            );
            assert!(!b.aceptar_deshabilitado(), "ppp {ppp} {from:?}");
            assert!(
                !b.ultimo.contiene(ERROR_PREFIX),
                "ppp {ppp} {from:?}: no hay motivo; hay {:?}",
                b.ultimo.lista()
            );
            assert_eq!(
                b.cuenta.llamadas, 1,
                "ppp {ppp} {from:?}: una llamada al abrir"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el informe, el destino y el permiso del color
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_el_informe_cambia_al_cambiar_de_destino_y_solo_entonces_recalcula() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        let contexto = format!("ppp {ppp}");
        assert_eq!(b.cuenta.llamadas, 1, "{contexto}: al abrir");
        assert!(
            b.ultimo
                .contiene("Sin pérdidas. Paleta nueva de 4 colores."),
            "{contexto}"
        );

        b.elige_destino(Mode::GrayAlpha);
        assert_eq!(b.cuenta.llamadas, 2, "{contexto}: un destino nuevo");
        assert!(
            b.ultimo.contiene("Pierde el color en 3 píxeles."),
            "{contexto}: hay {:?}",
            b.ultimo.lista()
        );
        assert!(!b.ultimo.contiene("Paleta nueva"), "{contexto}");

        // Reposo, casilla y clics en los demás campos: ningún cálculo más.
        b.reposo();
        b.reposo();
        assert_eq!(b.cuenta.llamadas, 2, "{contexto}: el reposo no calcula");
        b.clic_en(CASILLA);
        b.clic_en(CASILLA);
        assert_eq!(b.cuenta.llamadas, 2, "{contexto}: la casilla no calcula");

        b.elige_destino(Mode::Gray);
        assert_eq!(b.cuenta.llamadas, 3, "{contexto}");
        assert!(
            b.ultimo.contiene("Pierde el color en 3 píxeles."),
            "{contexto}"
        );
        assert!(
            b.ultimo.contiene("Pierde el alfa en 2 píxeles."),
            "{contexto}"
        );

        b.elige_destino(Mode::Indexed);
        assert_eq!(b.cuenta.llamadas, 4, "{contexto}");
        assert!(
            b.ultimo
                .contiene("Sin pérdidas. Paleta nueva de 4 colores."),
            "{contexto}"
        );
    }
}

#[test]
fn fila_11_el_informe_de_cada_origen_y_destino_dice_lo_que_perderia() {
    // (origen, destino, líneas que deben pintarse, líneas que no)
    let casos: [(Mode, Mode, &[&str], &[&str]); 11] = [
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
        for (from, destino, debe, no_debe) in casos {
            let mut b = Banco::abierto(ppp, Ventana::Grande, from);
            if destino != destinos(from)[0] {
                b.elige_destino(destino);
            }
            for linea in debe {
                assert!(
                    b.ultimo.contiene(linea),
                    "ppp {ppp} {from:?} a {destino:?}: falta {linea:?}; hay {:?}",
                    b.ultimo.lista()
                );
            }
            for linea in no_debe {
                assert!(
                    !b.ultimo.contiene(linea),
                    "ppp {ppp} {from:?} a {destino:?}: sobra {linea:?}"
                );
            }
        }
    }
}

#[test]
fn fila_11_con_un_destino_que_necesita_permiso_aceptar_esta_atenuado_con_el_motivo_visible() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for from in [Mode::Rgba, Mode::Indexed] {
                let contexto = format!("ppp {ppp} {ventana:?} {from:?}");
                let mut b = Banco::abierto(ppp, ventana, from);
                assert!(
                    !b.aceptar_deshabilitado(),
                    "{contexto}: a Indexado/RGBA, nada que permitir"
                );

                b.elige_destino(Mode::GrayAlpha);
                assert!(b.aceptar_deshabilitado(), "{contexto}: falta el permiso");
                assert!(
                    b.ultimo.contiene("error: falta permitir perder el color"),
                    "{contexto}: el motivo del pie es visible; hay {:?}",
                    b.ultimo.lista()
                );
                assert_eq!(
                    b.dialogo().block(),
                    Some("falta permitir perder el color"),
                    "{contexto}"
                );

                // Marcar la casilla con un clic real lo activa.
                b.clic_en(CASILLA);
                assert_eq!(
                    b.dialogo().checked("luma_gris_alfa"),
                    Some(true),
                    "{contexto}"
                );
                assert!(
                    !b.aceptar_deshabilitado(),
                    "{contexto}: con la casilla marcada"
                );
                assert!(
                    !b.ultimo.contiene(ERROR_PREFIX),
                    "{contexto}: ya no hay motivo; hay {:?}",
                    b.ultimo.lista()
                );

                // Y desmarcarla lo vuelve a atenuar.
                b.clic_en(CASILLA);
                assert!(b.aceptar_deshabilitado(), "{contexto}");
                assert!(
                    b.ultimo.contiene("error: falta permitir perder el color"),
                    "{contexto}"
                );
            }
        }
    }
}

#[test]
fn fila_11_gris_pide_la_casilla_y_el_fondo_y_cada_opcion_del_alfa_lo_cubre() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        let contexto = |p: &str| format!("ppp {ppp}: {p}");
        b.elige_destino(Mode::Gray);
        assert!(b.aceptar_deshabilitado(), "{}", contexto("sin nada"));
        assert!(
            b.ultimo.contiene("error: falta permitir perder el color"),
            "{}: {:?}",
            contexto("falta la casilla"),
            b.ultimo.lista()
        );

        b.clic_en(CASILLA);
        assert!(
            b.aceptar_deshabilitado(),
            "{}",
            contexto("con la casilla y sin fondo")
        );
        assert!(
            b.ultimo.contiene("error: elige el fondo para el alfa"),
            "{}: {:?}",
            contexto("falta el fondo"),
            b.ultimo.lista()
        );

        for (opcion, fondo) in [
            ("Fondo blanco", "Fondo del alfa: #ffffff"),
            ("Fondo negro", "Fondo del alfa: #000000"),
            ("Fondo: el color activo", "Fondo del alfa: #123456"),
        ] {
            b.clic_en(opcion);
            assert!(!b.aceptar_deshabilitado(), "{}", contexto(opcion));
            assert!(!b.ultimo.contiene(ERROR_PREFIX), "{}", contexto(opcion));
            assert!(
                b.ultimo.contiene(fondo),
                "{}: falta {fondo:?}; hay {:?}",
                contexto(opcion),
                b.ultimo.lista()
            );
        }
        assert_eq!(
            b.cuenta.llamadas,
            2,
            "{}",
            contexto("solo abrir y cambiar de destino")
        );

        // «No perder el alfa» vuelve a pedir el fondo.
        b.clic_en("No perder el alfa");
        assert!(b.aceptar_deshabilitado());
        assert!(b.ultimo.contiene("error: elige el fondo para el alfa"));
    }
}

#[test]
fn fila_11_un_color_activo_con_alfa_o_sin_color_atenua_aceptar_con_su_motivo() {
    for ppp in PPPS {
        for (activo, motivo) in [
            (
                Some(Rgba::new(0xff, 0, 0, 0x80)),
                "error: el color activo #ff000080 tiene alfa",
            ),
            (None, "error: no hay color activo"),
        ] {
            let mut b = Banco::abierto_con(
                ppp,
                Ventana::Grande,
                Mode::Rgba,
                imagen_de_banco(Mode::Rgba),
                activo,
            );
            b.elige_destino(Mode::Gray);
            b.clic_en(CASILLA);
            b.clic_en("Fondo: el color activo");
            assert!(b.aceptar_deshabilitado(), "ppp {ppp} {activo:?}");
            assert!(
                b.ultimo.contiene(motivo),
                "ppp {ppp} {activo:?}: falta {motivo:?}; hay {:?}",
                b.ultimo.lista()
            );
            assert!(
                !b.ultimo.contiene("Fondo del alfa"),
                "ppp {ppp} {activo:?}: no hay fondo válido"
            );
            // Otro fondo lo resuelve.
            b.clic_en("Fondo blanco");
            assert!(!b.aceptar_deshabilitado(), "ppp {ppp} {activo:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el grupo del alfa y el hexadecimal
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_el_grupo_del_alfa_solo_se_ve_con_destino_gris_y_el_hexadecimal_solo_con_su_opcion() {
    for ppp in PPPS {
        for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
            let mut b = Banco::abierto(ppp, Ventana::Grande, from);
            let contexto = |p: &str| format!("ppp {ppp} {from:?}: {p}");
            // Destino por defecto: ni casilla, ni grupo del alfa, ni hexadecimal.
            for texto in [CASILLA, ALFA_ROTULO, "Color"] {
                assert!(
                    b.ultimo.exacto(texto).is_none(),
                    "{}: {texto}",
                    contexto("por defecto")
                );
            }
            assert!(!b.dialogo().is_shown("alfa_gris"));

            // A Gris con alfa (solo desde RGBA e indexado): la casilla y ningún grupo del alfa.
            if from != Mode::GrayAlpha {
                b.elige_destino(Mode::GrayAlpha);
                assert!(
                    b.ultimo.exacto(CASILLA).is_some(),
                    "{}",
                    contexto("Gris con alfa")
                );
                assert!(
                    b.ultimo.exacto(ALFA_ROTULO).is_none(),
                    "{}",
                    contexto("Gris con alfa")
                );
                assert!(!b.dialogo().is_shown("alfa_gris"));
            }

            // A Gris: el grupo del alfa con sus cinco opciones y, desde RGBA e indexado, la casilla.
            b.elige_destino(Mode::Gray);
            assert!(
                b.ultimo.exacto(ALFA_ROTULO).is_some(),
                "{}",
                contexto("Gris")
            );
            for opcion in OPCIONES_DEL_ALFA {
                assert!(
                    b.ultimo.exacto(opcion).is_some(),
                    "{}: falta {opcion:?}; hay {:?}",
                    contexto("Gris"),
                    b.ultimo.lista()
                );
            }
            assert_eq!(
                b.ultimo.exacto(CASILLA).is_some(),
                from != Mode::GrayAlpha,
                "{}",
                contexto("la casilla solo desde RGBA e indexado")
            );
            assert!(
                b.ultimo.exacto("Color").is_none(),
                "{}",
                contexto("sin la opción del hexadecimal")
            );
            assert!(!b.dialogo().is_shown("fondo_hex"));

            // Con la opción del hexadecimal sale «Color» con su valor de nacimiento.
            b.clic_en("Fondo: un color hexadecimal");
            assert!(
                b.dialogo().is_shown("fondo_hex"),
                "{}",
                contexto("hexadecimal")
            );
            assert!(
                b.ultimo.exacto("Color").is_some(),
                "{}",
                contexto("hexadecimal")
            );
            assert!(
                b.ultimo.contiene("#808080"),
                "{}: nace en gris medio; hay {:?}",
                contexto("hexadecimal"),
                b.ultimo.lista()
            );

            // Otra opción lo oculta.
            b.clic_en("Fondo blanco");
            assert!(
                !b.dialogo().is_shown("fondo_hex"),
                "{}",
                contexto("otra opción")
            );
            assert!(
                b.ultimo.exacto("Color").is_none(),
                "{}",
                contexto("otra opción")
            );

            // Volver a otro destino oculta el grupo entero (y con él el hexadecimal).
            b.clic_en("Fondo: un color hexadecimal");
            b.elige_destino(destinos(from)[0]);
            assert!(
                b.ultimo.exacto(ALFA_ROTULO).is_none(),
                "{}",
                contexto("destino 0")
            );
            assert!(
                b.ultimo.exacto("Color").is_none(),
                "{}",
                contexto("destino 0")
            );
            assert!(!b.dialogo().is_shown("fondo_hex"));
        }
    }
}

#[test]
fn fila_11_desde_gris_no_hay_casilla_ni_grupo_ni_hexadecimal_con_ningun_destino() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Gray);
        for destino in destinos(Mode::Gray) {
            b.elige_destino(destino);
            for texto in [CASILLA, ALFA_ROTULO, "Color"] {
                assert!(
                    b.ultimo.exacto(texto).is_none(),
                    "ppp {ppp} a {destino:?}: sobra {texto:?}"
                );
            }
            assert!(
                !b.aceptar_deshabilitado(),
                "ppp {ppp} a {destino:?}: es exacto"
            );
            assert!(!b.ultimo.contiene(ERROR_PREFIX), "ppp {ppp} a {destino:?}");
        }
    }
}

#[test]
fn fila_11_hexadecimal_ilegible_con_alfa_y_valido_dan_cada_uno_su_error_o_nada() {
    for ppp in PPPS {
        for from in [Mode::Rgba, Mode::GrayAlpha] {
            let mut b = Banco::abierto(ppp, Ventana::Grande, from);
            let contexto = |p: &str| format!("ppp {ppp} {from:?}: {p}");
            b.elige_destino(Mode::Gray);
            if from == Mode::Rgba {
                b.clic_en(CASILLA);
            }
            b.clic_en("Fondo: un color hexadecimal");
            b.foco_hasta(FocusTarget::Field("fondo_hex"));

            // `#12` y `zz`: demasiado cortos.
            let corto = hex_notice(&HexError::Length(2));
            for texto in ["#12", "zz"] {
                b.escribe(texto);
                assert!(
                    b.ultimo.contiene(&format!("{ERROR_PREFIX}{corto}")),
                    "{}: falta el error en línea; hay {:?}",
                    contexto(texto),
                    b.ultimo.lista()
                );
                assert!(
                    b.ultimo.contiene(&format!("{ERROR_PREFIX}Color: {corto}")),
                    "{}: falta el motivo del pie; hay {:?}",
                    contexto(texto),
                    b.ultimo.lista()
                );
                assert!(b.aceptar_deshabilitado(), "{}", contexto(texto));
            }

            // `#ff000080`: legible, pero un fondo no lleva alfa.
            b.escribe("#ff000080");
            assert_eq!(
                b.dialogo().reason(),
                Some("Color: el fondo no lleva alfa".to_string()),
                "{}",
                contexto("#ff000080")
            );
            assert!(
                b.ultimo
                    .contiene(&format!("{ERROR_PREFIX}Color: el fondo no lleva alfa")),
                "{}: falta el motivo del pie; hay {:?}",
                contexto("#ff000080"),
                b.ultimo.lista()
            );
            assert!(b.aceptar_deshabilitado(), "{}", contexto("#ff000080"));

            // `#ff0000`: vale.
            b.escribe("#ff0000");
            assert_eq!(b.dialogo().reason(), None, "{}", contexto("#ff0000"));
            assert!(!b.aceptar_deshabilitado(), "{}", contexto("#ff0000"));
            assert!(!b.ultimo.contiene(ERROR_PREFIX), "{}", contexto("#ff0000"));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Aceptar, Intro, Escape y Cancelar
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_aceptar_sin_motivo_convierte_y_atenuado_no_hace_nada() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            // Sin nada que cubrir: Aceptar convierte con el plan por defecto.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            b.clic_en("Aceptar");
            assert_eq!(b.aceptados.len(), 1, "{}", b.describe());
            assert!(!b.modal.is_open());
            assert_eq!(
                plan_from_values(&b.aceptados[0], Mode::Rgba, Some(ACTIVO)),
                Some(plan(Mode::Indexed, false, None)),
                "ppp {ppp} {ventana:?}"
            );

            // Con un permiso que falta: Aceptar atenuado no convierte ni cierra.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            b.elige_destino(Mode::GrayAlpha);
            assert!(b.aceptar_deshabilitado());
            b.clic_en("Aceptar");
            assert!(
                b.aceptados.is_empty(),
                "ppp {ppp} {ventana:?}: {}",
                b.describe()
            );
            assert!(b.modal.is_open());
            b.clic_en(CASILLA);
            b.clic_en("Aceptar");
            assert_eq!(b.aceptados.len(), 1, "{}", b.describe());
            assert_eq!(
                plan_from_values(&b.aceptados[0], Mode::Rgba, Some(ACTIVO)),
                Some(plan(Mode::GrayAlpha, true, None)),
                "ppp {ppp} {ventana:?}"
            );
        }
    }
}

#[test]
fn fila_11_aceptar_con_el_destino_gris_entrega_el_fondo_elegido() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.elige_destino(Mode::Gray);
        b.clic_en(CASILLA);
        b.clic_en("Fondo negro");
        b.clic_en("Aceptar");
        assert_eq!(b.aceptados.len(), 1, "{}", b.describe());
        assert_eq!(
            plan_from_values(&b.aceptados[0], Mode::Rgba, Some(ACTIVO)),
            Some(plan(Mode::Gray, true, Some([0, 0, 0]))),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_11_intro_convierte_solo_con_todo_cubierto() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            // Con todo cubierto desde el principio: convierte.
            for from in MODOS {
                let mut b = Banco::abierto(ppp, ventana, from);
                b.pulsa(Key::Enter);
                assert_eq!(b.aceptados.len(), 1, "{from:?}: {}", b.describe());
                assert!(!b.modal.is_open());
                assert_eq!(
                    plan_from_values(&b.aceptados[0], from, Some(ACTIVO)),
                    Some(plan(destinos(from)[0], false, None)),
                    "ppp {ppp} {ventana:?} {from:?}"
                );
                assert_eq!(b.cancelados, 0);
            }

            // Con un permiso por cubrir: Intro no convierte; con él cubierto, sí.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            // Con un clic real en la opción: el foco queda en ella, como lo deja el usuario.
            b.elige_destino(Mode::GrayAlpha);
            assert!(b.aceptar_deshabilitado());
            b.pulsa(Key::Enter);
            assert!(
                b.aceptados.is_empty(),
                "ppp {ppp} {ventana:?}: {}",
                b.describe()
            );
            assert!(b.modal.is_open());
            b.marca("luma_gris_alfa", true);
            b.pulsa(Key::Enter);
            assert_eq!(
                b.aceptados.len(),
                1,
                "ppp {ppp} {ventana:?}: {}",
                b.describe()
            );
            assert_eq!(
                plan_from_values(&b.aceptados[0], Mode::Rgba, Some(ACTIVO)),
                Some(plan(Mode::GrayAlpha, true, None))
            );
        }
    }
}

#[test]
fn fila_11_con_mas_de_256_colores_aceptar_no_se_activa_con_nada_y_explica_el_cuantizador() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto_con(
                ppp,
                ventana,
                Mode::Rgba,
                rgba_de_257_colores(),
                Some(ACTIVO),
            );
            let contexto = format!("ppp {ppp} {ventana:?}");
            assert!(b.aceptar_deshabilitado(), "{contexto}");
            assert!(
                b.ultimo
                    .contiene("error: más de 256 colores: sin cuantizador"),
                "{contexto}: el motivo del pie; hay {:?}",
                b.ultimo.lista()
            );
            assert!(
                b.ultimo.contiene("cuantizador"),
                "{contexto}: el informe lo explica; hay {:?}",
                b.ultimo.lista()
            );
            assert_eq!(
                b.dialogo().block(),
                Some("más de 256 colores: sin cuantizador"),
                "{contexto}"
            );
            b.pulsa(Key::Enter);
            assert!(b.aceptados.is_empty(), "{contexto}");
            b.clic_en("Aceptar");
            assert!(b.aceptados.is_empty(), "{contexto}");
            assert!(b.modal.is_open());
            // Cualquier permiso marcado, en cualquier destino de salida, no lo activa a Indexado.
            b.elige("destino", 1);
            b.marca("luma_gris_alfa", true);
            b.elige("destino", 0);
            assert!(
                b.aceptar_deshabilitado(),
                "{contexto}: sigue siendo Indexado"
            );
            b.pulsa(Key::Enter);
            assert!(b.aceptados.is_empty(), "{contexto}");
        }
    }
}

#[test]
fn fila_11_cancelar_y_escape_cierran_sin_convertir() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for from in MODOS {
                let mut b = Banco::abierto(ppp, ventana, from);
                b.clic_en("Cancelar");
                assert_eq!(b.cancelados, 1, "{}", b.describe());
                assert!(b.aceptados.is_empty());
                assert!(!b.modal.is_open());

                let mut b = Banco::abierto(ppp, ventana, from);
                b.pulsa(Key::Escape);
                assert_eq!(b.cancelados, 1, "{}", b.describe());
                assert!(b.aceptados.is_empty());
                assert!(!b.modal.is_open());
            }
        }
    }
}

#[test]
fn fila_11_cancelar_con_un_permiso_pendiente_y_con_aceptar_atenuado_tambien_cierra() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.elige_destino(Mode::GrayAlpha);
        assert!(b.aceptar_deshabilitado());
        b.pulsa(Key::Escape);
        assert_eq!(b.cancelados, 1, "{}", b.describe());
        assert!(!b.modal.is_open());
        assert!(b.aceptados.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Tab
// ---------------------------------------------------------------------------------------------

fn comprueba_tab(b: &mut Banco, contexto: &str) {
    let orden = focus_order(b.dialogo());
    // `Aceptar` solo está en el orden si el conjunto es válido.
    assert_eq!(
        orden.contains(&FocusTarget::Accept),
        b.dialogo().is_valid(),
        "{contexto}: Aceptar y la validez"
    );
    assert!(orden.contains(&FocusTarget::Cancel), "{contexto}");
    // Solo lo visible.
    for &objetivo in &orden {
        match objetivo {
            FocusTarget::Field(id)
            | FocusTarget::Option(id, _)
            | FocusTarget::Minus(id)
            | FocusTarget::Plus(id)
            | FocusTarget::Zone(id) => assert!(
                b.dialogo().is_shown(id),
                "{contexto}: {objetivo:?} está oculto y está en el orden"
            ),
            FocusTarget::Accept | FocusTarget::Cancel => {}
        }
    }
    // Un ciclo entero y una vuelta más: cada paso es el siguiente del orden, y da la vuelta.
    let mut actual = b
        .focus
        .unwrap_or_else(|| panic!("{contexto}: no hay foco para empezar"));
    for paso in 0..=orden.len() {
        let posicion = orden
            .iter()
            .position(|t| *t == actual)
            .unwrap_or_else(|| panic!("{contexto}: {actual:?} no está en {orden:?}"));
        let esperado = orden[(posicion + 1) % orden.len()];
        b.pulsa(Key::Tab);
        assert_eq!(
            b.focus,
            Some(esperado),
            "{contexto}: paso {paso} de Tab desde {actual:?}; orden {orden:?}"
        );
        actual = esperado;
    }
}

#[test]
fn fila_11_tab_recorre_solo_lo_visible_y_da_la_vuelta_en_cada_estado() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            // (origen, destino, opción del alfa si procede)
            for (from, destino, alfa) in [
                (Mode::Rgba, Mode::Indexed, None),
                (Mode::Rgba, Mode::GrayAlpha, None),
                (Mode::Rgba, Mode::Gray, None),
                (Mode::Rgba, Mode::Gray, Some(4usize)),
                (Mode::Indexed, Mode::Gray, Some(4)),
                (Mode::GrayAlpha, Mode::Gray, None),
                (Mode::GrayAlpha, Mode::Gray, Some(4)),
                (Mode::Gray, Mode::Indexed, None),
            ] {
                let mut b = Banco::abierto(ppp, ventana, from);
                let indice = destinos(from)
                    .iter()
                    .position(|m| *m == destino)
                    .expect("destino");
                b.elige("destino", indice);
                if let Some(opcion) = alfa {
                    b.elige("alfa_gris", opcion);
                }
                let orden = focus_order(b.dialogo());
                // El grupo del destino lleva siempre sus tres opciones y, solo con Gris, el del alfa.
                for i in 0..3 {
                    assert!(orden.contains(&FocusTarget::Option("destino", i)));
                }
                assert_eq!(
                    orden.contains(&FocusTarget::Option("alfa_gris", 0)),
                    destino == Mode::Gray && from != Mode::Gray
                );
                assert_eq!(
                    orden.contains(&FocusTarget::Field("fondo_hex")),
                    alfa == Some(4) && destino == Mode::Gray
                );
                comprueba_tab(
                    &mut b,
                    &format!("ppp {ppp} {ventana:?} {from:?} a {destino:?} alfa {alfa:?}"),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: desplazamiento a la ventana mínima y a 960 × 720
// ---------------------------------------------------------------------------------------------

/// Pone el origen RGBA con destino Gris y, según `fondo`, «No perder el alfa» (0), «Fondo blanco» (1)
/// o el hexadecimal (4), con eventos directos y un fotograma entre medias.
fn pon_gris(b: &mut Banco, fondo: usize) {
    let gris = destinos(b.from)
        .iter()
        .position(|m| *m == Mode::Gray)
        .expect("destino Gris");
    b.elige("destino", gris);
    b.elige("alfa_gris", fondo);
}

/// Mueve el puntero al relleno izquierdo del cuerpo (`x = 3 u`) y da `muescas` de rueda hacia abajo.
fn rueda_sobre_el_cuerpo(b: &mut Banco, muescas: usize) {
    let layout = b.layout();
    let n = n_de(b.ppp);
    let pos = pos2(
        (layout.body.x + 3 * n) as f32 / b.ppp,
        (layout.body.y + layout.body.height / 2) as f32 / b.ppp,
    );
    b.fotograma(vec![mover(pos)]);
    b.reposo();
    for _ in 0..muescas {
        b.fotograma(vec![rueda_linea(false)]);
    }
    b.reposo();
}

/// La `y` (en puntos) del texto pintado exacto, aunque quede recortado.
fn y_de(b: &Banco, buscado: &str) -> Option<f32> {
    b.ultimo.formas.iter().find_map(|f| match f {
        Forma::Texto(t) if t.texto.trim() == buscado => Some(t.pos.y),
        _ => None,
    })
}

#[test]
fn fila_11_a_la_minima_el_cuerpo_se_desplaza_solo_con_destino_gris_y_solo_si_no_cabe() {
    for ppp in PPPS {
        // Destinos que no son Gris: nunca se desplaza.
        for destino in [Mode::Indexed, Mode::GrayAlpha] {
            let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
            if destino != Mode::Indexed {
                b.elige_destino(destino);
            }
            assert!(b.dialogo().content_h_u() <= 252, "ppp {ppp} {destino:?}");
            assert_eq!(b.layout().max_scroll_u, 0, "ppp {ppp} {destino:?}");
        }

        // Gris con el informe de dos líneas (color y alfa): cabe en 252 u (246).
        let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
        pon_gris(&mut b, 0);
        assert_eq!(
            b.dialogo().content_h_u(),
            246,
            "ppp {ppp}: RGBA a Gris sin fondo"
        );
        assert_eq!(
            b.layout().max_scroll_u,
            0,
            "ppp {ppp}: cabe: no se desplaza"
        );

        // Con «Fondo blanco» el informe gana una línea (fondo): 260 u, no cabe.
        let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
        pon_gris(&mut b, 1);
        assert_eq!(
            b.dialogo().content_h_u(),
            260,
            "ppp {ppp}: RGBA a Gris con fondo"
        );
        assert_eq!(b.layout().max_scroll_u, 260 - 252, "ppp {ppp}");

        // Con el hexadecimal: 296 u.
        let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
        pon_gris(&mut b, 4);
        assert_eq!(
            b.dialogo().content_h_u(),
            296,
            "ppp {ppp}: con el hexadecimal"
        );
        assert_eq!(b.layout().max_scroll_u, 296 - 252, "ppp {ppp}");

        // Desde Gris con alfa (sin casilla) cabe salvo con el hexadecimal.
        for (fondo, desplaza) in [(0usize, false), (1, false), (4, true)] {
            let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::GrayAlpha);
            pon_gris(&mut b, fondo);
            let contenido = b.dialogo().content_h_u();
            assert_eq!(
                b.layout().max_scroll_u,
                (contenido - 252).max(0),
                "ppp {ppp}: GrayAlpha a Gris, alfa {fondo}"
            );
            assert_eq!(
                b.layout().max_scroll_u > 0,
                desplaza,
                "ppp {ppp}: GrayAlpha a Gris, alfa {fondo}, contenido {contenido}"
            );
        }
    }
}

#[test]
fn fila_11_con_el_informe_de_siete_lineas_el_cuerpo_mide_316_352_296_y_332_u() {
    let siete = (1..=7)
        .map(|n| format!("linea {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    for (from, destino, alfa, esperado) in [
        (Mode::Rgba, Mode::Gray, 0usize, 316i64),
        (Mode::Rgba, Mode::Gray, 4, 352),
        (Mode::Indexed, Mode::Gray, 0, 316),
        (Mode::Indexed, Mode::Gray, 4, 352),
        (Mode::GrayAlpha, Mode::Gray, 0, 296),
        (Mode::GrayAlpha, Mode::Gray, 4, 332),
    ] {
        let mut d = Dialog::new(DialogKind::ConvertMode, convert_form(from)).expect("formulario");
        let gris = destinos(from)
            .iter()
            .position(|m| *m == destino)
            .expect("destino");
        d.event(Event::Choice {
            field: "destino",
            index: gris,
        });
        d.event(Event::Choice {
            field: "alfa_gris",
            index: alfa,
        });
        assert!(d.set_report("informe", siete.clone()));
        assert_eq!(d.content_h_u(), esperado, "{from:?} a Gris, alfa {alfa}");
        for ppp in PPPS {
            // A la mínima, el cuerpo mide 252 u y se desplaza lo que falta; a 960 × 720, nada.
            let n = n_de(ppp) as u32;
            let minima = dialog_layout(pixeles(ppp, Ventana::Minima), n, esperado, false, true);
            assert_eq!(minima.max_scroll_u, esperado - 252, "{from:?} ppp {ppp}");
            let grande = dialog_layout(pixeles(ppp, Ventana::Grande), n, esperado, false, true);
            assert_eq!(grande.max_scroll_u, 0, "{from:?} ppp {ppp}");
        }
    }
    // Con cualquier otro destino y siete líneas, cabe a la mínima.
    for (from, destino) in [
        (Mode::Rgba, Mode::Indexed),
        (Mode::Rgba, Mode::GrayAlpha),
        (Mode::Indexed, Mode::Rgba),
        (Mode::GrayAlpha, Mode::Indexed),
        (Mode::Gray, Mode::Rgba),
    ] {
        let mut d = Dialog::new(DialogKind::ConvertMode, convert_form(from)).expect("formulario");
        let indice = destinos(from)
            .iter()
            .position(|m| *m == destino)
            .expect("destino");
        d.event(Event::Choice {
            field: "destino",
            index: indice,
        });
        assert!(d.set_report("informe", siete.clone()));
        assert!(
            d.content_h_u() <= 252,
            "{from:?} a {destino:?}: {}",
            d.content_h_u()
        );
    }
}

#[test]
fn fila_11_a_la_minima_la_rueda_desplaza_el_cuerpo_y_titulo_pie_y_botones_no_se_mueven() {
    for ppp in PPPS {
        for fondo in [1usize, 4] {
            let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
            pon_gris(&mut b, fondo);
            let contexto = format!("ppp {ppp}, alfa {fondo}");
            let maximo = b.layout().max_scroll_u;
            assert!(maximo > 0, "{contexto}");
            let fijos = ["Convertir modo", "Aceptar", "Cancelar"];
            let antes_fijos: Vec<Option<f32>> = fijos.iter().map(|t| y_de(&b, t)).collect();
            let etiqueta_antes = y_de(&b, "Modo de destino").expect("Modo de destino");
            assert_eq!(b.dialogo().scroll_u(), 0, "{contexto}");

            rueda_sobre_el_cuerpo(&mut b, 2);
            assert_eq!(b.dialogo().scroll_u(), maximo, "{contexto}: hasta el final");
            let desplazado = etiqueta_antes - y_de(&b, "Modo de destino").expect("Modo de destino");
            assert!(
                (desplazado - maximo as f32 * b.u()).abs() <= 0.1,
                "{contexto}: el cuerpo se movió {desplazado} pt y debía {}",
                maximo as f32 * b.u()
            );
            let despues_fijos: Vec<Option<f32>> = fijos.iter().map(|t| y_de(&b, t)).collect();
            assert_eq!(
                despues_fijos, antes_fijos,
                "{contexto}: título, pie y botones fijos"
            );
            assert!(b.ultimo.exacto("Aceptar").is_some(), "{contexto}");
            assert!(b.ultimo.exacto("Cancelar").is_some(), "{contexto}");
            // El motivo del pie sigue visible con el cuerpo desplazado.
            assert!(
                b.ultimo.contiene("error: ") || !b.aceptar_deshabilitado(),
                "{contexto}: si Aceptar está atenuado, el motivo se ve"
            );
        }
    }
}

#[test]
fn fila_11_a_la_minima_el_motivo_y_los_botones_se_ven_con_el_cuerpo_al_final() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
        pon_gris(&mut b, 4);
        assert!(b.aceptar_deshabilitado());
        rueda_sobre_el_cuerpo(&mut b, 3);
        assert!(b.dialogo().scroll_u() > 0, "ppp {ppp}");
        assert!(
            b.ultimo.contiene("error: falta permitir perder el color"),
            "ppp {ppp}: el motivo del pie; hay {:?}",
            b.ultimo.lista()
        );
        assert!(b.ultimo.exacto("Aceptar").is_some());
        assert!(b.ultimo.exacto("Cancelar").is_some());
    }
}

#[test]
fn fila_11_a_la_minima_tab_lleva_a_la_vista_el_hexadecimal_que_estaba_fuera() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Minima, Mode::Rgba);
        pon_gris(&mut b, 4);
        assert_eq!(b.dialogo().scroll_u(), 0, "ppp {ppp}");
        b.foco_hasta(FocusTarget::Field("fondo_hex"));
        assert!(
            b.dialogo().scroll_u() > 0,
            "ppp {ppp}: el foco en el hexadecimal desplaza el cuerpo"
        );
        assert!(b.ultimo.exacto("Cancelar").is_some());
        assert_eq!(
            b.foco_de_egui(),
            Some(Banco::id_de("fondo_hex")),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_11_a_960_por_720_el_formulario_no_se_desplaza_en_ningun_estado() {
    for ppp in PPPS {
        for (from, fondo) in [(Mode::Rgba, 0usize), (Mode::Rgba, 4), (Mode::GrayAlpha, 4)] {
            let mut b = Banco::abierto(ppp, Ventana::Grande, from);
            pon_gris(&mut b, fondo);
            let contexto = format!("ppp {ppp} {from:?} alfa {fondo}");
            assert_eq!(b.layout().max_scroll_u, 0, "{contexto}");
            let fijos = ["Convertir modo", "Modo de destino", "Aceptar", "Cancelar"];
            let antes: Vec<Option<f32>> = fijos.iter().map(|t| y_de(&b, t)).collect();
            rueda_sobre_el_cuerpo(&mut b, 3);
            assert_eq!(b.dialogo().scroll_u(), 0, "{contexto}");
            let despues: Vec<Option<f32>> = fijos.iter().map(|t| y_de(&b, t)).collect();
            assert_eq!(despues, antes, "{contexto}: nada se mueve");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: contraste y glifos
// ---------------------------------------------------------------------------------------------

const MIN_TEXTO: f64 = 4.5;

/// El último relleno opaco anterior a `hasta` que contiene `sonda` (con el recorte aplicado).
fn relleno_bajo(formas: &[Forma], hasta: usize, sonda: Rect) -> Option<Relleno> {
    formas[..hasta].iter().rev().find_map(|f| match f {
        Forma::Relleno(r)
            if r.fill.a() == 255 && r.rect.intersect(r.clip).expand(0.01).contains_rect(sonda) =>
        {
            Some(*r)
        }
        _ => None,
    })
}

/// Los fallos de contraste de los textos del diálogo (no los de detrás, ni los atenuados) y
/// cuántos textos se comprobaron.
fn fallos_de_contraste(b: &Banco) -> (Vec<String>, usize) {
    let area = {
        let l = b.layout().dialog;
        let ppp = b.ppp;
        Rect::from_min_max(
            pos2(l.x as f32 / ppp, l.y as f32 / ppp),
            pos2((l.x + l.width) as f32 / ppp, (l.y + l.height) as f32 / ppp),
        )
        .expand(0.5)
    };
    let mut fallos = Vec::new();
    let mut comprobados = 0;
    for (i, forma) in b.ultimo.formas.iter().enumerate() {
        let Forma::Texto(t) = forma else { continue };
        let visible = t.rect().intersect(t.clip);
        if t.texto.trim().is_empty() || !visible.is_positive() || !area.contains_rect(visible) {
            continue;
        }
        if t.deshabilitado() {
            continue;
        }
        let sonda = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let Some(fondo) = relleno_bajo(&b.ultimo.formas, i, sonda) else {
            fallos.push(format!("el texto {:?} no tiene relleno debajo", t.texto));
            continue;
        };
        for color in &t.colores {
            comprobados += 1;
            let ratio = contrast_ratio(*color, fondo.fill);
            if ratio < MIN_TEXTO {
                fallos.push(format!(
                    "el texto {:?} ({color:?}) sobre {:?} da {ratio:.2} (< {MIN_TEXTO})",
                    t.texto, fondo.fill
                ));
            }
        }
    }
    (fallos, comprobados)
}

fn comprueba_contraste(b: &Banco, contexto: &str) {
    let (fallos, comprobados) = fallos_de_contraste(b);
    assert!(
        comprobados >= 5,
        "{contexto}: solo se comprobaron {comprobados} textos"
    );
    assert!(fallos.is_empty(), "{contexto}: {fallos:#?}");
}

#[test]
fn fila_11_todo_texto_pintado_del_dialogo_tiene_contraste_al_menos_4_5() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let contexto = |estado: &str| format!("ppp {ppp} {ventana:?}: {estado}");

            // Por defecto, desde cada origen.
            for from in MODOS {
                let b = Banco::abierto(ppp, ventana, from);
                comprueba_contraste(&b, &contexto(&format!("por defecto desde {from:?}")));
            }

            // Con un permiso por cubrir: el motivo del pie y Aceptar atenuado (exento).
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            b.elige_destino(Mode::GrayAlpha);
            comprueba_contraste(&b, &contexto("falta el permiso del color"));
            b.clic_en(CASILLA);
            comprueba_contraste(&b, &contexto("casilla marcada"));

            // RGBA a Gris con el informe de color, alfa y fondo.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            pon_gris(&mut b, 1);
            comprueba_contraste(&b, &contexto("Gris con fondo blanco"));
            pon_gris(&mut b, 0);
            comprueba_contraste(&b, &contexto("Gris sin fondo: falta el color"));
            b.marca("luma_gris", true);
            comprueba_contraste(&b, &contexto("Gris sin fondo: falta el fondo"));

            // Con el hexadecimal: inválido y con alfa.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            pon_gris(&mut b, 4);
            b.foco_hasta(FocusTarget::Field("fondo_hex"));
            b.escribe("zz");
            comprueba_contraste(&b, &contexto("hexadecimal inválido"));
            b.escribe("#ff000080");
            comprueba_contraste(&b, &contexto("hexadecimal con alfa"));
            b.escribe("#ff0000");
            comprueba_contraste(&b, &contexto("hexadecimal válido"));

            // Más de 256 colores: el informe del cuantizador y su motivo.
            let b = Banco::abierto_con(
                ppp,
                ventana,
                Mode::Rgba,
                rgba_de_257_colores(),
                Some(ACTIVO),
            );
            comprueba_contraste(&b, &contexto("más de 256 colores"));

            // Con el color activo con alfa: el motivo con su valor resuelto.
            let mut b = Banco::abierto_con(
                ppp,
                ventana,
                Mode::Rgba,
                imagen_de_banco(Mode::Rgba),
                Some(Rgba::new(0xff, 0, 0, 0x80)),
            );
            pon_gris(&mut b, 3);
            b.marca("luma_gris", true);
            comprueba_contraste(&b, &contexto("color activo con alfa"));
        }
    }
}

#[test]
fn fila_11_ningun_estado_del_dialogo_pinta_un_glifo_ausente() {
    // `MissingGlyphPolicy::Panic` hace fallar cualquier texto con un carácter que la fuente no tenga.
    // Se recorren los textos del formulario (rótulos, opciones, motivos y el informe más largo).
    for ppp in PPPS {
        let mut b = Banco::abierto_con(
            ppp,
            Ventana::Grande,
            Mode::Rgba,
            rgba_de_257_colores(),
            Some(Rgba::new(0xff, 0, 0, 0x80)),
        );
        for destino in destinos(Mode::Rgba) {
            b.elige_destino(destino);
        }
        pon_gris(&mut b, 3);
        pon_gris(&mut b, 4);
        b.foco_hasta(FocusTarget::Field("fondo_hex"));
        b.escribe("#12");
        b.escribe("#ff000080");
        let mut b = Banco::abierto_con(
            ppp,
            Ventana::Grande,
            Mode::Rgba,
            rgba_de_257_colores(),
            None,
        );
        assert!(
            b.ultimo.contiene("cuantizador"),
            "ppp {ppp}: {:?}",
            b.ultimo.lista()
        );
        pon_gris(&mut b, 3);
        // Los textos pintados están en el juego.
        for texto in b.ultimo.visibles() {
            assert!(en_el_juego(&texto.texto), "ppp {ppp}: {:?}", texto.texto);
        }
    }
}
