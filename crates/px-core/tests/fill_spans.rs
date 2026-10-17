//! Tests de `FillSpans` y de la historia por regiones (specs/hito-2-historia-regiones.md).
//!
//! Una prueba unitaria por cada fila de «Casos límite» (`fila_NN_...`) y una propiedad por
//! cada una de las seis de «Tests». Las imágenes miden como mucho 6 x 6 (salvo las dos de
//! la fila 14, de 4000 x 3 como mucho). El coste de la historia solo se mide con
//! `history_bytes()` y con comparaciones relativas: nunca se asume el valor de `size_of`.

use std::error::Error;
use std::fmt::{Debug, Display};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba, Span,
};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_span_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}
fn assert_dato_traits<T: Clone + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}

// Colores, imágenes, documentos y comandos de apoyo.

const TODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Valor inicial (transparente) de la imagen de `doc_d`.
fn c0() -> Rgba {
    Rgba::TRANSPARENT
}

fn c1() -> Rgba {
    Rgba::new(40, 50, 60, 255)
}

fn c2() -> Rgba {
    Rgba::new(70, 80, 90, 255)
}

fn c3() -> Rgba {
    Rgba::new(100, 110, 120, 255)
}

fn paleta_de(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

/// Crea una imagen de cualquier modo; el indexado usa una paleta de `n` entradas.
fn crea(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, paleta_de(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// `D` de la spec: documento de una imagen RGBA 4 x 3 recién creado.
fn doc_d() -> Document {
    Document::new(Image::new(4, 3, Mode::Rgba).unwrap())
}

/// Documento de una imagen indexada de 4 x 3 con paleta de 2 entradas.
fn doc_indexado() -> Document {
    Document::new(Image::new_indexed(4, 3, paleta_de(2)).unwrap())
}

fn px(x: u32, y: u32, c: Rgba) -> PixelEdit {
    PixelEdit {
        x,
        y,
        pixel: Pixel::Rgba(c),
    }
}

/// `S(...)` de la spec.
fn set(image: ImageId, pixels: &[PixelEdit]) -> Command {
    Command::SetPixels {
        image,
        pixels: pixels.to_vec(),
    }
}

/// `sp(y, a, b)` de la spec.
fn sp(y: u32, a: u32, b: u32) -> Span {
    Span {
        y,
        x_start: a,
        x_end: b,
    }
}

/// `FillSpans` con cualquier `Pixel`.
fn fill(image: ImageId, spans: &[Span], pixel: Pixel) -> Command {
    Command::FillSpans {
        image,
        spans: spans.to_vec(),
        pixel,
    }
}

/// `F(spans, c)` de la spec con un color RGBA.
fn fr(image: ImageId, spans: &[Span], c: Rgba) -> Command {
    fill(image, spans, Pixel::Rgba(c))
}

fn lote(comandos: Vec<Command>) -> Command {
    Command::Batch(comandos)
}

fn pixel_en(doc: &Document, x: u32, y: u32) -> Pixel {
    imagen_de(doc).get_pixel(x, y).unwrap()
}

fn imagen_de(doc: &Document) -> &Image {
    doc.image(doc.main_image()).unwrap()
}

fn fuera(x: u32, y: u32, width: u32, height: u32) -> ImageError {
    ImageError::OutOfBounds {
        x,
        y,
        width,
        height,
    }
}

fn fallo_pixel(pixel: usize, source: ImageError) -> DocumentError {
    DocumentError::Pixel { pixel, source }
}

fn fallo_span(span: usize, source: ImageError) -> DocumentError {
    DocumentError::Span { span, source }
}

fn en_lote(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

/// Comprueba cada píxel de la imagen RGBA principal: los de `listados` (si un píxel
/// aparece varias veces, vale el último) y todos los demás valen `resto`.
fn comprueba_imagen(doc: &Document, listados: &[(u32, u32, Rgba)], resto: Rgba) {
    let img = imagen_de(doc);
    for y in 0..img.height() {
        for x in 0..img.width() {
            let esperado = listados
                .iter()
                .rev()
                .find(|&&(lx, ly, _)| lx == x && ly == y)
                .map_or(resto, |&(_, _, c)| c);
            assert_eq!(img.get_pixel(x, y), Ok(Pixel::Rgba(esperado)), "({x}, {y})");
        }
    }
}

/// Cota para que un `undo`/`redo` defectuoso que nunca falla no cuelgue los tests.
const TOPE_PASOS: usize = 100_000;

/// Número de `undo()` que admite el documento (se cuenta en una copia).
fn pasos_deshacibles(doc: &Document) -> usize {
    let mut copia = doc.clone();
    let mut n = 0;
    while n < TOPE_PASOS && copia.undo().is_ok() {
        n += 1;
    }
    n
}

/// Número de `redo()` que admite el documento (se cuenta en una copia).
fn pasos_rehacibles(doc: &Document) -> usize {
    let mut copia = doc.clone();
    let mut n = 0;
    while n < TOPE_PASOS && copia.redo().is_ok() {
        n += 1;
    }
    n
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
#[derive(Debug, PartialEq, Eq)]
struct Estado {
    doc: Document,
    can_undo: bool,
    can_redo: bool,
    bytes: usize,
    limite: usize,
    deshacibles: usize,
    rehacibles: usize,
}

fn estado(doc: &Document) -> Estado {
    Estado {
        doc: doc.clone(),
        can_undo: doc.can_undo(),
        can_redo: doc.can_redo(),
        bytes: doc.history_bytes(),
        limite: doc.history_limit(),
        deshacibles: pasos_deshacibles(doc),
        rehacibles: pasos_rehacibles(doc),
    }
}

/// `apply(cmd)` da exactamente `esperado` y no toca nada observable.
fn rechaza(doc: &mut Document, cmd: Command, esperado: DocumentError) {
    let antes = estado(doc);
    assert_eq!(doc.apply(cmd.clone()), Err(esperado), "{cmd:?}");
    assert_eq!(estado(doc), antes, "{cmd:?}");
}

/// `apply(cmd)` da `Ok(false)` y no toca nada observable.
fn sin_cambios(doc: &mut Document, cmd: Command) {
    let antes = estado(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(false), "{cmd:?}");
    assert_eq!(estado(doc), antes, "{cmd:?}");
}

/// Coste de la historia tras un relleno de una fila entera de `D` (un solo trozo).
fn coste_de_un_relleno(y: u32) -> usize {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(fr(id, &[sp(y, 0, 4)], c1())), Ok(true));
    doc.history_bytes()
}

// Tipos nuevos de la API.

#[test]
fn traits_derivados_de_span_y_comandos() {
    assert_span_traits::<Span>();
    assert_dato_traits::<Command>();
    assert_dato_traits::<Document>();
    assert_error_traits::<DocumentError>();

    // Campos públicos y comparación.
    let t = Span {
        y: 1,
        x_start: 2,
        x_end: 3,
    };
    assert_eq!(t, sp(1, 2, 3));
    assert_ne!(t, sp(1, 2, 4));
    assert_ne!(t, sp(0, 2, 3));
    assert_ne!(t, sp(1, 1, 3));
    let copia = t;
    assert_eq!(copia, t);

    let id = ImageId::from_raw(0);
    let a = fr(id, &[sp(0, 0, 4)], c1());
    assert_eq!(a, a.clone());
    assert_ne!(a, fr(id, &[sp(0, 0, 3)], c1()));
    assert_ne!(a, fr(id, &[sp(0, 0, 4)], c2()));
    assert_ne!(a, fr(ImageId::from_raw(1), &[sp(0, 0, 4)], c1()));
    assert_ne!(a, set(id, &[px(0, 0, c1())]));
}

#[test]
fn errores_nuevos_tienen_mensaje() {
    let nuevos = [
        fallo_span(0, fuera(9, 9, 4, 3)),
        DocumentError::BadSpans { span: 3 },
        en_lote(1, DocumentError::BadSpans { span: 0 }),
        en_lote(1, fallo_span(2, fuera(9, 9, 4, 3))),
    ];
    for e in &nuevos {
        assert!(!e.to_string().is_empty(), "{e:?} sin mensaje");
    }
    assert_ne!(
        DocumentError::BadSpans { span: 0 },
        DocumentError::BadSpans { span: 1 }
    );
    assert_ne!(
        fallo_span(0, fuera(9, 9, 4, 3)),
        fallo_span(1, fuera(9, 9, 4, 3))
    );
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_01_una_fila_entera_con_undo_y_redo() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();

    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    comprueba_imagen(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(pasos_deshacibles(&doc), 1);
    let tras_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    comprueba_imagen(&doc, &[], c0());
    assert!(doc.can_redo());
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, tras_apply);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn fila_02_tramos_en_varias_filas_y_pegados() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    let spans = [sp(0, 1, 3), sp(1, 0, 2), sp(1, 2, 4), sp(2, 3, 4)];

    assert_eq!(doc.apply(fr(id, &spans, c1())), Ok(true));
    comprueba_imagen(
        &doc,
        &[
            (1, 0, c1()),
            (2, 0, c1()),
            (0, 1, c1()),
            (1, 1, c1()),
            (2, 1, c1()),
            (3, 1, c1()),
            (3, 2, c1()),
        ],
        c0(),
    );
    assert_eq!(pasos_deshacibles(&doc), 1);
    let tras_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, tras_apply);
}

#[test]
fn fila_03_el_valor_anterior_distinto_se_restaura() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();

    assert_eq!(doc.apply(set(id, &[px(1, 0, c2())])), Ok(true));
    let con_a = doc.clone();
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    comprueba_imagen(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    assert_eq!(pasos_deshacibles(&doc), 2);
    let tras_relleno = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    comprueba_imagen(&doc, &[(1, 0, c2())], c0());
    assert_eq!(doc, con_a);

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, tras_relleno);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
}

#[test]
fn fila_04_los_pixeles_que_ya_valian_el_color_no_se_tocan() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();

    assert_eq!(doc.apply(set(id, &[px(1, 0, c1())])), Ok(true));
    let con_a = doc.clone();
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    comprueba_imagen(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    let tras_relleno = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    comprueba_imagen(&doc, &[(1, 0, c1())], c0());
    assert_eq!(doc, con_a);

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, tras_relleno);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
}

#[test]
fn fila_05_sin_cambios_es_ok_false() {
    // En D: rellenar con el valor que ya hay.
    let mut doc = doc_d();
    let id = doc.main_image();
    sin_cambios(&mut doc, fr(id, &[sp(0, 0, 4)], c0()));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);

    // Varios tramos, de varias filas, todos ya a c0.
    sin_cambios(&mut doc, fr(id, &[sp(0, 1, 3), sp(2, 0, 4)], c0()));
    assert_eq!(doc.history_bytes(), 0);

    // Tras un `undo`, la pila de rehacer se conserva.
    let mut doc = doc_d();
    assert_eq!(doc.apply(set(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let antes = estado(&doc);
    assert_eq!(
        doc.apply(fr(id, &[sp(0, 0, 4)], c0())),
        Ok(false),
        "relleno sin cambios"
    );
    assert!(doc.can_redo());
    assert_eq!(doc.can_undo(), antes.can_undo);
    assert_eq!(doc.history_bytes(), antes.bytes);
    assert_eq!(estado(&doc), antes);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
}

#[test]
fn fila_06_lista_vacia_imagen_desconocida_y_pixel_invalido() {
    let mut doc = doc_d();
    let id = doc.main_image();

    rechaza(&mut doc, fr(id, &[], c1()), DocumentError::EmptyCommand);

    let desconocida = ImageId::from_raw(99);
    rechaza(
        &mut doc,
        Command::FillSpans {
            image: desconocida,
            spans: vec![sp(0, 0, 4)],
            pixel: Pixel::Rgba(c1()),
        },
        DocumentError::UnknownImage(desconocida),
    );

    rechaza(
        &mut doc,
        fill(id, &[sp(0, 0, 4)], Pixel::Gray(1)),
        fallo_pixel(
            0,
            ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray,
            },
        ),
    );

    // Indexed(9) en una imagen indexada con paleta de 2.
    let mut doc = doc_indexado();
    let id = doc.main_image();
    rechaza(
        &mut doc,
        fill(id, &[sp(0, 0, 4)], Pixel::Indexed(9)),
        fallo_pixel(0, ImageError::IndexNotInPalette { index: 9, len: 2 }),
    );
}

#[test]
fn orden_de_comprobacion_de_fill_spans() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let desconocida = ImageId::from_raw(99);
    let malos = [sp(9, 5, 2), sp(0, 0, 1)];

    // EmptyCommand gana a UnknownImage (y al píxel).
    rechaza(
        &mut doc,
        fill(desconocida, &[], Pixel::Gray(1)),
        DocumentError::EmptyCommand,
    );
    // UnknownImage gana al píxel y a los tramos.
    rechaza(
        &mut doc,
        fill(desconocida, &malos, Pixel::Gray(1)),
        DocumentError::UnknownImage(desconocida),
    );
    // El píxel gana a los tramos (vacío, desordenado y fuera del lienzo).
    rechaza(
        &mut doc,
        fill(id, &malos, Pixel::Gray(1)),
        fallo_pixel(
            0,
            ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray,
            },
        ),
    );
    // Con el píxel bien, el tramo 0 vacío (que además se sale) es `BadSpans`.
    rechaza(
        &mut doc,
        fr(id, &malos, c1()),
        DocumentError::BadSpans { span: 0 },
    );

    // En un documento indexado, un índice fuera de la paleta gana a los tramos malos.
    let mut doc = doc_indexado();
    let id = doc.main_image();
    rechaza(
        &mut doc,
        fill(id, &malos, Pixel::Indexed(2)),
        fallo_pixel(0, ImageError::IndexNotInPalette { index: 2, len: 2 }),
    );
}

#[test]
fn fila_07_tramo_fuera_del_lienzo() {
    let casos = [
        // y >= height: primer píxel fuera (x_start, y).
        (vec![sp(3, 0, 1)], 0usize, fuera(0, 3, 4, 3)),
        // x_end > width: (max(x_start, width), y).
        (vec![sp(0, 2, 5)], 0, fuera(4, 0, 4, 3)),
        // x_start y x_end fuera.
        (vec![sp(1, 5, 7)], 0, fuera(5, 1, 4, 3)),
        // El fallo está en el segundo tramo.
        (vec![sp(0, 0, 2), sp(1, 3, 9)], 1, fuera(4, 1, 4, 3)),
        // y fuera con x_start grande: sigue siendo (x_start, y), no el máximo con width.
        (vec![sp(5, 9, 10)], 0, fuera(9, 5, 4, 3)),
        // Valores extremos, sin desbordar ni hacer pánico.
        (
            vec![sp(u32::MAX, u32::MAX - 1, u32::MAX)],
            0,
            fuera(u32::MAX - 1, u32::MAX, 4, 3),
        ),
        (vec![sp(0, 0, u32::MAX)], 0, fuera(4, 0, 4, 3)),
        (vec![sp(2, 3, u32::MAX)], 0, fuera(4, 2, 4, 3)),
    ];
    for (spans, n, fallo) in casos {
        let mut doc = doc_d();
        let id = doc.main_image();
        rechaza(&mut doc, fr(id, &spans, c1()), fallo_span(n, fallo));
        // Atomicidad: los tramos anteriores al defectuoso no se escribieron.
        comprueba_imagen(&doc, &[], c0());
        assert!(!doc.can_undo(), "{spans:?}");
        assert_eq!(doc.history_bytes(), 0, "{spans:?}");
    }
}

#[test]
fn fila_07_tramo_fuera_deja_intacta_la_historia_existente() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(set(id, &[px(3, 2, c2())])), Ok(true));
    assert_eq!(doc.apply(set(id, &[px(2, 2, c3())])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_undo() && doc.can_redo());

    rechaza(
        &mut doc,
        fr(id, &[sp(0, 0, 2), sp(1, 3, 9)], c1()),
        fallo_span(1, fuera(4, 1, 4, 3)),
    );
    assert!(doc.can_redo());
}

#[test]
fn fila_08_tramos_vacios_o_invertidos() {
    for spans in [vec![sp(0, 2, 2)], vec![sp(0, 3, 1)]] {
        let mut doc = doc_d();
        let id = doc.main_image();
        rechaza(
            &mut doc,
            fr(id, &spans, c1()),
            DocumentError::BadSpans { span: 0 },
        );
        comprueba_imagen(&doc, &[], c0());
        assert!(!doc.can_undo());
    }
    // Un tramo vacío tampoco se tolera en medio de una lista válida.
    let mut doc = doc_d();
    let id = doc.main_image();
    rechaza(
        &mut doc,
        fr(id, &[sp(0, 0, 2), sp(1, 1, 1), sp(2, 0, 1)], c1()),
        DocumentError::BadSpans { span: 1 },
    );
    comprueba_imagen(&doc, &[], c0());
}

#[test]
fn fila_09_tramos_desordenados_solapados_o_repetidos() {
    let casos = [
        vec![sp(1, 0, 2), sp(0, 0, 2)],
        vec![sp(0, 0, 3), sp(0, 2, 4)],
        vec![sp(0, 0, 2), sp(0, 0, 2)],
    ];
    for spans in casos {
        let mut doc = doc_d();
        let id = doc.main_image();
        rechaza(
            &mut doc,
            fr(id, &spans, c1()),
            DocumentError::BadSpans { span: 1 },
        );
        comprueba_imagen(&doc, &[], c0());
        assert!(!doc.can_undo(), "{spans:?}");
    }
    // Desorden dentro de la misma fila: el segundo empieza antes del fin del primero.
    let mut doc = doc_d();
    let id = doc.main_image();
    rechaza(
        &mut doc,
        fr(id, &[sp(0, 2, 4), sp(0, 0, 1)], c1()),
        DocumentError::BadSpans { span: 1 },
    );
    // El desorden se detecta también en el tercer tramo.
    rechaza(
        &mut doc,
        fr(id, &[sp(0, 0, 1), sp(1, 0, 1), sp(0, 2, 3)], c1()),
        DocumentError::BadSpans { span: 2 },
    );
}

#[test]
fn fila_10_pegados_validos_y_orden_de_errores() {
    // Dos tramos pegados son válidos.
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    assert_eq!(
        doc.apply(fr(id, &[sp(0, 0, 2), sp(0, 2, 4)], c1())),
        Ok(true)
    );
    comprueba_imagen(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);

    // Gana el primer tramo defectuoso, aunque el siguiente también lo sea.
    let mut doc = doc_d();
    let id = doc.main_image();
    rechaza(
        &mut doc,
        fr(id, &[sp(1, 0, 9), sp(0, 0, 1)], c1()),
        fallo_span(0, fuera(4, 1, 4, 3)),
    );
    // El tramo 1 solapa y además se sale: `BadSpans` antes que `Span`.
    rechaza(
        &mut doc,
        fr(id, &[sp(0, 0, 2), sp(0, 1, 9)], c1()),
        DocumentError::BadSpans { span: 1 },
    );
    comprueba_imagen(&doc, &[], c0());
}

#[test]
fn fila_11_batch_mezclado_es_un_solo_paso() {
    // (x, color) del píxel suelto `a` (antes del relleno) y del `b` (después).
    let variantes = [
        ((1, c2()), (2, c3())),
        ((1, c2()), (1, c3())),
        ((0, c2()), (3, c2())),
    ];
    for ((ax, ac), (bx, bc)) in variantes {
        let mut doc = doc_d();
        let inicial = doc.clone();
        let id = doc.main_image();
        let comando = lote(vec![
            set(id, &[px(ax, 0, ac)]),
            fr(id, &[sp(0, 0, 4)], c1()),
            set(id, &[px(bx, 0, bc)]),
        ]);

        assert_eq!(doc.apply(comando), Ok(true));
        comprueba_imagen(
            &doc,
            &[
                (0, 0, c1()),
                (1, 0, c1()),
                (2, 0, c1()),
                (3, 0, c1()),
                (bx, 0, bc),
            ],
            c0(),
        );
        assert_eq!(pasos_deshacibles(&doc), 1);
        let tras_apply = doc.clone();

        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc, inicial, "a = ({ax}, 0), b = ({bx}, 0)");
        assert!(!doc.can_undo());
        assert!(doc.can_redo());

        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, tras_apply, "a = ({ax}, 0), b = ({bx}, 0)");
        assert!(doc.can_undo());
        assert!(!doc.can_redo());
    }
}

#[test]
fn fila_12_batch_con_dos_rellenos() {
    let fila0 = [sp(0, 0, 4)];

    // Dos rellenos iguales: solo cambia el primero, y es un único paso.
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    assert_eq!(
        doc.apply(lote(vec![fr(id, &fila0, c1()), fr(id, &fila0, c1())])),
        Ok(true)
    );
    let mut solo_el_primero = doc_d();
    assert_eq!(solo_el_primero.apply(fr(id, &fila0, c1())), Ok(true));
    assert_eq!(doc, solo_el_primero);
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());

    // Cambia y restaura: cuenta como cambio y el contenido es igual a D.
    let mut doc = doc_d();
    assert_eq!(
        doc.apply(lote(vec![fr(id, &fila0, c1()), fr(id, &fila0, c0())])),
        Ok(true)
    );
    assert_eq!(doc, inicial);
    assert!(doc.can_undo());
    assert!(doc.history_bytes() > 0);
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(doc.can_undo());
}

#[test]
fn batch_solo_con_rellenos_sin_cambios_no_crea_paso() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let fila0 = [sp(0, 0, 4)];
    sin_cambios(
        &mut doc,
        lote(vec![fr(id, &fila0, c0()), fr(id, &[sp(2, 1, 3)], c0())]),
    );
    assert_eq!(doc.history_bytes(), 0);

    // Un relleno sin cambios más un `SetPixels` sin cambios tampoco.
    sin_cambios(
        &mut doc,
        lote(vec![fr(id, &fila0, c0()), set(id, &[px(3, 2, c0())])]),
    );

    // Con un cambio en cualquiera de los dos, sí.
    assert_eq!(
        doc.apply(lote(vec![fr(id, &fila0, c0()), set(id, &[px(3, 2, c1())])])),
        Ok(true)
    );
    assert_eq!(pasos_deshacibles(&doc), 1);
    let mut otro = doc_d();
    assert_eq!(
        otro.apply(lote(vec![set(id, &[px(3, 2, c1())]), fr(id, &fila0, c0())])),
        Ok(true)
    );
    assert_eq!(pasos_deshacibles(&otro), 1);
    assert_eq!(doc, otro);
}

/// Documentos de partida para las pruebas de lotes erróneos: uno nuevo y otro con
/// historia y pila de rehacer.
fn documentos_con_y_sin_historia() -> Vec<Document> {
    let nuevo = doc_d();
    let mut con_historia = doc_d();
    let id = con_historia.main_image();
    assert_eq!(con_historia.apply(set(id, &[px(3, 2, c2())])), Ok(true));
    assert_eq!(con_historia.apply(set(id, &[px(2, 2, c3())])), Ok(true));
    assert_eq!(con_historia.undo(), Ok(()));
    assert!(con_historia.can_undo() && con_historia.can_redo());
    vec![nuevo, con_historia]
}

#[test]
fn fila_13_batch_con_fallo_interno_no_cambia_nada() {
    for mut doc in documentos_con_y_sin_historia() {
        let id = doc.main_image();
        let fila0 = [sp(0, 0, 4)];

        // `Span` en el comando 1; el relleno anterior se revierte.
        rechaza(
            &mut doc,
            lote(vec![fr(id, &fila0, c1()), fr(id, &[sp(3, 0, 1)], c1())]),
            en_lote(1, fallo_span(0, fuera(0, 3, 4, 3))),
        );

        // `BadSpans` en el comando 1 tras un `SetPixels`.
        rechaza(
            &mut doc,
            lote(vec![
                set(id, &[px(0, 0, c1())]),
                fr(id, &[sp(1, 0, 2), sp(0, 0, 1)], c1()),
            ]),
            en_lote(1, DocumentError::BadSpans { span: 1 }),
        );

        // Un `Batch` dentro de otro.
        rechaza(
            &mut doc,
            lote(vec![
                fr(id, &fila0, c1()),
                lote(vec![set(id, &[px(1, 0, c1())])]),
            ]),
            en_lote(1, DocumentError::NestedBatch),
        );
    }
}

#[test]
fn batch_con_relleno_vacio_desconocido_o_con_pixel_invalido() {
    let desconocida = ImageId::from_raw(99);
    for mut doc in documentos_con_y_sin_historia() {
        let id = doc.main_image();
        let bueno = set(id, &[px(0, 0, c1())]);

        rechaza(
            &mut doc,
            lote(vec![bueno.clone(), fr(id, &[], c1())]),
            en_lote(1, DocumentError::EmptyCommand),
        );
        rechaza(
            &mut doc,
            lote(vec![bueno.clone(), fr(desconocida, &[sp(0, 0, 1)], c1())]),
            en_lote(1, DocumentError::UnknownImage(desconocida)),
        );
        rechaza(
            &mut doc,
            lote(vec![
                bueno.clone(),
                fill(id, &[sp(0, 0, 1)], Pixel::Gray(1)),
            ]),
            en_lote(
                1,
                fallo_pixel(
                    0,
                    ImageError::ModeMismatch {
                        expected: Mode::Rgba,
                        found: Mode::Gray,
                    },
                ),
            ),
        );
        // El fallo en el primer comando también sale como `InBatch` con índice 0.
        rechaza(
            &mut doc,
            lote(vec![fr(id, &[sp(0, 2, 2)], c1()), bueno]),
            en_lote(0, DocumentError::BadSpans { span: 0 }),
        );
    }
}

#[test]
fn fila_14_el_coste_no_depende_del_ancho() {
    fn bytes_de_rellenar_todo(width: u32) -> usize {
        let mut doc = Document::new(Image::new(width, 3, Mode::Rgba).unwrap());
        let id = doc.main_image();
        let spans = [sp(0, 0, width), sp(1, 0, width), sp(2, 0, width)];
        assert_eq!(doc.apply(fr(id, &spans, c1())), Ok(true));
        assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
        assert_eq!(pixel_en(&doc, width - 1, 2), Pixel::Rgba(c1()));
        assert_eq!(pasos_deshacibles(&doc), 1);
        doc.history_bytes()
    }

    let pequeno = bytes_de_rellenar_todo(4);
    assert!(pequeno > 0);
    assert_eq!(bytes_de_rellenar_todo(4000), pequeno);
    assert_eq!(bytes_de_rellenar_todo(64), pequeno);
    assert_eq!(bytes_de_rellenar_todo(1), pequeno);

    // Con `SetPixels`, en cambio, el coste crece con los píxeles: el relleno es mucho
    // más barato en el lienzo ancho.
    let mut doc = Document::new(Image::new(4000, 3, Mode::Rgba).unwrap());
    let id = doc.main_image();
    let pixeles: Vec<PixelEdit> = (0..3)
        .flat_map(|y| (0..4000).map(move |x| px(x, y, c1())))
        .collect();
    assert_eq!(doc.apply(set(id, &pixeles)), Ok(true));
    assert!(
        pequeno < doc.history_bytes(),
        "{} no es menor que {}",
        pequeno,
        doc.history_bytes()
    );
}

#[test]
fn fila_15_limite_cero_conserva_solo_el_ultimo_relleno() {
    let mut doc = doc_d();
    let id = doc.main_image();
    doc.set_history_limit(0);
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c1())), Ok(true));

    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.history_bytes(), coste_de_un_relleno(1));

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    comprueba_imagen(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
}

#[test]
fn fila_15_un_relleno_mayor_que_el_limite_se_conserva() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    doc.set_history_limit(1);

    let toda = [sp(0, 0, 4), sp(1, 0, 4), sp(2, 0, 4)];
    assert_eq!(doc.apply(fr(id, &toda, c1())), Ok(true));
    assert!(doc.history_bytes() > doc.history_limit());
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));

    // Con otro paso encima, el grande (el más antiguo) se descarta y el nuevo se conserva.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 1)], c2())), Ok(true));
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 3, 2), Pixel::Rgba(c1()));
}

#[test]
fn fila_15_limite_justo_para_dos_rellenos() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c1())), Ok(true));
    let dos = doc.history_bytes();
    assert!(dos > 0);

    // Un límite igual a la suma de dos pasos no descarta nada.
    doc.set_history_limit(dos);
    assert_eq!(pasos_deshacibles(&doc), 2);
    assert_eq!(doc.history_bytes(), dos);

    // El tercero hace que se descarte el más antiguo.
    assert_eq!(doc.apply(fr(id, &[sp(2, 0, 4)], c1())), Ok(true));
    assert_eq!(pasos_deshacibles(&doc), 2);
    assert!(doc.history_bytes() <= dos);
    let mut copia = doc.clone();
    assert_eq!(copia.undo(), Ok(()));
    assert_eq!(copia.undo(), Ok(()));
    assert_eq!(copia.undo(), Err(DocumentError::NothingToUndo));
    let mut con_el_primero = doc_d();
    assert_eq!(con_el_primero.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(copia, con_el_primero);
}

#[test]
fn fila_16_indice_fuera_de_la_paleta() {
    // Literales en `IndexNotInPalette`, para no fijar el tipo entero de sus campos.
    let casos = [
        (
            2u8,
            fallo_pixel(0, ImageError::IndexNotInPalette { index: 2, len: 2 }),
        ),
        (
            9,
            fallo_pixel(0, ImageError::IndexNotInPalette { index: 9, len: 2 }),
        ),
        (
            255,
            fallo_pixel(0, ImageError::IndexNotInPalette { index: 255, len: 2 }),
        ),
    ];
    for (indice, esperado) in casos {
        let mut doc = doc_indexado();
        let id = doc.main_image();

        // Suelto.
        rechaza(
            &mut doc,
            fill(id, &[sp(0, 0, 2)], Pixel::Indexed(indice)),
            esperado.clone(),
        );

        // Segundo comando de un `Batch` tras un `SetPixels` válido, que se revierte.
        let ok = set(
            id,
            &[PixelEdit {
                x: 3,
                y: 2,
                pixel: Pixel::Indexed(1),
            }],
        );
        rechaza(
            &mut doc,
            lote(vec![ok, fill(id, &[sp(0, 0, 2)], Pixel::Indexed(indice))]),
            en_lote(1, esperado),
        );
        assert_eq!(pixel_en(&doc, 3, 2), Pixel::Indexed(0));
        assert!(!doc.can_undo());
    }
}

#[test]
fn fila_16_indexed_1_es_valido_en_paleta_de_2() {
    let mut doc = doc_indexado();
    let inicial = doc.clone();
    let id = doc.main_image();

    assert_eq!(
        doc.apply(fill(id, &[sp(0, 0, 2)], Pixel::Indexed(1))),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Indexed(1));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Indexed(1));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Indexed(0));
    assert_eq!(pixel_en(&doc, 0, 1), Pixel::Indexed(0));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);

    // Y dentro de un `Batch`.
    let ok = set(
        id,
        &[PixelEdit {
            x: 3,
            y: 2,
            pixel: Pixel::Indexed(1),
        }],
    );
    assert_eq!(
        doc.apply(lote(vec![ok, fill(id, &[sp(0, 0, 2)], Pixel::Indexed(1))])),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 3, 2), Pixel::Indexed(1));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Indexed(1));
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);

    // Rellenar con el índice que ya hay (0) no cambia nada.
    sin_cambios(&mut doc, fill(id, &[sp(0, 0, 4)], Pixel::Indexed(0)));
}

/// `D'` de la spec: `D` tras `S(px(1,0,c1))` y `S(px(2,1,c2))`.
fn d_prima() -> Document {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(set(id, &[px(1, 0, c1())])), Ok(true));
    assert_eq!(doc.apply(set(id, &[px(2, 1, c2())])), Ok(true));
    doc
}

/// Aplica `comando` sobre `D'` y comprueba la fila 17: un solo paso más, el estado tras
/// `apply` es `esperado` (los píxeles que no valen `c0`; en `D'` el `(2,1)` ya es `c2`),
/// `undo()` deja una imagen idéntica a `D'` y `redo()` deja exactamente la de tras `apply`.
fn comprueba_fila_17(comando: Command, esperado: &[(u32, u32, Rgba)]) {
    let mut doc = d_prima();
    let original = doc.clone();

    assert_eq!(doc.apply(comando.clone()), Ok(true), "{comando:?}");
    assert_eq!(pasos_deshacibles(&doc), 3, "{comando:?}");
    comprueba_imagen(&doc, esperado, c0());
    let tras_apply = doc.clone();

    for _ in 0..2 {
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc, original, "{comando:?}");
        assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c1()));
        assert_eq!(pasos_deshacibles(&doc), 2);
        assert!(doc.can_redo());

        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, tras_apply, "{comando:?}");
        comprueba_imagen(&doc, esperado, c0());
        assert_eq!(pasos_deshacibles(&doc), 3);
        assert!(!doc.can_redo());
    }
}

#[test]
fn fila_17a_el_relleno_pisa_al_pixel_suelto() {
    let id = doc_d().main_image();
    comprueba_fila_17(
        lote(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 0, 4)], c1()),
        ]),
        &[
            (0, 0, c1()),
            (1, 0, c1()),
            (2, 0, c1()),
            (3, 0, c1()),
            (2, 1, c2()),
        ],
    );
}

#[test]
fn fila_17b_el_pixel_suelto_pisa_al_relleno() {
    let id = doc_d().main_image();
    comprueba_fila_17(
        lote(vec![
            fr(id, &[sp(0, 0, 4)], c1()),
            set(id, &[px(1, 0, c2())]),
        ]),
        &[
            (0, 0, c1()),
            (1, 0, c2()),
            (2, 0, c1()),
            (3, 0, c1()),
            (2, 1, c2()),
        ],
    );
}

#[test]
fn fila_17c_pixel_relleno_pixel() {
    let id = doc_d().main_image();
    // `p` en (1,0) y `p'` en otra casilla de la fila.
    comprueba_fila_17(
        lote(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 0, 4)], c3()),
            set(id, &[px(2, 0, c1())]),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c3()),
            (2, 0, c1()),
            (3, 0, c3()),
            (2, 1, c2()),
        ],
    );
    // `p'` en la misma casilla que `p`, y con el valor que `D'` ya tenía ahí (c1).
    comprueba_fila_17(
        lote(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 0, 4)], c3()),
            set(id, &[px(1, 0, c1())]),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c1()),
            (2, 0, c3()),
            (3, 0, c3()),
            (2, 1, c2()),
        ],
    );
    // Un `SetPixels` con el mismo píxel repetido antes y después del relleno.
    comprueba_fila_17(
        lote(vec![
            set(id, &[px(1, 0, c2()), px(1, 0, c3())]),
            fr(id, &[sp(0, 1, 3)], c2()),
            set(id, &[px(2, 0, c3()), px(1, 0, c3()), px(2, 0, c1())]),
        ]),
        &[
            (0, 0, c0()),
            (1, 0, c3()),
            (2, 0, c1()),
            (3, 0, c0()),
            (2, 1, c2()),
        ],
    );
}

#[test]
fn fila_17d_dos_rellenos_seguidos_que_se_solapan() {
    let id = doc_d().main_image();
    comprueba_fila_17(
        lote(vec![
            fr(id, &[sp(0, 0, 3)], c3()),
            fr(id, &[sp(0, 1, 4)], c2()),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c2()),
            (2, 0, c2()),
            (3, 0, c2()),
            (2, 1, c2()),
        ],
    );
    comprueba_fila_17(
        lote(vec![
            fr(id, &[sp(0, 1, 4)], c2()),
            fr(id, &[sp(0, 0, 3)], c3()),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c3()),
            (2, 0, c3()),
            (3, 0, c2()),
            (2, 1, c2()),
        ],
    );
    // Relleno, píxel suelto en medio y otro relleno que lo pisa.
    comprueba_fila_17(
        lote(vec![
            fr(id, &[sp(0, 0, 4)], c3()),
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 1, 3)], c1()),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c1()),
            (2, 0, c1()),
            (3, 0, c3()),
            (2, 1, c2()),
        ],
    );
}

// Estrategias.

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

/// Imagen de hasta 6 x 6 en cualquier modo; el indexado lleva una paleta de 1 a 8 entradas.
fn imagen_6() -> impl Strategy<Value = Image> {
    (modo(), 1u32..=6, 1u32..=6, 1usize..=8).prop_map(|(m, w, h, n)| crea(m, w, h, n))
}

/// Lo que hace falta saber de una imagen para construir comandos válidos o con fallos.
#[derive(Clone, Copy, Debug)]
struct Forma {
    width: u32,
    height: u32,
    mode: Mode,
    colores: usize,
}

fn forma(img: &Image) -> Forma {
    Forma {
        width: img.width(),
        height: img.height(),
        mode: img.mode(),
        colores: img.palette().map_or(0, |p| p.len()),
    }
}

/// Píxel válido de un modo a partir de un valor pequeño. El 0 coincide con el valor
/// inicial de la imagen, y los demás se repiten mucho: así abundan los no-ops.
fn pixel_de(mode: Mode, colores: usize, valor: u8) -> Pixel {
    match mode {
        Mode::Rgba if valor == 0 => Pixel::Rgba(Rgba::TRANSPARENT),
        Mode::Rgba => Pixel::Rgba(Rgba::new(valor, 255 - valor, valor ^ 5, 255)),
        Mode::Indexed => Pixel::Indexed((valor as usize % colores.max(1)) as u8),
        Mode::Gray => Pixel::Gray(valor),
        Mode::GrayAlpha if valor == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: valor,
            alpha: 255,
        },
    }
}

fn otro_modo(mode: Mode, k: u8) -> Mode {
    let i = TODOS.iter().position(|&m| m == mode).unwrap();
    TODOS[(i + 1 + k as usize % 3) % TODOS.len()]
}

/// Tramos válidos (ordenados por `(y, x_start)`, sin solapes ni vacíos y siempre al menos
/// uno) dentro de una imagen de forma `f`. Cada fila `y` toma los bits de `masks[y]`: cada
/// serie de bits seguidos es un tramo, y con `corte` las series de 2 o más píxeles se
/// parten en dos tramos pegados.
fn spans_validos(f: Forma, masks: &[u8], corte: bool) -> Vec<Span> {
    let mut spans = Vec::new();
    for (y, &m) in masks.iter().enumerate().take(f.height as usize) {
        let y = y as u32;
        let mut x = 0u32;
        while x < f.width {
            if (m >> x) & 1 == 0 {
                x += 1;
                continue;
            }
            let ini = x;
            while x < f.width && (m >> x) & 1 == 1 {
                x += 1;
            }
            if corte && x - ini >= 2 {
                let medio = ini + (x - ini) / 2;
                spans.push(sp(y, ini, medio));
                spans.push(sp(y, medio, x));
            } else {
                spans.push(sp(y, ini, x));
            }
        }
    }
    if spans.is_empty() {
        spans.push(sp(0, 0, 1));
    }
    spans
}

fn masks() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0u8..64, 6)
}

/// Escrituras en bruto de un `SetPixels`: coordenadas sin acotar y un valor pequeño.
fn ediciones() -> impl Strategy<Value = Vec<(u32, u32, u8)>> {
    prop::collection::vec((any::<u32>(), any::<u32>(), 0u8..4), 1..=4)
}

/// Una hoja (un `SetPixels` o un `FillSpans`) en bruto. Las variantes con referencia
/// (`FillCubre`, `FillCopia`, `SetDentro`) apuntan, módulo el número de candidatas, a otra
/// hoja **del mismo lote**, para que los píxeles sueltos y los tramos se solapen a propósito:
///
/// - `FillCubre`: tramos de un píxel sobre los píxeles de un `Set` libre del lote.
/// - `FillCopia`: los mismos tramos (o su primera mitad) que un `Fill` propio del lote, con
///   otro valor: dos rellenos que se pisan.
/// - `SetDentro`: píxeles sueltos elegidos dentro de los tramos de un `Fill` propio.
///
/// Si el lote no tiene a quién apuntar, la hoja se degrada a una libre.
#[derive(Clone, Debug)]
enum HojaRaw {
    Fill {
        masks: Vec<u8>,
        corte: bool,
        valor: u8,
    },
    FillCubre {
        set: usize,
        valor: u8,
    },
    FillCopia {
        fill: usize,
        mitad: bool,
        valor: u8,
    },
    Set {
        ediciones: Vec<(u32, u32, u8)>,
    },
    SetDentro {
        fill: usize,
        ediciones: Vec<(u32, u32, u8)>,
    },
}

/// Fallos que se inyectan en un `FillSpans` ya construido. No garantizan el error
/// concreto (lo decide el modelo), solo lo provocan a menudo.
#[derive(Clone, Copy, Debug)]
enum FallaSpan {
    ListaVacia,
    IdDesconocido,
    OtroModo,
    IndiceFuera,
    Vacio,
    Desordenado,
    Solapado,
    FueraX,
    FueraY,
}

const FALLAS: [FallaSpan; 9] = [
    FallaSpan::ListaVacia,
    FallaSpan::IdDesconocido,
    FallaSpan::OtroModo,
    FallaSpan::IndiceFuera,
    FallaSpan::Vacio,
    FallaSpan::Desordenado,
    FallaSpan::Solapado,
    FallaSpan::FueraX,
    FallaSpan::FueraY,
];

#[derive(Clone, Debug)]
struct Hoja {
    raw: HojaRaw,
    /// Solo se aplica si la hoja resulta ser un `FillSpans`.
    falla: Option<(FallaSpan, u8)>,
}

#[derive(Clone, Debug)]
enum Cmd {
    Hoja(Hoja),
    Lote(Vec<Hoja>),
}

fn hoja_raw() -> impl Strategy<Value = HojaRaw> {
    prop_oneof![
        3 => (masks(), any::<bool>(), 0u8..4)
            .prop_map(|(masks, corte, valor)| HojaRaw::Fill { masks, corte, valor }),
        2 => (any::<usize>(), 0u8..4)
            .prop_map(|(set, valor)| HojaRaw::FillCubre { set, valor }),
        2 => (any::<usize>(), any::<bool>(), 0u8..4)
            .prop_map(|(fill, mitad, valor)| HojaRaw::FillCopia { fill, mitad, valor }),
        3 => ediciones().prop_map(|ediciones| HojaRaw::Set { ediciones }),
        3 => (any::<usize>(), ediciones())
            .prop_map(|(fill, ediciones)| HojaRaw::SetDentro { fill, ediciones }),
    ]
}

fn falla_span(con_fallos: bool) -> BoxedStrategy<Option<(FallaSpan, u8)>> {
    if con_fallos {
        prop_oneof![
            5 => Just(None::<(FallaSpan, u8)>),
            2 => (prop::sample::select(FALLAS.to_vec()), any::<u8>()).prop_map(Some),
        ]
        .boxed()
    } else {
        Just(None).boxed()
    }
}

fn hoja(con_fallos: bool) -> impl Strategy<Value = Hoja> {
    (hoja_raw(), falla_span(con_fallos)).prop_map(|(raw, falla)| Hoja { raw, falla })
}

/// Una hoja suelta o un `Batch` de 1 a 4 hojas (de 0 a 4 con fallos), con fallos o sin ellos.
fn cmd(con_fallos: bool) -> impl Strategy<Value = Cmd> {
    let largo = if con_fallos { 0..=4 } else { 1..=4 };
    prop_oneof![
        2 => hoja(con_fallos).prop_map(Cmd::Hoja),
        2 => prop::collection::vec(hoja(con_fallos), largo).prop_map(Cmd::Lote),
    ]
}

fn set_libre(f: Forma, id: ImageId, ediciones: &[(u32, u32, u8)]) -> Command {
    Command::SetPixels {
        image: id,
        pixels: ediciones
            .iter()
            .map(|&(xb, yb, v)| PixelEdit {
                x: xb % f.width,
                y: yb % f.height,
                pixel: pixel_de(f.mode, f.colores, v),
            })
            .collect(),
    }
}

fn tramos_en(cmds: &[Option<Command>], i: usize) -> Vec<Span> {
    match cmds.get(i) {
        Some(Some(Command::FillSpans { spans, .. })) => spans.clone(),
        _ => Vec::new(),
    }
}

fn pixeles_en(cmds: &[Option<Command>], i: usize) -> Vec<PixelEdit> {
    match cmds.get(i) {
        Some(Some(Command::SetPixels { pixels, .. })) => pixels.clone(),
        _ => Vec::new(),
    }
}

/// Aplica un fallo a un `FillSpans`; cualquier otro comando queda igual.
fn inyecta(f: Forma, cmd: Command, falla: FallaSpan, k: u8) -> Command {
    match cmd {
        Command::FillSpans {
            mut image,
            mut spans,
            mut pixel,
        } => {
            match falla {
                FallaSpan::ListaVacia => spans.clear(),
                // La imagen principal es la 0.
                FallaSpan::IdDesconocido => image = ImageId::from_raw(1 + k as u32),
                FallaSpan::OtroModo => pixel = pixel_de(otro_modo(f.mode, k), 8, k),
                // En una imagen no indexada esto es un `ModeMismatch`.
                FallaSpan::IndiceFuera => {
                    pixel = Pixel::Indexed((f.colores + (k as usize % 4)) as u8)
                }
                FallaSpan::Vacio => {
                    let n = spans.len().max(1);
                    if let Some(t) = spans.get_mut(k as usize % n) {
                        t.x_end = t.x_start;
                    }
                }
                FallaSpan::Desordenado => {
                    if spans.len() >= 2 {
                        spans.swap(0, 1);
                    } else {
                        spans.insert(0, sp(f.height, 0, 1));
                    }
                }
                FallaSpan::Solapado => {
                    if let Some(&t) = spans.first() {
                        spans.insert(1, t);
                    }
                }
                FallaSpan::FueraX => {
                    if let Some(t) = spans.last_mut() {
                        t.x_end = f.width + 1 + (k as u32 % 5);
                    }
                }
                FallaSpan::FueraY => {
                    if let Some(t) = spans.last_mut() {
                        t.y = f.height + (k as u32 % 5);
                    }
                }
            }
            Command::FillSpans {
                image,
                spans,
                pixel,
            }
        }
        otro => otro,
    }
}

/// Convierte las hojas de un lote en comandos. Las referencias se resuelven en una
/// segunda pasada contra las hojas independientes (sin fallos aún: los fallos se inyectan
/// al final), así que los píxeles sueltos caen dentro de los tramos y al revés.
fn resuelve(f: Forma, id: ImageId, hojas: &[Hoja]) -> Vec<Command> {
    let pixel = |v: u8| pixel_de(f.mode, f.colores, v);
    let relleno = |spans: Vec<Span>, v: u8| Command::FillSpans {
        image: id,
        spans,
        pixel: pixel(v),
    };

    // Primera pasada: hojas independientes.
    let mut cmds: Vec<Option<Command>> = hojas
        .iter()
        .map(|h| match &h.raw {
            HojaRaw::Fill {
                masks,
                corte,
                valor,
            } => Some(relleno(spans_validos(f, masks, *corte), *valor)),
            HojaRaw::Set { ediciones } => Some(set_libre(f, id, ediciones)),
            _ => None,
        })
        .collect();
    let rellenos: Vec<usize> = hojas
        .iter()
        .enumerate()
        .filter(|(_, h)| matches!(h.raw, HojaRaw::Fill { .. }))
        .map(|(i, _)| i)
        .collect();
    let sets: Vec<usize> = hojas
        .iter()
        .enumerate()
        .filter(|(_, h)| matches!(h.raw, HojaRaw::Set { .. }))
        .map(|(i, _)| i)
        .collect();

    // Segunda pasada: hojas que apuntan a otras del mismo lote.
    for (i, h) in hojas.iter().enumerate() {
        let nuevo = match &h.raw {
            HojaRaw::FillCubre { set, valor } => {
                let mut coords: Vec<(u32, u32)> = if sets.is_empty() {
                    vec![(0, 0)]
                } else {
                    pixeles_en(&cmds, sets[*set % sets.len()])
                        .iter()
                        .map(|p| (p.y, p.x))
                        .collect()
                };
                coords.sort_unstable();
                coords.dedup();
                let spans = coords.iter().map(|&(y, x)| sp(y, x, x + 1)).collect();
                relleno(spans, *valor)
            }
            HojaRaw::FillCopia { fill, mitad, valor } => {
                let mut spans = if rellenos.is_empty() {
                    vec![sp(0, 0, 1)]
                } else {
                    tramos_en(&cmds, rellenos[*fill % rellenos.len()])
                };
                if *mitad {
                    spans.truncate(spans.len().div_ceil(2));
                }
                relleno(spans, *valor)
            }
            HojaRaw::SetDentro { fill, ediciones } => {
                let tramos = if rellenos.is_empty() {
                    Vec::new()
                } else {
                    tramos_en(&cmds, rellenos[*fill % rellenos.len()])
                };
                if tramos.is_empty() {
                    set_libre(f, id, ediciones)
                } else {
                    Command::SetPixels {
                        image: id,
                        pixels: ediciones
                            .iter()
                            .map(|&(xb, yb, v)| {
                                let t = tramos[yb as usize % tramos.len()];
                                PixelEdit {
                                    x: t.x_start + xb % (t.x_end - t.x_start),
                                    y: t.y,
                                    pixel: pixel(v),
                                }
                            })
                            .collect(),
                    }
                }
            }
            _ => continue,
        };
        cmds[i] = Some(nuevo);
    }

    cmds.into_iter()
        .zip(hojas)
        .map(|(c, h)| {
            let c = c.expect("todas las hojas quedan resueltas");
            match h.falla {
                Some((falla, k)) => inyecta(f, c, falla, k),
                None => c,
            }
        })
        .collect()
}

fn comando(f: Forma, id: ImageId, raw: &Cmd) -> Command {
    match raw {
        Cmd::Hoja(h) => resuelve(f, id, std::slice::from_ref(h))
            .pop()
            .expect("una hoja da un comando"),
        Cmd::Lote(hojas) => Command::Batch(resuelve(f, id, hojas)),
    }
}

/// Documento con una imagen (con contenido previo, sacado de aplicar `semilla`) pero sin
/// historia.
fn documento_inicial(img: Image, semilla: &[Cmd]) -> Document {
    let f = forma(&img);
    let mut doc = Document::new(img);
    let id = doc.main_image();
    for c in semilla {
        let _ = doc.apply(comando(f, id, c));
    }
    Document::new(imagen_de(&doc).clone())
}

// Comandos totalmente arbitrarios (propiedad 3).

#[derive(Clone, Debug)]
enum ArbRaw {
    Fill {
        id: Option<u32>,
        spans: Vec<(u32, u32, u32)>,
        pixel: Pixel,
        /// Si hay valor, sustituye al píxel por uno válido del modo de la imagen.
        propio: Option<u8>,
    },
    Set {
        id: Option<u32>,
        ediciones: Vec<(u32, u32, u8)>,
    },
    Batch(Vec<ArbRaw>),
}

fn pixel_arbitrario() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(Rgba::new(r, g, b, a))),
        any::<u8>().prop_map(Pixel::Indexed),
        any::<u8>().prop_map(Pixel::Gray),
        (any::<u8>(), any::<u8>()).prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha }),
    ]
}

fn coordenada_arbitraria() -> impl Strategy<Value = u32> {
    prop_oneof![
        6 => 0u32..7,
        1 => any::<u32>(),
        1 => Just(u32::MAX),
    ]
}

fn id_arbitrario() -> impl Strategy<Value = Option<u32>> {
    prop_oneof![4 => Just(None), 1 => any::<u32>().prop_map(Some)]
}

fn arb_raw() -> impl Strategy<Value = ArbRaw> {
    let relleno = (
        id_arbitrario(),
        prop::collection::vec(
            (
                coordenada_arbitraria(),
                coordenada_arbitraria(),
                coordenada_arbitraria(),
            ),
            0..=5,
        ),
        pixel_arbitrario(),
        prop_oneof![2 => Just(None), 3 => (0u8..4).prop_map(Some)],
    )
        .prop_map(|(id, spans, pixel, propio)| ArbRaw::Fill {
            id,
            spans,
            pixel,
            propio,
        });
    let suelto =
        (id_arbitrario(), ediciones()).prop_map(|(id, ediciones)| ArbRaw::Set { id, ediciones });
    let hoja = prop_oneof![4 => relleno, 1 => suelto];
    hoja.prop_recursive(2, 16, 3, |interno| {
        prop::collection::vec(interno, 0..=3).prop_map(ArbRaw::Batch)
    })
}

fn arb_comando(f: Forma, id: ImageId, raw: &ArbRaw) -> Command {
    match raw {
        ArbRaw::Fill {
            id: otro,
            spans,
            pixel,
            propio,
        } => Command::FillSpans {
            image: otro.map_or(id, ImageId::from_raw),
            spans: spans
                .iter()
                .map(|&(y, a, b)| Span {
                    y,
                    x_start: a,
                    x_end: b,
                })
                .collect(),
            pixel: propio.map_or(*pixel, |v| pixel_de(f.mode, f.colores, v)),
        },
        ArbRaw::Set {
            id: otro,
            ediciones,
        } => Command::SetPixels {
            image: otro.map_or(id, ImageId::from_raw),
            pixels: ediciones
                .iter()
                .map(|&(xb, yb, v)| PixelEdit {
                    x: xb,
                    y: yb,
                    pixel: pixel_de(f.mode, f.colores, v),
                })
                .collect(),
        },
        ArbRaw::Batch(comandos) => {
            Command::Batch(comandos.iter().map(|c| arb_comando(f, id, c)).collect())
        }
    }
}

// Modelo de referencia: escribe píxel a píxel sobre instantáneas de `Image` (propiedades 2 y 5).

/// Aplica un comando hoja sobre `img` y dice si cambió algo. Para `FillSpans` sigue el
/// orden de la spec: `EmptyCommand`, `UnknownImage`, el píxel, y después cada tramo en
/// orden (`BadSpans` antes que `Span`), todo en una pasada de solo lectura antes de escribir.
fn modelo_hoja(img: &mut Image, id: ImageId, cmd: &Command) -> Result<bool, DocumentError> {
    match cmd {
        Command::SetPixels { image, pixels } => {
            if pixels.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            if *image != id {
                return Err(DocumentError::UnknownImage(*image));
            }
            let mut cambio = false;
            for (i, e) in pixels.iter().enumerate() {
                let antes = img.get_pixel(e.x, e.y);
                img.set_pixel(e.x, e.y, e.pixel)
                    .map_err(|source| fallo_pixel(i, source))?;
                if antes != Ok(e.pixel) {
                    cambio = true;
                }
            }
            Ok(cambio)
        }
        Command::FillSpans {
            image,
            spans,
            pixel,
        } => {
            if spans.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            if *image != id {
                return Err(DocumentError::UnknownImage(*image));
            }
            // El error de píxel lo da la propia imagen (en una copia, en la esquina 0, 0).
            img.clone()
                .set_pixel(0, 0, *pixel)
                .map_err(|source| fallo_pixel(0, source))?;
            let (w, h) = (img.width(), img.height());
            for (i, t) in spans.iter().enumerate() {
                let desordenado = i > 0 && {
                    let previo = &spans[i - 1];
                    t.y < previo.y || (t.y == previo.y && t.x_start < previo.x_end)
                };
                if t.x_start >= t.x_end || desordenado {
                    return Err(DocumentError::BadSpans { span: i });
                }
                if t.y >= h {
                    return Err(fallo_span(i, fuera(t.x_start, t.y, w, h)));
                }
                if t.x_end > w {
                    return Err(fallo_span(i, fuera(t.x_start.max(w), t.y, w, h)));
                }
            }
            let mut cambio = false;
            for t in spans {
                for x in t.x_start..t.x_end {
                    if img.get_pixel(x, t.y) != Ok(*pixel) {
                        img.set_pixel(x, t.y, *pixel)
                            .expect("el tramo y el píxel ya están validados");
                        cambio = true;
                    }
                }
            }
            Ok(cambio)
        }
        otro => unreachable!("comando sin modelo: {otro:?}"),
    }
}

/// Resultado esperado de `apply` sobre `actual`: la imagen resultante y si cambió, o el
/// error. Los comandos de un `Batch` se ejecutan en orden y el primer fallo gana.
fn modelo_apply(
    actual: &Image,
    id: ImageId,
    comando: &Command,
) -> Result<(Image, bool), DocumentError> {
    let mut trabajo = actual.clone();
    match comando {
        Command::Batch(comandos) => {
            if comandos.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            let mut cambio = false;
            for (index, c) in comandos.iter().enumerate() {
                match c {
                    Command::Batch(_) => return Err(en_lote(index, DocumentError::NestedBatch)),
                    hoja => {
                        cambio |=
                            modelo_hoja(&mut trabajo, id, hoja).map_err(|e| en_lote(index, e))?;
                    }
                }
            }
            Ok((trabajo, cambio))
        }
        hoja => {
            let cambio = modelo_hoja(&mut trabajo, id, hoja)?;
            Ok((trabajo, cambio))
        }
    }
}

/// El documento tiene exactamente `esperado` como contenido.
fn comprueba_contenido(doc: &Document, esperado: &Image) -> Result<(), TestCaseError> {
    prop_assert_eq!(doc.image(doc.main_image()), Some(esperado));
    prop_assert_eq!(doc, &Document::new(esperado.clone()));
    Ok(())
}

#[derive(Clone, Debug)]
enum Op {
    Apply(Cmd),
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => cmd(false).prop_map(Op::Apply),
        3 => cmd(true).prop_map(Op::Apply),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
    ]
}

/// Ejecuta `ops` sobre un documento y sobre el modelo (instantáneas de la imagen y un
/// cursor) y los compara tras cada operación.
fn ejecuta_modelo(inicial: Document, ops: &[Op]) -> Result<(), TestCaseError> {
    let img = imagen_de(&inicial).clone();
    let f = forma(&img);
    let mut doc = inicial;
    let id = doc.main_image();
    let mut instantes = vec![img];
    let mut cursor = 0usize;

    for op in ops {
        let antes = estado(&doc);
        match op {
            Op::Apply(raw) => {
                let cmd = comando(f, id, raw);
                let previsto = modelo_apply(&instantes[cursor], id, &cmd);
                let real = doc.apply(cmd.clone());
                match previsto {
                    Ok((nueva, true)) => {
                        prop_assert_eq!(real, Ok(true), "{:?}", cmd);
                        instantes.truncate(cursor + 1);
                        instantes.push(nueva);
                        cursor += 1;
                    }
                    Ok((_, false)) => {
                        prop_assert_eq!(real, Ok(false), "{:?}", cmd);
                        prop_assert_eq!(estado(&doc), antes);
                    }
                    Err(e) => {
                        prop_assert_eq!(real, Err(e), "{:?}", cmd);
                        prop_assert_eq!(estado(&doc), antes);
                    }
                }
            }
            Op::Undo => {
                if cursor > 0 {
                    prop_assert_eq!(doc.undo(), Ok(()));
                    cursor -= 1;
                } else {
                    prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
                    prop_assert_eq!(estado(&doc), antes);
                }
            }
            Op::Redo => {
                if cursor + 1 < instantes.len() {
                    prop_assert_eq!(doc.redo(), Ok(()));
                    cursor += 1;
                } else {
                    prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
                    prop_assert_eq!(estado(&doc), antes);
                }
            }
        }
        comprueba_contenido(&doc, &instantes[cursor])?;
        prop_assert_eq!(doc.can_undo(), cursor > 0);
        prop_assert_eq!(doc.can_redo(), cursor + 1 < instantes.len());
        prop_assert_eq!(pasos_deshacibles(&doc), cursor);
        prop_assert_eq!(pasos_rehacibles(&doc), instantes.len() - 1 - cursor);
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum OpArb {
    Cmd(Cmd),
    Arb(ArbRaw),
    Undo,
    Redo,
}

fn op_arb() -> impl Strategy<Value = OpArb> {
    prop_oneof![
        4 => cmd(true).prop_map(OpArb::Cmd),
        4 => arb_raw().prop_map(OpArb::Arb),
        1 => Just(OpArb::Undo),
        1 => Just(OpArb::Redo),
    ]
}

/// `apply(cmd)`: si no da `Ok(true)` no se toca nada observable; si lo da, hay algo que
/// deshacer y nada que rehacer.
fn aplica_y_comprueba(
    doc: &mut Document,
    cmd: Command,
    antes: &Estado,
) -> Result<(), TestCaseError> {
    match doc.apply(cmd.clone()) {
        Ok(true) => {
            prop_assert!(doc.can_undo());
            prop_assert!(!doc.can_redo());
        }
        Ok(false) | Err(_) => prop_assert_eq!(&estado(doc), antes, "{:?}", cmd),
    }
    Ok(())
}

/// Cuenta `undo()` hasta el primer `Err`, sin pasarse de `tope + 1` por si nunca falla.
fn deshace_todo(doc: &mut Document, tope: usize) -> usize {
    let mut n = 0;
    while n <= tope && doc.undo().is_ok() {
        n += 1;
    }
    n
}

fn rehace_todo(doc: &mut Document, tope: usize) -> usize {
    let mut n = 0;
    while n <= tope && doc.redo().is_ok() {
        n += 1;
    }
    n
}

/// Comprobaciones de la propiedad 5 sobre un documento que solo ha recibido `apply`:
/// `instantes[0]` es el inicial y cada `Ok(true)` añadió uno.
fn comprueba_limite(
    doc: &Document,
    instantes: &[Image],
    limite: usize,
) -> Result<(), TestCaseError> {
    let creados = instantes.len() - 1;
    let conservados = pasos_deshacibles(doc);

    prop_assert_eq!(doc.history_limit(), limite);
    prop_assert!(conservados <= creados, "{} > {}", conservados, creados);
    prop_assert!(
        creados == 0 || conservados >= 1,
        "el último paso siempre se conserva"
    );
    prop_assert!(
        doc.history_bytes() <= limite || conservados == 1,
        "history_bytes = {} > límite {} con {} pasos",
        doc.history_bytes(),
        limite,
        conservados
    );
    prop_assert_eq!(doc.can_undo(), conservados > 0);
    prop_assert!(!doc.can_redo());
    comprueba_contenido(doc, &instantes[creados])?;

    // Deshacer todo llega al instante en que empezó el primer paso conservado.
    let bytes_tras_apply = doc.history_bytes();
    let mut copia = doc.clone();
    prop_assert_eq!(deshace_todo(&mut copia, conservados), conservados);
    prop_assert_eq!(copia.undo(), Err(DocumentError::NothingToUndo));
    comprueba_contenido(&copia, &instantes[creados - conservados])?;

    // Rehacer todo devuelve el final, con el mismo coste que tras el `apply`: `undo` y
    // `redo` solo mueven los pasos entre pilas.
    prop_assert_eq!(rehace_todo(&mut copia, conservados), conservados);
    prop_assert_eq!(copia.redo(), Err(DocumentError::NothingToRedo));
    comprueba_contenido(&copia, &instantes[creados])?;
    prop_assert_eq!(copia.history_bytes(), bytes_tras_apply);
    Ok(())
}

/// Aplica `cmds` (válidos) a `doc`, comprueba cada resultado contra el modelo, añade a
/// `instantes` una imagen por cada `Ok(true)` y tras cada `apply` comprueba la propiedad 5.
fn aplica_validos(
    doc: &mut Document,
    instantes: &mut Vec<Image>,
    f: Forma,
    cmds: &[Cmd],
    limite: usize,
) -> Result<(), TestCaseError> {
    let id = doc.main_image();
    for raw in cmds {
        let cmd = comando(f, id, raw);
        let actual = instantes.last().unwrap().clone();
        let (nueva, cambio) = match modelo_apply(&actual, id, &cmd) {
            Ok(r) => r,
            Err(e) => {
                return Err(TestCaseError::fail(format!(
                    "el generador sin fallos produjo un comando inválido: {e:?} ({cmd:?})"
                )));
            }
        };
        prop_assert_eq!(doc.apply(cmd), Ok(cambio));
        if cambio {
            instantes.push(nueva);
        }
        comprueba_limite(doc, instantes, limite)?;
    }
    Ok(())
}

// `history_bytes()` es el coste de la pila de deshacer (fila 18 y propiedad 7).

/// Aplica `cmds` en orden, todos con `Ok(true)`, y comprueba que `history_bytes()` sube con
/// cada `apply`, baja con cada `undo` hasta 0 y sube con cada `redo` hasta el valor final,
/// pasando por los mismos valores. Devuelve esos valores (el primero es 0).
fn comprueba_bytes_de_la_pila(mut doc: Document, cmds: Vec<Command>) -> Vec<usize> {
    let mut bytes = vec![doc.history_bytes()];
    assert_eq!(bytes[0], 0);
    for cmd in cmds {
        assert_eq!(doc.apply(cmd), Ok(true));
        let ahora = doc.history_bytes();
        assert!(
            ahora > *bytes.last().unwrap(),
            "apply debe sumar el coste del paso"
        );
        bytes.push(ahora);
    }
    for (j, esperado) in bytes.iter().rev().skip(1).enumerate() {
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc.history_bytes(), *esperado, "tras {} undo", j + 1);
    }
    assert_eq!(doc.history_bytes(), 0);
    for (j, esperado) in bytes.iter().enumerate().skip(1) {
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc.history_bytes(), *esperado, "tras {j} redo");
    }
    bytes
}

#[test]
fn fila_18_history_bytes_sigue_la_pila_de_deshacer() {
    let id = doc_d().main_image();
    // Un `SetPixels`, un `FillSpans` y un `Batch` que mezcla ambos, uno tras otro.
    let bytes = comprueba_bytes_de_la_pila(
        doc_d(),
        vec![
            set(id, &[px(0, 0, c1())]),
            fr(id, &[sp(1, 0, 4), sp(2, 1, 3)], c2()),
            lote(vec![
                set(id, &[px(3, 0, c1())]),
                fr(id, &[sp(0, 0, 4)], c3()),
                set(id, &[px(2, 2, c1())]),
            ]),
        ],
    );
    assert_eq!(bytes.len(), 4);
}

#[test]
fn fila_18_un_solo_paso_de_cada_clase() {
    let id = doc_d().main_image();
    let casos = [
        set(id, &[px(0, 0, c1())]),
        fr(id, &[sp(0, 0, 4), sp(1, 0, 4), sp(2, 0, 4)], c1()),
        lote(vec![
            set(id, &[px(1, 1, c1())]),
            fr(id, &[sp(1, 0, 4)], c2()),
        ]),
    ];
    for cmd in casos {
        let mut doc = doc_d();
        assert_eq!(doc.apply(cmd), Ok(true));
        let coste = doc.history_bytes();
        assert!(coste > 0);
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(
            doc.history_bytes(),
            0,
            "tras undo la pila de deshacer está vacía"
        );
        assert!(doc.can_redo());
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(
            doc.history_bytes(),
            coste,
            "tras redo vuelve al coste del paso"
        );
    }
}

#[test]
fn fila_18_un_apply_tras_undo_cuenta_solo_su_paso() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c2())), Ok(true));
    let dos = doc.history_bytes();
    assert_eq!(doc.undo(), Ok(()));
    let uno = doc.history_bytes();
    assert!(uno < dos);
    // Un paso nuevo borra rehacer: la pila de deshacer pasa a tener el primero y el nuevo.
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c3())), Ok(true));
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), dos);
    // Un `apply` sin cambios y un `undo` fallido no tocan `history_bytes()`.
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c3())), Ok(false));
    assert_eq!(doc.history_bytes(), dos);
    let mut vacio = doc_d();
    assert_eq!(vacio.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(vacio.history_bytes(), 0);
}

// Propiedades.

proptest! {
    // Propiedad 1: central ampliada, con lotes mezclados y coordenadas solapadas.
    #[test]
    fn deshacer_todo_y_rehacer_todo_con_rellenos(
        img in imagen_6(),
        semilla in prop::collection::vec(cmd(false), 0..=3),
        cmds in prop::collection::vec(cmd(true), 0..=30),
    ) {
        let f = forma(&img);
        let inicial = documento_inicial(img, &semilla);
        let id = inicial.main_image();
        let mut doc = inicial.clone();

        let mut con_cambio = 0usize;
        for raw in &cmds {
            // Los que fallan se ignoran.
            if let Ok(true) = doc.apply(comando(f, id, raw)) {
                con_cambio += 1;
            }
        }
        let final_ = doc.clone();

        prop_assert_eq!(deshace_todo(&mut doc, cmds.len()), con_cambio);
        prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
        prop_assert_eq!(&doc, &inicial);
        prop_assert!(!doc.can_undo());

        prop_assert_eq!(rehace_todo(&mut doc, cmds.len()), con_cambio);
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
        prop_assert_eq!(&doc, &final_);
        prop_assert!(!doc.can_redo());
    }

    // Propiedad 2: la historia contra un modelo.
    #[test]
    fn historia_contra_un_modelo_con_rellenos(
        img in imagen_6(),
        semilla in prop::collection::vec(cmd(false), 0..=3),
        ops in prop::collection::vec(op(), 0..=40),
    ) {
        ejecuta_modelo(documento_inicial(img, &semilla), &ops)?;
    }

    // Propiedad 3: atomicidad y ausencia de pánicos con `FillSpans` arbitrarios.
    #[test]
    fn atomicidad_y_ausencia_de_panicos_con_rellenos(
        img in imagen_6(),
        semilla in prop::collection::vec(cmd(false), 0..=4),
        ops in prop::collection::vec(op_arb(), 0..=20),
    ) {
        let f = forma(&img);
        let mut doc = Document::new(img);
        let id = doc.main_image();
        for raw in &semilla {
            let _ = doc.apply(comando(f, id, raw));
        }

        for op in &ops {
            let antes = estado(&doc);
            match op {
                OpArb::Cmd(raw) => aplica_y_comprueba(&mut doc, comando(f, id, raw), &antes)?,
                OpArb::Arb(raw) => aplica_y_comprueba(&mut doc, arb_comando(f, id, raw), &antes)?,
                OpArb::Undo => match doc.undo() {
                    Ok(()) => prop_assert!(doc.can_redo()),
                    Err(e) => {
                        prop_assert_eq!(e, DocumentError::NothingToUndo);
                        prop_assert_eq!(estado(&doc), antes);
                    }
                },
                OpArb::Redo => match doc.redo() {
                    Ok(()) => prop_assert!(doc.can_undo()),
                    Err(e) => {
                        prop_assert_eq!(e, DocumentError::NothingToRedo);
                        prop_assert_eq!(estado(&doc), antes);
                    }
                },
            }
        }
    }

    // Propiedad 4, primera parte: un relleno nunca cuesta más que el `SetPixels`
    // equivalente (más un margen fijo), sobre cualquier contenido.
    #[test]
    fn el_relleno_no_cuesta_mas_que_set_pixels(
        img in imagen_6(),
        semilla in prop::collection::vec(cmd(false), 0..=4),
        masks in masks(),
        corte in any::<bool>(),
        valor in 0u8..4,
    ) {
        // Imagen con contenido previo (a menudo ruidoso), y una copia sin historia por rama.
        let previo = documento_inicial(img, &semilla);
        let imagen = imagen_de(&previo).clone();
        let f = forma(&imagen);
        let pixel = pixel_de(f.mode, f.colores, valor);
        let spans = spans_validos(f, &masks, corte);
        let pixeles: Vec<PixelEdit> = spans
            .iter()
            .flat_map(|t| (t.x_start..t.x_end).map(move |x| PixelEdit { x, y: t.y, pixel }))
            .collect();

        let mut con_relleno = Document::new(imagen.clone());
        let mut con_set = Document::new(imagen);
        let id = con_relleno.main_image();
        let r_relleno = con_relleno.apply(Command::FillSpans { image: id, spans, pixel });
        let r_set = con_set.apply(Command::SetPixels { image: id, pixels: pixeles });

        prop_assert_eq!(&r_relleno, &r_set);
        prop_assert_eq!(&con_relleno, &con_set);
        prop_assert!(
            con_relleno.history_bytes() <= con_set.history_bytes() + 128,
            "relleno {} B, SetPixels {} B",
            con_relleno.history_bytes(),
            con_set.history_bytes()
        );
    }

    // Propiedad 4, segunda parte: sobre fondo uniforme el coste depende de los trozos y
    // no de los píxeles (mismos tramos en las mismas filas con la mitad de ancho).
    #[test]
    fn el_coste_del_relleno_no_depende_del_ancho_de_los_tramos(
        img in imagen_6(),
        masks in masks(),
        corte in any::<bool>(),
        valor in 0u8..4,
    ) {
        let f = forma(&img);
        let pixel = pixel_de(f.mode, f.colores, valor);
        let spans = spans_validos(f, &masks, corte);
        let mitad: Vec<Span> = spans
            .iter()
            .map(|t| sp(t.y, t.x_start, t.x_start + ((t.x_end - t.x_start) / 2).max(1)))
            .collect();

        let mut completo = Document::new(img.clone());
        let mut reducido = Document::new(img);
        let id = completo.main_image();
        let r_completo = completo.apply(Command::FillSpans { image: id, spans, pixel });
        let r_reducido = reducido.apply(Command::FillSpans { image: id, spans: mitad, pixel });

        prop_assert_eq!(&r_completo, &r_reducido);
        prop_assert_eq!(completo.history_bytes(), reducido.history_bytes());
    }

    // Propiedad 5: el límite con el coste real, con el límite fijado antes de editar.
    #[test]
    fn limite_de_la_historia_con_rellenos(
        img in imagen_6(),
        limite in 0usize..=2000,
        cmds in prop::collection::vec(cmd(false), 0..=30),
    ) {
        let f = forma(&img);
        let mut doc = Document::new(img.clone());
        doc.set_history_limit(limite);
        let mut instantes = vec![img];
        aplica_validos(&mut doc, &mut instantes, f, &cmds, limite)?;
        comprueba_limite(&doc, &instantes, limite)?;
    }

    // Propiedad 7: `history_bytes()` es el coste de la pila de deshacer: baja con cada `undo`,
    // sube con cada `redo` y pasa por los mismos valores.
    #[test]
    fn history_bytes_sigue_la_pila_de_deshacer(
        img in imagen_6(),
        cmds in prop::collection::vec(cmd(false), 0..=12),
    ) {
        let f = forma(&img);
        let mut doc = Document::new(img);
        let id = doc.main_image();
        let mut bytes = vec![0usize];
        for raw in &cmds {
            if doc.apply(comando(f, id, raw)) == Ok(true) {
                bytes.push(doc.history_bytes());
            }
        }
        prop_assert!(bytes.windows(2).all(|w| w[0] < w[1]), "no crece: {bytes:?}");
        for esperado in bytes.iter().rev().skip(1) {
            prop_assert_eq!(doc.undo(), Ok(()));
            prop_assert_eq!(doc.history_bytes(), *esperado);
        }
        prop_assert_eq!(doc.history_bytes(), 0);
        for esperado in bytes.iter().skip(1) {
            prop_assert_eq!(doc.redo(), Ok(()));
            prop_assert_eq!(doc.history_bytes(), *esperado);
        }
    }

    // Propiedad 6: un `Batch` mixto es un paso.
    #[test]
    fn batch_mixto_es_un_paso(
        img in imagen_6(),
        semilla in prop::collection::vec(cmd(false), 0..=3),
        hojas in prop::collection::vec(hoja(false), 1..=6),
    ) {
        let f = forma(&img);
        let inicial = documento_inicial(img, &semilla);
        let id = inicial.main_image();
        let cmds = resuelve(f, id, &hojas);

        // Uno a uno.
        let mut uno_a_uno = inicial.clone();
        let mut alguno = false;
        for c in &cmds {
            match uno_a_uno.apply(c.clone()) {
                Ok(cambio) => alguno |= cambio,
                Err(e) => {
                    return Err(TestCaseError::fail(format!(
                        "un comando válido falló: {e:?} ({c:?})"
                    )))
                }
            }
        }

        // Como un Batch.
        let mut con_lote = inicial.clone();
        prop_assert_eq!(con_lote.apply(Command::Batch(cmds)), Ok(alguno));
        prop_assert_eq!(&con_lote, &uno_a_uno);

        if alguno {
            prop_assert!(con_lote.can_undo());
            prop_assert!(!con_lote.can_redo());
            prop_assert_eq!(pasos_deshacibles(&con_lote), 1);

            prop_assert_eq!(con_lote.undo(), Ok(()));
            prop_assert_eq!(&con_lote, &inicial);
            prop_assert!(!con_lote.can_undo());
            prop_assert!(con_lote.can_redo());

            prop_assert_eq!(con_lote.redo(), Ok(()));
            prop_assert_eq!(&con_lote, &uno_a_uno);
            prop_assert!(con_lote.can_undo());
            prop_assert!(!con_lote.can_redo());
        } else {
            prop_assert_eq!(estado(&con_lote), estado(&inicial));
        }
    }
}
