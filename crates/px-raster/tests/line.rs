//! Tests de la línea determinista (specs/hito-2-linea.md).
//!
//! Ninguna prueba recorre ni reserva un segmento grande: los segmentos de hasta 2^32 píxeles
//! solo se consultan con `len`, `nth`, `next`, `is_empty` y `clip_to`, y todo lo que se
//! recoge en un `Vec` mide como mucho 41 píxeles (coordenadas de -20 a 20) o 16 (lienzos de
//! hasta 16 x 16). Los lienzos y las imágenes miden como mucho 16 x 16.
//!
//! El modelo exacto de la definición (comportamiento 2) está en `Modelo`: se escribe aparte,
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
fn traits_derivados() {
    assert_line_traits::<Line>();
}

// Apoyo común.

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
/// Píxeles de `i32::MIN` a `i32::MAX`: no caben en `u32`.
const DOS_32: u64 = 1 << 32;

type Punto = (i32, i32);
type Par = (Punto, Punto);

/// Resultado esperado de un recorte vacío.
const VACIA: [Punto; 0] = [];

/// Índice de `nth`; todos los que usan los tests son como mucho `2^32 - 1`.
fn idx(i: u64) -> usize {
    usize::try_from(i).expect("el índice cabe en usize")
}

fn pixeles(a: Punto, b: Punto) -> Vec<Punto> {
    Line::new(a, b).collect()
}

fn recortada(a: Punto, b: Punto, width: u32, height: u32) -> Vec<Punto> {
    Line::new(a, b).clip_to(width, height).collect()
}

fn al_reves(mut v: Vec<Punto>) -> Vec<Punto> {
    v.reverse();
    v
}

// Modelo exacto de la definición (comportamiento 2), independiente de la implementación.

/// Datos de un segmento en `i128`: `dx`, `dy` y `n = max(|dx|, |dy|)` sin desbordes.
#[derive(Clone, Copy, Debug)]
struct Modelo {
    ax: i128,
    ay: i128,
    dx: i128,
    dy: i128,
    n: i128,
}

/// División entera hacia -infinito; `den` es siempre positivo.
fn piso(num: i128, den: i128) -> i128 {
    assert!(den > 0);
    let q = num / den;
    if num % den != 0 && num < 0 { q - 1 } else { q }
}

impl Modelo {
    fn new(a: Punto, b: Punto) -> Modelo {
        let (ax, ay) = (i128::from(a.0), i128::from(a.1));
        let dx = i128::from(b.0) - ax;
        let dy = i128::from(b.1) - ay;
        Modelo {
            ax,
            ay,
            dx,
            dy,
            n: dx.abs().max(dy.abs()),
        }
    }

    /// `|dx| >= |dy|`: el eje mayor es x (también con `n == 0`).
    fn eje_x(&self) -> bool {
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
    fn pixel(&self, i: u64) -> Punto {
        let i = i128::from(i);
        assert!(i <= self.n, "índice {i} fuera de 0..={}", self.n);
        let (x, y) = if self.n == 0 {
            (self.ax, self.ay)
        } else if self.eje_x() {
            (
                self.ax + self.dx.signum() * i,
                self.ay + piso(2 * self.dy * i + self.n, 2 * self.n),
            )
        } else {
            (
                self.ax + piso(2 * self.dx * i + self.n, 2 * self.n),
                self.ay + self.dy.signum() * i,
            )
        };
        (
            i32::try_from(x).expect("x cabe en i32"),
            i32::try_from(y).expect("y cabe en i32"),
        )
    }

    /// Todos los píxeles; solo para segmentos pequeños.
    fn todos(&self) -> Vec<Punto> {
        assert!(self.n <= 1000, "segmento demasiado largo para listarlo");
        (0..self.len()).map(|i| self.pixel(i)).collect()
    }
}

/// Dentro del lienzo `0..width x 0..height`.
fn dentro(p: Punto, width: u32, height: u32) -> bool {
    (0..i64::from(width)).contains(&i64::from(p.0))
        && (0..i64::from(height)).contains(&i64::from(p.1))
}

/// Píxeles del modelo que caen en el lienzo, en orden, sin recorrer el segmento: los índices
/// candidatos salen de las `width` columnas (eje mayor x) o las `height` filas (eje mayor y).
fn modelo_recortado(a: Punto, b: Punto, width: u32, height: u32) -> Vec<Punto> {
    let m = Modelo::new(a, b);
    if width == 0 || height == 0 {
        return Vec::new();
    }
    if m.n == 0 {
        return if dentro(a, width, height) {
            vec![a]
        } else {
            Vec::new()
        };
    }
    // En el eje mayor la coordenada es `origen + signo * i`, así que `i = signo * (c - origen)`.
    let (origen, signo, lado) = if m.eje_x() {
        (m.ax, m.dx.signum(), width)
    } else {
        (m.ay, m.dy.signum(), height)
    };
    let mut indices: Vec<i128> = (0..lado)
        .map(|c| signo * (i128::from(c) - origen))
        .filter(|i| (0..=m.n).contains(i))
        .collect();
    indices.sort_unstable();
    indices
        .into_iter()
        .map(|i| m.pixel(u64::try_from(i).expect("i no es negativo")))
        .filter(|&p| dentro(p, width, height))
        .collect()
}

// Imágenes, documentos y comandos de apoyo.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn doc_rgba(width: u32, height: u32) -> Document {
    Document::new(Image::new(width, height, Mode::Rgba).unwrap())
}

fn pixel_en(doc: &Document, x: u32, y: u32) -> Pixel {
    doc.image(ImageId::from_raw(0))
        .unwrap()
        .get_pixel(x, y)
        .unwrap()
}

fn edicion(x: u32, y: u32, pixel: Pixel) -> PixelEdit {
    PixelEdit { x, y, pixel }
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
fn instantanea(doc: &Document) -> (Document, bool, bool, usize) {
    (
        doc.clone(),
        doc.can_undo(),
        doc.can_redo(),
        doc.history_bytes(),
    )
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_un_solo_punto() {
    assert_eq!(pixeles((0, 0), (0, 0)), [(0, 0)]);
    let l = Line::new((0, 0), (0, 0));
    assert_eq!(l.len(), 1);
    assert!(!l.is_empty());
}

#[test]
fn fila_horizontal_a_mano() {
    assert_eq!(
        pixeles((0, 0), (4, 0)),
        [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
    );
}

#[test]
fn fila_vertical_a_mano() {
    assert_eq!(pixeles((0, 0), (0, 3)), [(0, 0), (0, 1), (0, 2), (0, 3)]);
}

#[test]
fn fila_diagonal_pura_a_mano() {
    assert_eq!(pixeles((0, 0), (3, 3)), [(0, 0), (1, 1), (2, 2), (3, 3)]);
    assert_eq!(pixeles((3, 3), (0, 0)), [(3, 3), (2, 2), (1, 1), (0, 0)]);
    assert_eq!(
        pixeles((0, 0), (3, -3)),
        [(0, 0), (1, -1), (2, -2), (3, -3)]
    );
}

#[test]
fn fila_empate_en_eje_x_a_mano() {
    // y_real = 0,5 en x = 1: el empate sube a y = 1 (hacia +y).
    assert_eq!(pixeles((0, 0), (2, 1)), [(0, 0), (1, 1), (2, 1)]);
    assert_eq!(pixeles((2, 1), (0, 0)), [(2, 1), (1, 1), (0, 0)]);
}

#[test]
fn fila_empate_con_pendiente_negativa() {
    assert_eq!(pixeles((0, 1), (2, 0)), [(0, 1), (1, 1), (2, 0)]);
}

#[test]
fn fila_pendiente_un_medio() {
    assert_eq!(
        pixeles((0, 0), (4, 2)),
        [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)]
    );
}

#[test]
fn fila_empate_en_eje_y_va_hacia_mas_x() {
    assert_eq!(
        pixeles((0, 0), (2, 4)),
        [(0, 0), (1, 1), (1, 2), (2, 3), (2, 4)]
    );
}

#[test]
fn fila_pendiente_sin_empate() {
    assert_eq!(pixeles((0, 0), (3, 1)), [(0, 0), (1, 0), (2, 1), (3, 1)]);
}

#[test]
fn fila_floor_con_negativos() {
    assert_eq!(
        pixeles((0, 0), (-4, -2)),
        [(0, 0), (-1, 0), (-2, -1), (-3, -1), (-4, -2)]
    );
    assert_eq!(
        pixeles((-4, -2), (0, 0)),
        [(-4, -2), (-3, -1), (-2, -1), (-1, 0), (0, 0)]
    );
}

#[test]
fn fila_horizontal_de_extremo_a_extremo() {
    let l = Line::new((MIN, 0), (MAX, 0));
    assert_eq!(l.len(), DOS_32);
    assert!(!l.is_empty());
    assert_eq!(l.clone().nth(0), Some((MIN, 0)));
    assert_eq!(l.clone().nth(idx(DOS_32 - 1)), Some((MAX, 0)));
    // El punto medio es el 2^31, y justo detrás del último ya no hay nada.
    assert_eq!(l.clone().nth(idx(DOS_32 / 2)), Some((0, 0)));
    // Justo detrás del último ya no hay nada (sin `nth(2^32)`, que no cabe en 32 bits).
    let mut u = l.clone();
    assert_eq!(u.nth(idx(DOS_32 - 1)), Some((MAX, 0)));
    assert_eq!(u.next(), None);
}

#[test]
fn fila_diagonal_de_extremo_a_extremo() {
    let l = Line::new((MIN, MIN), (MAX, MAX));
    assert_eq!(l.len(), DOS_32);
    assert_eq!(l.clone().nth(0), Some((MIN, MIN)));
    assert_eq!(l.clone().nth(1), Some((MIN + 1, MIN + 1)));
    assert_eq!(l.clone().nth(idx(DOS_32 - 1)), Some((MAX, MAX)));
    assert_eq!(l.clone().nth(idx(DOS_32 / 2)), Some((0, 0)));
}

#[test]
fn fila_dx_impar_de_2_32_menos_1() {
    // n = 2^32 - 1 e y sube de 0 a 1 justo a mitad: el salto de fila ocurre en el índice 2^31.
    let l = Line::new((MIN, 0), (MAX, 1));
    assert_eq!(l.len(), DOS_32);
    assert_eq!(l.clone().nth(idx(DOS_32 / 2 - 1)), Some((-1, 0)));
    assert_eq!(l.clone().nth(idx(DOS_32 / 2)), Some((0, 1)));
    assert_eq!(l.clone().nth(0), Some((MIN, 0)));
    assert_eq!(l.clone().nth(idx(DOS_32 - 1)), Some((MAX, 1)));
}

#[test]
fn fila_diagonal_enorme_recortada_a_4x4() {
    // Instantáneo: no recorre 2^32 píxeles.
    assert_eq!(
        recortada((MIN, MIN), (MAX, MAX), 4, 4),
        [(0, 0), (1, 1), (2, 2), (3, 3)]
    );
}

#[test]
fn fila_recorte_es_el_prefijo_de_la_linea() {
    assert_eq!(recortada((0, 0), (4, 2), 3, 2), [(0, 0), (1, 1), (2, 1)]);
}

#[test]
fn fila_recorte_vacio_con_caja_que_corta_el_lienzo() {
    // La caja envolvente de (-3,2)-(2,-3) corta el lienzo, pero la diagonal no lo toca.
    assert_eq!(recortada((-3, 2), (2, -3), 4, 4), VACIA);
    assert_eq!(recortada((10, 10), (20, 20), 4, 4), VACIA);
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
fn fila_coordenadas_negativas_recortadas_a_4x4() {
    // Todos los píxeles tienen x < 0 o y < 0.
    assert_eq!(recortada((-5, -5), (-1, -1), 4, 4), VACIA);
    assert_eq!(recortada((-5, 0), (-1, 0), 4, 4), VACIA);
    assert_eq!(recortada((0, -9), (0, -1), 4, 4), VACIA);
}

#[test]
fn fila_recorte_con_lado_cero_y_con_inicio_fuera() {
    assert_eq!(
        recortada((-2, 0), (5, 0), 4, 1),
        [(0, 0), (1, 0), (2, 0), (3, 0)]
    );
    // `width` o `height` cero no admiten ningún píxel.
    assert_eq!(recortada((0, 0), (3, 3), 0, 4), VACIA);
    assert_eq!(recortada((0, 0), (3, 3), 4, 0), VACIA);
    assert_eq!(recortada((0, 0), (3, 3), 0, 0), VACIA);
    assert!(Line::new((0, 0), (3, 3)).clip_to(0, 0).is_empty());
}

#[test]
fn fila_line_command_diagonal_rgba() {
    let doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    let esperado = Command::SetPixels {
        image: id,
        pixels: vec![
            edicion(0, 0, pixel),
            edicion(1, 1, pixel),
            edicion(2, 2, pixel),
            edicion(3, 3, pixel),
        ],
    };
    let antes = instantanea(&doc);
    assert_eq!(
        line_command(&doc, id, (0, 0), (3, 3), pixel),
        Ok(Some(esperado))
    );
    // Solo lee: el documento queda igual.
    assert_eq!(instantanea(&doc), antes);
}

#[test]
fn fila_draw_line_totalmente_fuera() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));

    // En un documento nuevo.
    let antes = instantanea(&doc);
    assert_eq!(
        draw_line(&mut doc, id, (10, 10), (20, 20), pixel),
        Ok(false)
    );
    assert!(!doc.can_undo());
    assert_eq!(instantanea(&doc), antes);
}

#[test]
fn fila_draw_line_fuera_con_historia_y_rehacer() {
    // Con historia y con algo que rehacer: nada se toca (ni `can_undo`, `can_redo` ni los bytes).
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 0), pixel), Ok(true));
    assert_eq!(draw_line(&mut doc, id, (0, 3), (3, 3), pixel), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    let antes = instantanea(&doc);
    assert!(antes.1 && antes.2);
    for (a, b) in [((-9, -9), (-1, -1)), ((4, 0), (9, 0)), ((0, -3), (3, -3))] {
        assert_eq!(draw_line(&mut doc, id, a, b, pixel), Ok(false));
        assert_eq!(instantanea(&doc), antes);
    }
}

#[test]
fn fila_draw_line_con_el_mismo_color_que_ya_hay() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 3), pixel), Ok(true));
    let antes = instantanea(&doc);
    // `apply` decide que no cambia nada y no añade paso.
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 3), pixel), Ok(false));
    assert_eq!(instantanea(&doc), antes);
}

#[test]
fn fila_draw_line_con_imagen_desconocida() {
    let mut doc = doc_rgba(4, 4);
    let desconocida = ImageId::from_raw(99);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    let antes = instantanea(&doc);
    assert_eq!(
        draw_line(&mut doc, desconocida, (0, 0), (3, 3), pixel),
        Err(DocumentError::UnknownImage(desconocida))
    );
    assert_eq!(
        line_command(&doc, desconocida, (0, 0), (3, 3), pixel),
        Err(DocumentError::UnknownImage(desconocida))
    );
    assert_eq!(instantanea(&doc), antes);
}

/// Comportamiento 11: la imagen se busca antes de recortar, así que `UnknownImage` gana a `None`
/// incluso con una línea que queda totalmente fuera del lienzo.
#[test]
fn fila_imagen_desconocida_gana_a_linea_fuera_del_lienzo() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let desconocida = ImageId::from_raw(99);
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    let fuera: [Par; 6] = [
        ((10, 10), (20, 20)),
        ((-5, -5), (-1, -1)),
        ((MIN, MIN), (MIN, MIN)),
        ((MIN, 0), (MIN, 5)),
        ((-3, 2), (2, -3)),
        ((MAX, MAX), (MIN, MAX)),
    ];
    let antes = instantanea(&doc);
    for (a, b) in fuera {
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
            line_command(&doc, desconocida, a, b, pixel),
            Err(DocumentError::UnknownImage(desconocida)),
            "{a:?}->{b:?}"
        );
        assert_eq!(
            draw_line(&mut doc, desconocida, a, b, pixel),
            Err(DocumentError::UnknownImage(desconocida)),
            "{a:?}->{b:?}"
        );
        assert_eq!(instantanea(&doc), antes);
    }
}

#[test]
fn fila_draw_line_con_pixel_de_otro_modo() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let antes = instantanea(&doc);
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
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(Rgba::TRANSPARENT));
    assert_eq!(instantanea(&doc), antes);
}

// Comportamientos que la tabla no lista como fila.

#[test]
fn line_command_no_comprueba_el_modo_del_pixel() {
    // Lo valida `Document` al aplicar el comando (comportamiento 9).
    let doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let esperado = Command::SetPixels {
        image: id,
        pixels: vec![edicion(0, 0, Pixel::Gray(7)), edicion(1, 0, Pixel::Gray(7))],
    };
    assert_eq!(
        line_command(&doc, id, (0, 0), (1, 0), Pixel::Gray(7)),
        Ok(Some(esperado))
    );
}

#[test]
fn draw_line_en_los_cuatro_modos() {
    let paleta = Palette::from_colors([rgba(0, 0, 0, 255), rgba(9, 8, 7, 255)]).unwrap();
    let casos = [
        (
            Document::new(Image::new(4, 4, Mode::Gray).unwrap()),
            Pixel::Gray(5),
        ),
        (
            Document::new(Image::new(4, 4, Mode::GrayAlpha).unwrap()),
            Pixel::GrayAlpha { gray: 5, alpha: 6 },
        ),
        (
            Document::new(Image::new_indexed(4, 4, paleta).unwrap()),
            Pixel::Indexed(1),
        ),
    ];
    for (mut doc, pixel) in casos {
        let id = ImageId::from_raw(0);
        assert_eq!(draw_line(&mut doc, id, (0, 1), (3, 1), pixel), Ok(true));
        for x in 0..4 {
            assert_eq!(pixel_en(&doc, x, 1), pixel, "{pixel:?} en x = {x}");
            assert_ne!(pixel_en(&doc, x, 0), pixel, "{pixel:?} en y = 0");
        }
        assert_eq!(doc.undo(), Ok(()));
        assert_ne!(pixel_en(&doc, 0, 1), pixel);
    }
}

#[test]
fn draw_line_deshacer_y_rehacer_es_un_solo_paso() {
    let mut doc = doc_rgba(4, 4);
    let id = ImageId::from_raw(0);
    let inicial = doc.clone();
    let pixel = Pixel::Rgba(rgba(200, 100, 50, 255));
    assert_eq!(draw_line(&mut doc, id, (0, 0), (3, 3), pixel), Ok(true));
    let dibujada = doc.clone();
    for k in 0..4 {
        assert_eq!(pixel_en(&doc, k, k), pixel);
    }
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, dibujada);
}

#[test]
fn nunca_hay_panico_con_los_extremos_de_i32() {
    let valores = [MIN, MIN + 1, -1, 0, 1, MAX - 1, MAX];
    for ax in valores {
        for ay in valores {
            for bx in valores {
                for by in valores {
                    let (a, b) = ((ax, ay), (bx, by));
                    let m = Modelo::new(a, b);
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
fn recorte_a_un_lienzo_enorme_no_recorre_ni_reserva() {
    // Los píxeles de x = 0 a MAX de la diagonal: 2^31 en total, todos dentro de u32::MAX x u32::MAX.
    let c = Line::new((MIN, MIN), (MAX, MAX)).clip_to(u32::MAX, u32::MAX);
    assert_eq!(c.len(), DOS_32 / 2);
    assert_eq!(c.clone().nth(0), Some((0, 0)));
    assert_eq!(c.clone().nth(idx(DOS_32 / 2 - 1)), Some((MAX, MAX)));
    assert_eq!(c.clone().nth(idx(DOS_32 / 2)), None);
}

#[test]
fn recorte_de_pendiente_enorme_coincide_con_la_formula() {
    // Eje x, salto de fila en el índice 2^31: desde x = 0 la fila es siempre 1.
    assert_eq!(
        recortada((MIN, 0), (MAX, 1), 4, 4),
        [(0, 1), (1, 1), (2, 1), (3, 1)]
    );
    // Horizontal enorme: solo las 16 columnas del lienzo.
    let esperado: Vec<Punto> = (0..16).map(|x| (x, 5)).collect();
    assert_eq!(recortada((MIN, 5), (MAX, 5), 16, 16), esperado);
    assert_eq!(recortada((MIN, 20), (MAX, 20), 16, 16), VACIA);
}

#[test]
fn recorte_conserva_el_orden_de_a_a_b() {
    assert_eq!(
        recortada((5, 0), (-2, 0), 4, 1),
        [(3, 0), (2, 0), (1, 0), (0, 0)]
    );
    assert_eq!(recortada((3, 3), (0, 0), 4, 4), pixeles((3, 3), (0, 0)));
}

#[test]
fn nth_y_next_en_una_linea_pequena() {
    let todos = [(0, 0), (1, 1), (2, 1), (3, 2), (4, 2)];
    for (i, esperado) in todos.iter().enumerate() {
        assert_eq!(Line::new((0, 0), (4, 2)).nth(i), Some(*esperado));
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
fn size_hint_en_lineas_pequenas_y_enormes() {
    let mut l = Line::new((0, 0), (4, 2));
    assert_eq!(l.size_hint(), (5, Some(5)));
    let _ = l.next();
    assert_eq!(l.size_hint(), (4, Some(4)));
    // Comportamiento 11: `(len, Some(len))` si `len` cabe en `usize`, `(usize::MAX, None)` si no.
    // 2^32 píxeles caben en `usize` de 64 bits y no en el de 32.
    let enorme = Line::new((MIN, 0), (MAX, 0));
    if usize::BITS >= 64 {
        assert_eq!(enorme.size_hint(), (1 << 32, Some(1 << 32)));
    } else {
        assert_eq!(enorme.size_hint(), (usize::MAX, None));
    }
    // Una línea recortada devuelve el tamaño de lo que queda, también con la cota superior.
    let recortada = Line::new((MIN, MIN), (MAX, MAX)).clip_to(4, 4);
    assert_eq!(recortada.size_hint(), (4, Some(4)));
    let vacia = Line::new((0, 0), (3, 3)).clip_to(0, 4);
    assert_eq!(vacia.size_hint(), (0, Some(0)));
}

// Estrategias.

/// Coordenadas pequeñas (-20..=20): hay colisiones, empates y diagonales.
fn pares_pequenos() -> impl Strategy<Value = Par> {
    let c = || -20i32..=20;
    ((c(), c()), (c(), c()))
}

/// Coordenada de `i32` completo, con los extremos y sus vecinos siempre presentes.
fn coordenada_completa() -> impl Strategy<Value = i32> {
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
fn pares_completos() -> impl Strategy<Value = Par> {
    (
        coordenada_completa(),
        coordenada_completa(),
        coordenada_completa(),
        coordenada_completa(),
        0u8..5,
        -2i32..=18,
        -2i32..=18,
    )
        .prop_map(|(ax, ay, bx, by, tipo, p, q)| match tipo {
            0 => ((ax, ay), (bx, by)),
            1 => ((ax, ay), (ax, by)),
            2 => ((ax, ay), (bx, ay)),
            3 => ((ax, p), (bx, q)),
            _ => ((p, ay), (q, by)),
        })
}

/// Punto para los recortes pequeños. Las coordenadas negativas se generan siempre: hay
/// variantes con las dos negativas y con una sola.
fn punto_para_recorte() -> impl Strategy<Value = Punto> {
    prop_oneof![
        3 => (-20i32..=20, -20i32..=20),
        1 => (-20i32..=-1, -20i32..=-1),
        1 => (-20i32..=-1, 0i32..=12),
        1 => (0i32..=12, -20i32..=-1),
    ]
}

fn pares_para_recorte() -> impl Strategy<Value = Par> {
    (punto_para_recorte(), punto_para_recorte())
}

/// Tamaños de lienzo de 0 a `max`. Los de ancho cero, alto cero y 0 x 0 se generan siempre.
fn tamanos_de_lienzo(max: u32) -> impl Strategy<Value = (u32, u32)> {
    prop_oneof![
        4 => (0..=max, 0..=max),
        1 => (Just(0u32), 0..=max),
        1 => (0..=max, Just(0u32)),
        1 => Just((0u32, 0u32)),
    ]
}

/// Índices que merece la pena mirar en un segmento de índice final `n`, más unos al azar.
fn indices_de_interes(n: u64, azar: &[u64]) -> Vec<u64> {
    let mut v = vec![0, 1u64.min(n), n / 2, n.saturating_sub(1), n];
    v.extend(azar.iter().map(|r| r % (n + 1)));
    v
}

/// Comprueba que `p` y `q`, consecutivos en la línea `a -> b`, cumplen la 8-conectividad y
/// la monotonía del comportamiento 4.
fn comprueba_paso(a: Punto, b: Punto, p: Punto, q: Punto) -> Result<(), TestCaseError> {
    let (dx, dy) = (
        i64::from(b.0) - i64::from(a.0),
        i64::from(b.1) - i64::from(a.1),
    );
    let (px, py) = (
        i64::from(q.0) - i64::from(p.0),
        i64::from(q.1) - i64::from(p.1),
    );
    prop_assert_eq!(px.abs().max(py.abs()), 1, "{:?} -> {:?}", p, q);
    let (mayor, menor, signo_mayor, signo_menor) = if dx.abs() >= dy.abs() {
        (px, py, dx.signum(), dy.signum())
    } else {
        (py, px, dy.signum(), dx.signum())
    };
    // El eje mayor es estrictamente monótono; el otro, monótono no estricto.
    prop_assert_eq!(mayor, signo_mayor, "eje mayor en {:?} -> {:?}", p, q);
    prop_assert!(
        menor == 0 || menor == signo_menor,
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
    fn simetria_con_coordenadas_pequenas((a, b) in pares_pequenos()) {
        let ida = pixeles(a, b);
        let vuelta = al_reves(pixeles(b, a));
        prop_assert_eq!(&ida, &vuelta);
        let conjunto_ida: BTreeSet<Punto> = ida.iter().copied().collect();
        let conjunto_vuelta: BTreeSet<Punto> = pixeles(b, a).into_iter().collect();
        prop_assert_eq!(conjunto_ida, conjunto_vuelta);
    }

    // Propiedad 1: simetría A-B, i32 completo (solo len y nth).
    #[test]
    fn simetria_con_i32_completo((a, b) in pares_completos()) {
        let m = Modelo::new(a, b);
        let n = m.n();
        let ab = Line::new(a, b);
        let ba = Line::new(b, a);
        prop_assert_eq!(ab.len(), ba.len());
        for i in [0, 1u64.min(n), n / 2, n.saturating_sub(1), n] {
            let directo = ab.clone().nth(idx(i));
            prop_assert!(directo.is_some());
            prop_assert_eq!(directo, ba.clone().nth(idx(n - i)), "i = {}", i);
        }
    }

    // Propiedad 2: extremos incluidos, coordenadas pequeñas.
    #[test]
    fn extremos_incluidos_con_coordenadas_pequenas((a, b) in pares_pequenos()) {
        let v = pixeles(a, b);
        prop_assert_eq!(v.first(), Some(&a));
        prop_assert_eq!(v.last(), Some(&b));
    }

    // Propiedad 2: extremos incluidos, i32 completo (nth(0) y nth(n)).
    #[test]
    fn extremos_incluidos_con_i32_completo((a, b) in pares_completos()) {
        let n = Modelo::new(a, b).n();
        let l = Line::new(a, b);
        prop_assert_eq!(l.clone().nth(0), Some(a));
        prop_assert_eq!(l.clone().nth(idx(n)), Some(b));
        // Y no hay nada después del último.
        prop_assert_eq!(l.clone().nth(idx(n).saturating_add(1)), None);
        prop_assert_eq!(l.clone().nth(usize::MAX), None);
    }

    // Propiedad 3: 8-conectividad y monotonía, coordenadas pequeñas.
    #[test]
    fn conectividad_con_coordenadas_pequenas((a, b) in pares_pequenos()) {
        let v = pixeles(a, b);
        for par in v.windows(2) {
            comprueba_paso(a, b, par[0], par[1])?;
        }
        // Distintos: consecuencia de lo anterior, pero se comprueba en directo.
        let distintos: BTreeSet<Punto> = v.iter().copied().collect();
        prop_assert_eq!(distintos.len(), v.len());
    }

    // Propiedad 3: 8-conectividad y monotonía, i32 completo, en pares nth(i), nth(i + 1).
    #[test]
    fn conectividad_con_i32_completo(
        (a, b) in pares_completos(),
        azar in prop::collection::vec(any::<u64>(), 4),
    ) {
        let n = Modelo::new(a, b).n();
        if n == 0 {
            return Ok(());
        }
        let l = Line::new(a, b);
        // Índices de 0 a n - 1: el siguiente existe siempre.
        let mut elegidos = vec![0, n / 2, n - 1];
        elegidos.extend(azar.iter().map(|r| r % n));
        for i in elegidos {
            let p = l.clone().nth(idx(i)).expect("i <= n");
            let q = l.clone().nth(idx(i + 1)).expect("i + 1 <= n");
            comprueba_paso(a, b, p, q)?;
        }
    }

    // Propiedad 4: longitud y exactitud, coordenadas pequeñas (se recorre entero).
    #[test]
    fn longitud_y_exactitud_con_coordenadas_pequenas((a, b) in pares_pequenos()) {
        let m = Modelo::new(a, b);
        prop_assert_eq!(pixeles(a, b), m.todos());

        // `len` baja de uno en uno hasta 0.
        let mut l = Line::new(a, b);
        let mut restantes = m.len();
        prop_assert_eq!(l.len(), restantes);
        while let Some(p) = l.next() {
            prop_assert_eq!(p, m.pixel(m.len() - restantes));
            restantes -= 1;
            prop_assert_eq!(l.len(), restantes);
            prop_assert_eq!(l.is_empty(), restantes == 0);
        }
        prop_assert_eq!(restantes, 0);
        prop_assert_eq!(l.next(), None);
    }

    // Propiedad 4: longitud y exactitud con cualquier i32 (len, nth y size_hint).
    #[test]
    fn longitud_y_exactitud_con_i32_completo(
        (a, b) in pares_completos(),
        azar in prop::collection::vec(any::<u64>(), 4),
    ) {
        let m = Modelo::new(a, b);
        let n = m.n();
        let l = Line::new(a, b);
        prop_assert_eq!(l.len(), m.len());
        prop_assert!(!l.is_empty());

        // `size_hint`: `len` recortado a `usize::MAX`; exacto si cabe.
        let (bajo, alto) = l.size_hint();
        let cabe = usize::try_from(l.len()).ok();
        prop_assert_eq!(bajo, cabe.unwrap_or(usize::MAX));
        // Comportamiento 11: `Some(len)` si cabe en `usize`; si no, sin cota superior.
        prop_assert_eq!(alto, cabe);

        for i in indices_de_interes(n, &azar) {
            let mut c = l.clone();
            prop_assert_eq!(c.nth(idx(i)), Some(m.pixel(i)), "nth({})", i);
            // Tras `nth(i)` quedan `n - i` píxeles y `next` da el siguiente del modelo.
            prop_assert_eq!(c.len(), n - i);
            let siguiente = if i < n { Some(m.pixel(i + 1)) } else { None };
            prop_assert_eq!(c.next(), siguiente, "next tras nth({})", i);
        }
    }

    // Propiedad 5: el recorte es el filtro, coordenadas pequeñas.
    #[test]
    fn recorte_igual_al_filtro_con_coordenadas_pequenas(
        (a, b) in pares_para_recorte(),
        (width, height) in tamanos_de_lienzo(12),
    ) {
        let esperado: Vec<Punto> = Line::new(a, b)
            .filter(|&p| dentro(p, width, height))
            .collect();
        let recorte = Line::new(a, b).clip_to(width, height);
        prop_assert_eq!(recorte.len(), u64::try_from(esperado.len()).unwrap());
        prop_assert_eq!(recorte.is_empty(), esperado.is_empty());
        prop_assert_eq!(recorte.collect::<Vec<Punto>>(), esperado);
    }

    // Propiedad 5: recorte con i32 completo y lienzo de hasta 16 x 16.
    #[test]
    fn recorte_con_i32_completo(
        (a, b) in pares_completos(),
        (width, height) in tamanos_de_lienzo(16),
    ) {
        let esperado = modelo_recortado(a, b, width, height);
        let recorte = Line::new(a, b).clip_to(width, height);
        // Como mucho una columna (o fila) del lienzo por píxel: no se recorre nada grande.
        prop_assert!(recorte.len() <= 16, "len = {}", recorte.len());
        prop_assert_eq!(recorte.len(), u64::try_from(esperado.len()).unwrap());
        let obtenido: Vec<Punto> = recorte.take(17).collect();
        for &p in &obtenido {
            prop_assert!(dentro(p, width, height), "{:?} fuera de {}x{}", p, width, height);
        }
        prop_assert_eq!(obtenido, esperado);
    }

    // Propiedad 6: el comando a través de `Document`.
    #[test]
    fn comando_a_traves_de_document(
        width in 1u32..=16,
        height in 1u32..=16,
        fondo in prop::collection::vec(0u8..4, 256),
        (a, b) in prop_oneof![pares_pequenos(), pares_completos()],
        previa in 0u8..3,
    ) {
        let color = Pixel::Rgba(rgba(200, 100, 50, 255));
        // Fondo con cuatro valores, uno de ellos el color de la línea: hay no-ops y cambios.
        let valor = |k: u8| match k {
            0 => Rgba::TRANSPARENT,
            1 => rgba(40, 50, 60, 255),
            2 => rgba(70, 80, 90, 255),
            _ => rgba(200, 100, 50, 255),
        };
        let mut imagen = Image::new(width, height, Mode::Rgba).unwrap();
        for y in 0..height {
            for x in 0..width {
                let k = fondo[usize::try_from(y * width + x).unwrap()];
                imagen.set_pixel(x, y, Pixel::Rgba(valor(k))).unwrap();
            }
        }
        let mut doc = Document::new(imagen);
        let id = ImageId::from_raw(0);

        // Historia previa: ninguna, un paso, o un paso deshecho (con algo que rehacer).
        // Este color no está en el fondo, así que el paso siempre cambia algo.
        if previa >= 1 {
            let raro = Pixel::Rgba(rgba(9, 9, 9, 9));
            let paso = Command::SetPixels { image: id, pixels: vec![edicion(0, 0, raro)] };
            prop_assert_eq!(doc.apply(paso), Ok(true));
        }
        if previa == 2 {
            prop_assert_eq!(doc.undo(), Ok(()));
        }
        let antes = instantanea(&doc);
        let imagen_antes = doc.image(id).unwrap().clone();

        let esperado = modelo_recortado(a, b, width, height);
        let comando = line_command(&doc, id, a, b, color);
        // `line_command` solo lee.
        prop_assert_eq!(instantanea(&doc), antes.clone());

        if esperado.is_empty() {
            // Trazo vacío: `None`, y `draw_line` no toca nada.
            prop_assert_eq!(comando, Ok(None));
            prop_assert_eq!(draw_line(&mut doc, id, a, b, color), Ok(false));
            prop_assert_eq!(instantanea(&doc), antes);
            return Ok(());
        }

        let ediciones: Vec<PixelEdit> = esperado
            .iter()
            .map(|&(x, y)| edicion(u32::try_from(x).unwrap(), u32::try_from(y).unwrap(), color))
            .collect();
        let comando = match comando {
            Ok(Some(c)) => c,
            otro => {
                return Err(TestCaseError::fail(format!("se esperaba Some(..), salió {otro:?}")));
            }
        };
        prop_assert_eq!(
            &comando,
            &Command::SetPixels { image: id, pixels: ediciones }
        );

        // `apply` dice `true` si y solo si algún píxel cambió de valor.
        let cambia = esperado.iter().any(|&(x, y)| {
            imagen_antes
                .get_pixel(u32::try_from(x).unwrap(), u32::try_from(y).unwrap())
                .unwrap()
                != color
        });
        let mut con_apply = doc.clone();
        prop_assert_eq!(con_apply.apply(comando), Ok(cambia));

        // `draw_line` hace lo mismo que `line_command` más `apply`.
        let mut con_draw = doc.clone();
        prop_assert_eq!(draw_line(&mut con_draw, id, a, b, color), Ok(cambia));
        prop_assert_eq!(instantanea(&con_draw), instantanea(&con_apply));

        // Los píxeles del recorte valen el color y todos los demás quedan como estaban.
        for y in 0..height {
            for x in 0..width {
                let punto = (i32::try_from(x).unwrap(), i32::try_from(y).unwrap());
                let valor_esperado = if esperado.contains(&punto) {
                    color
                } else {
                    imagen_antes.get_pixel(x, y).unwrap()
                };
                prop_assert_eq!(pixel_en(&con_apply, x, y), valor_esperado, "({}, {})", x, y);
            }
        }

        if cambia {
            prop_assert!(con_apply.can_undo());
            prop_assert!(!con_apply.can_redo());
            prop_assert_eq!(con_apply.undo(), Ok(()));
            // `undo` deja la imagen igual que la copia previa.
            prop_assert_eq!(con_apply.image(id), Some(&imagen_antes));
            prop_assert_eq!(con_apply.can_undo(), antes.1);
            prop_assert!(con_apply.can_redo());
        } else {
            // Sin cambios no hay paso nuevo ni se toca la historia.
            prop_assert_eq!(instantanea(&con_apply), antes);
        }
    }
}
