//! Tests de la línea determinista (specs/hito-2-linea.md).
//!
//! Ninguna prueba recorre ni reserva un segmento grande: los segmentos de hasta 2^32 píxeles
//! solo se consultan con `len`, `nth`, `next`, `is_empty` y `clip_to`, y todo lo que se
//! recoge en un `Vec` mide como mucho 41 píxeles (coordenadas de -20 a 20) o 16 (lienzos de
//! hasta 16 x 16). Los lienzos y las imágenes miden como mucho 16 x 16.
//!
//! El modelo exacto de la definición (comportamiento 2) está en `Model`: se escribe aparte,
//! solo con enteros (`i128`) y sin copiar ningún algoritmo de línea. Es lo único que
//! comparten las propiedades; las cuatro filas «a mano» de la tabla son listas literales.

// La spec pide probar `nth` (sobrescrito en `Line`) también con índice 0, no solo `next`.
#![allow(clippy::iter_nth_zero)]

use std::collections::BTreeSet;
use std::fmt::Debug;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba,
};
use px_raster::{Line, draw_line, line_command};

// Comprobación en tiempo de compilación de los traits exigidos.
fn assert_line_traits<T: Clone + Debug + Iterator<Item = (i32, i32)>>() {}

#[test]
fn traits_derived() {
    assert_line_traits::<Line>();
}

// Apoyo común.

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
/// Píxeles de `i32::MIN` a `i32::MAX`: no caben en `u32`.
const TWO_32: u64 = 1 << 32;

type Point = (i32, i32);
type Par = (Point, Point);

/// Resultado esperado de un recorte vacío.
const EMPTY: [Point; 0] = [];

/// Índice de `nth`; todos los que usan los tests son como mucho `2^32 - 1`.
fn idx(i: u64) -> usize {
    usize::try_from(i).expect("el índice cabe en usize")
}

fn pixels(a: Point, b: Point) -> Vec<Point> {
    Line::new(a, b).collect()
}

fn clipped(a: Point, b: Point, width: u32, height: u32) -> Vec<Point> {
    Line::new(a, b).clip_to(width, height).collect()
}

fn reversed(mut v: Vec<Point>) -> Vec<Point> {
    v.reverse();
    v
}

// Modelo exacto de la definición (comportamiento 2), independiente de la implementación.

/// Datos de un segmento en `i128`: `dx`, `dy` y `n = max(|dx|, |dy|)` sin desbordes.
#[derive(Clone, Copy, Debug)]
struct Model {
    ax: i128,
    ay: i128,
    dx: i128,
    dy: i128,
    n: i128,
}

/// División entera hacia -infinito; `den` es siempre positivo.
fn floor(num: i128, den: i128) -> i128 {
    assert!(den > 0);
    let q = num / den;
    if num % den != 0 && num < 0 { q - 1 } else { q }
}

impl Model {
    fn new(a: Point, b: Point) -> Model {
        let (ax, ay) = (i128::from(a.0), i128::from(a.1));
        let dx = i128::from(b.0) - ax;
        let dy = i128::from(b.1) - ay;
        Model {
            ax,
            ay,
            dx,
            dy,
            n: dx.abs().max(dy.abs()),
        }
    }

    /// `|dx| >= |dy|`: el eje mayor es x (también con `n == 0`).
    fn axis_x(&self) -> bool {
        self.dx.abs() >= self.dy.abs()
    }

    /// `n` como índice del último píxel.
    fn n(&self) -> u64 {
        u64::try_from(self.n).expect("n no es negativo")
    }

    /// Número de píxeles: `n + 1`.
    fn len(&self) -> u64 {
        u64::try_from(self.n + 1).expect("n + 1 es positivo")
    }

    /// Píxel `i`, con `0 <= i <= n`.
    fn pixel(&self, i: u64) -> Point {
        let i = i128::from(i);
        assert!(i <= self.n, "índice {i} fuera de 0..={}", self.n);
        let (x, y) = if self.n == 0 {
            (self.ax, self.ay)
        } else if self.axis_x() {
            (
                self.ax + self.dx.signum() * i,
                self.ay + floor(2 * self.dy * i + self.n, 2 * self.n),
            )
        } else {
            (
                self.ax + floor(2 * self.dx * i + self.n, 2 * self.n),
                self.ay + self.dy.signum() * i,
            )
        };
        (
            i32::try_from(x).expect("x cabe en i32"),
            i32::try_from(y).expect("y cabe en i32"),
        )
    }

    /// Todos los píxeles; solo para segmentos pequeños.
    fn all(&self) -> Vec<Point> {
        assert!(self.n <= 1000, "segmento demasiado largo para listarlo");
        (0..self.len()).map(|i| self.pixel(i)).collect()
    }
}

/// Dentro del lienzo `0..width x 0..height`.
fn inside(p: Point, width: u32, height: u32) -> bool {
    (0..i64::from(width)).contains(&i64::from(p.0))
        && (0..i64::from(height)).contains(&i64::from(p.1))
}

/// Píxeles del modelo que caen en el lienzo, en orden, sin recorrer el segmento: los índices
/// candidatos salen de las `width` columnas (eje mayor x) o las `height` filas (eje mayor y).
fn model_clipped(a: Point, b: Point, width: u32, height: u32) -> Vec<Point> {
    let m = Model::new(a, b);
    if width == 0 || height == 0 {
        return Vec::new();
    }
    if m.n == 0 {
        return if inside(a, width, height) {
            vec![a]
        } else {
            Vec::new()
        };
    }
    // En el eje mayor la coordenada es `source + sign * i`, así que `i = sign * (c - source)`.
    let (source, sign, side) = if m.axis_x() {
        (m.ax, m.dx.signum(), width)
    } else {
        (m.ay, m.dy.signum(), height)
    };
    let mut indices: Vec<i128> = (0..side)
        .map(|c| sign * (i128::from(c) - source))
        .filter(|i| (0..=m.n).contains(i))
        .collect();
    indices.sort_unstable();
    indices
        .into_iter()
        .map(|i| m.pixel(u64::try_from(i).expect("i no es negativo")))
        .filter(|&p| inside(p, width, height))
        .collect()
}

// Imágenes, documentos y comandos de apoyo.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn doc_rgba(width: u32, height: u32) -> Document {
    Document::new(Image::new(width, height, Mode::Rgba).unwrap())
}

fn pixel_at(doc: &Document, x: u32, y: u32) -> Pixel {
    doc.image(ImageId::from_raw(0))
        .unwrap()
        .get_pixel(x, y)
        .unwrap()
}

fn editing(x: u32, y: u32, pixel: Pixel) -> PixelEdit {
    PixelEdit { x, y, pixel }
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

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_a_single_point() {
    assert_eq!(pixels((0, 0), (0, 0)), [(0, 0)]);
    let l = Line::new((0, 0), (0, 0));
    assert_eq!(l.len(), 1);
    assert!(!l.is_empty());
}

#[test]
fn row_horizontal_by_hand() {
    assert_eq!(
        pixels((0, 0), (4, 0)),
        [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
    );
}

#[test]
fn row_vertical_by_hand() {
    assert_eq!(pixels((0, 0), (0, 3)), [(0, 0), (0, 1), (0, 2), (0, 3)]);
}

#[test]
fn row_pure_diagonal_by_hand() {
    assert_eq!(pixels((0, 0), (3, 3)), [(0, 0), (1, 1), (2, 2), (3, 3)]);
    assert_eq!(pixels((3, 3), (0, 0)), [(3, 3), (2, 2), (1, 1), (0, 0)]);
    assert_eq!(pixels((0, 0), (3, -3)), [(0, 0), (1, -1), (2, -2), (3, -3)]);
}

#[test]
fn row_tie_on_x_axis_by_hand() {
    // y_real = 0,5 en x = 1: el empate sube a y = 1 (hacia +y).
    assert_eq!(pixels((0, 0), (2, 1)), [(0, 0), (1, 1), (2, 1)]);
    assert_eq!(pixels((2, 1), (0, 0)), [(2, 1), (1, 1), (0, 0)]);
}

#[test]
fn row_tie_with_negative_slope() {
    assert_eq!(pixels((0, 1), (2, 0)), [(0, 1), (1, 1), (2, 0)]);
}

#[test]
fn row_slope_one_half() {
    assert_eq!(
        pixels((0, 0), (4, 2)),
        [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)]
    );
}

#[test]
fn row_tie_on_y_axis_goes_toward_plus_x() {
    assert_eq!(
        pixels((0, 0), (2, 4)),
        [(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)]
    );
}

#[test]
fn row_slope_without_tie() {
    assert_eq!(pixels((0, 0), (3, 1)), [(0, 0), (1, 0), (2, 1), (3, 1)]);
}

#[test]
fn row_floor_with_negatives() {
    assert_eq!(
        pixels((0, 0), (-4, -2)),
        [(0, 0), (-1, 0), (-2, -1), (-3, -1), (-4, -2)]
    );
    assert_eq!(
        pixels((-4, -2), (0, 0)),
        [(-4, -2), (-3, -1), (-2, -1), (-1, 0), (0, 0)]
    );
}

#[test]
fn row_horizontal_from_end_to_end() {
    let l = Line::new((MIN, 0), (MAX, 0));
    assert_eq!(l.len(), TWO_32);
    assert!(!l.is_empty());
    assert_eq!(l.clone().nth(0), Some((MIN, 0)));
    assert_eq!(l.clone().nth(idx(TWO_32 - 1)), Some((MAX, 0)));
    // El punto medio es el 2^31, y justo detrás del último ya no hay nada.
    assert_eq!(l.clone().nth(idx(TWO_32 / 2)), Some((0, 0)));
    // Justo detrás del último ya no hay nada (sin `nth(2^32)`, que no cabe en 32 bits).
    let mut u = l.clone();
    assert_eq!(u.nth(idx(TWO_32 - 1)), Some((MAX, 0)));
    assert_eq!(u.next(), None);
}

#[test]
fn row_diagonal_from_end_to_end() {
    let l = Line::new((MIN, MIN), (MAX, MAX));
    assert_eq!(l.len(), TWO_32);
    assert_eq!(l.clone().nth(0), Some((MIN, MIN)));
    assert_eq!(l.clone().nth(1), Some((MIN + 1, MIN + 1)));
    assert_eq!(l.clone().nth(idx(TWO_32 - 1)), Some((MAX, MAX)));
    assert_eq!(l.clone().nth(idx(TWO_32 / 2)), Some((0, 0)));
}

#[test]
fn row_odd_dx_of_2_32_minus_1() {
    // n = 2^32 - 1 e y sube de 0 a 1 justo a mitad: el salto de fila ocurre en el índice 2^31.
    let l = Line::new((MIN, 0), (MAX, 1));
    assert_eq!(l.len(), TWO_32);
    assert_eq!(l.clone().nth(idx(TWO_32 / 2 - 1)), Some((-1, 0)));
    assert_eq!(l.clone().nth(idx(TWO_32 / 2)), Some((0, 1)));
    assert_eq!(l.clone().nth(0), Some((MIN, 0)));
    assert_eq!(l.clone().nth(idx(TWO_32 - 1)), Some((MAX, 1)));
}

#[test]
fn row_huge_diagonal_clipped_to_4x4() {
    // Instantáneo: no recorre 2^32 píxeles.
    assert_eq!(
        clipped((MIN, MIN), (MAX, MAX), 4, 4),
        [(0, 0), (1, 1), (2, 2), (3, 3)]
    );
}

#[test]
fn row_clip_is_the_prefix_of_the_line() {
    assert_eq!(clipped((0, 0), (4, 2), 3, 2), [(0, 0), (1, 1), (2, 1)]);
}

#[test]
fn row_empty_clip_with_a_box_that_cuts_the_canvas() {
    // La caja envolvente de (-3,2)-(2,-3) corta el lienzo, pero la diagonal no lo toca.
    assert_eq!(clipped((-3, 2), (2, -3), 4, 4), EMPTY);
    assert_eq!(clipped((10, 10), (20, 20), 4, 4), EMPTY);
    let l = Line::new((-3, 2), (2, -3)).clip_to(4, 4);
    assert!(l.is_empty());
    assert_eq!(l.len(), 0);

    let doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(1, 2, 3, 255));
    assert_eq!(line_command(&doc, id, (-3, 2), (2, -3), pixel), Ok(None));
    assert_eq!(line_command(&doc, id, (10, 10), (20, 20), pixel), Ok(None));
}

#[test]
fn row_negative_coordinates_clipped_to_4x4() {
    // Todos los píxeles tienen x < 0 o y < 0.
    assert_eq!(clipped((-5, -5), (-1, -1), 4, 4), EMPTY);
    assert_eq!(clipped((-5, 0), (-1, 0), 4, 4), EMPTY);
    assert_eq!(clipped((0, -9), (0, -1), 4, 4), EMPTY);
}

#[test]
fn row_clip_with_zero_side_and_with_start_outside() {
    assert_eq!(
        clipped((-2, 0), (5, 0), 4, 1),
        [(0, 0), (1, 0), (2, 0), (3, 0)]
    );
    // `width` o `height` cero no admiten ningún píxel.
    assert_eq!(clipped((0, 0), (3, 3), 0, 4), EMPTY);
    assert_eq!(clipped((0, 0), (3, 3), 4, 0), EMPTY);
    assert_eq!(clipped((0, 0), (3, 3), 0, 0), EMPTY);
    assert!(Line::new((0, 0), (3, 3)).clip_to(0, 0).is_empty());
}

#[test]
fn row_line_command_diagonal_rgba() {
    let doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    let expected = Command::SetPixels {
        image: id,
        pixels: vec![
            editing(0, 0, pixel),
            editing(1, 1, pixel),
            editing(2, 2, pixel),
            editing(3, 3, pixel),
        ],
    };
    let before = snapshot(&doc);
    assert_eq!(
        line_command(&doc, id, (0, 0), (3, 3), pixel),
        Ok(Some(expected))
    );
    // Solo lee: el documento queda igual.
    assert_eq!(snapshot(&doc), before);
}

#[test]
fn row_draw_line_completely_outside() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));

    // En un documento nuevo.
    let before = snapshot(&doc);
    assert_eq!(
        draw_line(&mut doc, id, (10, 10), (20, 20), pixel),
        Ok(false)
    );
    assert!(!doc.can_undo());
    assert_eq!(snapshot(&doc), before);
}

#[test]
fn row_draw_line_outside_with_history_and_redo() {
    // Con historia y con algo que rehacer: nada se toca (ni `can_undo`, `can_redo` ni los bytes).
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 0), pixel), Ok(true));
    assert_eq!(draw_line(&mut doc, id, (0, 3), (3, 3), pixel), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    let before = snapshot(&doc);
    assert!(before.1 && before.2);
    for (a, b) in [((-9, -9), (-1, -1)), ((4, 0), (9, 0)), ((0, -3), (3, -3))] {
        assert_eq!(draw_line(&mut doc, id, a, b, pixel), Ok(false));
        assert_eq!(snapshot(&doc), before);
    }
}

#[test]
fn row_draw_line_with_the_color_already_there() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 3), pixel), Ok(true));
    let before = snapshot(&doc);
    // `apply` decide que no cambia nada y no añade paso.
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 3), pixel), Ok(false));
    assert_eq!(snapshot(&doc), before);
}

#[test]
fn row_draw_line_with_unknown_image() {
    let mut doc = doc_rgba(4, 4);
    let unknown = ImageId::from_raw(99);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    let before = snapshot(&doc);
    assert_eq!(
        draw_line(&mut doc, unknown, (0, 0), (3, 3), pixel),
        Err(DocumentError::UnknownImage(unknown))
    );
    assert_eq!(
        line_command(&doc, unknown, (0, 0), (3, 3), pixel),
        Err(DocumentError::UnknownImage(unknown))
    );
    assert_eq!(snapshot(&doc), before);
}

/// Comportamiento 11: la imagen se busca antes de recortar, así que `UnknownImage` gana a `None`
/// incluso con una línea que queda totalmente fuera del lienzo.
#[test]
fn row_unknown_image_beats_line_outside_the_canvas() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let unknown = ImageId::from_raw(99);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    let outside: [Par; 6] = [
        ((10, 10), (20, 20)),
        ((-5, -5), (-1, -1)),
        ((MIN, MIN), (MIN, MIN)),
        ((MIN, 0), (MIN, 5)),
        ((-3, 2), (2, -3)),
        ((MAX, MAX), (MIN, MAX)),
    ];
    let before = snapshot(&doc);
    for (a, b) in outside {
        // Con una imagen que existe, la línea queda fuera: sin comando y sin cambios.
        assert_eq!(
            line_command(&doc, id, a, b, pixel),
            Ok(None),
            "{a:?}->{b:?}"
        );
        assert_eq!(
            draw_line(&mut doc, id, a, b, pixel),
            Ok(false),
            "{a:?}->{b:?}"
        );
        // Con una que no existe, el error de imagen va primero.
        assert_eq!(
            line_command(&doc, unknown, a, b, pixel),
            Err(DocumentError::UnknownImage(unknown)),
            "{a:?}->{b:?}"
        );
        assert_eq!(
            draw_line(&mut doc, unknown, a, b, pixel),
            Err(DocumentError::UnknownImage(unknown)),
            "{a:?}->{b:?}"
        );
        assert_eq!(snapshot(&doc), before);
    }
}

#[test]
fn row_draw_line_with_pixel_of_another_mode() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let before = snapshot(&doc);
    assert_eq!(
        draw_line(&mut doc, id, (0, 0), (3, 3), Pixel::Gray(7)),
        Err(DocumentError::Pixel {
            pixel: 0,
            source: ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray
            }
        })
    );
    // La imagen queda intacta, también el primer píxel de la línea.
    assert_eq!(pixel_at(&doc, 0, 0), Pixel::Rgba(Rgba::TRANSPARENT));
    assert_eq!(snapshot(&doc), before);
}

// Comportamientos que la tabla no lista como fila.

#[test]
fn line_command_does_not_check_the_pixel_mode() {
    // Lo valida `Document` al aplicar el comando (comportamiento 9).
    let doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let expected = Command::SetPixels {
        image: id,
        pixels: vec![editing(0, 0, Pixel::Gray(7)), editing(1, 0, Pixel::Gray(7))],
    };
    assert_eq!(
        line_command(&doc, id, (0, 0), (1, 0), Pixel::Gray(7)),
        Ok(Some(expected))
    );
}

#[test]
fn draw_line_in_the_four_modes() {
    let palette = Palette::from_colors([rgba(0, 0, 0, 255), rgba(9, 8, 7, 255)]).unwrap();
    let cases = [
        (
            Document::new(Image::new(4, 4, Mode::Gray).unwrap()),
            Pixel::Gray(5),
        ),
        (
            Document::new(Image::new(4, 4, Mode::GrayAlpha).unwrap()),
            Pixel::GrayAlpha { gray: 5, alpha: 6 },
        ),
        (
            Document::new(Image::new_indexed(4, 4, palette).unwrap()),
            Pixel::Indexed(1),
        ),
    ];
    for (mut doc, pixel) in cases {
        let id = ImageId::from_raw(0);
        assert_eq!(draw_line(&mut doc, id, (0, 1), (3, 1), pixel), Ok(true));
        for x in 0..4 {
            assert_eq!(pixel_at(&doc, x, 1), pixel, "{pixel:?} en x = {x}");
            assert_ne!(pixel_at(&doc, x, 0), pixel, "{pixel:?} en y = 0");
        }
        assert_eq!(doc.undo(), Ok(()));
        assert_ne!(pixel_at(&doc, 0, 1), pixel);
    }
}

#[test]
fn draw_line_undo_and_redo_is_a_single_step() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let initial = doc.clone();
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 3), pixel), Ok(true));
    let drawn = doc.clone();
    for k in 0..4 {
        assert_eq!(pixel_at(&doc, k, k), pixel);
    }
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, drawn);
}

#[test]
fn there_is_never_a_panic_with_the_i32_extremes() {
    let values = [MIN, MIN + 1, -1, 0, 1, MAX - 1, MAX];
    for ax in values {
        for ay in values {
            for bx in values {
                for by in values {
                    let (a, b) = ((ax, ay), (bx, by));
                    let m = Model::new(a, b);
                    let l = Line::new(a, b);
                    assert_eq!(l.len(), m.len(), "{a:?} -> {b:?}");
                    assert_eq!(l.clone().nth(0), Some(a), "{a:?} -> {b:?}");
                    assert_eq!(l.clone().nth(idx(m.n())), Some(b), "{a:?} -> {b:?}");
                    // Solo se pregunta por el primero y se consulta el tamaño: nunca se recorre.
                    for (w, h) in [(0, 0), (0, 4), (4, 0), (4, 4), (u32::MAX, u32::MAX)] {
                        let c = l.clone().clip_to(w, h);
                        assert!(c.len() <= m.len());
                        let _ = c.clone().next();
                    }
                }
            }
        }
    }
}

#[test]
fn clipping_to_a_huge_canvas_neither_walks_nor_reserves() {
    // Los píxeles de x = 0 a MAX de la diagonal: 2^31 en total, todos dentro de u32::MAX x u32::MAX.
    let c = Line::new((MIN, MIN), (MAX, MAX)).clip_to(u32::MAX, u32::MAX);
    assert_eq!(c.len(), TWO_32 / 2);
    assert_eq!(c.clone().nth(0), Some((0, 0)));
    assert_eq!(c.clone().nth(idx(TWO_32 / 2 - 1)), Some((MAX, MAX)));
    assert_eq!(c.clone().nth(idx(TWO_32 / 2)), None);
}

#[test]
fn clipping_with_a_huge_slope_matches_the_formula() {
    // Eje x, salto de fila en el índice 2^31: desde x = 0 la fila es siempre 1.
    assert_eq!(
        clipped((MIN, 0), (MAX, 1), 4, 4),
        [(0, 1), (1, 1), (2, 1), (3, 1)]
    );
    // Horizontal enorme: solo las 16 columnas del lienzo.
    let expected: Vec<Point> = (0..16).map(|x| (x, 5)).collect();
    assert_eq!(clipped((MIN, 5), (MAX, 5), 16, 16), expected);
    assert_eq!(clipped((MIN, 20), (MAX, 20), 16, 16), EMPTY);
}

#[test]
fn clipping_keeps_the_order_from_a_to_b() {
    assert_eq!(
        clipped((5, 0), (-2, 0), 4, 1),
        [(3, 0), (2, 0), (1, 0), (0, 0)]
    );
    assert_eq!(clipped((3, 3), (0, 0), 4, 4), pixels((3, 3), (0, 0)));
}

#[test]
fn nth_and_next_on_a_small_line() {
    let all = [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)];
    for (i, expected) in all.iter().enumerate() {
        assert_eq!(Line::new((0, 0), (4, 2)).nth(i), Some(*expected));
    }
    assert_eq!(Line::new((0, 0), (4, 2)).nth(5), None);
    assert_eq!(Line::new((0, 0), (4, 2)).nth(usize::MAX), None);

    // `nth` consume lo que salta: después `next` y `len` siguen coherentes.
    let mut l = Line::new((0, 0), (4, 2));
    assert_eq!(l.nth(1), Some((1, 1)));
    assert_eq!(l.len(), 3);
    assert_eq!(l.next(), Some((2, 1)));
    assert_eq!(l.nth(1), Some((4, 2)));
    assert_eq!(l.len(), 0);
    assert!(l.is_empty());
    assert_eq!(l.next(), None);
    assert_eq!(l.nth(0), None);
}

#[test]
fn size_hint_on_small_and_huge_lines() {
    let mut l = Line::new((0, 0), (4, 2));
    assert_eq!(l.size_hint(), (5, Some(5)));
    let _ = l.next();
    assert_eq!(l.size_hint(), (4, Some(4)));
    // Comportamiento 11: `(len, Some(len))` si `len` cabe en `usize`, `(usize::MAX, None)` si no.
    // 2^32 píxeles caben en `usize` de 64 bits y no en el de 32.
    let huge = Line::new((MIN, 0), (MAX, 0));
    if usize::BITS >= 64 {
        assert_eq!(huge.size_hint(), (1 << 32, Some(1 << 32)));
    } else {
        assert_eq!(huge.size_hint(), (usize::MAX, None));
    }
    // Una línea recortada devuelve el tamaño de lo que queda, también con la cota superior.
    let clipped = Line::new((MIN, MIN), (MAX, MAX)).clip_to(4, 4);
    assert_eq!(clipped.size_hint(), (4, Some(4)));
    let empty = Line::new((0, 0), (3, 3)).clip_to(0, 4);
    assert_eq!(empty.size_hint(), (0, Some(0)));
}

// Estrategias.

/// Coordenadas pequeñas (-20..=20): hay colisiones, empates y diagonales.
fn pairs_small() -> impl Strategy<Value = Par> {
    let c = || -20i32..=20;
    ((c(), c()), (c(), c()))
}

/// Coordenada de `i32` completo, con los extremos y sus vecinos siempre presentes.
fn coordinate_complete() -> impl Strategy<Value = i32> {
    prop_oneof![
        6 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
        1 => -2i32..=2,
    ]
}

/// `i32` completo. Además de pares independientes, salen horizontales, verticales y
/// segmentos casi horizontales o verticales que cruzan un lienzo pequeño.
fn pairs_complete() -> impl Strategy<Value = Par> {
    (
        coordinate_complete(),
        coordinate_complete(),
        coordinate_complete(),
        coordinate_complete(),
        0u8..5,
        -2i32..=18,
        -2i32..=18,
    )
        .prop_map(|(ax, ay, bx, by, kind, p, q)| match kind {
            0 => ((ax, ay), (bx, by)),
            1 => ((ax, ay), (ax, by)),
            2 => ((ax, ay), (bx, ay)),
            3 => ((ax, p), (bx, q)),
            _ => ((p, ay), (q, by)),
        })
}

/// Punto para los recortes pequeños. Las coordenadas negativas se generan siempre: hay
/// variantes con las dos negativas y con una sola.
fn point_for_clip() -> impl Strategy<Value = Point> {
    prop_oneof![
        3 => (-20i32..=20, -20i32..=20),
        1 => (-20i32..=-1, -20i32..=-1),
        1 => (-20i32..=-1, 0i32..=12),
        1 => (0i32..=12, -20i32..=-1),
    ]
}

fn pairs_for_clip() -> impl Strategy<Value = Par> {
    (point_for_clip(), point_for_clip())
}

/// Tamaños de lienzo de 0 a `max`. Los de ancho cero, alto cero y 0 x 0 se generan siempre.
fn sizes_of_canvas(max: u32) -> impl Strategy<Value = (u32, u32)> {
    prop_oneof![
        4 => (0..=max, 0..=max),
        1 => (Just(0u32), 0..=max),
        1 => (0..=max, Just(0u32)),
        1 => Just((0u32, 0u32)),
    ]
}

/// Índices que merece la pena mirar en un segmento de índice final `n`, más unos al azar.
fn indices_of_interest(n: u64, chance: &[u64]) -> Vec<u64> {
    let mut v = vec![0, 1u64.min(n), n / 2, n.saturating_sub(1), n];
    v.extend(chance.iter().map(|r| r % (n + 1)));
    v
}

/// Comprueba que `p` y `q`, consecutivos en la línea `a -> b`, cumplen la 8-conectividad y
/// la monotonía del comportamiento 4.
fn checks_step(a: Point, b: Point, p: Point, q: Point) -> Result<(), TestCaseError> {
    let (dx, dy) = (
        i64::from(b.0) - i64::from(a.0),
        i64::from(b.1) - i64::from(a.1),
    );
    let (px, py) = (
        i64::from(q.0) - i64::from(p.0),
        i64::from(q.1) - i64::from(p.1),
    );
    prop_assert_eq!(px.abs().max(py.abs()), 1, "{:?} -> {:?}", p, q);
    let (greater, smaller, sign_greater, sign_smaller) = if dx.abs() >= dy.abs() {
        (px, py, dx.signum(), dy.signum())
    } else {
        (py, px, dy.signum(), dx.signum())
    };
    // El eje mayor es estrictamente monótono; el otro, monótono no estricto.
    prop_assert_eq!(greater, sign_greater, "eje mayor en {:?} -> {:?}", p, q);
    prop_assert!(
        smaller == 0 || smaller == sign_smaller,
        "eje menor en {:?} -> {:?}",
        p,
        q
    );
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: simetría A-B, coordenadas pequeñas.
    #[test]
    fn symmetry_with_small_coordinates((a, b) in pairs_small()) {
        let outward = pixels(a, b);
        let ret = reversed(pixels(b, a));
        prop_assert_eq!(&outward, &ret);
        let set_outward: BTreeSet<Point> = outward.iter().copied().collect();
        let set_return: BTreeSet<Point> = pixels(b, a).into_iter().collect();
        prop_assert_eq!(set_outward, set_return);
    }

    // Propiedad 1: simetría A-B, i32 completo (solo len y nth).
    #[test]
    fn symmetry_with_full_i32((a, b) in pairs_complete()) {
        let m = Model::new(a, b);
        let n = m.n();
        let ab = Line::new(a, b);
        let ba = Line::new(b, a);
        prop_assert_eq!(ab.len(), ba.len());
        for i in [0, 1u64.min(n), n / 2, n.saturating_sub(1), n] {
            let direct = ab.clone().nth(idx(i));
            prop_assert!(direct.is_some());
            prop_assert_eq!(direct, ba.clone().nth(idx(n - i)), "i = {}", i);
        }
    }

    // Propiedad 2: extremos incluidos, coordenadas pequeñas.
    #[test]
    fn endpoints_included_with_small_coordinates((a, b) in pairs_small()) {
        let v = pixels(a, b);
        prop_assert_eq!(v.first(), Some(&a));
        prop_assert_eq!(v.last(), Some(&b));
    }

    // Propiedad 2: extremos incluidos, i32 completo (nth(0) y nth(n)).
    #[test]
    fn endpoints_included_with_full_i32((a, b) in pairs_complete()) {
        let n = Model::new(a, b).n();
        let l = Line::new(a, b);
        prop_assert_eq!(l.clone().nth(0), Some(a));
        prop_assert_eq!(l.clone().nth(idx(n)), Some(b));
        // Y no hay nada después del último.
        prop_assert_eq!(l.clone().nth(idx(n).saturating_add(1)), None);
        prop_assert_eq!(l.clone().nth(usize::MAX), None);
    }

    // Propiedad 3: 8-conectividad y monotonía, coordenadas pequeñas.
    #[test]
    fn connectivity_with_small_coordinates((a, b) in pairs_small()) {
        let v = pixels(a, b);
        for par in v.windows(2) {
            checks_step(a, b, par[0], par[1])?;
        }
        // Distintos: consecuencia de lo anterior, pero se comprueba en directo.
        let different: BTreeSet<Point> = v.iter().copied().collect();
        prop_assert_eq!(different.len(), v.len());
    }

    // Propiedad 3: 8-conectividad y monotonía, i32 completo, en pares nth(i), nth(i + 1).
    #[test]
    fn connectivity_with_full_i32(
        (a, b) in pairs_complete(),
        chance in prop::collection::vec(any::<u64>(), 4),
    ) {
        let n = Model::new(a, b).n();
        if n == 0 {
            return Ok(());
        }
        let l = Line::new(a, b);
        // Índices de 0 a n - 1: el siguiente existe siempre.
        let mut chosen = vec![0, n / 2, n - 1];
        chosen.extend(chance.iter().map(|r| r % n));
        for i in chosen {
            let p = l.clone().nth(idx(i)).expect("i <= n");
            let q = l.clone().nth(idx(i + 1)).expect("i + 1 <= n");
            checks_step(a, b, p, q)?;
        }
    }

    // Propiedad 4: longitud y exactitud, coordenadas pequeñas (se recorre entero).
    #[test]
    fn length_and_exactness_with_small_coordinates((a, b) in pairs_small()) {
        let m = Model::new(a, b);
        prop_assert_eq!(pixels(a, b), m.all());

        // `len` baja de uno en uno hasta 0.
        let mut l = Line::new(a, b);
        let mut remaining = m.len();
        prop_assert_eq!(l.len(), remaining);
        while let Some(p) = l.next() {
            prop_assert_eq!(p, m.pixel(m.len() - remaining));
            remaining -= 1;
            prop_assert_eq!(l.len(), remaining);
            prop_assert_eq!(l.is_empty(), remaining == 0);
        }
        prop_assert_eq!(remaining, 0);
        prop_assert_eq!(l.next(), None);
    }

    // Propiedad 4: longitud y exactitud con cualquier i32 (len, nth y size_hint).
    #[test]
    fn length_and_exactness_with_full_i32(
        (a, b) in pairs_complete(),
        chance in prop::collection::vec(any::<u64>(), 4),
    ) {
        let m = Model::new(a, b);
        let n = m.n();
        let l = Line::new(a, b);
        prop_assert_eq!(l.len(), m.len());
        prop_assert!(!l.is_empty());

        // `size_hint`: `len` recortado a `usize::MAX`; exacto si cabe.
        let (under, height) = l.size_hint();
        let fits = usize::try_from(l.len()).ok();
        prop_assert_eq!(under, fits.unwrap_or(usize::MAX));
        // Comportamiento 11: `Some(len)` si cabe en `usize`; si no, sin cota superior.
        prop_assert_eq!(height, fits);

        for i in indices_of_interest(n, &chance) {
            let mut c = l.clone();
            prop_assert_eq!(c.nth(idx(i)), Some(m.pixel(i)), "nth({})", i);
            // Tras `nth(i)` quedan `n - i` píxeles y `next` da el siguiente del modelo.
            prop_assert_eq!(c.len(), n - i);
            let next = if i < n { Some(m.pixel(i + 1)) } else { None };
            prop_assert_eq!(c.next(), next, "next tras nth({})", i);
        }
    }

    // Propiedad 5: el recorte es el filtro, coordenadas pequeñas.
    #[test]
    fn clipping_equals_the_filter_with_small_coordinates(
        (a, b) in pairs_for_clip(),
        (width, height) in sizes_of_canvas(12),
    ) {
        let expected: Vec<Point> = Line::new(a, b)
            .filter(|&p| inside(p, width, height))
            .collect();
        let clip = Line::new(a, b).clip_to(width, height);
        prop_assert_eq!(clip.len(), u64::try_from(expected.len()).unwrap());
        prop_assert_eq!(clip.is_empty(), expected.is_empty());
        prop_assert_eq!(clip.collect::<Vec<Point>>(), expected);
    }

    // Propiedad 5: recorte con i32 completo y lienzo de hasta 16 x 16.
    #[test]
    fn clipping_with_full_i32(
        (a, b) in pairs_complete(),
        (width, height) in sizes_of_canvas(16),
    ) {
        let expected = model_clipped(a, b, width, height);
        let clip = Line::new(a, b).clip_to(width, height);
        // Como mucho una columna (o fila) del lienzo por píxel: no se recorre nada grande.
        prop_assert!(clip.len() <= 16, "len = {}", clip.len());
        prop_assert_eq!(clip.len(), u64::try_from(expected.len()).unwrap());
        let obtained: Vec<Point> = clip.take(17).collect();
        for &p in &obtained {
            prop_assert!(inside(p, width, height), "{:?} fuera de {}x{}", p, width, height);
        }
        prop_assert_eq!(obtained, expected);
    }

    // Propiedad 6: el comando a través de `Document`.
    #[test]
    fn command_through_document(
        width in 1u32..=16,
        height in 1u32..=16,
        background in prop::collection::vec(0u8..4, 256),
        (a, b) in prop_oneof![pairs_small(), pairs_complete()],
        preview in 0u8..3,
    ) {
        let color = Pixel::Rgba(rgba(200, 100, 50, 255));
        // Fondo con cuatro valores, uno de ellos el color de la línea: hay no-ops y cambios.
        let value = |k: u8| match k {
            0 => Rgba::TRANSPARENT,
            1 => rgba(40, 50, 60, 255),
            2 => rgba(70, 80, 90, 255),
            _ => rgba(200, 100, 50, 255),
        };
        let mut image_2 = Image::new(width, height, Mode::Rgba).unwrap();
        for y in 0..height {
            for x in 0..width {
                let k = background[usize::try_from(y * width + x).unwrap()];
                image_2.set_pixel(x, y, Pixel::Rgba(value(k))).unwrap();
            }
        }
        let mut doc = Document::new(image_2);
        let id = ImageId::from_raw(0);

        // Historia previa: ninguna, un paso, o un paso deshecho (con algo que rehacer).
        // Este color no está en el fondo, así que el paso siempre cambia algo.
        if preview >= 1 {
            let odd = Pixel::Rgba(rgba(9, 9, 9, 9));
            let step = Command::SetPixels { image: id, pixels: vec![editing(0, 0, odd)] };
            prop_assert_eq!(doc.apply(step), Ok(true));
        }
        if preview == 2 {
            prop_assert_eq!(doc.undo(), Ok(()));
        }
        let before = snapshot(&doc);
        let image_before = doc.image(id).unwrap().clone();

        let expected = model_clipped(a, b, width, height);
        let command = line_command(&doc, id, a, b, color);
        // `line_command` solo lee.
        prop_assert_eq!(snapshot(&doc), before.clone());

        if expected.is_empty() {
            // Trazo vacío: `None`, y `draw_line` no toca nada.
            prop_assert_eq!(command, Ok(None));
            prop_assert_eq!(draw_line(&mut doc, id, a, b, color), Ok(false));
            prop_assert_eq!(snapshot(&doc), before);
            return Ok(());
        }

        let edits: Vec<PixelEdit> = expected
            .iter()
            .map(|&(x, y)| editing(u32::try_from(x).unwrap(), u32::try_from(y).unwrap(), color))
            .collect();
        let command = match command {
            Ok(Some(c)) => c,
            other => {
                return Err(TestCaseError::fail(format!("se esperaba Some(..), salió {other:?}")));
            }
        };
        prop_assert_eq!(
            &command,
            &Command::SetPixels { image: id, pixels: edits }
        );

        // `apply` dice `true` si y solo si algún píxel cambió de valor.
        let changes = expected.iter().any(|&(x, y)| {
            image_before
                .get_pixel(u32::try_from(x).unwrap(), u32::try_from(y).unwrap())
                .unwrap()
                != color
        });
        let mut with_apply = doc.clone();
        prop_assert_eq!(with_apply.apply(command), Ok(changes));

        // `draw_line` hace lo mismo que `line_command` más `apply`.
        let mut with_draw = doc.clone();
        prop_assert_eq!(draw_line(&mut with_draw, id, a, b, color), Ok(changes));
        prop_assert_eq!(snapshot(&with_draw), snapshot(&with_apply));

        // Los píxeles del recorte valen el color y todos los demás quedan como estaban.
        for y in 0..height {
            for x in 0..width {
                let point = (i32::try_from(x).unwrap(), i32::try_from(y).unwrap());
                let value_expected = if expected.contains(&point) {
                    color
                } else {
                    image_before.get_pixel(x, y).unwrap()
                };
                prop_assert_eq!(pixel_at(&with_apply, x, y), value_expected, "({}, {})", x, y);
            }
        }

        if changes {
            prop_assert!(with_apply.can_undo());
            prop_assert!(!with_apply.can_redo());
            prop_assert_eq!(with_apply.undo(), Ok(()));
            // `undo` deja la imagen igual que la copia previa.
            prop_assert_eq!(with_apply.image(id), Some(&image_before));
            prop_assert_eq!(with_apply.can_undo(), before.1);
            prop_assert!(with_apply.can_redo());
        } else {
            // Sin cambios no hay paso nuevo ni se toca la historia.
            prop_assert_eq!(snapshot(&with_apply), before);
        }
    }
}
