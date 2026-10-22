//! Tests de `Session::convert_mode` (specs/hito-2-convertir-modo.md, unidad 20): filas 7 y 8 y la
//! propiedad 3. En ROJO hasta la implementación.
//!
//! - Fila 7: aplicar desde los cuatro modos a cada destino con permisos suficientes: `Ok(true)`, un
//!   solo paso, `history_bytes` entre los bytes anteriores y +2048, `state_id` nuevo, `modified()`,
//!   la imagen de `convert_image` con los mismos permisos, la elección que da `clamp_choice` (la
//!   misma de deshacer y rehacer), `view.reveal`, rango fuera, `undo` y `redo`, y el mismo modo
//!   (`Ok(false)` sin paso).
//! - Fila 8: los errores (`LossNotAllowed`, `TooManyColors`) y el mismo modo dejan documento, historia,
//!   `state_id`, `modified()` y elección intactos; `reserve_for_convert` con un `ImageAlloc` falso;
//!   los avisos (`convert_memory_notice`, `describe_converted`); la historia con límite 0 y con el
//!   límite por defecto, y que el aviso de historia de la fila 5 coincide con lo que de verdad pasa.
//! - Propiedad 3: aplicar es atómico y reversible, contra un modelo de instantáneas, a través de
//!   `Session`.
//!
//! **Suposiciones y ambigüedades:**
//! - «`history_bytes` es el mismo tras `apply`, `undo` y `redo`» se lee como en la spec 9: el paso
//!   cuesta lo mismo, así que tras `undo` vuelve a ser el de antes de convertir y tras `redo`, el de
//!   después de convertir.
//! - El caso de 4096² RGBA con el límite por defecto (64 MiB) reserva 64 MiB y va `#[ignore]`; en su
//!   lugar, sin ignorar, se prueban las mismas relaciones (`c > límite`, `límite/2 < c ≤ límite`) con
//!   una imagen gris de 64 × 64 y un límite pequeño que se fija con `set_history_limit`. Para evitar
//!   la reserva real en las pruebas del aviso haría falta una firma que no pida la imagen, por
//!   ejemplo `history_warning_for_bytes(pixel_bytes: u64, history: History)`.
//! - `Err` en `convert_mode` es `DocumentError::Convert(ConvertError::..)` (el que da `apply`).
//! - La quita pendiente y el rango se prueban a través de `normalize`/`ops` como en deshacer y
//!   rehacer: el rango de rotación se descarta y la quita pendiente se cancela y se avisa una vez.

use super::*;

use proptest::prelude::*;
use px_core::{
    Allowed, Command, ConvertError, Document, DocumentError, Image, ImageError, Mode, Palette,
    Pixel, PixelEdit, Rgba, convert_image, target_byte_len,
};
use px_ui::color::{ColorChoice, pixel_for_choice};

use crate::convert_mode::{
    Analysis, ConvertModeError, HISTORY_MARGIN, History, analyze, convert_memory_notice,
    describe_converted, history_warning, reserve_for_convert,
};
use crate::new_image::{ImageAlloc, NewImageSpec, SystemAlloc};

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

const TEST_MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const MIB: usize = 1 << 20;

/// Todo permitido: el color y el fondo blanco para el alfa.
const ALL: Allowed = Allowed {
    luma: true,
    drop_alpha: Some([255, 255, 255]),
};

const NOTICE_HISTORY_ALL: &str = "aviso: convertir descarta el historial anterior; solo se podrá deshacer hasta tu próxima edición";
const NOTICE_HISTORY_PART: &str = "aviso: convertir descarta parte del historial anterior";

/// Los destinos de `from` en el orden de D2.
fn targets_2(from: Mode) -> [Mode; 3] {
    let v: Vec<Mode> = TEST_MODES.into_iter().filter(|m| *m != from).collect();
    [v[0], v[1], v[2]]
}

fn rgba_de(pixels: &[Rgba]) -> Image {
    let mut image = Image::new(pixels.len() as u32, 1, Mode::Rgba).expect("imagen RGBA");
    for (x, p) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Rgba(*p))
            .expect("píxel");
    }
    image
}

fn gray_of(values: &[u8]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::Gray).expect("imagen Gray");
    for (x, v) in values.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Gray(*v))
            .expect("píxel");
    }
    image
}

fn gray_alpha_of(values: &[(u8, u8)]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::GrayAlpha).expect("imagen GrayAlpha");
    for (x, (gray, alpha)) in values.iter().enumerate() {
        image
            .set_pixel(
                x as u32,
                0,
                Pixel::GrayAlpha {
                    gray: *gray,
                    alpha: *alpha,
                },
            )
            .expect("píxel");
    }
    image
}

fn indexed_of(colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Image {
    let mut palette = Palette::from_colors(colors.iter().copied()).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente");
    let mut image = Image::new_indexed(pixels.len() as u32, 1, palette).expect("imagen indexada");
    for (x, i) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(*i))
            .expect("píxel");
    }
    image
}

/// Una imagen pequeña de cada modo. La indexada tiene 8 entradas (para que `Entry(5)` valga) con la
/// 2 transparente; la RGBA tiene color (3 píxeles) y alfa (2 píxeles) que perder al pasar a gris.
fn image_of(mode: Mode) -> Image {
    match mode {
        Mode::Rgba => rgba_de(&[
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 128),
            Rgba::opaque(7, 7, 7),
            Rgba::new(0, 0, 255, 128),
        ]),
        Mode::Gray => gray_of(&[0, 127, 255, 40]),
        Mode::GrayAlpha => gray_alpha_of(&[(10, 0), (10, 255), (200, 128), (30, 0)]),
        Mode::Indexed => indexed_of(
            &[
                Rgba::opaque(10, 10, 10),
                Rgba::opaque(200, 0, 0),
                Rgba::opaque(5, 5, 5),
                Rgba::opaque(1, 2, 3),
                Rgba::opaque(4, 5, 6),
                Rgba::opaque(7, 8, 9),
                Rgba::opaque(9, 9, 9),
                Rgba::opaque(255, 255, 255),
            ],
            Some(2),
            &[0, 1, 2, 3, 4, 5, 6, 7],
        ),
    }
}

fn session_of(image: Image) -> Session {
    let mut session = Session::sample(Document::new(image));
    session.view.reveal = false;
    session
}

fn image_current(session: &Session) -> &Image {
    session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen")
}

fn mode_current(session: &Session) -> Mode {
    image_current(session).mode()
}

fn bytes_of_pixels(image: &Image) -> usize {
    Image::byte_len(image.width(), image.height(), image.mode()).expect("tamaño válido")
}

/// Cuántos pasos de deshacer tiene el documento (se cuentan en una copia).
fn steps_of_undo(document: &Document) -> usize {
    let mut copy = document.clone();
    let mut steps_2 = 0;
    while copy.undo().is_ok() {
        steps_2 += 1;
    }
    steps_2
}

/// La elección que debe quedar para una imagen (escrita aparte de `clamp_choice`): en indexadas,
/// `Entry(min(i, len − 1))` y `Slot(n)` pasa a `Entry(n − 1)`; en las demás, la ranura si vale y, si
/// no, la 1.
fn choice_expected(image: &Image, choice: ColorChoice) -> ColorChoice {
    match image.palette() {
        Some(palette) => {
            let index_2 = match choice {
                ColorChoice::Entry(i) => usize::from(i),
                ColorChoice::Slot(n) => usize::from(n).saturating_sub(1),
            };
            ColorChoice::Entry(index_2.min(palette.len() - 1) as u8)
        }
        None => match choice {
            ColorChoice::Slot(n) if (1..=9).contains(&n) => choice,
            _ => ColorChoice::Slot(1),
        },
    }
}

/// Todo lo observable que un `Err` o un `Ok(false)` no deben tocar.
#[derive(Clone, Debug, PartialEq)]
struct TestSnapshot {
    image: Image,
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    history_bytes: usize,
    modified: bool,
    choice: ColorChoice,
    reveal: bool,
    slots: px_ui::color::SlotBank,
}

fn snapshot(session: &Session) -> TestSnapshot {
    TestSnapshot {
        image: image_current(session).clone(),
        state_id: session.document.state_id(),
        can_undo: session.document.can_undo(),
        can_redo: session.document.can_redo(),
        history_bytes: session.document.history_bytes(),
        modified: session.modified(),
        choice: session.choice,
        reveal: session.view.reveal,
        slots: session.slots.clone(),
    }
}

/// Una edición de un píxel que cambia el valor, en el modo que sea (`i` elige un valor distinto).
fn edits_pixel(session: &mut Session, x: u32, y: u32, i: u8) {
    let id = ImageId::from_raw(0);
    let mode = mode_current(session);
    let pixel = match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::opaque(i, 1, 2)),
        Mode::Gray => Pixel::Gray(i),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: i,
            alpha: 255,
        },
        Mode::Indexed => Pixel::Indexed(i % 8),
    };
    let change = session
        .document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit { x, y, pixel }],
        })
        .expect("editar");
    assert!(change, "la edición {i} debe cambiar el píxel ({x}, {y})");
}

// ---------------------------------------------------------------------------------------
// Fila 7: aplicar.
// ---------------------------------------------------------------------------------------

#[test]
fn row_7_from_each_mode_to_each_target_it_is_ok_true_with_a_single_step_and_the_image_of_convert_image()
 {
    for from in TEST_MODES {
        for target in targets_2(from) {
            let original = image_of(from);
            let mut session = session_of(original.clone());
            let context = format!("{from:?} a {target:?}");
            assert!(!session.document.can_undo(), "{context}");
            let h0 = session.document.history_bytes();
            let expected = convert_image(&original, target, ALL)
                .expect("con todo permitido se puede")
                .image;

            assert_eq!(session.convert_mode(target, ALL), Ok(true), "{context}");

            assert_eq!(image_current(&session), &expected, "{context}");
            assert_eq!(mode_current(&session), target, "{context}");
            assert!(session.document.can_undo(), "{context}");
            assert!(!session.document.can_redo(), "{context}");
            assert_eq!(
                steps_of_undo(&session.document),
                1,
                "{context}: un solo paso"
            );
            let growth = session.document.history_bytes() - h0;
            let pixels_2 = bytes_of_pixels(&original);
            assert!(
                (pixels_2..=pixels_2 + 2048).contains(&growth),
                "{context}: la historia creció {growth} y los píxeles anteriores son {pixels_2}"
            );
        }
    }
}

#[test]
fn row_7_the_state_is_new_and_the_document_is_left_modified() {
    for from in TEST_MODES {
        for target in targets_2(from) {
            let mut session = session_of(image_of(from));
            assert!(!session.modified());
            let before = session.document.state_id();
            assert_eq!(session.convert_mode(target, ALL), Ok(true));
            assert_ne!(session.document.state_id(), before, "{from:?} a {target:?}");
            assert!(session.modified(), "{from:?} a {target:?}");
        }
    }
}

#[test]
fn row_7_a_indexed_slot_3_becomes_entry_min_2_and_len_minus_1() {
    // Imágenes con 4 colores, con 3 y con 2 (el mínimo se aplica).
    for (name_2, image, expected) in [
        (
            "RGBA de 4 colores",
            image_of(Mode::Rgba),
            ColorChoice::Entry(2),
        ),
        (
            "Gray de 4 colores",
            image_of(Mode::Gray),
            ColorChoice::Entry(2),
        ),
        (
            "GrayAlpha de 3 entradas (con el transparente)",
            image_of(Mode::GrayAlpha),
            ColorChoice::Entry(2),
        ),
        (
            "Gray de 2 colores",
            gray_of(&[5, 6, 5, 6]),
            ColorChoice::Entry(1),
        ),
        (
            "RGBA de 1 color",
            rgba_de(&[Rgba::opaque(1, 2, 3), Rgba::opaque(1, 2, 3)]),
            ColorChoice::Entry(0),
        ),
    ] {
        let mut session = session_of(image);
        session.choice = ColorChoice::Slot(3);
        session.view.reveal = false;
        assert_eq!(
            session.convert_mode(Mode::Indexed, ALL),
            Ok(true),
            "{name_2}"
        );
        assert_eq!(session.choice, expected, "{name_2}");
        assert!(
            pixel_for_choice(image_current(&session), session.choice).is_some(),
            "{name_2}: la elección es válida"
        );
        assert!(
            session.view.reveal,
            "{name_2}: la elección cambió y el panel la muestra"
        );
    }
}

#[test]
fn row_7_from_indexed_entry_5_becomes_slot_1() {
    for target in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let mut session = session_of(image_of(Mode::Indexed));
        session.choice = ColorChoice::Entry(5);
        session.view.reveal = false;
        assert_eq!(session.convert_mode(target, ALL), Ok(true), "{target:?}");
        assert_eq!(session.choice, ColorChoice::Slot(1), "{target:?}");
        assert!(session.view.reveal, "{target:?}: la elección cambió");
    }
}

#[test]
fn row_7_between_modes_without_a_palette_the_slot_is_kept_and_the_panel_does_not_move() {
    for from in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for target in targets_2(from).into_iter().filter(|t| *t != Mode::Indexed) {
            for slot in [1u8, 3, 9] {
                let mut session = session_of(image_of(from));
                session.choice = ColorChoice::Slot(slot);
                session.view.reveal = false;
                assert_eq!(session.convert_mode(target, ALL), Ok(true));
                assert_eq!(
                    session.choice,
                    ColorChoice::Slot(slot),
                    "{from:?} a {target:?}, ranura {slot}"
                );
                assert!(
                    !session.view.reveal,
                    "{from:?} a {target:?}: la elección no cambió"
                );
            }
        }
    }
}

#[test]
fn row_7_the_resulting_choice_matches_the_model_for_all_the_pairs() {
    for from in TEST_MODES {
        for target in targets_2(from) {
            for choice in [
                ColorChoice::Slot(1),
                ColorChoice::Slot(3),
                ColorChoice::Slot(9),
                ColorChoice::Entry(0),
                ColorChoice::Entry(5),
            ] {
                // Una elección que valga en el origen: `Entry` solo en indexadas, `Slot` en las
                // demás.
                let valid = match (from, choice) {
                    (Mode::Indexed, ColorChoice::Entry(_)) => true,
                    (Mode::Indexed, ColorChoice::Slot(_)) => false,
                    (_, ColorChoice::Slot(_)) => true,
                    (_, ColorChoice::Entry(_)) => false,
                };
                if !valid {
                    continue;
                }
                let mut session = session_of(image_of(from));
                session.choice = choice;
                session.view.reveal = false;
                assert_eq!(session.convert_mode(target, ALL), Ok(true));
                let expected = choice_expected(image_current(&session), choice);
                assert_eq!(
                    session.choice, expected,
                    "{from:?} a {target:?} con {choice:?}"
                );
                assert_eq!(
                    session.view.reveal,
                    expected != choice,
                    "{from:?} a {target:?} con {choice:?}: reveal"
                );
            }
        }
    }
}

#[test]
fn row_7_converting_discards_the_rotation_range_and_cancels_the_pending_removal() {
    // Rango de rotación: RGBA → Indexado (el rango no puede sobrevivir al cambio de paleta).
    let mut session = session_of(image_of(Mode::Rgba));
    session.ops.set_range_end(3);
    assert_eq!(session.ops.range_end(), Some(3));
    assert_eq!(session.convert_mode(Mode::Indexed, ALL), Ok(true));
    assert_eq!(session.ops.range_end(), None, "rango fuera");

    // Quita pendiente: se cancela (el documento cambió) y se avisa una sola vez.
    let mut session = session_of(image_of(Mode::Indexed));
    session.ops.begin_removal(3, Some(1));
    assert!(session.ops.removal().is_some());
    assert!(!session.take_removal_cancelled());
    assert_eq!(session.convert_mode(Mode::Rgba, ALL), Ok(true));
    assert!(session.ops.removal().is_none(), "la quita se canceló");
    assert!(session.take_removal_cancelled(), "se avisa");
    assert!(!session.take_removal_cancelled(), "una sola vez");
}

#[test]
fn row_7_the_same_mode_gives_ok_false_without_a_step_or_changes() {
    for mode in TEST_MODES {
        let mut session = session_of(image_of(mode));
        // Con historia y con un paso por rehacer: el mismo modo no borra la pila de rehacer.
        edits_pixel(&mut session, 0, 0, 77);
        session.undo().expect("deshacer");
        session.view.reveal = false;
        let before = snapshot(&session);
        let steps_2 = steps_of_undo(&session.document);
        assert!(session.document.can_redo());
        for allowed in [Allowed::NOTHING, ALL] {
            assert_eq!(session.convert_mode(mode, allowed), Ok(false), "{mode:?}");
            assert_eq!(snapshot(&session), before, "{mode:?}");
            assert_eq!(steps_of_undo(&session.document), steps_2, "{mode:?}");
        }
    }
}

#[test]
fn row_7_undo_leaves_image_mode_and_palette_equal_and_redo_the_converted() {
    for from in TEST_MODES {
        for target in targets_2(from) {
            let context = format!("{from:?} a {target:?}");
            let original = image_of(from);
            let mut session = session_of(original.clone());
            let h0 = session.document.history_bytes();
            let s0 = session.document.state_id();
            assert_eq!(session.convert_mode(target, ALL), Ok(true), "{context}");
            let converted_2 = image_current(&session).clone();
            let h1 = session.document.history_bytes();
            let s1 = session.document.state_id();
            let choice_converted = session.choice;

            session.undo().expect("deshacer");
            assert_eq!(image_current(&session), &original, "{context}: undo");
            assert_eq!(mode_current(&session), from, "{context}: modo");
            assert_eq!(
                image_current(&session).palette(),
                original.palette(),
                "{context}: paleta"
            );
            assert_eq!(session.document.state_id(), s0, "{context}");
            assert!(!session.document.can_undo(), "{context}");
            assert!(session.document.can_redo(), "{context}");
            assert_eq!(session.document.history_bytes(), h0, "{context}: tras undo");
            // La elección se vuelve a normalizar; no se «recuerda» la anterior.
            assert_eq!(
                session.choice,
                choice_expected(&original, choice_converted),
                "{context}: elección tras undo"
            );
            assert!(
                pixel_for_choice(image_current(&session), session.choice).is_some(),
                "{context}"
            );
            if from == Mode::Indexed {
                assert_eq!(session.choice, ColorChoice::Entry(0), "{context}");
            }

            session.redo().expect("rehacer");
            assert_eq!(image_current(&session), &converted_2, "{context}: redo");
            assert_eq!(session.document.state_id(), s1, "{context}");
            assert_eq!(session.document.history_bytes(), h1, "{context}: tras redo");
            assert!(session.document.can_undo());
            assert!(!session.document.can_redo());
            assert!(
                pixel_for_choice(image_current(&session), session.choice).is_some(),
                "{context}"
            );
        }
    }
}

#[test]
fn row_7_after_undo_with_a_non_indexed_source_the_slot_is_1_even_if_there_was_another() {
    let mut session = session_of(image_of(Mode::Rgba));
    session.choice = ColorChoice::Slot(3);
    session.convert_mode(Mode::Indexed, ALL).expect("convertir");
    assert_eq!(session.choice, ColorChoice::Entry(2));
    session.undo().expect("deshacer");
    assert_eq!(
        session.choice,
        ColorChoice::Slot(1),
        "no recuerda la Slot(3)"
    );
    session.redo().expect("rehacer");
    assert_eq!(session.choice, ColorChoice::Entry(0), "Slot(1) → Entry(0)");
}

#[test]
fn row_7_two_conversions_are_two_steps_and_are_undone_in_order() {
    let original = image_of(Mode::Rgba);
    let mut session = session_of(original.clone());
    assert_eq!(session.convert_mode(Mode::GrayAlpha, ALL), Ok(true));
    let a_gray_alpha = image_current(&session).clone();
    assert_eq!(session.convert_mode(Mode::Rgba, ALL), Ok(true));
    assert_eq!(steps_of_undo(&session.document), 2);
    session.undo().expect("deshacer");
    assert_eq!(image_current(&session), &a_gray_alpha);
    session.undo().expect("deshacer");
    assert_eq!(image_current(&session), &original);
}

// ---------------------------------------------------------------------------------------
// Fila 8: fallos, reserva e historia.
// ---------------------------------------------------------------------------------------

/// 257×1 con 256 colores y el transparente: más de 256 entradas para indexado.
fn rgba_of_257_colors() -> Image {
    let mut colors: Vec<Rgba> = (0..256u32)
        .map(|i| Rgba::opaque((i % 256) as u8, (i / 256) as u8, 0))
        .collect();
    colors.push(Rgba::TRANSPARENT);
    rgba_de(&colors)
}

/// Sesión con historia (3 ediciones) y elección no inicial.
fn session_with_history(image: Image) -> Session {
    let mut session = session_of(image);
    for i in 0..3u8 {
        edits_pixel(&mut session, u32::from(i), 0, 100 + i);
    }
    session.choice = choice_expected(image_current(&session), ColorChoice::Slot(3));
    session.view.reveal = false;
    session
}

#[test]
fn row_8_loss_not_allowed_leaves_everything_intact() {
    let mut session = session_with_history(image_of(Mode::Rgba));
    let before = snapshot(&session);
    let steps_2 = steps_of_undo(&session.document);
    for allowed in [
        Allowed::NOTHING,
        Allowed {
            luma: true,
            drop_alpha: None,
        },
        Allowed {
            luma: false,
            drop_alpha: Some([255, 255, 255]),
        },
    ] {
        let result = session.convert_mode(Mode::Gray, allowed);
        assert!(
            matches!(
                result,
                Err(DocumentError::Convert(ConvertError::LossNotAllowed { .. }))
            ),
            "{allowed:?}: {result:?}"
        );
        assert_eq!(snapshot(&session), before, "{allowed:?}");
        assert_eq!(steps_of_undo(&session.document), steps_2);
    }
    // El informe lleva los contadores completos.
    match session.convert_mode(Mode::Gray, Allowed::NOTHING) {
        Err(DocumentError::Convert(ConvertError::LossNotAllowed { report })) => {
            // Las tres ediciones dejaron opacos los tres primeros píxeles: color en los cuatro y
            // alfa en el último.
            assert_eq!((report.color_lost, report.alpha_lost), (4, 1));
            assert_eq!(Analysis::Lossy(report), analyze(&before.image, Mode::Gray));
        }
        other_2 => panic!("{other_2:?}"),
    }
}

#[test]
fn row_8_too_many_colors_leaves_everything_intact() {
    let mut session = session_with_history(rgba_of_257_colors());
    let before = snapshot(&session);
    assert_eq!(
        session.convert_mode(Mode::Indexed, ALL),
        Err(DocumentError::Convert(ConvertError::TooManyColors {
            at_least: 257
        }))
    );
    assert_eq!(snapshot(&session), before);
}

#[test]
fn row_8_the_current_mode_gives_ok_false_and_leaves_everything_intact() {
    for mode in TEST_MODES {
        let mut session = session_with_history(image_of(mode));
        let before = snapshot(&session);
        assert_eq!(session.convert_mode(mode, ALL), Ok(false), "{mode:?}");
        assert_eq!(snapshot(&session), before, "{mode:?}");
    }
}

#[test]
fn row_8_an_error_changes_neither_the_redo_stack_nor_the_modified_mark() {
    let mut session = session_of(image_of(Mode::Rgba));
    edits_pixel(&mut session, 0, 0, 9);
    session.undo().expect("deshacer");
    assert!(session.document.can_redo());
    assert!(!session.modified());
    let before = snapshot(&session);
    assert!(session.convert_mode(Mode::Gray, Allowed::NOTHING).is_err());
    assert_eq!(snapshot(&session), before);
    assert!(session.document.can_redo(), "el error no borra el rehacer");
    assert!(!session.modified());
}

// --- reserve_for_convert ---

/// Un `ImageAlloc` falso: anota los `probe` y las imágenes pedidas, y falla donde se le dice.
struct False {
    probe_ok: bool,
    probes: Vec<usize>,
    images: usize,
}

impl False {
    fn new_2(probe_ok: bool) -> False {
        False {
            probe_ok,
            probes: Vec::new(),
            images: 0,
        }
    }
}

impl ImageAlloc for False {
    fn image(&mut self, spec: &NewImageSpec) -> Result<Image, ImageError> {
        self.images += 1;
        Err(ImageError::TooLarge {
            width: spec.width,
            height: spec.height,
        })
    }

    fn probe(&mut self, bytes: usize) -> bool {
        self.probes.push(bytes);
        self.probe_ok
    }
}

fn bytes_expected(image: &Image, target: Mode) -> u64 {
    let target_2 = target_byte_len(image.width(), image.height(), target).expect("cabe") as u64;
    target_2 + 4 * u64::from(image.width()) * u64::from(image.height())
}

#[test]
fn row_8_reserve_for_convert_fails_with_memory_of_target_byte_len_plus_4_n_without_calling_anything_else()
 {
    for from in TEST_MODES {
        for target in targets_2(from) {
            let image = image_of(from);
            let mut false_value = False::new_2(false);
            let result = reserve_for_convert(&image, target, &mut false_value);
            let expected_2_2_2 = bytes_expected(&image, target);
            match result {
                Err(ConvertModeError::Memory { bytes }) => {
                    assert_eq!(bytes, expected_2_2_2, "{from:?} a {target:?}");
                }
                Ok(()) => panic!("{from:?} a {target:?}: debía fallar"),
            }
            assert_eq!(
                false_value.probes,
                [usize::try_from(expected_2_2_2).expect("cabe")],
                "{from:?} a {target:?}: un solo probe con los bytes"
            );
            assert_eq!(
                false_value.images, 0,
                "{from:?} a {target:?}: no se pide ninguna imagen"
            );
        }
    }
}

#[test]
fn row_8_reserve_for_convert_with_probe_ok_gives_ok_with_a_single_probe() {
    for from in TEST_MODES {
        for target in targets_2(from) {
            let image = image_of(from);
            let mut false_value = False::new_2(true);
            assert!(
                reserve_for_convert(&image, target, &mut false_value).is_ok(),
                "{from:?} a {target:?}"
            );
            assert_eq!(false_value.probes.len(), 1, "{from:?} a {target:?}");
            assert_eq!(false_value.images, 0);
        }
    }
}

#[test]
fn row_8_reserve_for_convert_with_the_real_one_gives_ok() {
    for from in TEST_MODES {
        for target in targets_2(from) {
            assert!(
                reserve_for_convert(&image_of(from), target, &mut SystemAlloc).is_ok(),
                "{from:?} a {target:?}"
            );
        }
    }
}

#[test]
fn row_8_the_out_of_memory_notice_says_the_size_the_mode_and_the_mib() {
    // 8192² a RGBA: 256 MiB del destino y 256 MiB de la subida: 512 MiB exactos.
    let bytes = 512 * MIB as u64;
    assert_eq!(
        convert_memory_notice(8192, 8192, Mode::Rgba, bytes),
        "sin memoria para convertir 8192×8192 a RGBA (~512 MiB)"
    );
    for (target, mode_2) in [
        (Mode::Indexed, "indexado"),
        (Mode::Rgba, "RGBA"),
        (Mode::GrayAlpha, "gris con alfa"),
        (Mode::Gray, "gris"),
    ] {
        let text_2 = convert_memory_notice(3, 2, target, 7 * MIB as u64);
        assert_eq!(
            text_2,
            format!("sin memoria para convertir 3×2 a {mode_2} (~7 MiB)")
        );
    }
    // Con un tamaño que no es múltiplo de MiB, la cifra es la de redondear a un lado o al otro.
    let text_2 = convert_memory_notice(10, 10, Mode::Gray, 512 * MIB as u64 + 1);
    assert!(
        text_2 == "sin memoria para convertir 10×10 a gris (~512 MiB)"
            || text_2 == "sin memoria para convertir 10×10 a gris (~513 MiB)",
        "{text_2}"
    );
    // Con muy pocos bytes, al menos 1 MiB (nunca «~0 MiB»).
    let small = convert_memory_notice(1, 1, Mode::Gray, 5);
    assert!(
        small.starts_with("sin memoria para convertir 1×1 a gris (~"),
        "{small}"
    );
    assert!(small.ends_with(" MiB)"), "{small}");
    assert!(!small.contains("~0 MiB"), "{small}");
}

#[test]
fn row_8_the_success_notice_says_the_mode_and_only_the_losses_greater_than_zero() {
    // Tres rojos a gris con alfa: pierde el color en 3 píxeles (el ejemplo de la spec).
    let reds = rgba_de(&[Rgba::opaque(255, 0, 0); 3]);
    let analysis_2 = analyze(&reds, Mode::GrayAlpha);
    assert_eq!(
        describe_converted(Mode::GrayAlpha, Some(&analysis_2)),
        "imagen convertida a gris con alfa · perdido: color 3 px"
    );
    // Sin pérdida, solo la primera parte.
    let exact = analyze(&image_of(Mode::Gray), Mode::Rgba);
    assert_eq!(
        describe_converted(Mode::Rgba, Some(&exact)),
        "imagen convertida a RGBA"
    );
    // Con alfa y color.
    let with_the_two = analyze(&image_of(Mode::Rgba), Mode::Gray);
    assert_eq!(
        describe_converted(Mode::Gray, Some(&with_the_two)),
        "imagen convertida a gris · perdido: color 3 px, alfa 2 px"
    );
    // Y el informe vigente es el que pasó por `convert_mode`.
    let mut session = session_of(reds);
    assert_eq!(
        session.convert_mode(
            Mode::GrayAlpha,
            Allowed {
                luma: true,
                drop_alpha: None
            }
        ),
        Ok(true)
    );
    assert!(matches!(analysis_2, Analysis::Lossy(_)));
}

// --- Historia ---

/// Gris de 64 × 64 con `n` ediciones de un píxel cada una.
fn gray_with_edits(n: usize) -> Session {
    let mut session = session_of(Image::new(64, 64, Mode::Gray).expect("imagen"));
    for i in 0..n {
        edits_pixel(
            &mut session,
            (i % 64) as u32,
            (i / 64) as u32,
            (i % 250) as u8 + 1,
        );
    }
    session.view.reveal = false;
    session
}

#[test]
fn row_8_the_conversion_step_is_kept_even_if_the_limit_is_0_and_the_previous_ones_are_ejected() {
    let mut session = session_of(image_of(Mode::Rgba));
    for i in 0..3u8 {
        edits_pixel(&mut session, u32::from(i), 0, 50 + i);
    }
    session.document.set_history_limit(0);
    assert!(
        session.document.can_undo(),
        "el último paso nunca se descarta"
    );
    assert_eq!(steps_of_undo(&session.document), 1);
    let before_of_convert = image_current(&session).clone();

    assert_eq!(session.convert_mode(Mode::Gray, ALL), Ok(true));
    let converted_2 = image_current(&session).clone();
    assert_eq!(converted_2.mode(), Mode::Gray);

    // Entre los 16 bytes de píxeles de la imagen vieja (4×1 RGBA) y esos más 2048.
    let h = session.document.history_bytes();
    assert!((16..=16 + 2048).contains(&h), "history_bytes = {h}");
    assert_eq!(
        steps_of_undo(&session.document),
        1,
        "lo anterior se expulsó: solo queda la conversión"
    );

    // `undo` llega a la conversión y no más.
    session.undo().expect("deshacer la conversión");
    assert_eq!(image_current(&session), &before_of_convert);
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
    session.redo().expect("rehacer");
    assert_eq!(image_current(&session), &converted_2);
}

#[test]
fn row_8_with_limit_0_after_another_edit_only_that_one_remains() {
    let mut session = session_of(image_of(Mode::Rgba));
    for i in 0..3u8 {
        edits_pixel(&mut session, u32::from(i), 0, 50 + i);
    }
    session.document.set_history_limit(0);
    assert_eq!(session.convert_mode(Mode::Gray, ALL), Ok(true));
    let converted_2 = image_current(&session).clone();

    edits_pixel(&mut session, 0, 0, 200);
    assert_eq!(
        steps_of_undo(&session.document),
        1,
        "la edición expulsa la conversión"
    );
    session.undo().expect("deshacer la edición");
    assert_eq!(
        image_current(&session),
        &converted_2,
        "vuelve a la Gray de la conversión"
    );
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
    session.redo().expect("rehacer la edición");
    assert_eq!(mode_current(&session), Mode::Gray);
}

#[test]
fn row_8_with_the_default_limit_and_a_small_image_the_previous_steps_stay() {
    let original = image_of(Mode::Rgba);
    let mut session = session_of(original.clone());
    for i in 0..3u8 {
        edits_pixel(&mut session, u32::from(i), 0, 50 + i);
    }
    assert_eq!(
        session.document.history_limit(),
        Document::DEFAULT_HISTORY_LIMIT
    );
    assert_eq!(session.convert_mode(Mode::Gray, ALL), Ok(true));
    assert_eq!(
        steps_of_undo(&session.document),
        4,
        "las 3 ediciones y la conversión"
    );
    for _ in 0..4 {
        session.undo().expect("deshacer");
    }
    assert_eq!(
        image_current(&session),
        &original,
        "undo ×4 vuelve al original"
    );
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
}

#[test]
fn row_8_the_first_history_notice_matches_what_happens_c_greater_than_the_limit() {
    // Gris 64×64: P = 4096 bytes de píxeles; un paso cuesta de P a P + 2048.
    let pixels_2 = 64 * 64;
    let mut session = gray_with_edits(100);
    let limit_2 = pixels_2 - 1;
    session.document.set_history_limit(limit_2);
    let used_2 = session.document.history_bytes();
    let before = image_current(&session).clone();
    assert_eq!(
        history_warning(
            &before,
            History {
                limit: limit_2,
                used: used_2
            }
        )
        .as_deref(),
        Some(NOTICE_HISTORY_ALL)
    );

    assert_eq!(session.convert_mode(Mode::Rgba, ALL), Ok(true));
    assert!(
        session.document.history_bytes() > limit_2,
        "el paso supera el límite y se conserva"
    );
    assert_eq!(
        steps_of_undo(&session.document),
        1,
        "expulsa todo lo anterior"
    );
    let converted_2 = image_current(&session).clone();

    // «La siguiente edición lo expulsa a él.»
    edits_pixel(&mut session, 0, 0, 7);
    assert_eq!(steps_of_undo(&session.document), 1);
    session.undo().expect("deshacer la edición");
    assert_eq!(image_current(&session), &converted_2);
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
}

#[test]
fn row_8_the_second_history_notice_matches_what_happens_the_oldest_are_ejected() {
    let pixels_2 = 64 * 64;
    let mut session = gray_with_edits(100);
    let limit_2 = pixels_2 + 2560; // c = P + 2048 ≤ límite < 2c
    session.document.set_history_limit(limit_2);
    let used_2 = session.document.history_bytes();
    assert!(
        used_2 > 2560 && used_2 <= limit_2,
        "la prueba necesita una historia que no quepa junto al paso nuevo: usado = {used_2}"
    );
    let before = image_current(&session).clone();
    assert_eq!(
        history_warning(
            &before,
            History {
                limit: limit_2,
                used: used_2
            }
        )
        .as_deref(),
        Some(NOTICE_HISTORY_PART)
    );
    let steps_before = steps_of_undo(&session.document);

    assert_eq!(session.convert_mode(Mode::Rgba, ALL), Ok(true));
    let steps_after = steps_of_undo(&session.document);
    assert!(
        (1..=steps_before).contains(&steps_after),
        "se expulsan pasos antiguos: antes {steps_before}, después {steps_after}"
    );
    assert!(session.document.history_bytes() <= limit_2, "ahora cabe");
    // La conversión se conserva: el último `undo` posible vuelve a la imagen de antes.
    session.undo().expect("deshacer la conversión");
    assert_eq!(image_current(&session), &before);
}

#[test]
fn row_8_without_a_history_notice_nothing_is_ejected() {
    let pixels_2 = 64 * 64;
    // Por defecto: c ≪ límite/2.
    let mut session = gray_with_edits(20);
    let used_2 = session.document.history_bytes();
    let before = image_current(&session).clone();
    assert_eq!(
        history_warning(
            &before,
            History {
                limit: session.document.history_limit(),
                used: used_2
            }
        ),
        None
    );
    let steps_before = steps_of_undo(&session.document);
    assert_eq!(session.convert_mode(Mode::Rgba, ALL), Ok(true));
    assert_eq!(steps_of_undo(&session.document), steps_before + 1);

    // Con límite/2 < c ≤ límite pero h + c ≤ límite: tampoco hay aviso ni se expulsa nada.
    let mut session = gray_with_edits(3);
    let limit_2 = pixels_2 + 2560;
    session.document.set_history_limit(limit_2);
    let used_2 = session.document.history_bytes();
    let before = image_current(&session).clone();
    assert_eq!(
        history_warning(
            &before,
            History {
                limit: limit_2,
                used: used_2
            }
        ),
        None
    );
    let steps_before = steps_of_undo(&session.document);
    assert_eq!(session.convert_mode(Mode::Rgba, ALL), Ok(true));
    assert_eq!(steps_of_undo(&session.document), steps_before + 1);
    assert!(session.document.history_bytes() <= limit_2);
}

#[test]
fn row_8_the_margin_of_the_spec_covers_the_real_cost_of_a_conversion_step() {
    // El aviso supone que un paso cuesta como mucho P + `HISTORY_MARGIN`.
    for from in TEST_MODES {
        for target in targets_2(from) {
            let original = image_of(from);
            let mut session = session_of(original.clone());
            session.convert_mode(target, ALL).expect("convertir");
            assert!(
                session.document.history_bytes() as u64
                    <= bytes_of_pixels(&original) as u64 + HISTORY_MARGIN,
                "{from:?} a {target:?}"
            );
        }
    }
}

#[test]
#[ignore = "reserva 64 MiB de RGBA y 16 MiB de gris: cargo test -p px-app -- --ignored"]
fn row_8_rgba_of_4096_with_the_default_limit_convert_undo_another_edit_undo() {
    let original = Image::new(4096, 4096, Mode::Rgba).expect("RGBA 4096²");
    let mut session = session_of(original);
    let limit_2 = session.document.history_limit();
    let before = image_current(&session).clone();
    assert_eq!(
        history_warning(
            &before,
            History {
                limit: limit_2,
                used: 0
            }
        )
        .as_deref(),
        Some(NOTICE_HISTORY_ALL),
        "RGBA de 4096² con el límite por defecto: el primer aviso"
    );
    drop(before);

    let allowed = Allowed {
        luma: true,
        drop_alpha: Some([255, 255, 255]),
    };
    // El RGBA de ceros es transparente: pierde el alfa en todos los píxeles.
    assert_eq!(session.convert_mode(Mode::Gray, allowed), Ok(true));
    assert!(
        session.document.history_bytes() > limit_2,
        "el paso supera el límite"
    );
    assert_eq!(steps_of_undo(&session.document), 1);

    session.undo().expect("deshacer la conversión");
    assert_eq!(mode_current(&session), Mode::Rgba);
    edits_pixel(&mut session, 0, 0, 9);
    session.undo().expect("deshacer la edición");
    assert_eq!(mode_current(&session), Mode::Rgba);
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: aplicar es atómico y reversible (modelo de instantáneas).
// ---------------------------------------------------------------------------------------

const VALUES: [u8; 5] = [0, 1, 127, 128, 255];

#[derive(Clone, Debug)]
struct Seed {
    mode_2: usize,
    width: u32,
    height: u32,
    pixels_2: Vec<[usize; 4]>,
    colors: Vec<[usize; 4]>,
    n: usize,
    transparent: usize,
}

fn strategy_of_seed() -> impl Strategy<Value = Seed> {
    (
        0usize..4,
        1u32..=6,
        1u32..=6,
        prop::collection::vec(prop::array::uniform4(0usize..5), 36),
        prop::collection::vec(prop::array::uniform4(0usize..5), 6),
        1usize..=6,
        0usize..=6,
    )
        .prop_map(
            |(mode_2, width, height, pixels_2, colors, n, transparent)| Seed {
                mode_2,
                width,
                height,
                pixels_2,
                colors,
                n,
                transparent,
            },
        )
}

fn rgba_de_indices(v: &[usize; 4]) -> Rgba {
    Rgba::new(VALUES[v[0]], VALUES[v[1]], VALUES[v[2]], VALUES[v[3]])
}

fn image_of_seed(s: &Seed) -> Image {
    let (w, h) = (s.width, s.height);
    let total = (w * h) as usize;
    let mode = TEST_MODES[s.mode_2];
    let mut image = match mode {
        Mode::Indexed => {
            let colors: Vec<Rgba> = s.colors.iter().take(s.n).map(rgba_de_indices).collect();
            let mut palette = Palette::from_colors(colors).expect("paleta");
            if s.transparent < s.n {
                palette
                    .set_transparent_index(Some(s.transparent as u8))
                    .expect("índice transparente");
            }
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        other_2 => Image::new(w, h, other_2).expect("imagen"),
    };
    for (i, v) in s.pixels_2.iter().take(total).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let pixel = match mode {
            Mode::Rgba => Pixel::Rgba(rgba_de_indices(v)),
            Mode::Gray => Pixel::Gray(VALUES[v[0]]),
            Mode::GrayAlpha => Pixel::GrayAlpha {
                gray: VALUES[v[0]],
                alpha: VALUES[v[3]],
            },
            Mode::Indexed => Pixel::Indexed((v[0] % s.n) as u8),
        };
        image.set_pixel(x, y, pixel).expect("píxel");
    }
    image
}

#[derive(Clone, Debug)]
enum Op {
    /// `target_2` 0 a 2: `targets_2(actual)[target_2]`; 3: el propio modo actual.
    Convert {
        target_2: usize,
        luma: bool,
        background_2: usize,
    },
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0usize..4, any::<bool>(), 0usize..3).prop_map(|(target_2, luma, background_2)| Op::Convert {
            target_2,
            luma,
            background_2,
        }),
        1 => Just(Op::Undo),
        1 => Just(Op::Redo),
    ]
}

fn allowed_de(luma: bool, background_2: usize) -> Allowed {
    Allowed {
        luma,
        drop_alpha: match background_2 {
            0 => None,
            1 => Some([255, 255, 255]),
            _ => Some([0, 0, 0]),
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para documentos aleatorios (hasta 6 × 6) y secuencias de 0 a 12 conversiones válidas e
    /// inválidas con `undo` y `redo` intercalados, contra un modelo de instantáneas: tras cada
    /// operación el documento es igual a la instantánea del cursor (modo y paleta incluidos), un
    /// `Err` o un `Ok(false)` dejan documento, historia, `state_id` y elección **iguales**, y la
    /// elección es siempre válida.
    #[test]
    fn property_3_applying_is_atomic_and_reversible(
        seed in strategy_of_seed(),
        choice in 0usize..9,
        ops in prop::collection::vec(op(), 0..=12),
    ) {
        let initial = image_of_seed(&seed);
        let mut session = session_of(initial.clone());
        session.choice = match initial.palette() {
            Some(p) => ColorChoice::Entry((choice % p.len()) as u8),
            None => ColorChoice::Slot(choice as u8 + 1),
        };
        prop_assert!(pixel_for_choice(&initial, session.choice).is_some());

        let mut snapshots = vec![initial];
        let mut cursor = 0usize;

        for (i, operation) in ops.iter().enumerate() {
            let context = format!("paso {i}: {operation:?}");
            let before = snapshot(&session);
            let mut changes = false;
            match operation {
                Op::Convert { target_2, luma, background_2 } => {
                    let actual = snapshots[cursor].clone();
                    let target = if *target_2 == 3 {
                        actual.mode()
                    } else {
                        targets_2(actual.mode())[*target_2]
                    };
                    let allowed = allowed_de(*luma, *background_2);
                    let result = session.convert_mode(target, allowed);
                    if target == actual.mode() {
                        prop_assert_eq!(result, Ok(false), "{}", context);
                    } else {
                        match convert_image(&actual, target, allowed) {
                            Ok(conversion) => {
                                prop_assert_eq!(result, Ok(true), "{}", context);
                                snapshots.truncate(cursor + 1);
                                snapshots.push(conversion.image);
                                cursor += 1;
                                changes = true;
                            }
                            Err(error) => {
                                prop_assert_eq!(
                                    result,
                                    Err(DocumentError::Convert(error)),
                                    "{}",
                                    context
                                );
                            }
                        }
                    }
                }
                Op::Undo => {
                    let result = session.undo();
                    if cursor > 0 {
                        prop_assert_eq!(result, Ok(()), "{}", context);
                        cursor -= 1;
                        changes = true;
                    } else {
                        prop_assert_eq!(result, Err(DocumentError::NothingToUndo), "{}", context);
                    }
                }
                Op::Redo => {
                    let result = session.redo();
                    if cursor + 1 < snapshots.len() {
                        prop_assert_eq!(result, Ok(()), "{}", context);
                        cursor += 1;
                        changes = true;
                    } else {
                        prop_assert_eq!(result, Err(DocumentError::NothingToRedo), "{}", context);
                    }
                }
            }

            // El documento es la instantánea del cursor, con su modo y su paleta.
            let image_2 = image_current(&session);
            prop_assert_eq!(image_2, &snapshots[cursor], "{}", context);
            prop_assert_eq!(image_2.mode(), snapshots[cursor].mode(), "{}", context);
            prop_assert_eq!(image_2.palette(), snapshots[cursor].palette(), "{}", context);
            prop_assert_eq!(session.document.can_undo(), cursor > 0, "{}", context);
            prop_assert_eq!(
                session.document.can_redo(),
                cursor + 1 < snapshots.len(),
                "{}",
                context
            );
            // La elección es siempre válida.
            prop_assert!(
                pixel_for_choice(image_2, session.choice).is_some(),
                "{}: la elección {:?} no vale",
                context,
                session.choice
            );
            // Un `Err` o un `Ok(false)` dejan todo igual.
            if !changes && !matches!(operation, Op::Undo | Op::Redo) {
                prop_assert_eq!(snapshot(&session), before.clone(), "{}", context);
            }
            if !changes {
                prop_assert_eq!(session.document.state_id(), before.state_id, "{}", context);
                prop_assert_eq!(session.document.history_bytes(), before.history_bytes, "{}", context);
                prop_assert_eq!(session.choice, before.choice, "{}", context);
                prop_assert_eq!(session.modified(), before.modified, "{}", context);
            }
        }
    }
}
