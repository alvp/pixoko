//! Tests de `Session::cycle_choice` (specs/interfaz-teclado.md, unidad I.1, fila 7): recorrer los
//! colores con `]` y `[` (la acción `ColorNext`/`ColorPrev` de `dispatch` la usa con `+1` y `-1`).
//! En ROJO hasta la lógica: `Session::cycle_choice` no existe todavía.
//!
//! - Indexada de 256 entradas con `Entry(255)` y `Entry(0)`: dan la vuelta en los dos sentidos.
//! - RGBA con `Slot(9)` y `Slot(1)`; Gray y GrayAlpha: las nueve ranuras.
//! - Un paso por llamada; el panel muestra la fila elegida (`view.reveal`) si la elección cambió; el
//!   documento, `state_id`, `can_undo` y `can_redo` no cambian.
//! - Una `choice` que ya no vale (otro modo, entrada fuera de la paleta): `initial_choice`.
//! - Paleta de una sola entrada: se queda donde está.
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - `cycle_choice(delta: i8)` suma `delta` módulo el número de muestras (`cell_count`): `+1` es la
//!   siguiente y `-1` la anterior; el módulo para otros valores se prueba con `±2` y `±9`, que la
//!   spec no pide explícitamente pero se sigue de «un paso por pulsación».
//! - «Sin imagen» no se puede construir: un `Document` siempre tiene su imagen principal.
//! - Con una elección inválida, `reveal` no se comprueba (la spec solo fija `initial_choice`).
//! - Ni `ops` (la quita pendiente y el rango) ni el editor se comprueban aquí: son de `dispatch`.

use px_core::ImageId;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::color::{ColorChoice, initial_choice};

use super::Session;

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

fn different(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

fn indexed(n: usize) -> Image {
    let palette = Palette::from_colors(different(n)).expect("paleta");
    Image::new_indexed(4, 4, palette).expect("imagen indexada")
}

fn of_mode(mode: Mode) -> Image {
    Image::new(4, 4, mode).expect("imagen")
}

/// Sesión con un paso de historia hecho y otro deshecho (se puede deshacer y rehacer).
fn session_2(image: Image) -> Session {
    let mut document = Document::new(image);
    let id = ImageId::from_raw(0);
    for x in 0..2u32 {
        let image = document.image(id).expect("imagen").clone();
        let pixel = match image.mode() {
            Mode::Rgba => Pixel::Rgba(Rgba::opaque(10 + x as u8, 20, 30)),
            Mode::Indexed => {
                let len = image.palette().map_or(1, |p| p.len() as u32);
                Pixel::Indexed((x % len) as u8)
            }
            Mode::Gray => Pixel::Gray(10 + x as u8),
            Mode::GrayAlpha => Pixel::GrayAlpha {
                gray: 10 + x as u8,
                alpha: 255,
            },
        };
        // Si el píxel ya vale eso (la entrada 0 del principio), no hay paso: da igual.
        document
            .apply(Command::SetPixels {
                image: id,
                pixels: vec![PixelEdit { x, y: 0, pixel }],
            })
            .expect("comando válido");
    }
    let mut session = Session::sample(document);
    if session.document.can_undo() {
        session.undo().expect("deshacer un paso");
    }
    let _ = session.take_removal_cancelled();
    session.view.reveal = false;
    session
}

fn image_2(session: &Session) -> &Image {
    session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen principal")
}

/// Lo que `cycle_choice` no debe tocar nunca.
#[derive(Clone, Debug, PartialEq)]
struct TestDocument {
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    image: Image,
}

fn document(session: &Session) -> TestDocument {
    TestDocument {
        state_id: session.document.state_id(),
        can_undo: session.document.can_undo(),
        can_redo: session.document.can_redo(),
        image: image_2(session).clone(),
    }
}

/// Un paso con `delta`, comprobando elección, `reveal` y documento.
fn cycle(session: &mut Session, delta: i8, expected: ColorChoice, context: &str) {
    let before = document(session);
    let changes = session.choice != expected;
    session.view.reveal = false;
    session.cycle_choice(delta);
    assert_eq!(session.choice, expected, "{context}: delta {delta}");
    if changes {
        assert!(
            session.view.reveal,
            "{context}: delta {delta} no muestra la fila elegida"
        );
    }
    assert_eq!(document(session), before, "{context}: delta {delta}");
}

// ---------------------------------------------------------------------------------------------
// Indexada de 256
// ---------------------------------------------------------------------------------------------

#[test]
fn indexed_of_256_the_last_plus_one_gives_the_first() {
    let mut s = session_2(indexed(256));
    s.set_choice_revealing(ColorChoice::Entry(255));
    cycle(&mut s, 1, ColorChoice::Entry(0), "256");
}

#[test]
fn indexed_of_256_the_first_minus_one_gives_the_last() {
    let mut s = session_2(indexed(256));
    assert_eq!(s.choice, ColorChoice::Entry(0));
    cycle(&mut s, -1, ColorChoice::Entry(255), "256");
}

#[test]
fn indexed_of_256_one_step_per_call_in_both_directions() {
    let mut s = session_2(indexed(256));
    for input in 1..=20u8 {
        cycle(&mut s, 1, ColorChoice::Entry(input), "256 hacia delante");
    }
    for input in (0..20u8).rev() {
        cycle(&mut s, -1, ColorChoice::Entry(input), "256 hacia atrás");
    }
}

#[test]
fn indexed_of_256_a_whole_turn_goes_back_to_the_start() {
    let mut s = session_2(indexed(256));
    s.set_choice_revealing(ColorChoice::Entry(100));
    for _ in 0..256 {
        s.cycle_choice(1);
    }
    assert_eq!(s.choice, ColorChoice::Entry(100));
    for _ in 0..256 {
        s.cycle_choice(-1);
    }
    assert_eq!(s.choice, ColorChoice::Entry(100));
}

#[test]
fn indexed_of_9_wraps_around_at_both_ends() {
    let mut s = session_2(indexed(9));
    s.set_choice_revealing(ColorChoice::Entry(8));
    cycle(&mut s, 1, ColorChoice::Entry(0), "9 de la última");
    cycle(&mut s, -1, ColorChoice::Entry(8), "9 de la primera");
}

#[test]
fn indexed_of_a_single_entry_stays_where_it_is() {
    let mut s = session_2(indexed(1));
    for delta in [1, -1] {
        cycle(&mut s, delta, ColorChoice::Entry(0), "1 entrada");
    }
}

#[test]
fn indexed_of_two_entries_toggles() {
    let mut s = session_2(indexed(2));
    cycle(&mut s, 1, ColorChoice::Entry(1), "2 entradas");
    cycle(&mut s, 1, ColorChoice::Entry(0), "2 entradas");
    cycle(&mut s, -1, ColorChoice::Entry(1), "2 entradas");
}

// ---------------------------------------------------------------------------------------------
// RGBA, Gray y GrayAlpha: nueve ranuras
// ---------------------------------------------------------------------------------------------

#[test]
fn rgba_slot_9_plus_one_gives_slot_1_and_slot_1_minus_one_gives_slot_9() {
    let mut s = session_2(of_mode(Mode::Rgba));
    s.set_choice_revealing(ColorChoice::Slot(9));
    cycle(&mut s, 1, ColorChoice::Slot(1), "RGBA");
    cycle(&mut s, -1, ColorChoice::Slot(9), "RGBA");
}

#[test]
fn rgba_walks_the_nine_slots_one_by_one() {
    let mut s = session_2(of_mode(Mode::Rgba));
    assert_eq!(s.choice, ColorChoice::Slot(1));
    for slot in 2..=9u8 {
        cycle(&mut s, 1, ColorChoice::Slot(slot), "RGBA hacia delante");
    }
    cycle(&mut s, 1, ColorChoice::Slot(1), "RGBA de la última");
    // De la 1 hacia atrás da la 9 y luego baja de una en una.
    cycle(&mut s, -1, ColorChoice::Slot(9), "RGBA de la primera");
    for slot in (1..=8u8).rev() {
        cycle(&mut s, -1, ColorChoice::Slot(slot), "RGBA hacia atrás");
    }
}

#[test]
fn gray_and_gray_alpha_wrap_around_in_the_nine_slots() {
    for (name_2, mode) in [("Gray", Mode::Gray), ("GrayAlpha", Mode::GrayAlpha)] {
        let mut s = session_2(of_mode(mode));
        s.set_choice_revealing(ColorChoice::Slot(9));
        cycle(&mut s, 1, ColorChoice::Slot(1), name_2);
        cycle(&mut s, -1, ColorChoice::Slot(9), name_2);
        cycle(&mut s, -1, ColorChoice::Slot(8), name_2);
        cycle(&mut s, 1, ColorChoice::Slot(9), name_2);
    }
}

#[test]
fn in_modes_without_a_palette_there_are_always_nine_swatches() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let mut s = session_2(of_mode(mode));
        let mut views = vec![s.choice];
        for _ in 0..9 {
            s.cycle_choice(1);
            views.push(s.choice);
        }
        let expected_2_2: Vec<ColorChoice> =
            (0..=9u8).map(|i| ColorChoice::Slot(i % 9 + 1)).collect();
        assert_eq!(views, expected_2_2, "{mode:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Varios pasos, documento y reveal
// ---------------------------------------------------------------------------------------------

#[test]
fn delta_of_more_than_one_step_adds_modulo_the_number_of_swatches() {
    let mut s = session_2(indexed(9));
    s.set_choice_revealing(ColorChoice::Entry(7));
    cycle(&mut s, 2, ColorChoice::Entry(0), "9 con +2");
    cycle(&mut s, -2, ColorChoice::Entry(7), "9 con -2");
    cycle(&mut s, 9, ColorChoice::Entry(7), "9 con +9");
    cycle(&mut s, -9, ColorChoice::Entry(7), "9 con -9");
}

#[test]
fn cycle_choice_does_not_touch_the_document_or_the_history_in_any_mode() {
    let images = [
        indexed(9),
        indexed(256),
        of_mode(Mode::Rgba),
        of_mode(Mode::Gray),
        of_mode(Mode::GrayAlpha),
    ];
    for image in images {
        let mut s = session_2(image);
        let before = document(&s);
        assert!(before.can_redo, "la sesión de prueba puede rehacer");
        for _ in 0..30 {
            s.cycle_choice(1);
            s.cycle_choice(-1);
            s.cycle_choice(-1);
        }
        assert_eq!(document(&s), before, "{:?}", image_2(&s).mode());
    }
}

#[test]
fn the_chosen_row_is_shown_after_each_change() {
    for image in [indexed(9), of_mode(Mode::Rgba)] {
        let mut s = session_2(image);
        for _ in 0..12 {
            s.view.reveal = false;
            s.cycle_choice(1);
            assert!(s.view.reveal, "{:?}", s.choice);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Una choice que ya no vale
// ---------------------------------------------------------------------------------------------

#[test]
fn with_an_invalid_choice_cycle_choice_gives_the_initial_choice_in_both_directions() {
    let cases: Vec<(&str, Image, ColorChoice)> = vec![
        (
            "indexada con una ranura sin entrada",
            indexed(4),
            ColorChoice::Slot(9),
        ),
        (
            "indexada con una entrada fuera de la paleta",
            indexed(4),
            ColorChoice::Entry(200),
        ),
        (
            "indexada de 9 con la entrada 9",
            indexed(9),
            ColorChoice::Entry(9),
        ),
        (
            "RGBA con una entrada",
            of_mode(Mode::Rgba),
            ColorChoice::Entry(3),
        ),
        (
            "Gray con una entrada",
            of_mode(Mode::Gray),
            ColorChoice::Entry(0),
        ),
        (
            "GrayAlpha con una entrada",
            of_mode(Mode::GrayAlpha),
            ColorChoice::Entry(1),
        ),
        (
            "RGBA con la ranura 0",
            of_mode(Mode::Rgba),
            ColorChoice::Slot(0),
        ),
        (
            "RGBA con la ranura 10",
            of_mode(Mode::Rgba),
            ColorChoice::Slot(10),
        ),
    ];
    for (name_2, image, invalid) in cases {
        for delta in [1i8, -1] {
            let mut s = session_2(image.clone());
            let initial = initial_choice(image_2(&s));
            s.choice = invalid;
            let before = document(&s);
            s.cycle_choice(delta);
            assert_eq!(s.choice, initial, "{name_2}: delta {delta}");
            assert_eq!(document(&s), before, "{name_2}: delta {delta}");
        }
    }
}

#[test]
fn after_an_invalid_choice_the_next_step_is_already_normal() {
    let mut s = session_2(indexed(4));
    s.choice = ColorChoice::Entry(200);
    s.cycle_choice(1);
    assert_eq!(s.choice, ColorChoice::Entry(0));
    s.cycle_choice(1);
    assert_eq!(s.choice, ColorChoice::Entry(1));
}
