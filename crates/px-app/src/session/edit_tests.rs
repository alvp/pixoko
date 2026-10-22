//! Tests de la edición de color en `session` (specs/hito-2-editar-color.md): filas 1 (parte de
//! `commit_edit`), 4, 13, 14 y 15, y propiedad 4b (pasos de deshacer de una sesión indexada).
//!
//! Los ayudantes de `tests.rs` son privados de ese módulo: aquí se duplican los mínimos. Cada test
//! usa su propio directorio temporal y nunca escribe dentro de `golden/`.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{Command, DocumentError, Image, Mode, Palette, PaletteEditError, Pixel, PixelEdit};
use px_core::{Hsl, Rgba};
use px_formats::write_png;
use px_ui::canvas::CanvasEvent;
use px_ui::color::{ColorChoice, SlotBank, SlotColors, cell_label_with, initial_choice};
use px_ui::color_edit::{Commit, Control, EditTarget, HexOutcome, control_value, hex_text};
use px_ui::palette_grid::Scroll;
use px_ui::texture::{to_rgba8, to_rgba8_with_entry};
use px_ui::theme::NOTICE_PREFIX;
use px_ui::tool::ToolState;

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, imágenes y sesiones.
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
            std::env::temp_dir().join(format!("pixoko-px-app-edit-{}-{n}", std::process::id()));
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

/// Imagen pequeña y variada en el modo pedido (copia de `tests.rs`).
fn sample_image(mode: Mode) -> Image {
    let (w, h) = (5u32, 4u32);
    let mut image = match mode {
        Mode::Indexed => {
            let palette = Palette::from_colors([
                Rgba::opaque(0, 0, 0),
                Rgba::opaque(255, 0, 0),
                Rgba::opaque(0, 255, 0),
                Rgba::new(0, 0, 255, 128),
            ])
            .expect("paleta");
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        other => Image::new(w, h, other).expect("imagen"),
    };
    for y in 0..h {
        for x in 0..w {
            let n = (y * w + x) as u8;
            let pixel = match mode {
                Mode::Rgba => {
                    Pixel::Rgba(Rgba::new(n.wrapping_mul(13), n.wrapping_mul(7), n, 255 - n))
                }
                Mode::Indexed => Pixel::Indexed(n % 4),
                Mode::Gray => Pixel::Gray(n.wrapping_mul(11)),
                Mode::GrayAlpha => Pixel::GrayAlpha {
                    gray: n.wrapping_mul(11),
                    alpha: 255 - n,
                },
            };
            image.set_pixel(x, y, pixel).expect("píxel");
        }
    }
    image
}

/// Escribe `image` como PNG en `dir/name` y devuelve la ruta.
fn write_image(dir: &Path, name: &str, image: &Image) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, write_png(image).expect("write_png")).expect("escribir el PNG");
    path
}

fn main_image(document: &Document) -> &Image {
    document
        .image(ImageId::from_raw(0))
        .expect("imagen principal")
}

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);
const YELLOW: Rgba = Rgba::opaque(255, 255, 0);

/// Indexada con esa paleta y los píxeles `0, 1, 2, ...` (uno por entrada, en una fila).
fn indexed_row(palette: Palette) -> Image {
    let len = palette.len() as u32;
    let mut image = Image::new_indexed(len, 1, palette).expect("imagen indexada");
    for x in 0..len {
        image
            .set_pixel(x, 0, Pixel::Indexed(x as u8))
            .expect("píxel");
    }
    image
}

/// `F` de la spec: indexada 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`, sin transparente.
fn image_f() -> Image {
    indexed_row(Palette::from_colors([RED, GREEN, BLUE, BLACK]).expect("paleta F"))
}

fn session_f() -> Session {
    Session::sample(Document::new(image_f()))
}

/// Elige `choice` y pone al día el editor, como hace el fotograma siguiente.
fn select(session: &mut Session, choice: ColorChoice) {
    session.choice = choice;
    session.sync_editor();
}

fn palette_colors(session: &Session) -> Vec<Rgba> {
    let palette = main_image(&session.document)
        .palette()
        .expect("imagen indexada");
    (0..palette.len())
        .map(|i| palette.get(i as u8).expect("entrada"))
        .collect()
}

/// Un arrastre completo con el editor de la sesión: `begin`, `drag` por cada valor y `release`.
fn drag_and_release(
    session: &mut Session,
    control: Control,
    start: u32,
    moves: &[u32],
) -> Option<Commit> {
    assert!(session.editor.begin(control, start), "begin rechazado");
    for &value in moves {
        session.editor.drag(value);
    }
    session.editor.release()
}

/// R, G, B y H que muestra el editor, como los pinta el panel.
fn shown_values(session: &Session) -> (u32, u32, u32, u32) {
    let (rgba, hsl) = session.editor.shown().expect("el editor muestra un color");
    (
        control_value(rgba, hsl, Control::Red),
        control_value(rgba, hsl, Control::Green),
        control_value(rgba, hsl, Control::Blue),
        control_value(rgba, hsl, Control::Hue),
    )
}

// ---------------------------------------------------------------------------------------
// Fila 1: `commit_edit` con una entrada fuera de la paleta.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_commit_con_entrada_fuera_de_la_paleta_da_error_y_no_toca_el_documento() {
    let mut session = session_f();
    let image_before = main_image(&session.document).clone();
    let id_before = session.document.state_id();
    let bytes_before = session.document.history_bytes();

    let result = session.commit_edit(Commit {
        target: EditTarget::Entry(9),
        color: Rgba::opaque(1, 2, 3),
    });

    assert!(
        matches!(
            result,
            Err(DocumentError::PaletteEdit(
                PaletteEditError::IndexOutOfRange { index: 9, len: 4 }
            ))
        ),
        "resultado: {result:?}"
    );
    assert_eq!(main_image(&session.document), &image_before);
    assert_eq!(session.document.state_id(), id_before);
    assert_eq!(session.document.history_bytes(), bytes_before);
    assert!(!session.document.can_undo());
    assert!(!session.modified());
}

// ---------------------------------------------------------------------------------------
// Fila 4: un arrastre, un paso.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_4_un_arrastre_es_un_solo_paso_y_el_documento_no_cambia_hasta_soltar() {
    let mut session = session_f();
    let original = image_f();
    select(&mut session, ColorChoice::Entry(1));

    let id_before = session.document.state_id();
    let bytes_before = session.document.history_bytes();
    let untouched = |session: &Session| {
        assert_eq!(session.document.state_id(), id_before);
        assert!(!session.document.can_undo());
        assert_eq!(session.document.history_bytes(), bytes_before);
        assert_eq!(main_image(&session.document), &original);
    };

    // Pulsar ya fija el valor bajo el puntero: R 255 sobre G da amarillo.
    assert!(session.editor.begin(Control::Red, 255));
    assert!(session.editor.dragging());
    assert_eq!(
        session.editor.preview(),
        Some((EditTarget::Entry(1), YELLOW))
    );
    untouched(&session);

    session.editor.drag(128);
    assert_eq!(
        session.editor.preview(),
        Some((EditTarget::Entry(1), Rgba::opaque(128, 255, 0)))
    );
    untouched(&session);

    session.editor.drag(255);
    assert_eq!(
        session.editor.preview(),
        Some((EditTarget::Entry(1), YELLOW))
    );
    untouched(&session);

    let commit = session.editor.release().expect("el color cambió");
    assert_eq!(
        commit,
        Commit {
            target: EditTarget::Entry(1),
            color: YELLOW
        }
    );
    assert!(!session.editor.dragging());
    assert_eq!(session.editor.preview(), None);
    // Soltar no toca el documento: eso lo hace `commit_edit`.
    untouched(&session);

    assert!(session.commit_edit(commit).expect("commit_edit"));
    assert_eq!(palette_colors(&session), vec![RED, YELLOW, BLUE, BLACK]);
    for x in 0..4u32 {
        assert_eq!(
            main_image(&session.document).get_pixel(x, 0).unwrap(),
            Pixel::Indexed(x as u8),
            "los píxeles no se tocan (x = {x})"
        );
    }
    assert_ne!(session.document.state_id(), id_before);
    assert!(session.document.can_undo());
    assert!(session.modified());

    // Un solo undo restaura F exacta.
    session.document.undo().expect("undo");
    assert_eq!(main_image(&session.document), &original);
    assert!(!session.document.can_undo(), "era un solo paso");
    assert!(session.document.can_redo());

    session.document.redo().expect("redo");
    assert_eq!(palette_colors(&session), vec![RED, YELLOW, BLUE, BLACK]);
    assert!(!session.document.can_redo());
}

#[test]
fn fila_4_tras_confirmar_el_editor_sigue_mostrando_el_color_confirmado() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    let commit = drag_and_release(&mut session, Control::Red, 255, &[128, 255]).expect("cambio");
    assert!(session.commit_edit(commit).expect("commit_edit"));
    session.sync_editor();
    let (rgba, _) = session.editor.shown().expect("destino");
    assert_eq!(rgba, YELLOW);
}

#[test]
fn fila_4_cancelar_un_arrastre_no_deja_ni_un_paso() {
    let mut session = session_f();
    let original = image_f();
    select(&mut session, ColorChoice::Entry(1));
    let id_before = session.document.state_id();

    assert!(session.editor.begin(Control::Red, 255));
    session.editor.drag(10);
    session.editor.cancel();

    assert_eq!(session.editor.preview(), None);
    assert!(!session.editor.dragging());
    // Tras cancelar, los movimientos y la suelta no hacen nada.
    session.editor.drag(200);
    assert_eq!(session.editor.release(), None);
    assert_eq!(session.document.state_id(), id_before);
    assert!(!session.document.can_undo());
    assert_eq!(main_image(&session.document), &original);
}

// ---------------------------------------------------------------------------------------
// Fila 13: entradas duplicadas y entrada transparente.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_13a_duplicadas_se_sustituye_por_indice_y_las_demas_no_cambian() {
    let a = Rgba::opaque(200, 100, 50);
    let mut session = Session::sample(Document::new(indexed_row(
        Palette::from_colors([a, a, a]).expect("paleta"),
    )));
    select(&mut session, ColorChoice::Entry(1));
    let original = main_image(&session.document).clone();
    let original_bytes = to_rgba8(&original);
    let a_bytes = [a.r, a.g, a.b, a.a];

    assert!(session.editor.begin(Control::Red, 10));
    let (target, provisional) = session.editor.preview().expect("provisional");
    assert_eq!(target, EditTarget::Entry(1));
    assert_eq!(provisional, Rgba::opaque(10, 100, 50));

    // Vista previa: solo el píxel 1 cambia; el 0 y el 2 conservan los bytes de A.
    let preview = to_rgba8_with_entry(&original, 1, provisional);
    assert_eq!(&preview[0..4], &a_bytes);
    assert_eq!(&preview[4..8], &[10, 100, 50, 255]);
    assert_eq!(&preview[8..12], &a_bytes);
    assert_eq!(main_image(&session.document), &original, "imagen intacta");

    let commit = session.editor.release().expect("el color cambió");
    assert_eq!(commit.target, EditTarget::Entry(1));
    assert_eq!(commit.color, provisional);
    assert!(session.commit_edit(commit).expect("commit_edit"));

    assert_eq!(palette_colors(&session), vec![a, provisional, a]);
    let after = to_rgba8(main_image(&session.document));
    assert_eq!(&after[0..4], &a_bytes);
    assert_eq!(&after[4..8], &[10, 100, 50, 255]);
    assert_eq!(&after[8..12], &a_bytes);

    session.document.undo().expect("undo");
    assert_eq!(main_image(&session.document), &original);
    assert_eq!(to_rgba8(main_image(&session.document)), original_bytes);
}

#[test]
fn fila_13b_entrada_transparente_se_edita_el_color_guardado() {
    let mut palette = Palette::from_colors([RED, GREEN, BLUE]).expect("paleta");
    palette
        .set_transparent_index(Some(2))
        .expect("transparente");
    let mut session = Session::sample(Document::new(indexed_row(palette)));
    select(&mut session, ColorChoice::Entry(2));

    // El editor muestra el color guardado, no el resuelto con alfa 0.
    let (shown, _) = session.editor.shown().expect("destino");
    assert_eq!(shown, BLUE);
    assert_eq!(hex_text(shown), "#0000ff");

    let target = EditTarget::Entry(2);
    session.editor.type_hex(target, BLUE, "#090909".to_string());
    assert_eq!(session.editor.hex_display().as_deref(), Some("#090909"));
    let outcome = session.editor.confirm_hex(Mode::Indexed, false);
    let stored = Rgba::opaque(9, 9, 9);
    let commit = Commit {
        target,
        color: stored,
    };
    assert_eq!(outcome, HexOutcome::Commit(commit));

    assert!(
        session.commit_edit(commit).expect("commit_edit"),
        "Ok(true)"
    );
    let image = main_image(&session.document);
    let palette = image.palette().expect("paleta");
    assert_eq!(palette.get(2).unwrap(), stored, "guardado (9,9,9,255)");
    assert_eq!(palette.transparent_index(), Some(2));
    // Resuelto: alfa 0 en el píxel 2.
    let bytes = to_rgba8(image);
    assert_eq!(&bytes[8..12], &[9, 9, 9, 0]);
    let preview = to_rgba8_with_entry(image, 2, stored);
    assert_eq!(&preview[8..12], &[9, 9, 9, 0]);

    // El campo muestra el guardado y el aviso es el de la entrada transparente.
    session.sync_editor();
    let (shown, _) = session.editor.shown().expect("destino");
    assert_eq!(hex_text(shown), "#090909");
    let notice = session.editor.notice(true);
    assert_eq!(
        notice[0].trim_start_matches(NOTICE_PREFIX),
        "entrada transparente"
    );
    assert_eq!(notice[1], "se guarda con alfa 0");
}

// ---------------------------------------------------------------------------------------
// Fila 14: deshacer y rehacer con el editor abierto.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_14_deshacer_y_rehacer_mantienen_choice_y_actualizan_valores() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    assert_eq!(shown_values(&session), (0, 255, 0, 120));

    let commit = drag_and_release(&mut session, Control::Red, 255, &[]).expect("amarillo");
    assert!(session.commit_edit(commit).expect("commit_edit"));
    session.sync_editor();
    assert_eq!(session.choice, ColorChoice::Entry(1));
    assert_eq!(shown_values(&session), (255, 255, 0, 60));

    session.document.undo().expect("undo");
    session.sync_editor();
    assert_eq!(
        session.choice,
        ColorChoice::Entry(1),
        "undo no cambia choice"
    );
    // El color guardado cambió: se recalcula el Hsl (H 120).
    assert_eq!(shown_values(&session), (0, 255, 0, 120));
    assert_eq!(session.editor.shown().map(|(rgba, _)| rgba), Some(GREEN));
    assert_eq!(session.editor.hex_display(), None);

    session.document.redo().expect("redo");
    session.sync_editor();
    assert_eq!(
        session.choice,
        ColorChoice::Entry(1),
        "redo no cambia choice"
    );
    assert_eq!(shown_values(&session), (255, 255, 0, 60));
    assert_eq!(session.editor.shown().map(|(rgba, _)| rgba), Some(YELLOW));
}

#[test]
fn fila_14_deshacer_un_cambio_ajeno_al_color_no_recalcula_el_hsl() {
    // Entrada 0 gris: `Hsl::from_rgba` daría H 0, pero el editor conserva el H 100 que se le dio.
    let gray = Rgba::opaque(128, 128, 128);
    let palette = Palette::from_colors([gray, GREEN, BLUE, BLACK]).expect("paleta");
    let mut session = Session::sample(Document::new(indexed_row(palette)));
    select(&mut session, ColorChoice::Entry(0));

    assert_eq!(drag_and_release(&mut session, Control::Hue, 100, &[]), None);
    assert_eq!(shown_values(&session).3, 100);

    // Un cambio de píxeles (como un trazo) no toca el color guardado de la entrada 0.
    let id = ImageId::from_raw(0);
    assert!(
        session
            .document
            .apply(Command::SetPixels {
                image: id,
                pixels: vec![PixelEdit {
                    x: 1,
                    y: 0,
                    pixel: Pixel::Indexed(3),
                }],
            })
            .expect("apply")
    );
    session.sync_editor();
    assert_eq!(shown_values(&session).3, 100, "tras el trazo");
    session.document.undo().expect("undo");
    session.sync_editor();
    assert_eq!(shown_values(&session).3, 100, "tras deshacer el trazo");
    session.document.redo().expect("redo");
    session.sync_editor();
    assert_eq!(shown_values(&session).3, 100, "tras rehacer el trazo");

    // Un cambio externo del color guardado sí lo recalcula.
    session
        .document
        .apply(Command::SetPaletteColor {
            image: id,
            index: 0,
            color: BLUE,
        })
        .expect("apply");
    session.sync_editor();
    assert_eq!(shown_values(&session), (0, 0, 255, 240));
}

#[test]
fn fila_14_con_un_arrastre_en_curso_sync_no_cambia_nada() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(1));
    assert!(session.editor.begin(Control::Red, 255));
    session.editor.drag(128);
    let provisional = session.editor.preview();
    let shown = session.editor.shown();

    session.sync_editor();

    assert!(session.editor.dragging());
    assert_eq!(session.editor.preview(), provisional);
    assert_eq!(session.editor.shown(), shown);
    assert_eq!(session.choice, ColorChoice::Entry(1));
}

// ---------------------------------------------------------------------------------------
// `active_pixel` y `sync_editor`.
// ---------------------------------------------------------------------------------------

#[test]
fn active_pixel_en_indexadas_es_la_entrada_y_none_si_no_existe() {
    let mut session = session_f();
    session.choice = ColorChoice::Entry(2);
    assert_eq!(session.active_pixel(), Some(Pixel::Indexed(2)));
    session.choice = ColorChoice::Entry(9);
    assert_eq!(session.active_pixel(), None);
}

#[test]
fn active_pixel_sin_editar_usa_los_valores_por_defecto_de_cada_modo() {
    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    session.choice = ColorChoice::Slot(1);
    assert_eq!(session.active_pixel(), Some(Pixel::Rgba(BLACK)));
    session.choice = ColorChoice::Slot(10);
    assert_eq!(session.active_pixel(), None);

    let mut session = Session::sample(Document::new(sample_image(Mode::Gray)));
    session.choice = ColorChoice::Slot(5);
    assert_eq!(session.active_pixel(), Some(Pixel::Gray(128)));

    let mut session = Session::sample(Document::new(sample_image(Mode::GrayAlpha)));
    session.choice = ColorChoice::Slot(9);
    assert_eq!(
        session.active_pixel(),
        Some(Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );
}

#[test]
fn sync_editor_toma_el_destino_y_el_color_guardado_de_la_seleccion() {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(2));
    assert_eq!(
        session.editor.shown(),
        Some((BLUE, Hsl::from_rgba(BLUE))),
        "indexadas: la entrada guardada"
    );

    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    select(&mut session, ColorChoice::Slot(3));
    assert_eq!(
        session.editor.shown(),
        Some((RED, Hsl::from_rgba(RED))),
        "RGBA: la ranura 3 por defecto"
    );

    // Una selección sin destino válido no deja nada que editar.
    select(&mut session, ColorChoice::Slot(10));
    assert_eq!(session.editor.shown(), None);
}

// ---------------------------------------------------------------------------------------
// Fila 15: ranuras por modo.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_15_editar_una_ranura_no_es_un_paso_ni_modifica_el_documento() {
    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    select(&mut session, ColorChoice::Slot(3));
    let image_before = main_image(&session.document).clone();
    let id_before = session.document.state_id();
    let modified_before = session.modified();
    let bytes_before = session.document.history_bytes();

    // Ranura 3 (rojo por defecto): G a 255 da amarillo.
    let commit = drag_and_release(&mut session, Control::Green, 255, &[]).expect("cambio");
    assert_eq!(
        commit,
        Commit {
            target: EditTarget::Slot(3),
            color: YELLOW
        }
    );
    assert!(
        !session.commit_edit(commit).expect("commit_edit"),
        "una ranura no es del documento: Ok(false)"
    );

    assert_eq!(session.slots.get(Mode::Rgba).get(3), Some(YELLOW));
    assert_eq!(session.document.state_id(), id_before);
    assert!(!session.document.can_undo());
    assert_eq!(session.document.history_bytes(), bytes_before);
    assert_eq!(session.modified(), modified_before);
    assert_eq!(main_image(&session.document), &image_before);
    // El panel rotula la ranura con su color actual.
    assert_eq!(
        cell_label_with(
            main_image(&session.document),
            2,
            session.slots.get(Mode::Rgba)
        )
        .as_deref(),
        Some("color 3 #ffff00")
    );
    assert_eq!(session.active_pixel(), Some(Pixel::Rgba(YELLOW)));
}

#[test]
fn fila_15_el_lapiz_pinta_con_la_ranura_editada() {
    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    select(&mut session, ColorChoice::Slot(3));
    let commit = drag_and_release(&mut session, Control::Green, 255, &[]).expect("cambio");
    session.commit_edit(commit).expect("commit_edit");

    let pixel = session.active_pixel();
    assert_eq!(pixel, Some(Pixel::Rgba(YELLOW)));
    let id = ImageId::from_raw(0);
    let mut tools = ToolState::new();
    tools
        .canvas_event_with(
            &mut session.document,
            id,
            pixel,
            CanvasEvent::StrokeBegin((0, 0)),
        )
        .expect("begin");
    assert!(
        tools
            .canvas_event_with(&mut session.document, id, pixel, CanvasEvent::StrokeEnd)
            .expect("end"),
        "el trazo cambia el documento"
    );
    assert_eq!(
        main_image(&session.document).get_pixel(0, 0).unwrap(),
        Pixel::Rgba(YELLOW)
    );
}

#[test]
fn fila_15_ranura_9_y_gris_por_modo() {
    // RGBA: la ranura 9 es {0,0,0,0}; A a 255 la deja negra y opaca.
    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    select(&mut session, ColorChoice::Slot(9));
    let commit = drag_and_release(&mut session, Control::Alpha, 255, &[]).expect("cambio");
    assert!(!session.commit_edit(commit).expect("commit_edit"));
    assert_eq!(session.active_pixel(), Some(Pixel::Rgba(BLACK)));
    assert!(!session.document.can_undo());

    // Gray: la ranura 5 (gris 128) con G a 200.
    let tmp = TmpDir::new();
    let gray_path = write_image(tmp.path(), "gris.png", &sample_image(Mode::Gray));
    session.open(&gray_path, 16384, true).expect("abrir gris");
    select(&mut session, ColorChoice::Slot(5));
    assert_eq!(session.active_pixel(), Some(Pixel::Gray(128)));
    let commit = drag_and_release(&mut session, Control::Gray, 200, &[]).expect("cambio");
    assert_eq!(
        commit,
        Commit {
            target: EditTarget::Slot(5),
            color: Rgba::opaque(200, 200, 200)
        }
    );
    assert!(!session.commit_edit(commit).expect("commit_edit"));
    assert_eq!(session.active_pixel(), Some(Pixel::Gray(200)));
    assert!(!session.document.can_undo());
    assert!(!session.modified());

    // Cada modo tiene su tabla: lo de RGBA no se movió con lo de Gray. Abrir el gris restauró la
    // ranura 9 de RGBA (I.2a: es el fondo por defecto, transparente).
    assert_eq!(
        session.slots.get(Mode::Rgba).get(9),
        Some(Rgba::TRANSPARENT)
    );
    assert_eq!(
        session.slots.get(Mode::Gray).get(5),
        Some(Rgba::opaque(200, 200, 200))
    );
    assert_eq!(
        session.slots.get(Mode::GrayAlpha),
        &SlotColors::defaults(Mode::GrayAlpha)
    );
}

/// Sesión RGBA sobre un fichero de `dir`, con la ranura 3 amarilla, la 9 opaca y la 5 de Gray
/// en 200, lista para las pruebas de conservación del banco.
fn session_with_edited_bank(dir: &Path) -> (Session, PathBuf) {
    let rgba_path = write_image(dir, "rgba.png", &sample_image(Mode::Rgba));
    let gray_path = write_image(dir, "gris.png", &sample_image(Mode::Gray));
    let mut session = Session::sample(Document::new(sample_image(Mode::Indexed)));

    session.open(&rgba_path, 16384, true).expect("abrir RGBA");
    select(&mut session, ColorChoice::Slot(3));
    let commit = drag_and_release(&mut session, Control::Green, 255, &[]).expect("cambio");
    session.commit_edit(commit).expect("commit_edit");
    select(&mut session, ColorChoice::Slot(9));
    let commit = drag_and_release(&mut session, Control::Alpha, 255, &[]).expect("cambio");
    session.commit_edit(commit).expect("commit_edit");

    session.open(&gray_path, 16384, true).expect("abrir gris");
    select(&mut session, ColorChoice::Slot(5));
    let commit = drag_and_release(&mut session, Control::Gray, 200, &[]).expect("cambio");
    session.commit_edit(commit).expect("commit_edit");
    (session, rgba_path)
}

fn assert_bank_as_left(bank: &SlotBank, context: &str) {
    assert_eq!(
        bank.get(Mode::Rgba).get(3),
        Some(YELLOW),
        "{context}: RGBA 3"
    );
    // La ranura 9 editada se restaura al abrir otro fichero (I.2a, decisión 9): la única que lo
    // hace; las demás se conservan.
    assert_eq!(
        bank.get(Mode::Rgba).get(9),
        Some(Rgba::TRANSPARENT),
        "{context}: RGBA 9"
    );
    assert_eq!(
        bank.get(Mode::Gray).get(5),
        Some(Rgba::opaque(200, 200, 200)),
        "{context}: Gray 5"
    );
    assert_eq!(
        bank.get(Mode::GrayAlpha),
        &SlotColors::defaults(Mode::GrayAlpha),
        "{context}: GrayAlpha"
    );
}

#[test]
fn fila_15_el_banco_se_conserva_al_abrir_otros_ficheros() {
    let tmp = TmpDir::new();
    let (mut session, rgba_path) = session_with_edited_bank(tmp.path());
    let bank = session.slots.clone();
    assert_bank_as_left(&bank, "antes de abrir");

    let ga_path = write_image(tmp.path(), "ga.png", &sample_image(Mode::GrayAlpha));
    let idx_path = write_image(tmp.path(), "indexada.png", &sample_image(Mode::Indexed));
    for (label, path) in [
        ("gris con alfa", &ga_path),
        ("indexada", &idx_path),
        ("RGBA", &rgba_path),
    ] {
        session
            .open(path, 16384, true)
            .unwrap_or_else(|e| panic!("{label}: {e:?}"));
        assert_eq!(session.slots, bank, "tras abrir {label}");
        assert_bank_as_left(&session.slots, label);
    }

    // Volver a uno RGBA: las ranuras editadas siguen ahí y el lápiz las usa.
    select(&mut session, ColorChoice::Slot(3));
    assert_eq!(session.active_pixel(), Some(Pixel::Rgba(YELLOW)));
    select(&mut session, ColorChoice::Slot(9));
    assert_eq!(
        session.active_pixel(),
        Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        "la 9 vuelve a ser la transparente"
    );
}

#[test]
fn fila_15_el_banco_se_conserva_al_guardar_y_guardar_como() {
    let tmp = TmpDir::new();
    let (mut session, _) = session_with_edited_bank(tmp.path());
    let bank = session.slots.clone();
    // Algo que guardar: un trazo en el Gray abierto.
    let id = ImageId::from_raw(0);
    session
        .document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit {
                x: 0,
                y: 0,
                pixel: Pixel::Gray(1),
            }],
        })
        .expect("apply");
    session.choice = ColorChoice::Slot(5);
    session.scroll = Scroll {
        first_row: 2,
        rest: 1,
    };
    session.sync_editor();

    session.save(tmp.path()).expect("save");
    assert_eq!(session.slots, bank, "tras guardar");
    session.save_as(tmp.path()).expect("save_as");
    assert_eq!(session.slots, bank, "tras guardar como");
    assert_bank_as_left(&session.slots, "tras guardar como");

    // Guardar no reinicia ni la selección ni el desplazamiento ni el editor.
    assert_eq!(session.choice, ColorChoice::Slot(5));
    assert_eq!(
        session.scroll,
        Scroll {
            first_row: 2,
            rest: 1
        }
    );
    assert!(session.editor.shown().is_some());
}

#[test]
fn fila_15_abrir_reinicia_choice_scroll_y_editor() {
    let tmp = TmpDir::new();
    let (mut session, rgba_path) = session_with_edited_bank(tmp.path());
    let image = sample_image(Mode::Rgba);

    // Un editor en uso: destino al día y texto pendiente.
    select(&mut session, ColorChoice::Slot(4));
    session.scroll = Scroll {
        first_row: 7,
        rest: 3,
    };
    session.editor.type_hex(
        EditTarget::Slot(4),
        Rgba::opaque(0, 255, 0),
        "#12".to_string(),
    );
    assert!(session.editor.shown().is_some());

    session.open(&rgba_path, 16384, true).expect("abrir");

    assert_eq!(session.choice, initial_choice(&image));
    assert_eq!(session.scroll, Scroll::default());
    assert!(!session.editor.dragging());
    assert_eq!(session.editor.preview(), None);
    assert_eq!(session.editor.hex_display(), None);
    assert_eq!(session.editor.shown(), None, "editor reiniciado");
}

#[test]
fn fila_15_abrir_cancela_un_arrastre_en_curso() {
    let tmp = TmpDir::new();
    let (mut session, rgba_path) = session_with_edited_bank(tmp.path());
    select(&mut session, ColorChoice::Slot(5));
    assert!(session.editor.begin(Control::Gray, 10));
    assert!(session.editor.dragging());

    session.open(&rgba_path, 16384, true).expect("abrir");

    assert!(!session.editor.dragging());
    assert_eq!(session.editor.preview(), None);
    assert_eq!(session.editor.release(), None);
    assert_bank_as_left(&session.slots, "tras abrir con arrastre");
}

#[test]
fn fila_15_un_open_fallido_no_reinicia_el_editor() {
    let tmp = TmpDir::new();
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(2));
    session.scroll = Scroll {
        first_row: 1,
        rest: 1,
    };
    let bank = session.slots.clone();

    let result = session.open(&tmp.path().join("no-existe.png"), 16384, true);
    assert!(result.is_err());

    assert_eq!(session.choice, ColorChoice::Entry(2));
    assert_eq!(
        session.scroll,
        Scroll {
            first_row: 1,
            rest: 1
        }
    );
    assert_eq!(session.slots, bank);
    assert_eq!(session.editor.shown().map(|(rgba, _)| rgba), Some(BLUE));
}

// ---------------------------------------------------------------------------------------
// Propiedad 4b: pasos de deshacer de una sesión indexada con arrastres, hexadecimal y
// cambios de selección.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    /// `begin(control, start)`, un `drag` por valor y, o bien `release` + `commit_edit`, o bien
    /// `cancel`.
    Drag {
        control: Control,
        start: u32,
        moves: Vec<u32>,
        confirm: bool,
    },
    /// Teclear `text` en el campo del destino elegido y confirmar con Intro o al perder el foco.
    Hex { text: String, blur: bool },
    /// Elegir la entrada `n`.
    Select(u8),
}

fn any_op() -> impl Strategy<Value = Op> {
    let controls = vec![
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::Alpha,
    ];
    let hex = prop_oneof![
        "#?[0-9a-fA-F]{3,8}",
        "#[0-9a-f]{6}",
        "[ -~]{0,10}",
        Just(String::new()),
    ];
    prop_oneof![
        (
            prop::sample::select(controls),
            0u32..=400,
            prop::collection::vec(0u32..=400, 0..4),
            any::<bool>(),
        )
            .prop_map(|(control, start, moves, confirm)| Op::Drag {
                control,
                start,
                moves,
                confirm
            }),
        (hex, any::<bool>()).prop_map(|(text, blur)| Op::Hex { text, blur }),
        (0u8..4).prop_map(Op::Select),
    ]
}

/// Aplica una confirmación y comprueba su efecto en `state_id`: cambia exactamente con `Ok(true)`.
fn confirm(
    session: &mut Session,
    commit: Commit,
    before: u64,
    steps: &mut usize,
) -> Result<(), TestCaseError> {
    match session.commit_edit(commit) {
        Ok(true) => {
            prop_assert_ne!(
                session.document.state_id(),
                before,
                "Ok(true) cambia state_id"
            );
            *steps += 1;
        }
        Ok(false) => {
            prop_assert_eq!(
                session.document.state_id(),
                before,
                "Ok(false) no lo cambia"
            );
        }
        Err(error) => {
            return Err(TestCaseError::fail(format!(
                "commit_edit con {commit:?}: {error:?}"
            )));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn propiedad_4b_state_id_solo_cambia_con_confirmaciones_y_se_deshace_todo(
        ops in prop::collection::vec(any_op(), 0..16)
    ) {
        let initial = sample_image(Mode::Indexed);
        let mut session = Session::sample(Document::new(initial.clone()));
        let mut steps = 0usize;

        for op in ops {
            let before = session.document.state_id();
            let can_undo_before = session.document.can_undo();
            match op {
                Op::Drag { control, start, moves, confirm: confirmed } => {
                    session.sync_editor();
                    prop_assert!(session.editor.begin(control, start));
                    prop_assert!(session.editor.dragging());
                    prop_assert_eq!(session.document.state_id(), before, "tras begin");
                    for value in moves {
                        session.editor.drag(value);
                        prop_assert_eq!(session.document.state_id(), before, "durante el arrastre");
                        prop_assert_eq!(session.document.can_undo(), can_undo_before);
                    }
                    if confirmed {
                        let commit = session.editor.release();
                        prop_assert!(!session.editor.dragging());
                        prop_assert_eq!(session.document.state_id(), before, "tras release");
                        match commit {
                            Some(commit) => confirm(&mut session, commit, before, &mut steps)?,
                            None => prop_assert_eq!(session.document.state_id(), before),
                        }
                    } else {
                        session.editor.cancel();
                        prop_assert_eq!(session.editor.preview(), None);
                        prop_assert_eq!(session.editor.release(), None);
                        prop_assert_eq!(session.document.state_id(), before, "tras cancelar");
                    }
                }
                Op::Hex { text, blur } => {
                    session.sync_editor();
                    let image = main_image(&session.document);
                    let target = EditTarget::of(image, session.choice).expect("destino");
                    let base = target.stored(image, session.slots.get(Mode::Indexed)).expect("color");
                    session.editor.type_hex(target, base, text);
                    prop_assert_eq!(session.document.state_id(), before, "teclear no confirma");
                    match session.editor.confirm_hex(Mode::Indexed, blur) {
                        HexOutcome::Commit(commit) => confirm(&mut session, commit, before, &mut steps)?,
                        HexOutcome::Nothing | HexOutcome::Unchanged | HexOutcome::Invalid(_) => {
                            prop_assert_eq!(session.document.state_id(), before);
                        }
                    }
                }
                Op::Select(n) => {
                    session.choice = ColorChoice::Entry(n);
                    session.sync_editor();
                    prop_assert_eq!(session.document.state_id(), before, "elegir no es un paso");
                }
            }
            prop_assert!(!session.editor.dragging());
        }

        // Tantos undo como confirmaciones con Ok(true), y al final la paleta inicial exacta.
        let mut undone = 0usize;
        while session.document.can_undo() {
            session.document.undo().expect("undo");
            undone += 1;
            prop_assert!(undone <= steps, "más undo que confirmaciones");
        }
        prop_assert_eq!(undone, steps);
        prop_assert_eq!(main_image(&session.document), &initial);
    }
}
