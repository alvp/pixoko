//! Tests de los siete comandos de edición de la paleta de una imagen indexada
//! (specs/hito-2-paleta-editar.md): `SetPaletteColor`, `MovePaletteEntry`, `PermutePalette`,
//! `AddPaletteEntry`, `RemovePaletteEntry`, `SetTransparentIndex` y `RotateColors`, con
//! `DocumentError::PaletteEdit(PaletteEditError)`.
//!
//! Una prueba por cada fila 1 a 19 de la tabla de casos límite (`fila_NN_...`; las filas 14,
//! 15, 17 y 18 se reparten en varias) y una propiedad por cada una de las ocho de «Tests»
//! (`propiedad_N_...`). Las cifras de las filas son las de la spec, calculadas a mano; no se
//! recalculan con el código. Los oráculos de las propiedades 1, 3 y 8 están escritos aquí
//! (`visible`, `resueltos`, `rota_a_mano`...): no llaman a `Palette::resolved` ni a
//! `Image::set_pixel` para calcular lo esperado, solo leen `palette().colors()`,
//! `transparent_index()` y `get_pixel`. El coste de la historia solo se mide con
//! `history_bytes()` y con las cotas de la spec: nunca se asume el valor de `size_of`.

use std::error::Error;
use std::fmt::{Debug, Display};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, Command, Document, DocumentError, Image, ImageId, Mode, Palette, PaletteEditError,
    Pixel, PixelEdit, Rgba, Span,
};

// Colores de la spec.

const R: Rgba = Rgba::new(255, 0, 0, 255);
const G: Rgba = Rgba::new(0, 255, 0, 255);
const B: Rgba = Rgba::new(0, 0, 255, 255);
const K: Rgba = Rgba::new(0, 0, 0, 255);
const Y: Rgba = Rgba::new(255, 255, 0, 255);
const W: Rgba = Rgba::new(255, 255, 255, 255);

// Construcción de imágenes, documentos y comandos.

/// Imagen indexada con la paleta, el transparente y los píxeles dados (fila a fila).
fn indexada(width: u32, height: u32, colores: &[Rgba], t: Option<u8>, pixeles: &[u8]) -> Image {
    assert_eq!(pixeles.len(), (width * height) as usize);
    let mut paleta = Palette::from_colors(colores.iter().copied()).unwrap();
    paleta.set_transparent_index(t).unwrap();
    let mut img = Image::new_indexed(width, height, paleta).unwrap();
    for (i, &v) in pixeles.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, Pixel::Indexed(v))
            .unwrap();
    }
    img
}

/// `F` de la spec: Indexed 4 x 1, paleta `[R,G,B,K]`, píxeles `[0,1,2,3]`.
fn img_f(t: Option<u8>) -> Image {
    indexada(4, 1, &[R, G, B, K], t, &[0, 1, 2, 3])
}

fn doc_f(t: Option<u8>) -> Document {
    Document::new(img_f(t))
}

/// `n` colores opacos distintos (n <= 256).
fn colores_distintos(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

/// Documento con una imagen indexada de `w` x `h` (todos los píxeles en el índice 0) y una
/// paleta de `n` colores distintos.
fn doc_con_paleta(w: u32, h: u32, n: usize, t: Option<u8>) -> Document {
    let pixeles = vec![0u8; (w * h) as usize];
    Document::new(indexada(w, h, &colores_distintos(n), t, &pixeles))
}

fn imagen_de(doc: &Document) -> &Image {
    doc.image(ImageId::from_raw(0)).unwrap()
}

fn paleta_de(img: &Image) -> Vec<Rgba> {
    img.palette().unwrap().colors().to_vec()
}

fn transparente_de(img: &Image) -> Option<u8> {
    img.palette().and_then(Palette::transparent_index)
}

fn longitud_de_paleta(img: &Image) -> usize {
    img.palette().map_or(0, Palette::len)
}

/// Los índices de los píxeles, fila a fila.
fn pixeles_de(img: &Image) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            match img.get_pixel(x, y) {
                Ok(Pixel::Indexed(i)) => v.push(i),
                otro => panic!("se esperaba un píxel indexado y salió {otro:?}"),
            }
        }
    }
    v
}

/// Color visible de una entrada: el guardado, con alfa 0 si es la transparente.
fn visible(color: Rgba, es_transparente: bool) -> Rgba {
    if es_transparente {
        Rgba::new(color.r, color.g, color.b, 0)
    } else {
        color
    }
}

/// Color resuelto de cada píxel, calculado aquí a partir de la paleta guardada.
fn resueltos(img: &Image) -> Vec<Rgba> {
    let colores = paleta_de(img);
    let t = transparente_de(img);
    pixeles_de(img)
        .into_iter()
        .map(|v| visible(colores[usize::from(v)], t == Some(v)))
        .collect()
}

fn set(image: ImageId, x: u32, y: u32, pixel: Pixel) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit { x, y, pixel }],
    }
}

fn pon_color(image: ImageId, index: u8, color: Rgba) -> Command {
    Command::SetPaletteColor {
        image,
        index,
        color,
    }
}

fn mueve(image: ImageId, from: u8, to: u8) -> Command {
    Command::MovePaletteEntry { image, from, to }
}

fn permuta(image: ImageId, order: &[u8]) -> Command {
    Command::PermutePalette {
        image,
        order: order.to_vec(),
    }
}

fn anade(image: ImageId, color: Rgba) -> Command {
    Command::AddPaletteEntry { image, color }
}

fn quita(image: ImageId, index: u8, into: u8) -> Command {
    Command::RemovePaletteEntry { image, index, into }
}

fn transparente(image: ImageId, index: Option<u8>) -> Command {
    Command::SetTransparentIndex { image, index }
}

fn rota(image: ImageId, start: u8, len: u16, steps: i32) -> Command {
    Command::RotateColors {
        image,
        start,
        len,
        steps,
    }
}

/// Los siete comandos, con argumentos válidos para una imagen indexada de 2 o más entradas.
fn siete(image: ImageId) -> Vec<Command> {
    vec![
        pon_color(image, 0, R),
        mueve(image, 0, 1),
        permuta(image, &[1, 0]),
        anade(image, W),
        quita(image, 0, 1),
        transparente(image, Some(0)),
        rota(image, 0, 2, 1),
    ]
}

// Errores.

fn edicion(e: PaletteEditError) -> DocumentError {
    DocumentError::PaletteEdit(e)
}

fn fuera(index: u8, len: usize) -> DocumentError {
    edicion(PaletteEditError::IndexOutOfRange { index, len })
}

fn rango_fuera(start: u8, len: u16, palette_len: usize) -> DocumentError {
    edicion(PaletteEditError::RangeOutOfPalette {
        start,
        len,
        palette_len,
    })
}

fn en_lote(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

// Estado observable del documento.

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
    state_id: u64,
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
        state_id: doc.state_id(),
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

/// `apply(cmd)` da `Ok(false)` y no toca nada observable (ni historia ni `state_id`).
fn sin_paso(doc: &mut Document, cmd: Command) {
    let antes = estado(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(false), "{cmd:?}");
    assert_eq!(estado(doc), antes, "{cmd:?}");
}

/// `apply(cmd)` da `Ok(true)`: un paso nuevo, con `state_id` nuevo y la pila de rehacer vacía.
fn con_paso(doc: &mut Document, cmd: Command) {
    let id_antes = doc.state_id();
    let pasos = pasos_deshacibles(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(true), "{cmd:?}");
    assert_ne!(doc.state_id(), id_antes, "{cmd:?}");
    assert!(doc.can_undo(), "{cmd:?}");
    assert!(!doc.can_redo(), "{cmd:?}");
    assert_eq!(pasos_deshacibles(doc), pasos + 1, "{cmd:?}");
}

/// Aplica `cmd` (que debe dar `Ok(true)`), lo deshace y lo rehace; comprueba que
/// `history_bytes()` tras `redo` es el de tras `apply` y que `undo` baja al de antes.
/// Devuelve cuánto subió `history_bytes()`.
fn coste_del_paso(doc: &mut Document, cmd: Command) -> usize {
    let antes = doc.history_bytes();
    assert_eq!(doc.apply(cmd.clone()), Ok(true), "{cmd:?}");
    let tras_apply = doc.history_bytes();
    assert!(tras_apply >= antes, "{cmd:?}");
    assert_eq!(doc.undo(), Ok(()), "{cmd:?}");
    assert_eq!(doc.history_bytes(), antes, "{cmd:?}");
    assert_eq!(doc.redo(), Ok(()), "{cmd:?}");
    assert_eq!(doc.history_bytes(), tras_apply, "{cmd:?}");
    tras_apply - antes
}

// Tipos nuevos de la API.

fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}

#[test]
fn tipos_nuevos_de_la_api() {
    assert_error_traits::<PaletteEditError>();
    let errores = [
        PaletteEditError::NotIndexed { mode: Mode::Rgba },
        PaletteEditError::IndexOutOfRange { index: 4, len: 4 },
        PaletteEditError::BadPermutation,
        PaletteEditError::PaletteFull,
        PaletteEditError::RemoveIntoSelf { index: 2 },
        PaletteEditError::LastEntry,
        PaletteEditError::RangeOutOfPalette {
            start: 2,
            len: 3,
            palette_len: 4,
        },
    ];
    for (i, e) in errores.iter().enumerate() {
        assert!(!e.to_string().is_empty(), "{e:?}");
        assert_eq!(e, &e.clone());
        for (j, otro) in errores.iter().enumerate() {
            assert_eq!(i == j, e == otro, "{e:?} frente a {otro:?}");
        }
        let doc = edicion(e.clone());
        assert!(!doc.to_string().is_empty());
        assert!(doc.source().is_some(), "PaletteEdit lleva su causa");
    }
    assert_ne!(fuera(4, 4), fuera(5, 4));
    assert_ne!(fuera(4, 4), fuera(4, 5));

    let id = ImageId::from_raw(0);
    for cmd in siete(id) {
        assert_eq!(cmd, cmd.clone());
        assert!(!format!("{cmd:?}").is_empty());
    }
    assert_ne!(pon_color(id, 1, R), pon_color(id, 2, R));
    assert_ne!(pon_color(id, 1, R), pon_color(id, 1, G));
    assert_ne!(mueve(id, 0, 1), mueve(id, 1, 0));
    assert_ne!(permuta(id, &[0, 1]), permuta(id, &[1, 0]));
    assert_ne!(quita(id, 0, 1), quita(id, 1, 0));
    assert_ne!(transparente(id, None), transparente(id, Some(0)));
    assert_ne!(rota(id, 0, 4, 1), rota(id, 0, 4, -1));
    assert_ne!(rota(id, 0, 4, 1), rota(id, 0, 3, 1));
    assert_ne!(rota(id, 0, 4, 1), rota(id, 1, 4, 1));
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_01_set_palette_color_y_deshacer() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = imagen_de(&doc).clone();

    con_paso(&mut doc, pon_color(id, 1, Y));
    assert_eq!(paleta_de(imagen_de(&doc)), vec![R, Y, B, K]);
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 1, 2, 3]);
    assert_eq!(resueltos(imagen_de(&doc)), vec![R, Y, B, K]);
    assert_eq!(transparente_de(imagen_de(&doc)), None);
    let editada = doc.clone();

    // `undo` deja una imagen igual a la original; `redo`, otra vez `[R,Y,B,K]`.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &original);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(paleta_de(imagen_de(&doc)), vec![R, Y, B, K]);
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 1, 2, 3]);
    assert_eq!(doc, editada);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn fila_02_mismo_color_fuera_de_rango_y_entrada_transparente() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);

    // Mismo color: `Ok(false)` sin paso.
    sin_paso(&mut doc, pon_color(id, 1, G));
    // Índice fuera de la paleta.
    rechaza(&mut doc, pon_color(id, 4, R), fuera(4, 4));

    // Sobre la entrada transparente: `Ok(true)`, el RGB guardado cambia aunque no se vea.
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    con_paso(&mut doc, pon_color(id, 2, Rgba::new(9, 9, 9, 255)));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img)[2], Rgba::new(9, 9, 9, 255));
    assert_eq!(resueltos(img)[2], Rgba::new(9, 9, 9, 0));
    assert_eq!(transparente_de(img), Some(2));
    assert_eq!(pixeles_de(img), vec![0, 1, 2, 3]);
}

#[test]
fn fila_03_permutar_con_transparente() {
    let mut doc = doc_f(Some(0));
    let id = ImageId::from_raw(0);
    let antes = resueltos(imagen_de(&doc));
    let esperados = vec![
        Rgba::new(255, 0, 0, 0),
        Rgba::new(0, 255, 0, 255),
        Rgba::new(0, 0, 255, 255),
        Rgba::new(0, 0, 0, 255),
    ];
    assert_eq!(antes, esperados);

    con_paso(&mut doc, permuta(id, &[3, 2, 1, 0]));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![K, B, G, R]);
    assert_eq!(pixeles_de(img), vec![3, 2, 1, 0]);
    assert_eq!(transparente_de(img), Some(3));
    assert_eq!(resueltos(img), esperados);
}

#[test]
fn fila_04_permutar_con_entradas_sin_usar() {
    let mut doc = Document::new(indexada(4, 1, &[R, G, B, K], None, &[0, 0, 2, 2]));
    let id = ImageId::from_raw(0);
    assert_eq!(resueltos(imagen_de(&doc)), vec![R, R, B, B]);

    con_paso(&mut doc, permuta(id, &[2, 0, 3, 1]));
    let img = imagen_de(&doc);
    // `adelante = {0->1, 1->3, 2->0, 3->2}`; G y K (sin usar) viajan a 3 y 2.
    assert_eq!(paleta_de(img), vec![B, R, K, G]);
    assert_eq!(pixeles_de(img), vec![1, 1, 0, 0]);
    assert_eq!(resueltos(img), vec![R, R, B, B]);
}

#[test]
fn fila_05_permutacion_identidad_y_mal_formadas() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = imagen_de(&doc).clone();

    sin_paso(&mut doc, permuta(id, &[0, 1, 2, 3]));
    let mala = edicion(PaletteEditError::BadPermutation);
    rechaza(&mut doc, permuta(id, &[0, 1, 2]), mala.clone());
    rechaza(&mut doc, permuta(id, &[0, 1, 1, 3]), mala.clone());
    rechaza(&mut doc, permuta(id, &[0, 1, 2, 4]), mala);
    assert_eq!(imagen_de(&doc), &original);
}

#[test]
fn fila_06_mover_con_transparente_equivale_a_permutar() {
    let mut doc = doc_f(Some(1));
    let id = ImageId::from_raw(0);
    let mut con_permute = doc.clone();

    con_paso(&mut doc, mueve(id, 0, 2));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![G, B, R, K]);
    assert_eq!(pixeles_de(img), vec![2, 0, 1, 3]);
    assert_eq!(transparente_de(img), Some(0));

    // Mismo documento final que `PermutePalette([1,2,0,3])`.
    con_paso(&mut con_permute, permuta(id, &[1, 2, 0, 3]));
    assert_eq!(imagen_de(&con_permute), imagen_de(&doc));
}

#[test]
fn fila_07_mover_sin_cambio_y_fuera_de_rango() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);

    con_paso(&mut doc, mueve(id, 3, 1));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![R, K, G, B]);
    assert_eq!(pixeles_de(img), vec![0, 2, 3, 1]);

    sin_paso(&mut doc, mueve(id, 2, 2));
    rechaza(&mut doc, mueve(id, 0, 4), fuera(4, 4));
    // Por orden de campo: `from` antes que `to`.
    rechaza(&mut doc, mueve(id, 5, 9), fuera(5, 4));
}

#[test]
fn fila_08_anadir_entrada() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = imagen_de(&doc).clone();

    con_paso(&mut doc, anade(id, W));
    let img = imagen_de(&doc);
    assert_eq!(img.palette().unwrap().len(), 5);
    assert_eq!(paleta_de(img), vec![R, G, B, K, W]);
    assert_eq!(pixeles_de(img), vec![0, 1, 2, 3]);
    assert_eq!(transparente_de(img), None);
    let anadida = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc).palette().unwrap().len(), 4);
    assert_eq!(imagen_de(&doc), &original);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc).palette().unwrap().len(), 5);
    assert_eq!(doc, anadida);

    // Con 256 entradas: `PaletteFull`, todo intacto.
    let mut llena = Document::new(indexada(2, 1, &colores_distintos(256), None, &[0, 255]));
    let id = ImageId::from_raw(0);
    rechaza(
        &mut llena,
        anade(id, W),
        edicion(PaletteEditError::PaletteFull),
    );
    assert_eq!(imagen_de(&llena).palette().unwrap().len(), 256);
}

#[test]
fn fila_08_anadir_transparente_no_lo_vuelve_el_indice_transparente() {
    for t in [None, Some(2)] {
        let mut doc = doc_f(t);
        let id = ImageId::from_raw(0);
        con_paso(&mut doc, anade(id, Rgba::TRANSPARENT));
        let img = imagen_de(&doc);
        assert_eq!(paleta_de(img), vec![R, G, B, K, Rgba::TRANSPARENT]);
        assert_eq!(transparente_de(img), t);
        assert_eq!(pixeles_de(img), vec![0, 1, 2, 3]);
    }
}

#[test]
fn fila_09_quitar_y_deshacer_con_la_mascara() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = imagen_de(&doc).clone();
    assert_eq!(resueltos(&original), vec![R, G, B, K]);

    con_paso(&mut doc, quita(id, 1, 0));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![R, B, K]);
    assert_eq!(pixeles_de(img), vec![0, 0, 1, 2]);
    assert_eq!(resueltos(img), vec![R, R, B, K]);
    let quitada = doc.clone();

    // `undo` restaura exactamente: el píxel 0 sigue en 0 y el 1 vuelve a 1.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &original);
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 1, 2, 3]);
    assert_eq!(resueltos(imagen_de(&doc)), vec![R, G, B, K]);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, quitada);
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 0, 1, 2]);
}

#[test]
fn fila_10_quitar_con_transparente_mayor_que_el_quitado() {
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    let original = imagen_de(&doc).clone();

    // `into' = 3 - 1 = 2`.
    con_paso(&mut doc, quita(id, 0, 3));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![G, B, K]);
    assert_eq!(pixeles_de(img), vec![2, 0, 1, 2]);
    assert_eq!(transparente_de(img), Some(1));
    assert_eq!(resueltos(img), vec![K, G, Rgba::new(0, 0, 255, 0), K]);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &original);
}

#[test]
fn fila_11_quitar_la_transparente_y_quitar_hacia_la_transparente() {
    // Se quita la transparente: deja de haber transparente y sus píxeles toman el color de
    // `into`, ahora opaco.
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    con_paso(&mut doc, quita(id, 2, 0));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![R, G, K]);
    assert_eq!(pixeles_de(img), vec![0, 1, 0, 2]);
    assert_eq!(transparente_de(img), None);
    assert_eq!(resueltos(img)[2], R);

    // `into` es la transparente: los píxeles quedan transparentes.
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    con_paso(&mut doc, quita(id, 0, 2));
    let img = imagen_de(&doc);
    // `into' = 2 - 1 = 1`.
    assert_eq!(paleta_de(img), vec![G, B, K]);
    assert_eq!(pixeles_de(img), vec![1, 0, 1, 2]);
    assert_eq!(transparente_de(img), Some(1));
    let transparente_b = Rgba::new(0, 0, 255, 0);
    assert_eq!(resueltos(img)[0], transparente_b);
    assert_eq!(resueltos(img)[2], transparente_b);
}

#[test]
fn fila_12_quitar_errores_y_su_orden() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    rechaza(&mut doc, quita(id, 4, 0), fuera(4, 4));
    rechaza(&mut doc, quita(id, 0, 4), fuera(4, 4));
    // `index` antes que `into`.
    rechaza(&mut doc, quita(id, 5, 7), fuera(5, 4));
    rechaza(
        &mut doc,
        quita(id, 2, 2),
        edicion(PaletteEditError::RemoveIntoSelf { index: 2 }),
    );

    // Imagen 2 x 1 de una sola entrada.
    let mut una = Document::new(indexada(2, 1, &[R], None, &[0, 0]));
    let id = ImageId::from_raw(0);
    // `LastEntry` gana a `RemoveIntoSelf`.
    rechaza(
        &mut una,
        quita(id, 0, 0),
        edicion(PaletteEditError::LastEntry),
    );
    // `IndexOutOfRange` de `into` gana a `LastEntry`.
    rechaza(&mut una, quita(id, 0, 1), fuera(1, 1));
}

#[test]
fn fila_13_transparente_fijar_y_quitar() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);

    con_paso(&mut doc, transparente(id, Some(2)));
    let img = imagen_de(&doc);
    assert_eq!(pixeles_de(img), vec![0, 1, 2, 3]);
    assert_eq!(transparente_de(img), Some(2));
    assert_eq!(resueltos(img)[2], Rgba::new(0, 0, 255, 0));

    sin_paso(&mut doc, transparente(id, Some(2)));
    con_paso(&mut doc, transparente(id, Some(1)));
    assert_eq!(transparente_de(imagen_de(&doc)), Some(1));
    con_paso(&mut doc, transparente(id, None));
    assert_eq!(transparente_de(imagen_de(&doc)), None);
    assert_eq!(resueltos(imagen_de(&doc)), vec![R, G, B, K]);
    sin_paso(&mut doc, transparente(id, None));
    rechaza(&mut doc, transparente(id, Some(4)), fuera(4, 4));
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 1, 2, 3]);
}

#[test]
fn fila_14_imagen_no_indexada() {
    for mode in [Mode::Rgba, Mode::Gray] {
        let mut doc = Document::new(Image::new(2, 1, mode).unwrap());
        let id = ImageId::from_raw(0);
        let mala = edicion(PaletteEditError::NotIndexed { mode });
        // Los siete comandos, con argumentos válidos y con argumentos inválidos: el modo
        // se comprueba antes que los argumentos.
        let mut comandos = siete(id);
        comandos.extend([
            pon_color(id, 200, R),
            mueve(id, 9, 9),
            permuta(id, &[0, 0, 0]),
            quita(id, 0, 0),
            transparente(id, Some(200)),
            rota(id, 200, 200, 1),
        ]);
        for cmd in comandos {
            rechaza(&mut doc, cmd, mala.clone());
        }
    }
}

#[test]
fn fila_14_imagen_desconocida() {
    let mut doc = doc_f(None);
    let desconocida = ImageId::from_raw(99);
    let mala = DocumentError::UnknownImage(desconocida);
    let mut comandos = siete(desconocida);
    comandos.extend([
        pon_color(desconocida, 200, R),
        permuta(desconocida, &[0, 0, 0]),
        quita(desconocida, 2, 2),
        rota(desconocida, 200, 200, 1),
    ]);
    for cmd in comandos {
        rechaza(&mut doc, cmd, mala.clone());
    }
}

#[test]
fn fila_14_dentro_de_un_batch_no_se_admiten() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let desconocida = ImageId::from_raw(99);
    let original = imagen_de(&doc).clone();

    // El `SetPixels` anterior se revierte.
    let lote = Command::Batch(vec![set(id, 0, 0, Pixel::Indexed(3)), pon_color(id, 1, Y)]);
    rechaza(&mut doc, lote, en_lote(1, DocumentError::NotBatchable));
    assert_eq!(imagen_de(&doc), &original);
    assert!(!doc.can_undo());
    assert_eq!(doc.history_bytes(), 0);

    // `NotBatchable` gana a `UnknownImage`.
    let lote = Command::Batch(vec![permuta(desconocida, &[0, 1, 2, 3])]);
    rechaza(&mut doc, lote, en_lote(0, DocumentError::NotBatchable));
    // ... y a `RemoveIntoSelf`.
    let lote = Command::Batch(vec![quita(id, 2, 2)]);
    rechaza(&mut doc, lote, en_lote(0, DocumentError::NotBatchable));
    // Un lote con un `RotateColors` válido también se rechaza.
    let lote = Command::Batch(vec![rota(id, 0, 4, 1)]);
    rechaza(&mut doc, lote, en_lote(0, DocumentError::NotBatchable));

    // Los siete, solos y detrás de un `SetPixels` válido.
    for cmd in siete(id) {
        let lote = Command::Batch(vec![cmd.clone()]);
        rechaza(&mut doc, lote, en_lote(0, DocumentError::NotBatchable));
        let lote = Command::Batch(vec![set(id, 0, 0, Pixel::Indexed(3)), cmd]);
        rechaza(&mut doc, lote, en_lote(1, DocumentError::NotBatchable));
    }

    // Con historia previa y pila de rehacer, tampoco cambia nada.
    assert_eq!(doc.apply(set(id, 1, 0, Pixel::Indexed(0))), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let lote = Command::Batch(vec![mueve(id, 0, 1)]);
    rechaza(&mut doc, lote, en_lote(0, DocumentError::NotBatchable));
    assert!(doc.can_redo());
}

#[test]
fn fila_15_permute_cuesta_lo_mismo_en_cualquier_tamano() {
    let order: Vec<u8> = (1..16).chain(std::iter::once(0)).collect();
    let mut pequeno = doc_con_paleta(8, 8, 16, None);
    let mut grande = doc_con_paleta(512, 512, 16, None);
    let id = ImageId::from_raw(0);

    let coste_pequeno = coste_del_paso(&mut pequeno, permuta(id, &order));
    let coste_grande = coste_del_paso(&mut grande, permuta(id, &order));
    assert_eq!(coste_pequeno, coste_grande);
    assert!(
        (16..=512).contains(&coste_pequeno),
        "history_bytes() subió {coste_pequeno}"
    );
}

#[test]
fn fila_15_remove_sin_usar_y_remove_usado() {
    // Entrada sin usar en 512 x 512: no hay máscara.
    let mut doc = doc_con_paleta(512, 512, 16, None);
    let id = ImageId::from_raw(0);
    let coste = coste_del_paso(&mut doc, quita(id, 5, 0));
    assert!(coste <= 512, "history_bytes() subió {coste}");

    // Entrada usada en 64 x 64: máscara de `ceil(4096/64) * 8 = 512` bytes.
    let mut img = imagen_de(&doc_con_paleta(64, 64, 16, None)).clone();
    img.set_pixel(0, 0, Pixel::Indexed(1)).unwrap();
    let mut doc = Document::new(img);
    let id = ImageId::from_raw(0);
    let coste = coste_del_paso(&mut doc, quita(id, 1, 0));
    assert!(
        (512..=1024).contains(&coste),
        "history_bytes() subió {coste}"
    );
}

#[test]
fn fila_15_rotate_colors_cuesta_lo_mismo_en_cualquier_tamano() {
    let mut pequeno = doc_con_paleta(8, 8, 16, None);
    let mut grande = doc_con_paleta(512, 512, 16, None);
    let id = ImageId::from_raw(0);

    let coste_pequeno = coste_del_paso(&mut pequeno, rota(id, 0, 16, 1));
    let coste_grande = coste_del_paso(&mut grande, rota(id, 0, 16, 1));
    assert_eq!(coste_pequeno, coste_grande);
    assert!(
        coste_pequeno <= 512,
        "history_bytes() subió {coste_pequeno}"
    );
}

#[test]
fn fila_15_la_mascara_cuenta_en_el_limite_de_la_historia() {
    let mut doc = doc_con_paleta(64, 64, 16, None);
    let id = ImageId::from_raw(0);
    doc.set_history_limit(300);

    // El `Permute` cuesta menos de 300 y cabe. Con `order[nuevo] = antiguo`, el índice
    // antiguo 0 (el de todos los píxeles) pasa a ser el 15.
    let order: Vec<u8> = (1..16).chain(std::iter::once(0)).collect();
    assert_eq!(doc.apply(permuta(id, &order)), Ok(true));
    let tras_permute_bytes = doc.history_bytes();
    assert!(
        (16..300).contains(&tras_permute_bytes),
        "history_bytes() = {tras_permute_bytes}"
    );
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert!(pixeles_de(imagen_de(&doc)).iter().all(|&v| v == 15));
    let tras_permute = imagen_de(&doc).clone();

    // El `Remove` de una entrada usada guarda una máscara de 512 bytes: supera el límite,
    // se descarta el `Permute` y queda solo el `Remove`.
    assert_eq!(doc.apply(quita(id, 15, 0)), Ok(true));
    assert!(
        (512..=1024).contains(&doc.history_bytes()),
        "history_bytes() = {}",
        doc.history_bytes()
    );
    assert_eq!(pasos_deshacibles(&doc), 1, "queda solo el Remove");
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &tras_permute);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc).palette().unwrap().len(), 15);
}

#[test]
fn fila_16_rotar_colores() {
    // (comando, paleta esperada)
    let casos: [((u8, u16, i32), [Rgba; 4]); 5] = [
        ((0, 4, 1), [K, R, G, B]),
        ((1, 3, 1), [R, K, G, B]),
        ((0, 4, -1), [G, B, K, R]),
        ((1, 3, -1), [R, B, K, G]),
        ((0, 4, 5), [K, R, G, B]),
    ];
    for ((start, len, steps), esperada) in casos {
        let mut doc = doc_f(None);
        let id = ImageId::from_raw(0);
        let original = imagen_de(&doc).clone();

        con_paso(&mut doc, rota(id, start, len, steps));
        let img = imagen_de(&doc);
        assert_eq!(paleta_de(img), esperada.to_vec(), "{start} {len} {steps}");
        // Los píxeles conservan su índice y resuelven a la paleta nueva en ese orden.
        assert_eq!(pixeles_de(img), vec![0, 1, 2, 3], "{start} {len} {steps}");
        assert_eq!(resueltos(img), esperada.to_vec(), "{start} {len} {steps}");
        let rotada = doc.clone();

        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(imagen_de(&doc), &original, "{start} {len} {steps}");
        assert_eq!(paleta_de(imagen_de(&doc)), vec![R, G, B, K]);
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, rotada, "{start} {len} {steps}");
        assert_eq!(paleta_de(imagen_de(&doc)), esperada.to_vec());
    }
}

#[test]
fn fila_17_rotaciones_que_no_hacen_nada() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    // Múltiplos de `len` (también negativos), `len < 2` y rangos vacíos válidos (`start = 4`
    // es la longitud de la paleta).
    sin_paso(&mut doc, rota(id, 0, 4, 4));
    sin_paso(&mut doc, rota(id, 0, 4, -8));
    sin_paso(&mut doc, rota(id, 0, 4, 0));
    sin_paso(&mut doc, rota(id, 1, 1, 3));
    sin_paso(&mut doc, rota(id, 2, 0, 1));
    sin_paso(&mut doc, rota(id, 4, 0, 1));
    assert_eq!(imagen_de(&doc), &img_f(None));
}

#[test]
fn fila_17_steps_i32_min_sin_panico() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    // `i32::MIN = -2^31`, `2^31 mod 3 = 2`, luego `k = 1` y `[R,G,B]` pasa a `[B,R,G]`.
    con_paso(&mut doc, rota(id, 0, 3, i32::MIN));
    assert_eq!(paleta_de(imagen_de(&doc)), vec![B, R, G, K]);
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 1, 2, 3]);
}

#[test]
fn fila_17_paleta_con_colores_iguales_y_paleta_de_256() {
    // La rotación deja la paleta igual: `Ok(false)`.
    let mut doc = Document::new(indexada(4, 1, &[R, R, R, R], None, &[0, 1, 2, 3]));
    let id = ImageId::from_raw(0);
    sin_paso(&mut doc, rota(id, 0, 4, 1));

    // 256 entradas distintas: `len = 256` cabe en `u16`.
    let colores = colores_distintos(256);
    let mut doc = Document::new(indexada(2, 1, &colores, None, &[0, 255]));
    let id = ImageId::from_raw(0);
    // `-2^31` es múltiplo de 256.
    sin_paso(&mut doc, rota(id, 0, 256, i32::MIN));
    con_paso(&mut doc, rota(id, 0, 256, 1));
    let nueva = paleta_de(imagen_de(&doc));
    assert_eq!(nueva[0], colores[255]);
    for i in 1..256 {
        assert_eq!(nueva[i], colores[i - 1], "entrada {i}");
    }
    assert_eq!(pixeles_de(imagen_de(&doc)), vec![0, 255]);
}

#[test]
fn fila_18_rango_fuera_de_la_paleta() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    // 2 + 3 = 5 > 4.
    rechaza(&mut doc, rota(id, 2, 3, 1), rango_fuera(2, 3, 4));
    rechaza(&mut doc, rota(id, 5, 1, 1), rango_fuera(5, 1, 4));
    rechaza(&mut doc, rota(id, 0, 5, 1), rango_fuera(0, 5, 4));
    // 4 + 1 > 4.
    rechaza(&mut doc, rota(id, 4, 1, 1), rango_fuera(4, 1, 4));
}

#[test]
fn fila_18_el_error_de_rango_gana_a_ok_false() {
    // Un rango fuera de la paleta sigue siendo error aunque la rotación no hiciera nada.
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    rechaza(&mut doc, rota(id, 0, 5, 0), rango_fuera(0, 5, 4));
    rechaza(&mut doc, rota(id, 5, 0, 1), rango_fuera(5, 0, 4));
    rechaza(&mut doc, rota(id, 3, 2, 2), rango_fuera(3, 2, 4));
}

#[test]
fn fila_19_el_transparente_se_queda_en_su_ranura() {
    let mut doc = doc_f(Some(3));
    let id = ImageId::from_raw(0);
    let original = imagen_de(&doc).clone();
    assert_eq!(resueltos(&original), vec![R, G, B, Rgba::new(0, 0, 0, 0)]);

    con_paso(&mut doc, rota(id, 0, 4, 1));
    let img = imagen_de(&doc);
    assert_eq!(paleta_de(img), vec![K, R, G, B]);
    assert_eq!(transparente_de(img), Some(3));
    assert_eq!(pixeles_de(img), vec![0, 1, 2, 3]);
    assert_eq!(
        resueltos(img),
        vec![
            Rgba::new(0, 0, 0, 255),
            Rgba::new(255, 0, 0, 255),
            Rgba::new(0, 255, 0, 255),
            Rgba::new(0, 0, 255, 0),
        ]
    );

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &original);
}

// Estrategias y datos aleatorios.

/// Conjunto pequeño de colores para provocar duplicados y alfa 0.
const CONJUNTO: [Rgba; 8] = [
    R,
    G,
    B,
    K,
    W,
    Rgba::TRANSPARENT,
    Rgba::new(255, 0, 0, 0),
    Rgba::new(0, 255, 0, 128),
];

fn color_p() -> impl Strategy<Value = Rgba> {
    prop::sample::select(CONJUNTO.to_vec())
}

/// Longitud de paleta: de 1 a 16 y, en uno de cada tres casos, 255 o 256.
fn longitud() -> impl Strategy<Value = usize> {
    prop_oneof![
        2 => 1usize..=16,
        1 => prop::sample::select(vec![255usize, 256]),
    ]
}

/// El último índice de una paleta de `n` entradas.
fn ultimo(n: usize) -> u8 {
    u8::try_from(n.saturating_sub(1)).unwrap_or(u8::MAX)
}

/// Índice de píxel válido para una paleta de `n` entradas, con sesgo hacia el último.
fn indice_de_pixel(n: usize) -> impl Strategy<Value = u8> {
    prop_oneof![
        3 => (0..n).prop_map(|v| v as u8),
        1 => Just(ultimo(n)),
    ]
}

/// Imagen indexada de hasta `max` x `max`, con paleta de colores del conjunto pequeño y,
/// a veces, índice transparente.
fn imagen_idx(max: u32) -> impl Strategy<Value = Image> {
    (1..=max, 1..=max, longitud())
        .prop_flat_map(|(w, h, n)| {
            (
                Just((w, h)),
                prop::collection::vec(color_p(), n),
                prop::option::of(0..n),
                prop::collection::vec(indice_de_pixel(n), (w * h) as usize),
            )
        })
        .prop_map(|((w, h), colores, t, pixeles)| {
            indexada(w, h, &colores, t.map(|t| t as u8), &pixeles)
        })
}

/// Una imagen de 6 x 6 como mucho: casi siempre indexada y, a veces, de otro modo.
fn imagen_mixta() -> impl Strategy<Value = Image> {
    prop_oneof![
        9 => imagen_idx(6),
        1 => (
            prop::sample::select(vec![Mode::Rgba, Mode::Gray, Mode::GrayAlpha]),
            1u32..=6,
            1u32..=6,
        )
            .prop_map(|(mode, w, h)| Image::new(w, h, mode).unwrap()),
    ]
}

/// Índice `< n` con sesgo hacia el último.
fn ajusta(v: u16, n: usize) -> usize {
    if v.is_multiple_of(4) {
        n.saturating_sub(1)
    } else {
        usize::from(v / 4) % n.max(1)
    }
}

/// `u8` para un índice de una paleta de `n` entradas: casi siempre válido, y a veces el
/// límite (`n`, que está fuera) o 255.
fn indice_u8(v: u16, n: usize) -> u8 {
    match v % 8 {
        0 => ultimo(n),
        1 => u8::try_from(n).unwrap_or(u8::MAX),
        2 => u8::MAX,
        _ => u8::try_from(usize::from(v / 8) % n.max(1)).unwrap_or(u8::MAX),
    }
}

/// Permutación de `0..n` a partir de una semilla (Fisher-Yates con un generador lineal).
fn permutacion_de(n: usize, semilla: u64) -> Vec<u8> {
    let mut v: Vec<u8> = (0..n).map(|i| i as u8).collect();
    let mut s = semilla;
    for i in (1..n).rev() {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = ((s >> 33) as usize) % (i + 1);
        v.swap(i, j);
    }
    v
}

/// `order` de `PermutePalette`: según `kind`, una permutación, la identidad o una lista
/// mal formada (corta, larga, con un repetido o con un valor fuera de la paleta).
fn orden_de(n: usize, semilla: u64, kind: u8) -> Vec<u8> {
    let mut o = permutacion_de(n, semilla);
    match kind % 8 {
        1 => {
            o.pop();
        }
        2 => o.push(0),
        3 => {
            if let Some(&u) = o.last() {
                o[0] = u;
            }
        }
        4 => {
            if let Some(primero) = o.first_mut() {
                *primero = u8::try_from(n).unwrap_or(u8::MAX);
            }
        }
        5 => o = (0..n).map(|i| i as u8).collect(),
        _ => {}
    }
    o
}

/// `order` de un `Move`: `[0..n)` con `Vec::remove` e `insert`.
fn orden_de_mover(n: usize, from: usize, to: usize) -> Vec<u8> {
    let mut v: Vec<u8> = (0..n).map(|i| i as u8).collect();
    let x = v.remove(from);
    v.insert(to, x);
    v
}

fn pasos() -> impl Strategy<Value = i32> {
    prop_oneof![
        3 => any::<i32>(),
        3 => -20i32..=20,
        1 => Just(i32::MIN),
        1 => Just(i32::MAX),
    ]
}

/// Números en bruto para fabricar un comando de paleta contra una imagen concreta.
#[derive(Clone, Debug)]
struct Crudo {
    a: u16,
    b: u16,
    seed: u64,
    steps: i32,
    color: Rgba,
    kind: u8,
}

fn crudo() -> impl Strategy<Value = Crudo> {
    (
        any::<u16>(),
        any::<u16>(),
        any::<u64>(),
        pasos(),
        color_p(),
        0u8..8,
    )
        .prop_map(|(a, b, seed, steps, color, kind)| Crudo {
            a,
            b,
            seed,
            steps,
            color,
            kind,
        })
}

/// Un comando de paleta (el `tipo` elige uno de los siete) para una paleta de `n` entradas;
/// valido casi siempre y, a veces, con un índice, una permutación o un rango inválidos.
fn comando_paleta(tipo: u8, c: &Crudo, id: ImageId, n: usize) -> Command {
    let n = n.max(1);
    match tipo % 7 {
        0 => pon_color(id, indice_u8(c.a, n), c.color),
        1 => mueve(id, indice_u8(c.a, n), indice_u8(c.b, n)),
        2 => permuta(id, &orden_de(n, c.seed, c.kind)),
        3 => anade(id, c.color),
        4 => quita(id, indice_u8(c.a, n), indice_u8(c.b, n)),
        5 => transparente(
            id,
            if c.kind.is_multiple_of(4) {
                None
            } else {
                Some(indice_u8(c.a, n))
            },
        ),
        _ => {
            let start = indice_u8(c.a, n);
            let resto = n.saturating_sub(usize::from(start));
            let len = match c.b % 6 {
                0 => resto,
                1 => resto + 1,
                2 => 0,
                3 => 1,
                _ => usize::from(c.b / 6) % (resto + 1),
            };
            rota(id, start, u16::try_from(len).unwrap_or(u16::MAX), c.steps)
        }
    }
}

/// Rota a mano el trozo `start..start + len` de la paleta `k` posiciones hacia los índices
/// mayores: `nueva[start + i] = vieja[start + (i + len - k) mod len]`.
fn rota_a_mano(vieja: &[Rgba], start: usize, len: usize, k: usize) -> Vec<Rgba> {
    let mut nueva = vieja.to_vec();
    for i in 0..len {
        nueva[start + i] = vieja[start + (i + len - k) % len];
    }
    nueva
}

/// `steps` módulo `len` (siempre en `0..len`; 0 si `len` es 0).
fn modulo(steps: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let k = i64::from(steps).rem_euclid(len as i64);
    usize::try_from(k).unwrap_or(0)
}

/// Un píxel de la imagen actual, válido para su modo o (si `malo`) de otro modo.
fn pixel_para(img: &Image, v: u8, malo: bool) -> Pixel {
    let n = longitud_de_paleta(img).max(1);
    let alfa = if v.is_multiple_of(2) { 0 } else { 255 };
    if malo {
        return match img.mode() {
            Mode::Gray => Pixel::Rgba(Rgba::TRANSPARENT),
            _ => Pixel::Gray(v),
        };
    }
    match img.mode() {
        Mode::Rgba => Pixel::Rgba(Rgba::new(v, v ^ 0x55, v.wrapping_mul(3), alfa)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: v,
            alpha: alfa,
        },
        Mode::Gray => Pixel::Gray(v),
        Mode::Indexed => Pixel::Indexed((usize::from(v) % n) as u8),
    }
}

/// Documento con una imagen y algo de historia: un `SetPixels` válido y, según `deshacer`,
/// un `undo` que deja un paso por rehacer.
fn doc_con_historia(img: Image, v: u8, deshacer: bool) -> Document {
    let mut doc = Document::new(img);
    let id = ImageId::from_raw(0);
    let pixel = pixel_para(imagen_de(&doc), v, false);
    let _ = doc.apply(set(id, 0, 0, pixel));
    if deshacer {
        let _ = doc.undo();
    }
    doc
}

/// La invariante central: ningún píxel `>= len` de la paleta y el transparente es `None` o
/// `< len`, mirado solo por la API pública (en cualquier imagen indexada).
fn comprueba_invariante(doc: &Document, id: ImageId) -> Result<(), TestCaseError> {
    let Some(img) = doc.image(id) else {
        return Ok(());
    };
    let Some(paleta) = img.palette() else {
        return Ok(());
    };
    let n = paleta.len();
    prop_assert!(
        paleta
            .transparent_index()
            .is_none_or(|t| usize::from(t) < n),
        "transparente {:?} con {} entradas",
        paleta.transparent_index(),
        n
    );
    for y in 0..img.height() {
        for x in 0..img.width() {
            match img.get_pixel(x, y) {
                Ok(Pixel::Indexed(v)) => {
                    prop_assert!(usize::from(v) < n, "píxel {} con {} entradas", v, n)
                }
                otro => prop_assert!(false, "píxel inesperado {:?}", otro),
            }
        }
    }
    Ok(())
}

// Predicado independiente de la validación (propiedad 5).

enum Esperado {
    Err(DocumentError),
    SinPaso,
    Paso,
}

/// Lo que debe pasar al aplicar `cmd` (un comando de paleta suelto) según los puntos 4 y 5
/// de «Comportamiento». `img` es `None` si el identificador no existe.
fn predice(img: Option<&Image>, id: ImageId, cmd: &Command) -> Esperado {
    let Some(img) = img else {
        return Esperado::Err(DocumentError::UnknownImage(id));
    };
    if img.mode() != Mode::Indexed {
        return Esperado::Err(edicion(PaletteEditError::NotIndexed { mode: img.mode() }));
    }
    let colores = paleta_de(img);
    let n = colores.len();
    let t = transparente_de(img);
    let paso = |cambia: bool| {
        if cambia {
            Esperado::Paso
        } else {
            Esperado::SinPaso
        }
    };
    match cmd {
        Command::SetPaletteColor { index, color, .. } => {
            if usize::from(*index) >= n {
                return Esperado::Err(fuera(*index, n));
            }
            paso(colores[usize::from(*index)] != *color)
        }
        Command::MovePaletteEntry { from, to, .. } => {
            if usize::from(*from) >= n {
                return Esperado::Err(fuera(*from, n));
            }
            if usize::from(*to) >= n {
                return Esperado::Err(fuera(*to, n));
            }
            paso(from != to)
        }
        Command::PermutePalette { order, .. } => {
            let mut visto = vec![false; n];
            let mut valida = order.len() == n;
            for &o in order {
                match visto.get_mut(usize::from(o)) {
                    Some(v) if !*v => *v = true,
                    _ => valida = false,
                }
            }
            if !valida {
                return Esperado::Err(edicion(PaletteEditError::BadPermutation));
            }
            let identidad = order.iter().enumerate().all(|(i, &o)| usize::from(o) == i);
            paso(!identidad)
        }
        Command::AddPaletteEntry { .. } => {
            if n >= 256 {
                return Esperado::Err(edicion(PaletteEditError::PaletteFull));
            }
            Esperado::Paso
        }
        Command::RemovePaletteEntry { index, into, .. } => {
            if usize::from(*index) >= n {
                return Esperado::Err(fuera(*index, n));
            }
            if usize::from(*into) >= n {
                return Esperado::Err(fuera(*into, n));
            }
            if n == 1 {
                return Esperado::Err(edicion(PaletteEditError::LastEntry));
            }
            if index == into {
                return Esperado::Err(edicion(PaletteEditError::RemoveIntoSelf { index: *index }));
            }
            Esperado::Paso
        }
        Command::SetTransparentIndex { index, .. } => {
            if let Some(i) = index
                && usize::from(*i) >= n
            {
                return Esperado::Err(fuera(*i, n));
            }
            paso(*index != t)
        }
        Command::RotateColors {
            start, len, steps, ..
        } => {
            let (s, l) = (usize::from(*start), usize::from(*len));
            if s + l > n {
                return Esperado::Err(rango_fuera(*start, *len, n));
            }
            if l < 2 {
                return Esperado::SinPaso;
            }
            let k = modulo(*steps, l);
            if k == 0 {
                return Esperado::SinPaso;
            }
            paso(rota_a_mano(&colores, s, l, k) != colores)
        }
        otro => panic!("comando que no es de paleta: {otro:?}"),
    }
}

// Propiedad 1: oráculo independiente de reordenar.

/// La imagen que debe salir de reordenar con `order` (`order[nuevo] = antiguo`): paleta con
/// `order.map`, y píxeles y transparente con una búsqueda lineal del nuevo índice.
fn oraculo_reordenar(img: &Image, order: &[u8]) -> Image {
    let vieja = paleta_de(img);
    let nueva: Vec<Rgba> = order.iter().map(|&o| vieja[usize::from(o)]).collect();
    let nuevo_indice = |v: u8| -> u8 {
        let pos = order.iter().position(|&o| o == v).unwrap();
        u8::try_from(pos).unwrap()
    };
    let pixeles: Vec<u8> = pixeles_de(img).into_iter().map(nuevo_indice).collect();
    let t = transparente_de(img).map(nuevo_indice);
    indexada(img.width(), img.height(), &nueva, t, &pixeles)
}

/// Aplica `cmd` (equivalente a `order`) y `ida_y_vuelta` (que lo deshace) y compara con el
/// oráculo.
fn comprueba_reordenar(
    img: &Image,
    cmd: Command,
    order: &[u8],
    ida_y_vuelta: Command,
) -> Result<(), TestCaseError> {
    let identidad = order.iter().enumerate().all(|(i, &o)| usize::from(o) == i);
    let esperada = oraculo_reordenar(img, order);
    let antes = resueltos(img);

    let mut doc = Document::new(img.clone());
    let real = doc.apply(cmd);
    prop_assert_eq!(real, Ok(!identidad));
    // (a) La imagen coincide exactamente: píxeles, paleta y transparente.
    prop_assert_eq!(imagen_de(&doc), &esperada);
    prop_assert_eq!(pixeles_de(imagen_de(&doc)), pixeles_de(&esperada));
    prop_assert_eq!(paleta_de(imagen_de(&doc)), paleta_de(&esperada));
    prop_assert_eq!(transparente_de(imagen_de(&doc)), transparente_de(&esperada));
    // (b) Ningún color visible cambia.
    prop_assert_eq!(resueltos(imagen_de(&doc)), antes);
    comprueba_invariante(&doc, ImageId::from_raw(0))?;

    // Ida y vuelta.
    prop_assert_eq!(doc.apply(ida_y_vuelta), Ok(!identidad));
    prop_assert_eq!(imagen_de(&doc), img);
    Ok(())
}

// Propiedad 2: `Move` equivale a `Permute`.

fn prop_mover_es_permutar(img: Image, a: u16, b: u16) -> Result<(), TestCaseError> {
    let n = longitud_de_paleta(&img);
    let (from, to) = (ajusta(a, n), ajusta(b, n));
    let order = orden_de_mover(n, from, to);
    let id = ImageId::from_raw(0);

    let mut con_move = Document::new(img.clone());
    let mut con_permute = Document::new(img.clone());
    let r_move = con_move.apply(mueve(id, from as u8, to as u8));
    let r_permute = con_permute.apply(permuta(id, &order));
    prop_assert_eq!(&r_move, &Ok(from != to));
    prop_assert_eq!(&r_move, &r_permute);
    prop_assert_eq!(&con_move, &con_permute);
    prop_assert_eq!(con_move.history_bytes(), con_permute.history_bytes());
    prop_assert_eq!(con_move.can_undo(), con_permute.can_undo());

    if from != to {
        prop_assert_eq!(con_move.undo(), Ok(()));
        prop_assert_eq!(con_permute.undo(), Ok(()));
    }
    prop_assert_eq!(imagen_de(&con_move), &img);
    prop_assert_eq!(imagen_de(&con_permute), &img);
    Ok(())
}

// Propiedad 3: oráculo de quitar.

fn prop_quitar(img: Image, a: u16, b: u16) -> Result<(), TestCaseError> {
    let vieja = paleta_de(&img);
    let n = vieja.len();
    if n < 2 {
        return Ok(());
    }
    let index = ajusta(a, n);
    let mut into = ajusta(b, n);
    if into == index {
        into = (index + 1) % n;
    }
    let t = transparente_de(&img);
    let pix_antes = pixeles_de(&img);
    let res_antes = resueltos(&img);
    let res_into = visible(vieja[into], t == Some(into as u8));
    let into_nuevo = if into < index { into } else { into - 1 };

    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    prop_assert_eq!(doc.apply(quita(id, index as u8, into as u8)), Ok(true));
    let despues = imagen_de(&doc);
    let pix_despues = pixeles_de(despues);
    let res_despues = resueltos(despues);

    // La paleta es la antigua sin `index`.
    let mut paleta_esperada = vieja.clone();
    paleta_esperada.remove(index);
    prop_assert_eq!(paleta_de(despues), paleta_esperada);

    // El transparente: `None`, `< index`, `== index` y `> index`.
    let t_esperado = match t {
        None => None,
        Some(x) if usize::from(x) == index => None,
        Some(x) if usize::from(x) > index => Some(x - 1),
        Some(x) => Some(x),
    };
    prop_assert_eq!(transparente_de(despues), t_esperado);

    // Los píxeles que no valían `index` resuelven igual; los que sí, al resuelto de `into`.
    for (p, &v) in pix_antes.iter().enumerate() {
        if usize::from(v) == index {
            prop_assert_eq!(res_despues[p], res_into, "píxel {}", p);
        } else {
            prop_assert_eq!(res_despues[p], res_antes[p], "píxel {}", p);
        }
    }

    // Ningún píxel `>= len` y el recuento de `into'`.
    prop_assert!(pix_despues.iter().all(|&v| usize::from(v) < n - 1));
    let valian = |v: usize| pix_antes.iter().filter(|&&p| usize::from(p) == v).count();
    let ahora = pix_despues
        .iter()
        .filter(|&&p| usize::from(p) == into_nuevo)
        .count();
    prop_assert_eq!(ahora, valian(into) + valian(index));
    comprueba_invariante(&doc, id)?;

    // `undo` restaura la imagen `==` y `redo` la quitada.
    let quitada = despues.clone();
    prop_assert_eq!(doc.undo(), Ok(()));
    prop_assert_eq!(imagen_de(&doc), &img);
    prop_assert_eq!(doc.redo(), Ok(()));
    prop_assert_eq!(imagen_de(&doc), &quitada);
    Ok(())
}

// Propiedad 4: secuencias mezcladas.

#[derive(Clone, Debug)]
enum Op {
    /// Un comando de paleta suelto, a veces sobre un identificador desconocido.
    Paleta {
        tipo: u8,
        c: Crudo,
        desconocida: bool,
    },
    /// Un `Batch` con un comando de paleta, a veces tras un `SetPixels` válido.
    Lote {
        tipo: u8,
        c: Crudo,
        con_set: bool,
    },
    Set {
        edits: Vec<(u32, u32, u8)>,
        malo: bool,
    },
    Fill {
        y: u32,
        x0: u32,
        x1: u32,
        v: u8,
        malo: bool,
    },
    Convert {
        target: Mode,
        allowed: Allowed,
    },
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    let malo = || prop_oneof![5 => Just(false), 1 => Just(true)];
    prop_oneof![
        12 => (any::<u8>(), crudo(), prop_oneof![9 => Just(false), 1 => Just(true)])
            .prop_map(|(tipo, c, desconocida)| Op::Paleta { tipo, c, desconocida }),
        2 => (any::<u8>(), crudo(), any::<bool>())
            .prop_map(|(tipo, c, con_set)| Op::Lote { tipo, c, con_set }),
        2 => (prop::collection::vec((0u32..8, 0u32..8, any::<u8>()), 1..=3), malo())
            .prop_map(|(edits, malo)| Op::Set { edits, malo }),
        1 => (0u32..8, 0u32..8, 0u32..8, any::<u8>(), malo())
            .prop_map(|(y, x0, x1, v, malo)| Op::Fill { y, x0, x1, v, malo }),
        2 => (
            prop::sample::select(vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed]),
            any::<bool>(),
            prop::option::of(any::<[u8; 3]>()),
        )
            .prop_map(|(target, luma, drop_alpha)| Op::Convert {
                target,
                allowed: Allowed { luma, drop_alpha },
            }),
        3 => Just(Op::Undo),
        3 => Just(Op::Redo),
    ]
}

/// Ejecuta `ops` sobre un documento y sobre un modelo de instantáneas con un cursor.
fn ejecuta_secuencia(inicial: Image, ops: &[Op]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(inicial.clone());
    // Un límite de historia tan grande que nunca se descarta un paso.
    doc.set_history_limit(usize::MAX);
    let id = ImageId::from_raw(0);
    let mut instantes = vec![inicial];
    let mut cursor = 0usize;

    // Registra un paso nuevo: la pila de rehacer se vacía y la imagen actual es la nueva.
    fn nuevo_paso(doc: &Document, id: ImageId, instantes: &mut Vec<Image>, cursor: &mut usize) {
        instantes.truncate(*cursor + 1);
        instantes.push(doc.image(id).unwrap().clone());
        *cursor += 1;
    }

    for op in ops {
        let actual = instantes[cursor].clone();
        let antes = estado(&doc);
        match op {
            Op::Paleta {
                tipo,
                c,
                desconocida,
            } => {
                let objetivo = if *desconocida {
                    ImageId::from_raw(1 + u32::from(c.a % 3))
                } else {
                    id
                };
                let cmd = comando_paleta(*tipo, c, objetivo, longitud_de_paleta(&actual));
                let esperado = predice((!desconocida).then_some(&actual), objetivo, &cmd);
                let real = doc.apply(cmd.clone());
                match esperado {
                    Esperado::Err(e) => {
                        prop_assert_eq!(real, Err(e), "{:?}", cmd);
                        prop_assert_eq!(estado(&doc), antes, "{:?}", cmd);
                    }
                    Esperado::SinPaso => {
                        prop_assert_eq!(real, Ok(false), "{:?}", cmd);
                        prop_assert_eq!(estado(&doc), antes, "{:?}", cmd);
                    }
                    Esperado::Paso => {
                        prop_assert_eq!(real, Ok(true), "{:?}", cmd);
                        prop_assert_ne!(doc.state_id(), antes.state_id);
                        nuevo_paso(&doc, id, &mut instantes, &mut cursor);
                    }
                }
            }
            Op::Lote { tipo, c, con_set } => {
                let cmd = comando_paleta(*tipo, c, id, longitud_de_paleta(&actual));
                let mut comandos = Vec::new();
                if *con_set {
                    comandos.push(set(id, 0, 0, pixel_para(&actual, c.kind, false)));
                }
                comandos.push(cmd);
                let real = doc.apply(Command::Batch(comandos));
                prop_assert_eq!(
                    real,
                    Err(en_lote(usize::from(*con_set), DocumentError::NotBatchable))
                );
                prop_assert_eq!(estado(&doc), antes);
            }
            Op::Set { edits, malo } => {
                let pixels: Vec<PixelEdit> = edits
                    .iter()
                    .map(|&(x, y, v)| PixelEdit {
                        x,
                        y,
                        pixel: pixel_para(&actual, v, *malo),
                    })
                    .collect();
                match doc.apply(Command::SetPixels { image: id, pixels }) {
                    Ok(true) => nuevo_paso(&doc, id, &mut instantes, &mut cursor),
                    Ok(false) | Err(_) => prop_assert_eq!(estado(&doc), antes),
                }
            }
            Op::Fill { y, x0, x1, v, malo } => {
                let span = Span {
                    y: *y,
                    x_start: *x0,
                    x_end: *x1,
                };
                let pixel = pixel_para(&actual, *v, *malo);
                match doc.apply(Command::FillSpans {
                    image: id,
                    spans: vec![span],
                    pixel,
                }) {
                    Ok(true) => nuevo_paso(&doc, id, &mut instantes, &mut cursor),
                    Ok(false) | Err(_) => prop_assert_eq!(estado(&doc), antes),
                }
            }
            Op::Convert { target, allowed } => {
                match doc.apply(Command::ConvertMode {
                    image: id,
                    target: *target,
                    allowed: *allowed,
                }) {
                    Ok(true) => nuevo_paso(&doc, id, &mut instantes, &mut cursor),
                    Ok(false) | Err(_) => prop_assert_eq!(estado(&doc), antes),
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
        // La invariante central y el documento es la instantánea del cursor.
        comprueba_invariante(&doc, id)?;
        prop_assert_eq!(doc.image(id), Some(&instantes[cursor]));
        prop_assert_eq!(doc.can_undo(), cursor > 0);
        prop_assert_eq!(doc.can_redo(), cursor + 1 < instantes.len());
    }

    // Deshacer todo pasa por cada instantánea anterior y llega a la inicial exacta.
    let ultimo_instante = instantes.len() - 1;
    while cursor > 0 {
        prop_assert_eq!(doc.undo(), Ok(()));
        cursor -= 1;
        prop_assert_eq!(doc.image(id), Some(&instantes[cursor]));
        comprueba_invariante(&doc, id)?;
    }
    prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    prop_assert_eq!(doc.image(id), Some(&instantes[0]));
    // Rehacer todo pasa por cada una y llega a la final exacta.
    while cursor < ultimo_instante {
        prop_assert_eq!(doc.redo(), Ok(()));
        cursor += 1;
        prop_assert_eq!(doc.image(id), Some(&instantes[cursor]));
        comprueba_invariante(&doc, id)?;
    }
    prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    prop_assert_eq!(doc.image(id), instantes.last());
    Ok(())
}

// Propiedad 5: validación y atomicidad.

fn prop_validacion(
    img: Image,
    v: u8,
    deshacer: bool,
    tipo: u8,
    c: Crudo,
    desconocida: bool,
    forma: u8,
) -> Result<(), TestCaseError> {
    let mut doc = doc_con_historia(img, v, deshacer);
    let principal = ImageId::from_raw(0);
    let id = if desconocida {
        ImageId::from_raw(1 + u32::from(c.a % 4))
    } else {
        principal
    };
    let actual = imagen_de(&doc).clone();
    let cmd = comando_paleta(tipo, &c, id, longitud_de_paleta(&actual));
    let antes = estado(&doc);

    match forma % 4 {
        // Dentro de un `Batch`: `NotBatchable` gana a cualquier otro error del comando.
        2 => {
            let lote = Command::Batch(vec![cmd]);
            prop_assert_eq!(
                doc.apply(lote),
                Err(en_lote(0, DocumentError::NotBatchable))
            );
            prop_assert_eq!(estado(&doc), antes);
        }
        3 => {
            let lote = Command::Batch(vec![
                set(principal, 0, 0, pixel_para(&actual, v ^ 1, false)),
                cmd,
            ]);
            prop_assert_eq!(
                doc.apply(lote),
                Err(en_lote(1, DocumentError::NotBatchable))
            );
            prop_assert_eq!(estado(&doc), antes);
        }
        _ => {
            let esperado = predice((!desconocida).then_some(&actual), id, &cmd);
            let real = doc.apply(cmd.clone());
            match esperado {
                Esperado::Err(e) => {
                    prop_assert_eq!(real, Err(e), "{:?}", cmd);
                    prop_assert_eq!(estado(&doc), antes, "{:?}", cmd);
                }
                Esperado::SinPaso => {
                    prop_assert_eq!(real, Ok(false), "{:?}", cmd);
                    prop_assert_eq!(estado(&doc), antes, "{:?}", cmd);
                }
                Esperado::Paso => {
                    prop_assert_eq!(real, Ok(true), "{:?}", cmd);
                    prop_assert_ne!(doc.state_id(), antes.state_id);
                    prop_assert!(doc.can_undo());
                    prop_assert!(!doc.can_redo());
                    prop_assert_eq!(pasos_deshacibles(&doc), antes.deshacibles + 1);
                    comprueba_invariante(&doc, principal)?;
                }
            }
        }
    }
    Ok(())
}

// Propiedad 6: coste y límite.

/// Aplica `cmd` al documento; si da `Ok(true)` comprueba que `history_bytes()` tras `redo`
/// es el de tras `apply` y devuelve cuánto subió.
fn mide(
    doc: &mut Document,
    cmd: &Command,
) -> Result<(Result<bool, DocumentError>, usize), TestCaseError> {
    let antes = doc.history_bytes();
    let real = doc.apply(cmd.clone());
    if real != Ok(true) {
        prop_assert_eq!(doc.history_bytes(), antes);
        return Ok((real, 0));
    }
    let tras_apply = doc.history_bytes();
    prop_assert_eq!(doc.undo(), Ok(()));
    prop_assert_eq!(doc.history_bytes(), antes);
    prop_assert_eq!(doc.redo(), Ok(()));
    prop_assert_eq!(doc.history_bytes(), tras_apply);
    Ok((real, tras_apply - antes))
}

fn prop_coste(
    pequena: Image,
    w2: u32,
    h2: u32,
    semilla: u64,
    tipo: u8,
    c: Crudo,
) -> Result<(), TestCaseError> {
    // La misma paleta en dos tamaños de imagen.
    let colores = paleta_de(&pequena);
    let t = transparente_de(&pequena);
    let n = colores.len();
    let pixeles: Vec<u8> = (0..w2 * h2)
        .map(|i| {
            let h = semilla
                .wrapping_add(u64::from(i))
                .wrapping_mul(0x9E37_79B9_7F4A_7C15);
            u8::try_from(((h >> 40) as usize) % n).unwrap()
        })
        .collect();
    let grande = indexada(w2, h2, &colores, t, &pixeles);
    let id = ImageId::from_raw(0);
    let cmd = comando_paleta(tipo, &c, id, n);

    let imagenes = [pequena, grande];
    let mut resultados = Vec::new();
    for img in &imagenes {
        let mut doc = Document::new(img.clone());
        let (real, coste) = mide(&mut doc, &cmd)?;
        if let Command::RemovePaletteEntry { index, .. } = &cmd {
            if real == Ok(true) {
                let usada = pixeles_de(img).contains(index);
                let celdas = (img.width() as usize * img.height() as usize).div_ceil(64);
                let mascara = if usada { celdas * 8 } else { 0 };
                prop_assert!(
                    (mascara..=mascara + 512).contains(&coste),
                    "Remove subió {} con máscara {}",
                    coste,
                    mascara
                );
            }
        } else if real == Ok(true) {
            prop_assert!(coste <= 512, "{:?} subió {}", cmd, coste);
        }
        resultados.push((real, coste));
    }
    // El mismo resultado y, salvo en `Remove`, el mismo coste en los dos tamaños.
    prop_assert_eq!(&resultados[0].0, &resultados[1].0);
    if !matches!(cmd, Command::RemovePaletteEntry { .. }) {
        prop_assert_eq!(resultados[0].1, resultados[1].1, "{:?}", cmd);
    }
    Ok(())
}

/// Imagen de hasta 128 x 128 con todos los píxeles en el índice 0 (para que un `Remove` pueda
/// tener una máscara mayor que el límite).
fn imagen_grande() -> impl Strategy<Value = Image> {
    (
        1u32..=128,
        1u32..=128,
        prop::collection::vec(color_p(), 2..=16),
    )
        .prop_map(|(w, h, colores)| indexada(w, h, &colores, None, &vec![0; (w * h) as usize]))
}

fn prop_limite(img: Image, limite: usize, ops: &[(u8, Crudo)]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(img);
    doc.set_history_limit(limite);
    let id = ImageId::from_raw(0);
    let mut hubo_paso = false;

    for (tipo, c) in ops {
        let actual = imagen_de(&doc).clone();
        let antes = estado(&doc);
        let cmd = comando_paleta(*tipo, c, id, longitud_de_paleta(&actual));
        match doc.apply(cmd.clone()) {
            Ok(true) => {
                hubo_paso = true;
                // El último paso nunca se descarta: se puede deshacer y deja la anterior.
                let mut copia = doc.clone();
                prop_assert_eq!(copia.undo(), Ok(()), "{:?}", cmd);
                prop_assert_eq!(copia.image(id), Some(&actual), "{:?}", cmd);
            }
            Ok(false) | Err(_) => prop_assert_eq!(estado(&doc), antes, "{:?}", cmd),
        }
        let conservados = pasos_deshacibles(&doc);
        prop_assert!(
            doc.history_bytes() <= limite || conservados <= 1,
            "history_bytes = {} > límite {} con {} pasos",
            doc.history_bytes(),
            limite,
            conservados
        );
        prop_assert_eq!(doc.history_limit(), limite);
        if hubo_paso {
            prop_assert!(conservados >= 1, "el último paso siempre se conserva");
            prop_assert!(doc.can_undo());
        }
    }
    Ok(())
}

// Propiedad 7: cada edición toca solo lo suyo.

fn prop_toca_solo_lo_suyo(img: Image, a: u16, b: u16, color: Rgba) -> Result<(), TestCaseError> {
    let vieja = paleta_de(&img);
    let n = vieja.len();
    let t = transparente_de(&img);
    let pix = pixeles_de(&img);
    let res = resueltos(&img);
    let i = ajusta(a, n);

    // `SetPaletteColor(i, c)`: los mismos bytes y el resuelto cambia solo en los píxeles de
    // valor `i`.
    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    let real = doc.apply(pon_color(id, i as u8, color));
    prop_assert_eq!(real, Ok(vieja[i] != color));
    let despues = imagen_de(&doc);
    prop_assert_eq!(pixeles_de(despues), pix.clone());
    prop_assert_eq!(transparente_de(despues), t);
    let res_despues = resueltos(despues);
    for (p, &v) in pix.iter().enumerate() {
        if usize::from(v) == i {
            prop_assert_eq!(res_despues[p], visible(color, t == Some(v)), "píxel {}", p);
        } else {
            prop_assert_eq!(res_despues[p], res[p], "píxel {}", p);
        }
    }
    let mut paleta_esperada = vieja.clone();
    paleta_esperada[i] = color;
    prop_assert_eq!(paleta_de(despues), paleta_esperada);

    // `SetTransparentIndex`: los mismos bytes; solo cambia el alfa de los píxeles que usan
    // la entrada afectada (la antigua y la nueva).
    let nuevo_t = if b.is_multiple_of(5) {
        None
    } else {
        Some(ajusta(b, n) as u8)
    };
    let mut doc = Document::new(img.clone());
    let real = doc.apply(transparente(id, nuevo_t));
    prop_assert_eq!(real, Ok(nuevo_t != t));
    let despues = imagen_de(&doc);
    prop_assert_eq!(pixeles_de(despues), pix.clone());
    prop_assert_eq!(paleta_de(despues), vieja.clone());
    prop_assert_eq!(transparente_de(despues), nuevo_t);
    let res_despues = resueltos(despues);
    for (p, &v) in pix.iter().enumerate() {
        let afectado = t == Some(v) || nuevo_t == Some(v);
        if afectado {
            prop_assert_eq!(
                res_despues[p],
                visible(vieja[usize::from(v)], nuevo_t == Some(v)),
                "píxel {}",
                p
            );
            // El RGB nunca cambia: solo el alfa.
            prop_assert_eq!(
                (res_despues[p].r, res_despues[p].g, res_despues[p].b),
                (res[p].r, res[p].g, res[p].b)
            );
        } else {
            prop_assert_eq!(res_despues[p], res[p], "píxel {}", p);
        }
    }

    // `Add` no cambia ni un píxel ni el transparente.
    if n < 256 {
        let mut doc = Document::new(img.clone());
        prop_assert_eq!(doc.apply(anade(id, color)), Ok(true));
        let despues = imagen_de(&doc);
        prop_assert_eq!(pixeles_de(despues), pix.clone());
        prop_assert_eq!(transparente_de(despues), t);
        let mut esperada = vieja.clone();
        esperada.push(color);
        prop_assert_eq!(paleta_de(despues), esperada);
        prop_assert_eq!(resueltos(despues), res.clone());
    }
    Ok(())
}

// Propiedad 8: `RotateColors`.

fn prop_rotar(
    img: Image,
    a: u16,
    b: u16,
    steps_en_bruto: i32,
    sel: u8,
    m: i16,
) -> Result<(), TestCaseError> {
    let vieja = paleta_de(&img);
    let n = vieja.len();
    let t = transparente_de(&img);
    let pix = pixeles_de(&img);
    let res_antes = resueltos(&img);

    // Rango válido (el inválido se prueba en la propiedad 5) y `steps` de varias clases.
    let start = ajusta(a, n);
    let resto = n - start;
    let len = match b % 5 {
        0 => resto,
        1 => 0,
        2 => 1,
        _ => usize::from(b / 5) % (resto + 1),
    };
    let len_i32 = i32::try_from(len).unwrap();
    let steps = match sel % 5 {
        0 => steps_en_bruto,
        1 => len_i32 * i32::from(m),
        2 => i32::MIN,
        3 => i32::MAX,
        _ => i32::from(m),
    };
    let (start_u8, len_u16) = (start as u8, len as u16);

    // El oráculo: rotación a mano y color resuelto de cada píxel (el transparente se queda
    // en su ranura).
    let k = modulo(steps, len);
    let nueva = rota_a_mano(&vieja, start, len, k);
    let cambia = len >= 2 && k != 0 && nueva != vieja;

    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    let antes = estado(&doc);
    let real = doc.apply(rota(id, start_u8, len_u16, steps));
    prop_assert_eq!(
        real,
        Ok(cambia),
        "start {} len {} steps {}",
        start,
        len,
        steps
    );
    if !cambia {
        prop_assert_eq!(estado(&doc), antes);
    }
    let despues = imagen_de(&doc);
    prop_assert_eq!(paleta_de(despues), nueva.clone());
    // Los índices de los píxeles no cambian.
    prop_assert_eq!(pixeles_de(despues), pix.clone());
    prop_assert_eq!(transparente_de(despues), t);
    let res_despues = resueltos(despues);
    for (p, &v) in pix.iter().enumerate() {
        let esperado = visible(nueva[usize::from(v)], t == Some(v));
        prop_assert_eq!(res_despues[p], esperado, "píxel {}", p);
        // Los píxeles fuera del rango resuelven igual que antes.
        let usv = usize::from(v);
        if usv < start || usv >= start + len {
            prop_assert_eq!(res_despues[p], res_antes[p], "píxel {} fuera del rango", p);
        }
    }
    // Se conserva el multiconjunto de colores de la paleta.
    let mut ordenada_vieja = vieja.clone();
    let mut ordenada_nueva = paleta_de(despues);
    ordenada_vieja.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    ordenada_nueva.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    prop_assert_eq!(ordenada_nueva, ordenada_vieja);
    comprueba_invariante(&doc, id)?;

    if cambia {
        // Rotar `-k` devuelve la paleta exacta.
        let rotada = despues.clone();
        let k_i32 = i32::try_from(k).unwrap();
        prop_assert_eq!(doc.apply(rota(id, start_u8, len_u16, -k_i32)), Ok(true));
        prop_assert_eq!(imagen_de(&doc), &img);
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(imagen_de(&doc), &rotada);
        // `undo` y `redo` son exactos.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(imagen_de(&doc), &img);
        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(imagen_de(&doc), &rotada);
    }

    // Rotar `len` (y cualquier múltiplo, también negativo) da `Ok(false)` sin paso.
    let mut doc = Document::new(img.clone());
    sin_paso(
        &mut doc,
        rota(id, start_u8, len_u16, len_i32 * i32::from(m)),
    );
    sin_paso(&mut doc, rota(id, start_u8, len_u16, len_i32));
    sin_paso(&mut doc, rota(id, start_u8, len_u16, -len_i32));
    Ok(())
}

proptest! {
    // Propiedad 1: `PermutePalette` contra el oráculo, y ida y vuelta.
    #[test]
    fn propiedad_1_oraculo_de_permutar(
        img in imagen_idx(8),
        semilla in any::<u64>(),
        identidad in prop_oneof![1 => Just(true), 7 => Just(false)],
    ) {
        let n = longitud_de_paleta(&img);
        let order = if identidad {
            (0..n).map(|i| i as u8).collect()
        } else {
            permutacion_de(n, semilla)
        };
        // La inversa: `inversa[order[i]] = i`.
        let mut inversa = vec![0u8; n];
        for (nuevo, &antiguo) in order.iter().enumerate() {
            inversa[usize::from(antiguo)] = nuevo as u8;
        }
        let id = ImageId::from_raw(0);
        comprueba_reordenar(&img, permuta(id, &order), &order, permuta(id, &inversa))?;
    }

    // Propiedad 1: `MovePaletteEntry` contra el oráculo, y ida y vuelta con `Move(to, from)`.
    #[test]
    fn propiedad_1_oraculo_de_mover(img in imagen_idx(8), a in any::<u16>(), b in any::<u16>()) {
        let n = longitud_de_paleta(&img);
        let (from, to) = (ajusta(a, n), ajusta(b, n));
        let order = orden_de_mover(n, from, to);
        let id = ImageId::from_raw(0);
        comprueba_reordenar(
            &img,
            mueve(id, from as u8, to as u8),
            &order,
            mueve(id, to as u8, from as u8),
        )?;
    }

    // Propiedad 2: `Move` equivale a `Permute`.
    #[test]
    fn propiedad_2_mover_equivale_a_permutar(
        img in imagen_idx(8),
        a in any::<u16>(),
        b in any::<u16>(),
    ) {
        prop_mover_es_permutar(img, a, b)?;
    }

    // Propiedad 3: oráculo de `RemovePaletteEntry`.
    #[test]
    fn propiedad_3_oraculo_de_quitar(img in imagen_idx(8), a in any::<u16>(), b in any::<u16>()) {
        prop_quitar(img, a, b)?;
    }

    // Propiedad 4: secuencias mezcladas de los siete comandos, `SetPixels`, `FillSpans`,
    // `ConvertMode`, `undo` y `redo`.
    #[test]
    fn propiedad_4_secuencias_mezcladas(
        img in imagen_mixta(),
        ops in prop::collection::vec(op(), 0..=24),
    ) {
        ejecuta_secuencia(img, &ops)?;
    }

    // Propiedad 5: validación (contra un predicado independiente) y atomicidad.
    #[test]
    fn propiedad_5_validacion_y_atomicidad(
        img in imagen_mixta(),
        v in any::<u8>(),
        deshacer in any::<bool>(),
        tipo in any::<u8>(),
        c in crudo(),
        desconocida in prop_oneof![9 => Just(false), 1 => Just(true)],
        forma in any::<u8>(),
    ) {
        prop_validacion(img, v, deshacer, tipo, c, desconocida, forma)?;
    }

    // Propiedad 6, primera parte: coste independiente del tamaño de la imagen.
    #[test]
    fn propiedad_6_coste_del_paso(
        pequena in imagen_idx(8),
        w2 in 1u32..=64,
        h2 in 1u32..=64,
        semilla in any::<u64>(),
        tipo in any::<u8>(),
        c in crudo(),
    ) {
        prop_coste(pequena, w2, h2, semilla, tipo, c)?;
    }

    // Propiedad 6, segunda parte: con un límite de 0 a 2000 bytes, `history_bytes() <= límite`
    // o solo queda un paso, y el último paso nunca se descarta.
    #[test]
    fn propiedad_6_limite_de_la_historia(
        img in prop_oneof![1 => imagen_idx(8), 1 => imagen_grande()],
        limite in 0usize..=2000,
        ops in prop::collection::vec((any::<u8>(), crudo()), 1..=8),
    ) {
        prop_limite(img, limite, &ops)?;
    }

    // Propiedad 7: cada edición toca solo lo suyo.
    #[test]
    fn propiedad_7_cada_edicion_toca_solo_lo_suyo(
        img in imagen_idx(8),
        a in any::<u16>(),
        b in any::<u16>(),
        color in color_p(),
    ) {
        prop_toca_solo_lo_suyo(img, a, b, color)?;
    }

    // Propiedad 8: `RotateColors` contra un oráculo que rota a mano.
    #[test]
    fn propiedad_8_rotar_colores(
        img in imagen_idx(8),
        a in any::<u16>(),
        b in any::<u16>(),
        steps in pasos(),
        sel in any::<u8>(),
        m in any::<i16>(),
    ) {
        prop_rotar(img, a, b, steps, sel, m)?;
    }
}
