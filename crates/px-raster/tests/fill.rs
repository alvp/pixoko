//! Tests del relleno por inundación (specs/hito-2-relleno.md).
//!
//! Una prueba unitaria por cada fila de «Casos límite» (`fila_NN_...`) y una propiedad por
//! cada una de las seis de «Propiedades». Las filas 1 a 15 y las propiedades usan imágenes de
//! 12 x 12 como mucho. La fila 16 es la única grande: 1024 x 1024 y 2048 x 2048, las dos en
//! la ejecución normal (de 0,6 a 1,8 s cada una en debug); imprimen sus tiempos y solo fallan por
//! encima de 120 s.
//!
//! El oráculo (`region_ingenua`) es un recorrido pixel a pixel con `VecDeque`, 4 vecinos y
//! `Image::get_pixel`; no usa nada de `px-raster`. Las filas «a mano» son listas literales.

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba, Span,
};
use px_raster::{fill_command, flood_fill};

// Apoyo común.

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
const TODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// `A`, `B` y `C` de la tabla: tres colores distintos (`C` es el de relleno).
fn va() -> Pixel {
    Pixel::Rgba(rgba(10, 20, 30, 255))
}

fn vb() -> Pixel {
    Pixel::Rgba(rgba(40, 50, 60, 255))
}

fn vc() -> Pixel {
    Pixel::Rgba(rgba(200, 100, 50, 255))
}

/// `sp(y, a, b)` de la tabla.
fn sp(y: u32, x_start: u32, x_end: u32) -> Span {
    Span { y, x_start, x_end }
}

fn i(v: u32) -> i32 {
    i32::try_from(v).expect("cabe en i32")
}

fn ancho_usize(v: u32) -> usize {
    usize::try_from(v).expect("cabe en usize")
}

fn paleta(colores: &[Rgba]) -> Palette {
    Palette::from_colors(colores.iter().copied()).unwrap()
}

/// Imagen RGBA con el píxel que da `f` en cada casilla.
fn imagen_de(width: u32, height: u32, f: impl Fn(u32, u32) -> Pixel) -> Image {
    let mut img = Image::new(width, height, Mode::Rgba).unwrap();
    for y in 0..height {
        for x in 0..width {
            img.set_pixel(x, y, f(x, y)).unwrap();
        }
    }
    img
}

/// Imagen RGBA a partir de filas de texto con `A` y `B` (de arriba abajo).
fn imagen_matriz(filas: &[&str]) -> Image {
    let height = u32::try_from(filas.len()).unwrap();
    let width = u32::try_from(filas[0].len()).unwrap();
    imagen_de(width, height, |x, y| {
        match filas[ancho_usize(y)].as_bytes()[ancho_usize(x)] {
            b'A' => va(),
            b'B' => vb(),
            otro => panic!("carácter {otro} no válido en la matriz"),
        }
    })
}

fn doc_matriz(filas: &[&str]) -> Document {
    Document::new(imagen_matriz(filas))
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

fn edicion(x: u32, y: u32, pixel: Pixel) -> PixelEdit {
    PixelEdit { x, y, pixel }
}

/// Deja en `doc` un paso deshecho (`can_undo` a `false`, `can_redo` a `true`): así se ve si
/// una operación borra la pila de rehacer. El píxel `(0, 0)` debe poder cambiar a `otro`.
fn con_rehacer(doc: &mut Document, otro: Pixel) {
    let id = ImageId::from_raw(0);
    let paso = Command::SetPixels {
        image: id,
        pixels: vec![edicion(0, 0, otro)],
    };
    assert_eq!(doc.apply(paso), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
}

/// `Ok(Some(FillSpans { .. }))` con los datos dados.
fn comando(image: ImageId, spans: &[Span], pixel: Pixel) -> Result<Option<Command>, DocumentError> {
    Ok(Some(Command::FillSpans {
        image,
        spans: spans.to_vec(),
        pixel,
    }))
}

/// Comprueba una fila «a mano»: `fill_command` da exactamente `spans` con `vc()`, no toca el
/// documento; `flood_fill` lo aplica (`Ok(true)`, la imagen es la original con esos tramos
/// pintados y nada más) y `undo` la restaura.
fn comprueba_region(doc: &Document, semilla: (i32, i32), spans: &[Span]) {
    let id = ImageId::from_raw(0);
    let antes = instantanea(doc);
    assert_eq!(
        fill_command(doc, id, semilla.0, semilla.1, vc()),
        comando(id, spans, vc()),
        "semilla {semilla:?}"
    );
    assert_eq!(instantanea(doc), antes, "fill_command no toca el documento");

    let mut esperada = doc.image(id).unwrap().clone();
    for s in spans {
        for x in s.x_start..s.x_end {
            esperada.set_pixel(x, s.y, vc()).unwrap();
        }
    }
    let mut copia = doc.clone();
    assert_eq!(
        flood_fill(&mut copia, id, semilla.0, semilla.1, vc()),
        Ok(true)
    );
    assert_eq!(copia.image(id), Some(&esperada), "semilla {semilla:?}");
    assert_eq!(copia.undo(), Ok(()));
    assert_eq!(copia.image(id), doc.image(id), "undo restaura la imagen");
}

/// Comprueba que `fill_command` y `flood_fill` dan `Err(error)` y que el documento no cambia.
fn comprueba_error(
    doc: &mut Document,
    id: ImageId,
    semilla: (i32, i32),
    pixel: Pixel,
    error: &DocumentError,
) {
    let antes = instantanea(doc);
    assert_eq!(
        fill_command(doc, id, semilla.0, semilla.1, pixel),
        Err(error.clone()),
        "fill_command, semilla {semilla:?}"
    );
    assert_eq!(
        flood_fill(doc, id, semilla.0, semilla.1, pixel),
        Err(error.clone()),
        "flood_fill, semilla {semilla:?}"
    );
    assert_eq!(instantanea(doc), antes, "semilla {semilla:?}");
}

/// Comprueba que no hay nada que cambiar y que la historia no se toca.
fn comprueba_fuera(doc: &mut Document, id: ImageId, semilla: (i32, i32), pixel: Pixel) {
    let antes = instantanea(doc);
    assert_eq!(
        fill_command(doc, id, semilla.0, semilla.1, pixel),
        Ok(None),
        "fill_command, semilla {semilla:?}"
    );
    assert_eq!(
        flood_fill(doc, id, semilla.0, semilla.1, pixel),
        Ok(false),
        "flood_fill, semilla {semilla:?}"
    );
    assert_eq!(instantanea(doc), antes, "semilla {semilla:?}");
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_01_3x2_uniforme() {
    let doc = doc_matriz(&["AAA", "AAA"]);
    comprueba_region(&doc, (1, 1), &[sp(0, 0, 3), sp(1, 0, 3)]);
}

#[test]
fn fila_02_un_pixel() {
    let doc = doc_matriz(&["A"]);
    comprueba_region(&doc, (0, 0), &[sp(0, 0, 1)]);
}

#[test]
fn fila_03_fila_unica_no_se_parte_en_la_semilla() {
    let doc = doc_matriz(&["AAAAA"]);
    comprueba_region(&doc, (2, 0), &[sp(0, 0, 5)]);
}

#[test]
fn fila_04_ningun_tramo_cruza_de_fila() {
    let doc = doc_matriz(&["AA", "AA", "AA"]);
    comprueba_region(&doc, (1, 0), &[sp(0, 0, 2), sp(1, 0, 2), sp(2, 0, 2)]);
}

#[test]
fn fila_05_las_diagonales_no_cuentan() {
    let doc = doc_matriz(&["ABA", "BAB", "ABA"]);
    comprueba_region(&doc, (1, 1), &[sp(1, 1, 2)]);
    // Las esquinas (`A`) quedan como estaban.
    let mut copia = doc.clone();
    let id = ImageId::from_raw(0);
    assert_eq!(flood_fill(&mut copia, id, 1, 1, vc()), Ok(true));
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        assert_eq!(copia.image(id).unwrap().get_pixel(x, y), Ok(va()));
    }
}

#[test]
fn fila_06_tablero_la_region_es_un_pixel() {
    let filas = ["ABAB", "BABA", "ABAB", "BABA"];
    let doc = doc_matriz(&filas);
    // (1, 1) es `A` (x + y par).
    assert_eq!(
        doc.image(ImageId::from_raw(0)).unwrap().get_pixel(1, 1),
        Ok(va())
    );
    comprueba_region(&doc, (1, 1), &[sp(1, 1, 2)]);
}

#[test]
fn fila_07_columna_con_un_a_aislado() {
    let doc = doc_matriz(&["A", "B", "A", "A"]);
    comprueba_region(&doc, (0, 3), &[sp(2, 0, 1), sp(3, 0, 1)]);
}

#[test]
fn fila_08_se_descubre_en_otro_orden_y_sale_ordenado() {
    let doc = doc_matriz(&["AAA", "ABA", "ABA"]);
    comprueba_region(
        &doc,
        (2, 2),
        &[
            sp(0, 0, 3),
            sp(1, 0, 1),
            sp(1, 2, 3),
            sp(2, 0, 1),
            sp(2, 2, 3),
        ],
    );
}

#[test]
fn fila_09_peine_desde_dos_semillas() {
    let doc = doc_matriz(&["AAAAA", "ABABA", "ABABA"]);
    let esperados = [
        sp(0, 0, 5),
        sp(1, 0, 1),
        sp(1, 2, 3),
        sp(1, 4, 5),
        sp(2, 0, 1),
        sp(2, 2, 3),
        sp(2, 4, 5),
    ];
    comprueba_region(&doc, (4, 2), &esperados);
    comprueba_region(&doc, (0, 0), &esperados);
}

#[test]
fn fila_10_semilla_fuera_del_lienzo() {
    // 3 x 2, con historia y con algo que rehacer: nada se toca.
    let mut doc = doc_matriz(&["AAA", "AAA"]);
    con_rehacer(&mut doc, vb());
    let id = ImageId::from_raw(0);
    for semilla in [(-1, 0), (3, 0), (0, 2), (MIN, MAX)] {
        comprueba_fuera(&mut doc, id, semilla, vc());
    }
    // Otros extremos y esquinas contiguas al lienzo.
    for semilla in [
        (0, -1),
        (MAX, 0),
        (0, MAX),
        (MIN, 0),
        (0, MIN),
        (3, 2),
        (-1, -1),
    ] {
        comprueba_fuera(&mut doc, id, semilla, vc());
    }
    // Y la historia de un documento sin pasos tampoco cambia.
    let mut nuevo = doc_matriz(&["AAA", "AAA"]);
    let id = ImageId::from_raw(0);
    comprueba_fuera(&mut nuevo, id, (-1, 0), vc());
    assert!(!nuevo.can_undo());
}

#[test]
fn fila_11_imagen_desconocida_gana_a_none() {
    let mut doc = doc_matriz(&["AAA", "AAA"]);
    let desconocida = ImageId::from_raw(99);
    let error = DocumentError::UnknownImage(desconocida);
    // Semilla fuera y `C` distinto.
    comprueba_error(&mut doc, desconocida, (-1, 0), vc(), &error);
    // Semilla dentro y `C` igual al valor de la semilla.
    comprueba_error(&mut doc, desconocida, (0, 0), va(), &error);
    // Las dos cosas a la vez: semilla fuera y píxel igual al de la imagen.
    comprueba_error(&mut doc, desconocida, (MIN, MAX), va(), &error);
    // También gana a un píxel de otro modo.
    comprueba_error(&mut doc, desconocida, (0, 0), Pixel::Gray(1), &error);
    comprueba_error(&mut doc, desconocida, (0, 0), Pixel::Indexed(200), &error);
}

#[test]
fn fila_12_pixel_de_otro_modo_o_indice_fuera_de_la_paleta_gana_a_fuera() {
    // Indexed(0) sobre una imagen RGBA, con semilla fuera.
    let mut doc = doc_matriz(&["AAA", "AAA"]);
    let id = ImageId::from_raw(0);
    let error = DocumentError::Pixel {
        pixel: 0,
        source: ImageError::ModeMismatch {
            expected: Mode::Rgba,
            found: Mode::Indexed,
        },
    };
    comprueba_error(&mut doc, id, (-1, 0), Pixel::Indexed(0), &error);
    comprueba_error(&mut doc, id, (MIN, MAX), Pixel::Indexed(0), &error);
    // Con la semilla dentro es el mismo error.
    comprueba_error(&mut doc, id, (0, 0), Pixel::Indexed(0), &error);

    // Indexed(5) con una paleta de 3, con semilla fuera.
    let colores = [
        rgba(255, 0, 0, 255),
        rgba(0, 255, 0, 255),
        rgba(0, 0, 255, 255),
    ];
    let mut indexado = Document::new(Image::new_indexed(3, 2, paleta(&colores)).unwrap());
    let id = ImageId::from_raw(0);
    let error = DocumentError::Pixel {
        pixel: 0,
        source: ImageError::IndexNotInPalette { index: 5, len: 3 },
    };
    comprueba_error(&mut indexado, id, (-1, 0), Pixel::Indexed(5), &error);
    comprueba_error(&mut indexado, id, (0, 2), Pixel::Indexed(5), &error);
    comprueba_error(&mut indexado, id, (0, 0), Pixel::Indexed(5), &error);
    // El último índice válido y el primero inválido.
    assert_eq!(
        flood_fill(&mut indexado, id, 0, 0, Pixel::Indexed(2)),
        Ok(true)
    );
    let error = DocumentError::Pixel {
        pixel: 0,
        source: ImageError::IndexNotInPalette { index: 3, len: 3 },
    };
    comprueba_error(&mut indexado, id, (0, 0), Pixel::Indexed(3), &error);
}

#[test]
fn fila_13_pixel_igual_al_de_la_semilla_es_fuera() {
    let mut doc = doc_matriz(&["AAB", "ABB"]);
    con_rehacer(&mut doc, vc());
    let id = ImageId::from_raw(0);
    let antes = instantanea(&doc);
    assert!(!antes.1 && antes.2);
    // `C` igual al valor de la semilla: `A` en (0, 0) y `B` en (2, 0).
    comprueba_fuera(&mut doc, id, (0, 0), va());
    comprueba_fuera(&mut doc, id, (2, 0), vb());
    assert_eq!(instantanea(&doc), antes);

    // Con una pila de deshacer no vacía.
    let mut con_pasos = doc_matriz(&["AAB", "ABB"]);
    let id = ImageId::from_raw(0);
    assert_eq!(flood_fill(&mut con_pasos, id, 0, 0, vc()), Ok(true));
    let antes = instantanea(&con_pasos);
    assert!(antes.1 && !antes.2 && antes.3 > 0);
    comprueba_fuera(&mut con_pasos, id, (0, 0), vc());
    comprueba_fuera(&mut con_pasos, id, (2, 1), vb());
    assert_eq!(instantanea(&con_pasos), antes);

    // Imagen recién creada: transparente con transparente.
    let mut nueva = Document::new(Image::new(2, 2, Mode::Rgba).unwrap());
    let id = ImageId::from_raw(0);
    comprueba_fuera(&mut nueva, id, (1, 1), Pixel::Rgba(Rgba::TRANSPARENT));
}

fn doc_indexado_0_1_0(colores: &[Rgba]) -> Document {
    let mut img = Image::new_indexed(3, 1, paleta(colores)).unwrap();
    img.set_pixel(1, 0, Pixel::Indexed(1)).unwrap();
    Document::new(img)
}

#[test]
fn fila_14_indices_con_el_mismo_color_son_valores_distintos() {
    let rojo = rgba(255, 0, 0, 255);
    let azul = rgba(0, 0, 255, 255);
    let doc = doc_indexado_0_1_0(&[rojo, rojo, azul]);
    let id = ImageId::from_raw(0);
    for relleno in [Pixel::Indexed(2), Pixel::Indexed(1)] {
        // El índice 1 tiene el mismo color que el 0, pero es otro valor: la región es (0, 0).
        let antes = instantanea(&doc);
        assert_eq!(
            fill_command(&doc, id, 0, 0, relleno),
            comando(id, &[sp(0, 0, 1)], relleno),
            "relleno {relleno:?}"
        );
        assert_eq!(instantanea(&doc), antes);
        // Y es un cambio real: `Ok(true)`, también con `Indexed(1)`.
        let mut copia = doc.clone();
        assert_eq!(flood_fill(&mut copia, id, 0, 0, relleno), Ok(true));
        let img = copia.image(id).unwrap();
        assert_eq!(img.get_pixel(0, 0), Ok(relleno));
        assert_eq!(img.get_pixel(1, 0), Ok(Pixel::Indexed(1)));
        assert_eq!(img.get_pixel(2, 0), Ok(Pixel::Indexed(0)));
    }
    // Desde el índice 1, en medio, la región es solo él.
    assert_eq!(
        fill_command(&doc, id, 1, 0, Pixel::Indexed(2)),
        comando(id, &[sp(0, 1, 2)], Pixel::Indexed(2))
    );
}

#[test]
fn fila_14_indices_con_distinto_alfa_de_paleta_son_valores_distintos() {
    let rojo = rgba(255, 0, 0, 255);
    let rojo_translucido = rgba(255, 0, 0, 128);
    let doc = doc_indexado_0_1_0(&[rojo, rojo_translucido, rgba(0, 0, 255, 255)]);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, Pixel::Indexed(2)),
        comando(id, &[sp(0, 0, 1)], Pixel::Indexed(2))
    );
    let mut copia = doc.clone();
    assert_eq!(
        flood_fill(&mut copia, id, 2, 0, Pixel::Indexed(1)),
        Ok(true)
    );
    let img = copia.image(id).unwrap();
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Indexed(0)));
    assert_eq!(img.get_pixel(2, 0), Ok(Pixel::Indexed(1)));
}

#[test]
fn fila_15_rgb_igual_con_distinto_alfa_son_valores_distintos() {
    let mut img = Image::new(2, 1, Mode::Rgba).unwrap();
    img.set_pixel(0, 0, Pixel::Rgba(rgba(10, 20, 30, 255)))
        .unwrap();
    img.set_pixel(1, 0, Pixel::Rgba(rgba(10, 20, 30, 254)))
        .unwrap();
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, vc()),
        comando(id, &[sp(0, 0, 1)], vc())
    );
    assert_eq!(
        fill_command(&doc, id, 1, 0, vc()),
        comando(id, &[sp(0, 1, 2)], vc())
    );
    // Rellenar el primero con el valor del segundo es un cambio real.
    let igual_que_el_segundo = Pixel::Rgba(rgba(10, 20, 30, 254));
    assert_eq!(
        fill_command(&doc, id, 0, 0, igual_que_el_segundo),
        comando(id, &[sp(0, 0, 1)], igual_que_el_segundo)
    );
    let mut copia = doc.clone();
    assert_eq!(
        flood_fill(&mut copia, id, 0, 0, igual_que_el_segundo),
        Ok(true)
    );
    assert_eq!(
        copia.image(id).unwrap().get_pixel(0, 0),
        Ok(igual_que_el_segundo)
    );
    assert!(copia.can_undo());
}

#[test]
fn fila_15_invisibles_con_distinto_rgb_son_valores_distintos() {
    // Comportamiento 2: (0,0,0,0) y (9,9,9,0) son distintos aunque ambos sean invisibles.
    let mut img = Image::new(2, 1, Mode::Rgba).unwrap();
    img.set_pixel(1, 0, Pixel::Rgba(rgba(9, 9, 9, 0))).unwrap();
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Rgba(Rgba::TRANSPARENT)));
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, vc()),
        comando(id, &[sp(0, 0, 1)], vc())
    );
    assert_eq!(
        fill_command(&doc, id, 1, 0, vc()),
        comando(id, &[sp(0, 1, 2)], vc())
    );
}

#[test]
fn fila_15_gray_alpha_con_igual_gris_o_igual_alfa_son_valores_distintos() {
    let mut img = Image::new(2, 1, Mode::GrayAlpha).unwrap();
    img.set_pixel(0, 0, Pixel::GrayAlpha { gray: 5, alpha: 0 })
        .unwrap();
    img.set_pixel(1, 0, Pixel::GrayAlpha { gray: 6, alpha: 0 })
        .unwrap();
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    let relleno = Pixel::GrayAlpha { gray: 9, alpha: 9 };
    assert_eq!(
        fill_command(&doc, id, 0, 0, relleno),
        comando(id, &[sp(0, 0, 1)], relleno)
    );
    assert_eq!(
        fill_command(&doc, id, 1, 0, relleno),
        comando(id, &[sp(0, 1, 2)], relleno)
    );
    let mut copia = doc.clone();
    assert_eq!(flood_fill(&mut copia, id, 0, 0, relleno), Ok(true));
    let img = copia.image(id).unwrap();
    assert_eq!(img.get_pixel(0, 0), Ok(relleno));
    assert_eq!(
        img.get_pixel(1, 0),
        Ok(Pixel::GrayAlpha { gray: 6, alpha: 0 })
    );

    // Mismo gris con distinto alfa.
    let mut img = Image::new(2, 1, Mode::GrayAlpha).unwrap();
    img.set_pixel(
        0,
        0,
        Pixel::GrayAlpha {
            gray: 5,
            alpha: 255,
        },
    )
    .unwrap();
    img.set_pixel(
        1,
        0,
        Pixel::GrayAlpha {
            gray: 5,
            alpha: 254,
        },
    )
    .unwrap();
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, relleno),
        comando(id, &[sp(0, 0, 1)], relleno)
    );
}

// Fila 16: lienzos grandes.

/// Por encima de este tiempo hay un coste cuadrático: es lo único que hace fallar.
const LIMITE: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug)]
enum Grande {
    /// (a) Todo `A`, semilla en el centro.
    Uniforme,
    /// (b) Tablero (`A` si `x + y` es par), semilla en el centro.
    Tablero,
    /// (c) Peine: fila 0 llena y columnas pares `A` en el resto; semilla `(0, 1)`.
    Peine,
}

impl Grande {
    fn es_a(self, x: u32, y: u32) -> bool {
        match self {
            Grande::Uniforme => true,
            Grande::Tablero => (x + y).is_multiple_of(2),
            Grande::Peine => y == 0 || x.is_multiple_of(2),
        }
    }

    fn semilla(self, l: u32) -> (u32, u32) {
        match self {
            Grande::Uniforme | Grande::Tablero => (l / 2, l / 2),
            Grande::Peine => (0, 1),
        }
    }

    /// Casilla que cambia con el relleno.
    fn en_region(self, l: u32, x: u32, y: u32) -> bool {
        match self {
            Grande::Uniforme | Grande::Peine => self.es_a(x, y),
            Grande::Tablero => (x, y) == self.semilla(l),
        }
    }

    /// Comprueba los tramos de la tabla.
    fn comprueba_tramos(self, l: u32, spans: &[Span]) {
        match self {
            Grande::Uniforme => {
                assert_eq!(spans.len(), ancho_usize(l), "(a): L tramos");
                assert!(
                    spans.iter().copied().eq((0..l).map(|y| sp(y, 0, l))),
                    "(a): un tramo sp(y, 0, L) por fila"
                );
            }
            Grande::Tablero => {
                assert_eq!(spans, [sp(l / 2, l / 2, l / 2 + 1)], "(b): un solo tramo");
            }
            Grande::Peine => {
                let l64 = u64::from(l);
                let esperado = 1 + (l64 - 1) * l64 / 2;
                assert_eq!(u64::try_from(spans.len()).unwrap(), esperado, "(c): S");
                // Unos cuantos tramos concretos.
                assert_eq!(spans[0], sp(0, 0, l));
                assert_eq!(spans[1], sp(1, 0, 1));
                assert_eq!(spans[2], sp(1, 2, 3));
                assert_eq!(spans[ancho_usize(l / 2)], sp(1, l - 2, l - 1));
                assert_eq!(spans[ancho_usize(l / 2) + 1], sp(2, 0, 1));
                assert_eq!(spans[spans.len() - 1], sp(l - 1, l - 2, l - 1));
                // Y toda la secuencia, en el orden de la salida.
                let resto = (1..l).flat_map(|y| (0..l / 2).map(move |k| sp(y, 2 * k, 2 * k + 1)));
                assert!(
                    spans
                        .iter()
                        .copied()
                        .eq(std::iter::once(sp(0, 0, l)).chain(resto)),
                    "(c): la secuencia completa"
                );
            }
        }
    }
}

/// Ejecuta una de las pruebas grandes y escribe sus tiempos.
fn prueba_grande(caso: Grande, l: u32) {
    let inicio = Instant::now();
    let original = imagen_de(l, l, |x, y| if caso.es_a(x, y) { va() } else { vb() });
    let rellena = imagen_de(l, l, |x, y| {
        if caso.en_region(l, x, y) {
            vc()
        } else if caso.es_a(x, y) {
            va()
        } else {
            vb()
        }
    });
    let t_imagenes = inicio.elapsed();

    let mut doc = Document::new(original.clone());
    let id = ImageId::from_raw(0);
    let (sx, sy) = caso.semilla(l);

    let t0 = Instant::now();
    let resultado = fill_command(&doc, id, i(sx), i(sy), vc());
    let t_fill = t0.elapsed();
    let comando = match resultado {
        Ok(Some(c)) => c,
        otro => panic!("{caso:?} L={l}: se esperaba Some(..), salió {otro:?}"),
    };
    let numero_de_tramos = match &comando {
        Command::FillSpans {
            image,
            spans,
            pixel,
        } => {
            assert_eq!(*image, id);
            assert_eq!(*pixel, vc());
            caso.comprueba_tramos(l, spans);
            spans.len()
        }
        otro => panic!("{caso:?} L={l}: se esperaba FillSpans, salió {otro:?}"),
    };
    assert!(
        doc.image(id) == Some(&original),
        "fill_command no debe tocar la imagen"
    );

    let t1 = Instant::now();
    let aplicado = doc.apply(comando);
    let t_apply = t1.elapsed();
    assert_eq!(aplicado, Ok(true), "{caso:?} L={l}: apply");
    assert!(
        doc.image(id) == Some(&rellena),
        "{caso:?} L={l}: la imagen rellena"
    );

    let t2 = Instant::now();
    assert_eq!(doc.undo(), Ok(()));
    let t_undo = t2.elapsed();
    assert!(
        doc.image(id) == Some(&original),
        "{caso:?} L={l}: undo restaura la imagen"
    );

    let t3 = Instant::now();
    assert_eq!(doc.redo(), Ok(()));
    let t_redo = t3.elapsed();
    assert!(
        doc.image(id) == Some(&rellena),
        "{caso:?} L={l}: redo la rellena"
    );

    let total = inicio.elapsed();
    println!(
        "fila 16 {caso:?} L={l}: {numero_de_tramos} tramos; imágenes {t_imagenes:?}, \
         fill_command {t_fill:?}, apply {t_apply:?}, undo {t_undo:?}, redo {t_redo:?}, \
         total {total:?}, history_bytes {}",
        doc.history_bytes()
    );
    assert!(
        t_fill < LIMITE && t_apply < LIMITE && total < LIMITE,
        "{caso:?} L={l}: pasa de {LIMITE:?} (coste cuadrático)"
    );
}

#[test]
fn fila_16a_uniforme_1024() {
    prueba_grande(Grande::Uniforme, 1024);
}

#[test]
fn fila_16b_tablero_1024() {
    prueba_grande(Grande::Tablero, 1024);
}

#[test]
fn fila_16c_peine_1024() {
    prueba_grande(Grande::Peine, 1024);
}

#[test]
fn fila_16a_uniforme_2048() {
    prueba_grande(Grande::Uniforme, 2048);
}

#[test]
fn fila_16b_tablero_2048() {
    prueba_grande(Grande::Tablero, 2048);
}

#[test]
fn fila_16c_peine_2048() {
    prueba_grande(Grande::Peine, 2048);
}

// Oráculo.

/// Recorrido ingenuo pixel a pixel con cola: 4 vecinos y valor exacto (`get_pixel`).
fn region_ingenua(img: &Image, sx: u32, sy: u32) -> BTreeSet<(u32, u32)> {
    let valor = img.get_pixel(sx, sy).unwrap();
    let mut region = BTreeSet::new();
    let mut cola = VecDeque::new();
    region.insert((sx, sy));
    cola.push_back((sx, sy));
    while let Some((x, y)) = cola.pop_front() {
        let (xi, yi) = (i64::from(x), i64::from(y));
        for (nx, ny) in [(xi - 1, yi), (xi + 1, yi), (xi, yi - 1), (xi, yi + 1)] {
            let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) else {
                continue;
            };
            if !region.contains(&(nx, ny)) && img.get_pixel(nx, ny) == Ok(valor) {
                region.insert((nx, ny));
                cola.push_back((nx, ny));
            }
        }
    }
    region
}

/// Los píxeles de los tramos, uno por uno y con repeticiones si las hubiera.
fn pixeles_de(spans: &[Span]) -> Vec<(u32, u32)> {
    spans
        .iter()
        .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x, s.y)))
        .collect()
}

// Estrategias.

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

/// Paleta de seis entradas con casi duplicados: 0 y 1 igual color, 2 mismo RGB con otro alfa.
fn paleta_seis() -> Palette {
    paleta(&[
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 128),
        rgba(0, 0, 200, 255),
        rgba(0, 200, 0, 255),
        rgba(5, 5, 5, 0),
    ])
}

fn imagen_vacia(mode: Mode, width: u32, height: u32) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, paleta_seis()).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Valores posibles del fondo, con casi duplicados (hay al menos cuatro por modo).
fn pool_fondo(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(10, 20, 30, 254)),
            Pixel::Rgba(rgba(10, 20, 30, 0)),
            Pixel::Rgba(rgba(9, 9, 9, 0)),
            Pixel::Rgba(rgba(40, 50, 60, 255)),
        ],
        Mode::Indexed => (0..6).map(Pixel::Indexed).collect(),
        Mode::Gray => vec![
            Pixel::Gray(5),
            Pixel::Gray(6),
            Pixel::Gray(7),
            Pixel::Gray(8),
        ],
        Mode::GrayAlpha => vec![
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 255,
            },
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 254,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 6, alpha: 0 },
            Pixel::GrayAlpha {
                gray: 6,
                alpha: 255,
            },
        ],
    }
}

/// Valores de relleno válidos: los del fondo (hay no-ops y cambios a un casi duplicado) y
/// alguno que no está en el fondo.
fn pool_relleno(mode: Mode) -> Vec<Pixel> {
    let mut v = pool_fondo(mode);
    match mode {
        Mode::Rgba => {
            v.push(Pixel::Rgba(rgba(1, 2, 3, 4)));
            v.push(Pixel::Rgba(Rgba::TRANSPARENT));
        }
        Mode::Indexed => {}
        Mode::Gray => v.push(Pixel::Gray(99)),
        Mode::GrayAlpha => v.push(Pixel::GrayAlpha { gray: 99, alpha: 1 }),
    }
    v
}

/// Imagen pequeña con un fondo de 2 a 4 valores, una semilla dentro y un relleno válido.
#[derive(Clone, Debug)]
struct Caso {
    image: Image,
    seed: (u32, u32),
    pixel: Pixel,
}

impl Caso {
    fn documento(&self) -> (Document, ImageId) {
        let doc = Document::new(self.image.clone());
        let id = ImageId::from_raw(0);
        (doc, id)
    }

    fn semilla_i32(&self) -> (i32, i32) {
        (i(self.seed.0), i(self.seed.1))
    }

    fn valor(&self) -> Pixel {
        self.image.get_pixel(self.seed.0, self.seed.1).unwrap()
    }
}

fn casos() -> impl Strategy<Value = Caso> {
    (modo(), 1u32..=12, 1u32..=12)
        .prop_flat_map(|(mode, w, h)| {
            let celdas = ancho_usize(w * h);
            (
                Just((mode, w, h)),
                prop::sample::subsequence(pool_fondo(mode), 2..=4),
                prop::collection::vec(any::<u8>(), celdas),
                0..w,
                0..h,
                prop::sample::select(pool_relleno(mode)),
            )
        })
        .prop_map(|((mode, w, h), fondo, celdas, sx, sy, pixel)| {
            let mut image = imagen_vacia(mode, w, h);
            for y in 0..h {
                for x in 0..w {
                    let byte = celdas[ancho_usize(y * w + x)];
                    // La mitad de las casillas valen el primer valor: ganan las regiones grandes.
                    let k = if byte < 128 {
                        0
                    } else {
                        usize::from(byte) % fondo.len()
                    };
                    image.set_pixel(x, y, fondo[k]).unwrap();
                }
            }
            Caso {
                image,
                seed: (sx, sy),
                pixel,
            }
        })
}

/// Píxel de cualquier modo y con cualquier índice (a menudo fuera de la paleta de seis).
fn pixel_cualquiera() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(rgba(r, g, b, a))),
        prop_oneof![3 => 0u8..8, 1 => any::<u8>()].prop_map(Pixel::Indexed),
        any::<u8>().prop_map(Pixel::Gray),
        (any::<u8>(), any::<u8>()).prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha }),
    ]
}

/// Cualquier `i32`, con los extremos y las cercanías de un lienzo pequeño siempre presentes.
fn coordenada() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => -2i32..=13,
        2 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
    ]
}

// Apoyo de las propiedades.

/// Los tramos de un `Ok(Some(FillSpans { .. }))` sobre `id` con `pixel`; `None` si no hay
/// comando. Cualquier otra cosa es un fallo.
fn tramos_de(
    resultado: Result<Option<Command>, DocumentError>,
    id: ImageId,
    pixel: Pixel,
) -> Result<Option<Vec<Span>>, TestCaseError> {
    match resultado {
        Ok(Some(Command::FillSpans {
            image,
            spans,
            pixel: escrito,
        })) => {
            prop_assert_eq!(image, id);
            prop_assert_eq!(escrito, pixel);
            Ok(Some(spans))
        }
        Ok(None) => Ok(None),
        otro => Err(TestCaseError::fail(format!(
            "se esperaba Ok(None) u Ok(Some(FillSpans)), salió {otro:?}"
        ))),
    }
}

/// Imagen original con `region` escrita con `pixel`, solo con `Image::set_pixel`.
fn con_region_pintada(original: &Image, region: &BTreeSet<(u32, u32)>, pixel: Pixel) -> Image {
    let mut img = original.clone();
    for &(x, y) in region {
        img.set_pixel(x, y, pixel).unwrap();
    }
    img
}

/// Error que daría `Document::apply(SetPixels)` con `pixel` sobre un documento del mismo modo
/// y con la misma paleta que `img` (`None` si el píxel es válido).
fn error_de_apply(img: &Image, pixel: Pixel) -> Option<DocumentError> {
    let referencia = match img.palette() {
        Some(p) => Image::new_indexed(1, 1, p.clone()).unwrap(),
        None => Image::new(1, 1, img.mode()).unwrap(),
    };
    let mut doc = Document::new(referencia);
    let id = ImageId::from_raw(0);
    let paso = Command::SetPixels {
        image: id,
        pixels: vec![edicion(0, 0, pixel)],
    };
    doc.apply(paso).err()
}

/// Un paso previo que cambia `(0, 0)` por otro valor (si lo hay).
fn paso_previo(doc: &mut Document) {
    let id = ImageId::from_raw(0);
    let imagen = doc.image(id).unwrap();
    let actual = imagen.get_pixel(0, 0).unwrap();
    let otro = pool_relleno(imagen.mode())
        .into_iter()
        .find(|&p| p != actual);
    if let Some(otro) = otro {
        let paso = Command::SetPixels {
            image: id,
            pixels: vec![edicion(0, 0, otro)],
        };
        assert_eq!(doc.apply(paso), Ok(true));
    }
}

/// Forma de los tramos (propiedad 2).
fn comprueba_forma(img: &Image, valor: Pixel, spans: &[Span]) -> Result<(), TestCaseError> {
    let (w, h) = (img.width(), img.height());
    let tope = ancho_usize(h) * ancho_usize(w.div_ceil(2));
    prop_assert!(spans.len() <= tope, "{} tramos > {}", spans.len(), tope);
    for s in spans {
        prop_assert!(s.x_start < s.x_end, "tramo vacío {:?}", s);
        prop_assert!(s.y < h && s.x_end <= w, "{:?} fuera de {}x{}", s, w, h);
        // Máximo: el píxel de la izquierda y el de la derecha, si existen, no valen lo de la semilla.
        if s.x_start > 0 {
            prop_assert_ne!(img.get_pixel(s.x_start - 1, s.y).unwrap(), valor, "{:?}", s);
        }
        if s.x_end < w {
            prop_assert_ne!(img.get_pixel(s.x_end, s.y).unwrap(), valor, "{:?}", s);
        }
        for x in s.x_start..s.x_end {
            prop_assert_eq!(img.get_pixel(x, s.y).unwrap(), valor, "({}, {})", x, s.y);
        }
    }
    for par in spans.windows(2) {
        let (a, b) = (par[0], par[1]);
        prop_assert!(
            (a.y, a.x_start) < (b.y, b.x_start),
            "no están estrictamente ordenados: {:?} y {:?}",
            a,
            b
        );
        if a.y == b.y {
            // Sin solapes y fusionados: queda al menos un píxel distinto entre los dos.
            prop_assert!(
                a.x_end < b.x_start,
                "{:?} y {:?} se tocan o se solapan",
                a,
                b
            );
        }
    }
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: oráculo.
    #[test]
    fn region_igual_al_oraculo(caso in casos()) {
        let (mut doc, id) = caso.documento();
        let (sx, sy) = caso.seed;
        let (x, y) = caso.semilla_i32();
        let valor = caso.valor();
        let region = region_ingenua(&caso.image, sx, sy);

        let resultado = fill_command(&doc, id, x, y, caso.pixel);
        let cambia = caso.pixel != valor;
        match tramos_de(resultado, id, caso.pixel)? {
            None => prop_assert!(!cambia, "Ok(None) con un píxel distinto del de la semilla"),
            Some(spans) => {
                prop_assert!(cambia, "Some(..) con un píxel igual al de la semilla");
                let lista = pixeles_de(&spans);
                let conjunto: BTreeSet<(u32, u32)> = lista.iter().copied().collect();
                prop_assert_eq!(lista.len(), conjunto.len(), "píxeles repetidos en los tramos");
                prop_assert_eq!(&conjunto, &region);
            }
        }

        // Tras `flood_fill`: la original con la región en `pixel` y nada más.
        prop_assert_eq!(flood_fill(&mut doc, id, x, y, caso.pixel), Ok(cambia));
        let esperada = if cambia {
            con_region_pintada(&caso.image, &region, caso.pixel)
        } else {
            caso.image.clone()
        };
        prop_assert_eq!(doc.image(id), Some(&esperada));
    }

    // Propiedad 2: forma de los tramos.
    #[test]
    fn forma_de_los_tramos(caso in casos()) {
        let (doc, id) = caso.documento();
        let (x, y) = caso.semilla_i32();
        let valor = caso.valor();
        match tramos_de(fill_command(&doc, id, x, y, caso.pixel), id, caso.pixel)? {
            None => prop_assert_eq!(caso.pixel, valor),
            Some(spans) => {
                prop_assert!(!spans.is_empty());
                comprueba_forma(&caso.image, valor, &spans)?;
            }
        }
    }

    // Propiedad 3: historia de un solo paso.
    #[test]
    fn un_solo_paso_de_historia(caso in casos(), previa in any::<bool>()) {
        let (mut doc, id) = caso.documento();
        if previa {
            paso_previo(&mut doc);
        }
        let (x, y) = caso.semilla_i32();
        let antes = instantanea(&doc);
        let imagen_antes = doc.image(id).unwrap().clone();
        // El paso previo puede haber cambiado la semilla (solo si es (0, 0)).
        let valor_ahora = imagen_antes.get_pixel(caso.seed.0, caso.seed.1).unwrap();

        // `fill_command` no cambia el documento.
        let c = fill_command(&doc, id, x, y, caso.pixel);
        prop_assert!(c.is_ok(), "fill_command dio {:?}", c);
        prop_assert_eq!(instantanea(&doc), antes.clone());

        let cambia = caso.pixel != valor_ahora;
        prop_assert_eq!(flood_fill(&mut doc, id, x, y, caso.pixel), Ok(cambia));
        if !cambia {
            prop_assert_eq!(instantanea(&doc), antes);
            return Ok(());
        }
        let rellena = doc.image(id).unwrap().clone();
        prop_assert!(doc.can_undo());
        prop_assert!(!doc.can_redo());
        prop_assert!(doc.history_bytes() > antes.3);

        // Un solo paso: un `undo` deja la imagen y la historia como estaban.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(doc.image(id), Some(&imagen_antes));
        prop_assert_eq!(doc.can_undo(), antes.1);
        prop_assert!(doc.can_redo());
        prop_assert_eq!(doc.history_bytes(), antes.3);

        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(doc.image(id), Some(&rellena));
        prop_assert!(doc.can_undo());
        prop_assert!(!doc.can_redo());
    }

    // Propiedad 4: repetición y no-ops.
    #[test]
    fn repetir_y_no_ops(caso in casos()) {
        let (mut doc, id) = caso.documento();
        let (x, y) = caso.semilla_i32();
        let valor = caso.valor();
        let cambia = caso.pixel != valor;

        prop_assert_eq!(flood_fill(&mut doc, id, x, y, caso.pixel), Ok(cambia));
        // La segunda vez la semilla ya vale `pixel`: no hay nada que cambiar.
        let despues = instantanea(&doc);
        prop_assert_eq!(flood_fill(&mut doc, id, x, y, caso.pixel), Ok(false));
        prop_assert_eq!(fill_command(&doc, id, x, y, caso.pixel), Ok(None));
        prop_assert_eq!(instantanea(&doc), despues);

        if cambia {
            // Tras un `undo`, un relleno que da `Ok(false)` no borra la pila de rehacer.
            prop_assert_eq!(doc.undo(), Ok(()));
            prop_assert!(doc.can_redo());
            let tras_undo = instantanea(&doc);
            // Mismo valor que la semilla (el original): no cambia nada.
            prop_assert_eq!(flood_fill(&mut doc, id, x, y, valor), Ok(false));
            prop_assert!(doc.can_redo());
            prop_assert_eq!(instantanea(&doc), tras_undo.clone());
            // Semilla fuera del lienzo.
            prop_assert_eq!(flood_fill(&mut doc, id, -1, y, caso.pixel), Ok(false));
            prop_assert_eq!(flood_fill(&mut doc, id, x, MAX, caso.pixel), Ok(false));
            prop_assert!(doc.can_redo());
            prop_assert_eq!(instantanea(&doc), tras_undo);
        }
    }

    // Propiedad 5: errores y semillas arbitrarias.
    #[test]
    fn errores_y_semillas_arbitrarias(
        caso in casos(),
        semilla_del_caso in any::<bool>(),
        (xr, yr) in (coordenada(), coordenada()),
        otro_pixel in pixel_cualquiera(),
        usa_otro_pixel in any::<bool>(),
        ajena in any::<bool>(),
        raw in any::<u32>(),
    ) {
        let (mut doc, principal) = caso.documento();
        let (x, y) = if semilla_del_caso { caso.semilla_i32() } else { (xr, yr) };
        let pixel = if usa_otro_pixel { otro_pixel } else { caso.pixel };
        let id = if ajena { ImageId::from_raw(raw) } else { principal };
        let (w, h) = (i64::from(caso.image.width()), i64::from(caso.image.height()));
        let fuera = x < 0 || y < 0 || i64::from(x) >= w || i64::from(y) >= h;

        let antes = instantanea(&doc);
        // La imagen tal como estaba, antes de rellenar.
        let imagen_previa = doc.image(id).cloned();
        let c = fill_command(&doc, id, x, y, pixel);
        prop_assert_eq!(instantanea(&doc), antes.clone());
        let f = flood_fill(&mut doc, id, x, y, pixel);

        // `UnknownImage` > `Pixel` > fuera > igual.
        let Some(img) = imagen_previa else {
            let e = DocumentError::UnknownImage(id);
            prop_assert_eq!(c, Err(e.clone()));
            prop_assert_eq!(f, Err(e));
            prop_assert_eq!(instantanea(&doc), antes);
            return Ok(());
        };
        if let Some(e) = error_de_apply(&img, pixel) {
            prop_assert!(
                matches!(&e, DocumentError::Pixel { pixel: 0, .. }),
                "apply debería dar Pixel {{ pixel: 0, .. }}, dio {:?}", e
            );
            prop_assert_eq!(c, Err(e.clone()));
            prop_assert_eq!(f, Err(e));
            prop_assert_eq!(instantanea(&doc), antes);
            return Ok(());
        }
        if fuera {
            prop_assert_eq!(c, Ok(None));
            prop_assert_eq!(f, Ok(false));
            prop_assert_eq!(instantanea(&doc), antes);
            return Ok(());
        }
        let (ux, uy) = (u32::try_from(x).unwrap(), u32::try_from(y).unwrap());
        if img.get_pixel(ux, uy).unwrap() == pixel {
            prop_assert_eq!(c, Ok(None));
            prop_assert_eq!(f, Ok(false));
            prop_assert_eq!(instantanea(&doc), antes);
            return Ok(());
        }
        prop_assert!(
            matches!(c, Ok(Some(Command::FillSpans { .. }))),
            "se esperaba Some(FillSpans), salió {:?}", c
        );
        prop_assert_eq!(f, Ok(true));
        prop_assert_eq!(doc.image(id).unwrap().get_pixel(ux, uy), Ok(pixel));
    }

    // Propiedad 6: independencia de la semilla.
    #[test]
    fn independiente_de_la_semilla(caso in casos()) {
        let (doc, id) = caso.documento();
        let (x, y) = caso.semilla_i32();
        let base = fill_command(&doc, id, x, y, caso.pixel);
        let region = region_ingenua(&caso.image, caso.seed.0, caso.seed.1);
        for &(px, py) in &region {
            let otro = fill_command(&doc, id, i(px), i(py), caso.pixel);
            prop_assert_eq!(&otro, &base, "semilla ({}, {})", px, py);
        }
    }
}

// Unidad 14c: límites del relleno (specs/hito-2-limites-relleno.md).
//
// Filas 1 a 6 y 15 y propiedades 1 a 3 de la spec. Las filas 7 a 10 están en `px-core`, la 11 en
// `px-ui`, la 13 en `px-ui` (`tests/status.rs`) y la 12 y la 14 en `tests/fill_memory.rs` (que
// son `#[ignore]`). Las filas 4, 5 y 15 usan peines de 16384 x 368 (24 MiB en RGBA) y cuestan
// unos segundos en debug; el resto son imágenes de 12 de lado o menos.

mod limites {
    use super::*;
    use px_raster::fill_command_with_limit;

    /// Valor de la spec (D1). La única fuente es `Document::MAX_FILL_SPANS`; aquí se repite el
    /// número a propósito para que cambiarlo haga fallar la prueba.
    const TOPE: usize = 3_000_000;

    fn demasiado(limit: usize) -> DocumentError {
        DocumentError::FillTooFragmented { limit }
    }

    /// Peine de `w` x `h` RGBA: fila 0 toda `A` y, en el resto, `A` en las columnas pares y `B`
    /// en las impares. Con `dientes = Some(k)`, la última fila solo tiene `A` en las columnas
    /// pares `0, 2, .., 2(k - 1)`, y el resto `B`. Se construye con `Image::from_bytes`.
    fn peine(w: u32, h: u32, dientes: Option<u32>) -> Image {
        let (wu, hu) = (ancho_usize(w), ancho_usize(h));
        let a = [10u8, 20, 30, 255];
        let b = [40u8, 50, 60, 255];
        let mut data = Vec::with_capacity(wu * hu * 4);
        for y in 0..hu {
            for x in 0..wu {
                let es_a = match dientes {
                    _ if y == 0 => true,
                    Some(k) if y == hu - 1 => x % 2 == 0 && x / 2 < ancho_usize(k),
                    _ => x % 2 == 0,
                };
                data.extend_from_slice(if es_a { &a } else { &b });
            }
        }
        let img = Image::from_bytes(w, h, Mode::Rgba, data).unwrap();
        assert_eq!(
            img.get_pixel(0, 0),
            Ok(va()),
            "el orden de los bytes es RGBA"
        );
        assert_eq!(img.get_pixel(1, 0), Ok(va()));
        img
    }

    fn doc_peine(w: u32, h: u32, dientes: Option<u32>) -> (Document, ImageId) {
        let doc = Document::new(peine(w, h, dientes));
        let id = ImageId::from_raw(0);
        (doc, id)
    }

    fn tramos_del(resultado: Result<Option<Command>, DocumentError>) -> Vec<Span> {
        match resultado {
            Ok(Some(Command::FillSpans { spans, .. })) => spans,
            otro => panic!("se esperaba Ok(Some(FillSpans)), salió {otro:?}"),
        }
    }

    /// `apply` del comando da `Ok(true)` y el paso cabe en el límite por defecto de la historia.
    /// Si `ciclo`, `undo` restaura la imagen original y `redo` la rellena otra vez.
    fn aplica_y_comprueba(doc: &mut Document, id: ImageId, comando: Command, ciclo: bool) {
        let original = doc.image(id).unwrap().clone();
        assert_eq!(doc.apply(comando), Ok(true));
        assert!(
            doc.history_bytes() <= Document::DEFAULT_HISTORY_LIMIT,
            "el paso ocupa {} bytes y el límite por defecto es {}",
            doc.history_bytes(),
            Document::DEFAULT_HISTORY_LIMIT
        );
        if ciclo {
            let rellena = doc.image(id).unwrap().clone();
            assert!(rellena != original, "el relleno cambia la imagen");
            assert_eq!(rellena.get_pixel(0, 0), Ok(vc()));
            assert_eq!(rellena.get_pixel(1, 0), Ok(vc()));
            assert_eq!(rellena.get_pixel(1, 1), Ok(vb()), "la pared no cambia");
            assert_eq!(doc.undo(), Ok(()));
            assert!(doc.image(id) == Some(&original), "undo restaura la imagen");
            assert_eq!(doc.redo(), Ok(()));
            assert!(doc.image(id) == Some(&rellena), "redo la rellena");
        }
    }

    // Fila 1.

    #[test]
    fn fila_01_peine_5x3_justo_en_el_tope_y_uno_por_debajo() {
        let doc = doc_matriz(&["AAAAA", "ABABA", "ABABA"]);
        let id = ImageId::from_raw(0);
        let esperados = [
            sp(0, 0, 5),
            sp(1, 0, 1),
            sp(1, 2, 3),
            sp(1, 4, 5),
            sp(2, 0, 1),
            sp(2, 2, 3),
            sp(2, 4, 5),
        ];
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, vc(), 7),
            comando(id, &esperados, vc()),
            "7 tramos con tope 7"
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 4, 2, vc(), 7),
            comando(id, &esperados, vc()),
            "otra semilla, mismo resultado"
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, vc(), 6),
            Err(demasiado(6))
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 4, 2, vc(), 6),
            Err(demasiado(6))
        );
    }

    // Fila 2.

    #[test]
    fn fila_02_tope_cero_da_error_y_el_tablero_cabe_en_uno() {
        let doc = doc_matriz(&["AAA"]);
        let id = ImageId::from_raw(0);
        assert_eq!(
            fill_command_with_limit(&doc, id, 1, 0, vc(), 0),
            Err(demasiado(0))
        );
        // Con tope 1 el mismo relleno (1 tramo) sí cabe.
        assert_eq!(
            fill_command_with_limit(&doc, id, 1, 0, vc(), 1),
            comando(id, &[sp(0, 0, 3)], vc())
        );

        // Tablero de 64 x 64 (`A` si x + y es par), semilla en el centro: la región es un píxel.
        let tablero = Document::new(imagen_de(64, 64, |x, y| {
            if (x + y) % 2 == 0 { va() } else { vb() }
        }));
        let id = ImageId::from_raw(0);
        assert_eq!(
            fill_command_with_limit(&tablero, id, 32, 32, vc(), 1),
            comando(id, &[sp(32, 32, 33)], vc())
        );
    }

    // Fila 3.

    #[test]
    fn fila_03_con_tope_cero_el_tope_se_comprueba_el_ultimo() {
        let doc = doc_matriz(&["AAA", "AAA"]);
        let id = ImageId::from_raw(0);
        let antes = instantanea(&doc);

        let desconocida = ImageId::from_raw(99);
        assert_eq!(
            fill_command_with_limit(&doc, desconocida, 0, 0, vc(), 0),
            Err(DocumentError::UnknownImage(desconocida))
        );
        let resultado = fill_command_with_limit(&doc, id, 0, 0, Pixel::Indexed(0), 0);
        assert!(
            matches!(resultado, Err(DocumentError::Pixel { pixel: 0, .. })),
            "salió {resultado:?}"
        );
        assert_eq!(fill_command_with_limit(&doc, id, -1, 0, vc(), 0), Ok(None));
        assert_eq!(
            fill_command_with_limit(&doc, id, MIN, MAX, vc(), 0),
            Ok(None)
        );
        assert_eq!(fill_command_with_limit(&doc, id, 0, 0, va(), 0), Ok(None));
        // Y el tope, si hay algo que rellenar.
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, vc(), 0),
            Err(demasiado(0))
        );
        assert_eq!(
            instantanea(&doc),
            antes,
            "ninguna llamada toca el documento"
        );
    }

    // Fila 4.

    #[test]
    fn fila_04_flood_fill_con_el_tope_real_no_toca_documento_ni_historia() {
        let (mut doc, id) = doc_peine(16384, 368, None);
        // Un paso de deshacer y uno de rehacer, sobre paredes (columnas impares): no cambian
        // los tramos del peine.
        for x in [1, 3] {
            let paso = Command::SetPixels {
                image: id,
                pixels: vec![edicion(x, 5, vc())],
            };
            assert_eq!(doc.apply(paso), Ok(true));
        }
        assert_eq!(doc.undo(), Ok(()));
        assert!(doc.can_undo() && doc.can_redo());

        let imagen = doc.image(id).unwrap().clone();
        let (undo, redo, bytes, estado) = (
            doc.can_undo(),
            doc.can_redo(),
            doc.history_bytes(),
            doc.state_id(),
        );

        let resultado = flood_fill(&mut doc, id, 0, 0, vc());
        assert!(
            matches!(
                resultado,
                Err(DocumentError::FillTooFragmented {
                    limit: Document::MAX_FILL_SPANS
                })
            ),
            "salió {resultado:?}"
        );
        assert!(doc.image(id) == Some(&imagen), "la imagen no cambia");
        assert_eq!(doc.can_undo(), undo);
        assert_eq!(doc.can_redo(), redo, "la pila de rehacer sigue viva");
        assert_eq!(doc.history_bytes(), bytes);
        assert_eq!(doc.state_id(), estado, "no se llamó a apply");
    }

    // Fila 5.

    #[test]
    fn fila_05_peine_de_368_filas_supera_el_tope_real() {
        // S = 1 + 367 * 8192 = 3 006 465.
        let (doc, id) = doc_peine(16384, 368, None);
        assert_eq!(
            fill_command(&doc, id, 0, 0, vc()),
            Err(demasiado(TOPE)),
            "el error lleva el tope real"
        );
    }

    #[test]
    fn fila_05_peine_de_367_filas_cabe_y_su_paso_cabe_en_la_historia() {
        // S = 1 + 366 * 8192 = 2 998 273.
        let (mut doc, id) = doc_peine(16384, 367, None);
        let comando = fill_command(&doc, id, 0, 0, vc());
        let n = tramos_del(comando.clone()).len();
        assert_eq!(n, 2_998_273);
        let comando = comando.unwrap().unwrap();
        aplica_y_comprueba(&mut doc, id, comando, true);
    }

    // Fila 6 (la mitad de px-raster: la constante; el coste real del paso máximo va en px-core).

    #[test]
    fn fila_06_la_constante_del_tope_es_tres_millones() {
        assert_eq!(Document::MAX_FILL_SPANS, 3_000_000);
        assert_eq!(Document::MAX_FILL_SPANS, TOPE);
    }

    // Fila 15: justo por debajo, en y por encima del tope real. Peine de 16384 x 368 cuya última
    // fila solo tiene `k` dientes: S = 2 998 273 + k.

    #[test]
    fn fila_15_k_1726_un_tramo_por_debajo_del_tope() {
        let (mut doc, id) = doc_peine(16384, 368, Some(1726));
        let comando = fill_command(&doc, id, 0, 0, vc());
        assert_eq!(tramos_del(comando.clone()).len(), 2_999_999);
        aplica_y_comprueba(&mut doc, id, comando.unwrap().unwrap(), false);
    }

    #[test]
    fn fila_15_k_1727_justo_en_el_tope() {
        let (mut doc, id) = doc_peine(16384, 368, Some(1727));
        let comando = fill_command(&doc, id, 0, 0, vc());
        assert_eq!(tramos_del(comando.clone()).len(), 3_000_000);
        aplica_y_comprueba(&mut doc, id, comando.unwrap().unwrap(), false);
    }

    #[test]
    fn fila_15_k_1728_un_tramo_por_encima_del_tope() {
        let (doc, id) = doc_peine(16384, 368, Some(1728));
        assert_eq!(
            fill_command(&doc, id, 0, 0, vc()),
            Err(demasiado(TOPE)),
            "S = 3 000 001"
        );
    }

    // Apoyo de las propiedades.

    /// Número de tramos máximos (por fila) de una región.
    fn tramos_maximos(region: &BTreeSet<(u32, u32)>) -> usize {
        region
            .iter()
            .filter(|&&(x, y)| x == 0 || !region.contains(&(x - 1, y)))
            .count()
    }

    /// Topes pequeños con los extremos siempre presentes.
    fn tope_cualquiera() -> impl Strategy<Value = usize> {
        prop_oneof![
            6 => 0usize..=40,
            1 => Just(0usize),
            1 => Just(usize::MAX),
            1 => any::<usize>(),
        ]
    }

    proptest! {
        // Propiedad 1: oráculo del tope.
        #[test]
        fn tope_igual_al_oraculo(caso in casos(), l in 0usize..=40) {
            let (doc, id) = caso.documento();
            let (x, y) = caso.semilla_i32();
            let valor = caso.valor();
            let region = region_ingenua(&caso.image, caso.seed.0, caso.seed.1);
            let s = tramos_maximos(&region);

            let con_tope = fill_command_with_limit(&doc, id, x, y, caso.pixel, l);
            let sin_tope = fill_command(&doc, id, x, y, caso.pixel);
            if caso.pixel == valor {
                prop_assert_eq!(con_tope, Ok(None));
            } else if s > l {
                prop_assert_eq!(con_tope, Err(DocumentError::FillTooFragmented { limit: l }));
            } else {
                // Los mismos tramos que sin tope, y exactamente `s`.
                prop_assert_eq!(&con_tope, &sin_tope);
                let Ok(Some(Command::FillSpans { spans, .. })) = con_tope else {
                    return Err(TestCaseError::fail(format!("salió {con_tope:?}")));
                };
                prop_assert_eq!(spans.len(), s);
                prop_assert!(spans.len() <= l);
                prop_assert_eq!(spans.capacity(), spans.len());
            }
        }

        // Propiedad 2: atomicidad y orden de las comprobaciones.
        #[test]
        fn tope_atomicidad_y_orden(
            caso in casos(),
            l in tope_cualquiera(),
            semilla_del_caso in any::<bool>(),
            (xr, yr) in (coordenada(), coordenada()),
            otro_pixel in pixel_cualquiera(),
            usa_otro_pixel in any::<bool>(),
            ajena in any::<bool>(),
            raw in any::<u32>(),
        ) {
            let (mut doc, principal) = caso.documento();
            paso_previo(&mut doc);
            let (x, y) = if semilla_del_caso { caso.semilla_i32() } else { (xr, yr) };
            let pixel = if usa_otro_pixel { otro_pixel } else { caso.pixel };
            let id = if ajena { ImageId::from_raw(raw) } else { principal };
            let (w, h) = (i64::from(caso.image.width()), i64::from(caso.image.height()));
            let fuera = x < 0 || y < 0 || i64::from(x) >= w || i64::from(y) >= h;

            let antes = instantanea(&doc);
            let imagen_previa = doc.image(id).cloned();
            let c = fill_command_with_limit(&doc, id, x, y, pixel, l);
            // Con cualquier resultado, `fill_command_with_limit` solo lee.
            prop_assert_eq!(instantanea(&doc), antes.clone());

            // UnknownImage > Pixel > fuera > igual > tope.
            let Some(img) = imagen_previa else {
                prop_assert_eq!(c, Err(DocumentError::UnknownImage(id)));
                return Ok(());
            };
            if let Some(e) = error_de_apply(&img, pixel) {
                prop_assert!(
                    matches!(&e, DocumentError::Pixel { pixel: 0, .. }),
                    "apply debería dar Pixel {{ pixel: 0, .. }}, dio {:?}", e
                );
                prop_assert_eq!(c, Err(e));
                return Ok(());
            }
            if fuera {
                prop_assert_eq!(c, Ok(None));
                return Ok(());
            }
            let (ux, uy) = (u32::try_from(x).unwrap(), u32::try_from(y).unwrap());
            if img.get_pixel(ux, uy).unwrap() == pixel {
                prop_assert_eq!(c, Ok(None));
                return Ok(());
            }
            let s = tramos_maximos(&region_ingenua(&img, ux, uy));
            if s > l {
                prop_assert_eq!(c, Err(DocumentError::FillTooFragmented { limit: l }));
            } else {
                prop_assert!(
                    matches!(c, Ok(Some(Command::FillSpans { .. }))),
                    "se esperaba Some(FillSpans), salió {:?}", c
                );
            }
        }

        // Propiedad 3: monotonía del tope.
        #[test]
        fn tope_monotono(caso in casos(), a in tope_cualquiera(), b in tope_cualquiera()) {
            let (doc, id) = caso.documento();
            let (x, y) = caso.semilla_i32();
            let (menor, mayor) = (a.min(b), a.max(b));
            let con_menor = fill_command_with_limit(&doc, id, x, y, caso.pixel, menor);
            let con_mayor = fill_command_with_limit(&doc, id, x, y, caso.pixel, mayor);
            if con_menor.is_ok() {
                // Ok(x) con L implica Ok(x) con cualquier L' >= L.
                prop_assert_eq!(&con_mayor, &con_menor);
            }
            if con_mayor.is_err() {
                // Err con L implica Err con cualquier L' < L, y cada uno con su tope.
                prop_assert_eq!(
                    con_menor,
                    Err(DocumentError::FillTooFragmented { limit: menor })
                );
                prop_assert_eq!(
                    con_mayor,
                    Err(DocumentError::FillTooFragmented { limit: mayor })
                );
            }
        }
    }
}
