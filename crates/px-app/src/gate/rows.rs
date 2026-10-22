//! Filas 1 a 10 de la puerta del hito 2 (specs/hito-2-puerta.md). La fila 11 está en
//! `crates/px-golden/tests/png_interop/gate_oracle.rs` y la 12 en `large.rs`.
//!
//! Cada test crea su directorio de trabajo (`Work`), copia allí los reales y abre la copia:
//! nada se escribe en `golden/png/` y el guardián de `Work` lo comprueba al terminar.
//!
//! **Suposiciones y ambigüedades:**
//! - Fila 1, «`converted()` coincide con que el informe o `aux` no esté vacío»: se compara con
//!   «informe no vacío o `aux.appearance` presente» (los auxiliares inocuos no fuerzan copia).
//! - Fila 6, «`-copy-2` al repetir»: tras un `save_as` la ruta del documento es la copia, así que
//!   repetir sobre ella daría `x-copy-copy.png`; el `-copy-2` sale al repetir desde una sesión
//!   nueva que abre el mismo original.
//! - Fila 9, «al terminar toda la puerta»: las pruebas corren en paralelo y no hay un «final»; la
//!   instantánea inicial se toma una sola vez (`OnceLock`) antes de que nada escriba, cada
//!   `Work` comprueba al soltarse que `golden/png/` sigue igual, y un test dedicado lo comprueba
//!   también, además del SHA-256 de cada uno de los 4 PNG.
//! - Fila 2, el borrador de una indexada sin índice transparente: si la paleta tiene una entrada de
//!   alfa guardado 0 el borrador la usa (spec del cuentagotas y el borrador), así que solo se
//!   rechaza cuando no hay ninguna; el test de rechazos usa una paleta sin alfa 0.

use px_core::ImageId;
use std::fs;

use px_core::{Allowed, ConvertError, Document, DocumentError, Mode};
use px_formats::{AuxReport, read_png};
use px_ui::canvas::Canvas;

use crate::confirm::Confirm;
use crate::dialogs::{DialogKind, Finished, Outcome, open_gate};
use crate::files::FileError;
use crate::session::Session;

use super::common::sha256::{sha256_hex, vectors};
use super::common::*;

/// Texto del modo que lleva el aviso de apertura.
fn text_of_the_mode(mode_2: Mode) -> &'static str {
    match mode_2 {
        Mode::Indexed => "indexado",
        Mode::Gray => "gris",
        Mode::GrayAlpha => "gris con alfa",
        Mode::Rgba => "RGBA",
    }
}

/// Pixoko convierte el fichero al abrir: el informe de `read_png` no está vacío o hay un auxiliar
/// que cambia la apariencia.
fn converts(source: &Source) -> bool {
    let read_2 = read_png(&source.bytes).expect("read_png de la fuente");
    !read_2.report.is_empty() || read_2.aux.appearance.is_some()
}

// ---------------------------------------------------------------------------------------
// Fila 1: abrir.
// ---------------------------------------------------------------------------------------

#[test]
fn row_1_open_of_the_real_ones_and_the_generated() {
    let t = Work::new_2();
    for source in all() {
        let ctx = source.name_2.clone();
        let path_2 = t.put(&source);
        let mut s = session_empty();
        let notice = s
            .open(&path_2, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: open dio error: {e}"));
        let image_2 = s
            .document
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");

        // Imagen = lectura independiente del fichero (modelo, o el .sha256 de los reales).
        check_opening(&source, image_2);
        // Modo y tamaño, del IHDR.
        let (width, height, _, _) = ihdr(&source.bytes);
        assert_eq!(
            (image_2.width(), image_2.height()),
            (width, height),
            "{ctx}: tamaño"
        );
        assert_eq!(
            image_2.mode(),
            mode_of_the_file(&source.bytes),
            "{ctx}: modo"
        );

        // Estado de la sesión recién abierta.
        assert_eq!(s.document.state_id(), 0, "{ctx}: state_id");
        assert!(!s.modified(), "{ctx}: modified");
        assert!(!s.document.can_undo(), "{ctx}: hay historia que deshacer");
        assert!(!s.document.can_redo(), "{ctx}: hay historia que rehacer");
        assert_eq!(s.path(), Some(path_2.as_path()), "{ctx}: ruta");

        // El aviso lleva nombre, tamaño y modo.
        assert!(
            notice.contains(&source.name_2),
            "{ctx}: sin nombre: {notice}"
        );
        assert!(
            notice.contains(&format!("{width}×{height}")),
            "{ctx}: sin tamaño: {notice}"
        );
        let mode_2 = text_of_the_mode(image_2.mode());
        assert!(
            notice.contains(&format!(" · {mode_2}")),
            "{ctx}: sin modo {mode_2}: {notice}"
        );
        if image_2.mode() == Mode::Gray {
            assert!(
                !notice.contains("gris con alfa"),
                "{ctx}: modo equivocado: {notice}"
            );
        }

        // `converted()` coincide con el informe y los auxiliares de `read_png`.
        assert_eq!(s.converted(), converts(&source), "{ctx}: converted()");
        if let Some(expected) = source.converted_2 {
            assert_eq!(
                s.converted(),
                expected,
                "{ctx}: converted() por construcción"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2: herramientas.
// ---------------------------------------------------------------------------------------

#[test]
fn row_2_pencil_line_fill_and_eraser_over_each_source() {
    for source in all() {
        let t = Work::new_2();
        let mut w = open(&t, &source);
        let script = script_tools(&w.m);
        let results: Vec<bool> = script.iter().map(|step| w.apply(step)).collect();
        let with_change = results.iter().filter(|&&c| c).count();
        let ctx = &source.name_2;

        assert!(
            with_change >= 3,
            "{ctx}: el guion casi no hizo nada ({results:?})"
        );
        // Un paso de historia por gesto con cambio.
        assert_eq!(w.steps_2(), with_change, "{ctx}: pasos de historia");
        // El mismo trazo con el mismo color y el relleno de un mismo color no dan paso.
        assert!(
            !results[1],
            "{ctx}: repetir el trazo con el mismo color dio paso"
        );
        assert!(!results[6], "{ctx}: rellenar con el mismo color dio paso");
        if w.m.mode_2 == Mode::Gray {
            // El borrador se rechaza en gris, sin paso.
            assert_eq!(w.rejections, 2, "{ctx}: borrador en gris");
            assert!(
                !results[7] && !results[8],
                "{ctx}: el borrador cambió un gris"
            );
        }
        // `undo` deshace exactamente esos pasos.
        assert_eq!(w.undo_all(), with_change, "{ctx}: undo hasta el principio");
        assert!(!w.s.modified(), "{ctx}: modified al volver al principio");
    }
}

#[test]
fn row_2_eraser_rejected_in_gray_and_in_indexed_without_transparent() {
    // Gris: sin transparencia.
    let gray = model_of(Mode::Gray, 4, 4, vec![[100, 0, 0, 0]; 16]);
    let mut w = World::of_session(Session::sample(Document::new(gray.a_image())), "gris");
    assert!(!w.apply(&TestStep::Eraser(vec![(1, 1), (2, 1)])));
    assert_eq!(w.rejections, 1, "gris: el borrador debía rechazarse");
    assert_eq!(w.s.document.state_id(), 0, "gris: el rechazo dio paso");
    assert!(!w.s.document.can_undo());

    // Indexada sin índice transparente ni entradas de alfa 0: rechazado hasta fijarlo.
    let mut indexed = model_of(Mode::Indexed, 4, 4, vec![[0, 0, 0, 0]; 16]);
    indexed.palette = vec![[10, 10, 10, 255], [200, 0, 0, 255], [0, 200, 0, 255]];
    let mut w = World::of_session(
        Session::sample(Document::new(indexed.a_image())),
        "indexada",
    );
    assert!(!w.apply(&TestStep::Eraser(vec![(1, 1), (2, 1)])));
    assert_eq!(w.rejections, 1, "indexada: el borrador debía rechazarse");
    assert_eq!(w.steps_2(), 0, "indexada: el rechazo dio paso");

    // Al fijar el índice transparente, el borrador ya escribe en él.
    assert!(w.apply(&TestStep::Transparent(Some(1))));
    assert!(w.apply(&TestStep::Eraser(vec![(1, 1), (2, 1)])));
    assert_eq!(w.rejections, 1, "indexada: ya no debía rechazarse");
    assert_eq!(w.steps_2(), 2);
    assert_eq!(
        w.m.cels[4 + 1],
        [1, 0, 0, 0],
        "el borrador no escribió el índice transparente"
    );
}

#[test]
fn row_2_the_image_comparator_knows_how_to_fail() {
    // Control: un modelo con un píxel cambiado no es la imagen.
    let model = generated_2()[0].expected_2.clone().expect("modelo");
    let image_2 = model.a_image();
    assert_equal(&image_2, &model, "control: igual");
    let mut other_2 = model.clone();
    other_2.cels[5][0] ^= 1;
    let result = std::panic::catch_unwind(|| assert_equal(&image_2, &other_2, "control"));
    assert!(result.is_err(), "assert_igual no vio un píxel distinto");
}

// ---------------------------------------------------------------------------------------
// Fila 3: paleta.
// ---------------------------------------------------------------------------------------

#[test]
fn row_3_palette_of_the_indexed_ones() {
    let mut vistas = 0;
    for source in all()
        .into_iter()
        .filter(|f| mode_of_the_file(&f.bytes) == Mode::Indexed)
    {
        vistas += 1;
        let t = Work::new_2();
        let mut w = open(&t, &source);
        let ctx = source.name_2.clone();
        let mut changes_2 = 0;
        for step in script_palette(&w.m) {
            let antes_bytes = w.image_2().bytes().to_vec();
            let before_colors = w.image_2().palette().expect("paleta").colors().to_vec();
            if w.apply(&step) {
                changes_2 += 1;
            }
            if let TestStep::Transparent(_) = step {
                // `SetTransparent` solo mueve la marca: los índices y los colores guardados no cambian.
                assert!(
                    w.image_2().bytes() == &antes_bytes[..],
                    "{ctx}: SetTransparent cambió los índices"
                );
                assert_eq!(
                    w.image_2().palette().expect("paleta").colors(),
                    &before_colors[..],
                    "{ctx}: SetTransparent cambió los colores guardados"
                );
            }
        }
        assert!(
            changes_2 >= 6,
            "{ctx}: el guion casi no hizo nada ({changes_2} pasos)"
        );
        // Un paso por operación.
        assert_eq!(w.steps_2(), changes_2, "{ctx}: pasos de historia");
        assert_eq!(w.undo_all(), changes_2, "{ctx}: undo hasta el principio");
    }
    assert!(
        vistas >= 4,
        "se esperaban al menos 4 indexadas y hubo {vistas}"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 4: conversión de modo.
// ---------------------------------------------------------------------------------------

/// Convertir a gris sin permiso perdería algo: algún píxel con alfa < 255 (se compondría) o a > 0
/// con r, g y b distintos (se llevaría a gris).
fn loses_when_going_to_gray(m: &Model) -> bool {
    m.rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[3] < 255 || (p[3] > 0 && !(p[0] == p[1] && p[1] == p[2])))
}

#[test]
fn row_4_exact_and_lossy_conversion() {
    let (mut exact, mut with_lost) = (0, 0);
    for source in all() {
        let t = Work::new_2();
        let mut w = open(&t, &source);
        let ctx = source.name_2.clone();
        let original = w.image_2().try_clone().expect("copia");

        // La que pierde datos sin permiso da `Err` y no toca ni el documento ni la historia.
        if w.m.mode_2 != Mode::Gray && loses_when_going_to_gray(&w.m) {
            let result = w.s.convert_mode(Mode::Gray, Allowed::NOTHING);
            assert!(
                matches!(
                    result,
                    Err(DocumentError::Convert(ConvertError::LossNotAllowed { .. }))
                ),
                "{ctx}: a gris sin permiso dio {result:?}"
            );
            assert_eq!(
                w.s.document.state_id(),
                0,
                "{ctx}: el error cambió el state_id"
            );
            assert!(!w.s.document.can_undo(), "{ctx}: el error dejó un paso");
            assert!(w.image_2() == &original, "{ctx}: el error cambió la imagen");
            assert!(!w.s.modified(), "{ctx}: el error marcó cambios");
            w.check();
            with_lost += 1;
        }

        // El camino exacto de su modo da el modelo en un paso.
        let change = w.apply(&TestStep::Convert);
        if change {
            exact += 1;
            assert_eq!(w.steps_2(), 1, "{ctx}: la conversión no fue un solo paso");
            w.undo();
            assert!(
                w.image_2() == &original,
                "{ctx}: undo no devolvió la original"
            );
            w.redo();
        }
    }
    assert!(
        with_lost >= 3,
        "pocas conversiones con pérdida: {with_lost}"
    );
    assert!(exact >= 7, "pocas conversiones exactas: {exact}");
}

#[test]
fn row_4_too_many_colors_gives_an_error_without_changing_anything() {
    let many = rgba_of_many_colors();
    let mut w = World::of_session(Session::sample(Document::new(many.a_image())), "muchos");
    let original = w.image_2().try_clone().expect("copia");
    let result = w.s.convert_mode(Mode::Indexed, Allowed::NOTHING);
    assert!(
        matches!(
            result,
            Err(DocumentError::Convert(ConvertError::TooManyColors { .. }))
        ),
        "RGBA con más de 256 colores a indexado dio {result:?}"
    );
    assert_eq!(w.s.document.state_id(), 0);
    assert!(!w.s.document.can_undo());
    assert!(w.image_2() == &original);
    // El mismo caso por el camino del modelo.
    assert!(!w.apply(&TestStep::Convert));
    assert_eq!(w.steps_2(), 0);
}

// ---------------------------------------------------------------------------------------
// Fila 5: deshacer y rehacer.
// ---------------------------------------------------------------------------------------

#[test]
fn row_5_undo_and_redo_the_complete_script() {
    for source in all() {
        let t = Work::new_2();
        let mut w = open(&t, &source);
        let ctx = source.name_2.clone();
        let original = w.image_2().try_clone().expect("copia");
        for step in script_complete(&w.m) {
            w.apply(&step);
        }
        let n = w.steps_2();
        assert!(n >= 4, "{ctx}: el guion casi no hizo nada ({n} pasos)");
        let final_ = w.image_2().try_clone().expect("copia");

        // `undo` hasta el principio: cada paso da su instantánea (lo comprueba el mundo).
        assert_eq!(w.undo_all(), n, "{ctx}: pasos deshechos");
        assert!(
            w.image_2() == &original,
            "{ctx}: no volvió la imagen original exacta"
        );
        assert_eq!(w.s.document.state_id(), 0, "{ctx}: state_id al principio");
        assert!(!w.s.modified(), "{ctx}: modified al principio");

        // `redo` hasta el final.
        assert_eq!(w.redo_all(), n, "{ctx}: pasos rehechos");
        assert!(w.image_2() == &final_, "{ctx}: no volvió la imagen final");
        assert!(w.s.modified(), "{ctx}: modified al final");

        // Se guarda el estado final; después, `undo` ×2 + edición nueva + `redo`.
        save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save dio {e}"));
        w.mark_saved();
        w.undo();
        w.undo();
        assert!(w.s.modified(), "{ctx}: modified tras deshacer del guardado");
        let new = editing_that_changes(&w.m);
        assert!(w.apply(&new), "{ctx}: la edición nueva no cambió nada");
        // `redo` falla (lo comprueba el mundo) y el estado guardado ya no se alcanza.
        w.redo();
        assert!(w.s.modified(), "{ctx}: modified tras la edición nueva");
        while w.pos > 0 {
            w.undo();
            assert!(
                w.s.modified(),
                "{ctx}: el estado guardado volvió a ser alcanzable (undo)"
            );
        }
        while w.pos + 1 < w.snaps.len() {
            w.redo();
            assert!(
                w.s.modified(),
                "{ctx}: el estado guardado volvió a ser alcanzable (redo)"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 6: save y save_as.
// ---------------------------------------------------------------------------------------

#[test]
fn row_6_save_and_save_as_after_the_script() {
    for source in all() {
        let t = Work::new_2();
        let mut w = open(&t, &source);
        let ctx = source.name_2.clone();
        for step in script_complete(&w.m) {
            w.apply(&step);
        }
        let open_2 = w.s.path().expect("ruta").to_path_buf();
        let converted_2 = w.s.converted();
        let bytes_open = fs::read(&open_2).expect("leer la copia");

        // `save`: sobrescribe la copia (o escribe `-copy` si el fichero se convierte).
        let target_2 = if converted_2 {
            free_copy(&open_2)
        } else {
            open_2.clone()
        };
        let saved = save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
        assert_eq!(saved.path, target_2, "{ctx}: destino de save");
        assert_eq!(
            saved.copy_reason.is_some(),
            converted_2,
            "{ctx}: copy_reason"
        );
        w.mark_saved();
        assert_eq!(
            w.s.path(),
            Some(target_2.as_path()),
            "{ctx}: ruta tras save"
        );
        assert_equal(
            &leer(&target_2).image,
            &w.m.normalized(),
            &format!("{ctx}: relectura tras save"),
        );
        without_temporary(&t.names());
        if converted_2 {
            assert_eq!(
                fs::read(&open_2).expect("leer el original"),
                bytes_open,
                "{ctx}: save tocó el original que se convierte"
            );
        }

        // `save_as`: `x-copy.png`, el fichero anterior intacto y la ruta nueva.
        w.apply(&editing_that_changes(&w.m));
        let anterior = fs::read(&target_2).expect("leer lo guardado");
        let expected = free_copy(&target_2);
        let copy = save_as(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save_as: {e}"));
        assert_eq!(copy.path, expected, "{ctx}: destino de save_as");
        assert!(copy.copy_reason.is_none(), "{ctx}: save_as dio copy_reason");
        w.mark_saved();
        assert_eq!(
            w.s.path(),
            Some(copy.path.as_path()),
            "{ctx}: ruta tras save_as"
        );
        assert_eq!(
            fs::read(&target_2).expect("leer lo anterior"),
            anterior,
            "{ctx}: save_as tocó el fichero anterior"
        );
        assert_equal(
            &leer(&copy.path).image,
            &w.m.normalized(),
            &format!("{ctx}: relectura tras save_as"),
        );

        // Repetir desde el mismo original: `-copy-2`, sin tocar la primera copia.
        let first_copy = fs::read(&copy.path).expect("leer la copia");
        let mut s2 = session_empty();
        s2.open(&open_2, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: reabrir el original: {e}"));
        let mut w2 = World::of_session(s2, &ctx);
        w2.apply(&editing_that_changes(&w2.m));
        let expected_2_2 = free_copy(&open_2);
        assert!(
            expected_2_2.to_string_lossy().ends_with("-copia-2.png"),
            "{ctx}: se esperaba -copia-2 y es {}",
            expected_2_2.display()
        );
        let copy_2 =
            save_as(&mut w2.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save_as 2: {e}"));
        assert_eq!(
            copy_2.path, expected_2_2,
            "{ctx}: destino del segundo save_as"
        );
        assert_eq!(
            fs::read(&copy.path).expect("leer la primera copia"),
            first_copy,
            "{ctx}: el segundo save_as tocó la primera copia"
        );
        assert_equal(
            &leer(&copy_2.path).image,
            &w2.m.normalized(),
            &format!("{ctx}: relectura tras el segundo save_as"),
        );
        without_temporary(&t.names());
    }
}

#[test]
fn row_6_a_real_one_opened_directly_is_saved_in_the_work() {
    for source in real_2() {
        let t = Work::new_2();
        let ctx = source.name_2.clone();
        // La apertura directa del real es solo lectura.
        let real = golden_png().join(&source.name_2);
        let mut s = session_empty();
        s.open(&real, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: open del real: {e}"));
        check_opening(
            &source,
            s.document.image(ImageId::from_raw(0)).expect("imagen"),
        );
        let mut w = World::of_session(s, &ctx);
        w.apply(&editing_that_changes(&w.m));

        // Su primer guardado es `save_to(<trabajo>/…)`.
        let chosen = t.path_2().join("guardado.png");
        let saved = save_in(&mut w.s, &chosen).unwrap_or_else(|e| panic!("{ctx}: save_to: {e}"));
        assert_eq!(saved.path, chosen, "{ctx}: destino");
        assert!(saved.copy_reason.is_none(), "{ctx}: copy_reason");
        w.mark_saved();
        assert_eq!(w.s.path(), Some(chosen.as_path()), "{ctx}: ruta");
        assert_equal(&leer(&chosen).image, &w.m.normalized(), &ctx);
        assert_eq!(
            t.names(),
            vec!["guardado.png".to_string()],
            "{ctx}: ficheros del trabajo"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: diálogos con selector falso.
// ---------------------------------------------------------------------------------------

/// Resultado de repetir el pegamento de `main.rs` (`finish_dialog`) sin egui.
enum Pasted {
    Cancelled,
    TestSaved(Result<crate::session::Saved, FileError>),
    Open(Result<String, FileError>),
}

/// Lo que hacen `finish_dialog` y `open_file` de `main.rs` con un diálogo terminado: `Chosen` con
/// `Save` o `SaveAs` es `save_to`, y con `Open`, abrir con el descarte que dijo el diálogo.
fn glue(s: &mut Session, finished: Finished) -> Pasted {
    match finished.outcome {
        Outcome::Cancelled => Pasted::Cancelled,
        Outcome::Unavailable => panic!("el selector falso nunca falla"),
        Outcome::Chosen(path_2) => match finished.kind {
            DialogKind::Open => Pasted::Open(s.open(&path_2, Canvas::MAX_SIDE, finished.discard)),
            DialogKind::Save | DialogKind::SaveAs | DialogKind::Export => {
                Pasted::TestSaved(save_in(s, &path_2))
            }
        },
    }
}

#[test]
fn row_7_dialogs_with_a_fake_picker() {
    let t = Work::new_2();
    let generated = generated_2().remove(1);
    let image_2 = generated.expected_2.clone().expect("modelo").a_image();
    let mut w = World::of_session(Session::sample(Document::new(image_2)), "dialogos");
    let (mut dialogs_2, control) = dialogs_2(t.path_2());
    w.apply(&editing_that_changes(&w.m));
    assert!(w.s.modified() && w.s.path().is_none());

    // `Ctrl+S` sin ruta: propone `without-title.png`; el resultado `T/mi-dibujo` escribe
    // `mi-dibujo.png` y fija la ruta.
    assert!(dialogs_2.begin(
        DialogKind::Save,
        false,
        w.s.path(),
        w.s.converted(),
        0.0,
        None
    ));
    let request = control.last_2();
    assert_eq!(request.kind, DialogKind::Save);
    assert_eq!(request.file_name.as_deref(), Some("sin-titulo.png"));
    assert!(dialogs_2.pending());
    assert!(
        dialogs_2.poll(0.1).is_none(),
        "el diálogo debía seguir abierto"
    );
    control.choose(&t.path_2().join("mi-dibujo"));
    let finished = dialogs_2.poll(1.0).expect("el diálogo terminó");
    assert!(!dialogs_2.pending());
    assert_eq!(finished.kind, DialogKind::Save);
    let Pasted::TestSaved(result) = glue(&mut w.s, finished) else {
        panic!("Save no guardó");
    };
    let saved = result.expect("Save guardó");
    let first_2 = t.path_2().join("mi-dibujo.png");
    assert_eq!(saved.path, first_2);
    assert!(saved.copy_reason.is_none());
    w.mark_saved();
    assert_eq!(w.s.path(), Some(first_2.as_path()), "Save no fijó la ruta");
    assert_eq!(t.names(), vec!["mi-dibujo.png".to_string()]);
    assert_equal(&leer(&first_2).image, &w.m.normalized(), "mi-dibujo.png");
    let bytes_first = fs::read(&first_2).expect("leer mi-dibujo.png");

    // `SaveAs`: propone `mi-dibujo.png`, en el directorio de la ruta, y escribe lo elegido sin
    // tocar el primero.
    w.apply(&editing_that_changes(&w.m));
    assert!(dialogs_2.begin(
        DialogKind::SaveAs,
        false,
        w.s.path(),
        w.s.converted(),
        10.0,
        None
    ));
    let request = control.last_2();
    assert_eq!(request.file_name.as_deref(), Some("mi-dibujo.png"));
    assert_eq!(request.dir.as_deref(), Some(t.path_2()));
    let other = t.path_2().join("otra.png");
    control.choose(&other);
    let finished = dialogs_2.poll(11.0).expect("el diálogo terminó");
    let Pasted::TestSaved(result) = glue(&mut w.s, finished) else {
        panic!("SaveAs no guardó");
    };
    assert_eq!(result.expect("SaveAs guardó").path, other);
    w.mark_saved();
    assert_eq!(w.s.path(), Some(other.as_path()));
    assert_eq!(
        fs::read(&first_2).expect("leer"),
        bytes_first,
        "SaveAs tocó el primero"
    );
    assert_equal(&leer(&other).image, &w.m.normalized(), "otra.png");
    assert_eq!(
        t.names(),
        vec!["mi-dibujo.png".to_string(), "otra.png".to_string()]
    );

    // `Cancelled`: no escribe nada y deja `modified`.
    w.apply(&editing_that_changes(&w.m));
    let names = t.names();
    let startups = control.startups();
    assert!(dialogs_2.begin(
        DialogKind::SaveAs,
        false,
        w.s.path(),
        w.s.converted(),
        20.0,
        None
    ));
    assert_eq!(control.startups(), startups + 1);
    control.close();
    let finished = dialogs_2.poll(30.0).expect("el diálogo terminó");
    assert_eq!(finished.outcome, Outcome::Cancelled);
    assert!(matches!(glue(&mut w.s, finished), Pasted::Cancelled));
    assert_eq!(t.names(), names, "Cancelled escribió algo");
    assert!(w.s.modified(), "Cancelled quitó modified");
    assert_eq!(w.s.path(), Some(other.as_path()));
    w.check();

    // `Exists`: elegir `other_2` con `other_2.png` ya existente no escribe nada.
    let previous = t.write("otro.png", b"no soy un png");
    let names = t.names();
    assert!(dialogs_2.begin(
        DialogKind::SaveAs,
        false,
        w.s.path(),
        w.s.converted(),
        40.0,
        None
    ));
    control.choose(&t.path_2().join("otro"));
    let finished = dialogs_2.poll(41.0).expect("el diálogo terminó");
    let Pasted::TestSaved(result) = glue(&mut w.s, finished) else {
        panic!("el diálogo no llegó a guardar");
    };
    assert_eq!(result, Err(FileError::Exists(previous.clone())));
    assert_eq!(
        fs::read(&previous).expect("leer"),
        b"no soy un png",
        "Exists pisó otro.png"
    );
    assert_eq!(t.names(), names, "Exists escribió algo");
    assert!(w.s.modified(), "Exists quitó modified");
    assert_eq!(w.s.path(), Some(other.as_path()), "Exists cambió la ruta");
    without_temporary(&t.names());

    // `Open` de lo guardado, con la confirmación por repetición de `Ctrl+O` con cambios.
    let without_discard = w.s.open(&first_2, Canvas::MAX_SIDE, false);
    assert_eq!(
        without_discard,
        Err(FileError::Unsaved),
        "open con cambios sin descartar"
    );
    let mut confirmation = Confirm::new();
    assert_eq!(open_gate(w.s.modified(), &mut confirmation, 100.0), None);
    let discard = open_gate(w.s.modified(), &mut confirmation, 101.0).expect("segunda pulsación");
    assert!(discard);
    assert!(dialogs_2.begin(
        DialogKind::Open,
        discard,
        w.s.path(),
        w.s.converted(),
        102.0,
        None
    ));
    assert_eq!(control.last_2().file_name, None, "abrir no propone nombre");
    control.choose(&first_2);
    let finished = dialogs_2.poll(103.0).expect("el diálogo terminó");
    assert!(finished.discard);
    let Pasted::Open(result) = glue(&mut w.s, finished) else {
        panic!("Open no abrió");
    };
    result.expect("Open abrió");
    let open_2 = w.s.document.image(ImageId::from_raw(0)).expect("imagen");
    // Lo guardado en `mi-dibujo.png` era el modelo de entonces (`snaps[1]`), normalizado.
    assert_equal(open_2, &w.snaps[1].1.normalized(), "Open de mi-dibujo.png");
    assert_eq!(w.s.path(), Some(first_2.as_path()));
    assert!(!w.s.modified());
    assert_eq!(w.s.document.state_id(), 0);
}

// ---------------------------------------------------------------------------------------
// Fila 8: relectura.
// ---------------------------------------------------------------------------------------

#[test]
fn row_8_reread_and_second_save_with_the_same_bytes() {
    for source in all() {
        let t = Work::new_2();
        let mut w = open(&t, &source);
        let ctx = source.name_2.clone();
        for step in script_complete(&w.m) {
            w.apply(&step);
        }
        let saved = save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
        let bytes_1 = fs::read(&saved.path).expect("leer lo guardado");

        // Sin conversiones ni auxiliares: `write_png` no escribe nada que `read_png` anote.
        let read_2 = read_png(&bytes_1).unwrap_or_else(|e| panic!("{ctx}: read_png: {e}"));
        assert!(
            read_2.report.is_empty(),
            "{ctx}: conversiones: {:?}",
            read_2.report
        );
        assert_eq!(read_2.aux, AuxReport::default(), "{ctx}: auxiliares");

        // Otra sesión abre lo guardado.
        let mut s2 = session_empty();
        s2.open(&saved.path, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: reabrir: {e}"));
        let image_2 = s2.document.image(ImageId::from_raw(0)).expect("imagen");
        assert_equal(image_2, &w.m.normalized(), &format!("{ctx}: relectura"));
        assert!(!s2.converted(), "{ctx}: lo guardado se convierte al abrir");
        assert_eq!(s2.document.state_id(), 0, "{ctx}: state_id");
        assert!(!s2.modified(), "{ctx}: modified");

        // Y vuelve a guardar: `write_png` es determinista.
        let second = save(&mut s2, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save 2: {e}"));
        assert_eq!(second.path, saved.path, "{ctx}: ruta del segundo guardado");
        assert!(
            second.copy_reason.is_none(),
            "{ctx}: copy_reason del segundo guardado"
        );
        assert!(
            fs::read(&second.path).expect("leer el segundo") == bytes_1,
            "{ctx}: los bytes del segundo guardado no son los del primero"
        );
        without_temporary(&t.names());
    }
}

// ---------------------------------------------------------------------------------------
// Fila 9: golden/png intacto y controles negativos.
// ---------------------------------------------------------------------------------------

#[test]
fn row_9_golden_png_intact_at_the_end_of_the_gate() {
    let antes = initial();
    let mut names_expected: Vec<String> = REAL.iter().map(|n| (*n).to_string()).collect();
    names_expected.extend(REAL.iter().map(|n| format!("{n}.sha256")));
    names_expected.push("README.md".to_string());
    names_expected.sort();
    assert_eq!(
        antes.keys().cloned().collect::<Vec<_>>(),
        names_expected,
        "golden/png/ no tiene los 9 ficheros del corpus"
    );

    // Los 9 ficheros con los mismos bytes y ninguno más.
    let now = snapshot_of(&golden_png());
    if let Err(problems) = compare_snapshots(antes, &now) {
        panic!("golden/png/ cambió: {problems}");
    }
    // El SHA-256 de cada PNG es el de la instantánea inicial.
    for name_2 in REAL {
        assert_eq!(
            sha256_hex(&now[name_2]),
            sha256_hex(&antes[name_2]),
            "{name_2}: el SHA-256 cambió"
        );
    }
    golden_intact();
}

#[test]
fn row_9_own_sha256_meets_its_vectors() {
    for (name_2, message, expected_2) in vectors() {
        assert_eq!(sha256_hex(&message), expected_2, "vector {name_2}");
    }
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn row_9_control_outside_golden_panics_with_a_golden_file() {
    outside_of_golden(&golden_png().join("grafx2-golden1.png"));
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn row_9_control_outside_golden_panics_with_a_new_golden_file() {
    outside_of_golden(&golden_png().join("nuevo.png"));
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn row_9_control_outside_golden_panics_with_the_folder_itself() {
    outside_of_golden(&golden_png());
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn row_9_control_outside_golden_panics_with_a_path_with_dot_dot() {
    let rodeo = root_repo()
        .join("crates")
        .join("..")
        .join("golden")
        .join("png")
        .join("..")
        .join("png")
        .join("x.png");
    outside_of_golden(&rodeo);
}

#[test]
fn row_9_outside_golden_lets_the_work_and_the_rest_of_the_repo_through() {
    let t = Work::new_2();
    outside_of_golden(&t.path_2().join("x.png"));
    outside_of_golden(&golden_png().parent().expect("golden").join("otra-cosa.png"));
    outside_of_golden(&root_repo().join("target").join("x.png"));
}

#[test]
fn row_9_the_snapshot_comparator_knows_how_to_fail() {
    let base = initial().clone();
    assert_eq!(compare_snapshots(&base, &base), Ok(()));
    let name_2 = REAL[0];

    // Cambia un byte de un PNG.
    let mut changed = base.clone();
    changed.get_mut(name_2).expect("png")[10] ^= 1;
    let error = compare_snapshots(&base, &changed).expect_err("un byte distinto");
    assert!(error.contains(name_2), "{error}");

    // Cambia un byte del README.
    let mut changed = base.clone();
    changed.get_mut("README.md").expect("readme")[0] ^= 1;
    assert!(compare_snapshots(&base, &changed).is_err());

    // Aparece un fichero.
    let mut with_one_more = base.clone();
    with_one_more.insert("intruso.png".to_string(), vec![1, 2, 3]);
    let error = compare_snapshots(&base, &with_one_more).expect_err("fichero nuevo");
    assert!(error.contains("intruso.png"), "{error}");

    // Un PNG más corto, y un fichero que falta.
    let mut clipped = base.clone();
    clipped.get_mut(name_2).expect("png").pop();
    assert!(compare_snapshots(&base, &clipped).is_err());
    let mut without_one = base.clone();
    without_one.remove(name_2);
    let error = compare_snapshots(&base, &without_one).expect_err("fichero que falta");
    assert!(error.contains(name_2), "{error}");
}

// ---------------------------------------------------------------------------------------
// Fila 10: PNG que Pixoko convierte.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_pngs_that_pixoko_converts_save_a_copy_first() {
    let mut sources = converted_2_2();
    // Con los mismos criterios, los reales que se conviertan.
    sources.extend(real_2().into_iter().filter(converts));
    assert!(sources.len() >= 4);
    for source in sources {
        let t = Work::new_2();
        let ctx = source.name_2.clone();
        let mut w = open(&t, &source);
        assert!(w.s.converted(), "{ctx}: debía convertirse al abrir");
        let source_2 = w.s.path().expect("ruta").to_path_buf();
        let bytes_source = fs::read(&source_2).expect("leer la copia de origen");
        let copy = free_copy(&source_2);
        let names = {
            let mut v = vec![
                source_2
                    .file_name()
                    .expect("nombre")
                    .to_string_lossy()
                    .into_owned(),
                copy.file_name()
                    .expect("nombre")
                    .to_string_lossy()
                    .into_owned(),
            ];
            v.sort();
            v
        };

        // El primer `save` escribe `-copy` y deja intacta la copia de origen.
        let model_0 = w.m.clone();
        let first_2 = save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save 1: {e}"));
        assert_eq!(first_2.path, copy, "{ctx}: destino del primer save");
        assert!(
            first_2
                .copy_reason
                .as_deref()
                .is_some_and(|reason| !reason.is_empty()),
            "{ctx}: copy_reason debía ser Some"
        );
        assert_eq!(
            fs::read(&source_2).expect("leer"),
            bytes_source,
            "{ctx}: tocó el origen"
        );
        assert_eq!(t.names(), names, "{ctx}: ficheros tras el primer save");
        assert_equal(
            &leer(&copy).image,
            &model_0.normalized(),
            &format!("{ctx}: relectura de la copia"),
        );
        w.mark_saved();
        assert!(!w.s.converted(), "{ctx}: sigue convertida tras guardar");
        assert_eq!(w.s.path(), Some(copy.as_path()));

        // El segundo `save` sobrescribe la copia.
        w.apply(&editing_that_changes(&w.m));
        let second = save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save 2: {e}"));
        assert_eq!(second.path, copy, "{ctx}: destino del segundo save");
        assert!(
            second.copy_reason.is_none(),
            "{ctx}: el segundo save dio copy_reason"
        );
        assert_eq!(t.names(), names, "{ctx}: ficheros tras el segundo save");
        assert_eq!(
            fs::read(&source_2).expect("leer"),
            bytes_source,
            "{ctx}: tocó el origen (2)"
        );
        // La relectura da la imagen del modelo.
        assert_equal(
            &leer(&copy).image,
            &w.m.normalized(),
            &format!("{ctx}: relectura tras el segundo save"),
        );
        without_temporary(&t.names());
    }
}

#[test]
fn row_10_a_real_one_without_conversion_is_overwritten_in_the_work() {
    let mut seen = 0;
    for source in real_2().into_iter().filter(|f| !converts(f)) {
        seen += 1;
        let t = Work::new_2();
        let ctx = source.name_2.clone();
        let mut w = open(&t, &source);
        assert!(!w.s.converted(), "{ctx}: no debía convertirse");
        let source_2 = w.s.path().expect("ruta").to_path_buf();
        w.apply(&editing_that_changes(&w.m));
        let saved = save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
        assert_eq!(saved.path, source_2, "{ctx}: se sobrescribe la copia");
        assert!(saved.copy_reason.is_none(), "{ctx}: copy_reason");
        assert_eq!(
            t.names(),
            vec![source.name_2.clone()],
            "{ctx}: ficheros del trabajo"
        );
        assert_equal(
            &leer(&source_2).image,
            &w.m.normalized(),
            &format!("{ctx}: relectura"),
        );
        // Y nunca en golden/png: lo comprueba la instantánea al soltar el trabajo.
        assert_ne!(
            source_2.parent(),
            Some(golden_png().as_path()),
            "{ctx}: la copia está en golden/png"
        );
    }
    assert!(seen >= 1, "ningún real sin conversión");
}
