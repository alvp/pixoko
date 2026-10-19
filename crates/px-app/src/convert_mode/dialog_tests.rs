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
//!   (`falta: permitir perder el color`, sin `error:`); marcar la casilla lo activa.
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
                    b.ultimo.contiene("falta: permitir perder el color"),
                    "{contexto}: el motivo del pie es visible; hay {:?}",
                    b.ultimo.lista()
                );
                assert_eq!(
                    b.dialogo().block(),
                    Some("falta: permitir perder el color"),
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
                    b.ultimo.contiene("falta: permitir perder el color"),
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
            b.ultimo.contiene("falta: perder el color; fondo del alfa"),
            "{}: {:?}",
            contexto("faltan la casilla y el fondo"),
            b.ultimo.lista()
        );

        b.clic_en(CASILLA);
        assert!(
            b.aceptar_deshabilitado(),
            "{}",
            contexto("con la casilla y sin fondo")
        );
        assert!(
            b.ultimo.contiene("falta: elegir el fondo del alfa"),
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
        assert!(b.ultimo.contiene("falta: elegir el fondo del alfa"));
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
                b.ultimo.contiene("error: ")
                    || b.ultimo.contiene("falta: ")
                    || !b.aceptar_deshabilitado(),
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
            b.ultimo.contiene("falta: permitir perder el color"),
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

// ---------------------------------------------------------------------------------------------
// Unidad 20b, parte 1 (specs/hito-2-motivo-y-marcadores.md): filas 7 y 8. En ROJO hasta que
// existan `Dialog::problems`, `Dialog::set_summary`, `Dialog::summary`, `footer_text` y el
// resumen de `sync_convert`.
//
// - Fila 7: el caso de la prueba manual con clics reales. El pie se lee del texto pintado en el
//   rectángulo del motivo (`layout.reason`), no de `Dialog::reason()`. El cambio tras un clic se ve
//   **en el fotograma siguiente** al del clic (`clic_y_siguiente`: mover, pulsar, soltar y un
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
// - `faltan 3: color, fondo, valor` no se pinta (en el diálogo no es alcanzable y `footer_text` le
//   pondría `error: `, ver `tests.rs`).
// ---------------------------------------------------------------------------------------------

const FALTA_AMBOS_20B: &str = "falta: perder el color; fondo del alfa";
const FALTA_COLOR_20B: &str = "falta: permitir perder el color";
const ELIGE_FONDO_20B: &str = "falta: elegir el fondo del alfa";
const FALTA_COLOR_Y_VALOR_20B: &str = "falta: perder el color; fondo válido";

/// El texto pintado en el rectángulo del motivo (el pie), si lo hay.
fn pie_de(b: &Banco) -> Option<Texto> {
    let r = b.layout().reason;
    let ppp = b.ppp;
    b.ultimo
        .visibles()
        .find(|t| {
            (t.pos.x * ppp - r.x as f32).abs() < 1.5 && (t.pos.y * ppp - r.y as f32).abs() < 1.5
        })
        .cloned()
}

fn pie_texto(b: &Banco) -> Option<String> {
    pie_de(b).map(|t| t.texto.trim().to_string())
}

/// Un clic real sobre el texto `buscado` y **un** fotograma más: el siguiente al del evento.
fn clic_y_siguiente(b: &mut Banco, buscado: &str) {
    let centro = b.centro_de(buscado);
    b.clic(centro);
    b.fotograma(vec![]);
}

fn error_del_campo(corto: &str) -> String {
    format!("{ERROR_PREFIX}{corto}")
}

#[test]
fn fila_7_20b_rgba_con_transparencia_a_gris_sigue_el_caso_de_la_prueba_manual() {
    let corto = hex_notice(&HexError::Length(2));
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        let ctx = |paso: &str| format!("ppp {ppp}: {paso}");

        // Gris, sin marcar nada: faltan el color y el fondo, sin `error:`.
        b.elige_destino(Mode::Gray);
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_AMBOS_20B),
            "{}: {:?}",
            ctx("sin nada"),
            b.ultimo.lista()
        );
        assert!(b.aceptar_deshabilitado(), "{}", ctx("sin nada"));
        assert!(!b.ultimo.contiene(ERROR_PREFIX), "{}", ctx("sin nada"));

        // La casilla: queda el fondo, en el fotograma siguiente al clic.
        clic_y_siguiente(&mut b, CASILLA);
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(ELIGE_FONDO_20B),
            "{}: {:?}",
            ctx("casilla"),
            b.ultimo.lista()
        );
        assert!(b.aceptar_deshabilitado(), "{}", ctx("casilla"));

        // Un fondo: sin motivo y Aceptar activo, también en el fotograma siguiente.
        clic_y_siguiente(&mut b, "Fondo blanco");
        assert_eq!(
            pie_texto(&b),
            None,
            "{}: {:?}",
            ctx("blanco"),
            b.ultimo.lista()
        );
        assert!(!b.aceptar_deshabilitado(), "{}", ctx("blanco"));
        assert_eq!(b.dialogo().reason(), None, "{}", ctx("blanco"));
        assert!(!b.ultimo.contiene("falta:") && !b.ultimo.contiene(ERROR_PREFIX));

        // Volver a «No perder el alfa»: otra vez el fondo.
        clic_y_siguiente(&mut b, "No perder el alfa");
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(ELIGE_FONDO_20B),
            "{}",
            ctx("sin fondo")
        );
        assert!(b.aceptar_deshabilitado(), "{}", ctx("sin fondo"));

        // Desmarcar la casilla con el fondo ya cubierto: solo el color.
        clic_y_siguiente(&mut b, "Fondo negro");
        assert_eq!(pie_texto(&b), None, "{}", ctx("negro"));
        clic_y_siguiente(&mut b, CASILLA);
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_COLOR_20B),
            "{}",
            ctx("sin casilla")
        );

        // Hexadecimal ilegible con la casilla sin marcar: color y fondo válido; el error del campo
        // sigue junto al campo.
        clic_y_siguiente(&mut b, "Fondo: un color hexadecimal");
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_COLOR_20B),
            "{}: el hexadecimal por defecto vale",
            ctx("hexadecimal")
        );
        b.foco_hasta(FocusTarget::Field("fondo_hex"));
        b.escribe("zz");
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_COLOR_Y_VALOR_20B),
            "{}: {:?}",
            ctx("zz sin casilla"),
            b.ultimo.lista()
        );
        assert!(
            b.ultimo.exacto(&error_del_campo(corto)).is_some(),
            "{}: el error del campo sigue junto al campo; hay {:?}",
            ctx("zz sin casilla"),
            b.ultimo.lista()
        );
        assert!(
            !b.ultimo.contiene(&format!("{ERROR_PREFIX}Color:")),
            "{}: el detalle no va en el pie",
            ctx("zz sin casilla")
        );
        assert!(b.aceptar_deshabilitado(), "{}", ctx("zz sin casilla"));
        assert_eq!(b.dialogo().summary(), Some(FALTA_COLOR_Y_VALOR_20B));

        // La casilla marcada con eventos directos (sin mover el foco): solo el valor, con `error:`.
        b.marca("luma_gris", true);
        assert_eq!(
            pie_texto(&b),
            Some(format!("{ERROR_PREFIX}Color: {corto}")),
            "{}: {:?}",
            ctx("zz con casilla"),
            b.ultimo.lista()
        );
        assert!(
            b.ultimo.exacto(&error_del_campo(corto)).is_some(),
            "{}",
            ctx("zz con casilla")
        );
        assert_eq!(b.dialogo().summary(), None, "{}", ctx("zz con casilla"));
        assert!(b.aceptar_deshabilitado());

        // Y desmarcarla vuelve al resumen.
        b.marca("luma_gris", false);
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_COLOR_Y_VALOR_20B),
            "{}",
            ctx("otra vez")
        );

        // Arreglar el hexadecimal deja el color; con la casilla, nada.
        b.escribe("#ff0000");
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_COLOR_20B),
            "{}",
            ctx("arreglado")
        );
        b.marca("luma_gris", true);
        assert_eq!(pie_texto(&b), None, "{}", ctx("todo cubierto"));
        assert!(!b.aceptar_deshabilitado(), "{}", ctx("todo cubierto"));
    }
}

#[test]
fn fila_7_20b_con_la_casilla_marcada_antes_de_escribir_zz_el_pie_es_solo_el_error_del_valor() {
    let corto = hex_notice(&HexError::Length(2));
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.elige_destino(Mode::Gray);
        clic_y_siguiente(&mut b, CASILLA);
        clic_y_siguiente(&mut b, "Fondo: un color hexadecimal");
        b.foco_hasta(FocusTarget::Field("fondo_hex"));
        b.escribe("zz");
        assert_eq!(
            pie_texto(&b),
            Some(format!("{ERROR_PREFIX}Color: {corto}")),
            "ppp {ppp}: {:?}",
            b.ultimo.lista()
        );
        assert!(!b.ultimo.contiene("falta:"), "ppp {ppp}");
        assert!(
            b.ultimo.exacto(&error_del_campo(corto)).is_some(),
            "ppp {ppp}"
        );
        assert_eq!(b.dialogo().summary(), None, "ppp {ppp}");
        assert!(b.aceptar_deshabilitado());
    }
}

#[test]
fn fila_7_20b_el_hexadecimal_con_alfa_sin_la_casilla_tambien_da_color_y_fondo_valido() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.elige_destino(Mode::Gray);
        clic_y_siguiente(&mut b, "Fondo: un color hexadecimal");
        b.foco_hasta(FocusTarget::Field("fondo_hex"));
        b.escribe("#ff000080");
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some(FALTA_COLOR_Y_VALOR_20B),
            "ppp {ppp}: {:?}",
            b.ultimo.lista()
        );
        assert!(b.aceptar_deshabilitado(), "ppp {ppp}");
        b.marca("luma_gris", true);
        assert_eq!(
            pie_texto(&b).as_deref(),
            Some("error: Color: el fondo no lleva alfa"),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_7_20b_gris_con_alfa_desde_rgba_solo_pide_el_color_y_no_hay_resumen() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.elige_destino(Mode::GrayAlpha);
        assert_eq!(pie_texto(&b).as_deref(), Some(FALTA_COLOR_20B), "ppp {ppp}");
        assert_eq!(b.dialogo().summary(), None, "ppp {ppp}");
        clic_y_siguiente(&mut b, CASILLA);
        assert_eq!(pie_texto(&b), None, "ppp {ppp}");
        assert!(!b.aceptar_deshabilitado(), "ppp {ppp}");
    }
}

// --- Fila 8: encaje del texto ---

/// Un rectángulo en píxeles físicos pasado a puntos.
fn rect_en_puntos(r: px_ui::view::ScreenRect, ppp: f32) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32 / ppp, r.y as f32 / ppp),
        vec2(r.width as f32 / ppp, r.height as f32 / ppp),
    )
}

/// Comprueba el pie pintado de `b`: es `footer_text(motivo)`, mide como mucho 44 columnas, cabe en
/// `layout.reason.width`, no se recorta ni toca los botones y lleva su pista (`falta:` o `error:`).
fn comprueba_pie(b: &Banco, motivo: &str, ctx: &str) {
    let esperado = px_ui::dialog_view::footer_text(motivo);
    assert!(esperado.chars().count() <= 44, "{ctx}: {esperado:?}");
    assert!(
        esperado.starts_with("falta: ") || esperado.starts_with(ERROR_PREFIX),
        "{ctx}: la pista textual falta en {esperado:?}"
    );
    assert!(en_el_juego(&esperado), "{ctx}: {esperado:?}");
    let t = pie_de(b).unwrap_or_else(|| {
        panic!(
            "{ctx}: no se pinta el pie {esperado:?}; hay {:?}",
            b.ultimo.lista()
        )
    });
    assert_eq!(t.texto.trim(), esperado, "{ctx}");

    let layout = b.layout();
    let n = n_de(b.ppp);
    assert_eq!(
        layout.reason.width,
        308 * n,
        "{ctx}: el pie mide 44 columnas de 7 u"
    );
    let ancho_px = t.tamano.x * b.ppp;
    assert!(
        ancho_px <= layout.reason.width as f32 + 0.5,
        "{ctx}: {esperado:?} mide {ancho_px} px y el pie {} px",
        layout.reason.width
    );
    // Sin recorte.
    assert!(
        t.clip.expand(0.5).contains_rect(t.rect()),
        "{ctx}: {esperado:?} queda recortado ({:?} en {:?})",
        t.rect(),
        t.clip
    );
    // Sin solapamiento con los botones.
    let zona = t.rect();
    for boton in [Some(layout.accept), layout.cancel].into_iter().flatten() {
        assert!(
            !zona.intersect(rect_en_puntos(boton, b.ppp)).is_positive(),
            "{ctx}: {esperado:?} toca un botón"
        );
    }
    // Contraste.
    assert!(
        contrast_ratio(
            px_ui::dialog_view::ERROR_FG,
            px_ui::dialog_view::DIALOG_FILL
        ) >= MIN_TEXTO
    );
    comprueba_contraste(b, ctx);
}

#[derive(Clone, Copy, Debug)]
enum Via {
    Bloque,
    Resumen,
}

/// Los pies de D2 y los más largos posibles, y por dónde llegan al diálogo.
fn pies_fijados() -> Vec<(String, Via)> {
    vec![
        (FALTA_COLOR_20B.to_string(), Via::Bloque),
        (ELIGE_FONDO_20B.to_string(), Via::Bloque),
        (FALTA_AMBOS_20B.to_string(), Via::Resumen),
        (FALTA_COLOR_Y_VALOR_20B.to_string(), Via::Resumen),
        (
            "falta: fondo del alfa; fondo válido".to_string(),
            Via::Resumen,
        ),
        ("no hay color activo".to_string(), Via::Bloque),
        (
            "el color activo #ff000080 tiene alfa".to_string(),
            Via::Bloque,
        ),
        (
            "más de 256 colores: sin cuantizador".to_string(),
            Via::Bloque,
        ),
        ("la imagen convertida no cabe".to_string(), Via::Bloque),
        ("calculando el informe".to_string(), Via::Bloque),
        // El motivo más largo con `error: `: 37 caracteres, 44 en total.
        ("M".repeat(37), Via::Bloque),
        // El límite físico de `falta: `: 37 tras el prefijo, 44 en total.
        (format!("falta: {}", "M".repeat(37)), Via::Bloque),
        (format!("falta: {}", "M".repeat(37)), Via::Resumen),
    ]
}

#[test]
fn fila_8_20b_los_pies_fijados_caben_a_las_cinco_escalas_en_las_dos_ventanas() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            // Un solo banco por escala y ventana: sin `ConvertSync` el motivo no se recalcula.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            b.convert = None;
            for (motivo, via) in pies_fijados() {
                let ctx = format!("ppp {ppp} {ventana:?} {via:?} {motivo:?}");
                b.dialogo_mut().set_block(None);
                b.dialogo_mut().set_summary(None);
                match via {
                    Via::Bloque => b.dialogo_mut().set_block(Some(motivo.clone())),
                    Via::Resumen => b.dialogo_mut().set_summary(Some(motivo.clone())),
                }
                b.reposo();
                assert!(b.aceptar_deshabilitado(), "{ctx}");
                let pintado = b.dialogo().reason().expect("hay motivo");
                comprueba_pie(&b, &pintado, &ctx);
            }
        }
    }
}

#[test]
fn fila_8_20b_el_pie_mas_largo_con_error_y_el_mas_largo_con_falta_llenan_las_44_columnas() {
    // 37 caracteres con `error: ` y `falta: ` + 37: 44 columnas de 7 u cada uno.
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.convert = None;
        for motivo in ["M".repeat(37), format!("falta: {}", "M".repeat(37))] {
            b.dialogo_mut().set_block(Some(motivo.clone()));
            b.reposo();
            let pie = pie_de(&b).expect("pie");
            assert_eq!(
                pie.texto.trim().chars().count(),
                44,
                "ppp {ppp}: {motivo:?}"
            );
            let layout = b.layout();
            // Fuente monoespaciada: 44 columnas ocupan como mucho el ancho del pie.
            assert!(
                pie.tamano.x * ppp <= layout.reason.width as f32 + 0.5,
                "ppp {ppp}: {motivo:?} mide {} px de {}",
                pie.tamano.x * ppp,
                layout.reason.width
            );
        }
    }
}

#[test]
fn fila_8_20b_los_pies_reales_de_cada_estado_caben_a_las_cinco_escalas_en_las_dos_ventanas() {
    let corto = hex_notice(&HexError::Length(2));
    for ppp in PPPS {
        for ventana in VENTANAS {
            let ctx = |estado: &str| format!("ppp {ppp} {ventana:?}: {estado}");

            // Gris sin nada: color y fondo; con la casilla: el fondo.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            pon_gris(&mut b, 0);
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(motivo, FALTA_AMBOS_20B, "{}", ctx("sin nada"));
            comprueba_pie(&b, &motivo, &ctx("sin nada"));
            b.marca("luma_gris", true);
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(motivo, ELIGE_FONDO_20B, "{}", ctx("casilla"));
            comprueba_pie(&b, &motivo, &ctx("casilla"));

            // Gris con alfa sin la casilla.
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            b.elige_destino(Mode::GrayAlpha);
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(motivo, FALTA_COLOR_20B, "{}", ctx("gris con alfa"));
            comprueba_pie(&b, &motivo, &ctx("gris con alfa"));

            // Hexadecimal ilegible: con la casilla sin marcar (resumen) y con ella (valor solo).
            let mut b = Banco::abierto(ppp, ventana, Mode::Rgba);
            pon_gris(&mut b, 4);
            b.foco_hasta(FocusTarget::Field("fondo_hex"));
            b.escribe("zz");
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(motivo, FALTA_COLOR_Y_VALOR_20B, "{}", ctx("zz sin casilla"));
            comprueba_pie(&b, &motivo, &ctx("zz sin casilla"));
            b.marca("luma_gris", true);
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(
                motivo,
                format!("Color: {corto}"),
                "{}",
                ctx("zz con casilla")
            );
            comprueba_pie(&b, &motivo, &ctx("zz con casilla"));
            b.escribe("#ff000080");
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(motivo, "Color: el fondo no lleva alfa", "{}", ctx("alfa"));
            comprueba_pie(&b, &motivo, &ctx("alfa"));

            // Color activo con alfa: color y fondo válido (sin resumen).
            let mut b = Banco::abierto_con(
                ppp,
                ventana,
                Mode::Rgba,
                imagen_de_banco(Mode::Rgba),
                Some(Rgba::new(0xff, 0, 0, 0x80)),
            );
            pon_gris(&mut b, 3);
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(
                motivo,
                FALTA_COLOR_Y_VALOR_20B,
                "{}",
                ctx("activo con alfa")
            );
            comprueba_pie(&b, &motivo, &ctx("activo con alfa"));
            b.marca("luma_gris", true);
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(
                motivo,
                "el color activo #ff000080 tiene alfa",
                "{}",
                ctx("activo")
            );
            comprueba_pie(&b, &motivo, &ctx("activo"));

            // Más de 256 colores.
            let b = Banco::abierto_con(
                ppp,
                ventana,
                Mode::Rgba,
                rgba_de_257_colores(),
                Some(ACTIVO),
            );
            let motivo = b.dialogo().reason().expect("hay motivo");
            assert_eq!(
                motivo,
                "más de 256 colores: sin cuantizador",
                "{}",
                ctx("257")
            );
            comprueba_pie(&b, &motivo, &ctx("257"));
        }
    }
}

#[test]
fn fila_8_20b_la_pista_esta_en_el_texto_y_no_solo_en_el_color() {
    let mut b = Banco::abierto(2.0, Ventana::Grande, Mode::Rgba);
    pon_gris(&mut b, 0);
    let t = pie_de(&b).expect("pie");
    assert!(t.texto.trim().starts_with("falta:"), "{:?}", t.texto);
    b.marca("luma_gris", true);
    b.elige("alfa_gris", 3);
    b.dialogo_mut()
        .set_block(Some("no hay color activo".to_string()));
    b.convert = None;
    b.reposo();
    let t = pie_de(&b).expect("pie");
    assert!(t.texto.trim().starts_with("error:"), "{:?}", t.texto);
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
// que es donde están el centro, el radio y el trazo de un círculo (el `Pintado` de este banco solo
// guarda textos y rellenos). El banco no guarda las formas, así que `crudas` ejecuta un fotograma
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
//   `y = rótulo − 1 u`, 16 u de alto, `18 u + 7 u · letras` de ancho), el mismo en opción y casilla.

use px_ui::dialog_view::{DIALOG_FILL, FIELD_FILL, TOGGLE_EDGE};

const ACENTO: Color32 = px_ui::theme::ACCENT;
const MIN_MARCA_20B: f64 = 3.0;

#[derive(Clone, Copy, Debug)]
struct Disco {
    centro: Pos2,
    radio: f32,
    relleno: Color32,
    /// `TRANSPARENT` y `0.0` si no tiene trazo visible.
    trazo_color: Color32,
    trazo_ancho: f32,
    orden: usize,
}

#[derive(Clone, Copy, Debug)]
struct Caja {
    rect: Rect,
    relleno: Color32,
    trazo_color: Color32,
    trazo_ancho: f32,
}

#[derive(Debug, Default)]
struct Marcas {
    discos: Vec<Disco>,
    cajas: Vec<Caja>,
}

fn trazo_visible(s: eframe::egui::Stroke) -> (Color32, f32) {
    if s.width > 0.0 && s.color != Color32::TRANSPARENT {
        (s.color, s.width)
    } else {
        (Color32::TRANSPARENT, 0.0)
    }
}

fn recoge_marcas(shape: &Shape, fuera: &mut Marcas, orden: &mut usize) {
    match shape {
        Shape::Circle(c) => {
            let (trazo_color, trazo_ancho) = trazo_visible(c.stroke);
            fuera.discos.push(Disco {
                centro: c.center,
                radio: c.radius,
                relleno: c.fill,
                trazo_color,
                trazo_ancho,
                orden: *orden,
            });
            *orden += 1;
        }
        Shape::Rect(r) => {
            let (trazo_color, trazo_ancho) = trazo_visible(r.stroke);
            fuera.cajas.push(Caja {
                rect: r.rect,
                relleno: r.fill,
                trazo_color,
                trazo_ancho,
            });
            *orden += 1;
        }
        Shape::Vec(formas) => {
            for forma in formas {
                recoge_marcas(forma, fuera, orden);
            }
        }
        _ => {}
    }
}

fn marcas_de(formas: &[ClippedShape]) -> Marcas {
    let mut fuera = Marcas::default();
    let mut orden = 0;
    for clipped in formas {
        recoge_marcas(&clipped.shape, &mut fuera, &mut orden);
    }
    fuera
}

fn cerca_u(a: f32, b: f32, u: f32) -> bool {
    (a - b).abs() <= 0.05 * u
}

fn mismo_centro(a: Pos2, b: Pos2, u: f32) -> bool {
    cerca_u(a.x, b.x, u) && cerca_u(a.y, b.y, u)
}

/// Los aros (círculos con trazo `TOGGLE_EDGE`), de arriba abajo.
fn aros(m: &Marcas) -> Vec<Disco> {
    let mut v: Vec<Disco> = m
        .discos
        .iter()
        .copied()
        .filter(|d| d.trazo_color == TOGGLE_EDGE)
        .collect();
    v.sort_by(|a, b| a.centro.y.total_cmp(&b.centro.y));
    v
}

/// Los puntos (círculos rellenos de `ACCENT`), de arriba abajo.
fn puntos(m: &Marcas) -> Vec<Disco> {
    let mut v: Vec<Disco> = m
        .discos
        .iter()
        .copied()
        .filter(|d| d.relleno == ACENTO)
        .collect();
    v.sort_by(|a, b| a.centro.y.total_cmp(&b.centro.y));
    v
}

/// Las cajas de casilla: rectángulos con trazo `TOGGLE_EDGE` de hasta 20 u de lado (`TOGGLE_EDGE`
/// es `TEXT_DIM` y otros trazos grandes, como el marco del diálogo, pueden tener ese color).
fn casillas(m: &Marcas, u: f32) -> Vec<Caja> {
    let mut v: Vec<Caja> = m
        .cajas
        .iter()
        .copied()
        .filter(|c| c.trazo_color == TOGGLE_EDGE)
        .filter(|c| c.rect.width() <= 20.0 * u && c.rect.height() <= 20.0 * u)
        .collect();
    v.sort_by(|a, b| a.rect.center().y.total_cmp(&b.rect.center().y));
    v
}

/// Las marcas cuadradas: rellenos `ACCENT` sin trazo de hasta 10 u con el centro a menos de 5 u en
/// vertical de `y`.
fn marcas_cuadradas(m: &Marcas, u: f32, y: f32) -> Vec<Caja> {
    m.cajas
        .iter()
        .copied()
        .filter(|c| c.relleno == ACENTO && c.trazo_ancho == 0.0)
        .filter(|c| c.rect.width() <= 10.0 * u + 0.05 && c.rect.height() <= 10.0 * u + 0.05)
        .filter(|c| (c.rect.center().y - y).abs() <= 5.0 * u)
        .collect()
}

/// El relleno de un aro: el suyo o, si no tiene, el del último círculo relleno anterior y concéntrico.
fn relleno_de_aro(m: &Marcas, aro: &Disco) -> Color32 {
    if aro.relleno != Color32::TRANSPARENT {
        return aro.relleno;
    }
    m.discos
        .iter()
        .filter(|d| d.orden < aro.orden && d.relleno != Color32::TRANSPARENT)
        .rfind(|d| (d.centro - aro.centro).length() < 0.01)
        .map_or(Color32::TRANSPARENT, |d| d.relleno)
}

/// Lo que se espera de los marcadores según el estado del diálogo, comparado con lo pintado en
/// `formas` (ver la cabecera de la sección). Devuelve lo leído, para seguir con ello.
fn afirma_marcadores(
    dialog: &Dialog,
    formas: &[ClippedShape],
    ppp: f32,
    dialogo_x_px: i64,
    donde: &str,
) -> Marcas {
    let n = i64::from(text_scale(ppp));
    let u = n as f32 / ppp;
    let x = (dialogo_x_px + 15 * n) as f32 / ppp;
    let m = marcas_de(formas);
    let aros = aros(&m);
    let puntos = puntos(&m);
    let cajas = casillas(&m, u);

    // Lo que hay que ver: grupos (ids, opciones, elegida) y casillas (marcada), en orden.
    let mut grupos: Vec<(&str, usize, usize)> = Vec::new();
    let mut checks: Vec<bool> = Vec::new();
    for f in &dialog.spec().fields {
        if !dialog.is_shown(f.id) {
            continue;
        }
        match &f.kind {
            px_ui::dialog::FieldKind::Choice { options, .. } => {
                grupos.push((f.id, options.len(), dialog.chosen(f.id).expect("grupo")));
            }
            px_ui::dialog::FieldKind::Check { .. } => {
                checks.push(dialog.checked(f.id).expect("casilla"));
            }
            _ => {}
        }
    }

    // Opciones: un aro por opción, en su sitio y con las medidas de D5.
    let total: usize = grupos.iter().map(|g| g.1).sum();
    assert_eq!(
        aros.len(),
        total,
        "{donde}: un aro por opción de los grupos {grupos:?}; círculos pintados {:?}",
        m.discos
    );
    let mut primero = 0;
    for &(id, opciones, elegida) in &grupos {
        for i in 0..opciones {
            let aro = aros[primero + i];
            assert!(
                cerca_u(aro.centro.x, x, u),
                "{donde}: el aro {id}/{i} está en x = {} y debía estar en {x}",
                aro.centro.x
            );
            assert!(
                cerca_u(2.0 * aro.radio + aro.trazo_ancho, 10.0 * u, u),
                "{donde}: el aro {id}/{i} mide {} de diámetro exterior y debía medir {}",
                2.0 * aro.radio + aro.trazo_ancho,
                10.0 * u
            );
            assert!(
                cerca_u(aro.trazo_ancho, u, u),
                "{donde}: el trazo del aro {id}/{i} mide {} y debía medir {u}",
                aro.trazo_ancho
            );
            assert_eq!(
                relleno_de_aro(&m, &aro),
                FIELD_FILL,
                "{donde}: el aro {id}/{i} se rellena con el color del campo"
            );
            let aqui: Vec<&Disco> = puntos
                .iter()
                .filter(|p| mismo_centro(p.centro, aro.centro, u))
                .collect();
            assert_eq!(
                aqui.len(),
                usize::from(i == elegida),
                "{donde}: la opción {id}/{i} (elegida: {elegida}) lleva {} puntos",
                aqui.len()
            );
            if let Some(punto) = aqui.first() {
                assert!(
                    cerca_u(2.0 * punto.radio, 6.0 * u, u),
                    "{donde}: el punto de {id} mide {} y debía medir {}",
                    2.0 * punto.radio,
                    6.0 * u
                );
                assert_eq!(punto.trazo_ancho, 0.0, "{donde}: el punto no lleva trazo");
            }
        }
        primero += opciones;
    }
    assert_eq!(
        puntos.len(),
        grupos.len(),
        "{donde}: exactamente un punto por grupo ({grupos:?}): {puntos:?}"
    );

    // Casillas: una caja de 10 u por casilla, con su marca si está marcada.
    assert_eq!(
        cajas.len(),
        checks.len(),
        "{donde}: una caja con borde TOGGLE_EDGE por casilla; rectángulos de caja {:?}",
        m.cajas
            .iter()
            .filter(|c| c.rect.width() <= 20.0 * u)
            .collect::<Vec<_>>()
    );
    for (caja, marcada) in cajas.iter().zip(&checks) {
        let centro = caja.rect.center();
        assert!(
            cerca_u(caja.rect.width(), 10.0 * u, u) && cerca_u(caja.rect.height(), 10.0 * u, u),
            "{donde}: la caja mide {:?} y debía ser un cuadrado de {}",
            caja.rect.size(),
            10.0 * u
        );
        assert!(
            cerca_u(centro.x, x, u),
            "{donde}: la caja está centrada en x = {}, no en {x}",
            centro.x
        );
        assert!(
            cerca_u(caja.trazo_ancho, u, u),
            "{donde}: el borde de la caja mide {} y debía medir {u}",
            caja.trazo_ancho
        );
        assert_eq!(caja.relleno, FIELD_FILL, "{donde}: relleno de la caja");
        let marcas = marcas_cuadradas(&m, u, centro.y);
        assert_eq!(
            marcas.len(),
            usize::from(*marcada),
            "{donde}: casilla marcada = {marcada}; marcas {marcas:?}"
        );
        if let Some(marca) = marcas.first() {
            assert!(
                cerca_u(marca.rect.width(), 6.0 * u, u) && cerca_u(marca.rect.height(), 6.0 * u, u),
                "{donde}: la marca mide {:?} y debía ser un cuadrado de {}",
                marca.rect.size(),
                6.0 * u
            );
            assert!(
                mismo_centro(marca.rect.center(), centro, u),
                "{donde}: la marca está centrada en la caja"
            );
        }
        // Opción y casilla se distinguen por la forma: ningún círculo en la fila de la caja.
        assert!(
            m.discos
                .iter()
                .all(|d| (d.centro.y - centro.y).abs() > 3.0 * u),
            "{donde}: hay un círculo en la fila de una casilla: {:?}",
            m.discos
        );
    }
    for aro in &aros {
        for caja in &cajas {
            assert!(
                (aro.centro.y - caja.rect.center().y).abs() > 3.0 * u,
                "{donde}: un aro y una caja comparten fila"
            );
        }
    }

    // Contraste de lo pintado frente al campo y al diálogo.
    let mut colores: Vec<(&str, Color32)> = Vec::new();
    colores.extend(aros.iter().map(|d| ("aro", d.trazo_color)));
    colores.extend(puntos.iter().map(|d| ("punto", d.relleno)));
    colores.extend(cajas.iter().map(|c| ("borde de casilla", c.trazo_color)));
    for caja in &cajas {
        colores.extend(
            marcas_cuadradas(&m, u, caja.rect.center().y)
                .iter()
                .map(|c| ("marca de casilla", c.relleno)),
        );
    }
    for (nombre, color) in colores {
        for fondo in [FIELD_FILL, DIALOG_FILL] {
            let ratio = contrast_ratio(color, fondo);
            assert!(
                ratio >= MIN_MARCA_20B,
                "{donde}: {nombre} {color:?} sobre {fondo:?} da {ratio:.2} (< {MIN_MARCA_20B})"
            );
        }
    }
    m
}

/// El anillo de foco de la fila de `rotulo` con el foco en `objetivo`: tras comprobar que es el de
/// siempre (un trazo `ACCENT` de 2 u que rodea la fila entera, con el marcador dentro y con 3:1
/// sobre el campo y el diálogo), devuelve sus medidas en u: `x` desde `PAD_U`, `y` desde el rótulo,
/// alto, ancho sin las letras y trazo.
fn afirma_anillo(
    texto: &Pintado,
    formas: &[ClippedShape],
    ppp: f32,
    dialogo_x_px: i64,
    objetivo: FocusTarget,
    rotulo: &str,
    donde: &str,
) -> [f32; 5] {
    let n = i64::from(text_scale(ppp));
    let u = n as f32 / ppp;
    let m = marcas_de(formas);
    let anillos: Vec<Caja> = m
        .cajas
        .iter()
        .copied()
        .filter(|c| {
            c.trazo_color == ACENTO
                && cerca_u(c.trazo_ancho, 2.0 * u, u)
                && c.relleno == Color32::TRANSPARENT
        })
        .collect();
    assert_eq!(
        anillos.len(),
        1,
        "{donde}: el foco en {objetivo:?} pinta {} anillos ACCENT de 2 u: {anillos:?}",
        anillos.len()
    );
    let anillo = anillos[0];
    let etiqueta = texto
        .exacto(rotulo)
        .unwrap_or_else(|| panic!("{donde}: no se pinta {rotulo:?}; hay {:?}", texto.lista()));
    let letras = rotulo.chars().count() as f32;
    let x0 = (dialogo_x_px + 8 * n) as f32 / ppp;
    assert!(
        cerca_u(anillo.rect.min.x, x0, u)
            && cerca_u(anillo.rect.min.y, etiqueta.pos.y - u, u)
            && cerca_u(anillo.rect.height(), 16.0 * u, u)
            && cerca_u(anillo.rect.width(), (18.0 + 7.0 * letras) * u, u),
        "{donde}: el anillo de {objetivo:?} es {:?} y debía rodear la fila entera de {rotulo:?}",
        anillo.rect
    );
    for fondo in [FIELD_FILL, DIALOG_FILL] {
        assert!(
            contrast_ratio(ACENTO, fondo) >= MIN_MARCA_20B,
            "{donde}: anillo sobre {fondo:?}"
        );
    }
    let marcador = Rect::from_center_size(
        pos2(
            (dialogo_x_px + 15 * n) as f32 / ppp,
            etiqueta.pos.y + 7.0 * u,
        ),
        Vec2::splat(10.0 * u),
    );
    assert!(
        anillo.rect.contains_rect(marcador.shrink(0.01)),
        "{donde}: el marcador {marcador:?} queda fuera del anillo {:?}",
        anillo.rect
    );
    [
        (anillo.rect.min.x - x0) / u,
        (anillo.rect.min.y - etiqueta.pos.y) / u,
        anillo.rect.height() / u,
        anillo.rect.width() / u - 7.0 * letras,
        anillo.trazo_ancho / u,
    ]
}

/// Medidas de anillo esperadas: `x` en `PAD_U` (0 desde `PAD_U`), `y` a 1 u sobre el rótulo, 16 u de
/// alto, 18 u de ancho más 7 u por letra y 2 u de trazo.
fn afirma_medidas_del_anillo(medidas: &[[f32; 5]], donde: &str) {
    for m in medidas {
        assert!(
            m[0].abs() <= 0.05
                && (m[1] + 1.0).abs() <= 0.05
                && (m[2] - 16.0).abs() <= 0.05
                && (m[3] - 18.0).abs() <= 0.05
                && (m[4] - 2.0).abs() <= 0.05,
            "{donde}: {m:?}"
        );
    }
    for m in &medidas[1..] {
        for (a, b) in m.iter().zip(&medidas[0]) {
            assert!(
                (a - b).abs() <= 0.05,
                "{donde}: el anillo no es el mismo: {medidas:?}"
            );
        }
    }
}

impl Banco {
    /// Un fotograma más de reposo sin eventos, solo con el diálogo y su `sync`, y las formas crudas.
    fn crudas(&mut self) -> Vec<ClippedShape> {
        let size = pantalla(self.ppp, self.ventana);
        let Banco {
            ctx,
            modal,
            focus,
            convert,
            cuenta,
            image,
            ..
        } = &mut *self;
        ejecuta(ctx, size, vec![], |ui| {
            let c = ui.ctx().clone();
            apply_scale(&c);
            if let Modal::Open(dialog) = modal {
                let _ = show_dialog(&c, dialog, focus);
                if let Some(sync) = convert.as_mut() {
                    let historial = History {
                        limit: 64 << 20,
                        used: 0,
                    };
                    sync_convert(dialog, sync, image, 0, historial, cuenta);
                }
            }
        })
    }

    /// Reposo y las comprobaciones de marcadores sobre lo pintado.
    fn marcadores(&mut self, donde: &str) -> Marcas {
        self.reposo();
        let formas = self.crudas();
        let x = self.layout().dialog.x;
        afirma_marcadores(self.dialogo(), &formas, self.ppp, x, donde)
    }

    /// La casilla que se ve, si hay.
    fn casilla_visible(&self) -> Option<&'static str> {
        let d = self.dialogo();
        d.spec()
            .fields
            .iter()
            .find(|f| matches!(f.kind, px_ui::dialog::FieldKind::Check { .. }) && d.is_shown(f.id))
            .map(|f| f.id)
    }
}

#[test]
fn fila_9_convertir_modo_pinta_aros_y_casillas_cuadradas_en_cada_destino_y_con_cada_clic() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        let destinos = destinos(Mode::Rgba);
        b.marcadores(&format!("ppp {ppp} al abrir"));
        for (k, destino) in destinos.into_iter().enumerate() {
            // Clic real en el aro del destino (el primer grupo, de arriba abajo).
            let formas = b.crudas();
            let centro = aros(&marcas_de(&formas))[k].centro;
            b.clic(centro);
            b.reposo();
            assert_eq!(
                b.dialogo().chosen(FIELD_TARGET),
                Some(k),
                "ppp {ppp}: clic en el aro del destino {k}"
            );
            let donde = format!("ppp {ppp} destino {destino:?}");
            b.marcadores(&donde);

            // La casilla, si la hay, con clic real en su caja: marcada, marca cuadrada, y desmarcada.
            if let Some(casilla) = b.casilla_visible() {
                let u = b.u();
                for esperado in [true, false] {
                    let formas = b.crudas();
                    let caja = casillas(&marcas_de(&formas), u)[0];
                    b.clic(caja.rect.center());
                    b.reposo();
                    assert_eq!(
                        b.dialogo().checked(casilla),
                        Some(esperado),
                        "{donde}: clic en la caja de {casilla}"
                    );
                    b.marcadores(&format!("{donde} casilla {esperado}"));
                }
            }

            // El alfa (con destino Gris): un clic real en el aro de cada opción.
            if destino == Mode::Gray {
                for j in [4usize, 1, 0, 3, 2] {
                    let formas = b.crudas();
                    let centro = aros(&marcas_de(&formas))[3 + j].centro;
                    b.clic(centro);
                    b.reposo();
                    assert_eq!(
                        b.dialogo().chosen(FIELD_ALPHA),
                        Some(j),
                        "{donde}: clic en el aro {j} del alfa"
                    );
                    b.marcadores(&format!("{donde} alfa {j}"));
                }
            }
        }
    }
}

#[test]
fn fila_11_convertir_modo_el_anillo_de_foco_es_el_mismo_en_opcion_y_en_casilla() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        b.elige_destino(Mode::Gray);
        let donde = format!("ppp {ppp}");
        let objetivos = [
            (FocusTarget::Option(FIELD_TARGET, 0), "Indexado"),
            (FocusTarget::Option(FIELD_TARGET, 1), "Gris con alfa"),
            (FocusTarget::Option(FIELD_TARGET, 2), "Gris"),
            (FocusTarget::Field(FIELD_LUMA_GRAY), CASILLA),
            (FocusTarget::Option(FIELD_ALPHA, 0), OPCIONES_DEL_ALFA[0]),
            (FocusTarget::Option(FIELD_ALPHA, 4), OPCIONES_DEL_ALFA[4]),
        ];
        let mut medidas = Vec::new();
        for (objetivo, rotulo) in objetivos {
            b.foco_hasta(objetivo);
            let formas = b.crudas();
            let x = b.layout().dialog.x;
            medidas.push(afirma_anillo(
                &b.ultimo, &formas, ppp, x, objetivo, rotulo, &donde,
            ));
        }
        afirma_medidas_del_anillo(&medidas, &donde);
        // Enfocar no cambia el marcador.
        b.marcadores(&donde);
    }
}

#[test]
fn fila_11_convertir_modo_tab_y_espacio_eligen_y_alternan_como_hoy_con_el_marcador_nuevo() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, Mode::Rgba);
        for k in [1usize, 0, 2] {
            b.foco_hasta(FocusTarget::Option(FIELD_TARGET, k));
            b.pulsa(Key::Space);
            assert_eq!(
                b.dialogo().chosen(FIELD_TARGET),
                Some(k),
                "ppp {ppp}: Espacio en el destino {k}"
            );
            b.marcadores(&format!("ppp {ppp} Espacio en el destino {k}"));
        }
        // Con destino Gris: la casilla y una opción del alfa.
        for esperado in [true, false] {
            b.foco_hasta(FocusTarget::Field(FIELD_LUMA_GRAY));
            b.pulsa(Key::Space);
            assert_eq!(
                b.dialogo().checked(FIELD_LUMA_GRAY),
                Some(esperado),
                "ppp {ppp}: Espacio en la casilla"
            );
            b.marcadores(&format!("ppp {ppp} Espacio en la casilla {esperado}"));
        }
        b.foco_hasta(FocusTarget::Option(FIELD_ALPHA, 3));
        b.pulsa(Key::Space);
        assert_eq!(b.dialogo().chosen(FIELD_ALPHA), Some(3), "ppp {ppp}");
        b.marcadores(&format!("ppp {ppp} Espacio en el alfa 3"));
        assert!(
            b.aceptados.is_empty() && b.cancelados == 0,
            "{}",
            b.describe()
        );
    }
}
