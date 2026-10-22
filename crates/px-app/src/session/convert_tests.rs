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

const MODOS: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const MIB: usize = 1 << 20;

/// Todo permitido: el color y el fondo blanco para el alfa.
const TODO: Allowed = Allowed {
    luma: true,
    drop_alpha: Some([255, 255, 255]),
};

const AVISO_HISTORIA_TODO: &str = "aviso: convertir descarta el historial anterior; solo se podrá deshacer hasta tu próxima edición";
const AVISO_HISTORIA_PARTE: &str = "aviso: convertir descarta parte del historial anterior";

/// Los destinos de `from` en el orden de D2.
fn destinos(from: Mode) -> [Mode; 3] {
    let v: Vec<Mode> = MODOS.into_iter().filter(|m| *m != from).collect();
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

fn gris_de(values: &[u8]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::Gray).expect("imagen Gray");
    for (x, v) in values.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Gray(*v))
            .expect("píxel");
    }
    image
}

fn gris_alfa_de(values: &[(u8, u8)]) -> Image {
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

fn indexada_de(colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Image {
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
fn imagen_de(mode: Mode) -> Image {
    match mode {
        Mode::Rgba => rgba_de(&[
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 128),
            Rgba::opaque(7, 7, 7),
            Rgba::new(0, 0, 255, 128),
        ]),
        Mode::Gray => gris_de(&[0, 127, 255, 40]),
        Mode::GrayAlpha => gris_alfa_de(&[(10, 0), (10, 255), (200, 128), (30, 0)]),
        Mode::Indexed => indexada_de(
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

fn sesion_de(image: Image) -> Session {
    let mut session = Session::sample(Document::new(image));
    session.view.reveal = false;
    session
}

fn imagen_actual(session: &Session) -> &Image {
    session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen")
}

fn modo_actual(session: &Session) -> Mode {
    imagen_actual(session).mode()
}

fn bytes_de_pixeles(image: &Image) -> usize {
    Image::byte_len(image.width(), image.height(), image.mode()).expect("tamaño válido")
}

/// Cuántos pasos de deshacer tiene el documento (se cuentan en una copia).
fn pasos_de_deshacer(document: &Document) -> usize {
    let mut copia = document.clone();
    let mut pasos = 0;
    while copia.undo().is_ok() {
        pasos += 1;
    }
    pasos
}

/// La elección que debe quedar para una imagen (escrita aparte de `clamp_choice`): en indexadas,
/// `Entry(min(i, len − 1))` y `Slot(n)` pasa a `Entry(n − 1)`; en las demás, la ranura si vale y, si
/// no, la 1.
fn eleccion_esperada(image: &Image, choice: ColorChoice) -> ColorChoice {
    match image.palette() {
        Some(palette) => {
            let indice = match choice {
                ColorChoice::Entry(i) => usize::from(i),
                ColorChoice::Slot(n) => usize::from(n).saturating_sub(1),
            };
            ColorChoice::Entry(indice.min(palette.len() - 1) as u8)
        }
        None => match choice {
            ColorChoice::Slot(n) if (1..=9).contains(&n) => choice,
            _ => ColorChoice::Slot(1),
        },
    }
}

/// Todo lo observable que un `Err` o un `Ok(false)` no deben tocar.
#[derive(Clone, Debug, PartialEq)]
struct Foto {
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

fn foto(session: &Session) -> Foto {
    Foto {
        image: imagen_actual(session).clone(),
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
fn edita_pixel(session: &mut Session, x: u32, y: u32, i: u8) {
    let id = ImageId::from_raw(0);
    let mode = modo_actual(session);
    let pixel = match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::opaque(i, 1, 2)),
        Mode::Gray => Pixel::Gray(i),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: i,
            alpha: 255,
        },
        Mode::Indexed => Pixel::Indexed(i % 8),
    };
    let cambio = session
        .document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit { x, y, pixel }],
        })
        .expect("editar");
    assert!(cambio, "la edición {i} debe cambiar el píxel ({x}, {y})");
}

// ---------------------------------------------------------------------------------------
// Fila 7: aplicar.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_7_desde_cada_modo_a_cada_destino_es_ok_true_con_un_solo_paso_y_la_imagen_de_convert_image()
{
    for from in MODOS {
        for target in destinos(from) {
            let original = imagen_de(from);
            let mut session = sesion_de(original.clone());
            let contexto = format!("{from:?} a {target:?}");
            assert!(!session.document.can_undo(), "{contexto}");
            let h0 = session.document.history_bytes();
            let esperada = convert_image(&original, target, TODO)
                .expect("con todo permitido se puede")
                .image;

            assert_eq!(session.convert_mode(target, TODO), Ok(true), "{contexto}");

            assert_eq!(imagen_actual(&session), &esperada, "{contexto}");
            assert_eq!(modo_actual(&session), target, "{contexto}");
            assert!(session.document.can_undo(), "{contexto}");
            assert!(!session.document.can_redo(), "{contexto}");
            assert_eq!(
                pasos_de_deshacer(&session.document),
                1,
                "{contexto}: un solo paso"
            );
            let crecimiento = session.document.history_bytes() - h0;
            let pixeles = bytes_de_pixeles(&original);
            assert!(
                (pixeles..=pixeles + 2048).contains(&crecimiento),
                "{contexto}: la historia creció {crecimiento} y los píxeles anteriores son {pixeles}"
            );
        }
    }
}

#[test]
fn fila_7_el_estado_es_nuevo_y_el_documento_queda_modificado() {
    for from in MODOS {
        for target in destinos(from) {
            let mut session = sesion_de(imagen_de(from));
            assert!(!session.modified());
            let antes = session.document.state_id();
            assert_eq!(session.convert_mode(target, TODO), Ok(true));
            assert_ne!(session.document.state_id(), antes, "{from:?} a {target:?}");
            assert!(session.modified(), "{from:?} a {target:?}");
        }
    }
}

#[test]
fn fila_7_a_indexado_slot_3_pasa_a_entry_min_2_y_len_menos_1() {
    // Imágenes con 4 colores, con 3 y con 2 (el mínimo se aplica).
    for (nombre, image, esperada) in [
        (
            "RGBA de 4 colores",
            imagen_de(Mode::Rgba),
            ColorChoice::Entry(2),
        ),
        (
            "Gray de 4 colores",
            imagen_de(Mode::Gray),
            ColorChoice::Entry(2),
        ),
        (
            "GrayAlpha de 3 entradas (con el transparente)",
            imagen_de(Mode::GrayAlpha),
            ColorChoice::Entry(2),
        ),
        (
            "Gray de 2 colores",
            gris_de(&[5, 6, 5, 6]),
            ColorChoice::Entry(1),
        ),
        (
            "RGBA de 1 color",
            rgba_de(&[Rgba::opaque(1, 2, 3), Rgba::opaque(1, 2, 3)]),
            ColorChoice::Entry(0),
        ),
    ] {
        let mut session = sesion_de(image);
        session.choice = ColorChoice::Slot(3);
        session.view.reveal = false;
        assert_eq!(
            session.convert_mode(Mode::Indexed, TODO),
            Ok(true),
            "{nombre}"
        );
        assert_eq!(session.choice, esperada, "{nombre}");
        assert!(
            pixel_for_choice(imagen_actual(&session), session.choice).is_some(),
            "{nombre}: la elección es válida"
        );
        assert!(
            session.view.reveal,
            "{nombre}: la elección cambió y el panel la muestra"
        );
    }
}

#[test]
fn fila_7_de_indexado_entry_5_pasa_a_slot_1() {
    for target in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let mut session = sesion_de(imagen_de(Mode::Indexed));
        session.choice = ColorChoice::Entry(5);
        session.view.reveal = false;
        assert_eq!(session.convert_mode(target, TODO), Ok(true), "{target:?}");
        assert_eq!(session.choice, ColorChoice::Slot(1), "{target:?}");
        assert!(session.view.reveal, "{target:?}: la elección cambió");
    }
}

#[test]
fn fila_7_entre_modos_sin_paleta_la_ranura_se_conserva_y_el_panel_no_se_mueve() {
    for from in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for target in destinos(from).into_iter().filter(|t| *t != Mode::Indexed) {
            for slot in [1u8, 3, 9] {
                let mut session = sesion_de(imagen_de(from));
                session.choice = ColorChoice::Slot(slot);
                session.view.reveal = false;
                assert_eq!(session.convert_mode(target, TODO), Ok(true));
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
fn fila_7_la_eleccion_resultante_coincide_con_el_modelo_para_todas_las_parejas() {
    for from in MODOS {
        for target in destinos(from) {
            for choice in [
                ColorChoice::Slot(1),
                ColorChoice::Slot(3),
                ColorChoice::Slot(9),
                ColorChoice::Entry(0),
                ColorChoice::Entry(5),
            ] {
                // Una elección que valga en el origen: `Entry` solo en indexadas, `Slot` en las
                // demás.
                let valida = match (from, choice) {
                    (Mode::Indexed, ColorChoice::Entry(_)) => true,
                    (Mode::Indexed, ColorChoice::Slot(_)) => false,
                    (_, ColorChoice::Slot(_)) => true,
                    (_, ColorChoice::Entry(_)) => false,
                };
                if !valida {
                    continue;
                }
                let mut session = sesion_de(imagen_de(from));
                session.choice = choice;
                session.view.reveal = false;
                assert_eq!(session.convert_mode(target, TODO), Ok(true));
                let esperada = eleccion_esperada(imagen_actual(&session), choice);
                assert_eq!(
                    session.choice, esperada,
                    "{from:?} a {target:?} con {choice:?}"
                );
                assert_eq!(
                    session.view.reveal,
                    esperada != choice,
                    "{from:?} a {target:?} con {choice:?}: reveal"
                );
            }
        }
    }
}

#[test]
fn fila_7_convertir_descarta_el_rango_de_rotacion_y_cancela_la_quita_pendiente() {
    // Rango de rotación: RGBA → Indexado (el rango no puede sobrevivir al cambio de paleta).
    let mut session = sesion_de(imagen_de(Mode::Rgba));
    session.ops.set_range_end(3);
    assert_eq!(session.ops.range_end(), Some(3));
    assert_eq!(session.convert_mode(Mode::Indexed, TODO), Ok(true));
    assert_eq!(session.ops.range_end(), None, "rango fuera");

    // Quita pendiente: se cancela (el documento cambió) y se avisa una sola vez.
    let mut session = sesion_de(imagen_de(Mode::Indexed));
    session.ops.begin_removal(3, Some(1));
    assert!(session.ops.removal().is_some());
    assert!(!session.take_removal_cancelled());
    assert_eq!(session.convert_mode(Mode::Rgba, TODO), Ok(true));
    assert!(session.ops.removal().is_none(), "la quita se canceló");
    assert!(session.take_removal_cancelled(), "se avisa");
    assert!(!session.take_removal_cancelled(), "una sola vez");
}

#[test]
fn fila_7_el_mismo_modo_da_ok_false_sin_paso_ni_cambios() {
    for mode in MODOS {
        let mut session = sesion_de(imagen_de(mode));
        // Con historia y con un paso por rehacer: el mismo modo no borra la pila de rehacer.
        edita_pixel(&mut session, 0, 0, 77);
        session.undo().expect("deshacer");
        session.view.reveal = false;
        let antes = foto(&session);
        let pasos = pasos_de_deshacer(&session.document);
        assert!(session.document.can_redo());
        for allowed in [Allowed::NOTHING, TODO] {
            assert_eq!(session.convert_mode(mode, allowed), Ok(false), "{mode:?}");
            assert_eq!(foto(&session), antes, "{mode:?}");
            assert_eq!(pasos_de_deshacer(&session.document), pasos, "{mode:?}");
        }
    }
}

#[test]
fn fila_7_undo_deja_imagen_modo_y_paleta_iguales_y_redo_la_convertida() {
    for from in MODOS {
        for target in destinos(from) {
            let contexto = format!("{from:?} a {target:?}");
            let original = imagen_de(from);
            let mut session = sesion_de(original.clone());
            let h0 = session.document.history_bytes();
            let s0 = session.document.state_id();
            assert_eq!(session.convert_mode(target, TODO), Ok(true), "{contexto}");
            let convertida = imagen_actual(&session).clone();
            let h1 = session.document.history_bytes();
            let s1 = session.document.state_id();
            let eleccion_convertida = session.choice;

            session.undo().expect("deshacer");
            assert_eq!(imagen_actual(&session), &original, "{contexto}: undo");
            assert_eq!(modo_actual(&session), from, "{contexto}: modo");
            assert_eq!(
                imagen_actual(&session).palette(),
                original.palette(),
                "{contexto}: paleta"
            );
            assert_eq!(session.document.state_id(), s0, "{contexto}");
            assert!(!session.document.can_undo(), "{contexto}");
            assert!(session.document.can_redo(), "{contexto}");
            assert_eq!(
                session.document.history_bytes(),
                h0,
                "{contexto}: tras undo"
            );
            // La elección se vuelve a normalizar; no se «recuerda» la anterior.
            assert_eq!(
                session.choice,
                eleccion_esperada(&original, eleccion_convertida),
                "{contexto}: elección tras undo"
            );
            assert!(
                pixel_for_choice(imagen_actual(&session), session.choice).is_some(),
                "{contexto}"
            );
            if from == Mode::Indexed {
                assert_eq!(session.choice, ColorChoice::Entry(0), "{contexto}");
            }

            session.redo().expect("rehacer");
            assert_eq!(imagen_actual(&session), &convertida, "{contexto}: redo");
            assert_eq!(session.document.state_id(), s1, "{contexto}");
            assert_eq!(
                session.document.history_bytes(),
                h1,
                "{contexto}: tras redo"
            );
            assert!(session.document.can_undo());
            assert!(!session.document.can_redo());
            assert!(
                pixel_for_choice(imagen_actual(&session), session.choice).is_some(),
                "{contexto}"
            );
        }
    }
}

#[test]
fn fila_7_tras_undo_con_origen_no_indexado_la_ranura_es_la_1_aunque_hubiera_otra() {
    let mut session = sesion_de(imagen_de(Mode::Rgba));
    session.choice = ColorChoice::Slot(3);
    session
        .convert_mode(Mode::Indexed, TODO)
        .expect("convertir");
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
fn fila_7_dos_conversiones_son_dos_pasos_y_se_deshacen_en_orden() {
    let original = imagen_de(Mode::Rgba);
    let mut session = sesion_de(original.clone());
    assert_eq!(session.convert_mode(Mode::GrayAlpha, TODO), Ok(true));
    let a_gris_alfa = imagen_actual(&session).clone();
    assert_eq!(session.convert_mode(Mode::Rgba, TODO), Ok(true));
    assert_eq!(pasos_de_deshacer(&session.document), 2);
    session.undo().expect("deshacer");
    assert_eq!(imagen_actual(&session), &a_gris_alfa);
    session.undo().expect("deshacer");
    assert_eq!(imagen_actual(&session), &original);
}

// ---------------------------------------------------------------------------------------
// Fila 8: fallos, reserva e historia.
// ---------------------------------------------------------------------------------------

/// 257×1 con 256 colores y el transparente: más de 256 entradas para indexado.
fn rgba_de_257_colores() -> Image {
    let mut colores: Vec<Rgba> = (0..256u32)
        .map(|i| Rgba::opaque((i % 256) as u8, (i / 256) as u8, 0))
        .collect();
    colores.push(Rgba::TRANSPARENT);
    rgba_de(&colores)
}

/// Sesión con historia (3 ediciones) y elección no inicial.
fn sesion_con_historia(image: Image) -> Session {
    let mut session = sesion_de(image);
    for i in 0..3u8 {
        edita_pixel(&mut session, u32::from(i), 0, 100 + i);
    }
    session.choice = eleccion_esperada(imagen_actual(&session), ColorChoice::Slot(3));
    session.view.reveal = false;
    session
}

#[test]
fn fila_8_loss_not_allowed_deja_todo_intacto() {
    let mut session = sesion_con_historia(imagen_de(Mode::Rgba));
    let antes = foto(&session);
    let pasos = pasos_de_deshacer(&session.document);
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
        let resultado = session.convert_mode(Mode::Gray, allowed);
        assert!(
            matches!(
                resultado,
                Err(DocumentError::Convert(ConvertError::LossNotAllowed { .. }))
            ),
            "{allowed:?}: {resultado:?}"
        );
        assert_eq!(foto(&session), antes, "{allowed:?}");
        assert_eq!(pasos_de_deshacer(&session.document), pasos);
    }
    // El informe lleva los contadores completos.
    match session.convert_mode(Mode::Gray, Allowed::NOTHING) {
        Err(DocumentError::Convert(ConvertError::LossNotAllowed { report })) => {
            // Las tres ediciones dejaron opacos los tres primeros píxeles: color en los cuatro y
            // alfa en el último.
            assert_eq!((report.color_lost, report.alpha_lost), (4, 1));
            assert_eq!(Analysis::Lossy(report), analyze(&antes.image, Mode::Gray));
        }
        otro => panic!("{otro:?}"),
    }
}

#[test]
fn fila_8_too_many_colors_deja_todo_intacto() {
    let mut session = sesion_con_historia(rgba_de_257_colores());
    let antes = foto(&session);
    assert_eq!(
        session.convert_mode(Mode::Indexed, TODO),
        Err(DocumentError::Convert(ConvertError::TooManyColors {
            at_least: 257
        }))
    );
    assert_eq!(foto(&session), antes);
}

#[test]
fn fila_8_el_modo_actual_da_ok_false_y_deja_todo_intacto() {
    for mode in MODOS {
        let mut session = sesion_con_historia(imagen_de(mode));
        let antes = foto(&session);
        assert_eq!(session.convert_mode(mode, TODO), Ok(false), "{mode:?}");
        assert_eq!(foto(&session), antes, "{mode:?}");
    }
}

#[test]
fn fila_8_un_error_no_cambia_la_pila_de_rehacer_ni_la_marca_de_modificado() {
    let mut session = sesion_de(imagen_de(Mode::Rgba));
    edita_pixel(&mut session, 0, 0, 9);
    session.undo().expect("deshacer");
    assert!(session.document.can_redo());
    assert!(!session.modified());
    let antes = foto(&session);
    assert!(session.convert_mode(Mode::Gray, Allowed::NOTHING).is_err());
    assert_eq!(foto(&session), antes);
    assert!(session.document.can_redo(), "el error no borra el rehacer");
    assert!(!session.modified());
}

// --- reserve_for_convert ---

/// Un `ImageAlloc` falso: anota los `probe` y las imágenes pedidas, y falla donde se le dice.
struct Falso {
    probe_ok: bool,
    probes: Vec<usize>,
    imagenes: usize,
}

impl Falso {
    fn nuevo(probe_ok: bool) -> Falso {
        Falso {
            probe_ok,
            probes: Vec::new(),
            imagenes: 0,
        }
    }
}

impl ImageAlloc for Falso {
    fn image(&mut self, spec: &NewImageSpec) -> Result<Image, ImageError> {
        self.imagenes += 1;
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

fn bytes_esperados(image: &Image, target: Mode) -> u64 {
    let destino = target_byte_len(image.width(), image.height(), target).expect("cabe") as u64;
    destino + 4 * u64::from(image.width()) * u64::from(image.height())
}

#[test]
fn fila_8_reserve_for_convert_falla_con_memory_de_target_byte_len_mas_4_n_sin_llamar_a_nada_mas() {
    for from in MODOS {
        for target in destinos(from) {
            let image = imagen_de(from);
            let mut falso = Falso::nuevo(false);
            let resultado = reserve_for_convert(&image, target, &mut falso);
            let esperados = bytes_esperados(&image, target);
            match resultado {
                Err(ConvertModeError::Memory { bytes }) => {
                    assert_eq!(bytes, esperados, "{from:?} a {target:?}");
                }
                Ok(()) => panic!("{from:?} a {target:?}: debía fallar"),
            }
            assert_eq!(
                falso.probes,
                [usize::try_from(esperados).expect("cabe")],
                "{from:?} a {target:?}: un solo probe con los bytes"
            );
            assert_eq!(
                falso.imagenes, 0,
                "{from:?} a {target:?}: no se pide ninguna imagen"
            );
        }
    }
}

#[test]
fn fila_8_reserve_for_convert_con_probe_ok_da_ok_con_un_solo_probe() {
    for from in MODOS {
        for target in destinos(from) {
            let image = imagen_de(from);
            let mut falso = Falso::nuevo(true);
            assert!(
                reserve_for_convert(&image, target, &mut falso).is_ok(),
                "{from:?} a {target:?}"
            );
            assert_eq!(falso.probes.len(), 1, "{from:?} a {target:?}");
            assert_eq!(falso.imagenes, 0);
        }
    }
}

#[test]
fn fila_8_reserve_for_convert_con_el_real_da_ok() {
    for from in MODOS {
        for target in destinos(from) {
            assert!(
                reserve_for_convert(&imagen_de(from), target, &mut SystemAlloc).is_ok(),
                "{from:?} a {target:?}"
            );
        }
    }
}

#[test]
fn fila_8_el_aviso_de_falta_de_memoria_dice_el_tamano_el_modo_y_las_mib() {
    // 8192² a RGBA: 256 MiB del destino y 256 MiB de la subida: 512 MiB exactos.
    let bytes = 512 * MIB as u64;
    assert_eq!(
        convert_memory_notice(8192, 8192, Mode::Rgba, bytes),
        "sin memoria para convertir 8192×8192 a RGBA (~512 MiB)"
    );
    for (target, modo) in [
        (Mode::Indexed, "indexado"),
        (Mode::Rgba, "RGBA"),
        (Mode::GrayAlpha, "gris con alfa"),
        (Mode::Gray, "gris"),
    ] {
        let texto = convert_memory_notice(3, 2, target, 7 * MIB as u64);
        assert_eq!(
            texto,
            format!("sin memoria para convertir 3×2 a {modo} (~7 MiB)")
        );
    }
    // Con un tamaño que no es múltiplo de MiB, la cifra es la de redondear a un lado o al otro.
    let texto = convert_memory_notice(10, 10, Mode::Gray, 512 * MIB as u64 + 1);
    assert!(
        texto == "sin memoria para convertir 10×10 a gris (~512 MiB)"
            || texto == "sin memoria para convertir 10×10 a gris (~513 MiB)",
        "{texto}"
    );
    // Con muy pocos bytes, al menos 1 MiB (nunca «~0 MiB»).
    let pequeno = convert_memory_notice(1, 1, Mode::Gray, 5);
    assert!(
        pequeno.starts_with("sin memoria para convertir 1×1 a gris (~"),
        "{pequeno}"
    );
    assert!(pequeno.ends_with(" MiB)"), "{pequeno}");
    assert!(!pequeno.contains("~0 MiB"), "{pequeno}");
}

#[test]
fn fila_8_el_aviso_de_exito_dice_el_modo_y_solo_las_perdidas_mayores_que_cero() {
    // Tres rojos a gris con alfa: pierde el color en 3 píxeles (el ejemplo de la spec).
    let rojos = rgba_de(&[Rgba::opaque(255, 0, 0); 3]);
    let analisis = analyze(&rojos, Mode::GrayAlpha);
    assert_eq!(
        describe_converted(Mode::GrayAlpha, Some(&analisis)),
        "imagen convertida a gris con alfa · perdido: color 3 px"
    );
    // Sin pérdida, solo la primera parte.
    let exacta = analyze(&imagen_de(Mode::Gray), Mode::Rgba);
    assert_eq!(
        describe_converted(Mode::Rgba, Some(&exacta)),
        "imagen convertida a RGBA"
    );
    // Con alfa y color.
    let con_las_dos = analyze(&imagen_de(Mode::Rgba), Mode::Gray);
    assert_eq!(
        describe_converted(Mode::Gray, Some(&con_las_dos)),
        "imagen convertida a gris · perdido: color 3 px, alfa 2 px"
    );
    // Y el informe vigente es el que pasó por `convert_mode`.
    let mut session = sesion_de(rojos);
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
    assert!(matches!(analisis, Analysis::Lossy(_)));
}

// --- Historia ---

/// Gris de 64 × 64 con `n` ediciones de un píxel cada una.
fn gris_con_ediciones(n: usize) -> Session {
    let mut session = sesion_de(Image::new(64, 64, Mode::Gray).expect("imagen"));
    for i in 0..n {
        edita_pixel(
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
fn fila_8_el_paso_de_conversion_se_conserva_aunque_el_limite_sea_0_y_lo_anterior_se_expulsa() {
    let mut session = sesion_de(imagen_de(Mode::Rgba));
    for i in 0..3u8 {
        edita_pixel(&mut session, u32::from(i), 0, 50 + i);
    }
    session.document.set_history_limit(0);
    assert!(
        session.document.can_undo(),
        "el último paso nunca se descarta"
    );
    assert_eq!(pasos_de_deshacer(&session.document), 1);
    let antes_de_convertir = imagen_actual(&session).clone();

    assert_eq!(session.convert_mode(Mode::Gray, TODO), Ok(true));
    let convertida = imagen_actual(&session).clone();
    assert_eq!(convertida.mode(), Mode::Gray);

    // Entre los 16 bytes de píxeles de la imagen vieja (4×1 RGBA) y esos más 2048.
    let h = session.document.history_bytes();
    assert!((16..=16 + 2048).contains(&h), "history_bytes = {h}");
    assert_eq!(
        pasos_de_deshacer(&session.document),
        1,
        "lo anterior se expulsó: solo queda la conversión"
    );

    // `undo` llega a la conversión y no más.
    session.undo().expect("deshacer la conversión");
    assert_eq!(imagen_actual(&session), &antes_de_convertir);
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
    session.redo().expect("rehacer");
    assert_eq!(imagen_actual(&session), &convertida);
}

#[test]
fn fila_8_con_limite_0_tras_otra_edicion_solo_queda_esa() {
    let mut session = sesion_de(imagen_de(Mode::Rgba));
    for i in 0..3u8 {
        edita_pixel(&mut session, u32::from(i), 0, 50 + i);
    }
    session.document.set_history_limit(0);
    assert_eq!(session.convert_mode(Mode::Gray, TODO), Ok(true));
    let convertida = imagen_actual(&session).clone();

    edita_pixel(&mut session, 0, 0, 200);
    assert_eq!(
        pasos_de_deshacer(&session.document),
        1,
        "la edición expulsa la conversión"
    );
    session.undo().expect("deshacer la edición");
    assert_eq!(
        imagen_actual(&session),
        &convertida,
        "vuelve a la Gray de la conversión"
    );
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
    session.redo().expect("rehacer la edición");
    assert_eq!(modo_actual(&session), Mode::Gray);
}

#[test]
fn fila_8_con_el_limite_por_defecto_y_una_imagen_pequena_los_pasos_anteriores_siguen() {
    let original = imagen_de(Mode::Rgba);
    let mut session = sesion_de(original.clone());
    for i in 0..3u8 {
        edita_pixel(&mut session, u32::from(i), 0, 50 + i);
    }
    assert_eq!(
        session.document.history_limit(),
        Document::DEFAULT_HISTORY_LIMIT
    );
    assert_eq!(session.convert_mode(Mode::Gray, TODO), Ok(true));
    assert_eq!(
        pasos_de_deshacer(&session.document),
        4,
        "las 3 ediciones y la conversión"
    );
    for _ in 0..4 {
        session.undo().expect("deshacer");
    }
    assert_eq!(
        imagen_actual(&session),
        &original,
        "undo ×4 vuelve al original"
    );
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
}

#[test]
fn fila_8_el_aviso_de_historia_primero_coincide_con_lo_que_pasa_c_mayor_que_el_limite() {
    // Gris 64×64: P = 4096 bytes de píxeles; un paso cuesta de P a P + 2048.
    let pixeles = 64 * 64;
    let mut session = gris_con_ediciones(100);
    let limite = pixeles - 1;
    session.document.set_history_limit(limite);
    let usado = session.document.history_bytes();
    let antes = imagen_actual(&session).clone();
    assert_eq!(
        history_warning(
            &antes,
            History {
                limit: limite,
                used: usado
            }
        )
        .as_deref(),
        Some(AVISO_HISTORIA_TODO)
    );

    assert_eq!(session.convert_mode(Mode::Rgba, TODO), Ok(true));
    assert!(
        session.document.history_bytes() > limite,
        "el paso supera el límite y se conserva"
    );
    assert_eq!(
        pasos_de_deshacer(&session.document),
        1,
        "expulsa todo lo anterior"
    );
    let convertida = imagen_actual(&session).clone();

    // «La siguiente edición lo expulsa a él.»
    edita_pixel(&mut session, 0, 0, 7);
    assert_eq!(pasos_de_deshacer(&session.document), 1);
    session.undo().expect("deshacer la edición");
    assert_eq!(imagen_actual(&session), &convertida);
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
}

#[test]
fn fila_8_el_aviso_de_historia_segundo_coincide_con_lo_que_pasa_se_expulsan_los_mas_antiguos() {
    let pixeles = 64 * 64;
    let mut session = gris_con_ediciones(100);
    let limite = pixeles + 2560; // c = P + 2048 ≤ límite < 2c
    session.document.set_history_limit(limite);
    let usado = session.document.history_bytes();
    assert!(
        usado > 2560 && usado <= limite,
        "la prueba necesita una historia que no quepa junto al paso nuevo: usado = {usado}"
    );
    let antes = imagen_actual(&session).clone();
    assert_eq!(
        history_warning(
            &antes,
            History {
                limit: limite,
                used: usado
            }
        )
        .as_deref(),
        Some(AVISO_HISTORIA_PARTE)
    );
    let pasos_antes = pasos_de_deshacer(&session.document);

    assert_eq!(session.convert_mode(Mode::Rgba, TODO), Ok(true));
    let pasos_despues = pasos_de_deshacer(&session.document);
    assert!(
        (1..=pasos_antes).contains(&pasos_despues),
        "se expulsan pasos antiguos: antes {pasos_antes}, después {pasos_despues}"
    );
    assert!(session.document.history_bytes() <= limite, "ahora cabe");
    // La conversión se conserva: el último `undo` posible vuelve a la imagen de antes.
    session.undo().expect("deshacer la conversión");
    assert_eq!(imagen_actual(&session), &antes);
}

#[test]
fn fila_8_sin_aviso_de_historia_no_se_expulsa_nada() {
    let pixeles = 64 * 64;
    // Por defecto: c ≪ límite/2.
    let mut session = gris_con_ediciones(20);
    let usado = session.document.history_bytes();
    let antes = imagen_actual(&session).clone();
    assert_eq!(
        history_warning(
            &antes,
            History {
                limit: session.document.history_limit(),
                used: usado
            }
        ),
        None
    );
    let pasos_antes = pasos_de_deshacer(&session.document);
    assert_eq!(session.convert_mode(Mode::Rgba, TODO), Ok(true));
    assert_eq!(pasos_de_deshacer(&session.document), pasos_antes + 1);

    // Con límite/2 < c ≤ límite pero h + c ≤ límite: tampoco hay aviso ni se expulsa nada.
    let mut session = gris_con_ediciones(3);
    let limite = pixeles + 2560;
    session.document.set_history_limit(limite);
    let usado = session.document.history_bytes();
    let antes = imagen_actual(&session).clone();
    assert_eq!(
        history_warning(
            &antes,
            History {
                limit: limite,
                used: usado
            }
        ),
        None
    );
    let pasos_antes = pasos_de_deshacer(&session.document);
    assert_eq!(session.convert_mode(Mode::Rgba, TODO), Ok(true));
    assert_eq!(pasos_de_deshacer(&session.document), pasos_antes + 1);
    assert!(session.document.history_bytes() <= limite);
}

#[test]
fn fila_8_el_margen_de_la_spec_cubre_el_coste_real_de_un_paso_de_conversion() {
    // El aviso supone que un paso cuesta como mucho P + `HISTORY_MARGIN`.
    for from in MODOS {
        for target in destinos(from) {
            let original = imagen_de(from);
            let mut session = sesion_de(original.clone());
            session.convert_mode(target, TODO).expect("convertir");
            assert!(
                session.document.history_bytes() as u64
                    <= bytes_de_pixeles(&original) as u64 + HISTORY_MARGIN,
                "{from:?} a {target:?}"
            );
        }
    }
}

#[test]
#[ignore = "reserva 64 MiB de RGBA y 16 MiB de gris: cargo test -p px-app -- --ignored"]
fn fila_8_rgba_de_4096_con_el_limite_por_defecto_convertir_undo_otra_edicion_undo() {
    let original = Image::new(4096, 4096, Mode::Rgba).expect("RGBA 4096²");
    let mut session = sesion_de(original);
    let limite = session.document.history_limit();
    let antes = imagen_actual(&session).clone();
    assert_eq!(
        history_warning(
            &antes,
            History {
                limit: limite,
                used: 0
            }
        )
        .as_deref(),
        Some(AVISO_HISTORIA_TODO),
        "RGBA de 4096² con el límite por defecto: el primer aviso"
    );
    drop(antes);

    let allowed = Allowed {
        luma: true,
        drop_alpha: Some([255, 255, 255]),
    };
    // El RGBA de ceros es transparente: pierde el alfa en todos los píxeles.
    assert_eq!(session.convert_mode(Mode::Gray, allowed), Ok(true));
    assert!(
        session.document.history_bytes() > limite,
        "el paso supera el límite"
    );
    assert_eq!(pasos_de_deshacer(&session.document), 1);

    session.undo().expect("deshacer la conversión");
    assert_eq!(modo_actual(&session), Mode::Rgba);
    edita_pixel(&mut session, 0, 0, 9);
    session.undo().expect("deshacer la edición");
    assert_eq!(modo_actual(&session), Mode::Rgba);
    assert_eq!(session.undo(), Err(DocumentError::NothingToUndo));
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: aplicar es atómico y reversible (modelo de instantáneas).
// ---------------------------------------------------------------------------------------

const VALORES: [u8; 5] = [0, 1, 127, 128, 255];

#[derive(Clone, Debug)]
struct Semilla {
    modo: usize,
    ancho: u32,
    alto: u32,
    pixeles: Vec<[usize; 4]>,
    colores: Vec<[usize; 4]>,
    n: usize,
    transparente: usize,
}

fn estrategia_de_semilla() -> impl Strategy<Value = Semilla> {
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
            |(modo, ancho, alto, pixeles, colores, n, transparente)| Semilla {
                modo,
                ancho,
                alto,
                pixeles,
                colores,
                n,
                transparente,
            },
        )
}

fn rgba_de_indices(v: &[usize; 4]) -> Rgba {
    Rgba::new(VALORES[v[0]], VALORES[v[1]], VALORES[v[2]], VALORES[v[3]])
}

fn imagen_de_semilla(s: &Semilla) -> Image {
    let (w, h) = (s.ancho, s.alto);
    let total = (w * h) as usize;
    let mode = MODOS[s.modo];
    let mut image = match mode {
        Mode::Indexed => {
            let colores: Vec<Rgba> = s.colores.iter().take(s.n).map(rgba_de_indices).collect();
            let mut palette = Palette::from_colors(colores).expect("paleta");
            if s.transparente < s.n {
                palette
                    .set_transparent_index(Some(s.transparente as u8))
                    .expect("índice transparente");
            }
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        otro => Image::new(w, h, otro).expect("imagen"),
    };
    for (i, v) in s.pixeles.iter().take(total).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let pixel = match mode {
            Mode::Rgba => Pixel::Rgba(rgba_de_indices(v)),
            Mode::Gray => Pixel::Gray(VALORES[v[0]]),
            Mode::GrayAlpha => Pixel::GrayAlpha {
                gray: VALORES[v[0]],
                alpha: VALORES[v[3]],
            },
            Mode::Indexed => Pixel::Indexed((v[0] % s.n) as u8),
        };
        image.set_pixel(x, y, pixel).expect("píxel");
    }
    image
}

#[derive(Clone, Debug)]
enum Op {
    /// `destino` 0 a 2: `destinos(actual)[destino]`; 3: el propio modo actual.
    Convertir {
        destino: usize,
        luma: bool,
        fondo: usize,
    },
    Deshacer,
    Rehacer,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0usize..4, any::<bool>(), 0usize..3).prop_map(|(destino, luma, fondo)| Op::Convertir {
            destino,
            luma,
            fondo,
        }),
        1 => Just(Op::Deshacer),
        1 => Just(Op::Rehacer),
    ]
}

fn allowed_de(luma: bool, fondo: usize) -> Allowed {
    Allowed {
        luma,
        drop_alpha: match fondo {
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
    fn propiedad_3_aplicar_es_atomico_y_reversible(
        semilla in estrategia_de_semilla(),
        eleccion in 0usize..9,
        ops in prop::collection::vec(op(), 0..=12),
    ) {
        let inicial = imagen_de_semilla(&semilla);
        let mut session = sesion_de(inicial.clone());
        session.choice = match inicial.palette() {
            Some(p) => ColorChoice::Entry((eleccion % p.len()) as u8),
            None => ColorChoice::Slot(eleccion as u8 + 1),
        };
        prop_assert!(pixel_for_choice(&inicial, session.choice).is_some());

        let mut instantaneas = vec![inicial];
        let mut cursor = 0usize;

        for (i, operacion) in ops.iter().enumerate() {
            let contexto = format!("paso {i}: {operacion:?}");
            let antes = foto(&session);
            let mut cambia = false;
            match operacion {
                Op::Convertir { destino, luma, fondo } => {
                    let actual = instantaneas[cursor].clone();
                    let target = if *destino == 3 {
                        actual.mode()
                    } else {
                        destinos(actual.mode())[*destino]
                    };
                    let allowed = allowed_de(*luma, *fondo);
                    let resultado = session.convert_mode(target, allowed);
                    if target == actual.mode() {
                        prop_assert_eq!(resultado, Ok(false), "{}", contexto);
                    } else {
                        match convert_image(&actual, target, allowed) {
                            Ok(conversion) => {
                                prop_assert_eq!(resultado, Ok(true), "{}", contexto);
                                instantaneas.truncate(cursor + 1);
                                instantaneas.push(conversion.image);
                                cursor += 1;
                                cambia = true;
                            }
                            Err(error) => {
                                prop_assert_eq!(
                                    resultado,
                                    Err(DocumentError::Convert(error)),
                                    "{}",
                                    contexto
                                );
                            }
                        }
                    }
                }
                Op::Deshacer => {
                    let resultado = session.undo();
                    if cursor > 0 {
                        prop_assert_eq!(resultado, Ok(()), "{}", contexto);
                        cursor -= 1;
                        cambia = true;
                    } else {
                        prop_assert_eq!(resultado, Err(DocumentError::NothingToUndo), "{}", contexto);
                    }
                }
                Op::Rehacer => {
                    let resultado = session.redo();
                    if cursor + 1 < instantaneas.len() {
                        prop_assert_eq!(resultado, Ok(()), "{}", contexto);
                        cursor += 1;
                        cambia = true;
                    } else {
                        prop_assert_eq!(resultado, Err(DocumentError::NothingToRedo), "{}", contexto);
                    }
                }
            }

            // El documento es la instantánea del cursor, con su modo y su paleta.
            let imagen = imagen_actual(&session);
            prop_assert_eq!(imagen, &instantaneas[cursor], "{}", contexto);
            prop_assert_eq!(imagen.mode(), instantaneas[cursor].mode(), "{}", contexto);
            prop_assert_eq!(imagen.palette(), instantaneas[cursor].palette(), "{}", contexto);
            prop_assert_eq!(session.document.can_undo(), cursor > 0, "{}", contexto);
            prop_assert_eq!(
                session.document.can_redo(),
                cursor + 1 < instantaneas.len(),
                "{}",
                contexto
            );
            // La elección es siempre válida.
            prop_assert!(
                pixel_for_choice(imagen, session.choice).is_some(),
                "{}: la elección {:?} no vale",
                contexto,
                session.choice
            );
            // Un `Err` o un `Ok(false)` dejan todo igual.
            if !cambia && !matches!(operacion, Op::Deshacer | Op::Rehacer) {
                prop_assert_eq!(foto(&session), antes.clone(), "{}", contexto);
            }
            if !cambia {
                prop_assert_eq!(session.document.state_id(), antes.state_id, "{}", contexto);
                prop_assert_eq!(session.document.history_bytes(), antes.history_bytes, "{}", contexto);
                prop_assert_eq!(session.choice, antes.choice, "{}", contexto);
                prop_assert_eq!(session.modified(), antes.modified, "{}", contexto);
            }
        }
    }
}
