//! Tests de `stroke::Stroke`, la lógica de trazo pura (specs/hito-2-lapiz-deshacer.md,
//! secciones 1 y 2; filas 1 a 10 de «Casos límite» y propiedades 1 a 4).
//!
//! Las filas son listas literales calculadas a mano. Los oráculos de las propiedades se escriben
//! aparte con `px_raster::Line` (la geometría) y un `HashMap` (la regla «la última escritura
//! gana»); no comparten código con `Stroke`.
//!
//! Los lienzos y las imágenes miden como mucho 16 x 16, y las posiciones son `i32` arbitrarios:
//! `Stroke` recorta antes de recorrer, así que ninguna prueba recorre un segmento grande.

use px_core::ImageId;
use std::collections::{BTreeSet, HashMap, VecDeque};

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, Mode, Palette, Pixel, PixelEdit, Rgba,
};
use px_raster::Line;
use px_ui::stroke::Stroke;

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;

type Point = (i32, i32);

// Apoyo común.

fn pixel_a() -> Pixel {
    Pixel::Rgba(Rgba::opaque(200, 40, 40))
}

fn pixel_b() -> Pixel {
    Pixel::Rgba(Rgba::opaque(40, 40, 200))
}

/// Un píxel RGBA distinto para cada `k` (hasta 256).
fn pixel_k(k: usize) -> Pixel {
    Pixel::Rgba(Rgba::new(k as u8, (k * 7) as u8, 3, 255))
}

fn editing(x: i32, y: i32, pixel: Pixel) -> PixelEdit {
    PixelEdit {
        x: u32::try_from(x).expect("dentro del lienzo"),
        y: u32::try_from(y).expect("dentro del lienzo"),
        pixel,
    }
}

/// Las ediciones esperadas de una lista de coordenadas con un mismo píxel.
fn edits(coords: &[Point], pixel: Pixel) -> Vec<PixelEdit> {
    coords.iter().map(|&(x, y)| editing(x, y, pixel)).collect()
}

/// Crea un trazo en un lienzo de `width` x `height` y le da las posiciones, cada una con su píxel.
fn stroke_2(width: u32, height: u32, positions: &[(Point, Pixel)]) -> Stroke {
    let mut stroke = Stroke::new(width, height);
    for &(pos, pixel) in positions {
        stroke.move_to(pos, pixel);
    }
    stroke
}

/// Como `stroke_2`, con el mismo píxel en todas las posiciones.
fn stroke_with(width: u32, height: u32, positions: &[Point], pixel: Pixel) -> Stroke {
    let mut stroke = Stroke::new(width, height);
    for &pos in positions {
        stroke.move_to(pos, pixel);
    }
    stroke
}

fn coordinates(stroke: &Stroke) -> Vec<Point> {
    stroke
        .pixels()
        .iter()
        .map(|e| (e.x as i32, e.y as i32))
        .collect()
}

fn doc_rgba(width: u32, height: u32) -> Document {
    Document::new(Image::new(width, height, Mode::Rgba).unwrap())
}

fn image_of(doc: &Document) -> Image {
    doc.image(ImageId::from_raw(0)).unwrap().clone()
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
fn snapshot(doc: &Document) -> (Document, bool, bool, usize) {
    (
        doc.clone(),
        doc.can_undo(),
        doc.can_redo(),
        doc.history_bytes(),
    )
}

fn inside(p: Point, width: u32, height: u32) -> bool {
    (0..i64::from(width)).contains(&i64::from(p.0))
        && (0..i64::from(height)).contains(&i64::from(p.1))
}

// Filas 1 a 7: la lista de píxeles.

#[test]
fn row_1_a_position_paints_a_pixel() {
    let mut stroke = Stroke::new(8, 8);
    assert!(stroke.is_empty());
    assert_eq!(stroke.last(), None);
    stroke.move_to((2, 3), pixel_a());
    assert_eq!(stroke.pixels(), [editing(2, 3, pixel_a())]);
    assert!(!stroke.is_empty());
    assert_eq!(stroke.last(), Some((2, 3)));
}

#[test]
fn row_2_the_repeated_one_is_ignored_and_the_jump_leaves_no_gaps() {
    let stroke = stroke_with(8, 8, &[(1, 1), (1, 1), (4, 1)], pixel_a());
    assert_eq!(
        stroke.pixels(),
        edits(&[(1, 1), (2, 1), (3, 1), (4, 1)], pixel_a())
    );
    assert_eq!(stroke.last(), Some((4, 1)));
}

#[test]
fn row_3_a_crossing_visits_twice_and_leaves_once() {
    let stroke = stroke_with(8, 8, &[(0, 2), (4, 2), (2, 0), (2, 4)], pixel_a());
    let expected_2 = [
        (0, 2),
        (1, 2),
        (2, 2),
        (3, 2),
        (4, 2),
        (3, 1),
        (2, 0),
        (2, 1),
        (2, 3),
        (2, 4),
    ];
    assert_eq!(stroke.pixels().len(), 10);
    assert_eq!(coordinates(&stroke), expected_2);
}

#[test]
fn row_4_the_last_write_wins_and_the_segment_uses_the_new_pixel() {
    // `(1,1)` con A, `(3,1)` con A y otra vez `(1,1)` con B: el segmento de vuelta pinta los tres
    // con B, extremo ya pintado incluido.
    let ret = stroke_2(
        8,
        8,
        &[
            ((1, 1), pixel_a()),
            ((3, 1), pixel_a()),
            ((1, 1), pixel_b()),
        ],
    );
    assert_eq!(ret.pixels(), edits(&[(1, 1), (2, 1), (3, 1)], pixel_b()));
    // `(1,1)` con A y `(3,1)` con B: el extremo `(1,1)`, que estaba con A, pasa a B.
    let outward = stroke_2(8, 8, &[((1, 1), pixel_a()), ((3, 1), pixel_b())]);
    assert_eq!(
        outward.pixels(),
        edits(&[(1, 1), (2, 1), (3, 1)], pixel_b())
    );
}

#[test]
fn a_position_equal_to_the_previous_one_does_nothing_even_if_the_pixel_changes() {
    let mut stroke = Stroke::new(8, 8);
    stroke.move_to((2, 2), pixel_a());
    stroke.move_to((2, 2), pixel_b());
    assert_eq!(stroke.pixels(), [editing(2, 2, pixel_a())]);
}

#[test]
fn row_5_everything_outside_leaves_the_stroke_empty_but_remembers_the_last_position() {
    let stroke = stroke_with(8, 8, &[(-5, 2), (-1, 6), (-3, 0)], pixel_a());
    assert!(stroke.is_empty());
    assert!(stroke.pixels().is_empty());
    assert_eq!(stroke.last(), Some((-3, 0)));

    let mut doc = doc_rgba(8, 8);
    let id = ImageId::from_raw(0);
    let antes = snapshot(&doc);
    assert_eq!(stroke.commit(&mut doc, id), Ok(false));
    assert_eq!(snapshot(&doc), antes);
}

#[test]
fn row_6_the_round_trip_span_on_the_outside_is_not_closed_on_the_inside() {
    // (6,2)-(20,2) dentro da (6,2),(7,2); (20,2)-(6,4) tiene 15 píxeles (`i` de 0 a 14) y solo
    // los dos últimos caen dentro: (7,4) y (6,4). No hay línea entre (7,2) y (7,4).
    let stroke = stroke_with(8, 8, &[(6, 2), (20, 2), (6, 4)], pixel_a());
    assert_eq!(
        coordinates(&stroke),
        [(6, 2), (7, 2), (7, 4), (6, 4)],
        "calculado a mano"
    );
    // El mismo resultado con el oráculo de `Line`, que no depende de lo anterior.
    let mut oracle: Vec<Point> = vec![(6, 2)];
    for (a, b) in [((6, 2), (20, 2)), ((20, 2), (6, 4))] {
        for p in Line::new(a, b).clip_to(8, 8) {
            if !oracle.contains(&p) {
                oracle.push(p);
            }
        }
    }
    assert_eq!(coordinates(&stroke), oracle);
    assert_eq!(stroke.last(), Some((6, 4)));
}

#[test]
fn row_7_the_i32_extremes_do_not_panic() {
    let diagonal = stroke_with(8, 8, &[(MIN, MIN), (MAX, MAX)], pixel_a());
    let expected_2: Vec<Point> = (0..8).map(|k| (k, k)).collect();
    assert_eq!(coordinates(&diagonal), expected_2);
    assert_eq!(diagonal.last(), Some((MAX, MAX)));

    let one = stroke_with(1, 1, &[(0, 0), (5, 5)], pixel_a());
    assert_eq!(one.pixels(), [editing(0, 0, pixel_a())]);
    assert_eq!(one.last(), Some((5, 5)));
}

#[test]
fn all_the_i32_corners_among_themselves_do_not_panic() {
    let values = [MIN, MIN + 1, -1, 0, 1, 7, 8, MAX - 1, MAX];
    for ax in values {
        for ay in values {
            for bx in values {
                for by in values {
                    let stroke = stroke_with(8, 8, &[(ax, ay), (bx, by)], pixel_a());
                    for (x, y) in coordinates(&stroke) {
                        assert!(inside((x, y), 8, 8), "({x}, {y}) fuera de 8x8");
                    }
                    assert_eq!(stroke.last(), Some((bx, by)));
                }
            }
        }
    }
}

// Filas 8 a 10: `commit` contra el documento.

#[test]
fn row_8_a_stroke_that_changes_is_a_history_step() {
    let mut doc = doc_rgba(8, 8);
    let id = ImageId::from_raw(0);
    let original = image_of(&doc);
    let mut edited = original.clone();
    for x in 1..=3 {
        edited.set_pixel(x, 1, pixel_a()).unwrap();
    }

    let stroke = stroke_with(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(stroke.pixels().len(), 3);
    assert_eq!(stroke.commit(&mut doc, id), Ok(true));
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(image_of(&doc), edited);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), original, "un undo vuelve al original");
    assert!(!doc.can_undo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc), edited);
}

#[test]
fn row_8_with_two_strokes_one_undo_undoes_only_the_last() {
    let mut doc = doc_rgba(8, 8);
    let id = ImageId::from_raw(0);
    let original = image_of(&doc);

    let first = stroke_with(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(first.commit(&mut doc, id), Ok(true));
    let after_first = image_of(&doc);
    let second = stroke_with(8, 8, &[(1, 3), (3, 3)], pixel_b());
    assert_eq!(second.commit(&mut doc, id), Ok(true));
    assert_ne!(image_of(&doc), after_first);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), after_first);
    assert!(doc.can_undo());
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), original);
    assert!(!doc.can_undo());
}

#[test]
fn row_9_strokes_that_change_nothing_do_not_touch_the_history() {
    let mut doc = doc_rgba(8, 8);
    let id = ImageId::from_raw(0);
    let a = stroke_with(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(a.commit(&mut doc, id), Ok(true));
    let b = stroke_with(8, 8, &[(1, 3), (3, 3)], pixel_b());
    assert_eq!(b.commit(&mut doc, id), Ok(true));
    // Un `undo` previo: queda algo que rehacer, y no debe perderse.
    assert_eq!(doc.undo(), Ok(()));
    let antes = snapshot(&doc);
    assert!(antes.1 && antes.2, "hay algo que deshacer y que rehacer");

    // Vacío: no llama a `apply`, que daría `EmptyCommand`.
    assert_eq!(Stroke::new(8, 8).commit(&mut doc, id), Ok(false));
    assert_eq!(snapshot(&doc), antes);
    // Todo fuera del lienzo.
    let all_outside = stroke_with(8, 8, &[(-4, 2), (-1, 6)], pixel_a());
    assert!(all_outside.is_empty());
    assert_eq!(all_outside.commit(&mut doc, id), Ok(false));
    assert_eq!(snapshot(&doc), antes);
    // Con el color que ya hay (el primer trazo dejó A en la fila 1).
    let equal = stroke_with(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(equal.pixels().len(), 3);
    assert_eq!(equal.commit(&mut doc, id), Ok(false));
    assert_eq!(snapshot(&doc), antes);
    assert!(doc.can_redo(), "el redo pendiente sigue ahí");
}

#[test]
fn row_10_a_pixel_of_another_mode_gives_an_error_and_does_not_touch_the_document() {
    let mut doc = doc_rgba(8, 8);
    let id = ImageId::from_raw(0);
    let antes = snapshot(&doc);
    let stroke = stroke_with(8, 8, &[(1, 1), (3, 1)], Pixel::Indexed(0));
    assert_eq!(
        stroke.commit(&mut doc, id),
        Err(DocumentError::Pixel {
            pixel: 0,
            source: ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Indexed
            }
        })
    );
    assert_eq!(snapshot(&doc), antes);
}

#[test]
fn commit_applies_a_single_set_pixels_with_the_pixels_of_the_stroke() {
    // Lo mismo que `Document::apply(SetPixels { .. })` con la lista del trazo.
    let stroke = stroke_with(8, 8, &[(0, 2), (4, 2), (2, 0)], pixel_a());
    let pixels = stroke.pixels().to_vec();
    let mut with_stroke = doc_rgba(8, 8);
    let id = ImageId::from_raw(0);
    let mut by_hand = doc_rgba(8, 8);
    assert_eq!(
        by_hand.apply(Command::SetPixels { image: id, pixels }),
        Ok(true)
    );
    assert_eq!(stroke.commit(&mut with_stroke, id), Ok(true));
    assert_eq!(snapshot(&with_stroke), snapshot(&by_hand));
}

// Estrategias.

/// Coordenada `i32`: sesgo hacia dentro de un lienzo de hasta 16 y sus bordes, con los extremos.
fn coordinate() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => -3i32..=18,
        3 => 0i32..=15,
        1 => Just(0i32),
        1 => Just(15i32),
        1 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
    ]
}

fn positions(max: usize) -> impl Strategy<Value = Vec<Point>> {
    vec((coordinate(), coordinate()), 1..=max)
}

/// Si `inside`, lleva todas las posiciones al interior del lienzo; si no, las deja como están.
fn fit_2(pos: &[Point], width: u32, height: u32, inside: bool) -> Vec<Point> {
    pos.iter()
        .map(|&(x, y)| {
            if inside {
                (x.rem_euclid(width as i32), y.rem_euclid(height as i32))
            } else {
                (x, y)
            }
        })
        .collect()
}

// Oráculos.

/// Conjunto esperado: el primer punto y, para cada par consecutivo distinto, los puntos de
/// `Line` recortados al lienzo.
fn oracle_set(pos: &[Point], width: u32, height: u32) -> BTreeSet<Point> {
    let mut set = BTreeSet::new();
    let mut ultimo: Option<Point> = None;
    for &p in pos {
        match ultimo {
            None => {
                if inside(p, width, height) {
                    set.insert(p);
                }
            }
            Some(anterior) if anterior == p => {}
            Some(anterior) => set.extend(Line::new(anterior, p).clip_to(width, height)),
        }
        ultimo = Some(p);
    }
    set
}

/// Lista esperada con la regla «la última escritura gana» y el orden de primera visita.
/// `pixels_2[i]` es el píxel de la llamada `i`.
fn oracle_values(pos: &[Point], pixels_2: &[Pixel], width: u32, height: u32) -> Vec<PixelEdit> {
    let mut order: Vec<Point> = Vec::new();
    let mut values: HashMap<Point, Pixel> = HashMap::new();
    let mut ultimo: Option<Point> = None;
    for (&p, &pixel) in pos.iter().zip(pixels_2) {
        if ultimo == Some(p) {
            continue;
        }
        let points: Vec<Point> = match ultimo {
            None if inside(p, width, height) => vec![p],
            None => vec![],
            Some(anterior) => Line::new(anterior, p).clip_to(width, height).collect(),
        };
        for q in points {
            if values.insert(q, pixel).is_none() {
                order.push(q);
            }
        }
        ultimo = Some(p);
    }
    order
        .into_iter()
        .map(|q| editing(q.0, q.1, values[&q]))
        .collect()
}

/// ¿Es el conjunto 8-conexo? (vacío cuenta como conexo).
fn is_8_connected(set: &BTreeSet<Point>) -> bool {
    let Some(&start_2) = set.iter().next() else {
        return true;
    };
    let mut seen: BTreeSet<Point> = BTreeSet::from([start_2]);
    let mut queue: VecDeque<Point> = VecDeque::from([start_2]);
    while let Some((x, y)) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let neighbor = (x + dx, y + dy);
                if set.contains(&neighbor) && seen.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
    }
    seen.len() == set.len()
}

// Propiedad 1: oráculo geométrico y conexión.

proptest! {
    #[test]
    fn the_pixels_are_the_union_of_the_clipped_lines(
        width in 1u32..=16,
        height in 1u32..=16,
        pos in positions(8),
        all_inside in any::<bool>(),
    ) {
        let pos = fit_2(&pos, width, height, all_inside);
        let stroke = stroke_with(width, height, &pos, pixel_a());
        let obtained = coordinates(&stroke);

        // Todos dentro y sin repetidos.
        for &p in &obtained {
            prop_assert!(inside(p, width, height), "{:?} fuera de {}x{}", p, width, height);
        }
        let set: BTreeSet<Point> = obtained.iter().copied().collect();
        prop_assert_eq!(set.len(), obtained.len(), "coordenadas repetidas");

        // Exactamente la unión de los segmentos recortados, y nada más.
        prop_assert_eq!(&set, &oracle_set(&pos, width, height));
        prop_assert_eq!(stroke.is_empty(), set.is_empty());
        prop_assert_eq!(stroke.last(), pos.last().copied());

        // Todas las posiciones dentro: sin huecos.
        if all_inside {
            prop_assert!(is_8_connected(&set), "hueco en {:?}", set);
        }
    }
}

// Propiedad 2: valores y repeticiones.

proptest! {
    #[test]
    fn the_last_write_wins_and_the_order_is_that_of_the_first_visit(
        width in 1u32..=16,
        height in 1u32..=16,
        pos in positions(8),
        all_inside in any::<bool>(),
    ) {
        let pos = fit_2(&pos, width, height, all_inside);
        let pixels_2: Vec<Pixel> = (0..pos.len()).map(pixel_k).collect();
        let calls: Vec<(Point, Pixel)> = pos.iter().copied().zip(pixels_2.iter().copied()).collect();
        let stroke = stroke_2(width, height, &calls);
        prop_assert_eq!(
            stroke.pixels().to_vec(),
            oracle_values(&pos, &pixels_2, width, height)
        );
    }

    #[test]
    fn repeating_a_position_in_a_row_does_not_change_the_result(
        width in 1u32..=16,
        height in 1u32..=16,
        pos in positions(8),
        repetitions in vec(0usize..=3, 8),
        all_inside in any::<bool>(),
    ) {
        let pos = fit_2(&pos, width, height, all_inside);
        let base: Vec<(Point, Pixel)> = pos
            .iter()
            .enumerate()
            .map(|(i, &p)| (p, pixel_k(i)))
            .collect();
        let without_repeat = stroke_2(width, height, &base);

        // Cada posición se repite 0 a 3 veces seguidas, con otro píxel cada vez: se ignoran.
        let mut repeated: Vec<(Point, Pixel)> = Vec::new();
        for (i, &(p, pixel)) in base.iter().enumerate() {
            repeated.push((p, pixel));
            for j in 0..repetitions[i] {
                repeated.push((p, pixel_k(100 + 3 * i + j)));
            }
        }
        let with_repetitions = stroke_2(width, height, &repeated);
        prop_assert_eq!(without_repeat.pixels(), with_repetitions.pixels());
        prop_assert_eq!(without_repeat.last(), with_repetitions.last());
    }
}

// Imágenes de los cuatro modos para las propiedades 3 y 4.

const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Hasta cuatro valores distintos por modo (`v` de 0 a 3), que caben en la paleta de 4 colores.
fn valor(mode_2: Mode, v: u8) -> Pixel {
    match mode_2 {
        Mode::Rgba => Pixel::Rgba(Rgba::new(v * 60, 10, v * 3, 255)),
        Mode::Indexed => Pixel::Indexed(v),
        Mode::Gray => Pixel::Gray(v * 70),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: 5,
            alpha: 255 - v * 60,
        },
    }
}

fn image_empty(mode_2: Mode, width: u32, height: u32) -> Image {
    match mode_2 {
        Mode::Indexed => {
            let palette = Palette::from_colors((0..4).map(|i| Rgba::opaque(i * 50, 0, 0))).unwrap();
            Image::new_indexed(width, height, palette).unwrap()
        }
        other_2 => Image::new(width, height, other_2).unwrap(),
    }
}

/// Imagen con valores 0 a 2 repartidos según `noise`.
fn image_with_noise(mode_2: Mode, width: u32, height: u32, noise: &[u8]) -> Image {
    let mut image = image_empty(mode_2, width, height);
    for y in 0..height {
        for x in 0..width {
            let v = noise[(y * width + x) as usize % noise.len()] % 3;
            image.set_pixel(x, y, valor(mode_2, v)).unwrap();
        }
    }
    image
}

/// Imagen toda del valor `v`.
fn image_uniform(mode_2: Mode, width: u32, height: u32, v: u8) -> Image {
    image_with_noise(mode_2, width, height, &[v])
}

/// `SetPixels` que pone toda la imagen al valor `v`.
fn fill(doc: &mut Document, mode_2: Mode, v: u8) {
    let id = ImageId::from_raw(0);
    let (width, height) = {
        let image = doc.image(id).unwrap();
        (image.width(), image.height())
    };
    let pixels: Vec<PixelEdit> = (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| PixelEdit {
                x,
                y,
                pixel: valor(mode_2, v),
            })
        })
        .collect();
    assert_eq!(
        doc.apply(Command::SetPixels { image: id, pixels }),
        Ok(true)
    );
}

/// Un trazo con una lista de posiciones y un valor, aplicado con `commit` y comprobado contra
/// la imagen antes y después. Devuelve el resultado de `commit`.
fn check_a_step(
    doc: &mut Document,
    mode_2: Mode,
    pos: &[Point],
    v: u8,
) -> Result<bool, TestCaseError> {
    let id = ImageId::from_raw(0);
    let image = image_of(doc);
    let pixel = valor(mode_2, v);
    let stroke = stroke_with(image.width(), image.height(), pos, pixel);
    let changes = stroke
        .pixels()
        .iter()
        .any(|e| image.get_pixel(e.x, e.y).unwrap() != pixel);
    let mut edited = image.clone();
    for e in stroke.pixels() {
        edited.set_pixel(e.x, e.y, e.pixel).unwrap();
    }

    let antes = snapshot(doc);
    let result = stroke.commit(doc, id);
    prop_assert_eq!(result, Ok(changes));
    if changes {
        prop_assert!(doc.can_undo());
        prop_assert!(!doc.can_redo());
        prop_assert_eq!(image_of(doc), edited.clone());
        // Un solo paso: un undo vuelve exactamente a la imagen previa y un redo a la editada.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(image_of(doc), image);
        prop_assert!(doc.can_redo());
        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(image_of(doc), edited);
    } else {
        prop_assert_eq!(snapshot(doc), antes);
    }
    Ok(changes)
}

// Propiedad 3: un trazo, un paso.

proptest! {
    #[test]
    fn a_stroke_that_changes_is_exactly_one_step(
        mode_2 in 0usize..4,
        width in 1u32..=16,
        height in 1u32..=16,
        noise in vec(0u8..3, 256),
        pos in positions(8),
        v in 0u8..3,
        all_inside in any::<bool>(),
    ) {
        let mode_2 = TEST_MODES[mode_2];
        let mut doc = Document::new(image_with_noise(mode_2, width, height, &noise));
        let pos = fit_2(&pos, width, height, all_inside);
        check_a_step(&mut doc, mode_2, &pos, v)?;
    }

    #[test]
    fn k_strokes_are_undone_and_redone_one_by_one(
        mode_2 in 0usize..4,
        width in 1u32..=16,
        height in 1u32..=16,
        noise in vec(0u8..3, 256),
        strokes in vec((positions(6), 0u8..3, any::<bool>()), 1..=4),
    ) {
        let mode_2 = TEST_MODES[mode_2];
        let mut doc = Document::new(image_with_noise(mode_2, width, height, &noise));
        let original = image_of(&doc);

        let mut steps_2 = 0usize;
        for (pos, v, all_inside) in &strokes {
            let pos = fit_2(pos, width, height, *all_inside);
            if check_a_step(&mut doc, mode_2, &pos, *v)? {
                steps_2 += 1;
            }
        }
        let final_ = image_of(&doc);

        for _ in 0..steps_2 {
            prop_assert_eq!(doc.undo(), Ok(()));
        }
        prop_assert_eq!(image_of(&doc), original);
        prop_assert!(!doc.can_undo());
        for _ in 0..steps_2 {
            prop_assert_eq!(doc.redo(), Ok(()));
        }
        prop_assert_eq!(image_of(&doc), final_);
        prop_assert!(!doc.can_redo());

        // Un trazo nuevo tras un undo vacía lo rehacible. El valor 3 no está en la imagen, así
        // que el trazo siempre cambia algo.
        if steps_2 >= 1 {
            prop_assert_eq!(doc.undo(), Ok(()));
            prop_assert!(doc.can_redo());
            let new_2 = stroke_with(width, height, &[(0, 0)], valor(mode_2, 3));
            let id = ImageId::from_raw(0);
            prop_assert_eq!(new_2.commit(&mut doc, id), Ok(true));
            prop_assert!(!doc.can_redo());
        }
    }
}

// Propiedad 4: no-ops.

proptest! {
    #[test]
    fn strokes_that_change_nothing_do_not_touch_the_document(
        mode_2 in 0usize..4,
        width in 1u32..=16,
        height in 1u32..=16,
        preview in 0u8..3,
        kind in 0u8..3,
        pos in positions(8),
        outside_on_the_right in any::<bool>(),
    ) {
        let mode_2 = TEST_MODES[mode_2];
        let mut doc = Document::new(image_uniform(mode_2, width, height, 0));
        let id = ImageId::from_raw(0);
        // Historia previa: ninguna, un paso (la imagen entera pasa a valer 1), o ese paso
        // deshecho (con algo que rehacer).
        let mut actual = 0;
        if preview >= 1 {
            fill(&mut doc, mode_2, 1);
            actual = 1;
        }
        if preview == 2 {
            prop_assert_eq!(doc.undo(), Ok(()));
            actual = 0;
        }
        let antes = snapshot(&doc);

        let stroke = match kind {
            // Vacío.
            0 => Stroke::new(width, height),
            // Todo fuera: x < 0 o x >= width, con cualquier y.
            1 => {
                let outside: Vec<Point> = pos
                    .iter()
                    .map(|&(x, y)| {
                        let x = if x < 0 { x } else { -1 - x.min(MAX - 1) };
                        let x = if outside_on_the_right {
                            (width as i32 - 1).saturating_sub(x)
                        } else {
                            x
                        };
                        (x, y)
                    })
                    .collect();
                stroke_with(width, height, &outside, valor(mode_2, 2))
            }
            // Con el valor que ya tiene toda la imagen.
            _ => stroke_with(width, height, &pos, valor(mode_2, actual)),
        };
        if kind == 1 {
            prop_assert!(stroke.is_empty());
        }
        prop_assert_eq!(stroke.commit(&mut doc, id), Ok(false));
        prop_assert_eq!(snapshot(&doc), antes);
    }
}
