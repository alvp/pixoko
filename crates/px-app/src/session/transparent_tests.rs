//! Tests de `session` con el botón «Fijar/Quitar transparente» de la unidad 21
//! (specs/hito-2-fijar-transparente.md): filas 3, 4, 7, 8, 9 (la parte de `Session`) y 10, y
//! propiedades 2 (sesión) y 3 (guardado sin pérdida de aspecto). Sin ventana.
//!
//! Todo se escribe contra la API que aún no existe y debe estar en rojo hasta la lógica:
//! `PaletteOp::SetTransparent { index }`, `transparent_toggle`, `transparent_button_label`,
//! `info_flags_alpha0` e `is_alpha0_cell_with`. El botón se simula como el panel: el índice sale de
//! `transparent_toggle(palette, activa)` y se aplica con `Session::apply_palette_op`.
//!
//! Los ayudantes de los otros módulos de tests son privados: aquí se duplican los mínimos.

use super::*;

use proptest::prelude::*;
use px_core::{Command, DocumentError, Image, Mode, Palette, Pixel, Rgba};
use px_formats::{read_png, write_png};
use px_ui::color::{ColorChoice, SlotColors, info_flags_alpha0, is_alpha0_cell_with};
use px_ui::color_edit::{EditTarget, HexOutcome};
use px_ui::dirty::{BlockSet, dirty_of};
use px_ui::palette_ops::{
    PaletteOp, default_destination, rotation_range, transparent_button_label, transparent_toggle,
    uses_entry,
};
use px_ui::pick::eraser_label;
use px_ui::texture::to_rgba8;
use px_ui::view::Size;

use eframe::egui::{Context, Pos2, RawInput, Rect, vec2};

use crate::App;
use crate::new_image::initial_palette;

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);
/// `X` de `P5`.
const X: Rgba = Rgba::opaque(200, 30, 30);
/// `Y` de `P5`.
const Y: Rgba = Rgba::opaque(10, 10, 240);

const FIJAR: &str = "Fijar transparente";
const QUITAR: &str = "Quitar transparente";

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

fn session_f(transparent: Option<u8>) -> Session {
    Session::sample(Document::new(image_f(transparent)))
}

/// `P5`: `[R, G, B, X, Y]`, píxeles `[0, 1, 2, 3, 3]`.
fn session_p5() -> Session {
    Session::sample(Document::new(indexed(
        &[RED, GREEN, BLUE, X, Y],
        None,
        &[0, 1, 2, 3, 3],
    )))
}

fn img(session: &Session) -> &Image {
    session
        .document
        .image(session.document.main_image())
        .expect("imagen principal")
}

fn colors_of(image: &Image) -> Vec<Rgba> {
    image.palette().expect("indexada").colors().to_vec()
}

fn pixels_of(image: &Image) -> Vec<u8> {
    image.bytes().to_vec()
}

fn transparent_of(image: &Image) -> Option<u8> {
    image.palette().expect("indexada").transparent_index()
}

fn resolved(image: &Image, index: u8) -> Rgba {
    image
        .palette()
        .expect("indexada")
        .resolved(index)
        .expect("entrada")
}

fn len_of(session: &Session) -> usize {
    img(session).palette().expect("indexada").len()
}

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

/// Rótulo del botón para la activa de la sesión.
fn label_of(session: &Session) -> &'static str {
    let palette = img(session).palette().expect("indexada");
    transparent_button_label(palette, entry_of(session))
}

/// Pulsar el botón como el panel: el índice sale de `transparent_toggle` con la activa.
fn press_button(session: &mut Session) -> Result<PaletteOutcome, DocumentError> {
    let index = {
        let palette = img(session).palette().expect("indexada");
        transparent_toggle(palette, entry_of(session))
    };
    session.apply_palette_op(PaletteOp::SetTransparent { index }, false)
}

/// Pulsa el botón y exige `Ok(true)`; devuelve el aviso.
fn press_ok(session: &mut Session) -> String {
    let outcome = press_button(session).expect("el botón no da error");
    assert!(outcome.changed, "el botón debía cambiar el documento");
    outcome.notice.expect("el botón da un aviso")
}

/// Aplica `op` con `unused` como lo daría el panel.
fn run_ok(session: &mut Session, op: PaletteOp) -> String {
    let unused = match op {
        PaletteOp::Remove { index, .. } => !uses_entry(img(session), index),
        _ => false,
    };
    let outcome = session
        .apply_palette_op(op, unused)
        .expect("la operación no da error");
    assert!(outcome.changed, "{op:?} debía cambiar el documento");
    outcome.notice.expect("la operación da un aviso")
}

/// Pulsar `Quitar` (de la paleta) en la activa como el panel: con píxeles queda pendiente.
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

/// Un trozo `kind` del PNG (`PLTE`, `tRNS`...), sin cabecera ni CRC.
fn chunk<'a>(png: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let mut at = 8;
    while at + 8 <= png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().expect("4 bytes")) as usize;
        if &png[at + 4..at + 8] == kind {
            return Some(&png[at + 8..at + 8 + len]);
        }
        at += 12 + len;
    }
    None
}

fn slots() -> SlotColors {
    SlotColors::defaults(Mode::Indexed)
}

// ---------------------------------------------------------------------------------------
// Fila 3: fijar y quitar.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_fijar_mueve_solo_el_indice_en_un_paso_y_conserva_seleccion_y_rango() {
    let mut session = session_f(None);
    select(&mut session, ColorChoice::Entry(2));
    session.ops.set_range_end(3);
    let initial = img(&session).clone();
    let colors = colors_of(&initial);

    assert_eq!(label_of(&session), FIJAR);
    let palette = initial.palette().expect("indexada");
    assert_eq!(transparent_toggle(palette, 2), Some(2));
    assert!(!session.modified());

    let outcome = press_button(&mut session).expect("fijar");

    assert!(outcome.changed);
    assert_eq!(outcome.notice.as_deref(), Some("entrada 2 transparente"));
    assert_eq!(transparent_of(img(&session)), Some(2));
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3], "píxeles iguales");
    assert_eq!(colors_of(img(&session)), colors, "RGB y alfa guardados");
    assert_eq!(resolved(img(&session), 2), Rgba::new(0, 0, 255, 0));
    assert_eq!(resolved(img(&session), 1), GREEN, "las demás, como estaban");
    assert_eq!(session.choice, ColorChoice::Entry(2), "selección intacta");
    assert_eq!(session.ops.range_end(), Some(3), "rango conservado");
    assert!(session.modified());
    assert_eq!(label_of(&session), QUITAR, "el rótulo sale del estado");

    // Un solo paso.
    session.undo().expect("deshacer");
    assert_eq!(transparent_of(img(&session)), None);
    assert_eq!(img(&session), &initial, "la imagen original");
    assert!(!session.document.can_undo(), "un solo paso");
    assert!(!session.modified(), "el estado guardado");
    assert_eq!(label_of(&session), FIJAR);

    session.redo().expect("rehacer");
    assert_eq!(transparent_of(img(&session)), Some(2));
    assert!(session.modified());
    assert_eq!(label_of(&session), QUITAR);
}

#[test]
fn fila_3_quitar_deja_la_entrada_con_su_color_guardado_y_el_rotulo_sale_del_estado() {
    let mut session = session_f(Some(2));
    select(&mut session, ColorChoice::Entry(2));
    session.ops.set_range_end(3);
    let initial = img(&session).clone();

    assert_eq!(label_of(&session), QUITAR);
    let palette = initial.palette().expect("indexada");
    assert_eq!(transparent_toggle(palette, 2), None);
    assert_eq!(resolved(&initial, 2), Rgba::new(0, 0, 255, 0));

    let outcome = press_button(&mut session).expect("quitar");

    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 2 ya no es transparente")
    );
    assert_eq!(transparent_of(img(&session)), None);
    assert_eq!(colors_of(img(&session)), colors_of(&initial));
    assert_eq!(
        resolved(img(&session), 2),
        BLUE,
        "vuelve a su alfa guardado"
    );
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3]);
    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(session.ops.range_end(), Some(3));
    assert_eq!(label_of(&session), FIJAR);
    assert!(session.modified());

    session.undo().expect("deshacer");
    assert_eq!(transparent_of(img(&session)), Some(2));
    assert_eq!(img(&session), &initial);
    assert_eq!(
        label_of(&session),
        QUITAR,
        "tras deshacer, el rótulo cambia"
    );
    assert!(!session.document.can_undo());
    assert!(!session.modified());
}

#[test]
fn fila_3_poner_el_mismo_valor_da_ok_false_y_no_toca_nada() {
    let mut session = session_f(Some(2));
    select(&mut session, ColorChoice::Entry(2));
    let before = img(&session).clone();
    let id = session.document.state_id();

    let outcome = session
        .apply_palette_op(PaletteOp::SetTransparent { index: Some(2) }, false)
        .expect("Ok(false)");

    assert!(!outcome.changed);
    assert_eq!(img(&session), &before);
    assert_eq!(session.document.state_id(), id);
    assert!(!session.document.can_undo());
    assert!(!session.modified());
    assert_eq!(session.choice, ColorChoice::Entry(2));
}

/// Una edición previa de color (un paso de deshacer) en la entrada 0.
fn edit_first_color(session: &mut Session) {
    let image = session.document.main_image();
    let changed = session
        .document
        .apply(Command::SetPaletteColor {
            image,
            index: 0,
            color: Rgba::opaque(1, 2, 3),
        })
        .expect("editar color");
    assert!(changed);
}

#[test]
fn fila_3_el_mismo_valor_no_crea_paso_ni_marca_el_documento_ni_toca_el_historial() {
    // (transparente de partida, valor igual que se pide).
    for (start, same) in [(Some(2), Some(2)), (None, None)] {
        // `editada`: ya había un paso de color; `undone`: además se deshizo (hay rehacer).
        for (edited, undone) in [(false, false), (true, false), (true, true)] {
            let tag = format!("{start:?} editada {edited} deshecha {undone}");
            let mut session = session_f(start);
            select(&mut session, ColorChoice::Entry(1));
            session.ops.set_range_end(3);
            let original = img(&session).clone();
            if edited {
                edit_first_color(&mut session);
            }
            if undone {
                session.undo().expect("deshacer la edición");
                session.ops.set_range_end(3);
            }
            let before = img(&session).clone();
            let (id, bytes) = (
                session.document.state_id(),
                session.document.history_bytes(),
            );
            let (can_undo, can_redo, modified) = (
                session.document.can_undo(),
                session.document.can_redo(),
                session.modified(),
            );
            assert_eq!(modified, edited && !undone, "{tag}: punto de partida");

            let outcome = session
                .apply_palette_op(PaletteOp::SetTransparent { index: same }, false)
                .expect("Ok(false)");

            assert!(!outcome.changed, "{tag}");
            assert_eq!(img(&session), &before, "{tag}");
            assert_eq!(session.document.state_id(), id, "{tag}");
            assert_eq!(session.document.history_bytes(), bytes, "{tag}");
            assert_eq!(session.document.can_undo(), can_undo, "{tag}");
            assert_eq!(session.document.can_redo(), can_redo, "{tag}");
            assert_eq!(session.modified(), modified, "{tag}: modified igual");
            assert_eq!(session.choice, ColorChoice::Entry(1), "{tag}");
            assert_eq!(session.ops.range_end(), Some(3), "{tag}");

            // Un `Ctrl+Z` posterior deshace la edición anterior, no «el mismo valor».
            if edited && !undone {
                session.undo().expect("deshacer");
                assert_eq!(img(&session), &original, "{tag}: deshizo la edición");
                assert_eq!(transparent_of(img(&session)), start, "{tag}");
                assert!(!session.document.can_undo(), "{tag}: no había otro paso");
                assert!(!session.modified(), "{tag}");
            } else if undone {
                assert!(session.document.can_redo(), "{tag}");
                session.redo().expect("rehacer");
                assert_eq!(colors_of(img(&session))[0], Rgba::opaque(1, 2, 3), "{tag}");
            } else {
                assert!(
                    matches!(session.undo(), Err(DocumentError::NothingToUndo)),
                    "{tag}"
                );
            }
        }
    }
}

#[test]
fn fila_3_el_comando_directo_con_el_mismo_valor_da_ok_false_sin_paso_ni_marca() {
    for (start, same) in [(Some(2), Some(2)), (None, None)] {
        for edited in [false, true] {
            let tag = format!("{start:?} editada {edited}");
            let mut document = Document::new(image_f(start));
            let image = document.main_image();
            if edited {
                assert!(
                    document
                        .apply(Command::SetPaletteColor {
                            image,
                            index: 0,
                            color: Rgba::opaque(1, 2, 3),
                        })
                        .expect("editar color")
                );
            }
            let before = document.image(image).expect("imagen").clone();
            let (id, bytes, can_undo, can_redo) = (
                document.state_id(),
                document.history_bytes(),
                document.can_undo(),
                document.can_redo(),
            );

            let changed = document
                .apply(Command::SetTransparentIndex { image, index: same })
                .expect("Ok(false)");

            assert!(!changed, "{tag}");
            assert_eq!(document.image(image).expect("imagen"), &before, "{tag}");
            assert_eq!(document.state_id(), id, "{tag}");
            assert_eq!(document.history_bytes(), bytes, "{tag}");
            assert_eq!(document.can_undo(), can_undo, "{tag}");
            assert_eq!(document.can_redo(), can_redo, "{tag}");
            if edited {
                document.undo().expect("deshacer la edición");
                assert!(!document.can_undo(), "{tag}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 4: reemplazo.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_4_fijar_otra_reemplaza_la_marca_y_la_anterior_vuelve_a_su_alfa_guardado() {
    let mut session = session_f(Some(1));
    select(&mut session, ColorChoice::Entry(3));
    let initial = img(&session).clone();
    assert_eq!(resolved(&initial, 1), Rgba::new(0, 255, 0, 0));
    assert_eq!(label_of(&session), FIJAR);

    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 3 transparente (antes la 1)");
    assert_eq!(transparent_of(img(&session)), Some(3));
    assert_eq!(resolved(img(&session), 1), GREEN, "G vuelve a verse");
    assert_eq!(resolved(img(&session), 3), Rgba::new(0, 0, 0, 0));
    assert_eq!(colors_of(img(&session)), colors_of(&initial));
    assert_eq!(pixels_of(img(&session)), [0, 1, 2, 3]);

    // Un solo paso: un undo devuelve la marca anterior.
    session.undo().expect("deshacer");
    assert_eq!(transparent_of(img(&session)), Some(1));
    assert_eq!(img(&session), &initial);
    assert!(!session.document.can_undo());
}

#[test]
fn fila_4_si_la_anterior_tenia_alfa_guardado_0_sigue_con_alfa_0() {
    let clear_green = Rgba::new(0, 255, 0, 0);
    let mut session = Session::sample(Document::new(row(
        &[RED, clear_green, BLUE, BLACK],
        Some(1),
    )));
    select(&mut session, ColorChoice::Entry(3));

    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 3 transparente (antes la 1)");
    assert_eq!(transparent_of(img(&session)), Some(3));
    assert_eq!(resolved(img(&session), 1), clear_green, "sigue (0,255,0,0)");
}

#[test]
fn fila_4_con_tres_cifras_el_aviso_cabe_en_43_caracteres() {
    let mut session = Session::sample(Document::new(row(&distinct(256), Some(254))));
    select(&mut session, ColorChoice::Entry(255));

    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 255 transparente (antes la 254)");
    assert_eq!(notice.chars().count(), 39);
    assert!(notice.chars().count() <= 43);
    assert_eq!(transparent_of(img(&session)), Some(255));
}

// ---------------------------------------------------------------------------------------
// Fila 7: guardado.
// ---------------------------------------------------------------------------------------

/// Cuatro entradas opacas, píxeles `0, 1, 2, 3`, sin transparente.
fn opaque_four() -> Session {
    session_f(None)
}

#[test]
fn fila_7_fijar_la_1_guarda_trns_con_alfa_0_y_plte_intacta_y_al_reabrir_pierde_la_marca_no_el_aspecto()
 {
    let mut session = opaque_four();
    select(&mut session, ColorChoice::Entry(1));
    press_ok(&mut session);
    let before = img(&session).clone();

    let png = write_png(&before).expect("write_png");
    assert_eq!(chunk(&png, b"tRNS"), Some(&[255u8, 0][..]), "tRNS");
    assert_eq!(
        chunk(&png, b"PLTE"),
        Some(&[255u8, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0][..]),
        "PLTE con el RGB intacto"
    );

    let reopened = read_png(&png).expect("read_png").image;
    assert_eq!(transparent_of(&reopened), None, "la marca no vuelve");
    assert_eq!(colors_of(&reopened)[1], Rgba::new(0, 255, 0, 0), "alfa 0");
    assert_eq!(
        colors_of(&reopened)[0],
        RED,
        "las demás, con su alfa guardado"
    );
    assert_eq!(to_rgba8(&reopened), to_rgba8(&before), "mismo aspecto");

    // El panel y el botón tras reabrir.
    let palette = reopened.palette().expect("indexada");
    assert_eq!(transparent_button_label(palette, 1), FIJAR);
    assert_eq!(
        transparent_toggle(palette, 1),
        Some(1),
        "Fijar vuelve a poner la marca"
    );
    assert!(is_alpha0_cell_with(&reopened, 1, &slots()));
    assert!(!is_alpha0_cell_with(&reopened, 0, &slots()));
    assert_eq!(
        info_flags_alpha0(false, false, false, false, true),
        "alfa 0"
    );
    for cell in [0, 2, 3] {
        assert!(!is_alpha0_cell_with(&reopened, cell, &slots()), "{cell}");
    }
}

#[test]
fn fila_7_quitar_deja_un_png_sin_trns_y_la_imagen_reabierta_es_opaca() {
    let mut session = opaque_four();
    let original = img(&session).clone();
    select(&mut session, ColorChoice::Entry(1));
    press_ok(&mut session);
    let marked = write_png(img(&session)).expect("write_png");
    assert!(chunk(&marked, b"tRNS").is_some());

    press_ok(&mut session);
    assert_eq!(transparent_of(img(&session)), None);
    let png = write_png(img(&session)).expect("write_png");

    assert_eq!(chunk(&png, b"tRNS"), None, "sin tRNS");
    let reopened = read_png(&png).expect("read_png").image;
    assert_eq!(transparent_of(&reopened), None);
    assert!(colors_of(&reopened).iter().all(|color| color.a == 255));
    assert_eq!(to_rgba8(&reopened), to_rgba8(&original));
}

// ---------------------------------------------------------------------------------------
// Fila 8: nueva imagen.
// ---------------------------------------------------------------------------------------

fn session_new_indexed() -> Session {
    let mut session = session_f(None);
    let image = Image::new_indexed(8, 6, initial_palette()).expect("indexada");
    session.new_image(image, true).expect("nueva imagen");
    session
}

#[test]
fn fila_8_en_la_imagen_nueva_la_0_dice_quitar_y_al_pulsarlo_el_aspecto_no_cambia() {
    let mut session = session_new_indexed();
    select(&mut session, ColorChoice::Entry(0));
    let initial = img(&session).clone();
    assert_eq!(transparent_of(&initial), Some(0));
    assert_eq!(label_of(&session), QUITAR);
    let before = to_rgba8(&initial);
    assert!(
        before.chunks(4).all(|pixel| pixel[3] == 0),
        "todo transparente"
    );

    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 0 ya no es transparente");
    let image = img(&session);
    assert_eq!(transparent_of(image), None);
    assert_eq!(colors_of(image)[0], Rgba::new(0, 0, 0, 0), "#00000000");
    assert_eq!(to_rgba8(image), before, "todo sigue igual de transparente");
    assert_eq!(label_of(&session), FIJAR);
    assert!(is_alpha0_cell_with(image, 0, &slots()), "dice alfa 0");
    assert_eq!(
        info_flags_alpha0(false, false, false, false, true),
        "alfa 0"
    );
    assert_eq!(
        eraser_label(image),
        "transparente (entrada 0)",
        "el borrador usa la segunda rama"
    );

    session.undo().expect("deshacer");
    assert_eq!(transparent_of(img(&session)), Some(0));
    assert_eq!(img(&session), &initial);
    assert_eq!(label_of(&session), QUITAR);
}

#[test]
fn fila_8_con_la_25_negro_activa_el_boton_dice_fijar() {
    let session = session_new_indexed();
    assert_eq!(session.choice, ColorChoice::Entry(25), "el negro");
    assert_eq!(label_of(&session), FIJAR);
}

// ---------------------------------------------------------------------------------------
// Fila 9: texto pendiente y quita pendiente (la parte de `Session`).
// ---------------------------------------------------------------------------------------

/// Sesión con 8 entradas distintas, `Entry(3)` y `ff0000` tecleado sin confirmar.
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
fn fila_9_el_texto_pendiente_no_se_descarta_al_aplicar_la_marca() {
    let mut session = session_with_pending_text();

    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 3 transparente", "sin «texto descartado»");
    assert!(session.editor.has_pending(), "entry_fate = Same");
    assert_eq!(session.editor.pending_target(), Some(EditTarget::Entry(3)));
    assert!(!session.take_discarded_text());
    // Sin Commit, el documento tiene un solo paso.
    session.undo().expect("deshacer la marca");
    assert!(!session.document.can_undo());
}

#[test]
fn fila_9_con_el_campo_enfocado_el_commit_llega_antes_y_son_dos_pasos_en_ese_orden() {
    let mut session = session_with_pending_text();
    let original = colors_of(img(&session))[3];

    // Como el panel: el clic quita el foco, se confirma el texto (`Commit`) y luego se aplica.
    let HexOutcome::Commit(commit) = session.editor.confirm_hex(Mode::Indexed, true) else {
        panic!("el texto es válido y distinto del color base");
    };
    assert!(session.commit_edit(commit).expect("commit"));
    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 3 transparente");
    assert!(!session.editor.has_pending(), "no queda texto");
    assert_eq!(colors_of(img(&session))[3], Rgba::opaque(255, 0, 0));
    assert_eq!(transparent_of(img(&session)), Some(3));

    session.undo().expect("deshacer la marca");
    assert_eq!(transparent_of(img(&session)), None, "primero la marca");
    assert_eq!(colors_of(img(&session))[3], Rgba::opaque(255, 0, 0));
    session.undo().expect("deshacer el color");
    assert_eq!(colors_of(img(&session))[3], original, "después el color");
    assert!(!session.document.can_undo(), "dos pasos exactos");
}

#[test]
fn fila_9_con_quita_pendiente_se_aplica_la_marca_y_la_quita_se_cancela_con_aviso() {
    let mut session = session_p5();
    select(&mut session, ColorChoice::Entry(3));
    session.ops.set_range_end(4);
    assert!(press_remove(&mut session).is_none(), "queda pendiente");
    assert!(session.ops.removal().is_some());
    let id = session.document.state_id();

    let notice = press_ok(&mut session);

    assert_eq!(notice, "entrada 3 transparente");
    assert_ne!(session.document.state_id(), id);
    assert_eq!(transparent_of(img(&session)), Some(3));
    assert_eq!(session.ops.removal(), None, "la quita se cancela");
    assert!(session.take_removal_cancelled(), "con el aviso de la spec");
    assert!(!session.take_removal_cancelled(), "una sola vez");
    assert_eq!(
        px_ui::palette_ops::REMOVAL_CANCELLED_NOTICE,
        "quitar cancelado: el documento cambió"
    );
    assert_eq!(session.choice, ColorChoice::Entry(3), "selección intacta");
    assert_eq!(session.ops.range_end(), Some(4), "rango intacto");
    // Tras normalizar en el fotograma siguiente no vuelve a avisar.
    session.normalize();
    assert!(!session.take_removal_cancelled());
}

// ---------------------------------------------------------------------------------------
// Fila 10: límites, rango, deshacer y textura.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_con_una_entrada_fijar_y_quitar_funcionan_y_todo_queda_transparente() {
    let mut session = Session::sample(Document::new(indexed(&[RED], None, &[0, 0])));
    assert_eq!(len_of(&session), 1);
    assert_eq!(label_of(&session), FIJAR);

    let notice = press_ok(&mut session);
    assert_eq!(notice, "entrada 0 transparente");
    assert_eq!(transparent_of(img(&session)), Some(0));
    assert!(to_rgba8(img(&session)).chunks(4).all(|p| p[3] == 0));
    assert_eq!(label_of(&session), QUITAR);

    let notice = press_ok(&mut session);
    assert_eq!(notice, "entrada 0 ya no es transparente");
    assert_eq!(transparent_of(img(&session)), None);
    assert!(to_rgba8(img(&session)).chunks(4).all(|p| p[3] == 255));
}

#[test]
fn fila_10_con_256_entradas_fijar_en_la_255_y_quitar_la_marcada_en_0_y_en_la_ultima() {
    let mut session = Session::sample(Document::new(row(&distinct(256), None)));
    select(&mut session, ColorChoice::Entry(255));
    assert_eq!(press_ok(&mut session), "entrada 255 transparente");
    assert_eq!(transparent_of(img(&session)), Some(255));
    assert_eq!(label_of(&session), QUITAR);
    assert_eq!(press_ok(&mut session), "entrada 255 ya no es transparente");
    assert_eq!(transparent_of(img(&session)), None);

    for marked in [0u8, 255] {
        let mut session = Session::sample(Document::new(row(&distinct(256), Some(marked))));
        select(&mut session, ColorChoice::Entry(marked));
        assert_eq!(label_of(&session), QUITAR, "marcada {marked}");
        press_ok(&mut session);
        assert_eq!(transparent_of(img(&session)), None, "marcada {marked}");
        assert_eq!(label_of(&session), FIJAR, "marcada {marked}");
    }
}

#[test]
fn fila_10_el_rango_marcado_se_conserva_al_fijar_y_al_quitar() {
    let mut session = session_f(None);
    select(&mut session, ColorChoice::Entry(1));
    session.ops.set_range_end(3);

    press_ok(&mut session);
    assert_eq!(session.ops.range_end(), Some(3), "al fijar");
    assert_eq!(session.choice, ColorChoice::Entry(1));
    press_ok(&mut session);
    assert_eq!(session.ops.range_end(), Some(3), "al quitar");

    // El rango sigue valiendo para rotar: la marca no lo borró.
    let (start, len) = rotation_range(4, 1, session.ops.range_end());
    assert_eq!((start, len), (1, 3));
    run_ok(
        &mut session,
        PaletteOp::Rotate {
            start,
            len,
            steps: 1,
        },
    );
    assert_eq!(session.ops.range_end(), Some(3));
}

#[test]
fn fila_10_undo_y_redo_con_la_elegida_mas_alla_del_final_recortan_y_el_rotulo_es_el_del_estado() {
    let mut session = session_f(None);
    select(&mut session, ColorChoice::Entry(3));
    run_ok(&mut session, PaletteOp::Add);
    assert_eq!(len_of(&session), 5);
    assert_eq!(session.choice, ColorChoice::Entry(4));
    press_ok(&mut session);
    assert_eq!(transparent_of(img(&session)), Some(4));
    assert_eq!(label_of(&session), QUITAR);

    session.undo().expect("deshacer la marca");
    assert_eq!(transparent_of(img(&session)), None);
    assert_eq!(session.choice, ColorChoice::Entry(4));
    assert_eq!(label_of(&session), FIJAR);

    session.undo().expect("deshacer el Add");
    assert_eq!(len_of(&session), 4);
    assert_eq!(session.choice, ColorChoice::Entry(3), "recortada");
    assert_eq!(label_of(&session), FIJAR);

    session.redo().expect("rehacer el Add");
    assert_eq!(len_of(&session), 5);
    assert_eq!(session.choice, ColorChoice::Entry(3));

    session.redo().expect("rehacer la marca");
    assert_eq!(transparent_of(img(&session)), Some(4));
    assert_eq!(
        session.choice,
        ColorChoice::Entry(3),
        "la elección se queda"
    );
    assert_eq!(label_of(&session), FIJAR, "la activa (3) no es la marcada");
    select(&mut session, ColorChoice::Entry(4));
    assert_eq!(label_of(&session), QUITAR, "la activa (4) es la marcada");
}

/// Imagen indexada grande (varios bloques) con la paleta de `F`.
fn big_session() -> Session {
    let palette = Palette::from_colors([RED, GREEN, BLUE, BLACK]).expect("paleta");
    let image = Image::new_indexed(300, 200, palette).expect("imagen grande");
    Session::sample(Document::new(image))
}

#[test]
fn fila_10_dirty_of_de_set_transparent_index_es_la_imagen_entera() {
    let session = big_session();
    let id = session.document.main_image();
    let size = Size::new(300, 200);
    let full = BlockSet::full(300, 200);
    assert!(!full.is_empty());
    for index in [Some(0), Some(3), None] {
        let command = Command::SetTransparentIndex { image: id, index };
        assert_eq!(dirty_of(&command, size), full, "{index:?}");
    }
}

#[test]
fn fila_10_pulsar_anota_toda_la_imagen_en_el_registro_y_undo_y_redo_la_invalidan() {
    let mut session = big_session();
    let full = BlockSet::full(300, 200);
    select(&mut session, ColorChoice::Entry(2));
    // Se vacía lo anotado antes de la prueba.
    let _ = session.take_dirty();

    press_ok(&mut session);
    let state = session.document.state_id();
    assert_eq!(session.change_log().get(state), Some(&full));
    assert_eq!(
        session.take_dirty(),
        full,
        "una sola región: toda la imagen"
    );

    session.undo().expect("deshacer");
    assert_eq!(session.take_dirty(), full, "deshacer invalida la imagen");
    session.redo().expect("rehacer");
    assert_eq!(session.take_dirty(), full, "rehacer, también");
}

/// Un fotograma del lienzo de la app (como el del banco de `tiles_tests`) que devuelve los
/// cambios de textura de las baldosas de la imagen: tamaño y alfa de cada píxel de cada subida.
fn frame_uploads(app: &mut App, ctx: &Context) -> Vec<([usize; 2], Vec<u8>)> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(160.0, 120.0))),
        max_texture_side: Some(1024),
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        app.canvas.interact_ordered(ui);
        let id = app.session.document.main_image();
        let image = app.session.document.image(id).expect("imagen");
        let _ = app.canvas.sync(ui.ctx(), image);
        let _ = app.canvas.paint(ui);
    });
    let ids = app.canvas.texture_ids();
    let mut uploads = Vec::new();
    for (id, deltas) in &output.textures_delta.set {
        if !ids.contains(id) {
            continue;
        }
        for delta in deltas {
            let eframe::egui::ImageData::Color(color) = &delta.image;
            uploads.push((
                color.size,
                color.pixels.iter().map(|pixel| pixel.a()).collect(),
            ));
        }
    }
    output.textures_delta.clear();
    uploads
}

#[test]
fn fila_10_pulsar_sube_una_sola_textura_con_el_alfa_0_y_el_siguiente_fotograma_ninguna() {
    let mut app = App::new(Document::new(image_f(None)), None);
    let ctx = Context::default();
    frame_uploads(&mut app, &ctx);
    frame_uploads(&mut app, &ctx);
    app.session.choice = ColorChoice::Entry(2);
    app.session.sync_editor();
    assert!(
        frame_uploads(&mut app, &ctx).is_empty(),
        "sin cambios no se sube nada"
    );

    app.apply_palette_op(&ctx, PaletteOp::SetTransparent { index: Some(2) }, false);
    let uploads = frame_uploads(&mut app, &ctx);

    assert_eq!(uploads.len(), 1, "una sola subida: {uploads:?}");
    assert_eq!(uploads[0].0, [4, 1]);
    assert_eq!(uploads[0].1, [255, 255, 0, 255], "el píxel 2 con alfa 0");
    assert!(
        frame_uploads(&mut app, &ctx).is_empty(),
        "ninguna subida más"
    );

    // Quitar la marca vuelve a subir una sola vez, con el píxel opaco.
    app.apply_palette_op(&ctx, PaletteOp::SetTransparent { index: None }, false);
    let uploads = frame_uploads(&mut app, &ctx);
    assert_eq!(uploads.len(), 1, "{uploads:?}");
    assert_eq!(uploads[0].1, [255, 255, 255, 255]);
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: sesión indexada con secuencias de acciones.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Action {
    /// El botón nuevo: `Fijar` o `Quitar` según el estado.
    Toggle,
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
    Undo,
    Redo,
}

fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        6 => Just(Action::Toggle),
        3 => Just(Action::Add),
        4 => (any::<u8>(), any::<bool>()).prop_map(|(dest, confirm)| Action::Remove { dest, confirm }),
        3 => any::<u8>().prop_map(|to| Action::Move { to }),
        3 => any::<bool>().prop_map(|right| Action::Rotate { right }),
        2 => any::<u8>().prop_map(Action::ShiftClick),
        3 => any::<u8>().prop_map(Action::Click),
        3 => Just(Action::Undo),
        2 => Just(Action::Redo),
    ]
}

/// Imagen indexada de 1 a 256 entradas, a veces con duplicados y con transparente.
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
    /// Era el `Remove` de la quita pendiente.
    own_confirm: bool,
}

fn ran(kind: Kind) -> Ran {
    Ran {
        kind,
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
            own_confirm,
        },
        Ok(_) | Err(_) => ran(Kind::Unchanged),
    }
}

fn run_action(session: &mut Session, action: &Action) -> Ran {
    let len = len_of(session);
    let active = entry_of(session);
    match *action {
        Action::Toggle => {
            let index = {
                let palette = img(session).palette().expect("indexada");
                transparent_toggle(palette, active)
            };
            run_op(session, PaletteOp::SetTransparent { index }, false)
        }
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
        Action::Undo => match session.undo() {
            Ok(()) => ran(Kind::Undone),
            Err(_) => ran(Kind::Unchanged),
        },
        Action::Redo => match session.redo() {
            Ok(()) => ran(Kind::Redone),
            Err(_) => ran(Kind::Unchanged),
        },
    }
}

/// La invariante de la spec, con un oráculo propio: el índice transparente es `None` o `< len`,
/// el rótulo coincide con «la activa es la marcada» y la elección es una entrada válida.
fn check_invariants(session: &Session) -> Result<(), TestCaseError> {
    let image = img(session);
    let len = len_of(session);
    let transparent = transparent_of(image);
    if let Some(index) = transparent {
        prop_assert!(usize::from(index) < len, "transparente {index} con {len}");
    }
    let ColorChoice::Entry(active) = session.choice else {
        return Err(TestCaseError::fail(format!(
            "la elección no es una entrada: {:?}",
            session.choice
        )));
    };
    prop_assert!(usize::from(active) < len, "Entry({active}) con {len}");
    let expected = if transparent == Some(active) {
        QUITAR
    } else {
        FIJAR
    };
    prop_assert_eq!(label_of(session), expected);
    if let Some(end) = session.ops.range_end() {
        prop_assert!(usize::from(end) < len, "rango {end} con {len} entradas");
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn propiedad_2_sesion_con_el_boton_nuevo_mezclado_con_las_demas_acciones(
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
            let active = entry_of(&session);
            let was = transparent_of(&before);

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
                }
            }
            prop_assert_eq!(session.document.can_undo(), undo_depth > 0, "{:?}", action);
            prop_assert_eq!(session.document.can_redo(), redo_depth > 0, "{:?}", action);

            if matches!(action, Action::Toggle) {
                // El botón nunca da `Ok(false)` ni `Err`: siempre es un paso nuevo.
                prop_assert_eq!(ran.kind, Kind::New);
                let expected = if was == Some(active) { None } else { Some(active) };
                let after = img(&session);
                prop_assert_eq!(transparent_of(after), expected);
                // Solo cambió el índice: píxeles y colores guardados iguales.
                prop_assert_eq!(after.bytes(), before.bytes());
                prop_assert_eq!(colors_of(after), colors_of(&before));
                // La selección y el rango, intactos.
                prop_assert_eq!(session.choice, choice);
                prop_assert_eq!(session.ops.range_end(), range);
            }

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

// ---------------------------------------------------------------------------------------
// Propiedad 3: guardado sin pérdida de aspecto.
// ---------------------------------------------------------------------------------------

/// Imagen indexada de hasta 8×8 con paleta de 1 a 256 entradas, alfas cualesquiera y con o sin
/// índice transparente.
fn arb_saved_image() -> impl Strategy<Value = Image> {
    (
        1u32..=8,
        1u32..=8,
        prop_oneof![8 => 1usize..=16, 2 => 17usize..=256],
        any::<Option<u8>>(),
        prop::collection::vec(any::<(u8, u8, u8, u8)>(), 256),
        prop::collection::vec(any::<u8>(), 64),
    )
        .prop_map(|(width, height, len, transparent, raw, seeds)| {
            let colors: Vec<Rgba> = raw[..len]
                .iter()
                .enumerate()
                .map(|(i, &(_, g, b, a))| {
                    // Una de cada cuatro entradas con alfa 0 para que el caso se dé a menudo.
                    let a = if i % 4 == 3 { 0 } else { a };
                    Rgba::new(i as u8, g, b, a)
                })
                .collect();
            let transparent = transparent.map(|t| (usize::from(t) % len) as u8);
            let pixels: Vec<u8> = seeds[..(width * height) as usize]
                .iter()
                .map(|&p| (usize::from(p) % len) as u8)
                .collect();
            let mut palette = Palette::from_colors(colors).expect("paleta");
            palette
                .set_transparent_index(transparent)
                .expect("índice transparente");
            let mut image = Image::new_indexed(width, height, palette).expect("imagen");
            for (n, &pixel) in pixels.iter().enumerate() {
                let (x, y) = (n as u32 % width, n as u32 / width);
                image.set_pixel(x, y, Pixel::Indexed(pixel)).expect("píxel");
            }
            image
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn propiedad_3_guardar_y_reabrir_conserva_el_aspecto_y_el_panel_marca_alfa_0(
        image in arb_saved_image(),
    ) {
        let saved = colors_of(&image);
        let transparent = transparent_of(&image);
        // Oráculo propio del alfa resuelto de cada entrada.
        let expected: Vec<Rgba> = saved
            .iter()
            .enumerate()
            .map(|(i, color)| {
                if transparent == Some(i as u8) {
                    Rgba::new(color.r, color.g, color.b, 0)
                } else {
                    *color
                }
            })
            .collect();

        let png = write_png(&image).expect("write_png");
        let reopened = read_png(&png).expect("read_png").image;

        prop_assert_eq!(to_rgba8(&reopened), to_rgba8(&image), "mismo aspecto");
        prop_assert_eq!(transparent_of(&reopened), None);
        prop_assert_eq!(colors_of(&reopened), expected.clone());

        let palette = reopened.palette().expect("indexada");
        for (i, color) in expected.iter().enumerate() {
            let cell = i;
            prop_assert_eq!(
                transparent_button_label(palette, i as u8),
                FIJAR,
                "entrada {}", i
            );
            let alpha0 = color.a == 0;
            prop_assert_eq!(
                is_alpha0_cell_with(&reopened, cell, &slots()),
                alpha0,
                "entrada {}", i
            );
            prop_assert_eq!(
                info_flags_alpha0(false, false, false, false, alpha0),
                if alpha0 { "alfa 0" } else { "" },
                "entrada {}", i
            );
        }
    }
}
