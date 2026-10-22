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

const ALL_2: [Mode; 4] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];

// Construcción de imágenes, documentos y comandos.

fn report_2(color_lost: u64, alpha_lost: u64) -> ConversionReport {
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

fn gray_img(width: u32, height: u32, pixels: &[u8]) -> Image {
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

fn image_of(doc: &Document) -> &Image {
    doc.image(ImageId::from_raw(0)).unwrap()
}

fn convert_2(image: ImageId, target: Mode, allowed: Allowed) -> Command {
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

fn failure_convert(e: ConvertError) -> DocumentError {
    DocumentError::Convert(e)
}

fn lost(color: u64, alpha: u64) -> DocumentError {
    failure_convert(ConvertError::LossNotAllowed {
        report: report_2(color, alpha),
    })
}

fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

/// Cota para que un `undo`/`redo` defectuoso que nunca falla no cuelgue los tests.
const CAP_STEPS: usize = 100_000;

/// Número de `undo()` que admite el documento (se cuenta en una copia).
fn steps_undoable(doc: &Document) -> usize {
    let mut copy = doc.clone();
    let mut n = 0;
    while n < CAP_STEPS && copy.undo().is_ok() {
        n += 1;
    }
    n
}

/// Número de `redo()` que admite el documento (se cuenta en una copia).
fn steps_redoable(doc: &Document) -> usize {
    let mut copy = doc.clone();
    let mut n = 0;
    while n < CAP_STEPS && copy.redo().is_ok() {
        n += 1;
    }
    n
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
#[derive(Debug, PartialEq, Eq)]
struct State {
    doc: Document,
    can_undo: bool,
    can_redo: bool,
    bytes: usize,
    state_id: u64,
    limit: usize,
    undoable: usize,
    redoable: usize,
}

fn state(doc: &Document) -> State {
    State {
        doc: doc.clone(),
        can_undo: doc.can_undo(),
        can_redo: doc.can_redo(),
        bytes: doc.history_bytes(),
        state_id: doc.state_id(),
        limit: doc.history_limit(),
        undoable: steps_undoable(doc),
        redoable: steps_redoable(doc),
    }
}

/// `apply(cmd)` da exactamente `expected` y no toca nada observable.
fn rejects(doc: &mut Document, cmd: Command, expected: DocumentError) {
    let before = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Err(expected), "{cmd:?}");
    assert_eq!(state(doc), before, "{cmd:?}");
}

/// Bytes de píxeles de una imagen, según `Image::byte_len`.
fn bytes_de(img: &Image) -> usize {
    Image::byte_len(img.width(), img.height(), img.mode()).unwrap()
}

// Tipos nuevos de la API.

#[test]
fn errors_new_of_the_document() {
    let convert = lost(2, 0);
    assert!(!convert.to_string().is_empty());
    assert!(!DocumentError::NotBatchable.to_string().is_empty());
    assert!(convert.source().is_some(), "Convert lleva su causa");
    assert_ne!(convert, DocumentError::NotBatchable);
    assert_ne!(convert, lost(1, 0));
    assert!(
        !in_batch(1, DocumentError::NotBatchable)
            .to_string()
            .is_empty()
    );

    let cmd = convert_2(ImageId::from_raw(0), Mode::Gray, N);
    assert_eq!(cmd, cmd.clone());
    assert_ne!(cmd, convert_2(ImageId::from_raw(0), Mode::Gray, L));
    assert_ne!(cmd, convert_2(ImageId::from_raw(0), Mode::Rgba, N));
    assert_ne!(cmd, convert_2(ImageId::from_raw(1), Mode::Gray, N));
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_13_convert_and_undo() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();

    // Sin permiso: error y nada cambia.
    rejects(&mut doc, convert_2(id, Mode::Gray, N), lost(2, 0));
    assert_eq!(image_of(&doc), &original);
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);

    // Con `luma`: un paso nuevo.
    let id_before = doc.state_id();
    assert_eq!(doc.apply(convert_2(id, Mode::Gray, L)), Ok(true));
    assert_ne!(doc.state_id(), id_before);
    assert_eq!(image_of(&doc).mode(), Mode::Gray);
    assert_eq!(image_of(&doc), &gray_img(2, 1, &[54, 19]));
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(steps_undoable(&doc), 1);
    let converted = doc.clone();

    // `undo` deja una imagen igual a la original, en su modo.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc).mode(), Mode::Rgba);
    assert_eq!(image_of(&doc), &original);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());

    // `redo` deja la Gray otra vez.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc), &gray_img(2, 1, &[54, 19]));
    assert_eq!(doc, converted);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn row_14_document_errors_leave_everything_intact() {
    // Pérdida no permitida.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    rejects(&mut doc, convert_2(id, Mode::Gray, N), lost(2, 0));

    // Imagen desconocida: el id 99.
    let unknown = ImageId::from_raw(99);
    rejects(
        &mut doc,
        convert_2(unknown, Mode::Indexed, N),
        DocumentError::UnknownImage(unknown),
    );
    rejects(
        &mut doc,
        convert_2(unknown, Mode::Rgba, L_Y_A),
        DocumentError::UnknownImage(unknown),
    );

    // Una imagen de 257 colores a `Indexed`.
    let colors_2: Vec<(u8, u8, u8, u8)> = (0..257usize)
        .map(|i| ((i % 256) as u8, (i / 256) as u8, 0, 255))
        .collect();
    let mut large = Document::new(rgba_img(257, 1, &colors_2));
    let id_large = ImageId::from_raw(0);
    rejects(
        &mut large,
        convert_2(id_large, Mode::Indexed, N),
        failure_convert(ConvertError::TooManyColors { at_least: 257 }),
    );
    // Con todos los permisos tampoco: no hay cuantización.
    rejects(
        &mut large,
        convert_2(id_large, Mode::Indexed, L_Y_A),
        failure_convert(ConvertError::TooManyColors { at_least: 257 }),
    );

    // `ConvertMode` dentro de un `Batch`: el `SetPixels` anterior se revierte.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let batch = Command::Batch(vec![
        set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
        convert_2(id, Mode::Gray, L),
    ]);
    rejects(&mut doc, batch, in_batch(1, DocumentError::NotBatchable));
    assert_eq!(image_of(&doc).mode(), Mode::Rgba);
    assert!(!doc.can_undo());
    assert_eq!(doc.history_bytes(), 0);

    // Lo mismo con historia previa y pila de rehacer.
    assert_eq!(
        doc.apply(set(id, 1, 0, Pixel::Rgba(Rgba::new(9, 9, 9, 255)))),
        Ok(true)
    );
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let batch = Command::Batch(vec![
        set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
        set(id, 1, 0, Pixel::Rgba(Rgba::new(4, 5, 6, 255))),
        convert_2(id, Mode::Gray, L_Y_A),
    ]);
    rejects(&mut doc, batch, in_batch(2, DocumentError::NotBatchable));
    assert!(doc.can_redo());
}

/// `apply(cmd)` falla con `InBatch { index, source }` donde `source` cumple `es`, y no toca
/// nada observable.
fn rejects_in_batch(
    doc: &mut Document,
    cmd: Command,
    index: usize,
    es: impl Fn(&DocumentError) -> bool,
) {
    let before = state(doc);
    match doc.apply(cmd.clone()) {
        Err(DocumentError::InBatch { index: i, source }) => {
            assert_eq!(i, index, "índice del comando que falló en {cmd:?}");
            assert!(es(&source), "error inesperado {source:?} en {cmd:?}");
        }
        other => panic!("se esperaba InBatch en {cmd:?} y salió {other:?}"),
    }
    assert_eq!(state(doc), before, "{cmd:?}");
}

#[test]
fn row_20_not_batchable_beats_unknown_image_and_the_first_error_wins() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let unknown = ImageId::from_raw(99);
    let outside = |image: ImageId| set(image, 5, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255)));
    let es_not_batchable = |e: &DocumentError| *e == DocumentError::NotBatchable;

    // Solo un `ConvertMode` sobre una imagen que no existe: no es `UnknownImage`.
    let batch = Command::Batch(vec![convert_2(unknown, Mode::Gray, N)]);
    rejects_in_batch(&mut doc, batch, 0, es_not_batchable);

    // Con un `SetPixels` válido antes: el índice es el del `ConvertMode`, y el `SetPixels` se
    // revierte. El modo de destino y los permisos no importan.
    for (target, allowed) in [(Mode::Rgba, L_Y_A), (Mode::Indexed, N), (Mode::Gray, L)] {
        let batch = Command::Batch(vec![
            set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
            convert_2(unknown, target, allowed),
        ]);
        rejects_in_batch(&mut doc, batch, 1, es_not_batchable);
        assert_eq!(
            image_of(&doc).get_pixel(0, 0),
            Ok(Pixel::Rgba(Rgba::new(255, 0, 0, 255)))
        );
    }

    // Gana el primer error del lote: un `SetPixels` fuera del lienzo antes del `ConvertMode`...
    let batch = Command::Batch(vec![outside(id), convert_2(unknown, Mode::Gray, L)]);
    rejects_in_batch(&mut doc, batch, 0, |e| {
        matches!(e, DocumentError::Pixel { .. })
    });
    // ...y el `ConvertMode` primero, aunque después haya otro fallo.
    let batch = Command::Batch(vec![convert_2(unknown, Mode::Gray, L), outside(id)]);
    rejects_in_batch(&mut doc, batch, 0, es_not_batchable);

    // Con historia previa y pila de rehacer, tampoco cambia nada.
    assert_eq!(
        doc.apply(set(id, 1, 0, Pixel::Rgba(Rgba::new(9, 9, 9, 255)))),
        Ok(true)
    );
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let batch = Command::Batch(vec![
        set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255))),
        convert_2(unknown, Mode::Gray, N),
    ]);
    rejects_in_batch(&mut doc, batch, 1, es_not_batchable);
    assert!(doc.can_redo());
}

#[test]
fn row_15_with_limit_zero_the_big_step_is_kept() {
    let mut doc = Document::new(Image::new(4, 4, Mode::Rgba).unwrap());
    let id = ImageId::from_raw(0);
    let rgba_initial = image_of(&doc).clone();
    doc.set_history_limit(0);

    assert_eq!(doc.apply(convert_2(id, Mode::Gray, L_Y_A)), Ok(true));
    // 64 bytes de píxeles de la imagen vieja, más una cota de 2048 para cabecera y paleta.
    let bytes = doc.history_bytes();
    assert!((64..=2112).contains(&bytes), "history_bytes() = {bytes}");
    assert_eq!(
        steps_undoable(&doc),
        1,
        "un paso mayor que el límite no se descarta"
    );
    assert_eq!(image_of(&doc).mode(), Mode::Gray);
    let after_conversion = image_of(&doc).clone();

    // Un `SetPixels` válido sobre la imagen Gray.
    assert_eq!(doc.apply(set(id, 0, 0, Pixel::Gray(7))), Ok(true));
    assert_eq!(image_of(&doc).get_pixel(0, 0), Ok(Pixel::Gray(7)));
    let final_ = image_of(&doc).clone();
    assert_ne!(final_, after_conversion);

    // Solo queda el último paso: un único `undo`, que deja la Gray de la conversión.
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &after_conversion);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(image_of(&doc), &after_conversion);

    // Y un `redo` que repite el estado final.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc), &final_);
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    assert_ne!(image_of(&doc), &rgba_initial);
}

#[test]
fn row_15_with_the_default_limit_everything_can_be_undone() {
    let mut doc = Document::new(Image::new(4, 4, Mode::Rgba).unwrap());
    let id = ImageId::from_raw(0);
    let rgba_initial = image_of(&doc).clone();

    assert_eq!(doc.apply(convert_2(id, Mode::Gray, L_Y_A)), Ok(true));
    let bytes = doc.history_bytes();
    assert!((64..=2112).contains(&bytes), "history_bytes() = {bytes}");
    let after_conversion = image_of(&doc).clone();
    assert_eq!(doc.apply(set(id, 0, 0, Pixel::Gray(7))), Ok(true));
    let final_ = image_of(&doc).clone();
    assert_eq!(steps_undoable(&doc), 2);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &after_conversion);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &rgba_initial);
    assert_eq!(image_of(&doc).mode(), Mode::Rgba);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc), &after_conversion);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc), &final_);
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
}

#[test]
fn row_19_converting_to_the_current_mode_is_not_a_step() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(
        doc.apply(set(id, 0, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 255)))),
        Ok(true)
    );
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let before = state(&doc);

    // Con cualquier `allowed`, incluso con permisos de más: no se llama a `convert_image`.
    let permissions = [
        N,
        L,
        L_Y_A,
        Allowed {
            luma: false,
            drop_alpha: Some([3, 4, 5]),
        },
    ];
    for allowed in permissions {
        assert_eq!(
            doc.apply(convert_2(id, Mode::Rgba, allowed)),
            Ok(false),
            "{allowed:?}"
        );
        assert_eq!(state(&doc), before, "{allowed:?}");
        assert!(doc.can_redo(), "no borra la pila de rehacer");
        assert_eq!(doc.state_id(), before.state_id);
        assert_eq!(doc.history_bytes(), before.bytes);
    }

    // Lo que había por rehacer sigue ahí.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(
        image_of(&doc).get_pixel(0, 0),
        Ok(Pixel::Rgba(Rgba::new(1, 2, 3, 255)))
    );
}

// Estrategias.

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(ALL_2.to_vec())
}

const CHANNELS: [u8; 5] = [0, 1, 127, 128, 255];

fn channel() -> impl Strategy<Value = u8> {
    prop_oneof![any::<u8>(), prop::sample::select(CHANNELS.to_vec())]
}

fn small() -> impl Strategy<Value = u8> {
    prop::sample::select(CHANNELS.to_vec())
}

/// Colores uniformes, del conjunto pequeño y grises con alfa de un conjunto pequeño.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
        (small(), small(), small(), small()).prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
        (channel(), small()).prop_map(|(v, a)| Rgba::new(v, v, v, a)),
    ]
}

fn dims() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=6, 1u32..=6)
}

/// Imagen de hasta 6 x 6 en cualquier modo; la indexada lleva una paleta de 1 a 8 entradas
/// y a veces un índice transparente.
fn image_6() -> impl Strategy<Value = Image> {
    (
        mode_2(),
        dims(),
        prop::collection::vec(color(), 36),
        prop::collection::vec(color(), 1..=8),
        prop::option::of(any::<u8>()),
        prop::collection::vec(any::<u8>(), 36),
    )
        .prop_map(|(mode, (w, h), colors_2, palette_2, t, idx)| {
            let n = (w * h) as usize;
            let en = |i: usize| (i as u32 % w, i as u32 / w);
            let mut img = match mode {
                Mode::Indexed => {
                    let mut p = Palette::from_colors(palette_2.iter().copied()).unwrap();
                    p.set_transparent_index(t.map(|t| (t as usize % palette_2.len()) as u8))
                        .unwrap();
                    Image::new_indexed(w, h, p).unwrap()
                }
                _ => Image::new(w, h, mode).unwrap(),
            };
            for i in 0..n {
                let c = colors_2[i];
                let pixel = match mode {
                    Mode::Rgba => Pixel::Rgba(c),
                    Mode::GrayAlpha => Pixel::GrayAlpha {
                        gray: c.r,
                        alpha: c.a,
                    },
                    Mode::Gray => Pixel::Gray(c.g),
                    Mode::Indexed => Pixel::Indexed((idx[i] as usize % palette_2.len()) as u8),
                };
                let (px, py) = en(i);
                img.set_pixel(px, py, pixel).unwrap();
            }
            img
        })
}

fn permissions() -> impl Strategy<Value = Allowed> {
    (
        prop_oneof![2 => Just(true), 1 => Just(false)],
        prop_oneof![
            2 => (channel(), channel(), channel()).prop_map(|(r, g, b)| Some([r, g, b])),
            1 => Just(None),
        ],
    )
        .prop_map(|(luma, drop_alpha)| Allowed { luma, drop_alpha })
}

/// Permisos para conversiones que no fallan por pérdida.
fn all_the_permissions() -> impl Strategy<Value = Allowed> {
    (channel(), channel(), channel()).prop_map(|(r, g, b)| Allowed {
        luma: true,
        drop_alpha: Some([r, g, b]),
    })
}

fn alpha_of(v: u8) -> u8 {
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
        Mode::Rgba if v.is_multiple_of(2) => Pixel::Rgba(Rgba::new(v, v, v, alpha_of(v))),
        Mode::Rgba => Pixel::Rgba(Rgba::new(v, v ^ 0x55, v.wrapping_mul(3), alpha_of(v))),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: v,
            alpha: alpha_of(v),
        },
        Mode::Gray => Pixel::Gray(v),
        Mode::Indexed => Pixel::Indexed((usize::from(v) % len.max(1)) as u8),
    }
}

fn other_mode(mode: Mode) -> Mode {
    let i = ALL_2.iter().position(|&m| m == mode).unwrap();
    ALL_2[(i + 1) % ALL_2.len()]
}

fn length_of_palette(img: &Image) -> usize {
    img.palette().map_or(0, Palette::len)
}

/// Un píxel válido (o inválido, si `bad`) para la imagen actual.
fn pixel_for(img: &Image, v: u8, bad: bool) -> Pixel {
    if bad {
        pixel_de(other_mode(img.mode()), 8, v)
    } else {
        pixel_de(img.mode(), length_of_palette(img), v)
    }
}

// Modelo de instantáneas (propiedad 5).

#[derive(Clone, Debug)]
enum Op {
    Set {
        edits: Vec<(u32, u32, u8)>,
        bad: bool,
    },
    Fill {
        y: u32,
        x0: u32,
        x1: u32,
        v: u8,
        bad: bool,
    },
    Convert {
        target: Mode,
        allowed: Allowed,
        unknown: Option<u32>,
    },
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => (prop::collection::vec((0u32..8, 0u32..8, 0u8..8), 1..=3), prop_oneof![5 => Just(false), 1 => Just(true)])
            .prop_map(|(edits, bad)| Op::Set { edits, bad }),
        2 => (0u32..8, 0u32..8, 0u32..8, 0u8..8, prop_oneof![5 => Just(false), 1 => Just(true)])
            .prop_map(|(y, x0, x1, v, bad)| Op::Fill { y, x0, x1, v, bad }),
        5 => (mode_2(), permissions(), prop_oneof![9 => Just(None), 1 => (1u32..5).prop_map(Some)])
            .prop_map(|(target, allowed, unknown)| Op::Convert { target, allowed, unknown }),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
    ]
}

/// Lo que el modelo espera de un `SetPixels` o un `FillSpans`.
enum Expected {
    Error,
    WithoutChange,
    New(Image),
}

fn model_set(img: &Image, pixels: &[PixelEdit]) -> Expected {
    let mut work = img.clone();
    let mut change = false;
    for e in pixels {
        let before = work.get_pixel(e.x, e.y);
        if work.set_pixel(e.x, e.y, e.pixel).is_err() {
            return Expected::Error;
        }
        change |= before != Ok(e.pixel);
    }
    if change {
        Expected::New(work)
    } else {
        Expected::WithoutChange
    }
}

fn model_fill(img: &Image, span: Span, pixel: Pixel) -> Expected {
    if img.check_pixel(pixel).is_err()
        || span.x_start >= span.x_end
        || span.y >= img.height()
        || span.x_end > img.width()
    {
        return Expected::Error;
    }
    let mut work = img.clone();
    let mut change = false;
    for x in span.x_start..span.x_end {
        if work.get_pixel(x, span.y) != Ok(pixel) {
            work.set_pixel(x, span.y, pixel).unwrap();
            change = true;
        }
    }
    if change {
        Expected::New(work)
    } else {
        Expected::WithoutChange
    }
}

/// Ejecuta `ops` sobre un documento y sobre el modelo (instantáneas de la imagen y un
/// cursor) y los compara tras cada operación.
fn runs_model(initial: Image, ops: &[Op]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(initial.clone());
    let id = ImageId::from_raw(0);
    let mut instants = vec![initial];
    let mut cursor = 0usize;

    for op in ops {
        let actual = instants[cursor].clone();
        let before = state(&doc);
        match op {
            Op::Set { edits, bad } => {
                let pixels: Vec<PixelEdit> = edits
                    .iter()
                    .map(|&(x, y, v)| PixelEdit {
                        x,
                        y,
                        pixel: pixel_for(&actual, v, *bad),
                    })
                    .collect();
                let expected_2 = model_set(&actual, &pixels);
                let real = doc.apply(Command::SetPixels { image: id, pixels });
                applies_expected(real, expected_2, &mut instants, &mut cursor, &doc, &before)?;
            }
            Op::Fill { y, x0, x1, v, bad } => {
                let span = Span {
                    y: *y,
                    x_start: *x0,
                    x_end: *x1,
                };
                let pixel = pixel_for(&actual, *v, *bad);
                let expected_2 = model_fill(&actual, span, pixel);
                let real = doc.apply(Command::FillSpans {
                    image: id,
                    spans: vec![span],
                    pixel,
                });
                applies_expected(real, expected_2, &mut instants, &mut cursor, &doc, &before)?;
            }
            Op::Convert {
                target,
                allowed,
                unknown,
            } => {
                let goal = unknown.map_or(id, ImageId::from_raw);
                let real = doc.apply(convert_2(goal, *target, *allowed));
                if unknown.is_some() {
                    prop_assert_eq!(real, Err(DocumentError::UnknownImage(goal)));
                    prop_assert_eq!(state(&doc), before);
                } else if *target == actual.mode() {
                    // Convertir al modo actual no es un paso.
                    prop_assert_eq!(real, Ok(false));
                    prop_assert_eq!(state(&doc), before);
                } else {
                    match convert_image(&actual, *target, *allowed) {
                        Ok(conversion) => {
                            prop_assert_eq!(real, Ok(true));
                            prop_assert_ne!(doc.state_id(), before.state_id);
                            instants.truncate(cursor + 1);
                            instants.push(conversion.image);
                            cursor += 1;
                            // `undo` deja la imagen anterior y `redo` la convertida.
                            let mut copy = doc.clone();
                            prop_assert_eq!(copy.undo(), Ok(()));
                            prop_assert_eq!(copy.image(id), Some(&actual));
                            prop_assert_eq!(copy.redo(), Ok(()));
                            prop_assert_eq!(copy.image(id), Some(&instants[cursor]));
                        }
                        Err(e) => {
                            prop_assert_eq!(real, Err(DocumentError::Convert(e)));
                            prop_assert_eq!(state(&doc), before);
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
                    prop_assert_eq!(state(&doc), before);
                }
            }
            Op::Redo => {
                if cursor + 1 < instants.len() {
                    prop_assert_eq!(doc.redo(), Ok(()));
                    cursor += 1;
                } else {
                    prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
                    prop_assert_eq!(state(&doc), before);
                }
            }
        }
        // El documento es la instantánea del cursor, con el modo y la paleta.
        prop_assert_eq!(doc.image(id), Some(&instants[cursor]));
        prop_assert_eq!(
            doc.image(id).map(Image::mode),
            Some(instants[cursor].mode())
        );
        prop_assert_eq!(
            doc.image(id).and_then(Image::palette),
            instants[cursor].palette()
        );
        prop_assert_eq!(doc.can_undo(), cursor > 0);
        prop_assert_eq!(doc.can_redo(), cursor + 1 < instants.len());
        prop_assert_eq!(steps_undoable(&doc), cursor);
        prop_assert_eq!(steps_redoable(&doc), instants.len() - 1 - cursor);
    }
    Ok(())
}

/// Compara el resultado de un `SetPixels` o `FillSpans` con lo previsto y actualiza el modelo.
fn applies_expected(
    real: Result<bool, DocumentError>,
    expected_2: Expected,
    instants: &mut Vec<Image>,
    cursor: &mut usize,
    doc: &Document,
    before: &State,
) -> Result<(), TestCaseError> {
    match expected_2 {
        Expected::New(new) => {
            prop_assert_eq!(real, Ok(true));
            instants.truncate(*cursor + 1);
            instants.push(new);
            *cursor += 1;
        }
        Expected::WithoutChange => {
            prop_assert_eq!(real, Ok(false));
            prop_assert_eq!(&state(doc), before);
        }
        Expected::Error => {
            prop_assert!(real.is_err(), "se esperaba un error y dio {:?}", real);
            prop_assert_eq!(&state(doc), before);
        }
    }
    Ok(())
}

// Propiedad 6.

/// Documento con una imagen y algo de historia: unos `SetPixels` válidos y, según `undo`,
/// un `undo` que deja un paso por rehacer.
fn document_with_history(img: Image, sets: &[(u32, u32, u8)], undo: bool) -> Document {
    let mut doc = Document::new(img);
    let id = ImageId::from_raw(0);
    for &(x, y, v) in sets {
        let img = image_of(&doc);
        let pixel = pixel_de(img.mode(), length_of_palette(img), v);
        let (x, y) = (x % img.width(), y % img.height());
        let _ = doc.apply(set(id, x, y, pixel));
    }
    if undo {
        let _ = doc.undo();
    }
    doc
}

#[derive(Clone, Debug)]
enum Shape {
    /// `ConvertMode` sobre la imagen principal.
    Loose,
    /// `ConvertMode` con un identificador que no existe.
    Unknown(u32),
    /// `Batch` con `n` `SetPixels` válidos seguidos de `ConvertMode`.
    InBatch(usize),
}

fn shape_of_command() -> impl Strategy<Value = Shape> {
    prop_oneof![
        4 => Just(Shape::Loose),
        1 => (1u32..5).prop_map(Shape::Unknown),
        2 => (0usize..=2).prop_map(Shape::InBatch),
    ]
}

fn prop_atomicity(
    img: Image,
    sets: Vec<(u32, u32, u8)>,
    undo: bool,
    shape: &Shape,
    target: Mode,
    allowed: Allowed,
) -> Result<(), TestCaseError> {
    let mut doc = document_with_history(img, &sets, undo);
    let id = ImageId::from_raw(0);
    let before = state(&doc);
    let actual = image_of(&doc).clone();

    match shape {
        Shape::Unknown(raw) => {
            let goal = ImageId::from_raw(*raw);
            prop_assert_eq!(
                doc.apply(convert_2(goal, target, allowed)),
                Err(DocumentError::UnknownImage(goal))
            );
            prop_assert_eq!(state(&doc), before);
        }
        Shape::InBatch(n) => {
            let mut commands: Vec<Command> = (0..*n)
                .map(|i| {
                    let pixel = pixel_de(actual.mode(), length_of_palette(&actual), i as u8 + 2);
                    set(id, 0, 0, pixel)
                })
                .collect();
            commands.push(convert_2(id, target, allowed));
            prop_assert_eq!(
                doc.apply(Command::Batch(commands)),
                Err(in_batch(*n, DocumentError::NotBatchable))
            );
            prop_assert_eq!(state(&doc), before);
        }
        Shape::Loose => match doc.apply(convert_2(id, target, allowed)) {
            Err(_) | Ok(false) => {
                prop_assert_eq!(state(&doc), before);
            }
            Ok(true) => {
                // Coste del paso: los bytes de píxeles de la imagen anterior, más una cota
                // para cabecera y paleta.
                let previous = bytes_de(&actual);
                let growth = doc.history_bytes() - before.bytes;
                prop_assert!(
                    (previous..=previous + 2048).contains(&growth),
                    "el paso cuesta {} y los píxeles anteriores son {}",
                    growth,
                    previous
                );
                prop_assert!(doc.can_undo());
                prop_assert!(!doc.can_redo());
                prop_assert_ne!(doc.state_id(), before.state_id);

                // `undo` baja el coste al de antes y `redo` lo vuelve a subir.
                let after_apply = doc.history_bytes();
                let mut copy = doc.clone();
                prop_assert_eq!(copy.undo(), Ok(()));
                prop_assert_eq!(copy.history_bytes(), before.bytes);
                prop_assert_eq!(copy.image(id), Some(&actual));
                prop_assert_eq!(copy.redo(), Ok(()));
                prop_assert_eq!(copy.history_bytes(), after_apply);
                prop_assert_eq!(copy.image(id), doc.image(id));
            }
        },
    }
    Ok(())
}

fn prop_limit(img: Image, limit: usize, ops: &[(Mode, Allowed)]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(img);
    doc.set_history_limit(limit);
    let id = ImageId::from_raw(0);
    let mut was_step = false;

    for &(target, allowed) in ops {
        let actual = image_of(&doc).clone();
        let before = state(&doc);
        match doc.apply(convert_2(id, target, allowed)) {
            Ok(true) => {
                was_step = true;
                // El último paso nunca se descarta: se puede deshacer y deja la anterior.
                let mut copy = doc.clone();
                prop_assert_eq!(copy.undo(), Ok(()));
                prop_assert_eq!(copy.image(id), Some(&actual));
            }
            Ok(false) | Err(_) => prop_assert_eq!(state(&doc), before),
        }
        let kept = steps_undoable(&doc);
        prop_assert!(
            doc.history_bytes() <= limit || kept <= 1,
            "history_bytes = {} > límite {} con {} pasos",
            doc.history_bytes(),
            limit,
            kept
        );
        prop_assert_eq!(doc.history_limit(), limit);
        if was_step {
            prop_assert!(kept >= 1, "el último paso siempre se conserva");
            prop_assert!(doc.can_undo());
        }
    }
    Ok(())
}

proptest! {
    // Propiedad 5: `ConvertMode`, `SetPixels`, `FillSpans`, `undo` y `redo` contra un modelo
    // de instantáneas.
    #[test]
    fn convert_mode_and_undo_against_a_model(
        img in image_6(),
        ops in prop::collection::vec(op(), 0..=20),
    ) {
        runs_model(img, &ops)?;
    }

    // Propiedad 6, primera parte: atomicidad y coste (también en un `Batch` y con ids
    // desconocidos).
    #[test]
    fn atomicity_and_cost_of_convert_mode(
        img in image_6(),
        sets in prop::collection::vec((any::<u32>(), any::<u32>(), 0u8..8), 0..=3),
        undo in any::<bool>(),
        shape in shape_of_command(),
        target in mode_2(),
        allowed in permissions(),
    ) {
        prop_atomicity(img, sets, undo, &shape, target, allowed)?;
    }

    // Propiedad 6, segunda parte: con un límite de 0 a 2000 bytes, `history_bytes() <= límite`
    // o solo queda un paso, y el último paso nunca se descarta.
    #[test]
    fn history_limit_with_convert_mode(
        img in image_6(),
        limit in 0usize..=2000,
        ops in prop::collection::vec((mode_2(), all_the_permissions()), 1..=8),
    ) {
        prop_limit(img, limit, &ops)?;
    }
}
