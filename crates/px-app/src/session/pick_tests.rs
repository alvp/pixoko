//! Tests de `session` con el cuentagotas (specs/hito-2-cuentagotas-borrador.md): fila 9
//! (`Session::apply_pick`, partes a a d), fila 12 (el cuentagotas no modifica el documento) y la
//! parte de sesión de la propiedad 2 (`active_pixel()` es exactamente el píxel visitado).
//!
//! Los ayudantes de los otros módulos de tests son privados: aquí se duplican los mínimos. Los
//! gestos del cuentagotas los da un `ToolState` con eventos a mano, sin ventana, y cada `Picked`
//! se aplica con `apply_pick` como hace `main.rs`.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{Command, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::canvas::CanvasEvent;
use px_ui::color::{ColorChoice, SlotBank};
use px_ui::pick::Picked;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState};

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse.
struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new() -> TmpDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("pixoko-px-app-pick-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("crear el directorio temporal");
        TmpDir { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn img(session: &Session) -> &Image {
    session
        .document
        .image(session.document.main_image())
        .expect("imagen principal")
}

/// `n` colores distintos.
fn distinct(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

/// Indexada de una fila con `colors` y los píxeles `0, 1, 2, ...` (uno por entrada).
fn indexed_row(colors: &[Rgba]) -> Image {
    let palette = Palette::from_colors(colors.iter().copied()).expect("paleta");
    let mut image = Image::new_indexed(colors.len() as u32, 1, palette).expect("imagen indexada");
    for x in 0..colors.len() as u32 {
        image
            .set_pixel(x, 0, Pixel::Indexed(x as u8))
            .expect("píxel");
    }
    image
}

fn session_indexed(entries: usize) -> Session {
    Session::sample(Document::new(indexed_row(&distinct(entries))))
}

/// Imagen de `w`×`h` en `mode` con píxeles variados, alfa 0 incluido. La indexada tiene seis
/// entradas y la 5 es la transparente.
fn sample_image(mode: Mode, w: u32, h: u32) -> Image {
    let mut image = match mode {
        Mode::Indexed => {
            let mut palette = Palette::from_colors(distinct(6)).expect("paleta");
            palette
                .set_transparent_index(Some(5))
                .expect("índice transparente");
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        other => Image::new(w, h, other).expect("imagen"),
    };
    for y in 0..h {
        for x in 0..w {
            let n = (y * w + x) as u8;
            let pixel = match mode {
                Mode::Rgba => Pixel::Rgba(Rgba::new(
                    n.wrapping_mul(13),
                    n.wrapping_mul(7),
                    n,
                    if n.is_multiple_of(4) { 0 } else { 255 - n },
                )),
                Mode::Indexed => Pixel::Indexed(n % 6),
                Mode::Gray => Pixel::Gray(n.wrapping_mul(11)),
                Mode::GrayAlpha => Pixel::GrayAlpha {
                    gray: n.wrapping_mul(11),
                    alpha: if n.is_multiple_of(3) { 0 } else { 255 - n },
                },
            };
            image.set_pixel(x, y, pixel).expect("píxel");
        }
    }
    image
}

fn session_of(mode: Mode) -> Session {
    Session::sample(Document::new(sample_image(mode, 5, 4)))
}

/// Una edición como un paso: el píxel `(0, 0)` pasa a un valor que no tenía.
fn edit_first_pixel(session: &mut Session) {
    let image = session.document.main_image();
    let pixel = match img(session).mode() {
        Mode::Rgba => Pixel::Rgba(Rgba::new(1, 2, 3, 255)),
        Mode::Indexed => Pixel::Indexed(1),
        Mode::Gray => Pixel::Gray(77),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: 77,
            alpha: 255,
        },
    };
    let changed = session
        .document
        .apply(Command::SetPixels {
            image,
            pixels: vec![PixelEdit { x: 0, y: 0, pixel }],
        })
        .expect("editar un píxel");
    assert!(changed, "la edición debía cambiar el documento");
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    state_id: u64,
    modified: bool,
    can_undo: bool,
    can_redo: bool,
    history_bytes: usize,
    image: Image,
}

fn snapshot(session: &Session) -> Snapshot {
    Snapshot {
        state_id: session.document.state_id(),
        modified: session.modified(),
        can_undo: session.document.can_undo(),
        can_redo: session.document.can_redo(),
        history_bytes: session.document.history_bytes(),
        image: img(session).clone(),
    }
}

fn picking_tools() -> ToolState {
    let mut tools = ToolState::new();
    tools.apply_action(ToolAction::Select(Tool::Dropper));
    tools
}

/// Manda un evento al cuentagotas, exige `Ok(false)` y aplica cada `Picked` a la sesión.
/// Devuelve los `Picked` y los avisos de `apply_pick`.
fn send(
    tools: &mut ToolState,
    session: &mut Session,
    event: CanvasEvent,
) -> Vec<(Picked, PickOutcome)> {
    let id = session.document.main_image();
    let changed = tools
        .canvas_event_with(&mut session.document, id, None, event)
        .expect("el cuentagotas no da error");
    assert!(!changed, "el cuentagotas nunca cambia el documento");
    tools
        .take_events()
        .into_iter()
        .map(|event| match event {
            ToolEvent::Picked(picked) => (picked, session.apply_pick(picked)),
            other => panic!("evento inesperado del cuentagotas: {other:?}"),
        })
        .collect()
}

/// Arrastre del cuentagotas sobre toda la imagen en orden de filas, empezando fuera de ella y
/// con movimientos fuera tras cada fila. Termina en `(w-1, h-1)`.
fn drag_over_image(session: &mut Session) -> usize {
    let (w, h) = (img(session).width(), img(session).height());
    let mut tools = picking_tools();
    let mut picks = send(&mut tools, session, CanvasEvent::StrokeBegin((-2, 0))).len();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            picks += send(&mut tools, session, CanvasEvent::StrokeMove((x, y))).len();
        }
        picks += send(
            &mut tools,
            session,
            CanvasEvent::StrokeMove((w as i32 + 5, y)),
        )
        .len();
    }
    picks += send(&mut tools, session, CanvasEvent::StrokeEnd).len();
    assert!(!tools.busy());
    picks
}

fn last_pixel(session: &Session) -> Pixel {
    let image = img(session);
    image
        .get_pixel(image.width() - 1, image.height() - 1)
        .expect("último píxel")
}

// ---------------------------------------------------------------------------------------
// Fila 9: `apply_pick`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_9a_entry_elige_la_entrada_muestra_su_fila_y_borra_el_rango() {
    let mut session = session_indexed(12);
    session.choice = ColorChoice::Entry(2);
    session.ops.set_range_end(5);
    assert!(!session.view.reveal);
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Entry(7));

    assert_eq!(session.choice, ColorChoice::Entry(7));
    assert!(session.view.reveal, "el panel muestra la fila elegida");
    assert_eq!(session.ops.range_end(), None, "el rango se borra");
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(7)));
    assert_eq!(outcome, PickOutcome { notice: None });
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9a_entry_a_la_entrada_ya_elegida_no_toca_la_vista_ni_el_rango() {
    let mut session = session_indexed(12);
    session.choice = ColorChoice::Entry(2);
    session.ops.set_range_end(5);

    let outcome = session.apply_pick(Picked::Entry(2));

    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert!(!session.view.reveal, "la elección no cambió");
    assert_eq!(
        session.ops.range_end(),
        Some(5),
        "como set_choice_revealing"
    );
    assert_eq!(outcome.notice, None);
}

#[test]
fn fila_9a_entry_en_la_ultima_entrada_y_la_transparente_sin_aviso() {
    let mut session = Session::sample(Document::new(sample_image(Mode::Indexed, 3, 3)));

    let last = session.apply_pick(Picked::Entry(5));

    assert_eq!(session.choice, ColorChoice::Entry(5));
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(5)));
    assert_eq!(last.notice, None, "tampoco en la entrada transparente");
}

#[test]
fn fila_9a_entry_fuera_de_la_paleta_se_ignora() {
    for index in [12u8, 13, 200, 255] {
        let mut session = session_indexed(12);
        session.choice = ColorChoice::Entry(2);
        session.ops.set_range_end(5);
        let before = snapshot(&session);

        let outcome = session.apply_pick(Picked::Entry(index));

        assert_eq!(session.choice, ColorChoice::Entry(2), "Entry({index})");
        assert!(!session.view.reveal, "Entry({index})");
        assert_eq!(session.ops.range_end(), Some(5), "Entry({index})");
        assert_eq!(outcome, PickOutcome { notice: None }, "Entry({index})");
        assert_eq!(snapshot(&session), before, "Entry({index})");
    }
}

#[test]
fn fila_9b_con_quita_pendiente_el_cuentagotas_indexado_se_ignora() {
    let mut session = session_indexed(12);
    session.choice = ColorChoice::Entry(2);
    session.ops.set_range_end(5);
    session.ops.begin_removal(2, Some(3));
    let removal = session.ops.removal();
    assert!(removal.is_some());
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Entry(7));

    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert!(!session.view.reveal);
    assert_eq!(session.ops.range_end(), Some(5));
    assert_eq!(session.ops.removal(), removal, "la quita sigue pendiente");
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(2)));
    assert_eq!(outcome, PickOutcome { notice: None });
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9b_cancelada_la_quita_el_cuentagotas_vuelve_a_valer() {
    let mut session = session_indexed(12);
    session.choice = ColorChoice::Entry(2);
    session.ops.begin_removal(2, Some(3));
    session.ops.cancel_removal();

    session.apply_pick(Picked::Entry(7));

    assert_eq!(session.choice, ColorChoice::Entry(7));
}

#[test]
fn fila_9_entry_en_un_modo_sin_paleta_se_ignora() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let mut session = session_of(mode);
        session.choice = ColorChoice::Slot(3);
        let slots = session.slots.clone();
        let before = snapshot(&session);

        let outcome = session.apply_pick(Picked::Entry(1));

        assert_eq!(session.choice, ColorChoice::Slot(3), "{mode:?}");
        assert!(!session.view.reveal, "{mode:?}");
        assert_eq!(session.slots, slots, "{mode:?}");
        assert_eq!(outcome, PickOutcome { notice: None }, "{mode:?}");
        assert_eq!(snapshot(&session), before, "{mode:?}");
    }
}

#[test]
fn fila_9c_rgba_escribe_la_ranura_activa_y_no_cambia_la_eleccion() {
    let mut session = session_of(Mode::Rgba);
    session.choice = ColorChoice::Slot(3);
    let color = Rgba::new(10, 20, 30, 128);
    let mut expected = SlotBank::new();
    expected.get_mut(Mode::Rgba).set(3, color);
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Color(color));

    assert_eq!(session.slots.get(Mode::Rgba).get(3), Some(color));
    assert_eq!(
        session.slots, expected,
        "el resto de ranuras y de modos igual"
    );
    assert_eq!(session.choice, ColorChoice::Slot(3));
    assert_eq!(session.active_pixel(), Some(Pixel::Rgba(color)));
    assert_eq!(outcome, PickOutcome { notice: None });
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9c_rgba_transparente_se_copia_tal_cual_y_avisa() {
    let mut session = session_of(Mode::Rgba);
    session.choice = ColorChoice::Slot(3);
    let color = Rgba::new(10, 20, 30, 0);

    let outcome = session.apply_pick(Picked::Color(color));

    assert_eq!(session.slots.get(Mode::Rgba).get(3), Some(color));
    assert_eq!(session.active_pixel(), Some(Pixel::Rgba(color)));
    assert_eq!(
        outcome.notice.as_deref(),
        Some("píxel transparente copiado tal cual (#0a141e00)")
    );
}

#[test]
fn fila_9c_cada_ranura_recibe_su_color() {
    for slot in 1..=9u8 {
        let mut session = session_of(Mode::Rgba);
        session.choice = ColorChoice::Slot(slot);
        let color = Rgba::new(slot, 100, 200, 255);
        let mut expected = SlotBank::new();
        expected.get_mut(Mode::Rgba).set(slot, color);

        session.apply_pick(Picked::Color(color));

        assert_eq!(session.slots, expected, "ranura {slot}");
        assert_eq!(session.active_pixel(), Some(Pixel::Rgba(color)));
    }
}

#[test]
fn fila_9d_gray_escribe_la_ranura_del_modo_sin_aviso() {
    let mut session = session_of(Mode::Gray);
    session.choice = ColorChoice::Slot(5);
    let color = Rgba::opaque(96, 96, 96);
    let mut expected = SlotBank::new();
    expected.get_mut(Mode::Gray).set(5, color);
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Color(color));

    assert_eq!(session.slots, expected);
    assert_eq!(session.choice, ColorChoice::Slot(5));
    assert_eq!(session.active_pixel(), Some(Pixel::Gray(96)));
    assert_eq!(outcome, PickOutcome { notice: None });
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9d_gray_alpha_transparente_avisa_con_el_valor_exacto() {
    let mut session = session_of(Mode::GrayAlpha);
    session.choice = ColorChoice::Slot(2);
    let color = Rgba::new(7, 7, 7, 0);
    let mut expected = SlotBank::new();
    expected.get_mut(Mode::GrayAlpha).set(2, color);
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Color(color));

    assert_eq!(session.slots, expected);
    assert_eq!(session.choice, ColorChoice::Slot(2));
    assert_eq!(
        session.active_pixel(),
        Some(Pixel::GrayAlpha { gray: 7, alpha: 0 })
    );
    assert_eq!(
        outcome.notice.as_deref(),
        Some("píxel transparente copiado tal cual (#07070700)")
    );
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9d_gray_alpha_semitransparente_sin_aviso() {
    let mut session = session_of(Mode::GrayAlpha);
    session.choice = ColorChoice::Slot(1);

    let outcome = session.apply_pick(Picked::Color(Rgba::new(7, 7, 7, 200)));

    assert_eq!(
        session.active_pixel(),
        Some(Pixel::GrayAlpha {
            gray: 7,
            alpha: 200
        })
    );
    assert_eq!(outcome.notice, None);
}

#[test]
fn fila_9_color_con_imagen_indexada_se_ignora() {
    let mut session = session_indexed(12);
    session.choice = ColorChoice::Entry(2);
    session.ops.set_range_end(5);
    let slots = session.slots.clone();
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Color(Rgba::new(10, 20, 30, 0)));

    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(session.slots, slots);
    assert_eq!(session.ops.range_end(), Some(5));
    assert_eq!(
        outcome,
        PickOutcome { notice: None },
        "ignorado: sin aviso aunque el alfa sea 0"
    );
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9_color_con_una_eleccion_que_no_es_ranura_se_ignora() {
    let mut session = session_of(Mode::Rgba);
    session.choice = ColorChoice::Entry(2);
    let slots = session.slots.clone();
    let before = snapshot(&session);

    let outcome = session.apply_pick(Picked::Color(Rgba::new(10, 20, 30, 0)));

    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(session.slots, slots);
    assert_eq!(outcome, PickOutcome { notice: None });
    assert_eq!(snapshot(&session), before);
}

#[test]
fn fila_9_dos_elecciones_seguidas_se_quedan_con_la_ultima() {
    let mut session = session_of(Mode::Rgba);
    session.choice = ColorChoice::Slot(4);

    session.apply_pick(Picked::Color(Rgba::new(1, 2, 3, 255)));
    session.apply_pick(Picked::Color(Rgba::new(4, 5, 6, 255)));

    assert_eq!(
        session.active_pixel(),
        Some(Pixel::Rgba(Rgba::new(4, 5, 6, 255)))
    );
}

// ---------------------------------------------------------------------------------------
// Fila 12: el cuentagotas no modifica el documento.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Start {
    /// Recién creada: sin cambios.
    Fresh,
    /// Una edición previa, sin guardar.
    Edited,
    /// Una edición y un `undo`: hay algo que rehacer.
    Undone,
    /// Una edición ya guardada: `modified()` falso con historia.
    Saved,
}

const STARTS: [Start; 4] = [Start::Fresh, Start::Edited, Start::Undone, Start::Saved];

struct Case {
    session: Session,
    /// La imagen antes de la edición previa.
    original: Image,
    /// La imagen con la edición previa, si la hubo.
    edited: Option<Image>,
}

fn case(mode: Mode, start: Start, tmp: &TmpDir) -> Case {
    let mut session = session_of(mode);
    let original = img(&session).clone();
    let mut edited = None;
    match start {
        Start::Fresh => {}
        Start::Edited => {
            edit_first_pixel(&mut session);
            edited = Some(img(&session).clone());
        }
        Start::Undone => {
            edit_first_pixel(&mut session);
            edited = Some(img(&session).clone());
            session.undo().expect("deshacer");
        }
        Start::Saved => {
            edit_first_pixel(&mut session);
            edited = Some(img(&session).clone());
            session
                .save_to(&tmp.path().join("guardada.png"))
                .expect("guardar");
        }
    }
    Case {
        session,
        original,
        edited,
    }
}

fn check_dropper_leaves_document_alone(mode: Mode) {
    for start in STARTS {
        let tmp = TmpDir::new();
        let Case {
            mut session,
            original,
            edited,
        } = case(mode, start, &tmp);
        let label = format!("{mode:?}, {start:?}");
        match start {
            Start::Fresh => assert!(!session.modified(), "{label}: recién abierta"),
            Start::Edited => assert!(session.modified(), "{label}"),
            Start::Undone => assert!(session.document.can_redo(), "{label}"),
            Start::Saved => {
                assert!(!session.modified(), "{label}");
                assert!(session.document.can_undo(), "{label}");
            }
        }
        let before = snapshot(&session);

        let picks = drag_over_image(&mut session);

        assert_eq!(picks, 20, "{label}: un Picked por cada píxel de 5×4");
        assert_eq!(snapshot(&session), before, "{label}");
        assert_eq!(
            session.active_pixel(),
            Some(last_pixel(&session)),
            "{label}: el color activo es el del último píxel visitado"
        );

        let active = session.active_pixel();
        let choice = session.choice;
        let slots = session.slots.clone();
        match start {
            Start::Fresh => {}
            Start::Edited | Start::Saved => {
                session.undo().expect("Ctrl+Z deshace la edición previa");
                assert_eq!(img(&session), &original, "{label}");
                assert!(!session.document.can_undo(), "{label}");
                assert!(session.document.can_redo(), "{label}");
            }
            Start::Undone => {
                session.redo().expect("rehacer");
                assert_eq!(img(&session), edited.as_ref().unwrap(), "{label}");
            }
        }
        assert_eq!(session.choice, choice, "{label}: la elección no se deshace");
        assert_eq!(session.slots, slots, "{label}: ni las ranuras");
        assert_eq!(
            session.active_pixel(),
            active,
            "{label}: el color elegido sobrevive"
        );
    }
}

#[test]
fn fila_12_rgba_el_cuentagotas_no_modifica_el_documento() {
    check_dropper_leaves_document_alone(Mode::Rgba);
}

#[test]
fn fila_12_indexada_el_cuentagotas_no_modifica_el_documento() {
    check_dropper_leaves_document_alone(Mode::Indexed);
}

#[test]
fn fila_12_gray_el_cuentagotas_no_modifica_el_documento() {
    check_dropper_leaves_document_alone(Mode::Gray);
}

#[test]
fn fila_12_gray_alpha_el_cuentagotas_no_modifica_el_documento() {
    check_dropper_leaves_document_alone(Mode::GrayAlpha);
}

#[test]
fn fila_12_un_clic_en_un_pixel_transparente_tampoco_modifica_el_documento() {
    for mode in [Mode::Rgba, Mode::GrayAlpha] {
        let mut session = session_of(mode);
        let before = snapshot(&session);
        let mut tools = picking_tools();

        let picks = send(&mut tools, &mut session, CanvasEvent::StrokeBegin((0, 0)));
        send(&mut tools, &mut session, CanvasEvent::StrokeEnd);

        assert_eq!(picks.len(), 1, "{mode:?}");
        assert!(
            picks[0].1.notice.is_some(),
            "{mode:?}: el píxel (0, 0) es transparente"
        );
        assert_eq!(snapshot(&session), before, "{mode:?}");
        assert!(!session.modified(), "{mode:?}");
    }
}

#[test]
fn fila_12_indexada_la_eleccion_sigue_a_los_pixeles_y_el_documento_no_cambia() {
    let mut session = Session::sample(Document::new(indexed_row(&distinct(6))));
    let before = snapshot(&session);
    let mut tools = picking_tools();

    send(&mut tools, &mut session, CanvasEvent::StrokeBegin((3, 0)));
    assert_eq!(session.choice, ColorChoice::Entry(3));
    send(&mut tools, &mut session, CanvasEvent::StrokeMove((5, 0)));
    assert_eq!(session.choice, ColorChoice::Entry(5));
    send(&mut tools, &mut session, CanvasEvent::StrokeMove((20, 0)));
    assert_eq!(session.choice, ColorChoice::Entry(5), "fuera no elige");
    send(&mut tools, &mut session, CanvasEvent::StrokeEnd);

    assert_eq!(session.choice, ColorChoice::Entry(5));
    assert_eq!(snapshot(&session), before);
}

// ---------------------------------------------------------------------------------------
// Propiedad 2 (parte de sesión): `active_pixel()` es exactamente el píxel visitado.
// ---------------------------------------------------------------------------------------

fn arb_mode() -> impl Strategy<Value = Mode> {
    prop_oneof![
        Just(Mode::Rgba),
        Just(Mode::Indexed),
        Just(Mode::Gray),
        Just(Mode::GrayAlpha),
    ]
}

/// Una coordenada con sesgo hacia dentro y los bordes de imágenes de hasta 16 de lado.
fn arb_coord() -> BoxedStrategy<i32> {
    prop_oneof![
        10 => -2i32..=17,
        1 => Just(i32::MIN),
        1 => Just(i32::MAX),
        1 => Just(-1),
        1 => Just(0),
        1 => Just(15),
        1 => Just(16),
    ]
    .boxed()
}

fn arb_pos() -> impl Strategy<Value = (i32, i32)> {
    (arb_coord(), arb_coord())
}

/// Hasta cuatro gestos: pulsación, movimientos y suelta.
type Gesture = ((i32, i32), Vec<(i32, i32)>);

fn arb_gestures() -> impl Strategy<Value = Vec<Gesture>> {
    prop::collection::vec((arb_pos(), prop::collection::vec(arb_pos(), 0..30)), 1..=4)
}

/// Imagen de `w`×`h` en `mode` con los bytes de `seeds` (al menos 256).
fn image_from(mode: Mode, w: u32, h: u32, seeds: &[[u8; 4]]) -> Image {
    let mut image = match mode {
        Mode::Indexed => {
            let colors = seeds[..16]
                .iter()
                .map(|s| Rgba::new(s[0], s[1], s[2], s[3]));
            let mut palette = Palette::from_colors(colors).expect("paleta");
            if seeds[0][0].is_multiple_of(3) {
                palette
                    .set_transparent_index(Some(seeds[0][1] % 16))
                    .expect("índice transparente");
            }
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        other => Image::new(w, h, other).expect("imagen"),
    };
    for y in 0..h {
        for x in 0..w {
            let s = seeds[(y * w + x) as usize];
            let pixel = match mode {
                Mode::Rgba => Pixel::Rgba(Rgba::new(s[0], s[1], s[2], s[3])),
                Mode::Indexed => Pixel::Indexed(s[0] % 16),
                Mode::Gray => Pixel::Gray(s[0]),
                Mode::GrayAlpha => Pixel::GrayAlpha {
                    gray: s[0],
                    alpha: s[3],
                },
            };
            image.set_pixel(x, y, pixel).expect("píxel");
        }
    }
    image
}

proptest! {
    #[test]
    fn propiedad_2_sesion_active_pixel_es_el_ultimo_pixel_visitado(
        mode in arb_mode(),
        w in 1u32..=16,
        h in 1u32..=16,
        seeds in prop::collection::vec(any::<[u8; 4]>(), 256),
        gestures in arb_gestures(),
        edit_first in any::<bool>(),
    ) {
        let image = image_from(mode, w, h, &seeds);
        let mut session = Session::sample(Document::new(image));
        if edit_first {
            let id = session.document.main_image();
            let pixel = match mode {
                Mode::Rgba => Pixel::Rgba(Rgba::new(9, 9, 9, 9)),
                Mode::Indexed => Pixel::Indexed(15),
                Mode::Gray => Pixel::Gray(9),
                Mode::GrayAlpha => Pixel::GrayAlpha { gray: 9, alpha: 9 },
            };
            let _ = session.document.apply(Command::SetPixels {
                image: id,
                pixels: vec![PixelEdit { x: 0, y: 0, pixel }],
            });
        }
        let initial_active = session.active_pixel();
        let before = snapshot(&session);

        let mut expected_last: Option<(u32, u32)> = None;
        let mut expected_picks = 0usize;
        let mut picks = 0usize;
        let mut tools = picking_tools();
        for (begin, moves) in &gestures {
            let mut last: Option<(u32, u32)> = None;
            let mut visit = |pos: (i32, i32)| {
                let (Ok(x), Ok(y)) = (u32::try_from(pos.0), u32::try_from(pos.1)) else {
                    return;
                };
                if x < w && y < h && last != Some((x, y)) {
                    last = Some((x, y));
                    expected_last = Some((x, y));
                    expected_picks += 1;
                }
            };
            visit(*begin);
            for &pos in moves {
                visit(pos);
            }
            picks += send(&mut tools, &mut session, CanvasEvent::StrokeBegin(*begin)).len();
            for &pos in moves {
                picks += send(&mut tools, &mut session, CanvasEvent::StrokeMove(pos)).len();
            }
            picks += send(&mut tools, &mut session, CanvasEvent::StrokeEnd).len();
        }

        prop_assert_eq!(picks, expected_picks);
        prop_assert_eq!(snapshot(&session), before);
        match expected_last {
            Some((x, y)) => {
                let pixel = img(&session).get_pixel(x, y).expect("píxel dentro");
                prop_assert_eq!(session.active_pixel(), Some(pixel));
            }
            None => prop_assert_eq!(session.active_pixel(), initial_active),
        }
    }
}
