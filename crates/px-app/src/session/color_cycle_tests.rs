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

fn distintos(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

fn indexada(n: usize) -> Image {
    let paleta = Palette::from_colors(distintos(n)).expect("paleta");
    Image::new_indexed(4, 4, paleta).expect("imagen indexada")
}

fn de_modo(mode: Mode) -> Image {
    Image::new(4, 4, mode).expect("imagen")
}

/// Sesión con un paso de historia hecho y otro deshecho (se puede deshacer y rehacer).
fn sesion(image: Image) -> Session {
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

fn imagen(session: &Session) -> &Image {
    session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen principal")
}

/// Lo que `cycle_choice` no debe tocar nunca.
#[derive(Clone, Debug, PartialEq)]
struct Documento {
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    image: Image,
}

fn documento(session: &Session) -> Documento {
    Documento {
        state_id: session.document.state_id(),
        can_undo: session.document.can_undo(),
        can_redo: session.document.can_redo(),
        image: imagen(session).clone(),
    }
}

/// Un paso con `delta`, comprobando elección, `reveal` y documento.
fn ciclo(session: &mut Session, delta: i8, esperada: ColorChoice, contexto: &str) {
    let antes = documento(session);
    let cambia = session.choice != esperada;
    session.view.reveal = false;
    session.cycle_choice(delta);
    assert_eq!(session.choice, esperada, "{contexto}: delta {delta}");
    if cambia {
        assert!(
            session.view.reveal,
            "{contexto}: delta {delta} no muestra la fila elegida"
        );
    }
    assert_eq!(documento(session), antes, "{contexto}: delta {delta}");
}

// ---------------------------------------------------------------------------------------------
// Indexada de 256
// ---------------------------------------------------------------------------------------------

#[test]
fn indexada_de_256_la_ultima_mas_uno_da_la_primera() {
    let mut s = sesion(indexada(256));
    s.set_choice_revealing(ColorChoice::Entry(255));
    ciclo(&mut s, 1, ColorChoice::Entry(0), "256");
}

#[test]
fn indexada_de_256_la_primera_menos_uno_da_la_ultima() {
    let mut s = sesion(indexada(256));
    assert_eq!(s.choice, ColorChoice::Entry(0));
    ciclo(&mut s, -1, ColorChoice::Entry(255), "256");
}

#[test]
fn indexada_de_256_un_paso_por_llamada_en_los_dos_sentidos() {
    let mut s = sesion(indexada(256));
    for entrada in 1..=20u8 {
        ciclo(&mut s, 1, ColorChoice::Entry(entrada), "256 hacia delante");
    }
    for entrada in (0..20u8).rev() {
        ciclo(&mut s, -1, ColorChoice::Entry(entrada), "256 hacia atrás");
    }
}

#[test]
fn indexada_de_256_una_vuelta_entera_vuelve_al_principio() {
    let mut s = sesion(indexada(256));
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
fn indexada_de_9_da_la_vuelta_en_los_dos_extremos() {
    let mut s = sesion(indexada(9));
    s.set_choice_revealing(ColorChoice::Entry(8));
    ciclo(&mut s, 1, ColorChoice::Entry(0), "9 de la última");
    ciclo(&mut s, -1, ColorChoice::Entry(8), "9 de la primera");
}

#[test]
fn indexada_de_una_sola_entrada_se_queda_donde_esta() {
    let mut s = sesion(indexada(1));
    for delta in [1, -1] {
        ciclo(&mut s, delta, ColorChoice::Entry(0), "1 entrada");
    }
}

#[test]
fn indexada_de_dos_entradas_alterna() {
    let mut s = sesion(indexada(2));
    ciclo(&mut s, 1, ColorChoice::Entry(1), "2 entradas");
    ciclo(&mut s, 1, ColorChoice::Entry(0), "2 entradas");
    ciclo(&mut s, -1, ColorChoice::Entry(1), "2 entradas");
}

// ---------------------------------------------------------------------------------------------
// RGBA, Gray y GrayAlpha: nueve ranuras
// ---------------------------------------------------------------------------------------------

#[test]
fn rgba_slot_9_mas_uno_da_slot_1_y_slot_1_menos_uno_da_slot_9() {
    let mut s = sesion(de_modo(Mode::Rgba));
    s.set_choice_revealing(ColorChoice::Slot(9));
    ciclo(&mut s, 1, ColorChoice::Slot(1), "RGBA");
    ciclo(&mut s, -1, ColorChoice::Slot(9), "RGBA");
}

#[test]
fn rgba_recorre_las_nueve_ranuras_de_una_en_una() {
    let mut s = sesion(de_modo(Mode::Rgba));
    assert_eq!(s.choice, ColorChoice::Slot(1));
    for ranura in 2..=9u8 {
        ciclo(&mut s, 1, ColorChoice::Slot(ranura), "RGBA hacia delante");
    }
    ciclo(&mut s, 1, ColorChoice::Slot(1), "RGBA de la última");
    // De la 1 hacia atrás da la 9 y luego baja de una en una.
    ciclo(&mut s, -1, ColorChoice::Slot(9), "RGBA de la primera");
    for ranura in (1..=8u8).rev() {
        ciclo(&mut s, -1, ColorChoice::Slot(ranura), "RGBA hacia atrás");
    }
}

#[test]
fn gray_y_gray_alpha_dan_la_vuelta_en_las_nueve_ranuras() {
    for (nombre, mode) in [("Gray", Mode::Gray), ("GrayAlpha", Mode::GrayAlpha)] {
        let mut s = sesion(de_modo(mode));
        s.set_choice_revealing(ColorChoice::Slot(9));
        ciclo(&mut s, 1, ColorChoice::Slot(1), nombre);
        ciclo(&mut s, -1, ColorChoice::Slot(9), nombre);
        ciclo(&mut s, -1, ColorChoice::Slot(8), nombre);
        ciclo(&mut s, 1, ColorChoice::Slot(9), nombre);
    }
}

#[test]
fn en_los_modos_sin_paleta_siempre_hay_nueve_muestras() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let mut s = sesion(de_modo(mode));
        let mut vistas = vec![s.choice];
        for _ in 0..9 {
            s.cycle_choice(1);
            vistas.push(s.choice);
        }
        let esperadas: Vec<ColorChoice> = (0..=9u8).map(|i| ColorChoice::Slot(i % 9 + 1)).collect();
        assert_eq!(vistas, esperadas, "{mode:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Varios pasos, documento y reveal
// ---------------------------------------------------------------------------------------------

#[test]
fn delta_de_mas_de_un_paso_suma_modulo_el_numero_de_muestras() {
    let mut s = sesion(indexada(9));
    s.set_choice_revealing(ColorChoice::Entry(7));
    ciclo(&mut s, 2, ColorChoice::Entry(0), "9 con +2");
    ciclo(&mut s, -2, ColorChoice::Entry(7), "9 con -2");
    ciclo(&mut s, 9, ColorChoice::Entry(7), "9 con +9");
    ciclo(&mut s, -9, ColorChoice::Entry(7), "9 con -9");
}

#[test]
fn cycle_choice_no_toca_el_documento_ni_la_historia_en_ningun_modo() {
    let imagenes = [
        indexada(9),
        indexada(256),
        de_modo(Mode::Rgba),
        de_modo(Mode::Gray),
        de_modo(Mode::GrayAlpha),
    ];
    for image in imagenes {
        let mut s = sesion(image);
        let antes = documento(&s);
        assert!(antes.can_redo, "la sesión de prueba puede rehacer");
        for _ in 0..30 {
            s.cycle_choice(1);
            s.cycle_choice(-1);
            s.cycle_choice(-1);
        }
        assert_eq!(documento(&s), antes, "{:?}", imagen(&s).mode());
    }
}

#[test]
fn la_fila_elegida_se_muestra_tras_cada_cambio() {
    for image in [indexada(9), de_modo(Mode::Rgba)] {
        let mut s = sesion(image);
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
fn con_una_choice_invalida_cycle_choice_da_la_eleccion_inicial_en_los_dos_sentidos() {
    let casos: Vec<(&str, Image, ColorChoice)> = vec![
        (
            "indexada con una ranura sin entrada",
            indexada(4),
            ColorChoice::Slot(9),
        ),
        (
            "indexada con una entrada fuera de la paleta",
            indexada(4),
            ColorChoice::Entry(200),
        ),
        (
            "indexada de 9 con la entrada 9",
            indexada(9),
            ColorChoice::Entry(9),
        ),
        (
            "RGBA con una entrada",
            de_modo(Mode::Rgba),
            ColorChoice::Entry(3),
        ),
        (
            "Gray con una entrada",
            de_modo(Mode::Gray),
            ColorChoice::Entry(0),
        ),
        (
            "GrayAlpha con una entrada",
            de_modo(Mode::GrayAlpha),
            ColorChoice::Entry(1),
        ),
        (
            "RGBA con la ranura 0",
            de_modo(Mode::Rgba),
            ColorChoice::Slot(0),
        ),
        (
            "RGBA con la ranura 10",
            de_modo(Mode::Rgba),
            ColorChoice::Slot(10),
        ),
    ];
    for (nombre, image, invalida) in casos {
        for delta in [1i8, -1] {
            let mut s = sesion(image.clone());
            let inicial = initial_choice(imagen(&s));
            s.choice = invalida;
            let antes = documento(&s);
            s.cycle_choice(delta);
            assert_eq!(s.choice, inicial, "{nombre}: delta {delta}");
            assert_eq!(documento(&s), antes, "{nombre}: delta {delta}");
        }
    }
}

#[test]
fn tras_una_choice_invalida_el_siguiente_paso_ya_es_normal() {
    let mut s = sesion(indexada(4));
    s.choice = ColorChoice::Entry(200);
    s.cycle_choice(1);
    assert_eq!(s.choice, ColorChoice::Entry(0));
    s.cycle_choice(1);
    assert_eq!(s.choice, ColorChoice::Entry(1));
}
