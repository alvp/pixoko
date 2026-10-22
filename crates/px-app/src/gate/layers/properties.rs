//! Propiedades 1 a 3 de la puerta del hito 3 (specs/hito-3-puerta.md, `proptest`): 64 casos las dos
//! primeras y 32 la tercera (toca el disco); se baja con `PROPTEST_CASES` solo al desarrollar. La
//! propiedad 4 está en `px-golden`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{BlendMode, Mode};
use px_formats::read_png;
use px_raster::compose;
use px_ui::canvas::Canvas;

use super::model::{Cel4, Order, Stack};
use super::scripts::{session_indexed_of, session_rgba_of};
use super::world::World3;
use super::{Work3, check_golden_aseprite, equal_except_rgb_with_alpha_0, outside_golden_aseprite};
use crate::gate::common::{golden_intact, session_empty, without_temporary};
use crate::merge_flow::MergeKind;

/// Una orden abstracta: se concreta contra el estado del modelo (los índices, las coordenadas y los
/// colores dependen de él).
#[derive(Clone, Debug)]
enum A {
    New,
    Duplicate,
    Remove,
    Raise,
    Lower,
    Activate(u8),
    Visible(u8, bool),
    Lock(u8, bool),
    Opacity(u8, u8),
    Blend(u8, u8),
    Pencil(u8, u8, u8, u8, u8),
    Line(u8, u8, u8, u8, u8),
    Fill(u8, u8, u8),
    Eraser(u8, u8, u8, u8),
    Merge(u8),
    ConvertIndexed,
    ConvertRgba,
    Save,
}

fn order_a() -> impl Strategy<Value = A> {
    let b = any::<u8>;
    prop_oneof![
        3 => Just(A::New),
        1 => Just(A::Duplicate),
        1 => Just(A::Remove),
        1 => Just(A::Raise),
        1 => Just(A::Lower),
        3 => b().prop_map(A::Activate),
        2 => (b(), any::<bool>()).prop_map(|(i, v)| A::Visible(i, v)),
        1 => (b(), any::<bool>()).prop_map(|(i, v)| A::Lock(i, v)),
        2 => (b(), b()).prop_map(|(i, o)| A::Opacity(i, o)),
        2 => (b(), b()).prop_map(|(i, m)| A::Blend(i, m)),
        4 => (b(), b(), b(), b(), b()).prop_map(|(x, y, d, l, c)| A::Pencil(x, y, d, l, c)),
        3 => (b(), b(), b(), b(), b()).prop_map(|(x, y, d, l, c)| A::Line(x, y, d, l, c)),
        3 => (b(), b(), b()).prop_map(|(x, y, c)| A::Fill(x, y, c)),
        2 => (b(), b(), b(), b()).prop_map(|(x, y, d, l)| A::Eraser(x, y, d, l)),
        2 => b().prop_map(A::Merge),
        1 => Just(A::ConvertIndexed),
        1 => Just(A::ConvertRgba),
        1 => Just(A::Save),
    ]
}

/// Los seis colores RGBA de las propiedades (con translúcidos, nunca alfa 0) o un índice de 1 a 15.
fn color(m: &Stack, n: u8) -> Cel4 {
    const RGBA: [Cel4; 6] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 128],
        [10, 20, 30, 255],
        [200, 100, 0, 64],
    ];
    match m.mode_2 {
        Mode::Indexed => {
            // Una entrada de la paleta que no sea la transparente.
            let palette = m.palette.as_ref().expect("paleta");
            let t = palette.transparent_index();
            let valid: Vec<u8> = (0..palette.len() as u8).filter(|&j| Some(j) != t).collect();
            [
                valid
                    .get(usize::from(n) % valid.len().max(1))
                    .copied()
                    .unwrap_or(0),
                0,
                0,
                0,
            ]
        }
        _ => RGBA[usize::from(n) % RGBA.len()],
    }
}

/// Un segmento horizontal, vertical o a 45° que cabe en el lienzo desde `(x, y)`.
fn segment_inside(m: &Stack, x: u8, y: u8, dir: u8, largo: u8) -> ((i32, i32), (i32, i32)) {
    let (width, height) = (m.width as i32, m.height as i32);
    let start_2 = (i32::from(x) % width, i32::from(y) % height);
    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let (dx, dy) = DIRS[usize::from(dir) % 8];
    let until = |pos: i32, d: i32, cap: i32| match d {
        1 => cap - 1 - pos,
        -1 => pos,
        _ => i32::MAX,
    };
    let maximum = until(start_2.0, dx, width).min(until(start_2.1, dy, height));
    let steps_2 = i32::from(largo) % (maximum + 1);
    (
        start_2,
        (start_2.0 + dx * steps_2, start_2.1 + dy * steps_2),
    )
}

fn concrete(a: &A, m: &Stack) -> Order {
    let layer_2 = |i: u8| usize::from(i) % m.layers_2.len();
    match a {
        A::New => Order::New,
        A::Duplicate => Order::Duplicate,
        A::Remove => Order::Remove,
        A::Raise => Order::Mover(1),
        A::Lower => Order::Mover(-1),
        A::Activate(i) => Order::Activate(layer_2(*i)),
        A::Visible(i, v) => Order::Visible(layer_2(*i), *v),
        A::Lock(i, v) => Order::Lock(layer_2(*i), *v),
        A::Opacity(i, o) => Order::Opacity(layer_2(*i), *o),
        A::Blend(i, k) => Order::Blend(layer_2(*i), BlendMode::ALL[usize::from(*k) % 8]),
        A::Pencil(x, y, d, l, c) => {
            let (a, b) = segment_inside(m, *x, *y, *d, *l);
            Order::Pencil(vec![a, b], color(m, *c))
        }
        A::Line(x, y, d, l, c) => {
            let (a, b) = segment_inside(m, *x, *y, *d, *l);
            Order::Line(a, b, color(m, *c))
        }
        A::Fill(x, y, c) => Order::Fill(
            (
                i32::from(*x) % m.width as i32,
                i32::from(*y) % m.height as i32,
            ),
            color(m, *c),
        ),
        A::Eraser(x, y, d, l) => {
            let (a, b) = segment_inside(m, *x, *y, *d, *l);
            Order::Eraser(vec![a, b])
        }
        A::Merge(k) => Order::Merge(match k % 3 {
            0 => MergeKind::Down,
            1 => MergeKind::Visible,
            _ => MergeKind::Flatten,
        }),
        // Solo tienen sentido en el modo de origen; en el otro son una elección de capa sin paso.
        A::ConvertIndexed if m.mode_2 == Mode::Rgba => Order::Convert(Mode::Indexed),
        A::ConvertRgba if m.mode_2 == Mode::Indexed => Order::Convert(Mode::Rgba),
        A::ConvertIndexed | A::ConvertRgba | A::Save => Order::Activate(m.active_2),
    }
}

/// Un indexado sin índice transparente (el de un RGBA de una capa sin alfa 0 convertido).
fn without_transparent(w: &World3) -> bool {
    w.m.mode_2 == Mode::Indexed
        && w.m
            .palette
            .as_ref()
            .is_none_or(|p| p.transparent_index().is_none())
}

fn world(indexed_2: bool, width: u32, height: u32) -> World3 {
    if indexed_2 {
        World3::of_session(session_indexed_of(width, height), "propiedad")
    } else {
        World3::of_session(session_rgba_of(width, height), "propiedad")
    }
}

fn run(w: &mut World3, commands: &[A]) {
    for a in commands {
        let order = concrete(a, &w.m);
        w.apply(&order);
        if matches!(a, A::Save) {
            w.s.mark_unmodified();
            w.mark_saved();
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// P1. Guion aleatorio contra el modelo: tras cada paso la pila de la sesión es la del modelo
    /// y la composición la de `from_layers`; al final, guardar en `.aseprite` y reabrir da el
    /// mismo contenido (con la exención exacta del RGB bajo alfa 0).
    #[test]
    fn property_1_random_script_against_the_model(
        indexed_2 in any::<bool>(),
        width in 1u32..=10,
        height in 1u32..=10,
        commands in vec(order_a(), 1..=40),
    ) {
        let mut w = world(indexed_2, width, height);
        run(&mut w, &commands);
        let t = Work3::new_2();
        let path_2 = t.path_2().join("p1.aseprite");
        outside_golden_aseprite(&path_2);
        if without_transparent(&w) {
            // Indexado sin índice transparente: el escritor lo rechaza (3.8, decisión 2) y no
            // deja ningún fichero.
            let error = w.s.save_to(&path_2).expect_err("indexado sin índice transparente");
            prop_assert!(error.to_string().contains("no tiene entrada transparente"), "{error}");
            prop_assert!(t.names().is_empty());
            return Ok(());
        }
        w.s.save_to(&path_2).expect("guardar el .aseprite");
        let mut other = session_empty();
        other.open(&path_2, Canvas::MAX_SIDE, true).expect("reabrir");
        let layers_2 = other.document.layers().len();
        let read = Stack::of_document(&other.document, layers_2 - 1);
        prop_assert!(
            w.m.canonical().equal_content(&read.canonical()).is_ok(),
            "{:?}",
            w.m.canonical().equal_content(&read.canonical())
        );
        let antes = compose(&w.s.document).expect("compose");
        let after_2 = compose(&other.document).expect("compose");
        prop_assert_eq!(equal_except_rgb_with_alpha_0(&antes, &after_2), Ok(()));
    }

    /// P2. Historia con capas: deshacer k veces da la instantánea anotada tras el paso `n − k`
    /// para todo k (con una capa activa que existe siempre) y rehacer las vuelve a dar; «modificado»
    /// es falso si y solo si el `state_id` es el guardado, y nunca con otro contenido.
    #[test]
    fn property_2_the_history_with_layers_is_exact(
        indexed_2 in any::<bool>(),
        width in 1u32..=10,
        height in 1u32..=10,
        commands in vec(order_a(), 1..=40),
    ) {
        let mut w = world(indexed_2, width, height);
        run(&mut w, &commands);
        let (steps_2, final_) = (w.steps_2(), w.leer());
        prop_assert_eq!(w.undo_all(), steps_2);
        prop_assert_eq!(w.s.document.state_id(), 0);
        prop_assert_eq!(w.redo_all(), steps_2);
        prop_assert_eq!(final_.equal_content(&w.leer()), Ok(()));
        // Un paso a paso desde el final, cada vez comprobado dentro de `undo` y `redo`.
        for _ in 0..steps_2 {
            w.undo();
        }
        for _ in 0..steps_2 {
            w.redo();
        }
        prop_assert_eq!(final_.equal_content(&w.leer()), Ok(()));
    }
}

/// Lo que debe haber en un fichero escrito.
enum Expected {
    Aseprite(Stack),
    Png(px_core::Image),
}

#[derive(Clone, Debug)]
enum B {
    Edit(u8),
    SaveAs(u8, bool),
    Export(u8),
    Direct,
    TestCopy,
}

fn order_b() -> impl Strategy<Value = B> {
    prop_oneof![
        3 => any::<u8>().prop_map(B::Edit),
        3 => (0u8..3, any::<bool>()).prop_map(|(n, a)| B::SaveAs(n, a)),
        2 => (0u8..3).prop_map(B::Export),
        1 => Just(B::Direct),
        1 => Just(B::TestCopy),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// P3. Solo se escribe lo que se pide: con secuencias aleatorias de guardar, guardar como,
    /// exportar y guardar una copia, el directorio de trabajo contiene exactamente los ficheros de
    /// los `Saved` devueltos (sin `.pixoko-tmp`), cada uno con el contenido esperado en su último
    /// guardado, y `golden/aseprite/` y `golden/png/` quedan iguales.
    #[test]
    fn property_3_only_what_is_asked_is_written(
        indexed_2 in any::<bool>(),
        commands in vec(order_b(), 1..=14),
    ) {
        let t = Work3::new_2();
        let mut w = world(indexed_2, 6, 5);
        let mut expected_2: BTreeMap<PathBuf, Expected> = BTreeMap::new();
        let save = |w: &mut World3, expected_2: &mut BTreeMap<PathBuf, Expected>, path_2: PathBuf| {
            let aseprite = path_2.extension().is_some_and(|e| e == "aseprite");
            let content = if aseprite {
                Expected::Aseprite(w.m.canonical())
            } else {
                Expected::Png(compose(&w.s.document).expect("compose"))
            };
            expected_2.insert(path_2, content);
        };
        for order in &commands {
            match order {
                B::Edit(n) => {
                    let o = if n % 3 == 0 {
                        Order::New
                    } else {
                        let c = color(&w.m, *n);
                        Order::Fill((i32::from(*n) % 6, i32::from(*n) % 5), c)
                    };
                    w.apply(&o);
                }
                B::SaveAs(n, aseprite) => {
                    let ext = if *aseprite { "aseprite" } else { "png" };
                    let chosen = t.path_2().join(format!("f{n}.{ext}"));
                    outside_golden_aseprite(&chosen);
                    if *aseprite && without_transparent(&w) {
                        let error = w.s.save_to(&chosen).expect_err("sin índice transparente");
                        prop_assert!(error.to_string().contains("no tiene entrada transparente"), "{error}");
                        continue;
                    }
                    let saved = w.s.save_to(&chosen).expect("guardar");
                    prop_assert_eq!(&saved.path, &chosen);
                    w.mark_saved();
                    save(&mut w, &mut expected_2, saved.path);
                }
                B::Export(n) => {
                    let chosen = t.path_2().join(format!("e{n}.png"));
                    outside_golden_aseprite(&chosen);
                    let saved = w.s.export_flat(&chosen).expect("exportar");
                    save(&mut w, &mut expected_2, saved.path);
                }
                B::Direct => {
                    if w.s.path().is_some() && w.s.save_route() == crate::dialogs::SaveRoute::Direct {
                        let saved = w.s.save(t.path_2()).expect("guardar");
                        w.mark_saved();
                        save(&mut w, &mut expected_2, saved.path);
                    }
                }
                B::TestCopy => {
                    if w.s.path().is_some() {
                        let saved = w.s.save_as(t.path_2()).expect("guardar copia");
                        w.mark_saved();
                        save(&mut w, &mut expected_2, saved.path);
                    }
                }
            }
        }
        let names = t.names();
        without_temporary(&names);
        let mut requested: Vec<String> = expected_2
            .keys()
            .map(|r| r.file_name().expect("nombre").to_string_lossy().into_owned())
            .collect();
        requested.sort();
        prop_assert_eq!(&names, &requested, "el directorio tiene lo que se pidió y nada más");
        for (path_2, content) in &expected_2 {
            match content {
                Expected::Aseprite(stack) => {
                    let mut other = session_empty();
                    other.open(path_2, Canvas::MAX_SIDE, true).expect("reabrir");
                    let layers_2 = other.document.layers().len();
                    let read = Stack::of_document(&other.document, layers_2 - 1);
                    let r = stack.equal_content(&read.canonical());
                    prop_assert!(r.is_ok(), "{}: {:?}", path_2.display(), r);
                }
                Expected::Png(image_2) => {
                    let read_2 = read_png(&fs::read(path_2).expect("leer")).expect("PNG").image;
                    let r = equal_except_rgb_with_alpha_0(&read_2, image_2);
                    prop_assert!(r.is_ok(), "{}: {:?}", path_2.display(), r);
                }
            }
        }
        prop_assert_eq!(check_golden_aseprite(), Ok(()));
        golden_intact();
    }
}
