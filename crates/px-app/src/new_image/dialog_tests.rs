//! Tests de `new_image` con el diálogo (specs/hito-2-nueva-imagen.md, unidad 19): la fila 2 en su
//! parte de diálogo (`open_new_image` y `Dialog::set_limit_note`) y la fila 11, el banco de egui del
//! formulario real. En ROJO hasta la implementación.
//!
//! El banco reproduce el orden de fotograma de `App::ui` (como `px-ui/tests/dialog_open_focus.rs`):
//! primero el menú (`show_menu`), luego las acciones del fotograma (`NewImage` abre el `Modal` con
//! `open_new_image` y los `Locks` del fotograma) y, el último, `show_dialog` seguido de `sync_form`
//! (D8). Sin ventana ni capturas: `Context::run_ui` con `RawInput` sintético. El diálogo se abre con
//! un clic real en `Archivo → Nueva imagen…`.
//!
//! - Fila 2 (diálogo): máximo, rango visible, motivo en línea y del pie con la nota de la tarjeta
//!   gráfica, `Crear` atenuado con el motivo visible.
//! - Fila 11: foco inicial en el ancho desde el fotograma siguiente al del clic; teclear `0`, `8193`,
//!   vacío y `500` (y `5000` con `max_side` 4096); `−` y `+`; cambiar de modo y elegir «Color…»
//!   (`#12`, `zz`, `#ff0000`, `#80808080`); cambiar de modo con un fondo que no cabe (aviso en el
//!   informe hasta el siguiente cambio); `Intro`, `Crear`, `Cancelar`, `Escape`; `Tab`; el desplazamiento
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

fn maximo_esperado(max_side: usize) -> u32 {
    u32::try_from(max_side.clamp(1, 8192)).expect("cabe")
}

fn abierto(max_side: usize) -> Modal {
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, max_side, Locks::default()));
    modal
}

fn dialogo_de(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn dialogo_de_mut(modal: &mut Modal) -> &mut Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn numero(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Number {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn motivo_de(d: &Dialog, campo: &'static str) -> String {
    match d.validity(campo) {
        Some(Validity::Invalid(motivo)) => motivo,
        otro => panic!("{campo} debía ser inválido y es {otro:?}"),
    }
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

#[test]
fn fila_2_open_new_image_abre_el_dialogo_de_nueva_imagen() {
    let modal = abierto(16384);
    assert_eq!(modal.kind(), Some(DialogKind::NewImage));
    let d = dialogo_de(&modal);
    assert_eq!(d.spec().title, "Nueva imagen");
    assert!(d.is_valid());
}

#[test]
fn fila_2_open_new_image_devuelve_false_con_un_bloqueo_o_con_otro_dialogo_abierto() {
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
    let mut modal = abierto(4096);
    let antes = modal.clone();
    assert!(!open_new_image(&mut modal, 100, Locks::default()));
    assert_eq!(modal, antes, "no cambia el diálogo abierto");
}

#[test]
fn fila_2_con_la_gpu_por_debajo_de_8192_el_maximo_y_el_motivo_dicen_tarjeta_grafica() {
    for max_side in [8191usize, 4096, 2048, 100, 1] {
        let max = maximo_esperado(max_side);
        for (campo, etiqueta) in [("ancho", "Ancho"), ("alto", "Alto")] {
            let mut modal = abierto(max_side);
            let d = dialogo_de_mut(&mut modal);
            // Máximo del campo.
            let FieldKind::Number { max: tope, .. } = d
                .spec()
                .fields
                .iter()
                .find(|f| f.id == campo)
                .expect("campo")
                .kind
            else {
                panic!("{campo} no es numérico");
            };
            assert_eq!(tope, max, "max_side {max_side} {campo}");

            // `max` vale y `max + 1` da el texto con la nota.
            numero(d, campo, &max.to_string());
            assert_eq!(
                d.validity(campo),
                Some(Validity::Valid),
                "{max_side} {campo}"
            );
            assert!(d.is_valid());
            numero(d, campo, &(max + 1).to_string());
            let motivo = format!("máximo {max} (tarjeta gráfica)");
            assert_eq!(motivo_de(d, campo), motivo, "{max_side} {campo}");
            // El motivo del pie: `Ancho: máximo 4096 (tarjeta gráfica)` (36 caracteres con 4 cifras).
            let pie = format!("{etiqueta}: {motivo}");
            assert_eq!(d.reason(), Some(pie.clone()), "{max_side} {campo}");
            assert!(pie.chars().count() <= 37, "{pie}");
            if (1000..10000).contains(&max) {
                // 36 caracteres con «Ancho:» y 35 con «Alto:» (la nota y `máximo NNNN` miden 29).
                assert_eq!(
                    pie.chars().count(),
                    etiqueta.chars().count() + 2 + 29,
                    "{pie}"
                );
            }
            // Cabe en la línea de 44 columnas con `error: ` y está dentro del juego.
            let linea = format!("{ERROR_PREFIX}{motivo}");
            assert!(linea.chars().count() <= 44, "{linea}");
            assert!(en_el_juego(&linea), "{linea}");
            assert!(en_el_juego(&pie), "{pie}");
            assert!(!d.is_valid(), "Crear queda atenuado");
        }
    }
}

#[test]
fn fila_2_cuatro_mil_noventa_y_seis_dice_exactamente_error_maximo_4096_tarjeta_grafica() {
    let mut modal = abierto(4096);
    let d = dialogo_de_mut(&mut modal);
    numero(d, "ancho", "4097");
    assert_eq!(
        format!("{ERROR_PREFIX}{}", motivo_de(d, "ancho")),
        "error: máximo 4096 (tarjeta gráfica)"
    );
    assert_eq!(
        d.reason(),
        Some("Ancho: máximo 4096 (tarjeta gráfica)".to_string())
    );
    assert_eq!("Ancho: máximo 4096 (tarjeta gráfica)".chars().count(), 36);
}

#[test]
fn fila_2_los_valores_5000_cero_y_once_cifras_con_la_gpu_limitada() {
    for max_side in [8191usize, 4096, 2048, 100, 1] {
        let max = maximo_esperado(max_side);
        let mut modal = abierto(max_side);
        let d = dialogo_de_mut(&mut modal);
        numero(d, "ancho", "5000");
        if max >= 5000 {
            assert_eq!(d.validity("ancho"), Some(Validity::Valid), "{max_side}");
        } else {
            assert_eq!(
                motivo_de(d, "ancho"),
                format!("máximo {max} (tarjeta gráfica)"),
                "{max_side}"
            );
        }
        // `0` y las once cifras dicen lo de siempre, sin nota.
        numero(d, "ancho", "0");
        assert_eq!(motivo_de(d, "ancho"), "mínimo 1", "{max_side}");
        numero(d, "ancho", "99999999999");
        assert_eq!(motivo_de(d, "ancho"), "máximo 10 cifras", "{max_side}");
        numero(d, "ancho", "abc");
        assert_eq!(motivo_de(d, "ancho"), "solo dígitos 0-9", "{max_side}");
        numero(d, "ancho", "");
        assert_eq!(motivo_de(d, "ancho"), "escribe un número", "{max_side}");
    }
}

#[test]
fn fila_2_con_lado_de_textura_de_8192_o_mas_el_texto_es_el_de_siempre() {
    for max_side in [8192usize, 16384, 32768] {
        for campo in ["ancho", "alto"] {
            let mut modal = abierto(max_side);
            let d = dialogo_de_mut(&mut modal);
            numero(d, campo, "8193");
            assert_eq!(motivo_de(d, campo), "máximo 8192", "{max_side} {campo}");
            numero(d, campo, "8192");
            assert_eq!(d.validity(campo), Some(Validity::Valid));
        }
    }
}

#[test]
fn fila_2_set_limit_note_sobre_el_formulario_real_solo_vale_en_los_numericos() {
    let mut modal = abierto(16384);
    let d = dialogo_de_mut(&mut modal);
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
    numero(d, "ancho", "8193");
    assert_eq!(motivo_de(d, "ancho"), "máximo 8192 (tarjeta gráfica)");
    assert!(d.set_limit_note("ancho", None));
    assert_eq!(motivo_de(d, "ancho"), "máximo 8192");
}

// ---------------------------------------------------------------------------------------------
// Fila 11: ayudantes del banco
// ---------------------------------------------------------------------------------------------

/// Las cinco escalas de pantalla de la spec.
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

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

    /// Los textos visibles que, recortados, son `-` o `−`, de arriba abajo.
    fn menos(&self) -> Vec<Rect> {
        let mut v: Vec<Rect> = self
            .visibles()
            .filter(|t| matches!(t.texto.trim(), "−" | "-"))
            .map(Texto::rect)
            .collect();
        v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        v
    }

    fn mas(&self) -> Vec<Rect> {
        let mut v: Vec<Rect> = self
            .visibles()
            .filter(|t| t.texto.trim() == "+")
            .map(Texto::rect)
            .collect();
        v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        v
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

struct Banco {
    ctx: Context,
    ppp: f32,
    ventana: Ventana,
    max_side: usize,
    modal: Modal,
    focus: Option<FocusTarget>,
    sync: FormSync,
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
    fn nuevo(ppp: f32, ventana: Ventana, max_side: usize) -> Banco {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(eframe::egui::Theme::Dark, visuals(ppp));
        Banco {
            ctx,
            ppp,
            ventana,
            max_side,
            modal: Modal::Closed,
            focus: None,
            sync: FormSync::default(),
            aceptados: Vec::new(),
            cancelados: 0,
            traza: Vec::new(),
            abierto_en: None,
            ultimo: Pintado::default(),
        }
    }

    /// Con el diálogo abierto por un clic en `Archivo → Nueva imagen…` y unos fotogramas de reposo.
    fn abierto(ppp: f32, ventana: Ventana, max_side: usize) -> Banco {
        let mut b = Banco::nuevo(ppp, ventana, max_side);
        b.abre_con_el_raton();
        assert!(b.modal.is_open(), "ppp {ppp} {ventana:?}: no se abrió");
        b
    }

    fn dialogo(&self) -> &Dialog {
        dialogo_de(&self.modal)
    }

    fn dialogo_mut(&mut self) -> &mut Dialog {
        dialogo_de_mut(&mut self.modal)
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
                sync,
                max_side,
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
                    if action == AppAction::NewImage && open_new_image(modal, *max_side, locks) {
                        *focus = None;
                        *sync = FormSync::default();
                    }
                }
                // El diálogo, el último, y `sync_form` tras él (D8).
                if let Modal::Open(dialog) = modal {
                    pinto_el_dialogo = true;
                    reaccion = show_dialog(&c, dialog, focus);
                    sync_form(dialog, sync);
                }
            })
        };
        match reaccion {
            Reaction::Accepted(values) => {
                self.aceptados.push(values);
                self.modal.abandon();
                self.focus = None;
            }
            Reaction::Cancelled => {
                self.cancelados += 1;
                self.modal.abandon();
                self.focus = None;
            }
            Reaction::Rejected(_) | Reaction::None | Reaction::Pressed { .. } => {}
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

    fn pulsa(&mut self, key: Key) {
        self.fotograma(vec![tecla(key, Modifiers::NONE)]);
        self.reposo();
    }

    /// El centro del texto visible `buscado`: el exacto y, si no hay, el primero que lo contiene
    /// (como `dialog_open_focus.rs`, que busca las entradas del menú por contenido).
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

    /// `Archivo → Nueva imagen…` con el ratón, como lo hace el usuario.
    fn abre_con_el_raton(&mut self) {
        self.reposo();
        self.clic_en("Archivo");
        assert!(
            self.ultimo.contiene("Ctrl+N"),
            "el menú muestra el atajo; hay {:?}",
            self.ultimo.lista()
        );
        let entrada = self.centro_de("Nueva imagen…");
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

    /// El `Id` de egui del campo `id` (el que se enfoca al darle el foco).
    fn id_de(campo: &'static str) -> Id {
        Id::unique(("pixoko-dialog", campo))
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

    fn crear_deshabilitado(&self) -> bool {
        self.ultimo
            .exacto("Crear")
            .unwrap_or_else(|| panic!("no se pinta Crear; hay {:?}", self.ultimo.lista()))
            .deshabilitado()
    }

    fn describe(&self) -> String {
        format!(
            "ppp {} {:?}; aceptados {}; cancelados {}; foco {:?}; pintado {:?}",
            self.ppp,
            self.ventana,
            self.aceptados.len(),
            self.cancelados,
            self.focus,
            self.ultimo.lista()
        )
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

fn texto_del_informe(d: &Dialog) -> String {
    d.spec()
        .fields
        .iter()
        .find_map(|f| match &f.kind {
            FieldKind::ReadOnly { text, .. } if f.id == "informe" => Some(text.clone()),
            _ => None,
        })
        .expect("el formulario tiene el informe")
}

fn spec_del(b: &Banco) -> Option<NewImageSpec> {
    spec_from_values(&b.dialogo().values()?)
}

const AVISOS: [&str; 5] = [
    "Gris sin Transparente: pasa a Blanco",
    "Indexado sin Color: pasa a Transparente",
    "Color con alfa: pasa a Blanco",
    "Color no gris: pasa a Transparente",
    "Color no gris: pasa a Blanco",
];

fn hay_aviso_de_fondo(b: &Banco) -> bool {
    AVISOS.iter().any(|a| b.ultimo.contiene(a))
}

// ---------------------------------------------------------------------------------------------
// Fila 11: abrir
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_abierto_con_un_clic_en_el_menu_el_foco_inicial_es_el_ancho_desde_el_fotograma_siguiente()
{
    for ppp in PPPS {
        for ventana in VENTANAS {
            let b = Banco::abierto(ppp, ventana, 16384);
            assert_eq!(b.modal.kind(), Some(DialogKind::NewImage));
            assert!(
                b.abierto_en.is_some(),
                "ppp {ppp} {ventana:?}: nunca se pintó"
            );
            let foco = foco_tras_la_apertura(&b);
            assert!(
                foco.len() >= 3,
                "ppp {ppp} {ventana:?}: faltan fotogramas tras abrir"
            );
            for (fotograma, id) in foco {
                assert_eq!(
                    id,
                    Some(Banco::id_de("ancho")),
                    "ppp {ppp} {ventana:?}: el foco de egui en el fotograma {fotograma} \
                     (se abrió en {:?}) no es el ancho; {}",
                    b.abierto_en,
                    b.describe()
                );
            }
            assert_eq!(
                b.focus,
                Some(FocusTarget::Field("ancho")),
                "ppp {ppp} {ventana:?}"
            );
            assert_eq!(b.dialogo().text("ancho"), Some("64".to_string()));
            assert_eq!(b.dialogo().text("alto"), Some("64".to_string()));
        }
    }
}

#[test]
fn fila_11_el_dialogo_pinta_titulo_pie_y_botones_a_las_dos_ventanas() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let b = Banco::abierto(ppp, ventana, 16384);
            for texto in ["Nueva imagen", "Crear", "Cancelar", "Ancho", "Alto", "Modo"] {
                assert!(
                    b.ultimo.exacto(texto).is_some(),
                    "ppp {ppp} {ventana:?}: falta {texto:?}; hay {:?}",
                    b.ultimo.lista()
                );
            }
            assert!(
                !b.crear_deshabilitado(),
                "ppp {ppp} {ventana:?}: todo es válido"
            );
        }
    }
}

#[test]
fn fila_11_en_960_por_720_se_pinta_todo_el_formulario_de_indexado_con_su_informe_y_el_rango() {
    for ppp in PPPS {
        let b = Banco::abierto(ppp, Ventana::Grande, 16384);
        for texto in [
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
                b.ultimo.exacto(texto).is_some(),
                "ppp {ppp}: falta {texto:?}; hay {:?}",
                b.ultimo.lista()
            );
        }
        // Indexado no ofrece «Color…» ni el hexadecimal.
        assert!(b.ultimo.exacto("Color…").is_none(), "ppp {ppp}");
        assert!(b.ultimo.exacto("Color").is_none(), "ppp {ppp}");
        // El informe y el rango visible.
        assert!(b.ultimo.contiene("Memoria: menos de 1 MiB"), "ppp {ppp}");
        assert!(
            b.ultimo.contiene("Paleta: 32 colores, el 0 transparente"),
            "ppp {ppp}"
        );
        assert!(b.ultimo.contiene("1-8192"), "ppp {ppp}: el rango visible");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11 (y fila 2): teclear en el ancho
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_teclear_0_8193_vacio_y_500_en_el_ancho_da_errores_en_linea_y_crear_atenuado() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for (texto, motivo) in [
                ("0", Some("mínimo 1")),
                ("8193", Some("máximo 8192")),
                ("", Some("escribe un número")),
                ("500", None),
            ] {
                let mut b = Banco::abierto(ppp, ventana, 16384);
                b.escribe(texto);
                let contexto = format!("ppp {ppp} {ventana:?} {texto:?}");
                match motivo {
                    Some(motivo) => {
                        assert_eq!(
                            b.dialogo().validity("ancho"),
                            Some(Validity::Invalid(motivo.to_string())),
                            "{contexto}"
                        );
                        // En línea: `error: ` y el motivo.
                        assert!(
                            b.ultimo.contiene(&format!("{ERROR_PREFIX}{motivo}")),
                            "{contexto}: falta el error en línea; hay {:?}",
                            b.ultimo.lista()
                        );
                        // En el pie: `error: ` y `Ancho: motivo`, con Crear atenuado.
                        assert!(
                            b.ultimo.contiene(&format!("{ERROR_PREFIX}Ancho: {motivo}")),
                            "{contexto}: falta el motivo del pie; hay {:?}",
                            b.ultimo.lista()
                        );
                        assert!(b.crear_deshabilitado(), "{contexto}: Crear atenuado");
                    }
                    None => {
                        assert_eq!(b.dialogo().validity("ancho"), Some(Validity::Valid));
                        assert!(
                            !b.ultimo.contiene(ERROR_PREFIX),
                            "{contexto}: no hay errores; hay {:?}",
                            b.ultimo.lista()
                        );
                        assert!(!b.crear_deshabilitado(), "{contexto}");
                    }
                }
            }
        }
    }
}

#[test]
fn fila_11_teclear_en_el_alto_da_el_motivo_del_alto() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        b.foco_hasta(FocusTarget::Field("alto"));
        b.escribe("8193");
        assert!(b.ultimo.contiene("error: máximo 8192"), "ppp {ppp}");
        assert!(b.ultimo.contiene("error: Alto: máximo 8192"), "ppp {ppp}");
        assert!(b.crear_deshabilitado(), "ppp {ppp}");
    }
}

#[test]
fn fila_2_con_la_gpu_en_4096_el_rango_y_el_error_dicen_tarjeta_grafica_en_el_banco() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 4096);
            let contexto = format!("ppp {ppp} {ventana:?}");
            assert!(
                b.ultimo.contiene("1-4096"),
                "{contexto}: el rango visible; hay {:?}",
                b.ultimo.lista()
            );
            b.escribe("5000");
            assert!(
                b.ultimo.contiene("error: máximo 4096 (tarjeta gráfica)"),
                "{contexto}: error en línea; hay {:?}",
                b.ultimo.lista()
            );
            // El pie: `error: Ancho: máximo 4096 (tarjeta gráfica)` (43 caracteres, caben en 44).
            assert!(
                b.ultimo
                    .contiene("error: Ancho: máximo 4096 (tarjeta gráfica)"),
                "{contexto}: motivo del pie; hay {:?}",
                b.ultimo.lista()
            );
            assert!(
                b.crear_deshabilitado(),
                "{contexto}: Crear atenuado con el motivo visible"
            );

            b.escribe("4096");
            assert_eq!(
                b.dialogo().validity("ancho"),
                Some(Validity::Valid),
                "{contexto}"
            );
            assert!(!b.crear_deshabilitado(), "{contexto}");

            b.foco_hasta(FocusTarget::Field("alto"));
            b.escribe("5000");
            assert!(
                b.ultimo
                    .contiene("error: Alto: máximo 4096 (tarjeta gráfica)"),
                "{contexto}: el alto dice lo mismo; hay {:?}",
                b.ultimo.lista()
            );
        }
    }
}

#[test]
fn fila_11_menos_y_mas_cambian_el_ancho_de_uno_en_uno() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 16384);
            let contexto = format!("ppp {ppp} {ventana:?}");
            let mas = b.ultimo.mas();
            let menos = b.ultimo.menos();
            assert_eq!(mas.len(), 2, "{contexto}: un + por campo numérico");
            assert_eq!(menos.len(), 2, "{contexto}: un − por campo numérico");

            b.clic(mas[0].center());
            b.reposo();
            assert_eq!(
                b.dialogo().text("ancho"),
                Some("65".to_string()),
                "{contexto}"
            );
            assert_eq!(
                b.dialogo().text("alto"),
                Some("64".to_string()),
                "{contexto}"
            );
            for _ in 0..2 {
                let menos = b.ultimo.menos();
                b.clic(menos[0].center());
                b.reposo();
            }
            assert_eq!(
                b.dialogo().text("ancho"),
                Some("63".to_string()),
                "{contexto}"
            );
            let mas = b.ultimo.mas();
            b.clic(mas[1].center());
            b.reposo();
            assert_eq!(
                b.dialogo().text("alto"),
                Some("65".to_string()),
                "{contexto}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: modo, fondo y «Color…»
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_cambiar_de_modo_con_el_raton_muestra_solo_el_grupo_de_fondo_de_ese_modo() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        let contexto = |paso: &str| format!("ppp {ppp}: {paso}");

        // Indexado: Transparente, Blanco, Negro.
        assert!(
            b.dialogo().is_shown("fondo_indexado"),
            "{}",
            contexto("inicio")
        );
        assert!(b.ultimo.exacto("Transparente").is_some());
        assert!(b.ultimo.exacto("Color…").is_none());

        for (modo, fondo, con_transparente) in [
            ("RGBA", "fondo_rgba", true),
            ("Gris con alfa", "fondo_gris_alfa", true),
            ("Gris", "fondo_gris", false),
            ("Indexado", "fondo_indexado", true),
        ] {
            b.clic_en(modo);
            for otro in [
                "fondo_indexado",
                "fondo_rgba",
                "fondo_gris_alfa",
                "fondo_gris",
            ] {
                assert_eq!(
                    b.dialogo().is_shown(otro),
                    otro == fondo,
                    "{}: {otro}",
                    contexto(modo)
                );
            }
            assert!(b.ultimo.exacto("Blanco").is_some(), "{}", contexto(modo));
            assert!(b.ultimo.exacto("Negro").is_some(), "{}", contexto(modo));
            assert_eq!(
                b.ultimo.exacto("Transparente").is_some(),
                con_transparente,
                "{}: Gris no ofrece Transparente",
                contexto(modo)
            );
            let con_color = modo != "Indexado";
            assert_eq!(
                b.ultimo.exacto("Color…").is_some(),
                con_color,
                "{}: «Color…»",
                contexto(modo)
            );
            // Aún no se eligió «Color…»: ningún hexadecimal.
            for color in ["color_rgba", "color_gris_alfa", "color_gris"] {
                assert!(!b.dialogo().is_shown(color), "{}: {color}", contexto(modo));
            }
        }
    }
}

#[test]
fn fila_11_color_muestra_el_hexadecimal_solo_de_ese_modo_y_valida_el_texto() {
    for ppp in PPPS {
        for (modo, color, hex_largo_invalido) in [
            ("RGBA", "color_rgba", None),
            ("Gris con alfa", "color_gris_alfa", Some(HexError::NotGray)),
            ("Gris", "color_gris", Some(HexError::NotGray)),
        ] {
            let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
            let contexto = format!("ppp {ppp} {modo}");
            b.clic_en(modo);
            assert!(
                !b.dialogo().is_shown(color),
                "{contexto}: antes de «Color…»"
            );
            b.clic_en("Color…");
            for otro in ["color_rgba", "color_gris_alfa", "color_gris"] {
                assert_eq!(
                    b.dialogo().is_shown(otro),
                    otro == color,
                    "{contexto}: {otro}"
                );
            }
            assert!(
                b.ultimo.exacto("Color").is_some(),
                "{contexto}: rótulo del hexadecimal"
            );
            assert!(
                b.ultimo.contiene("#808080"),
                "{contexto}: nace en gris medio"
            );

            // `#12` y `zz`: demasiado cortos en cualquier modo.
            b.foco_hasta(FocusTarget::Field(color));
            let corto = hex_notice(&HexError::Length(2));
            for texto in ["#12", "zz"] {
                b.escribe(texto);
                assert_eq!(
                    b.dialogo().validity(color),
                    Some(Validity::Invalid(corto.to_string())),
                    "{contexto} {texto}"
                );
                assert!(
                    b.ultimo.contiene(&format!("{ERROR_PREFIX}{corto}")),
                    "{contexto} {texto}: falta el error; hay {:?}",
                    b.ultimo.lista()
                );
                assert!(b.crear_deshabilitado(), "{contexto} {texto}");
            }

            // `#ff0000`: vale en RGBA y no en los grises.
            b.escribe("#ff0000");
            match hex_largo_invalido {
                None => {
                    assert_eq!(
                        b.dialogo().validity(color),
                        Some(Validity::Valid),
                        "{contexto}"
                    );
                    assert!(!b.crear_deshabilitado(), "{contexto}");
                }
                Some(error) => {
                    assert!(
                        b.ultimo
                            .contiene(&format!("{ERROR_PREFIX}{}", hex_notice(&error))),
                        "{contexto}: #ff0000; hay {:?}",
                        b.ultimo.lista()
                    );
                    assert!(b.crear_deshabilitado(), "{contexto}");
                }
            }
            // `#80808080`: en Gris falta el alfa; en Gris con alfa y RGBA vale.
            b.escribe("#80808080");
            if modo == "Gris" {
                assert!(
                    b.ultimo
                        .contiene(&format!("{ERROR_PREFIX}{}", hex_notice(&HexError::NoAlpha))),
                    "{contexto}: #80808080; hay {:?}",
                    b.ultimo.lista()
                );
                assert!(b.crear_deshabilitado(), "{contexto}");
            } else {
                assert_eq!(
                    b.dialogo().validity(color),
                    Some(Validity::Valid),
                    "{contexto}"
                );
            }
        }
    }
}

#[test]
fn fila_11_el_hexadecimal_solo_se_ve_con_color_de_su_modo_al_cambiar_de_modo() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        b.clic_en("RGBA");
        b.clic_en("Color…");
        assert!(b.dialogo().is_shown("color_rgba"), "ppp {ppp}");
        assert!(
            b.ultimo.exacto("Color").is_some(),
            "ppp {ppp}: rótulo del hexadecimal"
        );

        // Un gris con alfa 255 viaja a Gris como su valor: «Color…» del grupo de Gris, con su
        // hexadecimal; el de RGBA se oculta.
        b.clic_en("Gris");
        assert!(
            !b.dialogo().is_shown("color_rgba"),
            "ppp {ppp}: el de RGBA se oculta"
        );
        assert!(
            b.dialogo().is_shown("color_gris"),
            "ppp {ppp}: el gris viaja como su valor"
        );
        assert!(!b.dialogo().is_shown("color_gris_alfa"), "ppp {ppp}");
        assert!(!hay_aviso_de_fondo(&b), "ppp {ppp}: viajó sin aviso");

        // Indexado no ofrece «Color…»: ningún hexadecimal y ningún rótulo «Color».
        b.clic_en("Indexado");
        for color in ["color_rgba", "color_gris_alfa", "color_gris"] {
            assert!(!b.dialogo().is_shown(color), "ppp {ppp}: {color}");
        }
        assert!(b.ultimo.exacto("Color").is_none(), "ppp {ppp}");
        assert!(b.ultimo.exacto("Color…").is_none(), "ppp {ppp}");
        assert!(
            b.ultimo.contiene("Indexado sin Color: pasa a Transparente"),
            "ppp {ppp}: dice qué fondo había y a cuál vuelve; hay {:?}",
            b.ultimo.lista()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el informe y el aviso del traslado del fondo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_el_informe_se_actualiza_tras_cada_cambio_de_tamano() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        let contexto = format!("ppp {ppp}");
        assert!(b.ultimo.contiene("Memoria: menos de 1 MiB"), "{contexto}");

        b.escribe("4097");
        b.foco_hasta(FocusTarget::Field("alto"));
        b.escribe("4096");
        for linea in report_text(Some((4097, 4096)), Mode::Indexed, None).lines() {
            assert!(
                b.ultimo.contiene(linea),
                "{contexto}: falta {linea:?}; hay {:?}",
                b.ultimo.lista()
            );
        }
        assert!(
            b.ultimo
                .contiene("aviso: más de 16777216 px; puede ir lento"),
            "{contexto}"
        );
        // El aviso de tamaño no impide crear.
        assert!(!b.crear_deshabilitado(), "{contexto}");

        b.escribe("64");
        assert!(
            !b.ultimo.contiene("aviso: más de"),
            "{contexto}: el aviso desaparece al bajar el tamaño; hay {:?}",
            b.ultimo.lista()
        );

        b.escribe("abc");
        assert!(b.ultimo.contiene("Memoria: —"), "{contexto}: sin valor");
        assert!(b.crear_deshabilitado(), "{contexto}");
    }
}

#[test]
fn fila_11_cambiar_de_modo_con_transparente_a_gris_avisa_en_el_informe_hasta_el_siguiente_cambio() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        let contexto = format!("ppp {ppp}");
        b.clic_en("RGBA");
        assert!(
            !hay_aviso_de_fondo(&b),
            "{contexto}: Transparente viaja a RGBA sin aviso"
        );
        b.clic_en("Gris");
        assert!(
            b.ultimo.contiene("Gris sin Transparente: pasa a Blanco"),
            "{contexto}: falta el aviso; hay {:?}",
            b.ultimo.lista()
        );
        let spec = spec_del(&b).expect("spec");
        assert_eq!(
            (spec.mode, spec.background),
            (Mode::Gray, Background::White),
            "{contexto}"
        );

        // El aviso dura hasta que cambia otro valor del formulario.
        b.reposo();
        assert!(
            b.ultimo.contiene("Gris sin Transparente: pasa a Blanco"),
            "{contexto}"
        );
        b.foco_hasta(FocusTarget::Field("ancho"));
        b.escribe("65");
        assert!(
            !hay_aviso_de_fondo(&b),
            "{contexto}: el aviso no sobrevive; hay {:?}",
            b.ultimo.lista()
        );
    }
}

#[test]
fn fila_11_un_color_que_no_cabe_en_el_modo_nuevo_vuelve_al_de_por_defecto_y_lo_dice() {
    for ppp in PPPS {
        for (destino, aviso, fondo_esperado, modo_esperado) in [
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
            let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
            let contexto = format!("ppp {ppp} → {destino}");
            b.clic_en("RGBA");
            b.clic_en("Color…");
            // Un rojo confirmado en el hexadecimal (la preparación no es lo que se prueba).
            {
                let d = b.dialogo_mut();
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
            b.reposo();
            assert!(!hay_aviso_de_fondo(&b), "{contexto}: aún sin aviso");

            b.clic_en(destino);
            assert!(
                b.ultimo.contiene(aviso),
                "{contexto}: falta {aviso:?}; hay {:?}",
                b.ultimo.lista()
            );
            let spec = spec_del(&b).expect("spec");
            assert_eq!(
                (spec.mode, spec.background),
                (modo_esperado, fondo_esperado),
                "{contexto}"
            );
            // Dice lo que había (Color) o lo que vuelve (Transparente/Blanco), sin pasar de 42.
            assert!(aviso.chars().count() <= 42);
        }
    }
}

#[test]
fn fila_11_blanco_y_negro_viajan_de_modo_sin_aviso() {
    for ppp in PPPS {
        for (fondo, fondo_esperado) in [("Blanco", Background::White), ("Negro", Background::Black)]
        {
            let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
            let contexto = format!("ppp {ppp} {fondo}");
            b.clic_en("RGBA");
            b.clic_en(fondo);
            for (destino, modo) in [
                ("Gris con alfa", Mode::GrayAlpha),
                ("Gris", Mode::Gray),
                ("Indexado", Mode::Indexed),
                ("RGBA", Mode::Rgba),
            ] {
                b.clic_en(destino);
                assert!(!hay_aviso_de_fondo(&b), "{contexto} → {destino}");
                let spec = spec_del(&b).expect("spec");
                assert_eq!(
                    (spec.mode, spec.background),
                    (modo, fondo_esperado),
                    "{contexto} → {destino}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Intro, Crear, Cancelar, Escape y Tab
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_intro_con_todo_valido_crea() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 16384);
            b.pulsa(Key::Enter);
            assert_eq!(b.aceptados.len(), 1, "{}", b.describe());
            assert!(!b.modal.is_open());
            let spec = spec_from_values(&b.aceptados[0]).expect("spec");
            assert_eq!(
                (spec.width, spec.height, spec.mode, spec.background),
                (64, 64, Mode::Indexed, Background::Transparent),
                "ppp {ppp} {ventana:?}"
            );
            assert_eq!(b.cancelados, 0);
        }
    }
}

#[test]
fn fila_11_intro_con_un_valor_invalido_no_crea_y_el_dialogo_sigue_abierto() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 16384);
            b.escribe("0");
            b.pulsa(Key::Enter);
            assert!(b.aceptados.is_empty(), "{}", b.describe());
            assert!(b.modal.is_open(), "ppp {ppp} {ventana:?}");
        }
    }
}

#[test]
fn fila_11_crear_acepta_y_atenuado_no_hace_nada() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 16384);
            b.clic_en("Crear");
            assert_eq!(b.aceptados.len(), 1, "{}", b.describe());
            assert!(!b.modal.is_open());

            let mut b = Banco::abierto(ppp, ventana, 16384);
            b.escribe("0");
            assert!(b.crear_deshabilitado());
            b.clic_en("Crear");
            assert!(
                b.aceptados.is_empty(),
                "ppp {ppp} {ventana:?}: Crear atenuado no crea"
            );
            assert!(b.modal.is_open());
        }
    }
}

#[test]
fn fila_11_cancelar_y_escape_cierran_sin_crear() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 16384);
            b.clic_en("Cancelar");
            assert_eq!(b.cancelados, 1, "{}", b.describe());
            assert!(b.aceptados.is_empty());
            assert!(!b.modal.is_open());

            let mut b = Banco::abierto(ppp, ventana, 16384);
            b.pulsa(Key::Escape);
            assert_eq!(b.cancelados, 1, "{}", b.describe());
            assert!(b.aceptados.is_empty());
            assert!(!b.modal.is_open());
        }
    }
}

fn comprueba_tab(b: &mut Banco, contexto: &str) {
    let orden = focus_order(b.dialogo());
    assert!(orden.contains(&FocusTarget::Accept), "{contexto}");
    assert!(orden.contains(&FocusTarget::Cancel), "{contexto}");
    // Solo lo visible.
    for &objetivo in &orden {
        match objetivo {
            FocusTarget::Field(id)
            | FocusTarget::Option(id, _)
            | FocusTarget::Minus(id)
            | FocusTarget::Plus(id)
            | FocusTarget::Zone(id)
            | FocusTarget::Button(id)
            | FocusTarget::ButtonAside(id) => assert!(
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
fn fila_11_tab_recorre_solo_lo_visible_y_da_la_vuelta_en_indexado() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            let mut b = Banco::abierto(ppp, ventana, 16384);
            let orden = focus_order(b.dialogo());
            assert_eq!(orden.first(), Some(&FocusTarget::Field("ancho")));
            for oculto in ["fondo_rgba", "fondo_gris_alfa", "fondo_gris"] {
                assert!(
                    !orden.contains(&FocusTarget::Option(oculto, 0)),
                    "ppp {ppp} {ventana:?}: {oculto} está oculto"
                );
            }
            comprueba_tab(&mut b, &format!("ppp {ppp} {ventana:?} indexado"));
        }
    }
}

#[test]
fn fila_11_tab_recorre_solo_lo_visible_con_color_en_rgba_y_en_gris() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for (indice_de_modo, fondo, indice_de_color, color) in [
                (1usize, "fondo_rgba", 3usize, "color_rgba"),
                (3, "fondo_gris", 2, "color_gris"),
            ] {
                let mut b = Banco::abierto(ppp, ventana, 16384);
                b.dialogo_mut().event(Event::Choice {
                    field: "modo",
                    index: indice_de_modo,
                });
                b.reposo();
                b.dialogo_mut().event(Event::Choice {
                    field: fondo,
                    index: indice_de_color,
                });
                b.reposo();
                let orden = focus_order(b.dialogo());
                assert!(orden.contains(&FocusTarget::Field(color)), "{color}");
                for otro in ["color_rgba", "color_gris_alfa", "color_gris"] {
                    assert_eq!(
                        orden.contains(&FocusTarget::Field(otro)),
                        otro == color,
                        "ppp {ppp} {ventana:?}: {otro}"
                    );
                }
                comprueba_tab(&mut b, &format!("ppp {ppp} {ventana:?} {color}"));
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
fn pon_rgba_con_color(b: &mut Banco) {
    b.dialogo_mut().event(Event::Choice {
        field: "modo",
        index: 1,
    });
    b.reposo();
    b.dialogo_mut().event(Event::Choice {
        field: "fondo_rgba",
        index: 3,
    });
    b.reposo();
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
fn fila_11_a_la_ventana_minima_el_cuerpo_mide_252_u_y_el_formulario_se_desplaza() {
    for ppp in PPPS {
        // Indexado por defecto: 280 u.
        let b = Banco::abierto(ppp, Ventana::Minima, 16384);
        let n = n_de(ppp);
        assert_eq!(b.dialogo().content_h_u(), 280, "ppp {ppp}: indexado");
        let layout = b.layout();
        assert_eq!(
            layout.body.height,
            252 * n,
            "ppp {ppp}: el cuerpo mide 252 u"
        );
        assert_eq!(layout.max_scroll_u, 280 - 252, "ppp {ppp}: indexado");

        // RGBA con «Color…»: 320 u.
        let mut b = Banco::abierto(ppp, Ventana::Minima, 16384);
        pon_rgba_con_color(&mut b);
        assert_eq!(b.dialogo().content_h_u(), 320, "ppp {ppp}: RGBA con Color…");
        assert_eq!(
            b.layout().max_scroll_u,
            320 - 252,
            "ppp {ppp}: RGBA con Color…"
        );
    }
}

#[test]
fn fila_11_a_la_minima_la_rueda_desplaza_el_cuerpo_y_titulo_pie_y_botones_no_se_mueven() {
    for ppp in PPPS {
        for con_color in [false, true] {
            let mut b = Banco::abierto(ppp, Ventana::Minima, 16384);
            if con_color {
                pon_rgba_con_color(&mut b);
            }
            let contexto = format!("ppp {ppp}, con Color… {con_color}");
            let maximo = b.layout().max_scroll_u;
            assert!(maximo > 0, "{contexto}");
            let fijos = ["Nueva imagen", "Crear", "Cancelar"];
            let antes_fijos: Vec<Option<f32>> = fijos.iter().map(|t| y_de(&b, t)).collect();
            let modo_antes = y_de(&b, "Modo").expect("Modo");
            assert_eq!(b.dialogo().scroll_u(), 0, "{contexto}");

            // Una muesca son 42 u (se recorta al máximo); con 320 u hacen falta dos.
            rueda_sobre_el_cuerpo(&mut b, 2);
            assert_eq!(b.dialogo().scroll_u(), maximo, "{contexto}: hasta el final");
            let desplazado = modo_antes - y_de(&b, "Modo").expect("Modo");
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
            assert!(
                b.ultimo.exacto("Crear").is_some(),
                "{contexto}: Crear se sigue viendo"
            );
            assert!(b.ultimo.exacto("Cancelar").is_some(), "{contexto}");
        }
    }
}

#[test]
fn fila_11_a_la_minima_tab_lleva_a_la_vista_el_hexadecimal_que_estaba_fuera() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Minima, 16384);
        pon_rgba_con_color(&mut b);
        assert_eq!(b.dialogo().scroll_u(), 0, "ppp {ppp}");
        b.foco_hasta(FocusTarget::Field("color_rgba"));
        assert!(
            b.dialogo().scroll_u() > 0,
            "ppp {ppp}: el foco en el hexadecimal desplaza el cuerpo"
        );
        assert!(b.ultimo.exacto("Crear").is_some());
        assert_eq!(
            b.foco_de_egui(),
            Some(Banco::id_de("color_rgba")),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_11_a_960_por_720_el_formulario_no_se_desplaza() {
    for ppp in PPPS {
        for con_color in [false, true] {
            let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
            if con_color {
                pon_rgba_con_color(&mut b);
            }
            let contexto = format!("ppp {ppp}, con Color… {con_color}");
            assert_eq!(b.layout().max_scroll_u, 0, "{contexto}");
            let antes: Vec<Option<f32>> = ["Nueva imagen", "Modo", "Crear", "Cancelar"]
                .iter()
                .map(|t| y_de(&b, t))
                .collect();
            rueda_sobre_el_cuerpo(&mut b, 3);
            assert_eq!(b.dialogo().scroll_u(), 0, "{contexto}");
            let despues: Vec<Option<f32>> = ["Nueva imagen", "Modo", "Crear", "Cancelar"]
                .iter()
                .map(|t| y_de(&b, t))
                .collect();
            assert_eq!(despues, antes, "{contexto}: nada se mueve");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: contraste
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

            // Por defecto.
            let mut b = Banco::abierto(ppp, ventana, 16384);
            comprueba_contraste(&b, &contexto("por defecto"));

            // Con un error en línea (y Crear atenuado, exento).
            b.escribe("0");
            comprueba_contraste(&b, &contexto("error en el ancho"));

            // Con la nota de la tarjeta gráfica.
            let mut b = Banco::abierto(ppp, ventana, 4096);
            b.escribe("5000");
            comprueba_contraste(&b, &contexto("error con tarjeta gráfica"));

            // RGBA con «Color…» y un hexadecimal inválido.
            let mut b = Banco::abierto(ppp, ventana, 16384);
            pon_rgba_con_color(&mut b);
            b.foco_hasta(FocusTarget::Field("color_rgba"));
            b.escribe("zz");
            comprueba_contraste(&b, &contexto("hexadecimal inválido"));
            b.escribe("#ff0000");
            comprueba_contraste(&b, &contexto("hexadecimal válido"));

            // Con el aviso del traslado del fondo en el informe.
            let mut b = Banco::abierto(ppp, ventana, 16384);
            {
                let d = b.dialogo_mut();
                d.event(Event::Choice {
                    field: "modo",
                    index: 1,
                });
            }
            b.reposo();
            {
                let d = b.dialogo_mut();
                d.event(Event::Choice {
                    field: "modo",
                    index: 3,
                });
            }
            b.reposo();
            assert!(
                texto_del_informe(b.dialogo()).contains("Gris sin Transparente: pasa a Blanco"),
                "el estado de la prueba lleva el aviso"
            );
            comprueba_contraste(&b, &contexto("con el aviso de fondo"));
        }
    }
}

#[test]
fn fila_11_los_textos_fijos_del_dialogo_estan_dentro_del_juego_de_caracteres() {
    let spec = new_image_form(16384);
    let mut textos: Vec<&str> = vec![spec.title, spec.accept];
    textos.extend(spec.cancel);
    for f in &spec.fields {
        textos.push(f.label);
        if let FieldKind::Choice { options, .. } = &f.kind {
            textos.extend(options.iter().copied());
        }
    }
    for texto in textos {
        assert!(en_el_juego(texto), "{texto:?} no está en INTERFACE_CHARSET");
    }
    for aviso in AVISOS {
        assert!(en_el_juego(aviso), "{aviso:?}");
        assert!(aviso.chars().count() <= 42, "{aviso:?}");
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
            sync,
            ..
        } = &mut *self;
        ejecuta(ctx, size, vec![], |ui| {
            let c = ui.ctx().clone();
            apply_scale(&c);
            if let Modal::Open(dialog) = modal {
                let _ = show_dialog(&c, dialog, focus);
                sync_form(dialog, sync);
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

    /// El `Id` y las opciones del grupo de fondo que se ve (el único `Choice` visible que no es el
    /// modo).
    fn grupo_de_fondo(&self) -> (&'static str, usize) {
        let d = self.dialogo();
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
fn fila_9_nueva_imagen_pinta_aros_y_un_punto_por_grupo_en_cada_modo_y_cada_fondo() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        b.marcadores(&format!("ppp {ppp} al abrir"));
        for modo in [1usize, 2, 3, 0, 2] {
            // Clic real en el aro del modo (el primer grupo, de arriba abajo).
            let formas = b.crudas();
            let centro = aros(&marcas_de(&formas))[modo].centro;
            b.clic(centro);
            b.reposo();
            assert_eq!(
                b.dialogo().chosen(FIELD_MODE),
                Some(modo),
                "ppp {ppp}: clic en el aro del modo {modo}"
            );
            b.marcadores(&format!("ppp {ppp} modo {modo}"));
            // Y en cada opción del fondo de ese modo (el segundo grupo).
            let (fondo, opciones) = b.grupo_de_fondo();
            for j in 0..opciones {
                let formas = b.crudas();
                let centro = aros(&marcas_de(&formas))[MODE_OPTIONS.len() + j].centro;
                b.clic(centro);
                b.reposo();
                assert_eq!(
                    b.dialogo().chosen(fondo),
                    Some(j),
                    "ppp {ppp} modo {modo}: clic en el aro {j} de {fondo}"
                );
                b.marcadores(&format!("ppp {ppp} modo {modo} {fondo} {j}"));
            }
        }
    }
}

#[test]
fn fila_11_nueva_imagen_el_anillo_de_foco_rodea_la_fila_de_cada_opcion_como_siempre() {
    for ppp in PPPS {
        let mut b = Banco::abierto(ppp, Ventana::Grande, 16384);
        let donde = format!("ppp {ppp}");
        let mut medidas = Vec::new();
        for (k, rotulo) in MODE_OPTIONS.iter().enumerate() {
            let objetivo = FocusTarget::Option(FIELD_MODE, k);
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

const GRIS_CLARO_A1: Rgba = Rgba::opaque(0xaa, 0xaa, 0xaa);
const GRIS_MEDIO_A1: Rgba = Rgba::new(0x80, 0x80, 0x80, 0x80);
const AZUL_A1: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const GRIS_POR_DEFECTO_A1: Rgba = Rgba::opaque(0x80, 0x80, 0x80);
const NO_VALIDO_A1: &str = "Color no válido: pasa a ";

/// Origen, último confirmado (o ninguno) y destino: con el aviso nuevo, con el de D1b de un color
/// que no viaja, con Indexado y con el alfa.
const CASOS_A1: [(Mode, Option<Rgba>, Mode); 5] = [
    (Mode::Rgba, None, Mode::GrayAlpha),
    (Mode::Gray, Some(GRIS_CLARO_A1), Mode::Rgba),
    (Mode::Rgba, Some(AZUL_A1), Mode::GrayAlpha),
    (Mode::GrayAlpha, None, Mode::Indexed),
    (Mode::Rgba, Some(GRIS_MEDIO_A1), Mode::Gray),
];

fn hex8_a1(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn etiqueta_a1(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "Indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "Gris con alfa",
        Mode::Gray => "Gris",
    }
}

fn modo_ix_a1(mode: Mode) -> usize {
    match mode {
        Mode::Indexed => 0,
        Mode::Rgba => 1,
        Mode::GrayAlpha => 2,
        Mode::Gray => 3,
    }
}

fn fondo_id_a1(mode: Mode) -> &'static str {
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

/// El fondo y el aviso que deja descartar un texto inválido con `ultimo` confirmado (A1-1 y A1-4).
fn tras_descartar_a1(from: Mode, ultimo: Rgba, to: Mode) -> (Background, String) {
    match carry_background(from, Background::Color(ultimo), to) {
        Carry::Kept(fondo) => (fondo, format!("{NO_VALIDO_A1}{}", hex8_a1(ultimo))),
        Carry::Reset { to: fondo, notice } => (fondo, notice.to_string()),
    }
}

/// Modo `from` con «Color…» y `confirmado` ya confirmado, con eventos directos (la preparación no es
/// lo que se prueba).
fn prepara_a1(b: &mut Banco, from: Mode, confirmado: Option<Rgba>) {
    let id = color_id_a1(from).expect("el modo tiene «Color…»");
    b.dialogo_mut().event(Event::Choice {
        field: "modo",
        index: modo_ix_a1(from),
    });
    b.reposo();
    b.dialogo_mut().event(Event::Choice {
        field: fondo_id_a1(from),
        index: color_ix_a1(from),
    });
    b.reposo();
    if let Some(c) = confirmado {
        let d = b.dialogo_mut();
        d.event(Event::Hex {
            field: id,
            text: hex8_a1(c),
        });
        d.event(Event::HexConfirm {
            field: id,
            blur: false,
        });
        b.reposo();
    }
    assert!(b.dialogo().is_shown(id), "{}", b.describe());
    assert_eq!(
        b.dialogo().hex_committed(id),
        Some(confirmado.unwrap_or(GRIS_POR_DEFECTO_A1))
    );
    let informe = texto_del_informe(b.dialogo());
    assert!(
        !hay_aviso_de_fondo(b) && !informe.contains(NO_VALIDO_A1),
        "la preparación no deja avisos; informe {informe:?}"
    );
}

/// Lo que ve `sync_form` del campo de origen y del modo en un fotograma.
#[derive(Clone, Debug)]
struct FotoA1 {
    modo: Option<usize>,
    pendiente: bool,
    texto: Option<String>,
    confirmado: Option<Rgba>,
}

fn foto_a1(b: &Banco, id: &'static str) -> FotoA1 {
    let d = b.dialogo();
    FotoA1 {
        modo: d.chosen("modo"),
        pendiente: d.hex_pending(id),
        texto: d.text(id),
        confirmado: d.hex_committed(id),
    }
}

/// Sube el cuerpo del todo con la rueda (a la ventana mínima se desplaza; a 960 × 720 no hace nada).
fn rueda_arriba_a1(b: &mut Banco) {
    let layout = b.layout();
    let n = n_de(b.ppp);
    let pos = pos2(
        (layout.body.x + 3 * n) as f32 / b.ppp,
        (layout.body.y + layout.body.height / 2) as f32 / b.ppp,
    );
    b.fotograma(vec![mover(pos)]);
    b.reposo();
    for _ in 0..4 {
        b.fotograma(vec![rueda_linea(true)]);
    }
    b.reposo();
    assert_eq!(b.dialogo().scroll_u(), 0, "{}", b.describe());
}

/// Lleva el foco al hexadecimal, escribe `zz` y comprueba que sigue enfocado y con el texto pendiente.
fn escribe_zz_a1(b: &mut Banco, id: &'static str) {
    b.foco_hasta(FocusTarget::Field(id));
    b.escribe("zz");
    assert!(b.dialogo().hex_pending(id), "{}", b.describe());
    assert_eq!(b.dialogo().text(id), Some("zz".to_string()));
    assert!(matches!(
        b.dialogo().validity(id),
        Some(Validity::Invalid(_))
    ));
    assert_eq!(
        b.foco_de_egui(),
        Some(Banco::id_de(id)),
        "el campo tiene el foco al escribir"
    );
}

/// Clic real en el texto `buscado`: mover, pulsar y soltar en tres fotogramas distintos, y tres de
/// reposo; una foto del campo `id` tras cada fotograma.
fn clic_con_traza_a1(b: &mut Banco, buscado: &str, id: &'static str) -> Vec<FotoA1> {
    let pos = b.centro_de(buscado);
    let mut traza = Vec::new();
    for eventos in [
        vec![mover(pos)],
        vec![boton(pos, true)],
        vec![boton(pos, false)],
        vec![],
        vec![],
        vec![],
    ] {
        b.fotograma(eventos);
        traza.push(foto_a1(b, id));
    }
    traza
}

/// Una tecla y tres fotogramas de reposo, con la foto del campo `id` tras cada uno.
fn tecla_con_traza_a1(b: &mut Banco, key: Key, id: &'static str) -> Vec<FotoA1> {
    let mut traza = Vec::new();
    b.fotograma(vec![tecla(key, Modifiers::NONE)]);
    traza.push(foto_a1(b, id));
    for _ in 0..3 {
        b.fotograma(vec![]);
        traza.push(foto_a1(b, id));
    }
    traza
}

/// El índice del fotograma en que cambió el modo (exactamente uno) respecto a `antes`.
fn fotograma_del_cambio_a1(antes: Option<usize>, traza: &[FotoA1], ctx: &str) -> usize {
    let mut previo = antes;
    let mut cambios = Vec::new();
    for (i, foto) in traza.iter().enumerate() {
        if foto.modo != previo {
            cambios.push(i);
        }
        previo = foto.modo;
    }
    assert_eq!(
        cambios.len(),
        1,
        "{ctx}: el modo cambia una vez; traza {traza:?}"
    );
    cambios[0]
}

/// El resultado de A1 o A2 con el aviso, en el diálogo y pintado.
fn comprueba_resultado_a1(b: &mut Banco, from: Mode, ultimo: Rgba, to: Mode, ctx: &str) {
    let (fondo, aviso) = tras_descartar_a1(from, ultimo, to);
    assert_eq!(
        texto_del_informe(b.dialogo()),
        report_text(Some((64, 64)), to, Some(&aviso)),
        "{ctx}: informe"
    );
    assert!(aviso.chars().count() <= 42 && en_el_juego(&aviso), "{ctx}");
    let spec = spec_del(b).unwrap_or_else(|| panic!("{ctx}: sin valores válidos"));
    assert_eq!(
        (spec.width, spec.height, spec.mode, spec.background),
        (64, 64, to, fondo),
        "{ctx}"
    );
    let viejo = color_id_a1(from).expect("color");
    assert!(!b.dialogo().is_shown(viejo), "{ctx}: el viejo se ve");
    if let (Background::Color(color), Some(id)) = (fondo, color_id_a1(to)) {
        assert!(b.dialogo().is_shown(id), "{ctx}");
        assert_eq!(b.dialogo().validity(id), Some(Validity::Valid), "{ctx}");
        assert!(
            !b.dialogo().hex_pending(id),
            "{ctx}: sin texto en el campo nuevo"
        );
        assert_eq!(b.dialogo().text(id), Some(hex_text(color)), "{ctx}");
        assert_eq!(b.dialogo().hex_committed(id), Some(color), "{ctx}");
        assert_eq!(
            b.dialogo().values().expect("válido").color(id),
            Some(color),
            "{ctx}"
        );
    }
    assert!(!b.crear_deshabilitado(), "{ctx}: Crear");
    // Pintado: a la ventana mínima el informe queda abajo del cuerpo.
    if b.ventana == Ventana::Minima {
        rueda_sobre_el_cuerpo(b, 3);
    }
    assert!(
        b.ultimo.contiene(&aviso),
        "{ctx}: falta {aviso:?} pintado; hay {:?}",
        b.ultimo.lista()
    );
}

#[test]
fn fila_a3_camino_a_escribir_zz_y_clic_en_el_modo_el_campo_ya_esta_revertido_y_el_aviso_sale() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for (from, confirmado, to) in CASOS_A1 {
                let ultimo = confirmado.unwrap_or(GRIS_POR_DEFECTO_A1);
                let ctx = format!("A3 ppp {ppp} {ventana:?} {from:?} {confirmado:?} → {to:?}");
                let id = color_id_a1(from).expect("color");
                let mut b = Banco::abierto(ppp, ventana, 16384);
                prepara_a1(&mut b, from, confirmado);
                escribe_zz_a1(&mut b, id);
                if ventana == Ventana::Minima {
                    rueda_arriba_a1(&mut b);
                    assert!(
                        b.dialogo().hex_pending(id),
                        "{ctx}: la rueda no cambia el campo"
                    );
                }

                let antes = b.dialogo().chosen("modo");
                let traza = clic_con_traza_a1(&mut b, etiqueta_a1(to), id);
                let cambio = fotograma_del_cambio_a1(antes, &traza, &ctx);
                let visto = &traza[cambio];
                eprintln!("{ctx}: en el fotograma {cambio} del clic sync_form ve {visto:?}");

                // La lectura del código: el clic quitó el foco al pulsar y el campo se revirtió a su
                // último confirmado antes (o en) el fotograma del cambio.
                assert!(
                    !visto.pendiente,
                    "{ctx}: LECTURA ERRÓNEA, el texto inválido SIGUE pendiente en el fotograma del cambio; traza {traza:?}"
                );
                assert_eq!(
                    visto.texto,
                    Some(hex_text(ultimo)),
                    "{ctx}: traza {traza:?}"
                );
                assert_eq!(visto.confirmado, Some(ultimo), "{ctx}");

                // Aun así, el aviso sale y el resultado es el de A1 o A2.
                comprueba_resultado_a1(&mut b, from, ultimo, to, &ctx);
            }
        }
    }
}

#[test]
fn fila_a4_camino_b_zz_intro_y_clic_en_el_modo_el_texto_sigue_pendiente_y_el_aviso_sale() {
    for ppp in PPPS {
        for ventana in VENTANAS {
            for (from, confirmado, to) in CASOS_A1 {
                let ultimo = confirmado.unwrap_or(GRIS_POR_DEFECTO_A1);
                let ctx = format!("A4 ppp {ppp} {ventana:?} {from:?} {confirmado:?} → {to:?}");
                let id = color_id_a1(from).expect("color");
                let mut b = Banco::abierto(ppp, ventana, 16384);
                prepara_a1(&mut b, from, confirmado);
                escribe_zz_a1(&mut b, id);
                b.pulsa(Key::Enter);
                // Intro con el texto inválido: el error y el foco se quedan; no se crea nada.
                assert!(b.modal.is_open() && b.aceptados.is_empty(), "{ctx}");
                assert!(b.dialogo().hex_pending(id), "{ctx}: el texto se queda");
                assert_eq!(b.dialogo().text(id), Some("zz".to_string()), "{ctx}");
                assert_eq!(
                    b.foco_de_egui(),
                    Some(Banco::id_de(id)),
                    "{ctx}: el foco se queda en el campo"
                );
                if ventana == Ventana::Minima {
                    rueda_arriba_a1(&mut b);
                }

                let antes = b.dialogo().chosen("modo");
                let traza = clic_con_traza_a1(&mut b, etiqueta_a1(to), id);
                let cambio = fotograma_del_cambio_a1(antes, &traza, &ctx);
                let visto = &traza[cambio];
                eprintln!("{ctx}: en el fotograma {cambio} del clic sync_form ve {visto:?}");

                assert!(
                    visto.pendiente,
                    "{ctx}: LECTURA ERRÓNEA, el texto inválido NO sigue pendiente en el fotograma del cambio; traza {traza:?}"
                );
                assert_eq!(
                    visto.texto,
                    Some("zz".to_string()),
                    "{ctx}: traza {traza:?}"
                );
                assert_eq!(visto.confirmado, Some(ultimo), "{ctx}");

                comprueba_resultado_a1(&mut b, from, ultimo, to, &ctx);
                // El campo del modo nuevo no conserva el texto.
                if let Some(nuevo) = color_id_a1(to) {
                    assert_ne!(b.dialogo().text(nuevo), Some("zz".to_string()), "{ctx}");
                }
            }
        }
    }
}

#[test]
fn fila_a4_camino_b_con_tab_hasta_el_selector_y_cambio_con_el_teclado() {
    let casos = [
        (Mode::Rgba, None, Mode::GrayAlpha),
        (Mode::Rgba, Some(AZUL_A1), Mode::Gray),
        (Mode::Gray, Some(GRIS_CLARO_A1), Mode::Indexed),
    ];
    for ppp in PPPS {
        for ventana in VENTANAS {
            for (from, confirmado, to) in casos {
                let ultimo = confirmado.unwrap_or(GRIS_POR_DEFECTO_A1);
                let ctx =
                    format!("A4 teclado ppp {ppp} {ventana:?} {from:?} {confirmado:?} → {to:?}");
                let id = color_id_a1(from).expect("color");
                let mut b = Banco::abierto(ppp, ventana, 16384);
                prepara_a1(&mut b, from, confirmado);
                escribe_zz_a1(&mut b, id);
                b.pulsa(Key::Enter);
                assert!(b.dialogo().hex_pending(id), "{ctx}: tras Intro");

                // Tab hasta la opción del modo de destino: el texto sigue pendiente por el camino.
                b.foco_hasta(FocusTarget::Option("modo", modo_ix_a1(to)));
                assert!(
                    b.dialogo().hex_pending(id),
                    "{ctx}: Tab no revierte el texto tras Intro"
                );
                assert_eq!(b.dialogo().text(id), Some("zz".to_string()), "{ctx}");

                // Espacio elige la opción enfocada.
                let antes = b.dialogo().chosen("modo");
                let traza = tecla_con_traza_a1(&mut b, Key::Space, id);
                let cambio = fotograma_del_cambio_a1(antes, &traza, &ctx);
                let visto = &traza[cambio];
                eprintln!("{ctx}: en el fotograma {cambio} sync_form ve {visto:?}");
                assert!(
                    visto.pendiente,
                    "{ctx}: LECTURA ERRÓNEA, el texto inválido NO sigue pendiente; traza {traza:?}"
                );
                assert_eq!(visto.texto, Some("zz".to_string()), "{ctx}");

                comprueba_resultado_a1(&mut b, from, ultimo, to, &ctx);
            }
        }
    }
}
