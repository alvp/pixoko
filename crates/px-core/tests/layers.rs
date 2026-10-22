//! Tests del modelo de capas de `Document` (specs/hito-3-capas-modelo.md, unidad 3.1): `LayerId`,
//! `Layer`, el lienzo (`width`, `height`, `mode`, `palette`), las celdas vacías que se
//! materializan al escribir, los seis comandos de capa (`AddLayer`, `RemoveLayer`, `MoveLayer`,
//! `RenameLayer`, `SetLayerOpacity`, `SetLayerVisible`), los límites (`MAX_LAYERS`,
//! `MAX_LAYER_NAME_BYTES`, `cells_limit`) y los nueve `DocumentError` nuevos.
//!
//! Una prueba por cada una de las 12 filas de «Casos» (`fila_NN_...`; las filas con varios casos
//! se reparten en `fila_NN_a`, `fila_NN_b`...) y una propiedad por cada una de las 5 de
//! «Propiedades» (`propiedad_N_...`, 256 casos). Las cifras de las filas son las de la spec,
//! calculadas a mano (RGBA 4 x 3: celda de 48 bytes, dos celdas 96): no se recalculan con el
//! código. Ninguna prueba reserva más de unos pocos KiB: las imágenes miden como mucho 4 x 4 y
//! los documentos de 256 capas son de 1 x 1. El coste de la historia solo se mide con
//! `history_bytes()` y con las cotas de la spec; nunca se asume el valor de `size_of`.
//!
//! Este fichero es la fase de pruebas en rojo: la API de capas no existe todavía, así que no
//! compila hasta la lógica B y C de la spec.
//!
//! Los `// SPEC?:` marcan dudas que la spec no resuelve; ahí la prueba solo comprueba lo que la
//! spec dice y evita lo demás.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::Debug;
use std::hash::Hash;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, Command, Document, DocumentError, Image, ImageId, Layer, LayerId, Mode, Palette,
    Pixel, PixelEdit, Rgba, Span,
};

// ---------------------------------------------------------------------------------------
// Colores, imágenes y documentos de apoyo.
// ---------------------------------------------------------------------------------------

/// `t` de la spec: el píxel transparente.
const T: Rgba = Rgba::TRANSPARENT;
const C1: Rgba = Rgba::new(40, 50, 60, 255);
const C2: Rgba = Rgba::new(70, 80, 90, 255);

const RED: Rgba = Rgba::new(255, 0, 0, 255);
const GREEN: Rgba = Rgba::new(0, 255, 0, 255);
const BLUE: Rgba = Rgba::new(0, 0, 255, 255);
const BLACK: Rgba = Rgba::new(0, 0, 0, 255);
const YELLOW: Rgba = Rgba::new(255, 255, 0, 255);
const WHITE: Rgba = Rgba::new(255, 255, 255, 255);

fn img_rgba(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Rgba).expect("imagen RGBA")
}

/// `D` de la spec: documento RGBA de 4 x 3 recién creado.
fn doc_d() -> Document {
    Document::new(img_rgba(4, 3))
}

/// Imagen indexada con la paleta y el índice transparente dados (todos los píxeles en 0).
fn img_indexed(w: u32, h: u32, colors_2: &[Rgba], t: Option<u8>) -> Image {
    let mut palette_2 = Palette::from_colors(colors_2.iter().copied()).expect("paleta");
    palette_2
        .set_transparent_index(t)
        .expect("índice transparente");
    Image::new_indexed(w, h, palette_2).expect("imagen indexada")
}

/// `n` colores opacos distintos (n <= 256).
fn colors_different(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

// Comandos de apoyo.

fn set_px(image: ImageId, x: u32, y: u32, pixel: Pixel) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit { x, y, pixel }],
    }
}

/// `S(c, [(x, y, color)])` de la spec.
fn sets(image: ImageId, x: u32, y: u32, c: Rgba) -> Command {
    set_px(image, x, y, Pixel::Rgba(c))
}

/// `F(c, row 0, color)` de la spec, en una celda de `width` píxeles de ancho.
fn row_0(image: ImageId, width: u32, c: Rgba) -> Command {
    Command::FillSpans {
        image,
        spans: vec![Span {
            y: 0,
            x_start: 0,
            x_end: width,
        }],
        pixel: Pixel::Rgba(c),
    }
}

fn adds_cmd(name_2: &str, at: usize) -> Command {
    Command::AddLayer {
        name: name_2.to_string(),
        at,
    }
}

fn renames(layer: LayerId, name_2: &str) -> Command {
    Command::RenameLayer {
        layer,
        name: name_2.to_string(),
    }
}

fn opacity_2(layer: LayerId, opacity: u8) -> Command {
    Command::SetLayerOpacity { layer, opacity }
}

fn visible(layer: LayerId, visible: bool) -> Command {
    Command::SetLayerVisible { layer, visible }
}

fn moves(layer: LayerId, to: usize) -> Command {
    Command::MoveLayer { layer, to }
}

fn removes(layer: LayerId) -> Command {
    Command::RemoveLayer { layer }
}

fn without_transparent(image: ImageId) -> Command {
    Command::SetTransparentIndex { image, index: None }
}

// Lectura de documentos.

/// Añade una capa en `at` y devuelve su `LayerId` (se lee de `layers()[at]` tras el `Ok(true)`).
fn adds(doc: &mut Document, name_2: &str, at: usize) -> LayerId {
    assert_eq!(doc.apply(adds_cmd(name_2, at)), Ok(true), "añadir {name_2}");
    doc.layers()[at].id()
}

/// `cel(X)` de la spec.
fn cel(doc: &Document, layer_2: LayerId) -> ImageId {
    doc.layer(layer_2).expect("la capa existe").cel()
}

fn ids(doc: &Document) -> Vec<(LayerId, ImageId)> {
    doc.layers().iter().map(|l| (l.id(), l.cel())).collect()
}

fn names(doc: &Document) -> Vec<String> {
    doc.layers().iter().map(|l| l.name().to_string()).collect()
}

fn pixel_rgba(doc: &Document, cel: ImageId, x: u32, y: u32) -> Rgba {
    let image_2 = doc.image(cel).expect("la celda tiene imagen");
    match image_2.get_pixel(x, y).expect("píxel dentro") {
        Pixel::Rgba(c) => c,
        other_2 => panic!("se esperaba RGBA, hay {other_2:?}"),
    }
}

/// Índices de paleta de todos los píxeles de la celda, fila a fila.
fn indices(doc: &Document, cel: ImageId) -> Vec<u8> {
    let image_2 = doc.image(cel).expect("la celda tiene imagen");
    let mut v = Vec::new();
    for y in 0..image_2.height() {
        for x in 0..image_2.width() {
            match image_2.get_pixel(x, y).expect("píxel dentro") {
                Pixel::Indexed(i) => v.push(i),
                other_2 => panic!("se esperaba indexado, hay {other_2:?}"),
            }
        }
    }
    v
}

/// `[A, B, C]`: `D` con dos capas vacías encima.
fn three_layers() -> (Document, [LayerId; 3]) {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    (d, [a, b, c])
}

/// Todo lo observable que un comando fallido o sin cambios no puede tocar.
struct Snapshot {
    doc: Document,
    can_undo_2: bool,
    can_redo_2: bool,
    history: usize,
    state: u64,
    materialized: usize,
}

impl Snapshot {
    fn of(doc: &Document) -> Snapshot {
        Snapshot {
            doc: doc.clone(),
            can_undo_2: doc.can_undo(),
            can_redo_2: doc.can_redo(),
            history: doc.history_bytes(),
            state: doc.state_id(),
            materialized: doc.materialized_bytes(),
        }
    }

    fn difference(&self, doc: &Document) -> Option<&'static str> {
        if *doc != self.doc {
            Some("el documento cambió")
        } else if doc.can_undo() != self.can_undo_2 {
            Some("can_undo cambió")
        } else if doc.can_redo() != self.can_redo_2 {
            Some("can_redo cambió")
        } else if doc.history_bytes() != self.history {
            Some("history_bytes cambió")
        } else if doc.state_id() != self.state {
            Some("state_id cambió")
        } else if doc.materialized_bytes() != self.materialized {
            Some("materialized_bytes cambió")
        } else {
            None
        }
    }

    fn assert_equal(&self, doc: &Document, context: &str) {
        assert_eq!(self.difference(doc), None, "{context}");
    }

    fn requires(&self, doc: &Document, context: &str) -> Result<(), TestCaseError> {
        match self.difference(doc) {
            None => Ok(()),
            Some(reason) => Err(TestCaseError::fail(format!("{context}: {reason}"))),
        }
    }

    /// Solo el documento, su `state_id` y los bytes materializados (lo que dicen las
    /// propiedades 1 y 5 al comparar dos estados).
    fn requires_state(&self, doc: &Document, context: &str) -> Result<(), TestCaseError> {
        if *doc != self.doc {
            return Err(TestCaseError::fail(format!(
                "{context}: el documento no es igual"
            )));
        }
        if doc.state_id() != self.state {
            return Err(TestCaseError::fail(format!(
                "{context}: state_id {} en vez de {}",
                doc.state_id(),
                self.state
            )));
        }
        if doc.materialized_bytes() != self.materialized {
            return Err(TestCaseError::fail(format!(
                "{context}: materialized_bytes {} en vez de {}",
                doc.materialized_bytes(),
                self.materialized
            )));
        }
        Ok(())
    }
}

/// Aplica `cmd`, comprueba el resultado y que el documento, la historia y el `state_id` no
/// cambiaron.
fn without_effect(doc: &mut Document, cmd: Command, expected_2: Result<bool, DocumentError>) {
    let snapshot = Snapshot::of(doc);
    let context = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), expected_2, "{context}");
    snapshot.assert_equal(doc, &context);
}

/// `InBatch { index, source }`.
fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

/// Los seis comandos de capa, todos con `Ok(true)` en un documento con la capa `a` y alguna más.
fn six_commands(a: LayerId) -> Vec<Command> {
    vec![
        adds_cmd("Z", 0),
        removes(a),
        moves(a, 1),
        renames(a, "Z"),
        opacity_2(a, 128),
        visible(a, false),
    ]
}

// Comprobaciones en tiempo de compilación de los traits exigidos (C-01, C-02).
fn assert_id_traits<X: Clone + Copy + Debug + PartialEq + Eq + Hash + PartialOrd + Ord>() {}
fn assert_datum_traits<X: Clone + Debug + PartialEq + Eq>() {}

#[test]
fn traits_ids_and_messages_of_the_new_errors() {
    // C-01, C-02: traits derivados; C-29: un mensaje por variante nueva y `source` vacío.
    assert_id_traits::<LayerId>();
    assert_datum_traits::<Layer>();
    assert_eq!(LayerId::from_raw(7).raw(), 7);
    let errors = [
        DocumentError::UnknownLayer(LayerId::from_raw(3)),
        DocumentError::LayerIndex { index: 5, max: 2 },
        DocumentError::LastLayer,
        DocumentError::TooManyLayers { limit: 256 },
        DocumentError::LayerName {
            len: 256,
            limit: 255,
        },
        DocumentError::NoTransparency { mode: Mode::Gray },
        DocumentError::IndexedOpacity,
        DocumentError::MultiLayerConvert { layers: 2 },
        DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        },
    ];
    let mut messages = HashSet::new();
    for error in &errors {
        let message = error.to_string();
        assert!(!message.is_empty(), "{error:?}");
        assert!(error.source().is_none(), "{error:?}");
        messages.insert(message);
    }
    assert_eq!(messages.len(), errors.len(), "los mensajes son distintos");
}

// ---------------------------------------------------------------------------------------
// Fila 1: `Document::new`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_01_a_new_document_has_one_layer() {
    let mut img = img_rgba(4, 3);
    img.set_pixel(1, 1, Pixel::Rgba(C1)).expect("píxel");
    let d = Document::new(img.clone());

    assert_eq!(d.layers().len(), 1);
    let a = &d.layers()[0];
    assert_eq!(a.name(), "Capa 1");
    assert_eq!(a.id(), LayerId::from_raw(0));
    assert!(a.visible());
    assert_eq!(a.opacity(), 255);
    assert_eq!(a.cel(), ImageId::from_raw(0));
    assert_eq!(d.image(a.cel()), Some(&img));
    assert_eq!(d.layer(a.id()), Some(a));
    assert_eq!(d.layer(LayerId::from_raw(1)), None);

    assert_eq!(d.width(), 4);
    assert_eq!(d.height(), 3);
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert_eq!(d.cel_bytes(), 48);
    assert_eq!(d.materialized_bytes(), 48);
    assert_eq!(d.worst_case_bytes(), 48);
    assert_eq!(d.state_id(), 0);
    assert!(!d.can_undo());
    assert!(!d.can_redo());
    assert_eq!(d.history_bytes(), 0);
    assert_eq!(d.cells_limit(), Document::DEFAULT_CELLS_LIMIT);
}

#[test]
fn row_01_b_the_indexed_canvas_takes_the_image_palette() {
    let img = img_indexed(4, 3, &[RED, GREEN, BLUE, BLACK], Some(1));
    let d = Document::new(img.clone());
    assert_eq!(d.mode(), Mode::Indexed);
    assert_eq!((d.width(), d.height()), (4, 3));
    assert_eq!(d.palette(), img.palette());
    assert!(d.palette().is_some());
    // `cel_bytes` son los bytes de píxeles, sin la paleta: 4 x 3 x 1.
    assert_eq!(d.cel_bytes(), 12);
    assert_eq!(d.materialized_bytes(), 12);
    assert_eq!(d.worst_case_bytes(), 12);
    assert_eq!(d.layers()[0].name(), "Capa 1");
    assert_eq!(d.image(ImageId::from_raw(0)), Some(&img));
}

// ---------------------------------------------------------------------------------------
// Fila 2: AddLayer en varias posiciones; deshacer y rehacer.
// ---------------------------------------------------------------------------------------

#[test]
fn row_02_add_layers_at_positions_and_undo_redo() {
    let mut d = doc_d();
    let original = d.clone();
    let mut cels_views = vec![ImageId::from_raw(0)];
    let mut layers_2 = vec![d.layers().to_vec()];
    let mut documents = vec![d.clone()];

    // `at == len` (encima de todas), el fondo y el medio.
    for (name_2, at) in [("B", 1usize), ("N", 0), ("M", 2)] {
        let before = d.materialized_bytes();
        assert_eq!(d.apply(adds_cmd(name_2, at)), Ok(true), "{name_2}");
        let new = &d.layers()[at];
        assert_eq!(new.name(), name_2);
        assert!(new.visible());
        assert_eq!(new.opacity(), 255);
        assert!(d.image(new.cel()).is_none(), "{name_2} nace vacía");
        assert!(!cels_views.contains(&new.cel()), "{name_2}: ImageId nuevo");
        cels_views.push(new.cel());
        assert_eq!(d.materialized_bytes(), before, "{name_2}");
        layers_2.push(d.layers().to_vec());
        documents.push(d.clone());
    }
    assert_eq!(d.layers().len(), 4);
    let ids_layer: HashSet<LayerId> = d.layers().iter().map(Layer::id).collect();
    assert_eq!(ids_layer.len(), 4, "LayerId distintos");

    // Un `undo` quita la última añadida; `redo` la repone con los mismos ids.
    d.undo().expect("deshacer");
    assert_eq!(d.layers(), layers_2[2].as_slice());
    assert_eq!(d, documents[2]);
    d.redo().expect("rehacer");
    assert_eq!(d.layers(), layers_2[3].as_slice());
    assert_eq!(d, documents[3]);

    for (i, expected_2_2) in layers_2.iter().enumerate().take(3).rev() {
        d.undo().expect("deshacer");
        assert_eq!(
            d.layers(),
            expected_2_2.as_slice(),
            "tras deshacer hasta {i}"
        );
    }
    assert_eq!(d, original);
    assert_eq!(d.state_id(), 0);
    assert!(!d.can_undo());
}

// ---------------------------------------------------------------------------------------
// Fila 3: escribir en una celda vacía la materializa.
// ---------------------------------------------------------------------------------------

#[test]
fn row_03_a_set_pixels_materializes_and_undo_empties_it() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    let before = d.clone();
    let state = d.state_id();

    assert_eq!(d.apply(sets(cb, 0, 0, C1)), Ok(true));
    assert!(d.image(cb).is_some());
    for y in 0..3 {
        for x in 0..4 {
            let expected_2 = if (x, y) == (0, 0) { C1 } else { T };
            assert_eq!(pixel_rgba(&d, cb, x, y), expected_2, "({x}, {y})");
        }
    }
    assert_eq!(d.materialized_bytes(), 96);
    let after_2 = d.clone();

    d.undo().expect("deshacer");
    assert_eq!(d, before);
    assert!(d.image(cb).is_none(), "la celda vuelve a estar vacía");
    assert_eq!(d.materialized_bytes(), 48);
    assert_eq!(d.state_id(), state);

    d.redo().expect("rehacer");
    assert_eq!(d, after_2);
    assert_eq!(pixel_rgba(&d, cb, 0, 0), C1);
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn row_03_b_fill_spans_materializes_and_undo_empties_it() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    let before = d.clone();

    assert_eq!(d.apply(row_0(cb, 4, C1)), Ok(true));
    for x in 0..4 {
        assert_eq!(pixel_rgba(&d, cb, x, 0), C1, "fila 0, x {x}");
        assert_eq!(pixel_rgba(&d, cb, x, 1), T, "fila 1, x {x}");
        assert_eq!(pixel_rgba(&d, cb, x, 2), T, "fila 2, x {x}");
    }
    assert_eq!(d.materialized_bytes(), 96);
    let after_2 = d.clone();

    d.undo().expect("deshacer");
    assert_eq!(d, before);
    assert!(d.image(cb).is_none());
    assert_eq!(d.materialized_bytes(), 48);

    d.redo().expect("rehacer");
    assert_eq!(d, after_2);
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn row_03_c_a_batch_materializes_several_cels_in_one_step() {
    let (mut d, [a, b, c]) = three_layers();
    let (ca, cb, cc) = (cel(&d, a), cel(&d, b), cel(&d, c));
    let before = d.clone();
    let state = d.state_id();

    let batch = Command::Batch(vec![
        sets(ca, 1, 1, C1),
        row_0(cb, 4, C2),
        sets(cc, 2, 2, C2),
    ]);
    assert_eq!(d.apply(batch), Ok(true));
    assert!(d.image(cb).is_some());
    assert!(d.image(cc).is_some());
    // Una celda ya con imagen más dos materializadas.
    assert_eq!(d.materialized_bytes(), 48 * 3);
    assert_eq!(pixel_rgba(&d, ca, 1, 1), C1);
    assert_eq!(pixel_rgba(&d, cb, 3, 0), C2);
    assert_eq!(pixel_rgba(&d, cc, 2, 2), C2);
    let after_2 = d.clone();

    // Un solo `undo` deshace el lote entero y deja las dos celdas vacías.
    d.undo().expect("deshacer");
    assert_eq!(d, before);
    assert!(d.image(cb).is_none());
    assert!(d.image(cc).is_none());
    assert_eq!(d.materialized_bytes(), 48);
    assert_eq!(d.state_id(), state);

    d.redo().expect("rehacer");
    assert_eq!(d, after_2);
}

#[test]
fn row_03_d_writing_transparent_into_an_empty_cel_does_not_materialize_it() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);

    without_effect(&mut d, sets(cb, 0, 0, T), Ok(false));
    assert!(d.image(cb).is_none());
    assert_eq!(d.materialized_bytes(), 48);

    without_effect(&mut d, row_0(cb, 4, T), Ok(false));
    assert!(d.image(cb).is_none());
    assert_eq!(d.materialized_bytes(), 48);
}

#[test]
fn row_03_e_writing_outside_the_canvas_leaves_no_half_cel() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    let snapshot = Snapshot::of(&d);

    let result = d.apply(sets(cb, 9, 9, C1));
    assert!(
        matches!(result, Err(DocumentError::Pixel { .. })),
        "{result:?}"
    );
    snapshot.assert_equal(&d, "píxel fuera");
    assert!(d.image(cb).is_none(), "la celda sigue vacía");
}

// ---------------------------------------------------------------------------------------
// Fila 4: RemoveLayer.
// ---------------------------------------------------------------------------------------

#[test]
fn row_04_a_removing_a_layer_with_image_and_undoing_returns_the_same() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let ca = ImageId::from_raw(0);
    assert_eq!(d.apply(sets(ca, 1, 1, C1)), Ok(true));
    adds(&mut d, "B", 1);

    let before = d.clone();
    let layers_2 = d.layers().to_vec();
    let image_2 = d.image(ca).expect("imagen de A").clone();
    let state = d.state_id();
    let materialized = d.materialized_bytes();
    let history = d.history_bytes();

    assert_eq!(d.apply(removes(a)), Ok(true));
    assert_eq!(d.layers().len(), 1);
    assert!(d.layer(a).is_none());
    assert!(d.image(ca).is_none());
    assert_eq!(d.materialized_bytes(), 0);
    // La celda guardada (48 bytes de píxeles) cuenta en la historia.
    assert!(d.history_bytes() >= history + 48, "{}", d.history_bytes());
    let after_2 = d.clone();

    d.undo().expect("deshacer");
    assert_eq!(d, before);
    assert_eq!(d.layers(), layers_2.as_slice());
    assert_eq!(d.layers()[0].id(), a);
    assert_eq!(d.layers()[0].cel(), ca);
    assert_eq!(d.image(ca), Some(&image_2));
    assert_eq!(d.materialized_bytes(), materialized);
    assert_eq!(d.state_id(), state);

    d.redo().expect("rehacer");
    assert_eq!(d, after_2);
}

#[test]
fn row_04_b_removing_the_only_layer_or_an_unknown_one_is_an_error() {
    // La única capa: `LastLayer`; el fondo no es especial pero tampoco se queda sin capas.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    without_effect(&mut d, removes(a), Err(DocumentError::LastLayer));

    // Un `LayerId` que no existe, con dos capas.
    // SPEC?: con un `LayerId` desconocido en un documento de una sola capa, la spec no dice si
    // gana `UnknownLayer` o `LastLayer`; aquí solo se prueba con dos capas.
    adds(&mut d, "B", 1);
    let false_value = LayerId::from_raw(999);
    without_effect(
        &mut d,
        removes(false_value),
        Err(DocumentError::UnknownLayer(false_value)),
    );
}

#[test]
fn row_04_c_ids_are_not_reused_after_removing() {
    fn maximums(d: &Document, max: &mut (u32, u32)) {
        for layer_2 in d.layers() {
            max.0 = max.0.max(layer_2.id().raw());
            max.1 = max.1.max(layer_2.cel().raw());
        }
    }

    let mut d = doc_d();
    let mut max = (0, 0);
    maximums(&d, &mut max);
    adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    maximums(&d, &mut max);

    // Quitar la de arriba (la de ids mayores) y añadir otra: ids mayores que cualquiera visto.
    assert_eq!(d.apply(removes(c)), Ok(true));
    let new = adds(&mut d, "D", 2);
    let layer_2 = d.layer(new).expect("la capa nueva existe");
    assert!(layer_2.id().raw() > max.0, "LayerId reutilizado");
    assert!(layer_2.cel().raw() > max.1, "ImageId reutilizado");
    maximums(&d, &mut max);

    // Deshacer el añadir y el quitar, y volver a añadir: sigue sin reutilizar nada.
    d.undo().expect("deshacer añadir");
    d.undo().expect("deshacer quitar");
    assert!(d.layer(c).is_some(), "C vuelve con su id");
    let other = adds(&mut d, "E", 3);
    let layer_2 = d.layer(other).expect("la capa nueva existe");
    assert!(layer_2.id().raw() > max.0, "LayerId reutilizado tras undo");
    assert!(layer_2.cel().raw() > max.1, "ImageId reutilizado tras undo");
}

// ---------------------------------------------------------------------------------------
// Fila 5: MoveLayer.
// ---------------------------------------------------------------------------------------

#[test]
fn row_05_a_move_undo_and_redo() {
    let (mut d, [a, b, c]) = three_layers();
    let cels: Vec<ImageId> = [a, b, c].iter().map(|&x| cel(&d, x)).collect();
    let abc = d.layers().to_vec();

    assert_eq!(d.apply(moves(a, 2)), Ok(true));
    let bca = d.layers().to_vec();
    let order: Vec<LayerId> = d.layers().iter().map(Layer::id).collect();
    assert_eq!(order, vec![b, c, a]);
    // Las celdas no cambian de id al mover.
    assert_eq!(cel(&d, a), cels[0]);
    assert_eq!(cel(&d, b), cels[1]);
    assert_eq!(cel(&d, c), cels[2]);

    // La segunda vez no hay cambio y no hay paso.
    without_effect(&mut d, moves(a, 2), Ok(false));

    d.undo().expect("deshacer");
    assert_eq!(d.layers(), abc.as_slice());
    d.redo().expect("rehacer");
    assert_eq!(d.layers(), bca.as_slice());
}

#[test]
fn row_05_b_move_out_of_range_or_unknown_layer_is_an_error() {
    let (mut d, [a, ..]) = three_layers();
    // `to` es la posición final: con 3 capas, la mayor válida es 2.
    without_effect(
        &mut d,
        moves(a, 3),
        Err(DocumentError::LayerIndex { index: 3, max: 2 }),
    );
    let false_value = LayerId::from_raw(999);
    without_effect(
        &mut d,
        moves(false_value, 0),
        Err(DocumentError::UnknownLayer(false_value)),
    );
}

// ---------------------------------------------------------------------------------------
// Fila 6: RenameLayer, SetLayerOpacity y SetLayerVisible.
// ---------------------------------------------------------------------------------------

#[test]
fn row_06_a_each_property_is_one_step_and_repeating_does_nothing() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let initial = d.layers().to_vec();
    let mut layers_2 = vec![initial.clone()];
    let mut states = vec![d.state_id()];

    assert_eq!(d.apply(renames(a, "Otra")), Ok(true));
    assert_eq!(d.layers()[0].name(), "Otra");
    layers_2.push(d.layers().to_vec());
    states.push(d.state_id());

    assert_eq!(d.apply(opacity_2(a, 128)), Ok(true));
    assert_eq!(d.layers()[0].opacity(), 128);
    layers_2.push(d.layers().to_vec());
    states.push(d.state_id());

    assert_eq!(d.apply(visible(a, false)), Ok(true));
    assert!(!d.layers()[0].visible());
    layers_2.push(d.layers().to_vec());
    states.push(d.state_id());

    // Cada uno es un paso (cada `state_id` es nuevo)...
    let different: HashSet<u64> = states.iter().copied().collect();
    assert_eq!(different.len(), 4);

    // ...y repetir el mismo valor no es un paso.
    without_effect(&mut d, renames(a, "Otra"), Ok(false));
    without_effect(&mut d, opacity_2(a, 128), Ok(false));
    without_effect(&mut d, visible(a, false), Ok(false));

    // Tres `undo` y ninguno más; `redo` los repone.
    for (i, expected_2_2) in layers_2.iter().enumerate().take(3).rev() {
        d.undo().expect("deshacer");
        assert_eq!(
            d.layers(),
            expected_2_2.as_slice(),
            "tras deshacer hasta {i}"
        );
        assert_eq!(d.state_id(), states[i]);
    }
    assert_eq!(d.undo(), Err(DocumentError::NothingToUndo));
    for (i, expected_2_2) in layers_2.iter().enumerate().skip(1) {
        d.redo().expect("rehacer");
        assert_eq!(
            d.layers(),
            expected_2_2.as_slice(),
            "tras rehacer hasta {i}"
        );
    }
    assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));
}

#[test]
fn row_06_b_the_name_measures_utf8_bytes() {
    let mut d = doc_d();
    let a = d.layers()[0].id();

    // 255 bytes valen.
    let just = "a".repeat(255);
    assert_eq!(d.apply(renames(a, &just)), Ok(true));
    assert_eq!(d.layers()[0].name(), just);
    // 255 bytes con caracteres de dos bytes (127 `é` y una `a`).
    let mixed = format!("{}a", "é".repeat(127));
    assert_eq!(mixed.len(), 255);
    assert_eq!(d.apply(renames(a, &mixed)), Ok(true));
    assert_eq!(d.layers()[0].name(), mixed);

    // 256 bytes no, ni de `a` ni de 128 `é` (128 caracteres, 256 bytes).
    for length in ["a".repeat(256), "é".repeat(128)] {
        without_effect(
            &mut d,
            renames(a, &length),
            Err(DocumentError::LayerName {
                len: 256,
                limit: 255,
            }),
        );
    }
    assert_eq!(d.layers()[0].name(), mixed);
}

#[test]
fn row_06_c_unknown_layer_in_the_three_commands() {
    let mut d = doc_d();
    let false_value = LayerId::from_raw(999);
    let error = Err(DocumentError::UnknownLayer(false_value));
    without_effect(&mut d, renames(false_value, "X"), error.clone());
    without_effect(&mut d, opacity_2(false_value, 128), error.clone());
    without_effect(&mut d, visible(false_value, false), error);
}

#[test]
fn row_06_d_in_indexed_opacity_is_always_255() {
    let mut d = Document::new(img_indexed(4, 3, &[RED, GREEN, BLUE, BLACK], Some(0)));
    let a = d.layers()[0].id();

    without_effect(
        &mut d,
        opacity_2(a, 128),
        Err(DocumentError::IndexedOpacity),
    );
    without_effect(&mut d, opacity_2(a, 255), Ok(false));
    assert_eq!(d.layers()[0].opacity(), 255);

    // La visibilidad sí vale.
    assert_eq!(d.apply(visible(a, false)), Ok(true));
    assert!(!d.layers()[0].visible());
    d.undo().expect("deshacer");
    assert!(d.layers()[0].visible());
}

// ---------------------------------------------------------------------------------------
// Fila 7: AddLayer y sus errores.
// ---------------------------------------------------------------------------------------

#[test]
fn row_07_a_without_transparent_value_one_cannot_be_added() {
    let mut gray = Document::new(Image::new(4, 3, Mode::Gray).expect("imagen gris"));
    without_effect(
        &mut gray,
        adds_cmd("B", 1),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );

    let mut indexed_2 = Document::new(img_indexed(4, 3, &[RED, GREEN], None));
    without_effect(
        &mut indexed_2,
        adds_cmd("B", 1),
        Err(DocumentError::NoTransparency {
            mode: Mode::Indexed,
        }),
    );
}

#[test]
fn row_07_b_with_transparent_value_one_can_be_added() {
    let mut gray_alpha = Document::new(Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"));
    let b = adds(&mut gray_alpha, "B", 1);
    assert!(gray_alpha.image(cel(&gray_alpha, b)).is_none());

    let mut indexed_2 = Document::new(img_indexed(4, 3, &[RED, GREEN], Some(1)));
    let b = adds(&mut indexed_2, "B", 1);
    assert!(indexed_2.image(cel(&indexed_2, b)).is_none());
    assert_eq!(indexed_2.layers()[1].opacity(), 255);
}

/// Documento de 1 x 1 con 256 capas (RGBA o indexado con índice transparente).
fn doc_256(indexed_2: bool) -> Document {
    let img = if indexed_2 {
        img_indexed(1, 1, &[RED, GREEN], Some(1))
    } else {
        img_rgba(1, 1)
    };
    let mut d = Document::new(img);
    for i in 1..Document::MAX_LAYERS {
        assert_eq!(d.apply(adds_cmd(&format!("c{i}"), i)), Ok(true), "capa {i}");
    }
    assert_eq!(d.layers().len(), 256);
    d
}

#[test]
fn row_07_c_with_256_layers_no_other_fits() {
    let mut d = doc_256(false);
    assert_eq!(d.worst_case_bytes(), 256 * d.cel_bytes());
    without_effect(
        &mut d,
        adds_cmd("una más", 0),
        Err(DocumentError::TooManyLayers { limit: 256 }),
    );
    assert_eq!(d.layers().len(), 256);
}

#[test]
fn row_07_d_position_greater_than_len_is_an_error() {
    let mut d = doc_d();
    // Con una capa, la mayor posición válida es 1.
    without_effect(
        &mut d,
        adds_cmd("B", 2),
        Err(DocumentError::LayerIndex { index: 2, max: 1 }),
    );
    without_effect(
        &mut d,
        adds_cmd("B", usize::MAX),
        Err(DocumentError::LayerIndex {
            index: usize::MAX,
            max: 1,
        }),
    );
    assert_eq!(d.apply(adds_cmd("B", 1)), Ok(true));
}

#[test]
fn row_07_e_a_256_byte_name_is_an_error() {
    let mut d = doc_d();
    without_effect(
        &mut d,
        adds_cmd(&"a".repeat(256), 1),
        Err(DocumentError::LayerName {
            len: 256,
            limit: 255,
        }),
    );
    // 255 bytes valen.
    assert_eq!(d.apply(adds_cmd(&"a".repeat(255), 1)), Ok(true));
}

#[test]
fn row_07_f_check_order_name_transparency_count_position() {
    let length = "a".repeat(256);
    let error_name = DocumentError::LayerName {
        len: 256,
        limit: 255,
    };

    // Nombre antes que transparencia (y que posición).
    let mut gray = Document::new(Image::new(4, 3, Mode::Gray).expect("imagen gris"));
    without_effect(&mut gray, adds_cmd(&length, 99), Err(error_name.clone()));
    // Transparencia antes que posición.
    without_effect(
        &mut gray,
        adds_cmd("B", 99),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );

    // Nombre antes que número de capas; número de capas antes que posición.
    let mut full = doc_256(false);
    without_effect(&mut full, adds_cmd(&length, 999), Err(error_name));
    without_effect(
        &mut full,
        adds_cmd("B", 999),
        Err(DocumentError::TooManyLayers { limit: 256 }),
    );

    // Transparencia antes que número de capas: 256 capas en indexado y luego sin índice
    // transparente.
    let mut indexed_2 = doc_256(true);
    let cel = indexed_2.layers()[0].cel();
    assert_eq!(indexed_2.apply(without_transparent(cel)), Ok(true));
    without_effect(
        &mut indexed_2,
        adds_cmd("B", 999),
        Err(DocumentError::NoTransparency {
            mode: Mode::Indexed,
        }),
    );
}

// ---------------------------------------------------------------------------------------
// Fila 8: los comandos de paleta editan todo el documento en un solo paso.
// ---------------------------------------------------------------------------------------

/// Indexado de 4 x 3, paleta `[RED, GREEN, BLUE, BLACK]` con el índice 0 transparente y tres
/// capas: `A` (píxeles de la entrada 2 en (0,0) y (1,1), y de la 1 en (3,2)), `B` (entrada 2 en
/// (0,0), 1 en (2,1) y 3 en (3,0)) y `C` vacía. Devuelve las tres celdas.
fn doc_palette() -> (Document, [ImageId; 3]) {
    let mut img = img_indexed(4, 3, &[RED, GREEN, BLUE, BLACK], Some(0));
    for (x, y, i) in [(0, 0, 2), (1, 1, 2), (3, 2, 1)] {
        img.set_pixel(x, y, Pixel::Indexed(i)).expect("píxel");
    }
    let mut d = Document::new(img);
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    let cb = cel(&d, b);
    let command = Command::SetPixels {
        image: cb,
        pixels: [(0, 0, 2), (2, 1, 1), (3, 0, 3)]
            .iter()
            .map(|&(x, y, i)| PixelEdit {
                x,
                y,
                pixel: Pixel::Indexed(i),
            })
            .collect(),
    };
    assert_eq!(d.apply(command), Ok(true));
    let cels = [d.layers()[0].cel(), cb, cel(&d, c)];
    (d, cels)
}

/// Toda celda con imagen tiene la paleta del lienzo (`Some`).
fn palettes_equal(d: &Document, context: &str) {
    let canvas = d.palette().expect("el lienzo indexado tiene paleta");
    for layer_2 in d.layers() {
        if let Some(image_2) = d.image(layer_2.cel()) {
            assert_eq!(
                image_2.palette(),
                Some(canvas),
                "{context}: {}",
                layer_2.name()
            );
        }
    }
}

/// Aplica el comando de paleta (con `image` = la celda `A`) y comprueba la fila 8.
fn checks_palette(name_2: &str, command: impl Fn(ImageId) -> Command, remaps: fn(u8) -> u8) {
    let (mut d, [ca, cb, cc]) = doc_palette();
    let before = d.clone();
    let state = d.state_id();
    let indices_a = indices(&d, ca);
    let indices_b = indices(&d, cb);

    assert_eq!(d.apply(command(ca)), Ok(true), "{name_2}");
    let after_2 = d.clone();
    palettes_equal(&d, name_2);
    // Los píxeles de las dos celdas se remapean igual.
    let expected_a: Vec<u8> = indices_a.iter().map(|&i| remaps(i)).collect();
    let expected_b: Vec<u8> = indices_b.iter().map(|&i| remaps(i)).collect();
    assert_eq!(indices(&d, ca), expected_a, "{name_2}: celda A");
    assert_eq!(indices(&d, cb), expected_b, "{name_2}: celda B");
    assert!(d.image(cc).is_none(), "{name_2}: C sigue vacía");

    // Un solo paso: un `undo` devuelve exactamente el documento y el `state_id` de antes.
    d.undo().expect("deshacer");
    assert_eq!(d, before, "{name_2}: undo");
    assert_eq!(d.state_id(), state, "{name_2}: undo");
    assert!(d.image(cc).is_none(), "{name_2}: C vacía tras undo");
    d.redo().expect("rehacer");
    assert_eq!(d, after_2, "{name_2}: redo");
    assert!(d.image(cc).is_none(), "{name_2}: C vacía tras redo");

    // Al materializarla, la celda vacía toma la paleta que hay entonces.
    assert_eq!(
        d.apply(set_px(cc, 0, 0, Pixel::Indexed(2))),
        Ok(true),
        "{name_2}: materializar C"
    );
    palettes_equal(&d, &format!("{name_2} con C materializada"));
    assert_eq!(
        d.image(cc).expect("C con imagen").palette(),
        d.palette(),
        "{name_2}: paleta de C"
    );
}

#[test]
fn row_08_a_each_palette_command_edits_all_cels_with_an_image() {
    checks_palette(
        "SetPaletteColor",
        |image| Command::SetPaletteColor {
            image,
            index: 2,
            color: WHITE,
        },
        |i| i,
    );
    checks_palette(
        "SetTransparentIndex",
        |image| Command::SetTransparentIndex {
            image,
            index: Some(1),
        },
        |i| i,
    );
    checks_palette(
        "AddPaletteEntry",
        |image| Command::AddPaletteEntry {
            image,
            color: YELLOW,
        },
        |i| i,
    );
    // La entrada 2 pasa a la posición 0 y las de en medio se desplazan.
    checks_palette(
        "MovePaletteEntry",
        |image| Command::MovePaletteEntry {
            image,
            from: 2,
            to: 0,
        },
        |i| match i {
            2 => 0,
            0 => 1,
            1 => 2,
            other_2 => other_2,
        },
    );
    // `order[new_2] = old` con `[3, 2, 1, 0]`: el índice antiguo `i` pasa a `3 - i`.
    checks_palette(
        "PermutePalette",
        |image| Command::PermutePalette {
            image,
            order: vec![3, 2, 1, 0],
        },
        |i| 3 - i,
    );
    // Los píxeles de la entrada 2 pasan a la 0 y los índices mayores bajan uno.
    checks_palette(
        "RemovePaletteEntry",
        |image| Command::RemovePaletteEntry {
            image,
            index: 2,
            into: 0,
        },
        |i| match i {
            2 => 0,
            3 => 2,
            other_2 => other_2,
        },
    );
    // Los píxeles conservan su índice (cambian los colores de las ranuras).
    checks_palette(
        "RotateColors",
        |image| Command::RotateColors {
            image,
            start: 0,
            len: 4,
            steps: 1,
        },
        |i| i,
    );
}

#[test]
fn row_08_b_with_one_layer_the_cost_of_each_palette_step_is_todays() {
    // Salvo `RemovePaletteEntry` (que además guarda una máscara por celda), un paso de
    // paleta de un documento de una capa cuesta como mucho 512 bytes.
    let commands_2 = |image: ImageId| {
        vec![
            Command::SetPaletteColor {
                image,
                index: 2,
                color: WHITE,
            },
            Command::SetTransparentIndex {
                image,
                index: Some(1),
            },
            Command::AddPaletteEntry {
                image,
                color: YELLOW,
            },
            Command::MovePaletteEntry {
                image,
                from: 2,
                to: 0,
            },
            Command::PermutePalette {
                image,
                order: vec![3, 2, 1, 0],
            },
            Command::RotateColors {
                image,
                start: 0,
                len: 4,
                steps: 1,
            },
        ]
    };
    let image_2 = img_indexed(4, 3, &[RED, GREEN, BLUE, BLACK], Some(0));
    for command in commands_2(ImageId::from_raw(0)) {
        let mut d = Document::new(image_2.clone());
        assert_eq!(d.history_bytes(), 0);
        let context = format!("{command:?}");
        assert_eq!(d.apply(command), Ok(true), "{context}");
        assert!(
            d.history_bytes() <= 512,
            "{context}: {} bytes",
            d.history_bytes()
        );
    }
}

#[test]
fn row_08_c_the_image_field_is_only_validated() {
    let (mut d, [_, _, cc]) = doc_palette();
    // Una celda vacía vale como `image`: se edita igualmente el documento entero.
    let command = Command::SetPaletteColor {
        image: cc,
        index: 2,
        color: WHITE,
    };
    assert_eq!(d.apply(command), Ok(true));
    assert_eq!(d.palette().expect("paleta").colors()[2], WHITE);
    palettes_equal(&d, "SetPaletteColor con image vacía");
    assert!(d.image(cc).is_none());

    // Un id que no es de ninguna celda es `UnknownImage`.
    let false_value = ImageId::from_raw(999);
    without_effect(
        &mut d,
        Command::SetPaletteColor {
            image: false_value,
            index: 0,
            color: YELLOW,
        },
        Err(DocumentError::UnknownImage(false_value)),
    );
}

// ---------------------------------------------------------------------------------------
// Fila 9: ConvertMode.
// ---------------------------------------------------------------------------------------

fn convert(image: ImageId, target: Mode) -> Command {
    Command::ConvertMode {
        image,
        target,
        allowed: Allowed::NOTHING,
    }
}

/// Documento RGBA de 4 x 3 con dos colores opacos y el resto transparente.
fn doc_with_colors() -> Document {
    let mut img = img_rgba(4, 3);
    img.set_pixel(0, 0, Pixel::Rgba(C1)).expect("píxel");
    img.set_pixel(1, 0, Pixel::Rgba(C2)).expect("píxel");
    Document::new(img)
}

#[test]
fn row_09_a_with_one_layer_the_canvas_follows_the_conversion() {
    let mut d = doc_with_colors();
    let cel = d.layers()[0].cel();
    let rgba = d.clone();

    // RGBA a indexado: el lienzo sigue a la imagen en `apply`, `undo` y `redo`.
    assert_eq!(d.apply(convert(cel, Mode::Indexed)), Ok(true));
    let indexed_2 = d.clone();
    assert_eq!(d.mode(), Mode::Indexed);
    assert!(d.palette().is_some());
    assert_eq!(d.palette(), d.image(cel).expect("imagen").palette());
    assert_eq!((d.width(), d.height()), (4, 3));
    assert_eq!(d.image(cel).expect("imagen").mode(), Mode::Indexed);

    d.undo().expect("deshacer");
    assert_eq!(d, rgba);
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);

    d.redo().expect("rehacer");
    assert_eq!(d, indexed_2);
    assert_eq!(d.mode(), Mode::Indexed);
    assert_eq!(d.palette(), d.image(cel).expect("imagen").palette());

    // Y de vuelta a RGBA.
    assert_eq!(d.apply(convert(cel, Mode::Rgba)), Ok(true));
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    d.undo().expect("deshacer");
    assert_eq!(d.mode(), Mode::Indexed);
    assert_eq!(d.palette(), d.image(cel).expect("imagen").palette());
    assert_eq!(d, indexed_2);
}

#[test]
fn row_09_b_with_two_layers_it_is_not_converted() {
    let mut d = doc_with_colors();
    adds(&mut d, "B", 1);
    let cel = d.layers()[0].cel();
    without_effect(
        &mut d,
        convert(cel, Mode::Indexed),
        Err(DocumentError::MultiLayerConvert { layers: 2 }),
    );
    assert_eq!(d.mode(), Mode::Rgba);
}

#[test]
fn row_09_c_to_indexed_with_opacity_other_than_255_is_not_converted() {
    let mut d = doc_with_colors();
    let a = d.layers()[0].id();
    let cel = d.layers()[0].cel();
    assert_eq!(d.apply(opacity_2(a, 128)), Ok(true));
    without_effect(
        &mut d,
        convert(cel, Mode::Indexed),
        Err(DocumentError::IndexedOpacity),
    );
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
}

// ---------------------------------------------------------------------------------------
// Fila 10: Batch con celdas vacías y con órdenes de capa.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_a_a_batch_that_materializes_is_one_step() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let (ca, cb) = (ImageId::from_raw(0), cel(&d, b));
    let before = d.clone();
    let state = d.state_id();

    let batch = Command::Batch(vec![sets(ca, 0, 0, C1), row_0(cb, 4, C2)]);
    assert_eq!(d.apply(batch), Ok(true));
    assert!(d.image(cb).is_some());
    assert_eq!(pixel_rgba(&d, ca, 0, 0), C1);
    assert_eq!(pixel_rgba(&d, cb, 2, 0), C2);

    d.undo().expect("deshacer");
    assert_eq!(d, before);
    assert!(d.image(cb).is_none(), "B vuelve a estar vacía");
    assert_eq!(d.state_id(), state);
}

#[test]
fn row_10_b_layer_commands_do_not_go_in_a_batch() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let ca = ImageId::from_raw(0);
    adds(&mut d, "B", 1);

    for command in six_commands(a) {
        let context = format!("{command:?}");
        without_effect(
            &mut d,
            Command::Batch(vec![command.clone()]),
            Err(in_batch(0, DocumentError::NotBatchable)),
        );
        // También detrás de una escritura válida, que se revierte.
        without_effect(
            &mut d,
            Command::Batch(vec![sets(ca, 0, 0, C1), command]),
            Err(in_batch(1, DocumentError::NotBatchable)),
        );
        assert_eq!(pixel_rgba(&d, ca, 0, 0), T, "{context}");
    }
}

#[test]
fn row_10_c_a_failing_batch_leaves_no_materialized_cels() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let (ca, cb) = (ImageId::from_raw(0), cel(&d, b));

    let batch = Command::Batch(vec![row_0(cb, 4, C1), sets(ca, 9, 9, C2)]);
    let snapshot = Snapshot::of(&d);
    let result = d.apply(batch);
    assert!(
        matches!(
            &result,
            Err(DocumentError::InBatch { index: 1, source })
                if matches!(**source, DocumentError::Pixel { .. })
        ),
        "{result:?}"
    );
    snapshot.assert_equal(&d, "lote fallido");
    assert!(d.image(cb).is_none(), "B no queda materializada");
    assert_eq!(d.materialized_bytes(), 48);
}

#[test]
fn row_10_d_an_unknown_id_in_a_batch_is_unknown_image_in_its_turn() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    let false_value = ImageId::from_raw(999);

    let batch = Command::Batch(vec![row_0(cb, 4, C1), sets(false_value, 0, 0, C2)]);
    without_effect(
        &mut d,
        batch,
        Err(in_batch(1, DocumentError::UnknownImage(false_value))),
    );
    assert!(d.image(cb).is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 11: límite de celdas materializadas, `worst_case_bytes` y constantes.
// ---------------------------------------------------------------------------------------

#[test]
fn row_11_a_materializing_above_the_limit_is_celstoolarge() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    d.set_cells_limit(d.cel_bytes());
    assert_eq!(d.cells_limit(), 48);

    without_effect(
        &mut d,
        sets(cb, 0, 0, C1),
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        }),
    );
    assert!(d.image(cb).is_none(), "ninguna celda a medias");
    assert_eq!(d.materialized_bytes(), 48);
    // También con un relleno.
    without_effect(
        &mut d,
        row_0(cb, 4, C1),
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        }),
    );
}

#[test]
fn row_11_b_redo_over_a_lower_limit_fails_and_can_be_retried() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    assert_eq!(d.apply(sets(cb, 0, 0, C1)), Ok(true));
    let with_cel = d.clone();
    d.undo().expect("deshacer");

    d.set_cells_limit(48);
    let snapshot = Snapshot::of(&d);
    let result = d.redo();
    assert!(
        matches!(result, Err(DocumentError::CelsTooLarge { .. })),
        "{result:?}"
    );
    // Nada cambió y el paso sigue en la pila de rehacer.
    snapshot.assert_equal(&d, "redo fallido");
    assert!(d.can_redo());
    assert!(d.image(cb).is_none());

    d.set_cells_limit(96);
    d.redo().expect("rehacer con el límite subido");
    assert_eq!(d, with_cel);
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn row_11_c_lowering_the_limit_discards_nothing() {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let cb = cel(&d, b);
    assert_eq!(d.apply(sets(cb, 0, 0, C1)), Ok(true));
    let before = d.clone();

    d.set_cells_limit(10);
    assert_eq!(d.cells_limit(), 10);
    assert_eq!(d, before);
    assert_eq!(d.materialized_bytes(), 96);
    // Escribir en una celda que ya tiene imagen no materializa nada (C-09: el límite «solo
    // impide materializar más»).
    assert_eq!(d.apply(sets(cb, 1, 0, C2)), Ok(true));
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn row_11_d_worst_case_and_constants() {
    let mut d = doc_d();
    assert_eq!(d.worst_case_bytes(), d.layers().len() * d.cel_bytes());
    for i in 1..4 {
        adds(&mut d, &format!("c{i}"), i);
        assert_eq!(d.layers().len(), i + 1);
        assert_eq!(d.worst_case_bytes(), (i + 1) * 48);
        // Cuenta todas las celdas aunque estén vacías.
        assert_eq!(d.materialized_bytes(), 48);
    }
    assert_eq!(Document::MAX_LAYERS, 256);
    assert_eq!(Document::MAX_LAYER_NAME_BYTES, 255);
    assert_eq!(Document::DEFAULT_CELLS_LIMIT, 2usize << 30);
    assert_eq!(Document::MEMORY_WARN_BYTES, 1usize << 30);
}

#[test]
fn row_11_e_a_batch_checks_all_empty_cels_before_running() {
    let (mut d, [a, b, c]) = three_layers();
    let (ca, cb, cc) = (cel(&d, a), cel(&d, b), cel(&d, c));
    // Cabe una celda más (48 + 48 = 96), pero el lote nombra dos vacías.
    d.set_cells_limit(96);
    let snapshot = Snapshot::of(&d);

    let batch = Command::Batch(vec![
        sets(ca, 0, 0, C1),
        row_0(cb, 4, C1),
        sets(cc, 0, 0, C2),
    ]);
    let result = d.apply(batch);
    // SPEC?: la spec no dice si este error sale envuelto en `InBatch` ni cuál es `needed` con
    // varias celdas (el texto dice `materialized_bytes() + cel_bytes() × cels vacías
    // nombradas`; C-28, `materialized_bytes() + cel_bytes()`). Se removes el envoltorio y solo se
    // comprueba `limit`.
    let error = match result {
        Err(DocumentError::InBatch { source, .. }) => *source,
        other_2 => other_2.expect_err("el lote debe fallar"),
    };
    assert!(
        matches!(error, DocumentError::CelsTooLarge { limit: 96, .. }),
        "{error:?}"
    );
    snapshot.assert_equal(&d, "lote que no cabe");
    // Ni la escritura sobre la celda con imagen, que va antes en el lote, se aplicó.
    assert_eq!(pixel_rgba(&d, ca, 0, 0), T);
    assert!(d.image(cb).is_none());
    assert!(d.image(cc).is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 12: `state_id` e igualdad con los comandos de capa.
// ---------------------------------------------------------------------------------------

/// Aplica `cmd` (`Ok(true)`) y comprueba que el `state_id` es nuevo.
fn applies_new(d: &mut Document, cmd: Command, seen: &mut Vec<u64>) {
    let context = format!("{cmd:?}");
    assert_eq!(d.apply(cmd), Ok(true), "{context}");
    let id = d.state_id();
    assert!(!seen.contains(&id), "{context}: state_id {id} repetido");
    seen.push(id);
}

#[test]
fn row_12_a_each_layer_command_gives_a_new_state_id_and_undo_redo_walk_them() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let mut seen = vec![d.state_id()];
    assert_eq!(seen[0], 0);

    applies_new(&mut d, adds_cmd("B", 1), &mut seen);
    let b = d.layers()[1].id();
    applies_new(&mut d, renames(b, "Beta"), &mut seen);
    applies_new(&mut d, opacity_2(b, 128), &mut seen);
    applies_new(&mut d, visible(b, false), &mut seen);
    applies_new(&mut d, moves(b, 0), &mut seen);
    applies_new(&mut d, removes(a), &mut seen);
    assert_eq!(seen.len(), 7);

    for (i, state) in seen.iter().enumerate().take(6).rev() {
        d.undo().expect("deshacer");
        assert_eq!(d.state_id(), *state, "undo hasta {i}");
    }
    for (i, state) in seen.iter().enumerate().skip(1) {
        d.redo().expect("rehacer");
        assert_eq!(d.state_id(), *state, "redo hasta {i}");
    }
}

#[test]
fn row_12_b_no_changes_and_errors_do_not_touch_the_state_id() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let false_value = LayerId::from_raw(999);

    // `Ok(false)`.
    without_effect(&mut d, renames(b, "B"), Ok(false));
    without_effect(&mut d, opacity_2(b, 255), Ok(false));
    without_effect(&mut d, visible(b, true), Ok(false));
    without_effect(&mut d, moves(b, 1), Ok(false));

    // Errores.
    without_effect(
        &mut d,
        adds_cmd("X", 99),
        Err(DocumentError::LayerIndex { index: 99, max: 2 }),
    );
    without_effect(
        &mut d,
        removes(false_value),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    without_effect(
        &mut d,
        moves(a, 99),
        Err(DocumentError::LayerIndex { index: 99, max: 1 }),
    );
    without_effect(
        &mut d,
        renames(false_value, "X"),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    without_effect(
        &mut d,
        opacity_2(false_value, 1),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    without_effect(
        &mut d,
        visible(false_value, false),
        Err(DocumentError::UnknownLayer(false_value)),
    );
}

/// `D` con una capa `B` vacía encima. Los ids son siempre los mismos.
fn base() -> (Document, LayerId) {
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    (d, b)
}

#[test]
fn row_12_c_equality_distinguishes_name_order_opacity_visibility_ids_and_empty() {
    let (reference, b) = base();
    assert_eq!(
        base().0,
        reference,
        "dos construcciones iguales son iguales"
    );

    // Nombre, opacidad, visibilidad y orden.
    for (that, cmd) in [
        ("nombre", renames(b, "Z")),
        ("opacidad", opacity_2(b, 128)),
        ("visibilidad", visible(b, false)),
        ("orden", moves(b, 0)),
    ] {
        let (mut d, _) = base();
        assert_eq!(d.apply(cmd), Ok(true), "{that}");
        assert_ne!(d, reference, "{that}");
    }

    // Mismo contenido y nombres, otros ids: se añadió y quitó otra capa antes.
    let mut other_ids = doc_d();
    let x = adds(&mut other_ids, "X", 1);
    assert_eq!(other_ids.apply(removes(x)), Ok(true));
    adds(&mut other_ids, "B", 1);
    assert_eq!(names(&other_ids), names(&reference));
    assert_ne!(other_ids, reference, "ids distintos");

    // Celda vacía frente a celda con una imagen toda transparente.
    let (mut transparent, b2) = base();
    let cb = cel(&transparent, b2);
    assert_eq!(transparent.apply(sets(cb, 0, 0, C1)), Ok(true));
    assert_eq!(transparent.apply(sets(cb, 0, 0, T)), Ok(true));
    assert!(transparent.image(cb).is_some());
    assert_ne!(transparent, reference, "vacía no es transparente");
}

#[test]
fn row_12_d_equality_does_not_look_at_the_history() {
    // Mismo contenido e ids con historias distintas.
    let (mut one, b) = base();
    assert_eq!(one.apply(renames(b, "X")), Ok(true));
    let (mut two, _) = base();
    assert_eq!(two.apply(renames(b, "Y")), Ok(true));
    assert_eq!(two.apply(renames(b, "X")), Ok(true));
    assert_ne!(one.history_bytes(), two.history_bytes());
    assert_eq!(one, two);

    // Un documento al que se le deshizo todo es igual a uno recién construido, aunque tenga
    // pasos por rehacer.
    let (new_2, b) = base();
    let mut undone = new_2.clone();
    let cb = cel(&undone, b);
    assert_eq!(undone.apply(sets(cb, 0, 0, C1)), Ok(true));
    assert_eq!(undone.apply(sets(cb, 0, 0, T)), Ok(true));
    undone.undo().expect("deshacer");
    undone.undo().expect("deshacer");
    assert!(undone.can_redo());
    assert_eq!(undone, new_2);
}

// ---------------------------------------------------------------------------------------
// Generadores de las propiedades.
// ---------------------------------------------------------------------------------------

/// Color de apoyo; `color(0)` es el transparente.
fn color(n: u8) -> Rgba {
    match n % 4 {
        0 => T,
        1 => C1,
        2 => C2,
        _ => Rgba::new(100, 110, 120, 255),
    }
}

fn mode_of(kind: u8) -> Mode {
    match kind {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        2 => Mode::Gray,
        _ => Mode::Indexed,
    }
}

/// Un píxel válido del modo (en indexado, el índice es `seed % modulo`, que cabe en la
/// paleta cuando `modulo` es su longitud).
fn pixel_valid(mode_2: Mode, seed: u8, modulo: usize) -> Pixel {
    match mode_2 {
        Mode::Rgba => Pixel::Rgba(color(seed)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: (seed % 3) * 100,
            alpha: if seed.is_multiple_of(2) { 0 } else { 255 },
        },
        Mode::Gray => Pixel::Gray(seed),
        Mode::Indexed => {
            Pixel::Indexed(u8::try_from(usize::from(seed) % modulo).expect("cabe en u8"))
        }
    }
}

/// Un píxel de una orden generada: casi siempre válido, a veces de otro modo o (en indexado)
/// fuera de la paleta, para inyectar fallos.
fn pixel_op(mode_2: Mode, seed: u8, palette_2: usize) -> Pixel {
    if seed.is_multiple_of(11) {
        return if mode_2 == Mode::Rgba {
            Pixel::Gray(seed)
        } else {
            Pixel::Rgba(color(seed))
        };
    }
    pixel_valid(mode_2, seed, palette_2 + 1)
}

/// Descripción de un documento inicial: de 1 a 4 capas, algunas con imagen y otras vacías.
#[derive(Clone, Debug)]
struct Plan {
    /// 0 RGBA, 1 grises con alfa, 2 grises, 3 indexado con índice transparente, 4 indexado al
    /// que se le quita el índice transparente después de añadir las capas.
    kind: u8,
    w: u32,
    h: u32,
    palette_2: usize,
    transparent: usize,
    layers_2: usize,
    /// Por capa: `None` vacía (la 0 siempre tiene imagen), `Some` píxeles `(x, y, seed)`.
    content: Vec<Option<Vec<(u8, u8, u8)>>>,
}

fn plan_strategy(
    layers_2: std::ops::RangeInclusive<usize>,
    with_grays: bool,
) -> impl Strategy<Value = Plan> {
    (
        0u8..5,
        1u32..=4,
        1u32..=4,
        1usize..=8,
        0usize..8,
        layers_2,
        prop::collection::vec(
            prop::option::of(prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..4)),
            4,
        ),
    )
        .prop_map(
            move |(kind, w, h, palette_2, transparent, layers_2, content)| {
                let kind = if !with_grays && kind == 2 { 3 } else { kind };
                Plan {
                    kind,
                    w,
                    h,
                    palette_2,
                    transparent,
                    // En grises no se puede añadir capas (no hay valor transparente).
                    layers_2: if kind == 2 { 1 } else { layers_2 },
                    content,
                }
            },
        )
}

fn image_base(plan: &Plan) -> Image {
    match plan.kind {
        0..=2 => {
            Image::new(plan.w, plan.h, mode_of(plan.kind)).expect("imagen de un modo sin paleta")
        }
        _ => {
            let t = u8::try_from(plan.transparent % plan.palette_2).expect("cabe en u8");
            img_indexed(plan.w, plan.h, &colors_different(plan.palette_2), Some(t))
        }
    }
}

/// Construye el documento del plan solo con `Document::new`, `AddLayer` y escrituras.
fn builds(plan: &Plan) -> Document {
    let mode_2 = mode_of(plan.kind);
    let mut image_2 = image_base(plan);
    if let Some(Some(pixels_2)) = plan.content.first() {
        for &(x, y, seed) in pixels_2 {
            let pixel = pixel_valid(mode_2, seed, plan.palette_2);
            image_2
                .set_pixel(u32::from(x) % plan.w, u32::from(y) % plan.h, pixel)
                .expect("píxel de la capa 0");
        }
    }
    let mut d = Document::new(image_2);
    for i in 1..plan.layers_2 {
        assert_eq!(d.apply(adds_cmd(&format!("Capa {}", i + 1), i)), Ok(true));
        if let Some(Some(pixels_2)) = plan.content.get(i)
            && !pixels_2.is_empty()
        {
            let image = d.layers()[i].cel();
            let pixels = pixels_2
                .iter()
                .map(|&(x, y, seed)| PixelEdit {
                    x: u32::from(x) % plan.w,
                    y: u32::from(y) % plan.h,
                    pixel: pixel_valid(mode_2, seed, plan.palette_2),
                })
                .collect();
            d.apply(Command::SetPixels { image, pixels })
                .expect("escribir en la capa");
        }
    }
    if plan.kind == 4 {
        let image = d.layers()[0].cel();
        assert_eq!(d.apply(without_transparent(image)), Ok(true));
    }
    d
}

/// A qué se refiere una orden generada: la capa (o su celda) número `n % len`, o un id que no
/// existe.
#[derive(Clone, Debug)]
enum Obj {
    Nth(usize),
    False(u32),
}

#[derive(Clone, Debug)]
enum PalOp {
    Color(u8, u8),
    Move(u8, u8),
    Permute(u8),
    Add(u8),
    Remove(u8, u8),
    Transp(Option<u8>),
    Rotate(u8, u16, i32),
}

/// Una orden generada. Se traduce a `Command` al ejecutarla, con el documento vivo delante,
/// para que los ids sean los del momento.
#[derive(Clone, Debug)]
enum Op {
    Set(Obj, Vec<(u8, u8, u8)>),
    /// Celda, tramos `(y, x, length)`, semilla del píxel y si se deja sin ordenar.
    Fill(Obj, Vec<(u8, u8, u8)>, u8, bool),
    Batch(Vec<Op>),
    Add(String, usize),
    Remove(Obj),
    Move(Obj, usize),
    Rename(Obj, String),
    Opacity(Obj, u8),
    Visible(Obj, bool),
    Pal(PalOp, Obj),
    Convert(Obj, u8),
    Undo,
    Redo,
    LimitHistory(u8),
    LimitCels(u8, usize),
}

fn obj(wild: bool) -> BoxedStrategy<Obj> {
    let false_value_2 = if wild {
        any::<u32>().boxed()
    } else {
        (1000u32..1010).boxed()
    };
    prop_oneof![
        6 => (0usize..8).prop_map(Obj::Nth),
        1 => false_value_2.prop_map(Obj::False),
    ]
    .boxed()
}

fn position(wild: bool) -> BoxedStrategy<usize> {
    if wild {
        prop_oneof![
            4 => 0usize..6,
            1 => any::<usize>(),
            1 => Just(usize::MAX),
        ]
        .boxed()
    } else {
        (0usize..6).boxed()
    }
}

fn name_2(wild: bool) -> BoxedStrategy<String> {
    let base = prop_oneof![
        6 => "[a-z]{0,8}",
        1 => Just("a".repeat(255)),
        1 => Just("a".repeat(256)),
        1 => Just("é".repeat(128)),
    ];
    if wild {
        prop_oneof![
            3 => base,
            1 => "\\PC{0,300}",
        ]
        .boxed()
    } else {
        base.boxed()
    }
}

fn opacity_strategy() -> BoxedStrategy<u8> {
    prop_oneof![
        1 => Just(255u8),
        1 => any::<u8>(),
    ]
    .boxed()
}

fn pal_op() -> BoxedStrategy<PalOp> {
    prop_oneof![
        (0u8..10, any::<u8>()).prop_map(|(i, s)| PalOp::Color(i, s)),
        (0u8..10, 0u8..10).prop_map(|(a, b)| PalOp::Move(a, b)),
        any::<u8>().prop_map(PalOp::Permute),
        any::<u8>().prop_map(PalOp::Add),
        (0u8..10, 0u8..10).prop_map(|(a, b)| PalOp::Remove(a, b)),
        prop::option::of(0u8..10).prop_map(PalOp::Transp),
        (0u8..10, 0u16..10, -5i32..5).prop_map(|(a, b, c)| PalOp::Rotate(a, b, c)),
    ]
    .boxed()
}

/// Órdenes que pueden ir dentro de un `Batch` (o sueltas).
fn sheet(wild: bool) -> BoxedStrategy<Op> {
    let pixels_2 = || prop::collection::vec((0u8..6, 0u8..6, any::<u8>()), 0..4);
    let spans_2 = || prop::collection::vec((0u8..5, 0u8..5, 1u8..4), 0..3);
    prop_oneof![
        6 => (obj(wild), pixels_2()).prop_map(|(o, p)| Op::Set(o, p)),
        4 => (obj(wild), spans_2(), any::<u8>(), prop::bool::weighted(0.15))
            .prop_map(|(o, t, s, d)| Op::Fill(o, t, s, d)),
        3 => (name_2(wild), position(wild)).prop_map(|(n, a)| Op::Add(n, a)),
        2 => obj(wild).prop_map(Op::Remove),
        2 => (obj(wild), position(wild)).prop_map(|(o, t)| Op::Move(o, t)),
        2 => (obj(wild), name_2(wild)).prop_map(|(o, n)| Op::Rename(o, n)),
        2 => (obj(wild), opacity_strategy()).prop_map(|(o, p)| Op::Opacity(o, p)),
        2 => (obj(wild), any::<bool>()).prop_map(|(o, v)| Op::Visible(o, v)),
        4 => (pal_op(), obj(wild)).prop_map(|(p, o)| Op::Pal(p, o)),
        1 => (obj(wild), 0u8..4).prop_map(|(o, m)| Op::Convert(o, m)),
    ]
    .boxed()
}

/// Una orden de nivel superior. `wild`: ids, posiciones y nombres arbitrarios, `Batch`
/// anidados. `with_history`: añade `undo`, `redo` y los dos límites.
fn op(wild: bool, with_history: bool) -> BoxedStrategy<Op> {
    let interior = if wild {
        prop_oneof![
            5 => sheet(true),
            1 => prop::collection::vec(sheet(true), 0..3).prop_map(Op::Batch),
        ]
        .boxed()
    } else {
        sheet(false)
    };
    let batch = prop::collection::vec(interior, 0..4).prop_map(Op::Batch);
    if with_history {
        prop_oneof![
            14 => sheet(wild),
            3 => batch,
            2 => Just(Op::Undo),
            2 => Just(Op::Redo),
            1 => any::<u8>().prop_map(Op::LimitHistory),
            1 => (any::<u8>(), any::<usize>()).prop_map(|(k, v)| Op::LimitCels(k, v)),
        ]
        .boxed()
    } else {
        prop_oneof![
            14 => sheet(wild),
            3 => batch,
        ]
        .boxed()
    }
}

fn cel_of(doc: &Document, o: &Obj) -> ImageId {
    match o {
        Obj::Nth(n) => doc
            .layers()
            .get(n % doc.layers().len().max(1))
            .map_or(ImageId::from_raw(0), Layer::cel),
        Obj::False(raw) => ImageId::from_raw(*raw),
    }
}

fn layer_of(doc: &Document, o: &Obj) -> LayerId {
    match o {
        Obj::Nth(n) => doc
            .layers()
            .get(n % doc.layers().len().max(1))
            .map_or(LayerId::from_raw(0), Layer::id),
        Obj::False(raw) => LayerId::from_raw(*raw),
    }
}

fn length_palette(doc: &Document) -> usize {
    doc.palette().map_or(1, Palette::len)
}

fn command_palette(doc: &Document, p: &PalOp, image: ImageId) -> Command {
    match p {
        PalOp::Color(index, s) => Command::SetPaletteColor {
            image,
            index: *index,
            color: Rgba::new(*s, *s / 2, 255 - *s, 255),
        },
        PalOp::Move(from, to) => Command::MovePaletteEntry {
            image,
            from: *from,
            to: *to,
        },
        PalOp::Permute(seed) => {
            let n = length_palette(doc);
            let mut order: Vec<u8> = (0..n).map(|i| u8::try_from(i).unwrap_or(0)).collect();
            if seed % 2 == 1 {
                order.reverse();
            } else if !order.is_empty() {
                let k = usize::from(*seed) % order.len();
                order.rotate_left(k);
            }
            if seed % 7 == 0 {
                order.push(0);
            }
            Command::PermutePalette { image, order }
        }
        PalOp::Add(s) => Command::AddPaletteEntry {
            image,
            color: Rgba::new(*s, 10, *s / 3, 255),
        },
        PalOp::Remove(index, into) => Command::RemovePaletteEntry {
            image,
            index: *index,
            into: *into,
        },
        PalOp::Transp(index) => Command::SetTransparentIndex {
            image,
            index: *index,
        },
        PalOp::Rotate(start, len, steps) => Command::RotateColors {
            image,
            start: *start,
            len: *len,
            steps: *steps,
        },
    }
}

/// Traduce una orden a `Command`; `None` para `undo`, `redo` y los límites.
fn a_command(doc: &Document, order: &Op) -> Option<Command> {
    let mode_2 = doc.mode();
    let palette_2 = length_palette(doc);
    Some(match order {
        Op::Set(o, pixels_2) => Command::SetPixels {
            image: cel_of(doc, o),
            pixels: pixels_2
                .iter()
                .map(|&(x, y, s)| PixelEdit {
                    x: u32::from(x),
                    y: u32::from(y),
                    pixel: pixel_op(mode_2, s, palette_2),
                })
                .collect(),
        },
        Op::Fill(o, spans_2, s, without_sort) => {
            let mut v: Vec<(u32, u32, u32)> = spans_2
                .iter()
                .map(|&(y, x, length)| {
                    (u32::from(y), u32::from(x), u32::from(x) + u32::from(length))
                })
                .collect();
            if !*without_sort {
                // Ordenados y sin solapes, salvo lo que se salga del lienzo.
                v.sort_unstable();
                let mut fin: Option<(u32, u32)> = None;
                v.retain(|&(y, x0, x1)| {
                    let is = fin.is_none_or(|(py, px)| y > py || (y == py && x0 >= px));
                    if is {
                        fin = Some((y, x1));
                    }
                    is
                });
            }
            Command::FillSpans {
                image: cel_of(doc, o),
                spans: v
                    .into_iter()
                    .map(|(y, x0, x1)| Span {
                        y,
                        x_start: x0,
                        x_end: x1,
                    })
                    .collect(),
                pixel: pixel_op(mode_2, *s, palette_2),
            }
        }
        Op::Batch(v) => Command::Batch(v.iter().filter_map(|o| a_command(doc, o)).collect()),
        Op::Add(nom, at) => Command::AddLayer {
            name: nom.clone(),
            at: *at,
        },
        Op::Remove(o) => removes(layer_of(doc, o)),
        Op::Move(o, to) => moves(layer_of(doc, o), *to),
        Op::Rename(o, nom) => renames(layer_of(doc, o), nom),
        Op::Opacity(o, p) => opacity_2(layer_of(doc, o), *p),
        Op::Visible(o, v) => visible(layer_of(doc, o), *v),
        Op::Pal(p, o) => command_palette(doc, p, cel_of(doc, o)),
        Op::Convert(o, m) => Command::ConvertMode {
            image: cel_of(doc, o),
            target: mode_of(*m % 4),
            allowed: Allowed {
                luma: true,
                drop_alpha: Some([0, 0, 0]),
            },
        },
        Op::Undo | Op::Redo | Op::LimitHistory(_) | Op::LimitCels(..) => return None,
    })
}

enum Step2 {
    Apply(Result<bool, DocumentError>),
    Undo(Result<(), DocumentError>),
    Redo(Result<(), DocumentError>),
    Limit,
}

fn runs(doc: &mut Document, order: &Op) -> Step2 {
    match order {
        Op::Undo => Step2::Undo(doc.undo()),
        Op::Redo => Step2::Redo(doc.redo()),
        Op::LimitHistory(k) => {
            doc.set_history_limit(match k % 4 {
                0 => 0,
                1 => 1,
                2 => 4096,
                _ => Document::DEFAULT_HISTORY_LIMIT,
            });
            Step2::Limit
        }
        Op::LimitCels(k, value) => {
            let cel = doc.cel_bytes();
            doc.set_cells_limit(match k % 6 {
                0 => 0,
                1 => cel,
                2 => cel.saturating_mul(2),
                3 => cel.saturating_mul(3),
                4 => usize::MAX,
                _ => *value,
            });
            Step2::Limit
        }
        _ => match a_command(doc, order) {
            Some(command) => Step2::Apply(doc.apply(command)),
            None => Step2::Limit,
        },
    }
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

/// Lo que ha salido en algún `layers()` anterior, para la invariante (c).
#[derive(Default)]
struct Seen {
    layer_to_cel: HashMap<LayerId, ImageId>,
    cel_to_layer: HashMap<ImageId, LayerId>,
    /// Se ha bajado `cells_limit()` alguna vez: (e) deja de exigirse.
    limit_lowered: bool,
}

/// Las invariantes (a) a (e) de la propiedad 2.
fn invariants(doc: &Document, seen: &mut Seen) -> Result<(), TestCaseError> {
    let layers_2 = doc.layers();
    // (d)
    prop_assert!(
        (1..=Document::MAX_LAYERS).contains(&layers_2.len()),
        "{} capas",
        layers_2.len()
    );
    let (mut ids_layer, mut ids_cel) = (HashSet::new(), HashSet::new());
    let mut sum = 0usize;
    for layer_2 in layers_2 {
        prop_assert!(layer_2.name().len() <= Document::MAX_LAYER_NAME_BYTES);
        // (c) únicos ahora y de la misma capa para siempre.
        prop_assert!(ids_layer.insert(layer_2.id()), "LayerId repetido");
        prop_assert!(ids_cel.insert(layer_2.cel()), "ImageId repetido");
        if let Some(before) = seen.layer_to_cel.insert(layer_2.id(), layer_2.cel()) {
            prop_assert_eq!(
                before,
                layer_2.cel(),
                "la capa {:?} cambió de celda",
                layer_2.id()
            );
        }
        if let Some(before) = seen.cel_to_layer.insert(layer_2.cel(), layer_2.id()) {
            prop_assert_eq!(
                before,
                layer_2.id(),
                "la celda {:?} cambió de capa",
                layer_2.cel()
            );
        }
        prop_assert_eq!(doc.layer(layer_2.id()), Some(layer_2));
        // (b)
        if doc.mode() == Mode::Indexed {
            prop_assert_eq!(layer_2.opacity(), 255, "opacidad en indexado");
        }
        // (a)
        if let Some(image_2) = doc.image(layer_2.cel()) {
            prop_assert_eq!(
                (image_2.width(), image_2.height(), image_2.mode()),
                (doc.width(), doc.height(), doc.mode())
            );
            prop_assert_eq!(
                image_2.palette(),
                doc.palette(),
                "paleta distinta del lienzo"
            );
            sum += image_2.bytes().len();
        }
    }
    prop_assert_eq!(doc.palette().is_some(), doc.mode() == Mode::Indexed);
    // (e)
    prop_assert_eq!(doc.materialized_bytes(), sum);
    prop_assert!(
        sum <= doc.cells_limit() || seen.limit_lowered,
        "{sum} bytes materializados con límite {}",
        doc.cells_limit()
    );
    prop_assert_eq!(
        Ok(doc.cel_bytes()),
        Image::byte_len(doc.width(), doc.height(), doc.mode())
    );
    prop_assert_eq!(doc.worst_case_bytes(), layers_2.len() * doc.cel_bytes());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 1: tras una secuencia de comandos, deshacer cada `Ok(true)` devuelve el
    /// documento inicial y rehacerlos el final.
    #[test]
    fn property_1_undo_and_redo_with_layers(
        plan in plan_strategy(1..=4, true),
        commands in prop::collection::vec(op(false, false), 0..=30),
    ) {
        let mut doc = builds(&plan);
        // SPEC?: la propiedad dice «`undo()` hasta `Err`». Para tener varias capas el documento
        // inicial se construye con `AddLayer`, así que su historia no está vacía; se deshacen
        // exactamente los `Ok(true)` de la secuencia y se compara con el estado inicial (y
        // `history_bytes()` con el inicial, que es 0 si el plan no necesitó pasos).
        let initial = Snapshot::of(&doc);
        let mut steps = 0usize;
        for order in &commands {
            if let Step2::Apply(Ok(true)) = runs(&mut doc, order) {
                steps += 1;
            }
        }
        let final_ = Snapshot::of(&doc);

        for i in 0..steps {
            prop_assert_eq!(doc.undo(), Ok(()), "deshacer {}", i);
        }
        initial.requires_state(&doc, "tras deshacer todo")?;
        prop_assert_eq!(doc.history_bytes(), initial.history);
        if !initial.can_undo_2 {
            prop_assert_eq!(doc.history_bytes(), 0);
            prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
        }

        for i in 0..steps {
            prop_assert_eq!(doc.redo(), Ok(()), "rehacer {}", i);
        }
        final_.requires_state(&doc, "tras rehacer todo")?;
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    }

    /// Propiedad 2: las invariantes (a) a (e) valen tras cada operación, también las fallidas.
    #[test]
    fn property_2_invariants_after_any_operation(
        plan in plan_strategy(1..=4, true),
        commands in prop::collection::vec(op(false, true), 0..=30),
    ) {
        let mut doc = builds(&plan);
        let mut seen = Seen::default();
        invariants(&doc, &mut seen)?;
        for order in &commands {
            let limit = doc.cells_limit();
            runs(&mut doc, order);
            if doc.cells_limit() < limit {
                seen.limit_lowered = true;
            }
            invariants(&doc, &mut seen)?;
        }
    }

    /// Propiedad 3: ni un pánico, y lo que falla o no cambia no toca nada.
    #[test]
    fn property_3_atomicity_and_no_panics(
        plan in plan_strategy(1..=4, true),
        commands in prop::collection::vec(op(true, true), 0..=30),
    ) {
        let mut doc = builds(&plan);
        for order in &commands {
            let before = Snapshot::of(&doc);
            let unchanged = match runs(&mut doc, order) {
                Step2::Apply(Ok(true)) | Step2::Limit => false,
                Step2::Apply(_) | Step2::Undo(Err(_)) | Step2::Redo(Err(_)) => true,
                Step2::Undo(Ok(())) | Step2::Redo(Ok(())) => false,
            };
            if unchanged {
                before.requires(&doc, &format!("{order:?}"))?;
            }
        }
    }

    /// Propiedad 5: quitar una capa que no es la última y deshacer devuelve el mismo
    /// documento, con los mismos ids.
    #[test]
    fn property_5_removing_a_layer_and_undoing_returns_the_same_document(
        plan in plan_strategy(2..=4, false),
        choice in any::<usize>(),
    ) {
        let mut doc = builds(&plan);
        // SPEC?: en grises no se puede tener más de una capa (no hay valor transparente), así que
        // el plan nunca genera grises para esta propiedad.
        prop_assert!(doc.layers().len() >= 2);
        let before = Snapshot::of(&doc);
        let ids_before = ids(&doc);
        let layer_2 = doc.layers()[choice % (doc.layers().len() - 1)].id();

        prop_assert_eq!(doc.apply(removes(layer_2)), Ok(true));
        prop_assert_eq!(doc.layers().len(), ids_before.len() - 1);
        let after = Snapshot::of(&doc);

        prop_assert_eq!(doc.undo(), Ok(()));
        before.requires_state(&doc, "tras deshacer el borrado")?;
        prop_assert_eq!(ids(&doc), ids_before);

        prop_assert_eq!(doc.redo(), Ok(()));
        after.requires_state(&doc, "tras rehacer el borrado")?;
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: la tabla de capas contra un modelo independiente.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct LayerModel {
    id: LayerId,
    cel: ImageId,
    name_2: String,
    visible: bool,
    opacity_2: u8,
}

#[derive(Clone, Debug)]
enum MOp {
    Add(String, usize),
    Remove(Obj),
    Move(Obj, usize),
    Rename(Obj, String),
    Opacity(Obj, u8),
    Visible(Obj, bool),
    Undo,
    Redo,
}

fn mop() -> BoxedStrategy<MOp> {
    prop_oneof![
        3 => (name_2(false), position(false)).prop_map(|(n, a)| MOp::Add(n, a)),
        2 => obj(false).prop_map(MOp::Remove),
        2 => (obj(false), position(false)).prop_map(|(o, t)| MOp::Move(o, t)),
        2 => (obj(false), name_2(false)).prop_map(|(o, n)| MOp::Rename(o, n)),
        2 => (obj(false), opacity_strategy()).prop_map(|(o, p)| MOp::Opacity(o, p)),
        2 => (obj(false), any::<bool>()).prop_map(|(o, v)| MOp::Visible(o, v)),
        2 => Just(MOp::Undo),
        2 => Just(MOp::Redo),
    ]
    .boxed()
}

/// Documento de una capa 2 x 2 sin historia: 0 RGBA, 1 grises con alfa, 2 grises, 3 indexado
/// con índice transparente, 4 indexado sin él.
fn doc_model(kind: u8) -> Document {
    Document::new(match kind {
        0..=2 => Image::new(2, 2, mode_of(kind)).expect("imagen"),
        3 => img_indexed(2, 2, &[RED, GREEN, BLUE], Some(1)),
        _ => img_indexed(2, 2, &[RED, GREEN, BLUE], None),
    })
}

struct Model {
    layers_2: Vec<LayerModel>,
    /// Instantáneas de la tabla tras cada estado, y cuál es la actual.
    history: Vec<Vec<LayerModel>>,
    cursor: usize,
    /// Mayor id de capa y de celda visto hasta ahora (el contador de ids).
    max_layer: u32,
    max_cel: u32,
    mode_2: Mode,
    transparency: bool,
}

impl Model {
    fn new_2(kind: u8) -> Model {
        let layers_2 = vec![LayerModel {
            id: LayerId::from_raw(0),
            cel: ImageId::from_raw(0),
            name_2: "Capa 1".to_string(),
            visible: true,
            opacity_2: 255,
        }];
        Model {
            history: vec![layers_2.clone()],
            layers_2,
            cursor: 0,
            max_layer: 0,
            max_cel: 0,
            mode_2: mode_of(kind),
            // Grises e indexado sin índice transparente no tienen valor transparente.
            transparency: !matches!(kind, 2 | 4),
        }
    }

    /// Posición de la capa a la que se refiere `o` y su `LayerId`; `None` si es desconocida.
    fn goal(&self, o: &Obj) -> (LayerId, Option<usize>) {
        match o {
            Obj::Nth(n) => {
                let pos = n % self.layers_2.len();
                (self.layers_2[pos].id, Some(pos))
            }
            Obj::False(raw) => (LayerId::from_raw(*raw), None),
        }
    }

    /// Anota el cambio (si lo hubo) y compara la tabla del documento con el modelo.
    fn closes(&mut self, doc: &Document, change: bool) -> Result<(), TestCaseError> {
        if change {
            self.history.truncate(self.cursor + 1);
            self.history.push(self.layers_2.clone());
            self.cursor += 1;
        }
        let real_2 = doc.layers();
        prop_assert_eq!(real_2.len(), self.layers_2.len());
        for (real, expected) in real_2.iter().zip(&self.layers_2) {
            prop_assert_eq!(real.id(), expected.id);
            prop_assert_eq!(real.cel(), expected.cel);
            prop_assert_eq!(real.name(), expected.name_2.as_str());
            prop_assert_eq!(real.visible(), expected.visible);
            prop_assert_eq!(real.opacity(), expected.opacity_2);
        }
        Ok(())
    }

    fn step(&mut self, doc: &mut Document, order: &MOp) -> Result<(), TestCaseError> {
        let mut change = false;
        match order {
            MOp::Add(nom, at) => {
                let len = self.layers_2.len();
                let expected_2 = if nom.len() > Document::MAX_LAYER_NAME_BYTES {
                    Err(DocumentError::LayerName {
                        len: nom.len(),
                        limit: 255,
                    })
                } else if !self.transparency {
                    Err(DocumentError::NoTransparency { mode: self.mode_2 })
                } else if len >= Document::MAX_LAYERS {
                    Err(DocumentError::TooManyLayers { limit: 256 })
                } else if *at > len {
                    Err(DocumentError::LayerIndex {
                        index: *at,
                        max: len,
                    })
                } else {
                    Ok(true)
                };
                let real = doc.apply(adds_cmd(nom, *at));
                prop_assert_eq!(&real, &expected_2, "{:?}", order);
                if real == Ok(true) {
                    let new = doc.layers().get(*at).ok_or_else(|| {
                        TestCaseError::fail(format!("no hay capa en la posición {at}"))
                    })?;
                    // Los ids nuevos los reparte el documento, mayores que cualquiera visto.
                    prop_assert!(new.id().raw() > self.max_layer, "LayerId reutilizado");
                    prop_assert!(new.cel().raw() > self.max_cel, "ImageId reutilizado");
                    self.max_layer = new.id().raw();
                    self.max_cel = new.cel().raw();
                    self.layers_2.insert(
                        *at,
                        LayerModel {
                            id: new.id(),
                            cel: new.cel(),
                            name_2: nom.clone(),
                            visible: true,
                            opacity_2: 255,
                        },
                    );
                    change = true;
                }
            }
            MOp::Remove(o) => {
                let (id, pos) = self.goal(o);
                // SPEC?: con un id desconocido y una sola capa, la spec no dice si gana
                // `UnknownLayer` o `LastLayer`; esa combinación no se genera.
                if pos.is_none() && self.layers_2.len() == 1 {
                    return Ok(());
                }
                let expected_2 = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if self.layers_2.len() == 1 => Err(DocumentError::LastLayer),
                    Some(_) => Ok(true),
                };
                let real = doc.apply(removes(id));
                prop_assert_eq!(&real, &expected_2, "{:?}", order);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.layers_2.remove(pos);
                    change = true;
                }
            }
            MOp::Move(o, to) => {
                let (id, pos) = self.goal(o);
                // SPEC?: con un id desconocido y una posición fuera de rango, la spec no dice
                // qué error gana; con un id desconocido la posición se normaliza a 0.
                let to = if pos.is_none() { 0 } else { *to };
                let len = self.layers_2.len();
                let expected_2 = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if to >= len => Err(DocumentError::LayerIndex {
                        index: to,
                        max: len - 1,
                    }),
                    Some(pos) => Ok(pos != to),
                };
                let real = doc.apply(moves(id, to));
                prop_assert_eq!(&real, &expected_2, "{:?}", order);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    let layer_2 = self.layers_2.remove(pos);
                    self.layers_2.insert(to, layer_2);
                    change = true;
                }
            }
            MOp::Rename(o, nom) => {
                let (id, pos) = self.goal(o);
                // SPEC?: con un id desconocido y un nombre largo, la spec no dice qué error
                // gana; con un id desconocido el nombre se normaliza a uno corto.
                let nom = if pos.is_none() { "x" } else { nom.as_str() };
                let expected_2 = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if nom.len() > Document::MAX_LAYER_NAME_BYTES => {
                        Err(DocumentError::LayerName {
                            len: nom.len(),
                            limit: 255,
                        })
                    }
                    Some(pos) => Ok(self.layers_2[pos].name_2 != nom),
                };
                let real = doc.apply(renames(id, nom));
                prop_assert_eq!(&real, &expected_2, "{:?}", order);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.layers_2[pos].name_2 = nom.to_string();
                    change = true;
                }
            }
            MOp::Opacity(o, opacity_new) => {
                let (id, pos) = self.goal(o);
                let indexed_2 = self.mode_2 == Mode::Indexed;
                // SPEC?: con un id desconocido y opacidad ≠ 255 en indexado, la spec no dice qué
                // error gana; con un id desconocido en indexado la opacidad se normaliza a 255.
                let value = if pos.is_none() && indexed_2 {
                    255
                } else {
                    *opacity_new
                };
                let expected_2 = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if indexed_2 && value != 255 => Err(DocumentError::IndexedOpacity),
                    Some(pos) => Ok(self.layers_2[pos].opacity_2 != value),
                };
                let real = doc.apply(opacity_2(id, value));
                prop_assert_eq!(&real, &expected_2, "{:?}", order);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.layers_2[pos].opacity_2 = value;
                    change = true;
                }
            }
            MOp::Visible(o, v) => {
                let (id, pos) = self.goal(o);
                let expected_2 = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(pos) => Ok(self.layers_2[pos].visible != *v),
                };
                let real = doc.apply(visible(id, *v));
                prop_assert_eq!(&real, &expected_2, "{:?}", order);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.layers_2[pos].visible = *v;
                    change = true;
                }
            }
            MOp::Undo => {
                let expected_2 = if self.cursor > 0 {
                    self.cursor -= 1;
                    self.layers_2 = self.history[self.cursor].clone();
                    Ok(())
                } else {
                    Err(DocumentError::NothingToUndo)
                };
                prop_assert_eq!(doc.undo(), expected_2);
            }
            MOp::Redo => {
                let expected_2 = if self.cursor + 1 < self.history.len() {
                    self.cursor += 1;
                    self.layers_2 = self.history[self.cursor].clone();
                    Ok(())
                } else {
                    Err(DocumentError::NothingToRedo)
                };
                prop_assert_eq!(doc.redo(), expected_2);
            }
        }
        self.closes(doc, change)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: la tabla de capas del documento coincide con un modelo independiente tras
    /// cada orden, con los mismos resultados y errores, y `undo`/`redo` siguen al cursor.
    #[test]
    fn property_4_the_layer_table_against_a_model(
        kind in 0u8..5,
        commands in prop::collection::vec(mop(), 0..=30),
    ) {
        let mut doc = doc_model(kind);
        let mut model = Model::new_2(kind);
        model.closes(&doc, false)?;
        for order in &commands {
            model.step(&mut doc, order)?;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Unidad 3.3, cambio C-01 (specs/hito-3-capas-vista.md, «Cambios exactos»):
// `Document::canvas_image(&self) -> &Image`, la plantilla de 1 x 1 con el modo y la paleta del
// lienzo. Fase de pruebas en rojo: no compilan hasta que exista `canvas_image`.
// ---------------------------------------------------------------------------------------

mod canvas_image {
    use super::*;

    /// El lienzo y `canvas_image()` coinciden: 1 x 1, el modo y la paleta del documento.
    fn matches(d: &Document, context: &str) {
        let template = d.canvas_image();
        assert_eq!(template.mode(), d.mode(), "{context}: modo");
        assert_eq!(template.palette(), d.palette(), "{context}: paleta");
        assert_eq!(
            (template.width(), template.height()),
            (1, 1),
            "{context}: tamaño"
        );
    }

    /// (a) En los cuatro modos: modo y paleta del lienzo (`None` salvo indexado), 1 x 1.
    #[test]
    fn a_has_the_canvas_mode_and_palette_in_all_four_modes() {
        let images = [
            img_rgba(4, 3),
            Image::new(4, 3, Mode::Gray).expect("imagen de grises"),
            Image::new(4, 3, Mode::GrayAlpha).expect("imagen de grises con alfa"),
            img_indexed(4, 3, &[RED, GREEN, BLUE], Some(0)),
        ];
        let modes = [Mode::Rgba, Mode::Gray, Mode::GrayAlpha, Mode::Indexed];
        for (image_2, mode_2) in images.into_iter().zip(modes) {
            let d = Document::new(image_2);
            assert_eq!(d.mode(), mode_2);
            matches(&d, &format!("{mode_2:?}"));
            assert_eq!(
                d.canvas_image().palette().is_some(),
                mode_2 == Mode::Indexed
            );
        }
    }

    /// (b) Sigue al lienzo en `apply`, `undo` y `redo`: comandos de paleta en indexado y
    /// `ConvertMode` de una capa (RGBA a indexado y vuelta).
    #[test]
    fn b_follows_canvas_changes_in_apply_undo_and_redo() {
        // Paleta: color, entrada nueva e índice transparente.
        let mut d = Document::new(img_indexed(4, 3, &[RED, GREEN, BLUE], Some(0)));
        let cel = d.layers()[0].cel();
        let commands_2 = [
            Command::SetPaletteColor {
                image: cel,
                index: 1,
                color: WHITE,
            },
            Command::AddPaletteEntry {
                image: cel,
                color: YELLOW,
            },
            Command::SetTransparentIndex {
                image: cel,
                index: Some(2),
            },
        ];
        let mut palettes = vec![d.palette().cloned()];
        for (i, command) in commands_2.into_iter().enumerate() {
            assert_eq!(d.apply(command), Ok(true), "comando {i}");
            matches(&d, &format!("apply {i}"));
            assert_eq!(d.canvas_image().palette(), d.palette());
            palettes.push(d.palette().cloned());
        }
        assert!(
            palettes.windows(2).all(|p| p[0] != p[1]),
            "cada paso cambia"
        );
        for i in (0..commands_len(&palettes)).rev() {
            d.undo().expect("deshacer");
            matches(&d, &format!("undo {i}"));
            assert_eq!(d.canvas_image().palette(), palettes[i].as_ref());
        }
        for (i, expected) in palettes.iter().enumerate().skip(1) {
            d.redo().expect("rehacer");
            matches(&d, &format!("redo {i}"));
            assert_eq!(d.canvas_image().palette(), expected.as_ref());
        }

        // Conversión de modo de una capa: RGBA -> indexado y de vuelta.
        let mut d = doc_with_colors();
        let cel = d.layers()[0].cel();
        matches(&d, "rgba");
        assert_eq!(d.canvas_image().mode(), Mode::Rgba);
        assert_eq!(d.apply(convert(cel, Mode::Indexed)), Ok(true));
        matches(&d, "apply a indexado");
        assert_eq!(d.canvas_image().mode(), Mode::Indexed);
        assert!(d.canvas_image().palette().is_some());
        assert_eq!(d.apply(convert(cel, Mode::Rgba)), Ok(true));
        matches(&d, "apply a rgba");
        assert_eq!(d.canvas_image().palette(), None);
        d.undo().expect("deshacer");
        matches(&d, "undo a indexado");
        assert_eq!(d.canvas_image().mode(), Mode::Indexed);
        d.undo().expect("deshacer");
        matches(&d, "undo a rgba");
        assert_eq!(d.canvas_image().mode(), Mode::Rgba);
        d.redo().expect("rehacer");
        matches(&d, "redo a indexado");
        assert_eq!(d.canvas_image().mode(), Mode::Indexed);
        d.redo().expect("rehacer");
        matches(&d, "redo a rgba");
        assert_eq!(d.canvas_image().mode(), Mode::Rgba);
    }

    fn commands_len(palettes: &[Option<Palette>]) -> usize {
        palettes.len() - 1
    }

    /// (c) Sigue válida con todas las celdas vacías y no es ninguna celda: `image(cel)` es
    /// `None` aunque `canvas_image()` exista.
    #[test]
    fn c_stays_valid_with_empty_cels() {
        // RGBA: dos capas y se quita la que tenía imagen (queda solo la vacía).
        let mut d = doc_d();
        let a = d.layers()[0].id();
        let b = adds(&mut d, "B", 1);
        assert_eq!(d.apply(removes(a)), Ok(true));
        assert_eq!(d.layers().len(), 1);
        assert!(d.image(cel(&d, b)).is_none(), "la celda restante es vacía");
        matches(&d, "rgba sin imagen");
        d.undo().expect("deshacer");
        matches(&d, "rgba tras undo");

        // Indexado: capas vacías encima y la paleta cambia con todas las celdas vacías.
        let mut d = Document::new(img_indexed(4, 3, &[RED, GREEN, BLUE], Some(0)));
        let a = d.layers()[0].id();
        let b = adds(&mut d, "B", 1);
        let c = adds(&mut d, "C", 2);
        matches(&d, "indexado con capas vacías encima");
        assert_eq!(d.apply(removes(a)), Ok(true));
        for layer_2 in [b, c] {
            assert!(d.image(cel(&d, layer_2)).is_none());
        }
        matches(&d, "indexado sin imágenes");
        let cb = cel(&d, b);
        assert_eq!(
            d.apply(Command::SetPaletteColor {
                image: cb,
                index: 1,
                color: WHITE,
            }),
            Ok(true)
        );
        matches(&d, "paleta editada con celdas vacías");
        assert_eq!(
            d.canvas_image().palette().expect("paleta").get(1),
            Ok(WHITE)
        );
        assert!(d.image(cb).is_none(), "editar la paleta no materializa");
        d.undo().expect("deshacer");
        matches(&d, "paleta deshecha con celdas vacías");
        assert_eq!(
            d.canvas_image().palette().expect("paleta").get(1),
            Ok(GREEN)
        );
    }

    /// (d) Es de solo lectura: no cambia `state_id`, `materialized_bytes()`, la historia ni la
    /// igualdad del documento (llamarla antes y después es inocuo).
    #[test]
    fn d_is_harmless_it_does_not_touch_state_bytes_history_or_equality() {
        let (mut d, [_, b, _]) = three_layers();
        assert_eq!(d.apply(sets(cel(&d, b), 1, 1, C1)), Ok(true));
        d.undo().expect("deshacer");
        let snapshot = Snapshot::of(&d);
        let copy = d.clone();

        let _ = d.canvas_image();
        snapshot.assert_equal(&d, "tras llamarla una vez");
        let _ = (d.canvas_image(), d.canvas_image());
        snapshot.assert_equal(&d, "tras llamarla varias veces");
        assert_eq!(d, copy);
        assert_eq!(copy, d);

        // Y el documento sigue evolucionando igual que su copia, sin haberla llamado.
        let mut without_call = copy.clone();
        let command = sets(cel(&d, b), 2, 2, C2);
        assert_eq!(d.apply(command.clone()), Ok(true));
        assert_eq!(without_call.apply(command), Ok(true));
        let _ = d.canvas_image();
        assert_eq!(d, without_call);
        assert_eq!(d.state_id(), without_call.state_id());
        assert_eq!(d.materialized_bytes(), without_call.materialized_bytes());
        assert_eq!(d.history_bytes(), without_call.history_bytes());
    }
}

// ---------------------------------------------------------------------------------------
// Unidad 3.4, cambios P-01 a P-04 (specs/hito-3-capas-interfaz.md, «Lista exacta de cambios en
// `px-core`»): `Layer::locked()`, `Command::SetLayerLocked` y `Command::DuplicateLayer`. Filas 1
// y 2 de «Casos», propiedad 1 y condición 1 del visto bueno. Fase de pruebas en rojo: no
// compilan hasta que existan los dos comandos y `locked()`.
//
// Las cifras de bytes son las de la spec de 3.1 (RGBA 4 x 3: celda de 48 bytes, `three_layers()`
// tiene la celda de `A` materializada y las de `B` y `C` vacías). Los errores de
// `DuplicateLayer` se comprueban con `without_effect`: ni documento, ni historia, ni `state_id`, ni
// `materialized_bytes()` cambian (es la forma observable de «antes de reservar»).
//
// SPEC?: «rehacer repone la copia sin copiar ni reservar celdas» no se prueba bajando el límite
// de celdas antes del `redo` (la spec no dice si `redo` de un paso con celda lo vuelve a
// comprobar, como hace `redo` de una escritura en la fila 11 b de 3.1); solo se prueba que
// rehacer deja el documento, el `state_id` y los bytes materializados del instante posterior.
// ---------------------------------------------------------------------------------------

mod duplicate_and_lock {
    use super::*;
    use px_core::BlendMode;

    fn locks(layer: LayerId, locked: bool) -> Command {
        Command::SetLayerLocked { layer, locked }
    }

    fn duplicates(layer: LayerId, name: &str) -> Command {
        Command::DuplicateLayer {
            layer,
            name: name.to_string(),
        }
    }

    // -----------------------------------------------------------------------------------
    // Fila 1: `SetLayerLocked`.
    // -----------------------------------------------------------------------------------

    /// (a) Bloquear, repetir, desbloquear, repetir; deshacer y rehacer; `==` y `state_id`;
    /// en los cuatro modos. Nace desbloqueada en `Document::new` y en `AddLayer`.
    #[test]
    fn row_01_a_lock_unlock_and_repeat_are_one_step_each() {
        let (mut d, b) = base();
        let a = d.layers()[0].id();
        assert!(!d.layer(a).expect("A").locked(), "Document::new");
        assert!(!d.layer(b).expect("B").locked(), "AddLayer");

        let initial = d.clone();
        let state_0 = d.state_id();
        let materialized = d.materialized_bytes();

        assert_eq!(d.apply(locks(b, true)), Ok(true));
        let state_1 = d.state_id();
        assert_ne!(state_1, state_0, "state_id nuevo al bloquear");
        assert!(d.layer(b).expect("B").locked());
        assert!(!d.layer(a).expect("A").locked(), "solo la pedida");
        assert_eq!(d.materialized_bytes(), materialized);
        // Dos documentos que solo difieren en el bloqueo no son iguales.
        assert_ne!(d, initial);
        assert_ne!(initial, d);
        let locked_2_2 = d.clone();

        // Repetir: `Ok(false)` sin tocar nada (ni historia ni `state_id`).
        without_effect(&mut d, locks(b, true), Ok(false));

        assert_eq!(d.apply(locks(b, false)), Ok(true));
        let state_2 = d.state_id();
        assert!(state_2 != state_1 && state_2 != state_0, "state_id nuevo");
        assert!(!d.layer(b).expect("B").locked());
        assert_eq!(d, initial, "desbloquear deja un documento igual");
        without_effect(&mut d, locks(b, false), Ok(false));

        // Un paso por comando: dos `undo` y dos `redo`.
        d.undo().expect("deshacer el desbloqueo");
        assert_eq!((&d, d.state_id()), (&locked_2_2, state_1));
        d.undo().expect("deshacer el bloqueo");
        assert_eq!((&d, d.state_id()), (&initial, state_0));
        d.redo().expect("rehacer el bloqueo");
        assert_eq!((&d, d.state_id()), (&locked_2_2, state_1));
        d.redo().expect("rehacer el desbloqueo");
        assert_eq!((&d, d.state_id()), (&initial, state_2));
        assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));

        // Sirve en todos los modos, incluido gris (una sola capa).
        let images = [
            img_rgba(4, 3),
            Image::new(4, 3, Mode::Gray).expect("gris"),
            Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"),
            img_indexed(4, 3, &[RED, GREEN], Some(1)),
        ];
        for image_2 in images {
            let mode_2 = image_2.mode();
            let mut m = Document::new(image_2);
            let layer_2 = m.layers()[0].id();
            let before = m.clone();
            assert_eq!(m.apply(locks(layer_2, true)), Ok(true), "{mode_2:?}");
            assert!(m.layer(layer_2).expect("capa").locked(), "{mode_2:?}");
            assert_ne!(m, before, "{mode_2:?}");
            m.undo().expect("deshacer");
            assert_eq!(m, before, "{mode_2:?}");
            m.redo().expect("rehacer");
            assert!(m.layer(layer_2).expect("capa").locked(), "{mode_2:?}");
        }
    }

    /// (b) Capa desconocida (también una ya quitada) y dentro de un `Batch`.
    #[test]
    fn row_01_b_unknown_layer_and_in_a_batch_change_nothing() {
        let (mut d, b) = base();
        let ca = ImageId::from_raw(0);
        let false_value = LayerId::from_raw(999);
        without_effect(
            &mut d,
            locks(false_value, true),
            Err(DocumentError::UnknownLayer(false_value)),
        );
        without_effect(
            &mut d,
            locks(false_value, false),
            Err(DocumentError::UnknownLayer(false_value)),
        );

        without_effect(
            &mut d,
            Command::Batch(vec![locks(b, true)]),
            Err(in_batch(0, DocumentError::NotBatchable)),
        );
        // También detrás de una escritura válida, que se revierte.
        without_effect(
            &mut d,
            Command::Batch(vec![sets(ca, 0, 0, C1), locks(b, true)]),
            Err(in_batch(1, DocumentError::NotBatchable)),
        );
        assert_eq!(pixel_rgba(&d, ca, 0, 0), T);
        assert!(!d.layer(b).expect("B").locked());

        // Una capa quitada ya no existe para el comando.
        assert_eq!(d.apply(removes(b)), Ok(true));
        without_effect(&mut d, locks(b, true), Err(DocumentError::UnknownLayer(b)));
    }

    // -----------------------------------------------------------------------------------
    // Fila 2: `DuplicateLayer`.
    // -----------------------------------------------------------------------------------

    /// (a) Copia de una capa con píxeles en medio de `[A, B, C]`: va en `posición + 1`, con
    /// `LayerId` e `ImageId` nuevos (tampoco los de una capa ya quitada), imagen `==` en otra
    /// memoria, un solo paso, y editar una no toca la otra. También arriba del todo y abajo.
    #[test]
    fn row_02_a_the_copy_goes_on_top_with_new_ids_and_without_sharing_memory() {
        let (mut d, [a, b, c]) = three_layers();
        assert_eq!(d.cel_bytes(), 48);
        let (ca, cb, cc) = (cel(&d, a), cel(&d, b), cel(&d, c));
        // Una capa añadida y quitada: sus ids no se reutilizan.
        let removed = adds(&mut d, "X", 3);
        let cel_removed = cel(&d, removed);
        assert_eq!(d.apply(removes(removed)), Ok(true));
        assert_eq!(d.apply(sets(cb, 1, 1, C1)), Ok(true));
        assert_eq!(d.apply(sets(cb, 3, 2, C2)), Ok(true));

        let before = d.clone();
        let state = d.state_id();
        let materialized = d.materialized_bytes();
        assert_eq!(materialized, 96, "A y B tienen celda");

        assert_eq!(d.apply(duplicates(b, "B copia")), Ok(true));
        assert_eq!(d.layers().len(), 4);
        assert_eq!(d.layers()[0].id(), a);
        assert_eq!(d.layers()[1].id(), b);
        assert_eq!(d.layers()[3].id(), c);
        let copy = d.layers()[2].id();
        let cel_copy = cel(&d, copy);
        assert_eq!(d.layer(copy).expect("copia").name(), "B copia");
        assert!(![a, b, c, removed].contains(&copy), "LayerId nuevo");
        assert!(
            ![ca, cb, cc, cel_removed].contains(&cel_copy),
            "ImageId nuevo"
        );
        // Las demás capas siguen como estaban.
        assert_eq!(cel(&d, b), cb);
        assert_eq!(d.layer(b).expect("B").name(), "B");

        let original = d.image(cb).expect("B tiene imagen");
        let image_copy = d.image(cel_copy).expect("la copia tiene imagen");
        assert_eq!(original, image_copy);
        assert_ne!(
            original.bytes().as_ptr(),
            image_copy.bytes().as_ptr(),
            "no comparte memoria"
        );
        assert_eq!(d.materialized_bytes(), materialized + d.cel_bytes());

        // Un solo paso: deshacer deja el documento igual, y rehacer repone la copia.
        let after_2 = d.clone();
        let state_after = d.state_id();
        assert_ne!(state_after, state);
        d.undo().expect("deshacer la copia");
        assert_eq!(d, before);
        assert_eq!(d.state_id(), state);
        assert_eq!(d.materialized_bytes(), materialized);
        assert!(d.image(cel_copy).is_none() && d.layer(copy).is_none());
        d.redo().expect("rehacer la copia");
        assert_eq!(d, after_2);
        assert_eq!(d.state_id(), state_after);
        assert_eq!(d.materialized_bytes(), materialized + d.cel_bytes());
        assert_eq!(cel(&d, copy), cel_copy, "rehacer repone los mismos ids");

        // Editar una no toca la otra, en los dos sentidos.
        assert_eq!(d.apply(sets(cel_copy, 0, 0, C2)), Ok(true));
        assert_eq!(pixel_rgba(&d, cb, 0, 0), T);
        assert_eq!(d.apply(sets(cb, 2, 0, C1)), Ok(true));
        assert_eq!(pixel_rgba(&d, cel_copy, 2, 0), T);
        assert_eq!(pixel_rgba(&d, cel_copy, 1, 1), C1, "lo copiado sigue");

        // Arriba del todo: `posición + 1 == len`; y de la de abajo: va en la posición 1.
        assert_eq!(d.apply(duplicates(c, "C copia")), Ok(true));
        assert_eq!(d.layers().len(), 5);
        assert_eq!(d.layers()[3].id(), c);
        assert_eq!(d.layers()[4].name(), "C copia");
        assert_eq!(d.apply(duplicates(a, "A copia")), Ok(true));
        assert_eq!(d.layers().len(), 6);
        assert_eq!(d.layers()[0].id(), a);
        assert_eq!(d.layers()[1].name(), "A copia");
        assert_eq!(d.layers()[2].id(), b);
    }

    /// (b) La copia hereda visible, opacidad, mezcla y bloqueo; cambiarlos en una no toca la
    /// otra; una capa corriente da una copia corriente.
    #[test]
    fn row_02_b_the_copy_inherits_opacity_blend_visibility_and_lock() {
        let (mut d, [_a, b, _c]) = three_layers();
        let cb = cel(&d, b);
        assert_eq!(d.apply(sets(cb, 0, 0, C1)), Ok(true));
        assert_eq!(d.apply(opacity_2(b, 128)), Ok(true));
        assert_eq!(
            d.apply(Command::SetLayerBlend {
                layer: b,
                blend: BlendMode::Multiply,
            }),
            Ok(true)
        );
        assert_eq!(d.apply(visible(b, false)), Ok(true));
        assert_eq!(d.apply(locks(b, true)), Ok(true));
        let original = d.layer(b).expect("B").clone();

        assert_eq!(d.apply(duplicates(b, "B2")), Ok(true));
        let copy = d.layers()[2].id();
        {
            let l = d.layer(copy).expect("copia");
            assert_eq!(l.name(), "B2");
            assert!(!l.visible());
            assert_eq!(l.opacity(), 128);
            assert_eq!(l.blend(), BlendMode::Multiply);
            assert!(l.locked(), "la copia conserva el bloqueo");
        }
        assert_eq!(d.layer(b).expect("B"), &original, "el original no cambia");

        // Desbloquear la copia no desbloquea el original, y al revés.
        assert_eq!(d.apply(locks(copy, false)), Ok(true));
        assert!(d.layer(b).expect("B").locked());
        assert_eq!(d.apply(opacity_2(b, 10)), Ok(true));
        assert_eq!(d.layer(copy).expect("copia").opacity(), 128);

        // Una capa corriente da una copia corriente.
        assert_eq!(d.apply(duplicates(copy, "B3")), Ok(true));
        let third = d.layers()[3].id();
        let l = d.layer(third).expect("tercera");
        assert!(!l.locked(), "hereda el desbloqueo");
        assert_eq!(
            (l.opacity(), l.blend(), l.visible()),
            (128, BlendMode::Multiply, false)
        );
    }

    /// (c) Una celda vacía sigue vacía: `materialized_bytes()` no sube, aunque no quepa otra
    /// celda; deshacer y rehacer la dejan vacía; escribir en la copia no materializa la otra.
    #[test]
    fn row_02_c_an_empty_cel_stays_empty_and_reserves_nothing() {
        let (mut d, [_a, b, c]) = three_layers();
        assert_eq!(d.apply(opacity_2(b, 100)), Ok(true));
        assert_eq!(d.apply(locks(b, true)), Ok(true));
        let cb = cel(&d, b);
        let materialized = d.materialized_bytes();
        assert_eq!(materialized, 48);
        assert_eq!(d.worst_case_bytes(), 3 * 48);
        // El límite justo no deja materializar ninguna otra celda: la copia vacía cabe igual.
        d.set_cells_limit(materialized);
        let before = d.clone();
        let state = d.state_id();

        assert_eq!(d.apply(duplicates(b, "vacía")), Ok(true));
        let copy = d.layers()[2].id();
        let cel_copy = cel(&d, copy);
        assert_ne!(cel_copy, cb);
        assert!(d.image(cb).is_none() && d.image(cel_copy).is_none());
        assert_eq!(d.materialized_bytes(), materialized);
        assert_eq!(d.worst_case_bytes(), 4 * 48);
        let l = d.layer(copy).expect("copia");
        assert_eq!((l.opacity(), l.locked()), (100, true));
        assert_eq!(d.layers()[3].id(), c);

        let after_2 = d.clone();
        d.undo().expect("deshacer");
        assert_eq!((&d, d.state_id()), (&before, state));
        d.redo().expect("rehacer");
        assert_eq!(d, after_2);
        assert!(d.image(cel_copy).is_none());
        assert_eq!(d.materialized_bytes(), materialized);

        // Con sitio, escribir en la copia materializa solo la copia.
        d.set_cells_limit(1 << 20);
        assert_eq!(d.apply(sets(cel_copy, 0, 0, C1)), Ok(true));
        assert!(d.image(cb).is_none(), "el original sigue vacío");
        assert_eq!(d.materialized_bytes(), materialized + 48);
    }

    /// (d) Errores, en este orden y sin tocar nada: `UnknownLayer`, `LayerName`,
    /// `TooManyLayers`, `CelsTooLarge`; y `NotBatchable` dentro de un lote.
    #[test]
    fn row_02_d_errors_come_in_order_and_without_touching_anything() {
        let false_value = LayerId::from_raw(999);
        let length = "a".repeat(256);

        // Con 256 capas: UnknownLayer gana a LayerName y a TooManyLayers; LayerName a
        // TooManyLayers; y con un nombre bueno, TooManyLayers (con celda o sin ella, y aunque
        // tampoco quepa otra celda: gana a CelsTooLarge).
        let mut d = doc_256(false);
        let first = d.layers()[0].id();
        let last = d.layers()[255].id();
        without_effect(
            &mut d,
            duplicates(false_value, &length),
            Err(DocumentError::UnknownLayer(false_value)),
        );
        without_effect(
            &mut d,
            duplicates(first, &length),
            Err(DocumentError::LayerName {
                len: 256,
                limit: 255,
            }),
        );
        let with_cel = Err(DocumentError::TooManyLayers { limit: 256 });
        without_effect(&mut d, duplicates(first, "copia"), with_cel.clone());
        without_effect(&mut d, duplicates(last, "copia"), with_cel.clone());
        d.set_cells_limit(d.materialized_bytes());
        without_effect(&mut d, duplicates(first, "copia"), with_cel);
        assert_eq!(d.layers().len(), 256);

        // LayerName antes que CelsTooLarge, con 255 bytes sí vale.
        let (mut d, [a, b, c]) = three_layers();
        let cb = cel(&d, b);
        assert_eq!(d.apply(sets(cb, 1, 1, C1)), Ok(true));
        let materialized = d.materialized_bytes();
        assert_eq!(materialized, 96);
        d.set_cells_limit(materialized);
        without_effect(
            &mut d,
            duplicates(b, &length),
            Err(DocumentError::LayerName {
                len: 256,
                limit: 255,
            }),
        );
        // CelsTooLarge: `needed = materialized_bytes() + cel_bytes()`, solo si la celda tiene
        // imagen; una capa vacía sí se copia aunque no quepa otra celda.
        let without_place = Err(DocumentError::CelsTooLarge {
            needed: 96 + 48,
            limit: 96,
        });
        without_effect(&mut d, duplicates(b, "copia"), without_place.clone());
        without_effect(&mut d, duplicates(a, "copia"), without_place);
        // Un byte menos de lo que hace falta sigue sin caber; justo lo que hace falta, sí.
        d.set_cells_limit(96 + 48 - 1);
        without_effect(
            &mut d,
            duplicates(b, "copia"),
            Err(DocumentError::CelsTooLarge {
                needed: 144,
                limit: 143,
            }),
        );
        assert_eq!(
            d.apply(duplicates(c, &"a".repeat(255))),
            Ok(true),
            "vacía y 255 bytes"
        );
        assert_eq!(
            d.apply(duplicates(b, "copia")),
            Err(DocumentError::CelsTooLarge {
                needed: 144,
                limit: 143,
            })
        );
        d.set_cells_limit(144);
        assert_eq!(d.apply(duplicates(b, "copia")), Ok(true));
        assert_eq!(d.materialized_bytes(), 144);

        // Dentro de un lote: NotBatchable, también detrás de una escritura que se revierte.
        let (mut d, [_a, b, _c]) = three_layers();
        let ca = ImageId::from_raw(0);
        without_effect(
            &mut d,
            Command::Batch(vec![duplicates(b, "copia")]),
            Err(in_batch(0, DocumentError::NotBatchable)),
        );
        without_effect(
            &mut d,
            Command::Batch(vec![sets(ca, 0, 0, C1), duplicates(b, "copia")]),
            Err(in_batch(1, DocumentError::NotBatchable)),
        );
        assert_eq!(pixel_rgba(&d, ca, 0, 0), T);
        assert_eq!(d.layers().len(), 3);
    }

    /// Copia `source` en `d` y comprueba la copia: imagen `==` en otra memoria, mismas
    /// propiedades, `+ cel_bytes()` y un solo paso que se deshace y se rehace.
    fn copy_exact(mut d: Document, source: LayerId, label: &str) {
        let before = d.clone();
        let state = d.state_id();
        let materialized = d.materialized_bytes();
        let p = d
            .layers()
            .iter()
            .position(|l| l.id() == source)
            .expect("el origen existe");
        assert_eq!(d.apply(duplicates(source, "Copia")), Ok(true), "{label}");
        let (o, c) = (&d.layers()[p], &d.layers()[p + 1]);
        assert_eq!(c.name(), "Copia", "{label}");
        assert!(c.id() != o.id() && c.cel() != o.cel(), "{label}: ids");
        assert_eq!(
            (c.visible(), c.opacity(), c.blend(), c.locked()),
            (o.visible(), o.opacity(), o.blend(), o.locked()),
            "{label}: propiedades"
        );
        let io = d.image(o.cel()).expect("el original tiene imagen");
        let ic = d.image(c.cel()).expect("la copia tiene imagen");
        assert_eq!(io, ic, "{label}: imagen");
        assert_eq!(io.palette(), ic.palette(), "{label}: paleta");
        assert_ne!(io.bytes().as_ptr(), ic.bytes().as_ptr(), "{label}: memoria");
        assert_eq!(d.materialized_bytes(), materialized + d.cel_bytes());
        let after_2 = d.clone();
        d.undo().expect("deshacer");
        assert_eq!((&d, d.state_id()), (&before, state), "{label}: undo");
        d.redo().expect("rehacer");
        assert_eq!(d, after_2, "{label}: redo");
    }

    /// (e) Funciona en RGBA, grises con alfa, indexado (con transparente) y grises.
    #[test]
    fn row_02_e_duplicate_works_in_rgba_gray_alpha_indexed_and_gray() {
        // RGBA.
        let (mut d, [_a, b, _c]) = three_layers();
        let cb = cel(&d, b);
        assert_eq!(d.apply(sets(cb, 2, 1, C2)), Ok(true));
        copy_exact(d, b, "RGBA");

        // Grises con alfa.
        let mut d = Document::new(Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"));
        let b = adds(&mut d, "B", 1);
        let pixel = Pixel::GrayAlpha {
            gray: 9,
            alpha: 200,
        };
        assert_eq!(d.apply(set_px(cel(&d, b), 1, 2, pixel)), Ok(true));
        copy_exact(d, b, "GrayAlpha");

        // Indexado con color transparente, con la capa bloqueada.
        let mut d = Document::new(img_indexed(4, 3, &[RED, GREEN, BLUE], Some(2)));
        let b = adds(&mut d, "B", 1);
        assert_eq!(
            d.apply(set_px(cel(&d, b), 2, 1, Pixel::Indexed(1))),
            Ok(true)
        );
        assert_eq!(d.apply(locks(b, true)), Ok(true));
        copy_exact(d, b, "Indexed");

        // Grises: no admite capas nuevas (no hay valor transparente), pero sí copiar.
        let mut d = Document::new(Image::new(4, 3, Mode::Gray).expect("gris"));
        let a = d.layers()[0].id();
        assert_eq!(
            d.apply(set_px(ImageId::from_raw(0), 1, 1, Pixel::Gray(77))),
            Ok(true)
        );
        assert!(
            d.apply(adds_cmd("B", 1)).is_err(),
            "gris no admite AddLayer"
        );
        copy_exact(d, a, "Gray");
    }

    // -----------------------------------------------------------------------------------
    // Propiedad 1: duplicar y bloquear se deshacen exactamente.
    // -----------------------------------------------------------------------------------

    #[derive(Clone, Debug)]
    struct LayerPlan {
        opacity_2: u8,
        blend_2: u8,
        visible: bool,
        locked_2: bool,
        /// `None`: celda vacía; `Some`: píxeles `(x, y, seed)` (puede seguir vacía si todos
        /// son transparentes).
        content: Option<Vec<(u8, u8, u8)>>,
    }

    #[derive(Clone, Debug)]
    struct DocPlan {
        /// 0 RGBA, 1 grises con alfa, 2 indexado con índice transparente.
        kind: u8,
        w: u32,
        h: u32,
        palette_2: usize,
        transparent: usize,
        /// Celdas que caben además de las ya materializadas.
        slack: usize,
        layers_2: Vec<LayerPlan>,
    }

    fn layer_plan() -> impl Strategy<Value = LayerPlan> {
        (
            any::<u8>(),
            0u8..8,
            any::<bool>(),
            any::<bool>(),
            prop::option::of(prop::collection::vec(
                (any::<u8>(), any::<u8>(), any::<u8>()),
                1..6,
            )),
        )
            .prop_map(
                |(opacity_2, blend_2, visible, locked_2, content)| LayerPlan {
                    opacity_2,
                    blend_2,
                    visible,
                    locked_2,
                    content,
                },
            )
    }

    fn doc_plan() -> impl Strategy<Value = DocPlan> {
        (
            0u8..3,
            1u32..=130,
            1u32..=70,
            2usize..=8,
            0usize..8,
            0usize..=12,
            prop::collection::vec(layer_plan(), 1..=8),
        )
            .prop_map(
                |(kind, w, h, palette_2, transparent, slack, layers_2)| DocPlan {
                    kind,
                    w,
                    h,
                    palette_2,
                    transparent,
                    slack,
                    layers_2,
                },
            )
    }

    /// Lo que la secuencia sabe del documento: modo, tamaño y longitud de la paleta.
    struct Cx {
        mode_2: Mode,
        w: u32,
        h: u32,
        palette_2: usize,
    }

    fn builds_doc(plan: &DocPlan) -> (Document, Cx) {
        let (image_2, mode_2) = match plan.kind {
            0 => (img_rgba(plan.w, plan.h), Mode::Rgba),
            1 => (
                Image::new(plan.w, plan.h, Mode::GrayAlpha).expect("gris con alfa"),
                Mode::GrayAlpha,
            ),
            _ => {
                let t = u8::try_from(plan.transparent % plan.palette_2).expect("cabe en u8");
                (
                    img_indexed(plan.w, plan.h, &colors_different(plan.palette_2), Some(t)),
                    Mode::Indexed,
                )
            }
        };
        let cx = Cx {
            mode_2,
            w: plan.w,
            h: plan.h,
            palette_2: plan.palette_2,
        };
        let mut d = Document::new(image_2);
        for i in 1..plan.layers_2.len() {
            assert_eq!(d.apply(adds_cmd(&format!("Capa {}", i + 1), i)), Ok(true));
        }
        for (i, layer_2) in plan.layers_2.iter().enumerate() {
            let (id, image) = (d.layers()[i].id(), d.layers()[i].cel());
            if let Some(pixels_2) = &layer_2.content {
                let pixels = pixels_2
                    .iter()
                    .map(|&(x, y, seed)| PixelEdit {
                        x: u32::from(x) % plan.w,
                        y: u32::from(y) % plan.h,
                        pixel: pixel_valid(mode_2, seed, plan.palette_2),
                    })
                    .collect();
                assert!(d.apply(Command::SetPixels { image, pixels }).is_ok());
            }
            if mode_2 != Mode::Indexed {
                if layer_2.opacity_2 != 255 {
                    assert_eq!(d.apply(opacity_2(id, layer_2.opacity_2)), Ok(true));
                }
                let blend = BlendMode::ALL[usize::from(layer_2.blend_2) % 8];
                if blend != BlendMode::Normal {
                    assert_eq!(
                        d.apply(Command::SetLayerBlend { layer: id, blend }),
                        Ok(true)
                    );
                }
            }
            if !layer_2.visible {
                assert_eq!(d.apply(visible(id, false)), Ok(true));
            }
            if layer_2.locked_2 {
                assert_eq!(d.apply(locks(id, true)), Ok(true));
            }
        }
        // Un límite que a veces no deja materializar más celdas.
        d.set_cells_limit(d.materialized_bytes() + plan.slack * d.cel_bytes());
        (d, cx)
    }

    #[derive(Clone, Debug)]
    enum Order {
        Add(usize),
        Dup(usize),
        Remove(usize),
        Move(usize, usize),
        Rename(usize, u8),
        Opacity(usize, u8),
        Visible(usize, bool),
        Blend(usize, u8),
        Locked(usize, bool),
        Set(usize, Vec<(u8, u8, u8)>),
        Fill(usize, u8, u8, u8, u8),
    }

    fn order() -> impl Strategy<Value = Order> {
        (
            0u8..20,
            any::<usize>(),
            any::<usize>(),
            any::<u8>(),
            any::<bool>(),
            prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>()), 1..5),
        )
            .prop_map(|(kind, k, t, v, b, px)| match kind {
                0..=1 => Order::Add(t),
                2..=6 => Order::Dup(k),
                7..=8 => Order::Remove(k),
                9..=10 => Order::Move(k, t),
                11 => Order::Rename(k, v),
                12 => Order::Opacity(k, v),
                13 => Order::Visible(k, b),
                14 => Order::Blend(k, v),
                15..=16 => Order::Locked(k, b),
                17..=18 => Order::Set(k, px),
                _ => Order::Fill(k, px[0].0, px[0].1, px[0].2, v),
            })
    }

    /// El comando de la orden con el documento vivo delante (los índices valen `% len`).
    fn translates(doc: &Document, o: &Order, cx: &Cx) -> Command {
        let layers_2 = doc.layers();
        let n = layers_2.len();
        let id = |k: usize| layers_2[k % n].id();
        match o {
            Order::Add(at) => adds_cmd(&format!("N{at}"), at % (n + 1)),
            Order::Dup(k) => duplicates(id(*k), &format!("Copia {}", k % 1000)),
            Order::Remove(k) => removes(id(*k)),
            Order::Move(k, to) => moves(id(*k), to % n),
            Order::Rename(k, s) => renames(id(*k), &format!("R{s}")),
            Order::Opacity(k, v) => opacity_2(id(*k), *v),
            Order::Visible(k, v) => visible(id(*k), *v),
            Order::Blend(k, m) => Command::SetLayerBlend {
                layer: id(*k),
                blend: BlendMode::ALL[usize::from(*m) % 8],
            },
            Order::Locked(k, v) => locks(id(*k), *v),
            Order::Set(k, pixels_2) => Command::SetPixels {
                image: layers_2[k % n].cel(),
                pixels: pixels_2
                    .iter()
                    .map(|&(x, y, seed)| PixelEdit {
                        x: u32::from(x) % cx.w,
                        y: u32::from(y) % cx.h,
                        pixel: pixel_valid(cx.mode_2, seed, cx.palette_2),
                    })
                    .collect(),
            },
            Order::Fill(k, y, x0, length, seed) => {
                let x_start = u32::from(*x0) % cx.w;
                Command::FillSpans {
                    image: layers_2[k % n].cel(),
                    spans: vec![Span {
                        y: u32::from(*y) % cx.h,
                        x_start,
                        x_end: (x_start + 1 + u32::from(*length)).min(cx.w),
                    }],
                    pixel: pixel_valid(cx.mode_2, *seed, cx.palette_2),
                }
            }
        }
    }

    /// Ids de capa y de celda que ya existieron alguna vez (los ids no se reutilizan).
    #[derive(Default)]
    struct Seen {
        layers_2: HashSet<LayerId>,
        cels: HashSet<ImageId>,
    }

    impl Seen {
        fn records(&mut self, doc: &Document) {
            for l in doc.layers() {
                self.layers_2.insert(l.id());
                self.cels.insert(l.cel());
            }
        }
    }

    /// `materialized_bytes()` es la suma real de las celdas con imagen y cabe en el límite; los
    /// ids de capa y de celda no se repiten.
    fn invariants(doc: &Document) -> Result<(), TestCaseError> {
        let real: usize = doc
            .layers()
            .iter()
            .filter_map(|l| doc.image(l.cel()))
            .map(|i| i.bytes().len())
            .sum();
        prop_assert_eq!(doc.materialized_bytes(), real);
        prop_assert!(doc.materialized_bytes() <= doc.cells_limit());
        let layers_2: HashSet<LayerId> = doc.layers().iter().map(Layer::id).collect();
        let cels: HashSet<ImageId> = doc.layers().iter().map(Layer::cel).collect();
        prop_assert_eq!(layers_2.len(), doc.layers().len());
        prop_assert_eq!(cels.len(), doc.layers().len());
        Ok(())
    }

    type Summary<'a> = (
        LayerId,
        ImageId,
        &'a str,
        bool,
        u8,
        BlendMode,
        bool,
        Option<&'a Image>,
    );

    fn summary<'a>(doc: &'a Document, l: &'a Layer) -> Summary<'a> {
        (
            l.id(),
            l.cel(),
            l.name(),
            l.visible(),
            l.opacity(),
            l.blend(),
            l.locked(),
            doc.image(l.cel()),
        )
    }

    /// Un píxel válido del modo y distinto de `actual`, si lo hay.
    fn other_pixel(cx: &Cx, actual: Pixel) -> Option<Pixel> {
        (1..=8u8)
            .map(|s| pixel_valid(cx.mode_2, s, cx.palette_2))
            .find(|p| *p != actual)
    }

    /// Tras una copia `Ok(true)`: el original y las demás capas no cambian, la copia tiene ids
    /// nuevos, el nombre dado, las propiedades del original y su imagen en otra memoria; y
    /// editar una no toca la otra.
    fn checks_copy(
        doc: &mut Document,
        before: &Document,
        source: LayerId,
        name_2: &str,
        cx: &Cx,
        seen: &Seen,
        states: &mut Vec<u64>,
    ) -> Result<(), TestCaseError> {
        let p = before
            .layers()
            .iter()
            .position(|l| l.id() == source)
            .expect("el origen existía");
        prop_assert_eq!(doc.layers().len(), before.layers().len() + 1);
        {
            let previous: Vec<_> = before.layers().iter().map(|l| summary(before, l)).collect();
            let rest: Vec<_> = doc
                .layers()
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != p + 1)
                .map(|(_, l)| summary(doc, l))
                .collect();
            prop_assert_eq!(
                previous,
                rest,
                "la copia no cambia al original ni a las demás"
            );
        }
        let (original, copy) = (&doc.layers()[p], &doc.layers()[p + 1]);
        prop_assert_eq!(copy.name(), name_2);
        prop_assert!(!seen.layers_2.contains(&copy.id()), "LayerId nuevo");
        prop_assert!(!seen.cels.contains(&copy.cel()), "ImageId nuevo");
        prop_assert_eq!(
            (copy.visible(), copy.opacity(), copy.blend(), copy.locked()),
            (
                original.visible(),
                original.opacity(),
                original.blend(),
                original.locked()
            )
        );
        let (cel_o, cel_c) = (original.cel(), copy.cel());
        let (io, ic) = (doc.image(cel_o), doc.image(cel_c));
        match (io, ic) {
            (None, None) => {
                prop_assert_eq!(doc.materialized_bytes(), before.materialized_bytes());
                return Ok(());
            }
            (Some(io), Some(ic)) => {
                prop_assert_eq!(io, ic);
                prop_assert_ne!(io.bytes().as_ptr(), ic.bytes().as_ptr());
                prop_assert_eq!(
                    doc.materialized_bytes(),
                    before.materialized_bytes() + doc.cel_bytes()
                );
            }
            other_2 => {
                return Err(TestCaseError::fail(format!(
                    "la celda vacía y la copia deben coincidir: {other_2:?}"
                )));
            }
        }

        // Editar la copia no toca al original; editar el original no toca a la copia.
        let actual = doc
            .image(cel_c)
            .expect("copia")
            .get_pixel(0, 0)
            .expect("(0, 0)");
        if let Some(pixel) = other_pixel(cx, actual) {
            let original_before = doc.image(cel_o).expect("original").clone();
            let editing = set_px(cel_c, 0, 0, pixel);
            if doc.apply(editing) == Ok(true) {
                states.push(doc.state_id());
            }
            prop_assert_eq!(doc.image(cel_o), Some(&original_before));
            let copy_before = doc.image(cel_c).expect("copia").clone();
            let actual = doc
                .image(cel_o)
                .expect("original")
                .get_pixel(0, 0)
                .expect("(0, 0)");
            if let Some(pixel) = other_pixel(cx, actual) {
                let editing = set_px(cel_o, 0, 0, pixel);
                if doc.apply(editing) == Ok(true) {
                    states.push(doc.state_id());
                }
                prop_assert_eq!(doc.image(cel_c), Some(&copy_before));
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 1: con cualquier secuencia de capas, bloqueos y pintura, los bytes
        /// materializados son la suma real y caben en el límite, una copia nunca cambia a su
        /// original, `DuplicateLayer` y `SetLayerLocked` son un solo paso, y deshacer hasta el
        /// principio y rehacer hasta el final dan los documentos (y `state_id`) de entonces.
        #[test]
        fn property_1_duplicate_and_lock_undo_exactly(
            plan in doc_plan(),
            commands in prop::collection::vec(order(), 0..=25),
        ) {
            let (mut doc, cx) = builds_doc(&plan);
            invariants(&doc)?;
            let initial = doc.clone();
            let mut states = vec![doc.state_id()];
            let mut seen = Seen::default();
            seen.records(&doc);

            for o in &commands {
                let before = doc.clone();
                let command = translates(&doc, o, &cx);
                let copied = match &command {
                    Command::DuplicateLayer { layer, name } => Some((*layer, name.clone())),
                    _ => None,
                };
                let a_step = copied.is_some() || matches!(command, Command::SetLayerLocked { .. });
                let result = doc.apply(command);
                invariants(&doc)?;
                match result {
                    Ok(true) => {
                        prop_assert!(!states.contains(&doc.state_id()), "state_id nuevo");
                        states.push(doc.state_id());
                        if a_step {
                            let mut c = doc.clone();
                            prop_assert_eq!(c.undo(), Ok(()));
                            prop_assert!(c == before, "un solo paso de historia");
                            prop_assert_eq!(c.state_id(), before.state_id());
                        }
                        if let Some((source, name_2)) = copied {
                            checks_copy(
                                &mut doc, &before, source, &name_2, &cx, &seen, &mut states,
                            )?;
                        }
                        seen.records(&doc);
                    }
                    Ok(false) | Err(_) => {
                        prop_assert!(doc == before, "sin cambios: {:?}", result);
                        prop_assert_eq!(doc.state_id(), before.state_id());
                        prop_assert_eq!(doc.history_bytes(), before.history_bytes());
                        prop_assert_eq!(doc.can_redo(), before.can_redo());
                    }
                }
            }

            let final_ = doc.clone();
            let steps = states.len() - 1;
            for i in (0..steps).rev() {
                prop_assert_eq!(doc.undo(), Ok(()), "deshacer {}", i);
                prop_assert_eq!(doc.state_id(), states[i], "state_id al deshacer a {}", i);
                invariants(&doc)?;
            }
            prop_assert!(doc == initial, "deshacer hasta el principio");
            prop_assert_eq!(doc.materialized_bytes(), initial.materialized_bytes());

            for (i, state) in states.iter().enumerate().take(steps + 1).skip(1) {
                prop_assert_eq!(doc.redo(), Ok(()), "rehacer {}", i);
                prop_assert_eq!(doc.state_id(), *state, "state_id al rehacer a {}", i);
                invariants(&doc)?;
            }
            prop_assert!(doc == final_, "rehacer hasta el final");
            prop_assert_eq!(doc.materialized_bytes(), final_.materialized_bytes());
            prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
        }
    }
}

// ---------------------------------------------------------------------------------------
// Unidad 3.8, cambios C-01 a C-04 (specs/hito-3-aseprite.md, «Cambios en px-core»):
// `LayerSpec`, `Document::from_layers`, `DocumentError::NoLayers` y `CelMismatch { layer }`.
// Fila 1 (tres pruebas) y propiedad P2 (256 casos). Fase de pruebas en rojo: no compilan hasta
// que existan. No se prueba `CelsTooLarge` aquí: pesaría GiB.
// ---------------------------------------------------------------------------------------

mod from_layers {
    use super::*;
    use px_core::{BlendMode, LayerSpec};

    fn spec(
        name_2: &str,
        visible: bool,
        opacity: u8,
        blend: BlendMode,
        locked: bool,
        image: Option<Image>,
    ) -> LayerSpec {
        LayerSpec {
            name: name_2.to_string(),
            visible,
            opacity,
            blend,
            locked,
            image,
        }
    }

    /// Capa visible, opaca, Normal y desbloqueada.
    fn simple(name_2: &str, image: Option<Image>) -> LayerSpec {
        spec(name_2, true, 255, BlendMode::Normal, false, image)
    }

    fn rgba_with(w: u32, h: u32, pixels_2: &[(u32, u32, Rgba)]) -> Image {
        let mut image_2 = img_rgba(w, h);
        for &(x, y, c) in pixels_2 {
            image_2.set_pixel(x, y, Pixel::Rgba(c)).expect("píxel");
        }
        image_2
    }

    /// (a) Un documento RGBA correcto: ids `0..n`, historia vacía, `state_id` 0, propiedades y
    /// celdas tal cual, y `==` con el construido a mano con comandos.
    #[test]
    fn row_01_a_rgba_ids_empty_history_state_id_0_and_equal_to_the_hand_built_one() {
        let a = rgba_with(4, 3, &[(0, 0, RED), (3, 2, GREEN)]);
        let c = rgba_with(4, 3, &[(1, 1, BLUE)]);
        let d = Document::from_layers(
            4,
            3,
            Mode::Rgba,
            None,
            vec![
                simple("fondo", Some(a.clone())),
                simple("vacía", None),
                spec(
                    "arriba",
                    false,
                    90,
                    BlendMode::Multiply,
                    true,
                    Some(c.clone()),
                ),
            ],
        )
        .expect("documento correcto");

        assert_eq!((d.width(), d.height(), d.mode()), (4, 3, Mode::Rgba));
        assert!(d.palette().is_none());
        assert_eq!(d.layers().len(), 3);
        for (i, layer_2) in d.layers().iter().enumerate() {
            assert_eq!(layer_2.id(), LayerId::from_raw(i as u32), "LayerId({i})");
            assert_eq!(layer_2.cel(), ImageId::from_raw(i as u32), "ImageId({i})");
        }
        let names: Vec<&str> = d.layers().iter().map(Layer::name).collect();
        assert_eq!(names, ["fondo", "vacía", "arriba"]);
        let third = &d.layers()[2];
        assert_eq!(
            (
                third.visible(),
                third.opacity(),
                third.blend(),
                third.locked()
            ),
            (false, 90, BlendMode::Multiply, true)
        );
        assert_eq!(d.image(d.layers()[0].cel()), Some(&a));
        assert_eq!(
            d.image(d.layers()[1].cel()),
            None,
            "la celda vacía no reserva memoria"
        );
        assert_eq!(d.image(d.layers()[2].cel()), Some(&c));
        assert_eq!(d.materialized_bytes(), 2 * 48);
        // Historia vacía y estado inicial.
        assert_eq!(d.state_id(), 0);
        assert!(!d.can_undo() && !d.can_redo());
        assert_eq!(d.history_bytes(), 0);

        // El mismo documento hecho con `Document::new` y comandos.
        let mut by_hand = Document::new(a);
        let (id0, id1, id2) = (
            LayerId::from_raw(0),
            LayerId::from_raw(1),
            LayerId::from_raw(2),
        );
        for order in [
            Command::RenameLayer {
                layer: id0,
                name: "fondo".to_string(),
            },
            Command::AddLayer {
                name: "vacía".to_string(),
                at: 1,
            },
            Command::AddLayer {
                name: "arriba".to_string(),
                at: 2,
            },
            set_px(ImageId::from_raw(2), 1, 1, Pixel::Rgba(BLUE)),
            Command::SetLayerOpacity {
                layer: id2,
                opacity: 90,
            },
            Command::SetLayerBlend {
                layer: id2,
                blend: BlendMode::Multiply,
            },
            Command::SetLayerVisible {
                layer: id2,
                visible: false,
            },
            Command::SetLayerLocked {
                layer: id2,
                locked: true,
            },
        ] {
            assert!(by_hand.apply(order).is_ok());
        }
        assert_eq!(by_hand.layer(id1).map(Layer::name), Some("vacía"));
        assert!(by_hand == d, "el construido a mano es igual");
        assert!(d == by_hand);
    }

    /// (b) Indexado con paleta, y los límites exactos se aceptan: 256 capas, nombre de 255 bytes.
    #[test]
    fn row_01_b_indexed_and_exact_limits_are_accepted() {
        let colors_2 = [RED, GREEN, BLUE];
        let palette_2 = |t: Option<u8>| {
            let mut p = Palette::from_colors(colors_2).expect("paleta");
            p.set_transparent_index(t).expect("índice transparente");
            p
        };
        let p = palette_2(Some(0));
        let image_2 = Image::filled_indexed(4, 3, p.clone(), 2).expect("imagen");
        let d = Document::from_layers(
            4,
            3,
            Mode::Indexed,
            Some(p.clone()),
            vec![simple("a", Some(image_2.clone())), simple("b", None)],
        )
        .expect("indexado correcto");
        assert_eq!(d.mode(), Mode::Indexed);
        assert_eq!(d.palette(), Some(&p));
        assert_eq!(d.image(d.layers()[0].cel()), Some(&image_2));
        assert_eq!(d.image(d.layers()[1].cel()), None);
        assert_eq!(
            (d.state_id(), d.can_undo(), d.history_bytes()),
            (0, false, 0)
        );

        // 256 capas de 1 × 1 y un nombre de 255 bytes.
        let name_2 = "ñ".repeat(127) + "a";
        assert_eq!(name_2.len(), 255);
        let mut layers_2: Vec<LayerSpec> =
            (0..255).map(|i| simple(&format!("c{i}"), None)).collect();
        layers_2.push(simple(&name_2, Some(img_rgba(1, 1))));
        let d = Document::from_layers(1, 1, Mode::Rgba, None, layers_2).expect("256 capas");
        assert_eq!(d.layers().len(), 256);
        assert_eq!(d.layers()[255].name(), name_2);
        assert_eq!(d.layers()[255].id(), LayerId::from_raw(255));
    }

    /// (c) Cada defecto aislado da su error y no se construye ningún documento.
    #[test]
    fn row_01_c_each_defect_gives_its_error_and_no_document() {
        let p3 = |t: Option<u8>| {
            let mut p = Palette::from_colors([RED, GREEN, BLUE]).expect("paleta");
            p.set_transparent_index(t).expect("índice transparente");
            p
        };
        let indexed = |palette_2: &Palette| {
            Image::filled_indexed(4, 3, palette_2.clone(), 1).expect("imagen")
        };
        let other = Palette::from_colors([RED, GREEN, WHITE]).expect("otra paleta");
        let ok_rgba = || simple("ok", Some(img_rgba(4, 3)));

        #[allow(clippy::type_complexity)]
        let cases: Vec<(&str, Result<Document, DocumentError>, DocumentError)> = vec![
            (
                "0 capas",
                Document::from_layers(4, 3, Mode::Rgba, None, vec![]),
                DocumentError::NoLayers,
            ),
            (
                "257 capas",
                Document::from_layers(
                    1,
                    1,
                    Mode::Rgba,
                    None,
                    (0..257).map(|i| simple(&format!("c{i}"), None)).collect(),
                ),
                DocumentError::TooManyLayers { limit: 256 },
            ),
            (
                "nombre de 256 bytes",
                Document::from_layers(4, 3, Mode::Rgba, None, vec![simple(&"a".repeat(256), None)]),
                DocumentError::LayerName {
                    len: 256,
                    limit: 255,
                },
            ),
            (
                "celda de otro tamaño",
                Document::from_layers(
                    4,
                    3,
                    Mode::Rgba,
                    None,
                    vec![ok_rgba(), simple("x", Some(img_rgba(3, 3)))],
                ),
                DocumentError::CelMismatch { layer: 1 },
            ),
            (
                "celda de otro modo",
                Document::from_layers(
                    4,
                    3,
                    Mode::Rgba,
                    None,
                    vec![simple(
                        "x",
                        Some(Image::new(4, 3, Mode::Gray).expect("gris")),
                    )],
                ),
                DocumentError::CelMismatch { layer: 0 },
            ),
            (
                "celda con otra paleta",
                Document::from_layers(
                    4,
                    3,
                    Mode::Indexed,
                    Some(p3(Some(0))),
                    vec![simple("x", Some(indexed(&other)))],
                ),
                DocumentError::CelMismatch { layer: 0 },
            ),
            (
                "indexado con opacidad distinta de 255",
                Document::from_layers(
                    4,
                    3,
                    Mode::Indexed,
                    Some(p3(Some(0))),
                    vec![spec(
                        "x",
                        true,
                        128,
                        BlendMode::Normal,
                        false,
                        Some(indexed(&p3(Some(0)))),
                    )],
                ),
                DocumentError::IndexedOpacity,
            ),
            (
                "indexado con mezcla distinta de Normal",
                Document::from_layers(
                    4,
                    3,
                    Mode::Indexed,
                    Some(p3(Some(0))),
                    vec![spec(
                        "x",
                        true,
                        255,
                        BlendMode::Multiply,
                        false,
                        Some(indexed(&p3(Some(0)))),
                    )],
                ),
                DocumentError::IndexedBlend,
            ),
            (
                "celda vacía en indexado sin índice transparente",
                Document::from_layers(4, 3, Mode::Indexed, Some(p3(None)), vec![simple("x", None)]),
                DocumentError::NoTransparency {
                    mode: Mode::Indexed,
                },
            ),
            (
                "celda vacía en grises (sin valor transparente)",
                Document::from_layers(4, 3, Mode::Gray, None, vec![simple("x", None)]),
                DocumentError::NoTransparency { mode: Mode::Gray },
            ),
        ];
        for (name_2, result, expected_2) in cases {
            assert_eq!(result, Err(expected_2), "{name_2}");
        }
    }

    /// Unidad L1, fila 6: un lienzo imposible (ancho o alto 0, más de `Image::MAX_BYTES`) o una
    /// paleta que no corresponde al modo es `InvalidCanvas`, aunque las capas sean válidas; una
    /// celda que no encaja sigue siendo `CelMismatch`, y los dos errores se leen distinto.
    #[test]
    fn row_06_impossible_canvas_is_invalid_canvas_and_non_fitting_cel_is_cel_mismatch() {
        let palette_2 = Palette::from_colors([RED, GREEN, BLUE]).expect("paleta");
        let cases: Vec<(&str, Result<Document, DocumentError>)> = vec![
            (
                "ancho 0",
                Document::from_layers(0, 3, Mode::Rgba, None, vec![simple("a", None)]),
            ),
            (
                "alto 0",
                Document::from_layers(4, 0, Mode::Rgba, None, vec![simple("a", None)]),
            ),
            (
                "más de Image::MAX_BYTES",
                Document::from_layers(100_000, 100_000, Mode::Rgba, None, vec![simple("a", None)]),
            ),
            (
                "paleta en un modo que no es indexado",
                Document::from_layers(4, 3, Mode::Rgba, Some(palette_2), vec![simple("a", None)]),
            ),
            (
                "indexado sin paleta",
                Document::from_layers(4, 3, Mode::Indexed, None, vec![simple("a", None)]),
            ),
        ];
        for (name_2, result) in cases {
            assert_eq!(result.err(), Some(DocumentError::InvalidCanvas), "{name_2}");
        }
        let cel = Document::from_layers(
            4,
            3,
            Mode::Rgba,
            None,
            vec![simple("a", Some(img_rgba(3, 3)))],
        )
        .expect_err("la celda no encaja");
        assert_eq!(cel, DocumentError::CelMismatch { layer: 0 });
        assert_ne!(
            DocumentError::InvalidCanvas.to_string(),
            cel.to_string(),
            "los dos mensajes son distintos"
        );
    }

    // -----------------------------------------------------------------------------------
    // P2: `from_layers` no construye documentos inválidos.
    // -----------------------------------------------------------------------------------

    /// Una celda de 1 a 4 píxeles de lado y de cualquiera de los cuatro modos; `variant` decide
    /// cuánto se parece a lo que pide el lienzo.
    fn cel_of(variant: u8, w: u32, h: u32, mode_2: Mode, palette_2: Option<&Palette>) -> Image {
        let other = Palette::from_colors([BLACK, WHITE]).expect("otra paleta");
        let result = match variant {
            // Coincide en todo (si se puede).
            0 => match (mode_2, palette_2) {
                (Mode::Indexed, Some(p)) => Image::new_indexed(w, h, p.clone()),
                (Mode::Indexed, None) => Image::new(w, h, Mode::Rgba),
                (m, _) => Image::new(w, h, m),
            },
            // Otro tamaño.
            1 => Image::new(w + 1, h, Mode::Rgba),
            // Otro modo.
            2 => Image::new(
                w,
                h,
                if mode_2 == Mode::Gray {
                    Mode::GrayAlpha
                } else {
                    Mode::Gray
                },
            ),
            // Otra paleta.
            _ => Image::new_indexed(w, h, other),
        };
        result.expect("celda de la prueba")
    }

    /// Las invariantes de un documento válido (spec, P2).
    fn invariants_p_2(d: &Document) -> Result<(), TestCaseError> {
        let n = d.layers().len();
        prop_assert!((1..=Document::MAX_LAYERS).contains(&n), "{} capas", n);
        for (i, layer_2) in d.layers().iter().enumerate() {
            prop_assert!(
                layer_2.name().len() <= Document::MAX_LAYER_NAME_BYTES,
                "nombre de {}",
                i
            );
            prop_assert_eq!(layer_2.id(), LayerId::from_raw(i as u32));
            prop_assert_eq!(layer_2.cel(), ImageId::from_raw(i as u32));
            if d.mode() == Mode::Indexed {
                prop_assert_eq!(layer_2.opacity(), 255);
                prop_assert_eq!(layer_2.blend(), BlendMode::Normal);
            }
            if let Some(image_2) = d.image(layer_2.cel()) {
                prop_assert_eq!(
                    (image_2.width(), image_2.height(), image_2.mode()),
                    (d.width(), d.height(), d.mode())
                );
                prop_assert_eq!(image_2.palette(), d.palette());
            }
        }
        prop_assert!(d.materialized_bytes() <= d.cells_limit());
        prop_assert_eq!(d.state_id(), 0);
        prop_assert!(!d.can_undo() && !d.can_redo());
        prop_assert_eq!(d.history_bytes(), 0);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// P2. Para listas arbitrarias de `LayerSpec` (tamaños, modos y paletas incoherentes,
        /// opacidades, mezclas, nombres largos, 0 a 300 capas) y cualquier lienzo: o `Err`, o un
        /// documento que cumple todas las invariantes.
        #[test]
        fn property_2_from_layers_does_not_build_invalid_documents(
            (w, h) in (1u32..=4, 1u32..=4),
            mode_2 in prop::sample::select(vec![Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha]),
            with_palette in 0u8..4,
            transparent in prop::option::of(0u8..2),
            amount in prop_oneof![8 => 0usize..=5, 1 => Just(256usize), 1 => Just(257usize), 1 => Just(300usize)],
            seeds in prop::collection::vec(
                (
                    prop::sample::select(vec![0usize, 1, 10, 255, 256, 300]),
                    any::<u8>(),
                    0usize..8,
                    (any::<bool>(), any::<bool>()),
                    prop_oneof![2 => Just(None), 5 => (0u8..4).prop_map(Some)],
                ),
                300,
            ),
        ) {
            // `with_palette`: 0 no hay paleta, 1 y 2 una de dos colores, 3 otra de un color.
            let palette_2 = match with_palette {
                0 => None,
                3 => Some(Palette::from_colors([RED]).expect("paleta")),
                _ => {
                    let mut p = Palette::from_colors([RED, GREEN]).expect("paleta");
                    p.set_transparent_index(transparent).expect("transparente");
                    Some(p)
                }
            };
            let layers_2: Vec<LayerSpec> = seeds
                .iter()
                .take(amount)
                .map(|(length, opacity_2, blend_2, (visible, locked_2), variant)| {
                    // Las muy numerosas llevan celdas vacías o de 1 × 1 para no pesar.
                    let image_2 = variant.map(|v| cel_of(v, w, h, mode_2, palette_2.as_ref()));
                    spec(&"a".repeat(*length), *visible, *opacity_2, BlendMode::ALL[*blend_2], *locked_2, image_2)
                })
                .collect();
            if let Ok(d) = Document::from_layers(w, h, mode_2, palette_2, layers_2) {
                invariants_p_2(&d)?;
            }
        }
    }
}
