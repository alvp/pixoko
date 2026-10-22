//! Propiedades 1 a 4 de la puerta del hito 2 (specs/hito-2-puerta.md), con `proptest`.
//!
//! Casos fijados por la spec (D6): 64 en las propiedades 1 y 2 y 32 en la 3 y la 4 (tocan el
//! disco). `PROPTEST_CASES` solo puede **bajarlos** al desarrollar (`cases`): sin él, valen los de la
//! spec.
//!
//! Los gestos se generan como números crudos que se interpretan contra el modelo actual (`action`):
//! así toda secuencia es válida para el modo y la paleta del momento y se puede cambiar de modo a
//! mitad de secuencia con las conversiones exactas. El modelo restringe la línea a H, V y 45°.
//!
//! **Suposiciones y ambigüedades:**
//! - Propiedad 2: `modified` se comprueba contra el estado de apertura (sin guardar, para no tocar el
//!   disco; los guardados están en las propiedades 1 y 3 y en las filas 5 a 8). Además de recorrer la
//!   historia entera, se prueba una rama nueva tras deshacer unos pasos: el estado no se reutiliza y
//!   `redo` falla.
//! - Propiedad 4: con `Ok`, el modelo se siembra con lo que abrió la sesión (el fichero mutado no tiene
//!   lectura independiente) y se comprueba que editar, guardar y reabrir lo conserva.

use px_core::ImageId;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use proptest::prelude::*;
use px_core::{Document, Mode};
use px_ui::canvas::Canvas;

use crate::files::FileError;
use crate::session::{Saved, Session};

use super::common::*;

/// Casos de una propiedad: los de la spec, o menos si `PROPTEST_CASES` los baja.
fn cases(from_spec: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map_or(from_spec, |requested| requested.min(from_spec))
}

/// Un gesto o una operación en bruto: un tipo y seis números.
type Raw = (u8, [u16; 6]);

fn strategy_raw(max: usize) -> impl Strategy<Value = Vec<Raw>> {
    proptest::collection::vec(
        (any::<u8>(), proptest::array::uniform6(any::<u16>())),
        1..=max,
    )
}

fn mode_of(selection: u8) -> Mode {
    match selection % 4 {
        0 => Mode::Rgba,
        1 => Mode::Indexed,
        2 => Mode::Gray,
        _ => Mode::GrayAlpha,
    }
}

/// Imagen aleatoria repetible: RGBA con alfa 0, 128 y 255 y 3 a 300 colores; indexada con 1 a 40
/// entradas con alfa propio y, a veces, índice transparente; gris; gris con alfa.
fn model_random(mode_2: Mode, width: usize, height: usize, seed: u64) -> Model {
    let mut r = Lcg::new(seed);
    let mut m = model_of(mode_2, width, height, vec![[0; 4]; width * height]);
    const ALPHAS: [u8; 4] = [0, 128, 255, 255];
    match mode_2 {
        Mode::Rgba => {
            let pool = [3u32, 8, 40, 300][r.until(4) as usize];
            let colors: Vec<[u8; 4]> = (0..pool)
                .map(|_| {
                    [
                        r.until(256) as u8,
                        r.until(256) as u8,
                        r.until(256) as u8,
                        ALPHAS[r.until(4) as usize],
                    ]
                })
                .collect();
            for cel in &mut m.cels {
                *cel = colors[r.until(pool) as usize];
            }
        }
        Mode::Indexed => {
            let cap = if r.until(4) == 0 { 40 } else { 8 };
            let length = 1 + r.until(cap);
            m.palette = (0..length)
                .map(|_| {
                    [
                        r.until(256) as u8,
                        r.until(256) as u8,
                        r.until(256) as u8,
                        ALPHAS[r.until(4) as usize],
                    ]
                })
                .collect();
            m.transparent = (r.until(2) == 0).then(|| r.until(length) as u8);
            for cel in &mut m.cels {
                *cel = [r.until(length) as u8, 0, 0, 0];
            }
        }
        Mode::Gray => {
            for cel in &mut m.cels {
                *cel = [r.until(256) as u8, 0, 0, 0];
            }
        }
        Mode::GrayAlpha => {
            for cel in &mut m.cels {
                *cel = [r.until(256) as u8, ALPHAS[r.until(4) as usize], 0, 0];
            }
        }
    }
    m
}

enum TestAction {
    TestStep(TestStep),
    Save,
}

const DIRECTIONS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Un punto en el lienzo o a un par de píxeles de él (para ejercitar el recorte).
fn coord(m: &Model, v: &[u16; 6]) -> (i32, i32) {
    (
        i32::from(v[0]) % (m.width as i32 + 4) - 2,
        i32::from(v[1]) % (m.height as i32 + 4) - 2,
    )
}

fn length(m: &Model, bits: u16) -> i32 {
    1 + i32::from(bits) % (m.width.max(m.height) as i32 + 3)
}

fn choice_of(m: &Model, v: u16) -> Choice {
    if m.mode_2 == Mode::Indexed {
        Choice::TestInput((usize::from(v) % m.palette.len()) as u8)
    } else {
        Choice::TestSlot(1 + (v % 9) as u8)
    }
}

/// Un trayecto de 1 a 3 tramos, cada uno horizontal, vertical o a 45°.
fn path_of(m: &Model, v: &[u16; 6]) -> Vec<(i32, i32)> {
    let mut point = coord(m, v);
    let mut path_2 = vec![point];
    for i in 0..=usize::from(v[5] % 3) {
        let (dx, dy) = DIRECTIONS[usize::from((v[2] >> (3 * i)) % 8)];
        let n = length(m, v[3] >> (4 * i));
        point = (point.0 + dx * n, point.1 + dy * n);
        path_2.push(point);
    }
    path_2
}

/// Interpreta un gesto crudo contra el modelo actual. Las operaciones de paleta solo valen en
/// indexadas y, en las demás, se cambian por un gesto.
fn action(m: &Model, kind: u8, v: &[u16; 6], with_save: bool) -> TestAction {
    let indexed = m.mode_2 == Mode::Indexed;
    let long_palette = m.palette.len().max(1);
    let pencil = || TestStep::Pencil(path_of(m, v), choice_of(m, v[4]));
    let line = || {
        let a = coord(m, v);
        let (dx, dy) = DIRECTIONS[usize::from(v[2] % 8)];
        let n = length(m, v[3]);
        TestStep::Line(a, (a.0 + dx * n, a.1 + dy * n), choice_of(m, v[4]))
    };
    let fill = || TestStep::Fill(coord(m, v), choice_of(m, v[4]));
    let eraser = || TestStep::Eraser(path_of(m, v));
    TestAction::TestStep(match kind % 12 {
        0 | 1 => pencil(),
        2 => line(),
        3 => fill(),
        4 => eraser(),
        5 if indexed => TestStep::ColorInput(
            (usize::from(v[0]) % long_palette) as u8,
            [
                v[1] as u8,
                (v[1] >> 8) as u8,
                v[2] as u8,
                [0, 128, 255][usize::from(v[3] % 3)],
            ],
        ),
        5 => pencil(),
        6 if indexed => {
            let start_2 = usize::from(v[0]) % long_palette;
            TestStep::Rotate {
                start_2: start_2 as u8,
                length: (1 + usize::from(v[1]) % (long_palette - start_2)) as u16,
                steps_2: i32::from(v[2] % 7) - 3,
            }
        }
        6 => line(),
        7 if indexed => TestStep::Move {
            from_index: (usize::from(v[0]) % long_palette) as u8,
            a: (usize::from(v[1]) % long_palette) as u8,
        },
        7 => fill(),
        8 if indexed && long_palette < 256 => TestStep::Add {
            active_2: (usize::from(v[0]) % long_palette) as u8,
        },
        8 => eraser(),
        9 if indexed => TestStep::Transparent(if v[0].is_multiple_of(4) {
            None
        } else {
            Some((usize::from(v[1]) % long_palette) as u8)
        }),
        9 => pencil(),
        10 => TestStep::Convert,
        _ if with_save => return TestAction::Save,
        _ => pencil(),
    })
}

/// `save` y relectura: lo escrito es el modelo normalizado, sin conversiones.
fn save_and_reread(w: &mut World, t: &Work) {
    let without_path = w.s.path().is_none();
    let saved = save(&mut w.s, t.path_2()).unwrap_or_else(|e| panic!("save dio {e}"));
    if without_path {
        assert_eq!(saved.path, t.path_2().join("sin-titulo.png"));
    }
    w.mark_saved();
    let read_2 = read_2_2(&saved.path);
    assert!(read_2.report.is_empty(), "relectura con conversiones");
    assert_equal(&read_2.image, &w.m.normalized(), "relectura tras save");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(64)))]

    /// Propiedad 1: tras cada paso de un guion aleatorio, `Session` es el modelo; al final, la
    /// relectura es el modelo normalizado.
    #[test]
    fn property_1_random_script_against_the_model(
        selection in any::<u8>(),
        width in 1usize..=24,
        height in 1usize..=24,
        seed in any::<u64>(),
        raw in strategy_raw(30),
    ) {
        let t = Work::new_2();
        let model = model_random(mode_of(selection), width, height, seed);
        let session_2 = Session::sample(Document::new(model.a_image()));
        let mut w = World::of_session(session_2, "propiedad 1");
        w.check();
        for (kind, v) in &raw {
            match action(&w.m, *kind, v, true) {
                TestAction::TestStep(step) => {
                    w.apply(&step);
                }
                TestAction::Save => save_and_reread(&mut w, &t),
            }
        }
        save_and_reread(&mut w, &t);
        prop_assert_eq!(t.names(), vec!["sin-titulo.png".to_string()]);
    }

    /// Propiedad 2: `undo` k veces da la instantánea del paso `n − k` y `redo` la vuelve a dar;
    /// `modified` es falso si y solo si el `state_id` es el guardado, y nunca con una imagen distinta.
    #[test]
    fn property_2_history(
        selection in any::<u8>(),
        width in 1usize..=24,
        height in 1usize..=24,
        seed in any::<u64>(),
        raw in strategy_raw(30),
        branch in (any::<u8>(), proptest::array::uniform6(any::<u16>())),
        cut in any::<u8>(),
    ) {
        let model = model_random(mode_of(selection), width, height, seed);
        let session_2 = Session::sample(Document::new(model.a_image()));
        let mut w = World::of_session(session_2, "propiedad 2");
        for (kind, v) in &raw {
            if let TestAction::TestStep(step) = action(&w.m, *kind, v, false) {
                w.apply(&step);
            }
        }
        let n = w.steps_2();
        prop_assert_eq!(w.pos, n);

        // `undo` k veces: la instantánea anotada tras el paso n − k (lo comprueba el mundo).
        for k in 1..=n {
            w.undo();
            prop_assert_eq!(w.pos, n - k);
        }
        prop_assert_eq!(w.s.document.state_id(), 0);
        prop_assert!(!w.s.modified());
        w.undo();
        // `redo` las vuelve a dar.
        for k in 1..=n {
            w.redo();
            prop_assert_eq!(w.pos, k);
        }
        w.redo();
        // Para todo k: k pasos atrás y k adelante.
        for k in 1..=n {
            for _ in 0..k {
                w.undo();
            }
            for _ in 0..k {
                w.redo();
            }
            prop_assert_eq!(w.pos, n);
        }

        // Una rama nueva tras deshacer unos pasos: `redo` falla y no se reutiliza ningún estado.
        let back = usize::from(cut) % (n + 1);
        for _ in 0..back {
            w.undo();
        }
        let (kind, v) = branch;
        let change = match action(&w.m, kind, &v, false) {
            TestAction::TestStep(step) => w.apply(&step),
            TestAction::Save => false,
        };
        if change {
            prop_assert_eq!(w.steps_2(), n - back + 1);
        }
        w.redo();
        w.undo_all();
        w.redo_all();
    }

    /// Propiedad 3: con secuencias aleatorias de `save`, `save_as` y `save_to` desde una copia, el
    /// directorio de trabajo contiene exactamente los ficheros de los `Saved` devueltos (y el de
    /// origen), sin temporales; cada uno es la relectura esperada y `golden/png/` queda igual.
    #[test]
    fn property_3_only_what_is_asked_is_written(
        source in 0usize..6,
        operations in proptest::collection::vec(
            (0u8..4, proptest::array::uniform6(any::<u16>()), 0usize..NAMES.len()),
            1..=8,
        ),
    ) {
        let t = Work::new_2();
        let mut sources = real_2();
        sources.push(source_rgb_8());
        sources.push(source_with_iccp());
        let mut w = open(&t, &sources[source]);
        let source_2 = w.s.path().expect("ruta").to_path_buf();
        let bytes_source = fs::read(&source_2).expect("leer el origen");
        let mut written: BTreeMap<PathBuf, Model> = BTreeMap::new();

        for (kind, v, name_2) in &operations {
            let actual = w.s.path().expect("ruta").to_path_buf();
            let converted_2 = w.s.converted();
            match kind {
                0 => {
                    let step = TestStep::Pencil(path_of(&w.m, v), choice_of(&w.m, v[4]));
                    w.apply(&step);
                }
                1 => {
                    let target_2 = if converted_2 { free_copy(&actual) } else { actual.clone() };
                    let saved = save(&mut w.s, t.path_2()).expect("save");
                    register(&mut w, &mut written, &saved, &target_2, converted_2);
                }
                2 => {
                    let target_2 = free_copy(&actual);
                    let saved = save_as(&mut w.s, t.path_2()).expect("save_as");
                    register(&mut w, &mut written, &saved, &target_2, false);
                }
                _ => {
                    let chosen = t.path_2().join(NAMES[*name_2]);
                    let target_2 = with_png(&chosen);
                    let exists = target_2 != chosen && fs::symlink_metadata(&target_2).is_ok();
                    let modified_2 = w.s.modified();
                    let result = save_in(&mut w.s, &chosen);
                    if exists {
                        prop_assert_eq!(result, Err(FileError::Exists(target_2)));
                        prop_assert_eq!(w.s.path(), Some(actual.as_path()));
                        prop_assert_eq!(w.s.modified(), modified_2);
                    } else {
                        let copy = converted_2 && target_2 == actual;
                        let expected_2 = if copy { free_copy(&actual) } else { target_2 };
                        let saved = result.expect("save_to");
                        register(&mut w, &mut written, &saved, &expected_2, copy);
                    }
                }
            }
        }

        // Exactamente los ficheros de los `Saved` y el de origen, sin temporales.
        let mut expected_2_2_2: BTreeSet<String> = written
            .keys()
            .map(|path_2| path_2.file_name().expect("nombre").to_string_lossy().into_owned())
            .collect();
        expected_2_2_2.insert(source_2.file_name().expect("nombre").to_string_lossy().into_owned());
        let there_is = t.names();
        without_temporary(&there_is);
        prop_assert_eq!(there_is, expected_2_2_2.into_iter().collect::<Vec<_>>());
        for (path_2, model) in &written {
            assert_equal(&read_2_2(path_2).image, model, &format!("{}", path_2.display()));
        }
        if !written.contains_key(&source_2) {
            prop_assert_eq!(fs::read(&source_2).expect("leer el origen"), bytes_source);
        }
        // `golden/png/` queda igual: lo comprueba el guardián del trabajo al soltarse.
    }

    /// Propiedad 4: un real con 1 a 4 bytes cambiados da `Ok` o `Err` sin pánico; con `Err` la
    /// sesión queda igual; con `Ok`, editar, guardar y reabrir da el modelo.
    #[test]
    fn property_4_mutated_real_ones(
        real in 0usize..4,
        changes_2 in proptest::collection::vec((any::<usize>(), 1u8..=255), 1..=4),
    ) {
        let t = Work::new_2();
        let source = real_2().remove(real);
        let mut mutated = source.bytes.clone();
        for (position, xor) in &changes_2 {
            let at = position % mutated.len();
            mutated[at] ^= xor;
        }
        let path_mutated = t.write("mutado.png", &mutated);

        // Una sesión con cambios sin guardar, que abre el fichero mutado descartándolos.
        let mut w = open(&t, &source);
        w.apply(&editing_that_changes(&w.m));
        let document = w.s.document.clone();
        let (id, path_2, modified_2) = (
            w.s.document.state_id(),
            w.s.path().map(|p| p.to_path_buf()),
            w.s.modified(),
        );
        match w.s.open(&path_mutated, Canvas::MAX_SIDE, true) {
            Err(_) => {
                prop_assert!(w.s.document == document, "un open fallido cambió el documento");
                prop_assert_eq!(w.s.document.state_id(), id);
                prop_assert_eq!(w.s.path().map(|p| p.to_path_buf()), path_2);
                prop_assert_eq!(w.s.modified(), modified_2);
                w.check();
            }
            Ok(_) => {
                let mut w = World::of_session(w.s, "mutado");
                prop_assert_eq!(w.s.document.state_id(), 0);
                prop_assert!(!w.s.modified());
                if let Some(step) = edit_that_changes_opt(&w.m) {
                    prop_assert!(w.apply(&step));
                }
                let open_2 = w.s.path().expect("ruta").to_path_buf();
                let target_2 = if w.s.converted() { free_copy(&open_2) } else { open_2 };
                let saved = save(&mut w.s, t.path_2()).expect("save");
                prop_assert_eq!(&saved.path, &target_2);
                let mut other = session_empty();
                other.open(&saved.path, Canvas::MAX_SIDE, false).expect("reabrir");
                let image_2 = other.document.image(ImageId::from_raw(0)).expect("imagen");
                assert_equal(image_2, &w.m.normalized(), "mutado reabierto");
            }
        }
    }
}

/// Nombres como los que da un diálogo: sin extensión, con ella en minúsculas o mayúsculas, con
/// espacios y varios puntos.
const NAMES: [&str; 6] = ["a", "b.png", "C.PNG", "con espacios", "d.e", "otra.png"];

/// Tras un guardado que salió bien: destino y motivo de copia esperados, `modified` falso y
/// relectura igual al modelo normalizado.
fn register(
    w: &mut World,
    written: &mut BTreeMap<PathBuf, Model>,
    saved: &Saved,
    expected_2: &std::path::Path,
    with_reason: bool,
) {
    assert_eq!(saved.path, expected_2, "destino del guardado");
    assert_eq!(saved.copy_reason.is_some(), with_reason, "copy_reason");
    w.mark_saved();
    assert_eq!(w.s.path(), Some(expected_2), "ruta tras guardar");
    assert!(!w.s.converted(), "sigue convertida tras guardar");
    assert_equal(
        &read_2_2(expected_2).image,
        &w.m.normalized(),
        "relectura tras guardar",
    );
    written.insert(expected_2.to_path_buf(), w.m.normalized());
}
