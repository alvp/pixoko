//! Filas 5 a 9 y 11 de la puerta del hito 3 (specs/hito-3-puerta.md, «Casos»). La fila 10 está en
//! `large.rs` (`#[ignore]`) y las propiedades 1 a 3 en `properties.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use px_formats::{read_png, write_png};
use px_raster::compose;
use px_ui::canvas::Canvas;

use super::model::Stack;
use super::scripts::{
    script_indexed, script_rgba, script_short_indexed, script_short_rgba, session_indexed_2,
    session_rgba,
};
use super::world::World3;
use super::{
    Work3, check_golden_aseprite, equal_except_rgb_with_alpha_0, golden_aseprite, initial,
    outside_golden_aseprite,
};
use crate::convert_mode::WAIT_FRAMES;
use crate::dialogs::{DialogKind, Outcome, SaveRoute, proposed_save_name};
use crate::gate::common::sha256::sha256_hex;
use crate::gate::common::{
    compare_snapshots, dialogs_2, names_in, session_empty, without_temporary,
};
use crate::save_wait::{StartSave, aseprite_pixels, save_pixels, start_save};
use crate::session::with_flatten_notice;

// ---------------------------------------------------------------------------------------
// Filas 5 y 6: la sesión de punta a punta.
// ---------------------------------------------------------------------------------------

#[test]
fn row_05_rgba_end_to_end() {
    let mut w = World3::of_session(session_rgba(), "rgba");
    script_rgba(&mut w);
    assert!(w.steps_2() >= 35, "el guion hace {} pasos", w.steps_2());
    assert!(w.s.modified());
}

#[test]
fn row_06_indexed_end_to_end() {
    let mut w = World3::of_session(session_indexed_2(), "indexado");
    script_indexed(&mut w);
    assert!(w.steps_2() >= 20, "el guion hace {} pasos", w.steps_2());
    // La paleta es la misma en todas las celdas tras cada orden (la comprueba `equal` del modelo,
    // que compara la paleta del documento) y, además, la de cada celda.
    let palette = w.s.document.palette().expect("paleta").clone();
    for layer_2 in w.s.document.layers() {
        if let Some(image_2) = w.s.document.image(layer_2.cel()) {
            assert_eq!(image_2.palette(), Some(&palette), "paleta de la celda");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: deshacer y rehacer todo.
// ---------------------------------------------------------------------------------------

fn history(mut w: World3) {
    let steps_2 = w.steps_2();
    let last_2_2 = w.read_2_2();
    assert_eq!(w.undo_all(), steps_2, "deshacer hasta el principio");
    assert_eq!(w.s.document.state_id(), 0, "al principio, state_id 0");
    assert!(!w.s.modified(), "al principio no hay cambios");
    assert_eq!(w.m.layers_2.len(), 1, "al principio, el documento original");
    assert_eq!(w.redo_all(), steps_2, "rehacer hasta el final");
    last_2_2
        .equal_content(&w.read_2_2())
        .expect("el final es el final");
    assert!(w.s.modified());

    // Guardado en medio, una orden nueva tras deshacer y el estado guardado deja de ser alcanzable.
    for _ in 0..5 {
        w.undo();
    }
    w.s.mark_unmodified();
    w.mark_saved();
    assert!(!w.s.modified());
    w.undo();
    assert!(w.s.modified(), "un paso antes de lo guardado");
    assert!(w.apply(&super::model::Order::New));
    assert!(w.s.modified());
    w.redo(); // al final de la rama: `NothingToRedo`, comprobado dentro.
    w.undo();
    assert!(
        w.s.modified(),
        "el estado guardado ya no es alcanzable: sigue modificado"
    );
    w.check();
}

#[test]
fn row_07_undo_and_redo_everything_in_rgba_and_in_indexed() {
    let mut rgba = World3::of_session(session_rgba(), "historia rgba");
    script_rgba(&mut rgba);
    history(rgba);
    let mut indexed_2 = World3::of_session(session_indexed_2(), "historia indexado");
    script_indexed(&mut indexed_2);
    history(indexed_2);
}

// ---------------------------------------------------------------------------------------
// Fila 8: guardar y reabrir.
// ---------------------------------------------------------------------------------------

fn save_and_reopen(indexed_2: bool) {
    let t = Work3::new_2();
    let label_2 = if indexed_2 {
        "guardar indexado"
    } else {
        "guardar rgba"
    };
    let mut w = World3::of_session(
        if indexed_2 {
            session_indexed_2()
        } else {
            session_rgba()
        },
        label_2,
    );
    if indexed_2 {
        script_short_indexed(&mut w);
    } else {
        script_short_rgba(&mut w);
    }
    let layers_2 = w.m.layers_2.len();
    assert!(layers_2 >= 3);

    // `Ctrl+S` sin ruta con 3 o más capas: diálogo que propone `without-title.aseprite`.
    assert_eq!(w.s.save_route(), SaveRoute::Dialog);
    let (mut dialogs_2, control) = dialogs_2(t.path_2());
    let proposed = PathBuf::from(proposed_save_name(w.s.path(), layers_2));
    assert!(dialogs_2.begin(
        DialogKind::Save,
        false,
        Some(&proposed),
        w.s.converted(),
        0.0,
        None
    ));
    assert_eq!(
        control.last_2().file_name.as_deref(),
        Some("sin-titulo.aseprite")
    );
    let target_2 = t.path_2().join("prueba.aseprite");
    control.choose(&target_2);
    let finished = dialogs_2.poll(1.0).expect("el diálogo terminó");
    let Outcome::Chosen(chosen) = finished.outcome else {
        panic!("el diálogo no eligió ninguna ruta");
    };
    outside_golden_aseprite(&chosen);
    let saved = w.s.save_to(&chosen).expect("guardar el .aseprite");
    assert_eq!(saved.path, target_2);
    assert!(saved.copy_reason.is_none());
    w.mark_saved();
    assert_eq!(
        w.s.path(),
        Some(target_2.as_path()),
        "la sesión pasa a ese fichero"
    );
    assert_eq!(w.s.save_route(), SaveRoute::Direct);
    assert_eq!(t.names(), vec!["prueba.aseprite".to_string()]);

    // Reabrir: la capa activa es la de arriba y el documento es el de antes, sin ids.
    let mut other = session_empty();
    let notice = other
        .open(&target_2, Canvas::MAX_SIDE, true)
        .expect("reabrir el .aseprite");
    assert!(notice.contains("prueba.aseprite"), "{notice}");
    assert!(!other.converted() && !other.modified());
    assert_eq!(other.document.state_id(), 0);
    let last_2 = other.document.layers().last().expect("capas").id();
    assert_eq!(
        other.active_layer(),
        last_2,
        "la capa activa es la de arriba"
    );
    let read = Stack::of_document(&other.document, layers_2 - 1);
    w.m.canonical()
        .equal_content(&read.canonical())
        .expect("el reabierto es el documento guardado");
    let before = compose(&w.s.document).expect("compose");
    let after_2 = compose(&other.document).expect("compose");
    equal_except_rgb_with_alpha_0(&before, &after_2).expect("la composición sobrevive");
    if !indexed_2 {
        // Vector a mano de 3.2: Multiplicar de `SOURCE` sobre `TEST_BACKGROUND`.
        assert_eq!(
            super::rgba_of(&after_2, 0, 0),
            px_core::Rgba::new(69, 11, 45, 255)
        );
    }

    // Exportar el PNG aplanado: ni ruta, ni historia ni «modificado» cambian.
    let (id, path_doc) = (w.s.document.state_id(), w.s.path().map(Path::to_path_buf));
    let plane = t.path_2().join("plano");
    outside_golden_aseprite(&plane);
    let exported = w.s.export_flat(&plane).expect("exportar");
    assert_eq!(exported.path, t.path_2().join("plano.png"));
    assert_eq!(w.s.document.state_id(), id);
    assert_eq!(w.s.path(), path_doc.as_deref());
    assert!(!w.s.modified());
    let png = read_png(&fs::read(&exported.path).expect("leer"))
        .expect("PNG")
        .image;
    equal_except_rgb_with_alpha_0(&png, &before).expect("el PNG exportado es la composición");
    if !indexed_2 {
        assert_eq!(
            super::rgba_of(&png, 0, 0),
            px_core::Rgba::new(69, 11, 45, 255)
        );
    }

    // Guardar como PNG: aplanado, con su aviso, y la sesión pasa a ese PNG.
    let notice = with_flatten_notice("guardado".to_string(), &w.s.document);
    assert!(notice.contains("se guarda aplanado"), "{notice}");
    let other_2 = t.path_2().join("aplanado.png");
    outside_golden_aseprite(&other_2);
    let saved = w.s.save_to(&other_2).expect("guardar el PNG aplanado");
    assert_eq!(saved.path, other_2);
    w.mark_saved();
    let png = read_png(&fs::read(&other_2).expect("leer"))
        .expect("PNG")
        .image;
    equal_except_rgb_with_alpha_0(&png, &before).expect("el PNG guardado es la composición");
    // Con 3 o más capas y ruta PNG, `Ctrl+S` pide el diálogo (y propone `.aseprite`).
    assert_eq!(w.s.save_route(), SaveRoute::Dialog);
    assert_eq!(
        proposed_save_name(w.s.path(), layers_2),
        "aplanado.aseprite"
    );

    let names = t.names();
    without_temporary(&names);
    assert_eq!(
        names,
        vec![
            "aplanado.png".to_string(),
            "plano.png".to_string(),
            "prueba.aseprite".to_string()
        ]
    );
}

#[test]
fn row_08_save_and_reopen_in_rgba() {
    save_and_reopen(false);
}

#[test]
fn row_08_save_and_reopen_in_indexed() {
    save_and_reopen(true);
}

#[test]
fn row_08_a_png_with_two_layers_asks_for_the_dialog_and_proposes_aseprite() {
    let t = Work3::new_2();
    let base = t.path_2().join("base.png");
    let image_2 =
        px_core::Image::filled(4, 4, px_core::Pixel::Rgba(px_core::Rgba::new(1, 2, 3, 255)))
            .expect("imagen");
    fs::write(&base, write_png(&image_2).expect("PNG")).expect("escribir base.png");
    let mut session_2 = session_empty();
    session_2
        .open(&base, Canvas::MAX_SIDE, true)
        .expect("abrir");
    assert_eq!(
        session_2.save_route(),
        SaveRoute::Direct,
        "una capa: guarda sin diálogo"
    );
    let mut w = World3::of_session(session_2, "png con capas");
    assert!(w.apply(&super::model::Order::New));
    assert_eq!(w.s.save_route(), SaveRoute::Dialog);
    assert_eq!(proposed_save_name(w.s.path(), 2), "base.aseprite");
    // Elegir `.png` en ese diálogo guarda aplanado, como siempre.
    let (mut dialogs_2, control) = dialogs_2(t.path_2());
    let proposed = PathBuf::from("base.aseprite");
    assert!(dialogs_2.begin(DialogKind::Save, false, Some(&proposed), false, 0.0, None));
    control.choose(&t.path_2().join("elegido.png"));
    let Some(Outcome::Chosen(path_2)) = dialogs_2.poll(1.0).map(|f| f.outcome) else {
        panic!("el diálogo no eligió");
    };
    let saved = w.s.save_to(&path_2).expect("guardar");
    assert_eq!(saved.path, t.path_2().join("elegido.png"));
}

// ---------------------------------------------------------------------------------------
// Fila 9: los 29 por la sesión.
// ---------------------------------------------------------------------------------------

fn cases_of_the_corpus() -> Vec<String> {
    let mut cases: Vec<String> = names_in(&golden_aseprite())
        .into_iter()
        .filter_map(|n| n.strip_suffix(".aseprite").map(str::to_string))
        .collect();
    cases.sort();
    cases
}

#[test]
fn row_09_the_29_through_the_session_are_opened_saved_and_reopened() {
    let t = Work3::new_2();
    let cases = cases_of_the_corpus();
    assert_eq!(cases.len(), 29, "golden/aseprite tiene los 29 casos");
    for case in &cases {
        let name_2 = format!("{case}.aseprite");
        let copy = t.copy_of_the_corpus(case, &name_2);
        let mut s = session_empty();
        let notice = s
            .open(&copy, Canvas::MAX_SIDE, true)
            .unwrap_or_else(|e| panic!("{case}: abrir dio {e}"));
        assert!(notice.contains(&name_2), "{case}: {notice}");
        assert!(
            notice.contains(&format!("{}×{}", s.document.width(), s.document.height())),
            "{case}: {notice}"
        );
        assert!(!s.converted(), "{case}: sin avisos de lo ignorado");
        assert_eq!(s.document.state_id(), 0, "{case}");
        assert!(!s.modified(), "{case}");
        let layers_2 = s.document.layers().len();
        let last_2 = s.document.layers().last().expect("capas").id();
        assert_eq!(
            s.active_layer(),
            last_2,
            "{case}: la activa es la de arriba"
        );
        let original = Stack::of_document(&s.document, layers_2 - 1);

        outside_golden_aseprite(&copy);
        let saved = s
            .save(t.path_2())
            .unwrap_or_else(|e| panic!("{case}: guardar dio {e}"));
        assert_eq!(saved.path, copy, "{case}: sobrescribe la copia");
        assert!(saved.copy_reason.is_none(), "{case}");

        let mut other = session_empty();
        other
            .open(&copy, Canvas::MAX_SIDE, true)
            .unwrap_or_else(|e| panic!("{case}: reabrir dio {e}"));
        let read = Stack::of_document(&other.document, layers_2 - 1);
        original
            .canonical()
            .equal_content(&read.canonical())
            .unwrap_or_else(|e| panic!("{case}: {e}"));
    }
    let names = t.names();
    without_temporary(&names);
    assert_eq!(names.len(), 29, "una copia por caso y nada más");
}

// ---------------------------------------------------------------------------------------
// Fila 10 (condición 6): «Guardando…» al guardar un `.aseprite` grande.
// ---------------------------------------------------------------------------------------

/// Un documento de `side_2` × `side_2` con una capa por elemento de `visible` (`Some` sin imagen no
/// existe: las vacías son `None`).
fn document_of_cels(side_2: u32, layers_2: &[(bool, bool)]) -> px_core::Document {
    use px_core::{BlendMode, Image, LayerSpec, Mode, Pixel, Rgba};
    let specs = layers_2
        .iter()
        .enumerate()
        .map(|(i, &(visible, with_image))| LayerSpec {
            name: format!("capa {i}"),
            visible,
            opacity: 255,
            blend: BlendMode::Normal,
            locked: false,
            image: with_image.then(|| {
                Image::filled(side_2, side_2, Pixel::Rgba(Rgba::new(i as u8, 2, 3, 255)))
                    .expect("imagen")
            }),
        })
        .collect();
    px_core::Document::from_layers(side_2, side_2, Mode::Rgba, None, specs).expect("documento")
}

#[test]
fn row_10_a_big_aseprite_asks_for_saving_and_waits_the_usual_frames() {
    // 4 capas con imagen de 2048² son exactamente 2²⁴ píxeles de celda: se guarda ya; con 5, no.
    let four = document_of_cels(2048, &[(true, true); 4]);
    assert_eq!(aseprite_pixels(&four), 1 << 24);
    assert_eq!(start_save(save_pixels(&four, true)), StartSave::Now);
    let five = document_of_cels(2048, &[(true, true); 5]);
    assert_eq!(aseprite_pixels(&five), 5 * 2048 * 2048);
    let StartSave::Later(mut waits) = start_save(save_pixels(&five, true)) else {
        panic!("5 capas de 2048² pasan de 2²⁴ píxeles: debía esperar");
    };
    // La misma secuencia que «Guardando…» de un PNG: ni un fotograma antes ni después.
    for frame in 1..WAIT_FRAMES {
        assert!(!waits.tick(), "el fotograma {frame} aún no guarda");
    }
    assert!(
        waits.tick(),
        "guarda exactamente en el fotograma {WAIT_FRAMES}"
    );
    assert!(!waits.tick(), "y no vuelve a guardar");
    // Las celdas vacías no se escriben y no cuentan; las ocultas sí (el `.aseprite` las guarda),
    // y un PNG solo compone las visibles.
    let blend_2 = document_of_cels(
        2048,
        &[
            (true, true),
            (false, true),
            (false, true),
            (true, false),
            (true, true),
        ],
    );
    assert_eq!(aseprite_pixels(&blend_2), 4 * 2048 * 2048);
    assert_eq!(save_pixels(&blend_2, true), 4 * 2048 * 2048);
    assert_eq!(save_pixels(&blend_2, false), 2 * 2048 * 2048);
}

// ---------------------------------------------------------------------------------------
// Fila 11: `golden/aseprite/` intacto y los controles negativos.
// ---------------------------------------------------------------------------------------

#[test]
fn row_11_golden_aseprite_intact_and_the_control_knows_how_to_fail() {
    let _t = Work3::new_2();
    let (snapshot, fingerprints) = initial();
    assert_eq!(snapshot.len(), 88, "29 casos de tres ficheros y el README");
    assert_eq!(fingerprints.len(), 88);
    check_golden_aseprite().expect("golden/aseprite es el de antes");
    // Control: un byte cambiado, un fichero más y uno menos dan `Err`, y el SHA-256 lo ve.
    let mut broken = snapshot.clone();
    broken.get_mut("mixed-rgba.png").expect("fichero")[40] ^= 1;
    let error = compare_snapshots(snapshot, &broken).expect_err("un byte cambiado");
    assert!(error.contains("mixed-rgba.png"), "{error}");
    assert_ne!(
        sha256_hex(&broken["mixed-rgba.png"]),
        fingerprints["mixed-rgba.png"]
    );
    let mut with = snapshot.clone();
    with.insert("intruso.aseprite".to_string(), vec![1, 2, 3]);
    let error = compare_snapshots(snapshot, &with).expect_err("un fichero más");
    assert!(error.contains("aparece intruso.aseprite"), "{error}");
    let mut without = snapshot.clone();
    without.remove("layer-locked.aseprite");
    let error = compare_snapshots(snapshot, &without).expect_err("un fichero menos");
    assert!(error.contains("falta layer-locked.aseprite"), "{error}");
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/aseprite")]
fn row_11_a_golden_aseprite_path_panics() {
    outside_golden_aseprite(&golden_aseprite().join("nuevo.aseprite"));
}

#[test]
fn row_11_the_work_hangs_from_target_and_is_deleted_even_if_the_test_fails() {
    use std::sync::Mutex;
    let path_2 = Mutex::new(None::<PathBuf>);
    let t = Work3::new_2();
    let own = t.path_2().to_path_buf();
    assert!(
        own.components().any(|c| c.as_os_str() == "pixoko-gate"),
        "{}",
        own.display()
    );
    assert!(own.starts_with(
        fs::canonicalize(crate::gate::common::root_repo().join("target")).expect("target")
    ));
    drop(t);
    assert!(!own.exists(), "el trabajo se borra al soltarse");
    let failure = std::panic::catch_unwind(|| {
        let t = Work3::new_2();
        *path_2.lock().unwrap_or_else(|e| e.into_inner()) = Some(t.path_2().to_path_buf());
        fs::write(t.path_2().join("x"), b"x").expect("escribir");
        panic!("la prueba falla a propósito");
    });
    assert!(failure.is_err());
    let own = path_2
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .expect("ruta");
    assert!(!own.exists(), "también se borra si la prueba falla");
}
