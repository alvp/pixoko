//! Tests de `Command::ConvertMode` en el documento (specs/hito-2-conversion-modos.md):
//! `DocumentError::Convert` y `DocumentError::NotBatchable`.
//!
//! Una prueba por cada fila 13, 14 (la parte del documento), 15 y 19 de la tabla de casos
//! límite (`fila_NN_...`) y una propiedad por cada una de las dos últimas de «Tests» (5 y 6).
//! Las imágenes miden como mucho 6 x 6 (salvo la de 257 colores de la fila 14). El coste de
//! la historia solo se mide con `history_bytes()` y con las cotas de la spec: nunca se asume
//! el valor de `size_of`.

use std::error::Error;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, Command, ConversionReport, ConvertError, Document, DocumentError, Image, ImageId,
    Mode, Palette, Pixel, PixelEdit, Rgba, Span, convert_image,
};

// Constantes de la notación de la tabla.

const N: Allowed = Allowed::NOTHING;
const L: Allowed = Allowed {
    luma: true,
    drop_alpha: None,
};
const L_Y_A: Allowed = Allowed {
    luma: true,
    drop_alpha: Some([255, 255, 255]),
};

const TODOS: [Mode; 4] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];

// Construcción de imágenes, documentos y comandos.

fn informe(color_lost: u64, alpha_lost: u64) -> ConversionReport {
    ConversionReport {
        color_lost,
        alpha_lost,
    }
}

fn rgba_img(width: u32, height: u32, pixels: &[(u8, u8, u8, u8)]) -> Image {
    assert_eq!(pixels.len(), (width * height) as usize);
    let mut img = Image::new(width, height, Mode::Rgba).unwrap();
    for (i, &(r, g, b, a)) in pixels.iter().enumerate() {
        img.set_pixel(
            i as u32 % width,
            i as u32 / width,
            Pixel::Rgba(Rgba::new(r, g, b, a)),
        )
        .unwrap();
    }
    img
}

fn gris_img(width: u32, height: u32, pixels: &[u8]) -> Image {
    assert_eq!(pixels.len(), (width * height) as usize);
    let mut img = Image::new(width, height, Mode::Gray).unwrap();
    for (i, &g) in pixels.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, Pixel::Gray(g))
            .unwrap();
    }
    img
}

/// `D` de la spec: documento con una imagen Rgba de 2 x 1, rojo y azul opacos.
fn doc_d() -> Document {
    Document::new(rgba_img(2, 1, &[(255, 0, 0, 255), (0, 0, 255, 255)]))
}

fn imagen_de(doc: &Document) -> &Image {
    doc.image(doc.main_image()).unwrap()
}

fn convertir(image: ImageId, target: Mode, allowed: Allowed) -> Command {
    Command::ConvertMode {
        image,
        target,
        allowed,
    }
}

fn set(image: ImageId, x: u32, y: u32, pixel: Pixel) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit { x, y, pixel }],
    }
}

fn fallo_convert(e: ConvertError) -> DocumentError {
    DocumentError::Convert(e)
}

fn perdida(color: u64, alfa: u64) -> DocumentError {
    fallo_convert(ConvertError::LossNotAllowed {
        report: informe(color, alfa),
    })
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

/// Bytes de píxeles de una imagen, según `Image::byte_len`.
fn bytes_de(img: &Image) -> usize {
    Image::byte_len(img.width(), img.height(), img.mode()).unwrap()
}

// Tipos nuevos de la API.

#[test]
fn errores_nuevos_del_documento() {
    let convert = perdida(2, 0);
    assert!(!convert.to_string().is_empty());
    assert!(!DocumentError::NotBatchable.to_string().is_empty());
    assert!(convert.source().is_some(), "Convert lleva su causa");
    assert_ne!(convert, DocumentError::NotBatchable);
    assert_ne!(convert, perdida(1, 0));
    assert!(
        !en_lote(1, DocumentError::NotBatchable)
            .to_string()
            .is_empty()
    );

    let cmd = convertir(ImageId::from_raw(0), Mode::Gray, N);
    assert_eq!(cmd, cmd.clone());
    assert_ne!(cmd, convertir(ImageId::from_raw(0), Mode::Gray, L));
    assert_ne!(cmd, convertir(ImageId::from_raw(0), Mode::Rgba, N));
    assert_ne!(cmd, convertir(ImageId::from_raw(1), Mode::Gray, N));
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_13_convertir_y_deshacer() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let original = imagen_de(&doc).clone();

    // Sin permiso: error y nada cambia.
    rechaza(&mut doc, convertir(id, Mode::Gray, N), perdida(2, 0));
    assert_eq!(imagen_de(&doc), &original);
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);

    // Con `luma`: un paso nuevo.
    let id_antes = doc.state_id();
    assert_eq!(doc.apply(convertir(id, Mode::Gray, L)), Ok(true));
    assert_ne!(doc.state_id(), id_antes);
    assert_eq!(imagen_de(&doc).mode(), Mode::Gray);
    assert_eq!(imagen_de(&doc), &gris_img(2, 1, &[54, 19]));
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(pasos_deshacibles(&doc), 1);
    let convertido = doc.clone();

    // `undo` deja una imagen igual a la original, en su modo.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc).mode(), Mode::Rgba);
    assert_eq!(imagen_de(&doc), &original);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());

    // `redo` deja la Gray otra vez.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc), &gris_img(2, 1, &[54, 19]));
    assert_eq!(doc, convertido);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn fila_14_errores_del_documento_dejan_todo_intacto() {
    // Pérdida no permitida.
    let mut doc = doc_d();
    let id = doc.main_image();
    rechaza(&mut doc, convertir(id, Mode::Gray, N), perdida(2, 0));

    // Imagen desconocida: el id 99.
    let desconocida = ImageId::from_raw(99);
    rechaza(
        &mut doc,
        convertir(desconocida, Mode::Indexed, N),
        DocumentError::UnknownImage(desconocida),
    );
    rechaza(
        &mut doc,
        convertir(desconocida, Mode::Rgba, L_Y_A),
        DocumentError::UnknownImage(desconocida),
    );

    // Una imagen de 257 colores a `Indexed`.
    let colores: Vec<(u8, u8, u8, u8)> = (0..257usize)
        .map(|i| ((i % 256) as u8, (i / 256) as u8, 0, 255))
        .collect();
    let mut grande = Document::new(rgba_img(257, 1, &colores));
    let id_grande = grande.main_image();
    rechaza(
        &mut grande,
        convertir(id_grande, Mode::Indexed, N),
        fallo_convert(ConvertError::TooManyColors { at_least: 257 }),
    );
    // Con todos los permisos tampoco: no hay cuantización.
    rechaza(
        &mut grande,
        convertir(id_grande, Mode::Indexed, L_Y_A),
        fallo_convert(ConvertError::TooManyColors { at_least: 257 }),
    );

    // `ConvertMode` dentro de un `Batch`: el `SetPixels` anterior se revierte.
    let mut doc = doc_d();
    let id = doc.main_image();
    let lote = Command::Batch(vec![
        set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
        convertir(id, Mode::Gray, L),
    ]);
    rechaza(&mut doc, lote, en_lote(1, DocumentError::NotBatchable));
    assert_eq!(imagen_de(&doc).mode(), Mode::Rgba);
    assert!(!doc.can_undo());
    assert_eq!(doc.history_bytes(), 0);

    // Lo mismo con historia previa y pila de rehacer.
    assert_eq!(
        doc.apply(set(id, 1, 0, Pixel::Rgba(Rgba::new(9, 9, 9, 255)))),
        Ok(true)
    );
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let lote = Command::Batch(vec![
        set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
        set(id, 1, 0, Pixel::Rgba(Rgba::new(4, 5, 6, 255))),
        convertir(id, Mode::Gray, L_Y_A),
    ]);
    rechaza(&mut doc, lote, en_lote(2, DocumentError::NotBatchable));
    assert!(doc.can_redo());
}

/// `apply(cmd)` falla con `InBatch { index, source }` donde `source` cumple `es`, y no toca
/// nada observable.
fn rechaza_en_lote(
    doc: &mut Document,
    cmd: Command,
    index: usize,
    es: impl Fn(&DocumentError) -> bool,
) {
    let antes = estado(doc);
    match doc.apply(cmd.clone()) {
        Err(DocumentError::InBatch { index: i, source }) => {
            assert_eq!(i, index, "índice del comando que falló en {cmd:?}");
            assert!(es(&source), "error inesperado {source:?} en {cmd:?}");
        }
        otro => panic!("se esperaba InBatch en {cmd:?} y salió {otro:?}"),
    }
    assert_eq!(estado(doc), antes, "{cmd:?}");
}

#[test]
fn fila_20_not_batchable_gana_a_unknown_image_y_el_primer_error_gana() {
    let mut doc = doc_d();
    let id = doc.main_image();
    let desconocida = ImageId::from_raw(99);
    let fuera = |image: ImageId| set(image, 5, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255)));
    let es_not_batchable = |e: &DocumentError| *e == DocumentError::NotBatchable;

    // Solo un `ConvertMode` sobre una imagen que no existe: no es `UnknownImage`.
    let lote = Command::Batch(vec![convertir(desconocida, Mode::Gray, N)]);
    rechaza_en_lote(&mut doc, lote, 0, es_not_batchable);

    // Con un `SetPixels` válido antes: el índice es el del `ConvertMode`, y el `SetPixels` se
    // revierte. El modo de destino y los permisos no importan.
    for (target, allowed) in [(Mode::Rgba, L_Y_A), (Mode::Indexed, N), (Mode::Gray, L)] {
        let lote = Command::Batch(vec![
            set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
            convertir(desconocida, target, allowed),
        ]);
        rechaza_en_lote(&mut doc, lote, 1, es_not_batchable);
        assert_eq!(
            imagen_de(&doc).get_pixel(0, 0),
            Ok(Pixel::Rgba(Rgba::new(255, 0, 0, 255)))
        );
    }

    // Gana el primer error del lote: un `SetPixels` fuera del lienzo antes del `ConvertMode`...
    let lote = Command::Batch(vec![fuera(id), convertir(desconocida, Mode::Gray, L)]);
    rechaza_en_lote(&mut doc, lote, 0, |e| {
        matches!(e, DocumentError::Pixel { .. })
    });
    // ...y el `ConvertMode` primero, aunque después haya otro fallo.
    let lote = Command::Batch(vec![convertir(desconocida, Mode::Gray, L), fuera(id)]);
    rechaza_en_lote(&mut doc, lote, 0, es_not_batchable);

    // Con historia previa y pila de rehacer, tampoco cambia nada.
    assert_eq!(
        doc.apply(set(id, 1, 0, Pixel::Rgba(Rgba::new(9, 9, 9, 255)))),
        Ok(true)
    );
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let lote = Command::Batch(vec![
        set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
        convertir(desconocida, Mode::Gray, N),
    ]);
    rechaza_en_lote(&mut doc, lote, 1, es_not_batchable);
    assert!(doc.can_redo());
}

#[test]
fn fila_15_con_limite_cero_el_paso_grande_se_conserva() {
    let mut doc = Document::new(Image::new(4, 4, Mode::Rgba).unwrap());
    let id = doc.main_image();
    let rgba_inicial = imagen_de(&doc).clone();
    doc.set_history_limit(0);

    assert_eq!(doc.apply(convertir(id, Mode::Gray, L_Y_A)), Ok(true));
    // 64 bytes de píxeles de la imagen vieja, más una cota de 2048 para cabecera y paleta.
    let bytes = doc.history_bytes();
    assert!((64..=2112).contains(&bytes), "history_bytes() = {bytes}");
    assert_eq!(
        pasos_deshacibles(&doc),
        1,
        "un paso mayor que el límite no se descarta"
    );
    assert_eq!(imagen_de(&doc).mode(), Mode::Gray);
    let tras_conversion = imagen_de(&doc).clone();

    // Un `SetPixels` válido sobre la imagen Gray.
    assert_eq!(doc.apply(set(id, 0, 0, Pixel::Gray(7))), Ok(true));
    assert_eq!(imagen_de(&doc).get_pixel(0, 0), Ok(Pixel::Gray(7)));
    let final_ = imagen_de(&doc).clone();
    assert_ne!(final_, tras_conversion);

    // Solo queda el último paso: un único `undo`, que deja la Gray de la conversión.
    assert_eq!(pasos_deshacibles(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &tras_conversion);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(imagen_de(&doc), &tras_conversion);

    // Y un `redo` que repite el estado final.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc), &final_);
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    assert_ne!(imagen_de(&doc), &rgba_inicial);
}

#[test]
fn fila_15_con_el_limite_por_defecto_se_deshace_todo() {
    let mut doc = Document::new(Image::new(4, 4, Mode::Rgba).unwrap());
    let id = doc.main_image();
    let rgba_inicial = imagen_de(&doc).clone();

    assert_eq!(doc.apply(convertir(id, Mode::Gray, L_Y_A)), Ok(true));
    let bytes = doc.history_bytes();
    assert!((64..=2112).contains(&bytes), "history_bytes() = {bytes}");
    let tras_conversion = imagen_de(&doc).clone();
    assert_eq!(doc.apply(set(id, 0, 0, Pixel::Gray(7))), Ok(true));
    let final_ = imagen_de(&doc).clone();
    assert_eq!(pasos_deshacibles(&doc), 2);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &tras_conversion);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), &rgba_inicial);
    assert_eq!(imagen_de(&doc).mode(), Mode::Rgba);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc), &tras_conversion);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc), &final_);
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
}

#[test]
fn fila_19_convertir_al_modo_actual_no_es_un_paso() {
    let mut doc = doc_d();
    let id = doc.main_image();
    assert_eq!(
        doc.apply(set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255)))),
        Ok(true)
    );
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let antes = estado(&doc);

    // Con cualquier `allowed`, incluso con permisos de más: no se llama a `convert_image`.
    let permisos = [
        N,
        L,
        L_Y_A,
        Allowed {
            luma: false,
            drop_alpha: Some([3, 4, 5]),
        },
    ];
    for allowed in permisos {
        assert_eq!(
            doc.apply(convertir(id, Mode::Rgba, allowed)),
            Ok(false),
            "{allowed:?}"
        );
        assert_eq!(estado(&doc), antes, "{allowed:?}");
        assert!(doc.can_redo(), "no borra la pila de rehacer");
        assert_eq!(doc.state_id(), antes.state_id);
        assert_eq!(doc.history_bytes(), antes.bytes);
    }

    // Lo que había por rehacer sigue ahí.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(
        imagen_de(&doc).get_pixel(0, 0),
        Ok(Pixel::Rgba(Rgba::new(1, 2, 3, 255)))
    );
}

// Estrategias.

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

const CANALES: [u8; 5] = [0, 1, 127, 128, 255];

fn canal() -> impl Strategy<Value = u8> {
    prop_oneof![any::<u8>(), prop::sample::select(CANALES.to_vec())]
}

fn pequeno() -> impl Strategy<Value = u8> {
    prop::sample::select(CANALES.to_vec())
}

/// Colores uniformes, del conjunto pequeño y grises con alfa de un conjunto pequeño.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
        (pequeno(), pequeno(), pequeno(), pequeno()).prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
        (canal(), pequeno()).prop_map(|(v, a)| Rgba::new(v, v, v, a)),
    ]
}

fn dims() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=6, 1u32..=6)
}

/// Imagen de hasta 6 x 6 en cualquier modo; la indexada lleva una paleta de 1 a 8 entradas
/// y a veces un índice transparente.
fn imagen_6() -> impl Strategy<Value = Image> {
    (
        modo(),
        dims(),
        prop::collection::vec(color(), 36),
        prop::collection::vec(color(), 1..=8),
        prop::option::of(any::<u8>()),
        prop::collection::vec(any::<u8>(), 36),
    )
        .prop_map(|(mode, (w, h), colores, paleta, t, idx)| {
            let n = (w * h) as usize;
            let en = |i: usize| (i as u32 % w, i as u32 / w);
            let mut img = match mode {
                Mode::Indexed => {
                    let mut p = Palette::from_colors(paleta.iter().copied()).unwrap();
                    p.set_transparent_index(t.map(|t| (t as usize % paleta.len()) as u8))
                        .unwrap();
                    Image::new_indexed(w, h, p).unwrap()
                }
                _ => Image::new(w, h, mode).unwrap(),
            };
            for i in 0..n {
                let c = colores[i];
                let pixel = match mode {
                    Mode::Rgba => Pixel::Rgba(c),
                    Mode::GrayAlpha => Pixel::GrayAlpha {
                        gray: c.r,
                        alpha: c.a,
                    },
                    Mode::Gray => Pixel::Gray(c.g),
                    Mode::Indexed => Pixel::Indexed((idx[i] as usize % paleta.len()) as u8),
                };
                let (px, py) = en(i);
                img.set_pixel(px, py, pixel).unwrap();
            }
            img
        })
}

fn permisos() -> impl Strategy<Value = Allowed> {
    (
        prop_oneof![2 => Just(true), 1 => Just(false)],
        prop_oneof![
            2 => (canal(), canal(), canal()).prop_map(|(r, g, b)| Some([r, g, b])),
            1 => Just(None),
        ],
    )
        .prop_map(|(luma, drop_alpha)| Allowed { luma, drop_alpha })
}

/// Permisos para conversiones que no fallan por pérdida.
fn todos_los_permisos() -> impl Strategy<Value = Allowed> {
    (canal(), canal(), canal()).prop_map(|(r, g, b)| Allowed {
        luma: true,
        drop_alpha: Some([r, g, b]),
    })
}

fn alfa_de(v: u8) -> u8 {
    match v % 4 {
        0 => 0,
        1 => 128,
        _ => 255,
    }
}

/// Píxel de un modo a partir de un valor pequeño; los de `Rgba` son grises si `v` es par.
/// En `Indexed`, `v` se reduce módulo la longitud de la paleta (`len`).
fn pixel_de(mode: Mode, len: usize, v: u8) -> Pixel {
    match mode {
        Mode::Rgba if v.is_multiple_of(2) => Pixel::Rgba(Rgba::new(v, v, v, alfa_de(v))),
        Mode::Rgba => Pixel::Rgba(Rgba::new(v, v ^ 0x55, v.wrapping_mul(3), alfa_de(v))),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: v,
            alpha: alfa_de(v),
        },
        Mode::Gray => Pixel::Gray(v),
        Mode::Indexed => Pixel::Indexed((usize::from(v) % len.max(1)) as u8),
    }
}

fn otro_modo(mode: Mode) -> Mode {
    let i = TODOS.iter().position(|&m| m == mode).unwrap();
    TODOS[(i + 1) % TODOS.len()]
}

fn longitud_de_paleta(img: &Image) -> usize {
    img.palette().map_or(0, Palette::len)
}

/// Un píxel válido (o inválido, si `malo`) para la imagen actual.
fn pixel_para(img: &Image, v: u8, malo: bool) -> Pixel {
    if malo {
        pixel_de(otro_modo(img.mode()), 8, v)
    } else {
        pixel_de(img.mode(), longitud_de_paleta(img), v)
    }
}

// Modelo de instantáneas (propiedad 5).

#[derive(Clone, Debug)]
enum Op {
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
        desconocida: Option<u32>,
    },
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => (prop::collection::vec((0u32..8, 0u32..8, 0u8..8), 1..=3), prop_oneof![5 => Just(false), 1 => Just(true)])
            .prop_map(|(edits, malo)| Op::Set { edits, malo }),
        2 => (0u32..8, 0u32..8, 0u32..8, 0u8..8, prop_oneof![5 => Just(false), 1 => Just(true)])
            .prop_map(|(y, x0, x1, v, malo)| Op::Fill { y, x0, x1, v, malo }),
        5 => (modo(), permisos(), prop_oneof![9 => Just(None), 1 => (1u32..5).prop_map(Some)])
            .prop_map(|(target, allowed, desconocida)| Op::Convert { target, allowed, desconocida }),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
    ]
}

/// Lo que el modelo espera de un `SetPixels` o un `FillSpans`.
enum Previsto {
    Error,
    SinCambio,
    Nuevo(Image),
}

fn modelo_set(img: &Image, pixels: &[PixelEdit]) -> Previsto {
    let mut trabajo = img.clone();
    let mut cambio = false;
    for e in pixels {
        let antes = trabajo.get_pixel(e.x, e.y);
        if trabajo.set_pixel(e.x, e.y, e.pixel).is_err() {
            return Previsto::Error;
        }
        cambio |= antes != Ok(e.pixel);
    }
    if cambio {
        Previsto::Nuevo(trabajo)
    } else {
        Previsto::SinCambio
    }
}

fn modelo_fill(img: &Image, span: Span, pixel: Pixel) -> Previsto {
    if img.check_pixel(pixel).is_err()
        || span.x_start >= span.x_end
        || span.y >= img.height()
        || span.x_end > img.width()
    {
        return Previsto::Error;
    }
    let mut trabajo = img.clone();
    let mut cambio = false;
    for x in span.x_start..span.x_end {
        if trabajo.get_pixel(x, span.y) != Ok(pixel) {
            trabajo.set_pixel(x, span.y, pixel).unwrap();
            cambio = true;
        }
    }
    if cambio {
        Previsto::Nuevo(trabajo)
    } else {
        Previsto::SinCambio
    }
}

/// Ejecuta `ops` sobre un documento y sobre el modelo (instantáneas de la imagen y un
/// cursor) y los compara tras cada operación.
fn ejecuta_modelo(inicial: Image, ops: &[Op]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(inicial.clone());
    let id = doc.main_image();
    let mut instantes = vec![inicial];
    let mut cursor = 0usize;

    for op in ops {
        let actual = instantes[cursor].clone();
        let antes = estado(&doc);
        match op {
            Op::Set { edits, malo } => {
                let pixels: Vec<PixelEdit> = edits
                    .iter()
                    .map(|&(x, y, v)| PixelEdit {
                        x,
                        y,
                        pixel: pixel_para(&actual, v, *malo),
                    })
                    .collect();
                let previsto = modelo_set(&actual, &pixels);
                let real = doc.apply(Command::SetPixels { image: id, pixels });
                aplica_previsto(real, previsto, &mut instantes, &mut cursor, &doc, &antes)?;
            }
            Op::Fill { y, x0, x1, v, malo } => {
                let span = Span {
                    y: *y,
                    x_start: *x0,
                    x_end: *x1,
                };
                let pixel = pixel_para(&actual, *v, *malo);
                let previsto = modelo_fill(&actual, span, pixel);
                let real = doc.apply(Command::FillSpans {
                    image: id,
                    spans: vec![span],
                    pixel,
                });
                aplica_previsto(real, previsto, &mut instantes, &mut cursor, &doc, &antes)?;
            }
            Op::Convert {
                target,
                allowed,
                desconocida,
            } => {
                let objetivo = desconocida.map_or(id, ImageId::from_raw);
                let real = doc.apply(convertir(objetivo, *target, *allowed));
                if desconocida.is_some() {
                    prop_assert_eq!(real, Err(DocumentError::UnknownImage(objetivo)));
                    prop_assert_eq!(estado(&doc), antes);
                } else if *target == actual.mode() {
                    // Convertir al modo actual no es un paso.
                    prop_assert_eq!(real, Ok(false));
                    prop_assert_eq!(estado(&doc), antes);
                } else {
                    match convert_image(&actual, *target, *allowed) {
                        Ok(conversion) => {
                            prop_assert_eq!(real, Ok(true));
                            prop_assert_ne!(doc.state_id(), antes.state_id);
                            instantes.truncate(cursor + 1);
                            instantes.push(conversion.image);
                            cursor += 1;
                            // `undo` deja la imagen anterior y `redo` la convertida.
                            let mut copia = doc.clone();
                            prop_assert_eq!(copia.undo(), Ok(()));
                            prop_assert_eq!(copia.image(id), Some(&actual));
                            prop_assert_eq!(copia.redo(), Ok(()));
                            prop_assert_eq!(copia.image(id), Some(&instantes[cursor]));
                        }
                        Err(e) => {
                            prop_assert_eq!(real, Err(DocumentError::Convert(e)));
                            prop_assert_eq!(estado(&doc), antes);
                        }
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
        // El documento es la instantánea del cursor, con el modo y la paleta.
        prop_assert_eq!(doc.image(id), Some(&instantes[cursor]));
        prop_assert_eq!(
            doc.image(id).map(Image::mode),
            Some(instantes[cursor].mode())
        );
        prop_assert_eq!(
            doc.image(id).and_then(Image::palette),
            instantes[cursor].palette()
        );
        prop_assert_eq!(doc.can_undo(), cursor > 0);
        prop_assert_eq!(doc.can_redo(), cursor + 1 < instantes.len());
        prop_assert_eq!(pasos_deshacibles(&doc), cursor);
        prop_assert_eq!(pasos_rehacibles(&doc), instantes.len() - 1 - cursor);
    }
    Ok(())
}

/// Compara el resultado de un `SetPixels` o `FillSpans` con lo previsto y actualiza el modelo.
fn aplica_previsto(
    real: Result<bool, DocumentError>,
    previsto: Previsto,
    instantes: &mut Vec<Image>,
    cursor: &mut usize,
    doc: &Document,
    antes: &Estado,
) -> Result<(), TestCaseError> {
    match previsto {
        Previsto::Nuevo(nueva) => {
            prop_assert_eq!(real, Ok(true));
            instantes.truncate(*cursor + 1);
            instantes.push(nueva);
            *cursor += 1;
        }
        Previsto::SinCambio => {
            prop_assert_eq!(real, Ok(false));
            prop_assert_eq!(&estado(doc), antes);
        }
        Previsto::Error => {
            prop_assert!(real.is_err(), "se esperaba un error y dio {:?}", real);
            prop_assert_eq!(&estado(doc), antes);
        }
    }
    Ok(())
}

// Propiedad 6.

/// Documento con una imagen y algo de historia: unos `SetPixels` válidos y, según `deshacer`,
/// un `undo` que deja un paso por rehacer.
fn documento_con_historia(img: Image, sets: &[(u32, u32, u8)], deshacer: bool) -> Document {
    let mut doc = Document::new(img);
    let id = doc.main_image();
    for &(x, y, v) in sets {
        let img = imagen_de(&doc);
        let pixel = pixel_de(img.mode(), longitud_de_paleta(img), v);
        let (x, y) = (x % img.width(), y % img.height());
        let _ = doc.apply(set(id, x, y, pixel));
    }
    if deshacer {
        let _ = doc.undo();
    }
    doc
}

#[derive(Clone, Debug)]
enum Forma {
    /// `ConvertMode` sobre la imagen principal.
    Suelto,
    /// `ConvertMode` con un identificador que no existe.
    Desconocida(u32),
    /// `Batch` con `n` `SetPixels` válidos seguidos de `ConvertMode`.
    EnLote(usize),
}

fn forma_de_comando() -> impl Strategy<Value = Forma> {
    prop_oneof![
        4 => Just(Forma::Suelto),
        1 => (1u32..5).prop_map(Forma::Desconocida),
        2 => (0usize..=2).prop_map(Forma::EnLote),
    ]
}

fn prop_atomicidad(
    img: Image,
    sets: Vec<(u32, u32, u8)>,
    deshacer: bool,
    forma: &Forma,
    target: Mode,
    allowed: Allowed,
) -> Result<(), TestCaseError> {
    let mut doc = documento_con_historia(img, &sets, deshacer);
    let id = doc.main_image();
    let antes = estado(&doc);
    let actual = imagen_de(&doc).clone();

    match forma {
        Forma::Desconocida(raw) => {
            let objetivo = ImageId::from_raw(*raw);
            prop_assert_eq!(
                doc.apply(convertir(objetivo, target, allowed)),
                Err(DocumentError::UnknownImage(objetivo))
            );
            prop_assert_eq!(estado(&doc), antes);
        }
        Forma::EnLote(n) => {
            let mut comandos: Vec<Command> = (0..*n)
                .map(|i| {
                    let pixel = pixel_de(actual.mode(), longitud_de_paleta(&actual), i as u8 + 2);
                    set(id, 0, 0, pixel)
                })
                .collect();
            comandos.push(convertir(id, target, allowed));
            prop_assert_eq!(
                doc.apply(Command::Batch(comandos)),
                Err(en_lote(*n, DocumentError::NotBatchable))
            );
            prop_assert_eq!(estado(&doc), antes);
        }
        Forma::Suelto => match doc.apply(convertir(id, target, allowed)) {
            Err(_) | Ok(false) => {
                prop_assert_eq!(estado(&doc), antes);
            }
            Ok(true) => {
                // Coste del paso: los bytes de píxeles de la imagen anterior, más una cota
                // para cabecera y paleta.
                let anterior = bytes_de(&actual);
                let crecimiento = doc.history_bytes() - antes.bytes;
                prop_assert!(
                    (anterior..=anterior + 2048).contains(&crecimiento),
                    "el paso cuesta {} y los píxeles anteriores son {}",
                    crecimiento,
                    anterior
                );
                prop_assert!(doc.can_undo());
                prop_assert!(!doc.can_redo());
                prop_assert_ne!(doc.state_id(), antes.state_id);

                // `undo` baja el coste al de antes y `redo` lo vuelve a subir.
                let tras_apply = doc.history_bytes();
                let mut copia = doc.clone();
                prop_assert_eq!(copia.undo(), Ok(()));
                prop_assert_eq!(copia.history_bytes(), antes.bytes);
                prop_assert_eq!(copia.image(id), Some(&actual));
                prop_assert_eq!(copia.redo(), Ok(()));
                prop_assert_eq!(copia.history_bytes(), tras_apply);
                prop_assert_eq!(copia.image(id), doc.image(id));
            }
        },
    }
    Ok(())
}

fn prop_limite(img: Image, limite: usize, ops: &[(Mode, Allowed)]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(img);
    doc.set_history_limit(limite);
    let id = doc.main_image();
    let mut hubo_paso = false;

    for &(target, allowed) in ops {
        let actual = imagen_de(&doc).clone();
        let antes = estado(&doc);
        match doc.apply(convertir(id, target, allowed)) {
            Ok(true) => {
                hubo_paso = true;
                // El último paso nunca se descarta: se puede deshacer y deja la anterior.
                let mut copia = doc.clone();
                prop_assert_eq!(copia.undo(), Ok(()));
                prop_assert_eq!(copia.image(id), Some(&actual));
            }
            Ok(false) | Err(_) => prop_assert_eq!(estado(&doc), antes),
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

proptest! {
    // Propiedad 5: `ConvertMode`, `SetPixels`, `FillSpans`, `undo` y `redo` contra un modelo
    // de instantáneas.
    #[test]
    fn convert_mode_y_undo_contra_un_modelo(
        img in imagen_6(),
        ops in prop::collection::vec(op(), 0..=20),
    ) {
        ejecuta_modelo(img, &ops)?;
    }

    // Propiedad 6, primera parte: atomicidad y coste (también en un `Batch` y con ids
    // desconocidos).
    #[test]
    fn atomicidad_y_coste_de_convert_mode(
        img in imagen_6(),
        sets in prop::collection::vec((any::<u32>(), any::<u32>(), 0u8..8), 0..=3),
        deshacer in any::<bool>(),
        forma in forma_de_comando(),
        target in modo(),
        allowed in permisos(),
    ) {
        prop_atomicidad(img, sets, deshacer, &forma, target, allowed)?;
    }

    // Propiedad 6, segunda parte: con un límite de 0 a 2000 bytes, `history_bytes() <= límite`
    // o solo queda un paso, y el último paso nunca se descarta.
    #[test]
    fn limite_de_la_historia_con_convert_mode(
        img in imagen_6(),
        limite in 0usize..=2000,
        ops in prop::collection::vec((modo(), todos_los_permisos()), 1..=8),
    ) {
        prop_limite(img, limite, &ops)?;
    }
}
