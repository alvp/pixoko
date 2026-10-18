//! Tests de `session` con las operaciones de la paleta (specs/hito-2-paleta-operaciones.md):
//! filas 3 a 6, 8, 9, 10, 11 y 13 con `Session::apply_palette_op`, `undo`, `redo`, `normalize` y
//! `take_removal_cancelled`, y propiedad 4 (sesión indexada con secuencias de acciones).
//!
//! Los ayudantes de los otros módulos de tests son privados: aquí se duplican los mínimos. La
//! quita pendiente la maneja el test como lo haría el panel (`begin_removal`, `set_destination`,
//! `Confirmar` = `apply_palette_op(Remove { .. }, false)`).

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{Command, DocumentError, Image, Palette, PaletteEditError, Pixel, Rgba};
use px_formats::write_png;
use px_ui::color::ColorChoice;
use px_ui::color_edit::{Commit, EditTarget};
use px_ui::palette_ops::{
    NOTHING_TO_ROTATE_NOTICE, PaletteOp, REMOVAL_CANCELLED_NOTICE, Removal, default_destination,
    rotation_range, uses_entry,
};

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
            std::env::temp_dir().join(format!("pixoko-px-app-ops-{}-{n}", std::process::id()));
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

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);
/// `X` de `P5`.
const X: Rgba = Rgba::opaque(200, 30, 30);
/// `Y` de `P5`.
const Y: Rgba = Rgba::opaque(10, 10, 240);

fn write_image(dir: &Path, name: &str, image: &Image) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, write_png(image).expect("write_png")).expect("escribir el PNG");
    path
}

/// Indexada de una fila con esa paleta, índice transparente y píxeles.
fn indexed(colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Image {
    let mut palette = Palette::from_colors(colors.iter().copied()).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente");
    let mut image = Image::new_indexed(pixels.len() as u32, 1, palette).expect("imagen indexada");
    for (x, &pixel) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(pixel))
            .expect("píxel");
    }
    image
}

/// Con esa paleta y los píxeles `0, 1, 2, ...` (uno por entrada).
fn row(colors: &[Rgba], transparent: Option<u8>) -> Image {
    let pixels: Vec<u8> = (0..colors.len()).map(|i| i as u8).collect();
    indexed(colors, transparent, &pixels)
}

/// `n` colores distintos.
fn distinct(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect()
}

/// `F` de la spec: `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
fn image_f(transparent: Option<u8>) -> Image {
    row(&[RED, GREEN, BLUE, BLACK], transparent)
}

fn session_f() -> Session {
    Session::sample(Document::new(image_f(None)))
}

/// `P5` de la spec: `[R, G, B, X, Y]`, píxeles `[0, 1, 2, 3, 3]`.
fn session_p5(transparent: Option<u8>) -> Session {
    Session::sample(Document::new(indexed(
        &[RED, GREEN, BLUE, X, Y],
        transparent,
        &[0, 1, 2, 3, 3],
    )))
}

fn main_image(document: &Document) -> &Image {
    document
        .image(document.main_image())
        .expect("imagen principal")
}

fn img(session: &Session) -> &Image {
    main_image(&session.document)
}

fn colors_of(image: &Image) -> Vec<Rgba> {
    image.palette().expect("indexada").colors().to_vec()
}

fn pixels_of(image: &Image) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..image.height() {
        for x in 0..image.width() {
            match image.get_pixel(x, y).expect("píxel") {
                Pixel::Indexed(i) => out.push(i),
                other => panic!("píxel no indexado: {other:?}"),
            }
        }
    }
    out
}

fn transparent_of(image: &Image) -> Option<u8> {
    image.palette().expect("indexada").transparent_index()
}

fn len_of(session: &Session) -> usize {
    img(session).palette().expect("indexada").len()
}

/// Índice de la entrada elegida (el test falla si la elección no es una entrada).
fn entry_of(session: &Session) -> u8 {
    match session.choice {
        ColorChoice::Entry(i) => i,
        other => panic!("la elección no es una entrada: {other:?}"),
    }
}

/// Elige `choice` y pone al día el editor, como hace el fotograma siguiente.
fn select(session: &mut Session, choice: ColorChoice) {
    session.choice = choice;
    session.sync_editor();
}

/// Aplica `op` con `unused` calculado como lo daría el panel (`uses_entry` de la quitada).
fn run(session: &mut Session, op: PaletteOp) -> Result<PaletteOutcome, DocumentError> {
    let unused = match op {
        PaletteOp::Remove { index, .. } => !uses_entry(img(session), index),
        _ => false,
    };
    session.apply_palette_op(op, unused)
}

/// Aplica `op` y exige `Ok(true)`; devuelve el aviso.
fn run_ok(session: &mut Session, op: PaletteOp) -> String {
    let outcome = run(session, op).expect("la operación no da error");
    assert!(outcome.changed, "{op:?} debía cambiar el documento");
    outcome.notice.expect("la operación da un resumen")
}

/// Pulsar `Quitar` en la activa como el panel: sin píxeles se aplica ya con el destino por
/// defecto (`None` si es pendiente); con píxeles queda pendiente con el destino por defecto.
fn press_remove(session: &mut Session) -> Option<PaletteOutcome> {
    session.normalize();
    let index = entry_of(session);
    let (default, used) = {
        let image = img(session);
        let palette = image.palette().expect("indexada");
        (
            default_destination(palette, index),
            uses_entry(image, index),
        )
    };
    if used {
        session.ops.begin_removal(index, default);
        None
    } else {
        let into = default.unwrap_or(if index == 0 { 1 } else { 0 });
        Some(
            session
                .apply_palette_op(PaletteOp::Remove { index, into }, true)
                .expect("quitar sin píxeles"),
        )
    }
}

/// `Confirmar` de la quita pendiente.
fn confirm_removal(session: &mut Session) -> Result<PaletteOutcome, DocumentError> {
    let removal = session.ops.removal().expect("hay una quita pendiente");
    let into = removal.into.expect("la quita tiene destino");
    session.apply_palette_op(
        PaletteOp::Remove {
            index: removal.index,
            into,
        },
        false,
    )
}

/// Cambia el color de la entrada 0 con un `SetPaletteColor` directo (un paso de deshacer).
fn edit_color(session: &mut Session, index: u8, color: Rgba) {
    let image = session.document.main_image();
    let changed = session
        .document
        .apply(Command::SetPaletteColor {
            image,
            index,
            color,
        })
        .expect("editar color");
    assert!(changed);
}

// ---------------------------------------------------------------------------------------
// Fila 3: añadir.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_anadir_copia_el_color_de_la_activa_y_selecciona_la_nueva() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    assert!(!session.view.reveal);

    let outcome = session
        .apply_palette_op(PaletteOp::Add, false)
        .expect("añadir");

    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 4 añadida (copia de la 1)")
    );
    assert_eq!(colors_of(img(&session)), [RED, GREEN, BLUE, BLACK, GREEN]);
    assert_eq!(
        pixels_of(img(&session)),
        [0, 1, 2, 3],
        "los píxeles no cambian"
    );
    assert_eq!(session.choice, ColorChoice::Entry(4));
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(4)));
    assert!(session.view.reveal, "el panel muestra la fila de la nueva");
    // Un solo paso.
    session.document.undo().expect("deshacer");
    assert!(!session.document.can_undo());
    assert_eq!(len_of(&session), 4);
}

#[test]
fn fila_3_deshacer_y_rehacer_recortan_la_seleccion_y_piden_mostrarla() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    run_ok(&mut session, PaletteOp::Add);
    assert_eq!(session.choice, ColorChoice::Entry(4));

    session.view.reveal = false;
    session.undo().expect("deshacer");
    assert_eq!(len_of(&session), 4);
    assert_eq!(
        session.choice,
        ColorChoice::Entry(3),
        "recortada a la última"
    );
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(3)));
    assert!(session.view.reveal, "la elección cambió: se muestra");

    session.view.reveal = false;
    session.redo().expect("rehacer");
    assert_eq!(len_of(&session), 5);
    assert_eq!(colors_of(img(&session))[4], GREEN);
    assert_eq!(
        session.choice,
        ColorChoice::Entry(3),
        "la elección se queda"
    );
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(3)));
    assert!(!session.view.reveal, "la elección no cambió");
}

#[test]
fn fila_3_la_copia_de_la_transparente_es_su_color_resuelto_y_no_es_transparente() {
    let mut session = Session::sample(Document::new(image_f(Some(3))));
    select(&mut session, ColorChoice::Entry(3));

    run_ok(&mut session, PaletteOp::Add);

    let colors = colors_of(img(&session));
    assert_eq!(colors.len(), 5);
    assert_eq!(colors[3], BLACK, "la transparente guarda su color");
    assert_eq!(colors[4], Rgba::new(0, 0, 0, 0), "la copia es el resuelto");
    assert_eq!(transparent_of(img(&session)), Some(3));
    assert_eq!(session.choice, ColorChoice::Entry(4));
}

#[test]
fn fila_3_con_256_entradas_anadir_da_error_y_no_toca_nada() {
    let mut session = Session::sample(Document::new(row(&distinct(256), None)));
    select(&mut session, ColorChoice::Entry(10));
    let before = img(&session).clone();
    let id = session.document.state_id();

    let result = session.apply_palette_op(PaletteOp::Add, false);

    assert!(
        matches!(
            result,
            Err(DocumentError::PaletteEdit(PaletteEditError::PaletteFull))
        ),
        "resultado: {result:?}"
    );
    assert_eq!(img(&session), &before);
    assert_eq!(session.document.state_id(), id);
    assert!(!session.document.can_undo());
    assert_eq!(session.choice, ColorChoice::Entry(10));
    assert!(!session.view.reveal);
}

#[test]
fn fila_3_con_255_entradas_pasa_a_256_y_el_siguiente_da_error() {
    let mut session = Session::sample(Document::new(row(&distinct(255), None)));
    select(&mut session, ColorChoice::Entry(254));

    run_ok(&mut session, PaletteOp::Add);
    assert_eq!(len_of(&session), 256);
    assert_eq!(session.choice, ColorChoice::Entry(255));
    assert_eq!(colors_of(img(&session))[255], colors_of(img(&session))[254]);

    let result = session.apply_palette_op(PaletteOp::Add, false);
    assert!(
        matches!(
            result,
            Err(DocumentError::PaletteEdit(PaletteEditError::PaletteFull))
        ),
        "resultado: {result:?}"
    );
    assert_eq!(len_of(&session), 256);
}

#[test]
fn fila_3_con_una_entrada_anadir_da_dos_y_selecciona_la_segunda() {
    let mut session = Session::sample(Document::new(indexed(&[RED], None, &[0, 0])));
    run_ok(&mut session, PaletteOp::Add);
    assert_eq!(colors_of(img(&session)), [RED, RED]);
    assert_eq!(session.choice, ColorChoice::Entry(1));
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(1)));
}

// ---------------------------------------------------------------------------------------
// Fila 4: quitar con y sin píxeles.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_4_destino_por_defecto_y_uso_de_las_entradas_de_p5() {
    let session = session_p5(None);
    let image = img(&session);
    let palette = image.palette().expect("indexada");
    assert_eq!(default_destination(palette, 3), Some(0));
    assert_eq!(default_destination(palette, 4), Some(2));
    assert_eq!(default_destination(palette, 0), Some(3));
    for index in 0..4 {
        assert!(uses_entry(image, index), "entrada {index}");
    }
    assert!(!uses_entry(image, 4));
}

#[test]
fn fila_4_quitar_con_pixeles_queda_pendiente_y_confirmar_aplica_un_solo_paso() {
    let mut session = session_p5(None);
    select(&mut session, ColorChoice::Entry(3));
    let initial = img(&session).clone();
    let id = session.document.state_id();

    assert!(press_remove(&mut session).is_none(), "queda pendiente");
    assert_eq!(
        session.ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(0)
        })
    );
    assert_eq!(session.document.state_id(), id, "pendiente: nada cambió");
    assert!(!session.document.can_undo());

    assert!(session.ops.set_destination(2, len_of(&session)));
    assert!(
        !session.ops.set_destination(3, len_of(&session)),
        "la propia"
    );
    assert_eq!(session.ops.removal().map(|r| r.into), Some(Some(2)));
    assert_eq!(
        session.choice,
        ColorChoice::Entry(3),
        "no cambia la selección"
    );

    let outcome = confirm_removal(&mut session).expect("confirmar");
    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 3 quitada; píxeles pasan a la 2")
    );
    assert_eq!(colors_of(img(&session)), [RED, GREEN, BLUE, Y]);
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 2, 2]);
    assert_eq!(session.choice, ColorChoice::Entry(3), "Y ocupa la 3");
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(3)));
    assert_eq!(session.ops.removal(), None);
    assert!(!session.take_removal_cancelled(), "es la propia quita");

    // Un solo deshacer restaura P5 exacta.
    session.undo().expect("deshacer");
    assert_eq!(img(&session), &initial);
    assert!(!session.document.can_undo());
    assert!(session.document.can_redo());
}

#[test]
fn fila_4_cancelar_la_quita_deja_el_documento_intacto() {
    let mut session = session_p5(None);
    select(&mut session, ColorChoice::Entry(3));
    let initial = img(&session).clone();
    let id = session.document.state_id();

    assert!(press_remove(&mut session).is_none());
    session.ops.cancel_removal();
    session.normalize();

    assert_eq!(session.ops.removal(), None);
    assert_eq!(img(&session), &initial);
    assert_eq!(session.document.state_id(), id);
    assert!(!session.document.can_undo());
    assert!(
        !session.take_removal_cancelled(),
        "cancelar a mano no avisa"
    );
}

#[test]
fn fila_4_quitar_una_entrada_sin_pixeles_se_aplica_ya() {
    let mut session = session_p5(None);
    select(&mut session, ColorChoice::Entry(4));

    let outcome = press_remove(&mut session).expect("se aplica ya");

    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 4 quitada (sin píxeles)")
    );
    assert_eq!(colors_of(img(&session)), [RED, GREEN, BLUE, X]);
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3, 3]);
    assert_eq!(
        session.choice,
        ColorChoice::Entry(3),
        "recortada a la última"
    );
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(3)));
    assert_eq!(session.ops.removal(), None);
}

#[test]
fn fila_4_quitar_de_en_medio_deja_la_seleccion_en_el_mismo_indice() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    let notice = run_ok(&mut session, PaletteOp::Remove { index: 1, into: 0 });
    assert_eq!(notice, "entrada 1 quitada; píxeles pasan a la 0");
    assert_eq!(colors_of(img(&session)), [RED, BLUE, BLACK]);
    assert_eq!(pixels_of(img(&session)), [0, 0, 1, 2]);
    assert_eq!(session.choice, ColorChoice::Entry(1), "ahora es la B");
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(1)));
}

#[test]
fn fila_4_el_destino_mayor_usa_la_numeracion_posterior_en_el_resumen() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(0));
    let notice = run_ok(&mut session, PaletteOp::Remove { index: 0, into: 2 });
    assert_eq!(notice, "entrada 0 quitada; píxeles pasan a la 1");
    assert_eq!(colors_of(img(&session)), [GREEN, BLUE, BLACK]);
    assert_eq!(pixels_of(img(&session)), [1, 0, 1, 2]);
    assert_eq!(session.choice, ColorChoice::Entry(0));
}

// ---------------------------------------------------------------------------------------
// Fila 5: transparente, única, duplicadas y 256.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_5_la_transparente_no_tiene_destino_por_defecto_y_confirma_con_el_elegido() {
    let mut session = session_p5(Some(3));
    select(&mut session, ColorChoice::Entry(3));
    let initial = img(&session).clone();
    assert_eq!(
        default_destination(img(&session).palette().expect("paleta"), 3),
        None
    );

    assert!(press_remove(&mut session).is_none());
    assert_eq!(
        session.ops.removal(),
        Some(Removal {
            index: 3,
            into: None
        })
    );
    assert!(session.ops.set_destination(4, len_of(&session)));

    let outcome = confirm_removal(&mut session).expect("confirmar");
    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 3 quitada; píxeles pasan a la 3")
    );
    assert_eq!(transparent_of(img(&session)), None);
    assert_eq!(colors_of(img(&session)), [RED, GREEN, BLUE, Y]);
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3, 3]);
    assert_eq!(session.choice, ColorChoice::Entry(3));

    session.undo().expect("deshacer");
    assert_eq!(img(&session), &initial, "un solo paso");
    assert!(!session.document.can_undo());
}

#[test]
fn fila_5_quitar_la_transparente_sin_pixeles_se_aplica_directo() {
    let mut session = session_p5(Some(4));
    select(&mut session, ColorChoice::Entry(4));

    let outcome = press_remove(&mut session).expect("se aplica ya");

    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 4 quitada; era la transparente")
    );
    assert_eq!(transparent_of(img(&session)), None);
    assert_eq!(colors_of(img(&session)), [RED, GREEN, BLUE, X]);
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3, 3]);
}

#[test]
fn fila_5_con_destino_la_transparente_los_pixeles_quedan_transparentes() {
    // Y es la transparente (4); la entrada 3 pasa sus píxeles a ella.
    let mut session = session_p5(Some(4));
    select(&mut session, ColorChoice::Entry(3));
    let notice = run_ok(&mut session, PaletteOp::Remove { index: 3, into: 4 });
    assert_eq!(notice, "entrada 3 quitada; píxeles pasan a la 3");
    assert_eq!(colors_of(img(&session)), [RED, GREEN, BLUE, Y]);
    assert_eq!(transparent_of(img(&session)), Some(3));
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3, 3]);
}

#[test]
fn fila_5_con_una_sola_entrada_quitar_da_error_y_no_deja_nada_pendiente() {
    let mut session = Session::sample(Document::new(indexed(&[RED], None, &[0, 0])));
    let before = img(&session).clone();
    let id = session.document.state_id();

    let result = session.apply_palette_op(PaletteOp::Remove { index: 0, into: 0 }, false);

    assert!(
        matches!(
            result,
            Err(DocumentError::PaletteEdit(PaletteEditError::LastEntry))
        ),
        "resultado: {result:?}"
    );
    assert_eq!(img(&session), &before);
    assert_eq!(session.document.state_id(), id);
    assert!(!session.document.can_undo());
    assert_eq!(session.ops.removal(), None);
    assert_eq!(session.choice, ColorChoice::Entry(0));
}

#[test]
fn fila_5_duplicadas_quitar_la_1_tiene_destino_0_y_quitar_la_0_destino_1() {
    let colors = [RED, RED, BLUE];
    for (index, expected_destination, expected_pixels) in
        [(1u8, 0u8, [0u8, 0, 1]), (0, 1, [0, 0, 1])]
    {
        let mut session = Session::sample(Document::new(row(&colors, None)));
        select(&mut session, ColorChoice::Entry(index));
        let initial = img(&session).clone();
        let palette = initial.palette().expect("paleta");
        assert_eq!(
            default_destination(palette, index),
            Some(expected_destination),
            "quitar la {index}"
        );

        assert!(press_remove(&mut session).is_none(), "tiene píxeles");
        assert_eq!(
            session.ops.removal().map(|r| r.into),
            Some(Some(expected_destination))
        );
        let outcome = confirm_removal(&mut session).expect("confirmar");
        assert!(outcome.changed);
        assert_eq!(colors_of(img(&session)), [RED, BLUE], "quitar la {index}");
        assert_eq!(
            pixels_of(img(&session)),
            expected_pixels,
            "quitar la {index}"
        );
        session.undo().expect("deshacer");
        assert_eq!(img(&session), &initial, "quitar la {index}");
        assert!(!session.document.can_undo(), "un solo paso");
    }
}

#[test]
fn fila_5_con_256_entradas_quitar_la_ultima_deja_la_254() {
    let mut session = Session::sample(Document::new(row(&distinct(256), None)));
    select(&mut session, ColorChoice::Entry(255));
    let initial = img(&session).clone();

    let notice = run_ok(
        &mut session,
        PaletteOp::Remove {
            index: 255,
            into: 254,
        },
    );

    assert_eq!(notice, "entrada 255 quitada; píxeles pasan a la 254");
    assert_eq!(len_of(&session), 255);
    assert_eq!(session.choice, ColorChoice::Entry(254));
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(254)));
    session.undo().expect("deshacer");
    assert_eq!(img(&session), &initial);
    assert!(!session.document.can_undo());
}

// ---------------------------------------------------------------------------------------
// Fila 6: rotar.
// ---------------------------------------------------------------------------------------

/// El documento de `session` tras aplicar `steps` con `Document::apply` directamente.
fn rotated_directly(
    transparent: Option<u8>,
    start: u8,
    len: u16,
    steps: i32,
    expect_change: bool,
) -> Image {
    let mut document = Document::new(image_f(transparent));
    let image = document.main_image();
    let changed = document
        .apply(Command::RotateColors {
            image,
            start,
            len,
            steps,
        })
        .expect("rotar");
    assert_eq!(changed, expect_change);
    main_image(&document).clone()
}

#[test]
fn fila_6_rotar_toda_la_paleta_es_igual_que_el_comando_directo() {
    for (steps, expected, notice) in [
        (1, [BLACK, RED, GREEN, BLUE], "paleta rotada a la derecha"),
        (
            -1,
            [GREEN, BLUE, BLACK, RED],
            "paleta rotada a la izquierda",
        ),
    ] {
        let mut session = session_f();
        select(&mut session, ColorChoice::Entry(2));
        let (start, len) = rotation_range(4, 2, session.ops.range_end());
        assert_eq!((start, len), (0, 4));

        let text = run_ok(&mut session, PaletteOp::Rotate { start, len, steps });

        assert_eq!(text, notice);
        assert_eq!(colors_of(img(&session)), expected, "steps {steps}");
        assert_eq!(img(&session), &rotated_directly(None, 0, 4, steps, true));
        assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3], "píxeles sin tocar");
        assert_eq!(
            session.choice,
            ColorChoice::Entry(2),
            "la selección se queda"
        );
        assert_eq!(session.active_pixel(), Some(Pixel::Indexed(2)));
    }
}

#[test]
fn fila_6_rotar_con_rango_conserva_la_seleccion_y_el_rango() {
    for (steps, expected, notice) in [
        (
            1,
            [RED, BLACK, GREEN, BLUE],
            "rotadas entradas 1 a 3 a la derecha",
        ),
        (
            -1,
            [RED, BLUE, BLACK, GREEN],
            "rotadas entradas 1 a 3 a la izquierda",
        ),
    ] {
        let mut session = session_f();
        select(&mut session, ColorChoice::Entry(1));
        session.ops.set_range_end(3);
        let (start, len) = rotation_range(4, 1, session.ops.range_end());
        assert_eq!((start, len), (1, 3));

        let text = run_ok(&mut session, PaletteOp::Rotate { start, len, steps });

        assert_eq!(text, notice);
        assert_eq!(colors_of(img(&session)), expected, "steps {steps}");
        assert_eq!(img(&session), &rotated_directly(None, 1, 3, steps, true));
        assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3]);
        assert_eq!(session.choice, ColorChoice::Entry(1), "la activa no cambia");
        assert_eq!(session.ops.range_end(), Some(3), "el rango se queda");

        // Se puede repetir.
        run_ok(&mut session, PaletteOp::Rotate { start, len, steps });
        assert_eq!(session.ops.range_end(), Some(3));
    }
}

#[test]
fn fila_6_con_la_transparente_dentro_del_rango_se_queda_en_su_ranura() {
    let mut session = Session::sample(Document::new(image_f(Some(3))));
    select(&mut session, ColorChoice::Entry(2));
    run_ok(
        &mut session,
        PaletteOp::Rotate {
            start: 0,
            len: 4,
            steps: 1,
        },
    );
    assert_eq!(transparent_of(img(&session)), Some(3));
    assert_eq!(img(&session), &rotated_directly(Some(3), 0, 4, 1, true));
}

#[test]
fn fila_6_rotar_colores_iguales_da_ok_false_con_aviso_y_no_toca_el_documento() {
    // Rotar un rango de dos entradas con colores iguales no hace nada.
    let mut session = Session::sample(Document::new(row(&[RED, RED, BLUE], None)));
    let before = img(&session).clone();
    let id = session.document.state_id();

    let outcome = session
        .apply_palette_op(
            PaletteOp::Rotate {
                start: 0,
                len: 2,
                steps: 1,
            },
            false,
        )
        .expect("Ok(false)");

    assert!(!outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("nada que rotar: colores iguales")
    );
    assert_eq!(img(&session), &before);
    assert_eq!(session.document.state_id(), id);
    assert!(!session.document.can_undo());
}

#[test]
fn fila_6_las_otras_operaciones_borran_el_rango_y_rotar_no() {
    let ops = [
        PaletteOp::Add,
        PaletteOp::Remove { index: 2, into: 0 },
        PaletteOp::Move { from: 0, to: 3 },
    ];
    for op in ops {
        let mut session = session_f();
        select(&mut session, ColorChoice::Entry(1));
        session.ops.set_range_end(3);
        run_ok(&mut session, op);
        assert_eq!(session.ops.range_end(), None, "{op:?} borra el rango");
    }

    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    session.ops.set_range_end(3);
    run_ok(
        &mut session,
        PaletteOp::Rotate {
            start: 1,
            len: 3,
            steps: 1,
        },
    );
    assert_eq!(session.ops.range_end(), Some(3));
}

#[test]
fn fila_6_un_clic_normal_que_cambia_la_eleccion_borra_el_rango() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    session.ops.set_range_end(3);
    session.set_choice_revealing(ColorChoice::Entry(0));
    assert_eq!(session.ops.range_end(), None);
    assert_eq!(session.choice, ColorChoice::Entry(0));
}

// ---------------------------------------------------------------------------------------
// Fila 7: mover.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_7_mover_de_0_a_2_reordena_selecciona_to_y_es_un_solo_paso() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(0));
    let initial = img(&session).clone();

    let notice = run_ok(&mut session, PaletteOp::Move { from: 0, to: 2 });

    assert_eq!(notice, "entrada 0 movida a la posición 2");
    assert_eq!(colors_of(img(&session)), [GREEN, BLUE, RED, BLACK]);
    assert_eq!(
        pixels_of(img(&session)),
        [2, 0, 1, 3],
        "la imagen no cambia"
    );
    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(2)));

    session.undo().expect("deshacer");
    assert_eq!(img(&session), &initial);
    assert!(!session.document.can_undo(), "un solo undo");
}

#[test]
fn fila_7_mover_a_la_misma_posicion_no_hace_nada() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    let before = img(&session).clone();
    let id = session.document.state_id();

    let outcome = session
        .apply_palette_op(PaletteOp::Move { from: 1, to: 1 }, false)
        .expect("Ok(false)");

    assert!(!outcome.changed);
    assert_eq!(img(&session), &before);
    assert_eq!(session.document.state_id(), id);
    assert!(!session.document.can_undo());
    assert_eq!(session.choice, ColorChoice::Entry(1));
}

// ---------------------------------------------------------------------------------------
// Fila 8: deshacer y rehacer.
// ---------------------------------------------------------------------------------------

/// La selección es válida y el color de dibujo sale de ella; `None` si todo está bien.
fn selection_problem(session: &Session) -> Option<String> {
    let len = len_of(session);
    let ColorChoice::Entry(i) = session.choice else {
        return Some(format!(
            "la elección no es una entrada: {:?}",
            session.choice
        ));
    };
    if usize::from(i) >= len {
        return Some(format!("Entry({i}) fuera de una paleta de {len}"));
    }
    if session.active_pixel() != Some(Pixel::Indexed(i)) {
        return Some(format!(
            "active_pixel {:?} con Entry({i})",
            session.active_pixel()
        ));
    }
    None
}

#[test]
fn fila_8_tras_deshacer_y_rehacer_la_seleccion_es_valida_y_el_rango_esta_vacio() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    session.ops.set_range_end(3);
    run_ok(
        &mut session,
        PaletteOp::Rotate {
            start: 1,
            len: 3,
            steps: 1,
        },
    );
    assert_eq!(session.ops.range_end(), Some(3));

    session.undo().expect("deshacer");
    assert_eq!(selection_problem(&session), None);
    assert_eq!(session.ops.range_end(), None, "deshacer borra el rango");

    session.ops.set_range_end(2);
    session.redo().expect("rehacer");
    assert_eq!(selection_problem(&session), None);
    assert_eq!(session.ops.range_end(), None, "rehacer borra el rango");
}

#[test]
fn fila_8_quitar_la_ultima_deshacer_no_persigue_y_rehacer_recorta_con_reveal() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(3));
    run_ok(&mut session, PaletteOp::Remove { index: 3, into: 2 });
    assert_eq!(session.choice, ColorChoice::Entry(2));

    session.view.reveal = false;
    session.undo().expect("deshacer");
    assert_eq!(len_of(&session), 4);
    assert_eq!(
        session.choice,
        ColorChoice::Entry(2),
        "no persigue a la entrada"
    );
    assert_eq!(selection_problem(&session), None);
    assert!(!session.view.reveal, "la elección no cambió");

    // Con la última elegida, rehacer la quita recorta y pide mostrar la fila.
    session.set_choice_revealing(ColorChoice::Entry(3));
    session.view.reveal = false;
    session.redo().expect("rehacer");
    assert_eq!(len_of(&session), 3);
    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(selection_problem(&session), None);
    assert!(session.view.reveal, "la elección cambió");
}

#[test]
fn fila_8_quitar_de_en_medio_y_mover_no_persiguen_a_la_entrada() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    run_ok(&mut session, PaletteOp::Remove { index: 1, into: 0 });
    assert_eq!(session.choice, ColorChoice::Entry(1));
    session.view.reveal = false;
    session.undo().expect("deshacer");
    assert_eq!(session.choice, ColorChoice::Entry(1));
    assert_eq!(selection_problem(&session), None);
    assert!(!session.view.reveal);
    session.redo().expect("rehacer");
    assert_eq!(session.choice, ColorChoice::Entry(1));
    assert_eq!(selection_problem(&session), None);

    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(0));
    run_ok(&mut session, PaletteOp::Move { from: 0, to: 2 });
    assert_eq!(session.choice, ColorChoice::Entry(2));
    session.view.reveal = false;
    session.undo().expect("deshacer");
    assert_eq!(
        session.choice,
        ColorChoice::Entry(2),
        "no persigue a la entrada"
    );
    assert_eq!(selection_problem(&session), None);
    assert!(!session.view.reveal);
    session.redo().expect("rehacer");
    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(colors_of(img(&session)), [GREEN, BLUE, RED, BLACK]);
    assert_eq!(selection_problem(&session), None);
}

#[test]
fn fila_8_undo_y_redo_sin_nada_que_hacer_dan_error_y_no_tocan_nada() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(2));
    let before = img(&session).clone();
    let id = session.document.state_id();
    assert!(matches!(session.undo(), Err(DocumentError::NothingToUndo)));
    assert!(matches!(session.redo(), Err(DocumentError::NothingToRedo)));
    assert_eq!(img(&session), &before);
    assert_eq!(session.document.state_id(), id);
    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert!(!session.view.reveal);
}

// ---------------------------------------------------------------------------------------
// Fila 9: quita pendiente y cambios del documento.
// ---------------------------------------------------------------------------------------

/// `P5` con `Entry(3)` y una quita pendiente (destino por defecto) ya sincronizada.
fn session_with_pending_removal() -> Session {
    let mut session = session_p5(None);
    select(&mut session, ColorChoice::Entry(3));
    assert!(press_remove(&mut session).is_none());
    assert!(session.ops.removal().is_some());
    session
}

#[test]
fn fila_9_sin_cambios_en_el_documento_la_quita_sigue_y_no_avisa() {
    let mut session = session_with_pending_removal();
    for _ in 0..3 {
        session.normalize();
    }
    assert_eq!(
        session.ops.removal(),
        Some(Removal {
            index: 3,
            into: Some(0)
        })
    );
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_9_deshacer_cancela_la_quita_y_avisa_una_sola_vez() {
    let mut session = session_p5(None);
    edit_color(&mut session, 4, BLACK);
    select(&mut session, ColorChoice::Entry(3));
    assert!(press_remove(&mut session).is_none());

    session.undo().expect("deshacer");

    assert_eq!(session.ops.removal(), None);
    assert!(session.take_removal_cancelled());
    assert!(!session.take_removal_cancelled(), "se lee una sola vez");
}

#[test]
fn fila_9_rehacer_cancela_la_quita() {
    let mut session = session_p5(None);
    edit_color(&mut session, 4, BLACK);
    session.undo().expect("deshacer");
    select(&mut session, ColorChoice::Entry(3));
    assert!(press_remove(&mut session).is_none());

    session.redo().expect("rehacer");

    assert_eq!(session.ops.removal(), None);
    assert!(session.take_removal_cancelled());
}

#[test]
fn fila_9_un_apply_directo_cancela_la_quita_al_normalizar() {
    let mut session = session_with_pending_removal();
    edit_color(&mut session, 0, BLACK);
    session.normalize();
    assert_eq!(session.ops.removal(), None);
    assert!(session.take_removal_cancelled());
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_9_abrir_otro_fichero_cancela_la_quita_y_avisa_una_vez() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_f(None));
    let mut session = session_with_pending_removal();

    session.open(&path, 16384, true).expect("abrir");

    assert_eq!(session.ops.removal(), None);
    assert!(session.take_removal_cancelled());
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_9_un_open_con_error_no_cancela_la_quita() {
    let tmp = TmpDir::new();
    let mut session = session_with_pending_removal();
    let result = session.open(&tmp.path().join("no-existe.png"), 16384, true);
    assert!(result.is_err());
    session.normalize();
    assert!(session.ops.removal().is_some());
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_9_abrir_sin_quita_pendiente_no_avisa_y_reinicia_el_rango() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_f(None));
    let mut session = session_f();
    session.ops.set_range_end(3);
    session.open(&path, 16384, true).expect("abrir");
    assert!(!session.take_removal_cancelled());
    assert_eq!(session.ops.range_end(), None, "open hace ops.reset()");
}

#[test]
fn fila_9_la_op_remove_de_la_propia_quita_no_deja_el_aviso() {
    let mut session = session_with_pending_removal();
    let outcome = confirm_removal(&mut session).expect("confirmar");
    assert!(outcome.changed);
    assert_eq!(session.ops.removal(), None);
    session.normalize();
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_9_otra_operacion_cancela_la_quita_y_deja_el_aviso() {
    let ops = [
        PaletteOp::Add,
        PaletteOp::Move { from: 3, to: 1 },
        PaletteOp::Rotate {
            start: 0,
            len: 5,
            steps: 1,
        },
    ];
    for op in ops {
        let mut session = session_with_pending_removal();
        run_ok(&mut session, op);
        session.normalize();
        assert_eq!(session.ops.removal(), None, "{op:?}");
        assert!(session.take_removal_cancelled(), "{op:?}");
        assert!(!session.take_removal_cancelled(), "{op:?}");
    }
}

#[test]
fn fila_9_una_operacion_que_no_cambia_nada_no_cancela_la_quita() {
    let mut session = session_with_pending_removal();
    let outcome = session
        .apply_palette_op(
            PaletteOp::Rotate {
                start: 0,
                len: 1,
                steps: 1,
            },
            false,
        )
        .expect("Ok(false)");
    assert!(!outcome.changed);
    session.normalize();
    assert!(session.ops.removal().is_some());
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_9_el_aviso_de_cancelacion_es_el_de_la_spec() {
    assert_eq!(
        REMOVAL_CANCELLED_NOTICE,
        "quitar cancelado: el documento cambió"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 10: texto pendiente del campo hexadecimal.
// ---------------------------------------------------------------------------------------

/// Sesión con 8 entradas distintas, todas usadas, `Entry(3)` y `ff0000` tecleado sin confirmar.
fn session_with_pending_text() -> Session {
    let mut session = Session::sample(Document::new(row(&distinct(8), None)));
    select(&mut session, ColorChoice::Entry(3));
    let base = colors_of(img(&session))[3];
    session
        .editor
        .type_hex(EditTarget::Entry(3), base, "ff0000".to_string());
    assert!(session.editor.has_pending());
    session
}

#[test]
fn fila_10_el_texto_pendiente_se_descarta_o_se_conserva_segun_el_destino_de_la_entrada() {
    // (operación, aviso de descarte o `None` si el texto se conserva, resumen).
    let cases: [(PaletteOp, Option<&str>, &str); 11] = [
        (
            PaletteOp::Move { from: 3, to: 5 },
            Some("texto descartado: la entrada se movió"),
            "entrada 3 movida a la posición 5",
        ),
        (
            PaletteOp::Move { from: 1, to: 4 },
            Some("texto descartado: la entrada se movió"),
            "entrada 1 movida a la posición 4",
        ),
        (
            PaletteOp::Move { from: 5, to: 0 },
            Some("texto descartado: la entrada se movió"),
            "entrada 5 movida a la posición 0",
        ),
        (
            PaletteOp::Move { from: 6, to: 7 },
            None,
            "entrada 6 movida a la posición 7",
        ),
        (
            PaletteOp::Remove { index: 3, into: 0 },
            Some("texto descartado: la entrada ya no existe"),
            "entrada 3 quitada; píxeles pasan a la 0",
        ),
        (
            PaletteOp::Remove { index: 1, into: 0 },
            Some("texto descartado: la entrada se movió"),
            "entrada 1 quitada; píxeles pasan a la 0",
        ),
        (
            PaletteOp::Remove { index: 5, into: 0 },
            None,
            "entrada 5 quitada; píxeles pasan a la 0",
        ),
        (
            PaletteOp::Rotate {
                start: 2,
                len: 3,
                steps: 1,
            },
            Some("texto descartado: el color cambió"),
            "rotadas entradas 2 a 4 a la derecha",
        ),
        (
            PaletteOp::Rotate {
                start: 5,
                len: 2,
                steps: 1,
            },
            None,
            "rotadas entradas 5 a 6 a la derecha",
        ),
        (PaletteOp::Add, None, "entrada 8 añadida (copia de la 3)"),
        (
            PaletteOp::Rotate {
                start: 0,
                len: 8,
                steps: -1,
            },
            Some("texto descartado: el color cambió"),
            "paleta rotada a la izquierda",
        ),
    ];

    for (op, stale, summary) in cases {
        let mut session = session_with_pending_text();
        let notice = run_ok(&mut session, op);
        match stale {
            Some(stale) => {
                assert!(!session.editor.has_pending(), "{op:?}: se descarta");
                assert_eq!(notice, format!("{stale} · {summary}"), "{op:?}");
            }
            None => {
                assert!(session.editor.has_pending(), "{op:?}: se conserva");
                assert_eq!(
                    session.editor.pending_target(),
                    Some(EditTarget::Entry(3)),
                    "{op:?}"
                );
                assert_eq!(notice, summary, "{op:?}");
            }
        }
        // Sin `Commit`: el documento tiene un solo paso, el de la operación.
        session.undo().expect("deshacer la operación");
        assert!(!session.document.can_undo(), "{op:?}: un solo paso");
    }
}

#[test]
fn fila_10_el_texto_descartado_no_se_aplica_al_documento() {
    let mut session = session_with_pending_text();
    let before = colors_of(img(&session));
    run_ok(&mut session, PaletteOp::Move { from: 3, to: 5 });
    assert!(!session.editor.has_pending());
    // Solo se movieron entradas: los mismos colores en otro orden, sin el tecleado.
    let after = colors_of(img(&session));
    let mut sorted_before = before.clone();
    let mut sorted_after = after;
    sorted_before.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    sorted_after.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    assert_eq!(sorted_before, sorted_after);
}

// ---------------------------------------------------------------------------------------
// Fila 11: límites.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_11_con_una_entrada_rotar_da_ok_false_y_anadir_da_dos() {
    let mut session = Session::sample(Document::new(indexed(&[RED], None, &[0])));
    let outcome = session
        .apply_palette_op(
            PaletteOp::Rotate {
                start: 0,
                len: 1,
                steps: 1,
            },
            false,
        )
        .expect("Ok(false)");
    assert!(!outcome.changed);
    assert_eq!(outcome.notice.as_deref(), Some(NOTHING_TO_ROTATE_NOTICE));
    assert!(!session.document.can_undo());

    run_ok(&mut session, PaletteOp::Add);
    assert_eq!(len_of(&session), 2);
}

#[test]
fn fila_11_con_256_entradas_mover_de_0_a_255_selecciona_la_255() {
    let mut session = Session::sample(Document::new(row(&distinct(256), None)));
    select(&mut session, ColorChoice::Entry(0));
    let first = colors_of(img(&session))[0];
    run_ok(&mut session, PaletteOp::Move { from: 0, to: 255 });
    assert_eq!(colors_of(img(&session))[255], first);
    assert_eq!(session.choice, ColorChoice::Entry(255));
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(255)));
}

#[test]
fn fila_11_la_transparente_en_0_se_lleva_con_mover_y_en_la_ultima_no_se_mueve_al_rotar() {
    let mut session = Session::sample(Document::new(image_f(Some(0))));
    select(&mut session, ColorChoice::Entry(0));
    run_ok(&mut session, PaletteOp::Move { from: 0, to: 3 });
    assert_eq!(transparent_of(img(&session)), Some(3));

    let mut session = Session::sample(Document::new(image_f(Some(3))));
    select(&mut session, ColorChoice::Entry(0));
    run_ok(
        &mut session,
        PaletteOp::Rotate {
            start: 0,
            len: 4,
            steps: 1,
        },
    );
    assert_eq!(transparent_of(img(&session)), Some(3));
}

#[test]
fn fila_11_un_rango_de_toda_la_paleta_da_los_mismos_pasos_que_sin_rango() {
    let mut with_range = session_f();
    select(&mut with_range, ColorChoice::Entry(0));
    with_range.ops.set_range_end(3);
    let (start, len) = rotation_range(4, 0, with_range.ops.range_end());
    assert_eq!((start, len), (0, 4));
    run_ok(
        &mut with_range,
        PaletteOp::Rotate {
            start,
            len,
            steps: 1,
        },
    );

    let mut without = session_f();
    select(&mut without, ColorChoice::Entry(0));
    let (start, len) = rotation_range(4, 0, None);
    run_ok(
        &mut without,
        PaletteOp::Rotate {
            start,
            len,
            steps: 1,
        },
    );

    assert_eq!(img(&with_range), img(&without));
}

// ---------------------------------------------------------------------------------------
// Fila 13: rotar sin nada que rotar.
// ---------------------------------------------------------------------------------------

/// Pasos de historia con `can_undo` y `can_redo` verdaderos: un `SetPaletteColor`, un `Add` y
/// su `undo`. Deja la paleta con `colors.len()` entradas.
fn with_undo_and_redo(colors: &[Rgba]) -> Session {
    let mut session = Session::sample(Document::new(row(colors, None)));
    edit_color(&mut session, 0, Rgba::opaque(1, 2, 3));
    run_ok(&mut session, PaletteOp::Add);
    session.undo().expect("deshacer el Add");
    assert!(session.document.can_undo());
    assert!(session.document.can_redo());
    session
}

fn assert_nothing_to_rotate(session: &mut Session, op: PaletteOp) {
    let before = img(session).clone();
    let id = session.document.state_id();
    let (choice, range) = (session.choice, session.ops.range_end());
    let view = session.view;

    let outcome = session.apply_palette_op(op, false).expect("Ok(false)");

    assert!(!outcome.changed, "{op:?}");
    assert_eq!(
        outcome.notice.as_deref(),
        Some("nada que rotar: una sola entrada"),
        "{op:?}"
    );
    assert_eq!(img(session), &before, "{op:?}");
    assert_eq!(session.document.state_id(), id, "{op:?}");
    assert!(session.document.can_undo(), "{op:?}");
    assert!(session.document.can_redo(), "{op:?}");
    assert_eq!(session.choice, choice, "{op:?}");
    assert_eq!(session.ops.range_end(), range, "{op:?}");
    assert_eq!(session.view, view, "{op:?}");
}

#[test]
fn fila_13_rotar_con_una_entrada_da_ok_false_sin_paso_y_con_aviso() {
    for steps in [1, -1] {
        let mut session = with_undo_and_redo(&[RED]);
        assert_eq!(len_of(&session), 1);
        assert_nothing_to_rotate(
            &mut session,
            PaletteOp::Rotate {
                start: 0,
                len: 1,
                steps,
            },
        );
    }
}

#[test]
fn fila_13_rotar_con_un_rango_de_una_entrada_da_ok_false_y_conserva_el_rango() {
    for steps in [1, -1] {
        let mut session = with_undo_and_redo(&[RED, GREEN, BLUE, BLACK]);
        select(&mut session, ColorChoice::Entry(2));
        session.ops.set_range_end(2);
        let (start, len) = rotation_range(4, 2, session.ops.range_end());
        assert_eq!((start, len), (2, 1));
        assert_nothing_to_rotate(&mut session, PaletteOp::Rotate { start, len, steps });
        assert_eq!(session.ops.range_end(), Some(2));
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: sesión indexada con secuencias de acciones.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Action {
    Add,
    /// `Quitar` en la activa: con quita pendiente, `Confirmar` o `Cancelar`; sin ella, empieza
    /// (o se aplica ya) con un destino aleatorio.
    Remove {
        dest: u8,
        confirm: bool,
    },
    Move {
        to: u8,
    },
    Rotate {
        right: bool,
    },
    ShiftClick(u8),
    Click(u8),
    Key(u8),
    Undo,
    Redo,
    SetColor {
        index: u8,
        color: (u8, u8, u8, u8),
    },
}

fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => Just(Action::Add),
        4 => (any::<u8>(), any::<bool>()).prop_map(|(dest, confirm)| Action::Remove { dest, confirm }),
        3 => any::<u8>().prop_map(|to| Action::Move { to }),
        3 => any::<bool>().prop_map(|right| Action::Rotate { right }),
        2 => any::<u8>().prop_map(Action::ShiftClick),
        2 => any::<u8>().prop_map(Action::Click),
        1 => (1u8..=9).prop_map(Action::Key),
        3 => Just(Action::Undo),
        2 => Just(Action::Redo),
        2 => (any::<u8>(), any::<(u8, u8, u8, u8)>())
            .prop_map(|(index, color)| Action::SetColor { index, color }),
    ]
}

/// Imagen indexada de 1 a 256 entradas, a veces con duplicados y transparente.
fn arb_image() -> impl Strategy<Value = Image> {
    (
        prop_oneof![8 => 1usize..=12, 1 => 250usize..=256],
        any::<bool>(),
        any::<Option<u8>>(),
        prop::collection::vec(any::<(u8, u8, u8, u8)>(), 256),
        prop::collection::vec(any::<u8>(), 1..=16),
    )
        .prop_map(|(len, duplicates, transparent, raw, pixels)| {
            let colors: Vec<Rgba> = (0..len)
                .map(|i| {
                    if duplicates {
                        let (r, g, b, a) = raw[i % 3];
                        Rgba::new(r, g, b, a)
                    } else {
                        let (_, g, b, a) = raw[i];
                        Rgba::new(i as u8, g, b, a)
                    }
                })
                .collect();
            let transparent = transparent.map(|t| (usize::from(t) % len) as u8);
            let pixels: Vec<u8> = pixels
                .iter()
                .map(|&p| (usize::from(p) % len) as u8)
                .collect();
            indexed(&colors, transparent, &pixels)
        })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    New,
    Undone,
    Redone,
    Unchanged,
}

struct Ran {
    kind: Kind,
    /// Con `Err` u `Ok(false)` de una operación, la selección y el rango tampoco cambian.
    selection_stays: bool,
    /// Era el `Remove` de la quita pendiente.
    own_confirm: bool,
}

fn ran(kind: Kind) -> Ran {
    Ran {
        kind,
        selection_stays: false,
        own_confirm: false,
    }
}

fn run_op(session: &mut Session, op: PaletteOp, own_confirm: bool) -> Ran {
    let unused = match op {
        PaletteOp::Remove { index, .. } => !uses_entry(img(session), index),
        _ => false,
    };
    match session.apply_palette_op(op, unused) {
        Ok(outcome) if outcome.changed => Ran {
            kind: Kind::New,
            selection_stays: false,
            own_confirm,
        },
        Ok(_) | Err(_) => Ran {
            kind: Kind::Unchanged,
            selection_stays: true,
            own_confirm: false,
        },
    }
}

fn run_action(session: &mut Session, action: &Action) -> Ran {
    let len = len_of(session);
    let active = entry_of(session);
    match *action {
        Action::Add => run_op(session, PaletteOp::Add, false),
        Action::Remove { dest, confirm } => {
            if let Some(removal) = session.ops.removal() {
                if confirm {
                    let into = removal.into.expect("la quita del test tiene destino");
                    run_op(
                        session,
                        PaletteOp::Remove {
                            index: removal.index,
                            into,
                        },
                        true,
                    )
                } else {
                    session.ops.cancel_removal();
                    ran(Kind::Unchanged)
                }
            } else if len < 2 {
                run_op(
                    session,
                    PaletteOp::Remove {
                        index: active,
                        into: active,
                    },
                    false,
                )
            } else {
                let mut into = usize::from(dest) % len;
                if into == usize::from(active) {
                    into = (into + 1) % len;
                }
                let into = into as u8;
                if uses_entry(img(session), active) {
                    session.ops.begin_removal(active, Some(into));
                    ran(Kind::Unchanged)
                } else {
                    run_op(
                        session,
                        PaletteOp::Remove {
                            index: active,
                            into,
                        },
                        false,
                    )
                }
            }
        }
        Action::Move { to } => run_op(
            session,
            PaletteOp::Move {
                from: active,
                to: (usize::from(to) % len) as u8,
            },
            false,
        ),
        Action::Rotate { right } => {
            let (start, span) = rotation_range(len, active, session.ops.range_end());
            run_op(
                session,
                PaletteOp::Rotate {
                    start,
                    len: span,
                    steps: if right { 1 } else { -1 },
                },
                false,
            )
        }
        Action::ShiftClick(cell) => {
            if session.ops.removal().is_none() {
                session.ops.set_range_end((usize::from(cell) % len) as u8);
            }
            ran(Kind::Unchanged)
        }
        Action::Click(cell) => {
            let cell = (usize::from(cell) % len) as u8;
            if session.ops.removal().is_some() {
                session.ops.set_destination(cell, len);
            } else {
                session.set_choice_revealing(ColorChoice::Entry(cell));
            }
            ran(Kind::Unchanged)
        }
        Action::Key(n) => {
            let index = usize::from(n) - 1;
            if session.ops.removal().is_none() && index < len {
                session.set_choice_revealing(ColorChoice::Entry(index as u8));
            }
            ran(Kind::Unchanged)
        }
        Action::Undo => match session.undo() {
            Ok(()) => ran(Kind::Undone),
            Err(_) => Ran {
                kind: Kind::Unchanged,
                selection_stays: false,
                own_confirm: false,
            },
        },
        Action::Redo => match session.redo() {
            Ok(()) => ran(Kind::Redone),
            Err(_) => Ran {
                kind: Kind::Unchanged,
                selection_stays: false,
                own_confirm: false,
            },
        },
        Action::SetColor { index, color } => {
            let (r, g, b, a) = color;
            let result = session.commit_edit(Commit {
                target: EditTarget::Entry((usize::from(index) % len) as u8),
                color: Rgba::new(r, g, b, a),
            });
            match result {
                Ok(true) => ran(Kind::New),
                Ok(false) | Err(_) => Ran {
                    kind: Kind::Unchanged,
                    selection_stays: true,
                    own_confirm: false,
                },
            }
        }
    }
}

/// La selección, el rango y la quita pendiente valen para la paleta actual.
fn check_invariants(session: &Session) -> Result<(), TestCaseError> {
    let len = len_of(session);
    if let Some(problem) = selection_problem(session) {
        return Err(TestCaseError::fail(problem));
    }
    if let Some(end) = session.ops.range_end() {
        prop_assert!(usize::from(end) < len, "rango {end} con {len} entradas");
    }
    if let Some(removal) = session.ops.removal() {
        prop_assert!(
            usize::from(removal.index) < len,
            "quita {removal:?} con {len} entradas"
        );
        if let Some(into) = removal.into {
            prop_assert!(usize::from(into) < len, "destino {removal:?} con {len}");
            prop_assert_ne!(into, removal.index);
        }
        prop_assert!(len >= 2, "una quita pendiente necesita dos entradas");
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn propiedad_4_sesion_indexada_con_acciones_mezcladas(
        initial in arb_image(),
        actions in prop::collection::vec(arb_action(), 0..=24),
    ) {
        let mut session = Session::sample(Document::new(initial.clone()));
        session.normalize();
        check_invariants(&session)?;
        let (mut undo_depth, mut redo_depth) = (0usize, 0usize);

        for action in &actions {
            let before = img(&session).clone();
            let id = session.document.state_id();
            let (can_undo, can_redo) = (session.document.can_undo(), session.document.can_redo());
            let (choice, range) = (session.choice, session.ops.range_end());
            let pending = session.ops.removal().is_some();

            let ran = run_action(&mut session, action);
            session.normalize();

            match ran.kind {
                Kind::New => {
                    undo_depth += 1;
                    redo_depth = 0;
                    prop_assert_ne!(session.document.state_id(), id, "{:?}", action);
                }
                Kind::Undone => {
                    undo_depth -= 1;
                    redo_depth += 1;
                }
                Kind::Redone => {
                    redo_depth -= 1;
                    undo_depth += 1;
                }
                Kind::Unchanged => {
                    prop_assert_eq!(img(&session), &before, "{:?}", action);
                    prop_assert_eq!(session.document.state_id(), id, "{:?}", action);
                    prop_assert_eq!(session.document.can_undo(), can_undo, "{:?}", action);
                    prop_assert_eq!(session.document.can_redo(), can_redo, "{:?}", action);
                    if ran.selection_stays {
                        prop_assert_eq!(session.choice, choice, "{:?}", action);
                        prop_assert_eq!(session.ops.range_end(), range, "{:?}", action);
                    }
                }
            }
            prop_assert_eq!(session.document.can_undo(), undo_depth > 0, "{:?}", action);
            prop_assert_eq!(session.document.can_redo(), redo_depth > 0, "{:?}", action);

            let changed = ran.kind != Kind::Unchanged;
            if changed {
                prop_assert!(session.ops.removal().is_none(), "{:?}", action);
            }
            prop_assert_eq!(
                session.take_removal_cancelled(),
                pending && changed && !ran.own_confirm,
                "aviso de quita cancelada tras {:?}",
                action
            );
            check_invariants(&session)?;
        }

        // Deshacer todo llega a la imagen inicial y rehacer todo, a la final.
        let last = img(&session).clone();
        let mut undone = 0usize;
        while session.undo().is_ok() {
            undone += 1;
            check_invariants(&session)?;
        }
        prop_assert_eq!(undone, undo_depth, "undos posibles");
        prop_assert_eq!(img(&session), &initial);
        for _ in 0..undo_depth {
            prop_assert!(session.redo().is_ok());
            check_invariants(&session)?;
        }
        prop_assert_eq!(img(&session), &last);
    }
}
