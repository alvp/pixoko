//! Tests de `Document` y los comandos con deshacer (specs/hito-1-document.md).
//!
//! Ningún test reserva más de unos pocos KiB: las imágenes miden como mucho 4 x 4 y las
//! historias tienen como mucho unas decenas de pasos. Para medir pasos en las pruebas
//! de límite solo se usan `history_bytes`, `history_limit`, `set_history_limit` y el
//! número de `undo()` que admite una copia: nunca se asume el tamaño exacto de un paso.

use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba,
};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_id_traits<T: Clone + Copy + Debug + PartialEq + Eq + Hash + PartialOrd + Ord>() {}
fn assert_edit_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}
fn assert_dato_traits<T: Clone + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}

#[test]
fn traits_derivados() {
    assert_id_traits::<ImageId>();
    assert_edit_traits::<PixelEdit>();
    assert_dato_traits::<Command>();
    assert_dato_traits::<Document>();
    assert_error_traits::<DocumentError>();
}

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

fn px(x: u32, y: u32, c: Rgba) -> PixelEdit {
    PixelEdit {
        x,
        y,
        pixel: Pixel::Rgba(c),
    }
}

/// `S(...)` de la spec.
fn s(image: ImageId, pixels: &[PixelEdit]) -> Command {
    Command::SetPixels {
        image,
        pixels: pixels.to_vec(),
    }
}

fn lote(comandos: Vec<Command>) -> Command {
    Command::Batch(comandos)
}

fn pixel_en(doc: &Document, x: u32, y: u32) -> Pixel {
    doc.image(doc.main_image())
        .unwrap()
        .get_pixel(x, y)
        .unwrap()
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

fn en_lote(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
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

/// Coste en bytes de la historia tras un `apply` de un solo píxel que cambia, medido en
/// un documento nuevo con el límite por defecto.
fn coste_de_un_paso() -> usize {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    doc.history_bytes()
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

// Identificadores, constantes y errores.

#[test]
fn image_id_ida_y_vuelta_del_valor_crudo() {
    for raw in [0u32, 1, 99, u32::MAX] {
        assert_eq!(ImageId::from_raw(raw).raw(), raw);
    }
    assert_eq!(ImageId::from_raw(7), ImageId::from_raw(7));
    assert_ne!(ImageId::from_raw(7), ImageId::from_raw(8));
    assert!(ImageId::from_raw(1) < ImageId::from_raw(2));
}

#[test]
fn la_imagen_principal_tiene_el_valor_crudo_0() {
    let doc = doc_d();
    assert_eq!(doc.main_image().raw(), 0);
    assert_eq!(doc.main_image(), ImageId::from_raw(0));
}

#[test]
fn limite_por_defecto_vale_64_mib() {
    assert_eq!(Document::DEFAULT_HISTORY_LIMIT, 64 << 20);
}

#[test]
fn comandos_son_datos_comparables() {
    let id = ImageId::from_raw(0);
    let a = s(id, &[px(0, 0, c1()), px(1, 0, c2())]);
    assert_eq!(a, a.clone());
    assert_ne!(a, s(id, &[px(0, 0, c1())]));
    assert_ne!(
        a,
        s(ImageId::from_raw(1), &[px(0, 0, c1()), px(1, 0, c2())])
    );
    assert_eq!(lote(vec![a.clone()]), lote(vec![a.clone()]));
    assert_ne!(lote(vec![a.clone()]), a);
    assert_eq!(px(1, 2, c1()), px(1, 2, c1()));
    assert_ne!(px(1, 2, c1()), px(2, 1, c1()));
}

#[test]
fn errores_con_display_y_source() {
    let interno = fuera(9, 9, 4, 3);
    let con_fuente = [
        fallo_pixel(0, interno.clone()),
        en_lote(0, DocumentError::NestedBatch),
    ];
    for e in &con_fuente {
        assert!(e.source().is_some(), "{e:?} debería exponer su source");
    }
    assert_eq!(
        con_fuente[0].source().map(|e| e.to_string()),
        Some(interno.to_string())
    );
    assert_eq!(
        con_fuente[1].source().map(|e| e.to_string()),
        Some(DocumentError::NestedBatch.to_string())
    );

    let todos = [
        DocumentError::UnknownImage(ImageId::from_raw(99)),
        fallo_pixel(1, interno),
        DocumentError::EmptyCommand,
        DocumentError::NestedBatch,
        en_lote(2, DocumentError::EmptyCommand),
        DocumentError::NothingToUndo,
        DocumentError::NothingToRedo,
        DocumentError::OutOfMemory,
    ];
    for e in &todos {
        assert!(!e.to_string().is_empty(), "{e:?} sin mensaje");
    }
}

#[test]
fn igualdad_de_documentos_compara_contenido_y_no_historia() {
    // Mismo contenido por caminos distintos.
    let mut a = doc_d();
    let mut b = doc_d();
    let id = a.main_image();
    assert_eq!(a.apply(s(id, &[px(0, 0, c1()), px(0, 0, c2())])), Ok(true));
    assert_eq!(b.apply(s(id, &[px(0, 0, c2())])), Ok(true));
    assert_eq!(a, b);
    assert_ne!(a.history_bytes(), 0);

    // Un documento con historia es igual a uno sin historia si el contenido coincide.
    let mut c = doc_d();
    assert_eq!(c.apply(s(id, &[px(0, 0, c2())])), Ok(true));
    assert_eq!(c.apply(s(id, &[px(0, 0, c0())])), Ok(true));
    assert_eq!(c, doc_d());

    // Contenido distinto: distintos.
    assert_ne!(a, doc_d());
    let otra_imagen = Document::new(Image::new(4, 3, Mode::Gray).unwrap());
    assert_ne!(otra_imagen, doc_d());
}

#[test]
fn history_limit_se_lee_y_se_escribe() {
    let mut doc = doc_d();
    for limite in [123usize, 0, usize::MAX, 1] {
        doc.set_history_limit(limite);
        assert_eq!(doc.history_limit(), limite);
    }
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_new_deja_el_documento_sin_historia() {
    let img = Image::new(4, 3, Mode::Rgba).unwrap();
    let doc = Document::new(img.clone());
    assert_eq!(doc.image(doc.main_image()), Some(&img));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);
    assert_eq!(doc.history_limit(), Document::DEFAULT_HISTORY_LIMIT);
}

#[test]
fn fila_new_con_imagen_indexada() {
    let img = Image::new_indexed(2, 2, paleta_de(3)).unwrap();
    let doc = Document::new(img.clone());
    assert_eq!(doc.image(doc.main_image()), Some(&img));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);
    assert_eq!(doc.history_limit(), Document::DEFAULT_HISTORY_LIMIT);
}

#[test]
fn fila_apply_undo_redo() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();

    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    let tras_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, tras_apply);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn fila_apply_nuevo_borra_la_pila_de_rehacer() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let a = px(0, 0, c1());
    let b = px(1, 0, c2());

    assert_eq!(doc.apply(s(id, &[a])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());

    assert_eq!(doc.apply(s(id, &[b])), Ok(true));
    assert!(!doc.can_redo());
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));

    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    let mut esperado = doc_d();
    assert_eq!(esperado.apply(s(id, &[b])), Ok(true));
    assert_eq!(doc, esperado);
}

#[test]
fn fila_undo_y_redo_en_documento_nuevo() {
    let mut doc = doc_d();
    let antes = estado(&doc);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_coordenadas_repetidas_en_un_mismo_set_pixels() {
    // Gana la última escritura; deshacer restaura el valor anterior al comando.
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    assert_eq!(
        doc.apply(s(id, &[px(0, 0, c1()), px(0, 0, c2())])),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c2()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());
}

#[test]
fn fila_escritura_que_cambia_y_restaura_cuenta_como_cambio() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    assert_eq!(
        doc.apply(s(id, &[px(0, 0, c1()), px(0, 0, c0())])),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    // El contenido es el mismo, pero hay un paso (de dos cambios).
    assert_eq!(doc, inicial);
    assert!(doc.can_undo());
    assert!(doc.history_bytes() > 0);
    assert_eq!(pasos_deshacibles(&doc), 1);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
}

#[test]
fn fila_comandos_vacios_son_empty_command() {
    // En un documento nuevo.
    let mut doc = doc_d();
    let id = doc.main_image();
    let antes = estado(&doc);
    assert_eq!(doc.apply(s(id, &[])), Err(DocumentError::EmptyCommand));
    assert_eq!(doc.apply(lote(vec![])), Err(DocumentError::EmptyCommand));
    assert_eq!(estado(&doc), antes);

    // Con historia y con algo que rehacer: nada se toca.
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(doc.apply(s(id, &[px(1, 0, c1())])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    let antes = estado(&doc);
    assert!(antes.can_undo && antes.can_redo);
    assert_eq!(doc.apply(s(id, &[])), Err(DocumentError::EmptyCommand));
    assert_eq!(doc.apply(lote(vec![])), Err(DocumentError::EmptyCommand));
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_fuera_de_rango() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let antes = estado(&doc);

    assert_eq!(
        doc.apply(s(id, &[px(4, 0, c1())])),
        Err(fallo_pixel(0, fuera(4, 0, 4, 3)))
    );
    assert_eq!(estado(&doc), antes);

    assert_eq!(
        doc.apply(s(id, &[px(0, 0, c1()), px(9, 9, c1())])),
        Err(fallo_pixel(1, fuera(9, 9, 4, 3)))
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert!(!doc.can_undo());
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_fallo_tras_escrituras_repetidas_restaura_el_valor_original() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(s(id, &[px(0, 0, c3())])), Ok(true));
    let antes = estado(&doc);

    let r = doc.apply(s(
        id,
        &[
            px(0, 0, c1()),
            px(0, 0, c2()),
            px(1, 1, c1()),
            px(4, 3, c1()),
        ],
    ));
    assert_eq!(r, Err(fallo_pixel(3, fuera(4, 3, 4, 3))));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c3()));
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_otro_modo_e_indice_fuera_de_la_paleta() {
    // Gray(1) en una imagen RGBA.
    let mut doc = doc_d();
    let id = doc.main_image();
    let antes = estado(&doc);
    let gris = PixelEdit {
        x: 0,
        y: 0,
        pixel: Pixel::Gray(1),
    };
    assert_eq!(
        doc.apply(s(id, &[gris])),
        Err(fallo_pixel(
            0,
            ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray
            }
        ))
    );
    assert_eq!(estado(&doc), antes);

    // Indexed(9) en una imagen indexada con paleta de 2.
    let mut doc = Document::new(Image::new_indexed(4, 3, paleta_de(2)).unwrap());
    let id = doc.main_image();
    let antes = estado(&doc);
    let indice = PixelEdit {
        x: 0,
        y: 0,
        pixel: Pixel::Indexed(9),
    };
    assert_eq!(
        doc.apply(s(id, &[indice])),
        Err(fallo_pixel(
            0,
            ImageError::IndexNotInPalette { index: 9, len: 2 }
        ))
    );
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_imagen_desconocida() {
    let mut doc = doc_d();
    let desconocida = ImageId::from_raw(99);
    let antes = estado(&doc);
    assert_eq!(doc.image(desconocida), None);
    assert_eq!(
        doc.apply(s(desconocida, &[px(0, 0, c1())])),
        Err(DocumentError::UnknownImage(desconocida))
    );
    assert_eq!(doc.image(desconocida), None);
    assert_eq!(estado(&doc), antes);
}

#[test]
fn orden_de_comprobacion_de_set_pixels() {
    let mut doc = doc_d();
    let desconocida = ImageId::from_raw(99);
    let antes = estado(&doc);

    // EmptyCommand gana a UnknownImage.
    assert_eq!(
        doc.apply(s(desconocida, &[])),
        Err(DocumentError::EmptyCommand)
    );
    // UnknownImage gana a los fallos de píxel.
    assert_eq!(
        doc.apply(s(desconocida, &[px(9, 9, c1())])),
        Err(DocumentError::UnknownImage(desconocida))
    );
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_comando_invalido_tras_undo_conserva_rehacer() {
    let id = doc_d().main_image();
    let invalidos = [
        s(id, &[]),
        s(id, &[px(4, 0, c1())]),
        s(ImageId::from_raw(99), &[px(0, 0, c1())]),
        lote(vec![]),
        lote(vec![s(id, &[px(0, 0, c1())]), lote(vec![])]),
    ];
    for invalido in invalidos {
        let mut doc = doc_d();
        assert_eq!(doc.apply(s(id, &[px(0, 0, c2())])), Ok(true));
        let con_a = doc.clone();
        assert_eq!(doc.undo(), Ok(()));
        let antes = estado(&doc);

        assert!(doc.apply(invalido.clone()).is_err(), "{invalido:?}");
        assert!(doc.can_redo(), "{invalido:?}");
        assert_eq!(estado(&doc), antes, "{invalido:?}");
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, con_a);
    }
}

#[test]
fn fila_escribir_el_valor_que_ya_hay_no_cambia_nada() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let antes = estado(&doc);
    assert_eq!(doc.apply(s(id, &[px(0, 0, c0())])), Ok(false));
    assert!(!doc.can_undo());
    assert_eq!(doc.history_bytes(), 0);
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_no_op_tras_undo_conserva_rehacer() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    let con_a = doc.clone();
    assert_eq!(doc.undo(), Ok(()));
    let antes = estado(&doc);
    assert!(antes.can_redo);

    assert_eq!(doc.apply(s(id, &[px(0, 0, c0())])), Ok(false));
    assert!(doc.can_redo());
    assert_eq!(doc.can_undo(), antes.can_undo);
    assert_eq!(doc.history_bytes(), antes.bytes);
    assert_eq!(estado(&doc), antes);

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, con_a);
}

#[test]
fn fila_batch_es_un_solo_paso() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    let comando = lote(vec![
        s(id, &[px(0, 0, c1())]),
        s(id, &[px(1, 0, c2())]),
        s(id, &[px(2, 0, c3())]),
    ]);

    assert_eq!(doc.apply(comando), Ok(true));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c3()));
    assert_eq!(pasos_deshacibles(&doc), 1);
    let tras_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, tras_apply);
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c3()));
}

#[test]
fn fila_batch_con_no_ops() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let no_op = s(id, &[px(3, 2, c0())]);
    let a = s(id, &[px(0, 0, c1())]);

    // Un no-op y un cambio: un paso que guarda solo el cambio de `a`.
    assert_eq!(doc.apply(lote(vec![no_op.clone(), a.clone()])), Ok(true));
    assert_eq!(pasos_deshacibles(&doc), 1);
    let mut solo_a = doc_d();
    assert_eq!(solo_a.apply(a), Ok(true));
    assert_eq!(doc, solo_a);
    assert_eq!(doc.history_bytes(), solo_a.history_bytes());

    // Solo no-ops: sin paso y sin tocar la historia.
    let mut otro = doc_d();
    let antes = estado(&otro);
    assert_eq!(otro.apply(lote(vec![no_op.clone(), no_op])), Ok(false));
    assert!(!otro.can_undo());
    assert_eq!(otro.history_bytes(), 0);
    assert_eq!(estado(&otro), antes);
}

#[test]
fn fila_batch_con_fallo_interno_no_cambia_nada() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(s(id, &[px(3, 2, c2())])), Ok(true));
    let antes = estado(&doc);

    // Fuera de rango en el comando 1 (la escritura (1, 0) anterior al fallo se revierte).
    let fuera_de_rango = lote(vec![
        s(id, &[px(0, 0, c1())]),
        s(id, &[px(1, 0, c1()), px(9, 9, c1())]),
        s(id, &[px(2, 0, c1())]),
    ]);
    assert_eq!(
        doc.apply(fuera_de_rango),
        Err(en_lote(1, fallo_pixel(1, fuera(9, 9, 4, 3))))
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c0()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c0()));
    assert_eq!(estado(&doc), antes);

    // Batch anidado en el comando 1.
    let anidado = lote(vec![
        s(id, &[px(0, 0, c1())]),
        lote(vec![s(id, &[px(1, 0, c1())])]),
    ]);
    assert_eq!(
        doc.apply(anidado),
        Err(en_lote(1, DocumentError::NestedBatch))
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert_eq!(estado(&doc), antes);
}

#[test]
fn batch_con_comando_vacio_o_imagen_desconocida_dentro() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let antes = estado(&doc);

    assert_eq!(
        doc.apply(lote(vec![s(id, &[px(0, 0, c1())]), s(id, &[])])),
        Err(en_lote(1, DocumentError::EmptyCommand))
    );
    assert_eq!(estado(&doc), antes);

    let desconocida = ImageId::from_raw(99);
    assert_eq!(
        doc.apply(lote(vec![
            s(id, &[px(0, 0, c1())]),
            s(desconocida, &[px(0, 0, c2())])
        ])),
        Err(en_lote(1, DocumentError::UnknownImage(desconocida)))
    );
    assert_eq!(estado(&doc), antes);

    // Un lote vacío dentro de un lote es EmptyCommand en el comando interno, y un
    // lote dentro de otro, aunque esté vacío, es NestedBatch o EmptyCommand según
    // el orden de comprobación de la spec (no está fijado): solo se comprueba el índice
    // y que no se toca nada.
    let r = doc.apply(lote(vec![s(id, &[px(0, 0, c1())]), lote(vec![])]));
    assert!(
        matches!(r, Err(DocumentError::InBatch { index: 1, .. })),
        "salió {r:?}"
    );
    assert_eq!(estado(&doc), antes);
}

#[test]
fn fila_limite_cero_conserva_solo_el_ultimo_paso() {
    let mut doc = doc_d();
    let id = doc.main_image();
    doc.set_history_limit(0);
    for x in 0..3 {
        assert_eq!(doc.apply(s(id, &[px(x, 0, c1())])), Ok(true));
    }
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.history_bytes(), coste_de_un_paso());

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c0()));
}

#[test]
fn fila_limite_justo_para_dos_pasos() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(doc.apply(s(id, &[px(1, 0, c1())])), Ok(true));
    let dos = doc.history_bytes();
    assert!(dos > 0);

    // Un límite igual al tamaño de la pila no descarta nada (solo si es mayor).
    doc.set_history_limit(dos);
    assert_eq!(doc.history_limit(), dos);
    assert_eq!(pasos_deshacibles(&doc), 2);
    assert_eq!(doc.history_bytes(), dos);

    // El tercero hace que se descarte el más antiguo.
    assert_eq!(doc.apply(s(id, &[px(2, 0, c1())])), Ok(true));
    assert_eq!(pasos_deshacibles(&doc), 2);
    assert!(doc.history_bytes() <= dos);
    let mut copia = doc.clone();
    assert_eq!(copia.undo(), Ok(()));
    assert_eq!(copia.undo(), Ok(()));
    assert_eq!(copia.undo(), Err(DocumentError::NothingToUndo));
    let mut con_el_primero = doc_d();
    assert_eq!(con_el_primero.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(copia, con_el_primero);

    // Con límite 0 queda solo el último.
    doc.set_history_limit(0);
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.history_bytes(), coste_de_un_paso());
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c0()));
}

#[test]
fn fila_un_paso_mayor_que_el_limite_se_conserva() {
    let mut doc = doc_d();
    let inicial = doc.clone();
    let id = doc.main_image();
    doc.set_history_limit(1);
    let grande: Vec<PixelEdit> = (0..4).map(|x| px(x, 0, c1())).collect();

    assert_eq!(doc.apply(s(id, &grande)), Ok(true));
    assert!(doc.history_bytes() > doc.history_limit());
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, inicial);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));

    // Con otro paso encima, el grande (el más antiguo) se descarta y el nuevo se conserva.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc.apply(s(id, &[px(0, 1, c2())])), Ok(true));
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 1), Pixel::Rgba(c0()));
    assert_eq!(pixel_en(&doc, 3, 0), Pixel::Rgba(c1()));
}

#[test]
fn bajar_el_limite_no_borra_la_pila_de_rehacer() {
    let mut doc = doc_d();
    let id = doc.main_image();
    for x in 0..3 {
        assert_eq!(doc.apply(s(id, &[px(x, 0, c1())])), Ok(true));
    }
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pasos_rehacibles(&doc), 2);

    doc.set_history_limit(0);
    assert_eq!(pasos_rehacibles(&doc), 2);
    assert!(doc.can_redo());
    assert_eq!(pasos_deshacibles(&doc), 1);
}

// Estrategias.

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

/// Imagen de hasta 4 x 4 en cualquier modo; el indexado lleva una paleta de 1 a 8 entradas.
fn imagen_pequena() -> impl Strategy<Value = Image> {
    (modo(), 1u32..=4, 1u32..=4, 1usize..=8).prop_map(|(m, w, h, n)| crea(m, w, h, n))
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

/// Fallos inyectables en un píxel.
#[derive(Clone, Copy, Debug)]
enum Falla {
    FueraX,
    FueraY,
    OtroModo,
    IndiceFuera,
}

/// Una escritura en bruto: coordenadas sin acotar, un valor pequeño y un posible fallo.
#[derive(Clone, Copy, Debug)]
struct EdicionRaw {
    xb: u32,
    yb: u32,
    valor: u8,
    falla: Option<Falla>,
}

/// `SetPixels` en bruto: puede apuntar a un id desconocido y tener la lista vacía.
#[derive(Clone, Debug)]
struct SetRaw {
    desconocida: Option<u32>,
    ediciones: Vec<EdicionRaw>,
}

#[derive(Clone, Debug)]
enum SubRaw {
    Set(SetRaw),
    Anidado(Vec<SetRaw>),
}

#[derive(Clone, Debug)]
enum CmdRaw {
    Set(SetRaw),
    Batch(Vec<SubRaw>),
}

fn falla(con_fallos: bool) -> impl Strategy<Value = Option<Falla>> {
    let mut opciones: Vec<Option<Falla>> = vec![None];
    if con_fallos {
        opciones.extend([None; 29]);
        opciones.extend([
            Some(Falla::FueraX),
            Some(Falla::FueraY),
            Some(Falla::OtroModo),
            Some(Falla::IndiceFuera),
        ]);
    }
    prop::sample::select(opciones)
}

fn edicion_raw(con_fallos: bool) -> impl Strategy<Value = EdicionRaw> {
    (any::<u32>(), any::<u32>(), 0u8..4, falla(con_fallos)).prop_map(|(xb, yb, valor, falla)| {
        EdicionRaw {
            xb,
            yb,
            valor,
            falla,
        }
    })
}

/// `SetPixels` en bruto. Sin fallos: lista de 1 a 5 escrituras válidas (con coordenadas
/// repetidas y no-ops). Con fallos: además listas vacías, ids desconocidos y escrituras
/// fuera de la imagen, de otro modo o con índice fuera de la paleta.
fn set_raw(con_fallos: bool) -> impl Strategy<Value = SetRaw> {
    let desconocida = if con_fallos {
        prop_oneof![
            12 => Just(None),
            1 => (1u32..=u32::MAX).prop_map(Some),
        ]
        .boxed()
    } else {
        Just(None).boxed()
    };
    let ediciones = if con_fallos {
        prop_oneof![
            1 => Just(Vec::<EdicionRaw>::new()),
            10 => prop::collection::vec(edicion_raw(true), 1..=5),
        ]
        .boxed()
    } else {
        prop::collection::vec(edicion_raw(false), 1..=5).boxed()
    };
    (desconocida, ediciones).prop_map(|(desconocida, ediciones)| SetRaw {
        desconocida,
        ediciones,
    })
}

fn sub_raw(con_fallos: bool) -> impl Strategy<Value = SubRaw> {
    if con_fallos {
        prop_oneof![
            12 => set_raw(true).prop_map(SubRaw::Set),
            1 => prop::collection::vec(set_raw(true), 1..=2).prop_map(SubRaw::Anidado),
        ]
        .boxed()
    } else {
        set_raw(false).prop_map(SubRaw::Set).boxed()
    }
}

/// `SetPixels` (tres de cada cuatro) o `Batch` de 0 a 4 comandos, con fallos o sin ellos.
fn cmd_raw(con_fallos: bool) -> impl Strategy<Value = CmdRaw> {
    let largo = if con_fallos { 0..=4 } else { 1..=4 };
    prop_oneof![
        3 => set_raw(con_fallos).prop_map(CmdRaw::Set),
        1 => prop::collection::vec(sub_raw(con_fallos), largo).prop_map(CmdRaw::Batch),
    ]
}

fn edicion(f: Forma, e: &EdicionRaw) -> PixelEdit {
    let mut x = e.xb % f.width;
    let mut y = e.yb % f.height;
    let mut pixel = pixel_de(f.mode, f.colores, e.valor);
    match e.falla {
        None => {}
        Some(Falla::FueraX) => x = f.width + e.xb % 1000,
        Some(Falla::FueraY) => y = f.height + e.yb % 1000,
        Some(Falla::OtroModo) => pixel = pixel_de(otro_modo(f.mode, e.valor), 8, e.valor),
        // En una imagen no indexada esto es un `ModeMismatch`.
        Some(Falla::IndiceFuera) => pixel = Pixel::Indexed((f.colores + e.valor as usize) as u8),
    }
    PixelEdit { x, y, pixel }
}

fn set_cmd(f: Forma, id: ImageId, raw: &SetRaw) -> Command {
    Command::SetPixels {
        image: raw.desconocida.map_or(id, ImageId::from_raw),
        pixels: raw.ediciones.iter().map(|e| edicion(f, e)).collect(),
    }
}

fn comando(f: Forma, id: ImageId, raw: &CmdRaw) -> Command {
    match raw {
        CmdRaw::Set(s) => set_cmd(f, id, s),
        CmdRaw::Batch(subs) => Command::Batch(
            subs.iter()
                .map(|sub| match sub {
                    SubRaw::Set(s) => set_cmd(f, id, s),
                    SubRaw::Anidado(v) => {
                        Command::Batch(v.iter().map(|s| set_cmd(f, id, s)).collect())
                    }
                })
                .collect(),
        ),
    }
}

// Comandos totalmente arbitrarios (propiedad 3).

/// Escritura con coordenadas y píxel arbitrarios; `propio` la sustituye por un píxel
/// del modo de la imagen para que los comandos también fallen a mitad de camino.
#[derive(Clone, Copy, Debug)]
struct ArbEdit {
    x: u32,
    y: u32,
    pixel: Pixel,
    propio: Option<u8>,
}

#[derive(Clone, Debug)]
enum ArbRaw {
    Set(Option<u32>, Vec<ArbEdit>),
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
        6 => 0u32..5,
        1 => any::<u32>(),
        1 => Just(u32::MAX),
    ]
}

fn arb_edit() -> impl Strategy<Value = ArbEdit> {
    (
        coordenada_arbitraria(),
        coordenada_arbitraria(),
        pixel_arbitrario(),
        prop_oneof![2 => Just(None), 3 => (0u8..4).prop_map(Some)],
    )
        .prop_map(|(x, y, pixel, propio)| ArbEdit {
            x,
            y,
            pixel,
            propio,
        })
}

fn arb_cmd() -> impl Strategy<Value = ArbRaw> {
    let hoja = (
        prop_oneof![4 => Just(None), 1 => any::<u32>().prop_map(Some)],
        prop::collection::vec(arb_edit(), 0..=5),
    )
        .prop_map(|(id, ediciones)| ArbRaw::Set(id, ediciones));
    hoja.prop_recursive(3, 24, 4, |interno| {
        prop::collection::vec(interno, 0..=4).prop_map(ArbRaw::Batch)
    })
}

fn arb_comando(f: Forma, id: ImageId, raw: &ArbRaw) -> Command {
    match raw {
        ArbRaw::Set(desconocida, ediciones) => Command::SetPixels {
            image: desconocida.map_or(id, ImageId::from_raw),
            pixels: ediciones
                .iter()
                .map(|e| PixelEdit {
                    x: e.x,
                    y: e.y,
                    pixel: e.propio.map_or(e.pixel, |v| pixel_de(f.mode, f.colores, v)),
                })
                .collect(),
        },
        ArbRaw::Batch(comandos) => {
            Command::Batch(comandos.iter().map(|c| arb_comando(f, id, c)).collect())
        }
    }
}

// Modelo de referencia (propiedades 2 y 5).

/// Aplica un `SetPixels` sobre una copia con `Image::set_pixel` y dice si cambió algo.
/// "Cambió" es que alguna escritura modificó el valor que había en ese momento.
fn modelo_set(
    img: &mut Image,
    id: ImageId,
    image: ImageId,
    pixels: &[PixelEdit],
) -> Result<bool, DocumentError> {
    if pixels.is_empty() {
        return Err(DocumentError::EmptyCommand);
    }
    if image != id {
        return Err(DocumentError::UnknownImage(image));
    }
    let mut cambio = false;
    for (i, e) in pixels.iter().enumerate() {
        let antes = img.get_pixel(e.x, e.y);
        img.set_pixel(e.x, e.y, e.pixel)
            .map_err(|source| DocumentError::Pixel { pixel: i, source })?;
        if antes != Ok(e.pixel) {
            cambio = true;
        }
    }
    Ok(cambio)
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
        Command::SetPixels { image, pixels } => {
            let cambio = modelo_set(&mut trabajo, id, *image, pixels)?;
            Ok((trabajo, cambio))
        }
        Command::Batch(comandos) => {
            if comandos.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            let mut cambio = false;
            for (index, c) in comandos.iter().enumerate() {
                match c {
                    Command::SetPixels { image, pixels } => {
                        cambio |= modelo_set(&mut trabajo, id, *image, pixels)
                            .map_err(|e| en_lote(index, e))?;
                    }
                    Command::Batch(_) => return Err(en_lote(index, DocumentError::NestedBatch)),
                    otro => unreachable!("comando sin modelo: {otro:?}"),
                }
            }
            Ok((trabajo, cambio))
        }
        otro => unreachable!("comando sin modelo: {otro:?}"),
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
    Apply(CmdRaw),
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => cmd_raw(false).prop_map(Op::Apply),
        2 => cmd_raw(true).prop_map(Op::Apply),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
    ]
}

/// Ejecuta `ops` sobre un documento y sobre el modelo (instantáneas de la imagen y un
/// cursor) y los compara tras cada operación.
fn ejecuta_modelo(img: Image, ops: &[Op]) -> Result<(), TestCaseError> {
    let f = forma(&img);
    let mut doc = Document::new(img.clone());
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
    Apply(ArbRaw),
    Undo,
    Redo,
    Limite(usize),
}

fn op_arbitraria() -> impl Strategy<Value = OpArb> {
    prop_oneof![
        5 => arb_cmd().prop_map(OpArb::Apply),
        1 => Just(OpArb::Undo),
        1 => Just(OpArb::Redo),
        1 => prop_oneof![
            any::<usize>(),
            0usize..4096,
            Just(0usize),
            Just(usize::MAX),
        ]
        .prop_map(OpArb::Limite),
    ]
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

/// Comprobaciones de la propiedad 5 sobre un documento que solo ha recibido `apply`
/// (y quizá cambios de límite): `instantes[0]` es el inicial y cada `Ok(true)` añadió uno.
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
    let mut copia = doc.clone();
    prop_assert_eq!(deshace_todo(&mut copia, conservados), conservados);
    prop_assert_eq!(copia.undo(), Err(DocumentError::NothingToUndo));
    comprueba_contenido(&copia, &instantes[creados - conservados])?;

    // Rehacer todo devuelve el final.
    prop_assert_eq!(rehace_todo(&mut copia, conservados), conservados);
    prop_assert_eq!(copia.redo(), Err(DocumentError::NothingToRedo));
    comprueba_contenido(&copia, &instantes[creados])?;
    Ok(())
}

/// Aplica `cmds` (válidos) a `doc`, comprueba cada resultado contra el modelo y añade a
/// `instantes` una imagen por cada `Ok(true)`. Si hay `limite`, tras cada `apply` comprueba
/// la propiedad 5.
fn aplica_validos(
    doc: &mut Document,
    instantes: &mut Vec<Image>,
    f: Forma,
    cmds: &[CmdRaw],
    limite: Option<usize>,
) -> Result<(), TestCaseError> {
    let id = doc.main_image();
    for raw in cmds {
        let cmd = comando(f, id, raw);
        let actual = instantes.last().unwrap().clone();
        let (nueva, cambio) = match modelo_apply(&actual, id, &cmd) {
            Ok(r) => r,
            Err(e) => {
                return Err(TestCaseError::fail(format!(
                    "el generador sin fallos produjo un comando inválido: {e:?}"
                )));
            }
        };
        prop_assert_eq!(doc.apply(cmd), Ok(cambio));
        if cambio {
            instantes.push(nueva);
        }
        if let Some(limite) = limite {
            comprueba_limite(doc, instantes, limite)?;
        }
    }
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: deshacer todo y rehacer todo.
    #[test]
    fn deshacer_todo_y_rehacer_todo(
        img in imagen_pequena(),
        cmds in prop::collection::vec(cmd_raw(true), 0..=30),
    ) {
        let f = forma(&img);
        let inicial = Document::new(img);
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
    fn historia_contra_un_modelo(
        img in imagen_pequena(),
        ops in prop::collection::vec(op(), 0..=40),
    ) {
        ejecuta_modelo(img, &ops)?;
    }

    // Propiedad 3: atomicidad y ausencia de pánicos con comandos arbitrarios.
    #[test]
    fn atomicidad_y_ausencia_de_panicos(
        img in imagen_pequena(),
        semilla in prop::collection::vec(cmd_raw(false), 0..=4),
        ops in prop::collection::vec(op_arbitraria(), 0..=20),
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
                OpArb::Apply(raw) => {
                    let cmd = arb_comando(f, id, raw);
                    match doc.apply(cmd.clone()) {
                        Ok(true) => {
                            prop_assert!(doc.can_undo());
                            prop_assert!(!doc.can_redo());
                        }
                        Ok(false) | Err(_) => prop_assert_eq!(estado(&doc), antes, "{:?}", cmd),
                    }
                }
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
                OpArb::Limite(limite) => {
                    doc.set_history_limit(*limite);
                    prop_assert_eq!(doc.history_limit(), *limite);
                    // Cambiar el límite no toca el contenido ni la pila de rehacer.
                    prop_assert_eq!(&doc, &antes.doc);
                    prop_assert_eq!(doc.can_redo(), antes.can_redo);
                    prop_assert_eq!(pasos_rehacibles(&doc), antes.rehacibles);
                }
            }
        }
    }

    // Propiedad 4: un Batch es un paso.
    #[test]
    fn batch_es_un_paso(
        img in imagen_pequena(),
        lista in prop::collection::vec(set_raw(false), 1..=6),
    ) {
        let f = forma(&img);
        let inicial = Document::new(img);
        let id = inicial.main_image();
        let cmds: Vec<Command> = lista.iter().map(|s| set_cmd(f, id, s)).collect();

        // Uno a uno.
        let mut uno_a_uno = inicial.clone();
        let mut alguno = false;
        for c in &cmds {
            match uno_a_uno.apply(c.clone()) {
                Ok(cambio) => alguno |= cambio,
                Err(e) => {
                    return Err(TestCaseError::fail(format!(
                        "un SetPixels válido falló: {e:?} ({c:?})"
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

    // Propiedad 5: el límite de la historia, con el límite fijado antes de editar.
    #[test]
    fn limite_de_la_historia(
        img in imagen_pequena(),
        limite in 0usize..=2000,
        cmds in prop::collection::vec(cmd_raw(false), 0..=30),
    ) {
        let f = forma(&img);
        let mut doc = Document::new(img.clone());
        doc.set_history_limit(limite);
        let mut instantes = vec![img];
        aplica_validos(&mut doc, &mut instantes, f, &cmds, Some(limite))?;
        comprueba_limite(&doc, &instantes, limite)?;
    }

    // Propiedad 5, segunda parte: bajar el límite con `set_history_limit` cumple lo mismo.
    #[test]
    fn bajar_el_limite_de_la_historia(
        img in imagen_pequena(),
        cmds in prop::collection::vec(cmd_raw(false), 0..=30),
        limites in prop::collection::vec(0usize..=2000, 1..=3),
    ) {
        let f = forma(&img);
        let mut doc = Document::new(img.clone());
        let mut instantes = vec![img];
        aplica_validos(&mut doc, &mut instantes, f, &cmds, None)?;

        // Con el límite por defecto no se descarta nada.
        comprueba_limite(&doc, &instantes, Document::DEFAULT_HISTORY_LIMIT)?;
        prop_assert_eq!(pasos_deshacibles(&doc), instantes.len() - 1);

        for limite in limites {
            doc.set_history_limit(limite);
            comprueba_limite(&doc, &instantes, limite)?;
        }
    }
}
