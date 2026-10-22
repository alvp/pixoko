//! Tests del modo de mezcla de capa de `px-core` (specs/hito-3-mezcla.md, unidad 3.2, cambios
//! `P-01` a `P-08`): `BlendMode`, `Layer::blend`, `Command::SetLayerBlend` y
//! `DocumentError::IndexedBlend`.
//!
//! Es la fila 12 de «Casos», repartida en `fila_12_a`, `fila_12_b`... (un caso por prueba), más
//! una prueba de los valores del enum y del mensaje (`P-01`, `P-02` y `P-05`). La composición
//! (`compose`) vive en `px-raster` y no se toca aquí.
//!
//! Este fichero es la fase de pruebas en rojo: `BlendMode`, `SetLayerBlend` e `IndexedBlend` no
//! existen todavía, así que no compila hasta la lógica A de la spec.

use std::collections::HashSet;
use std::error::Error;

use px_core::{
    Allowed, BlendMode, Command, Document, DocumentError, Image, LayerId, Mode, Palette, Pixel,
    PixelEdit, Rgba,
};

// ---------------------------------------------------------------------------------------
// Apoyo.
// ---------------------------------------------------------------------------------------

const C1: Rgba = Rgba::new(40, 50, 60, 255);
const C2: Rgba = Rgba::new(70, 80, 90, 255);

/// Documento RGBA de 4 x 3 con dos colores opacos y el resto transparente.
fn doc_d() -> Document {
    let mut img = Image::new(4, 3, Mode::Rgba).expect("imagen RGBA");
    img.set_pixel(0, 0, Pixel::Rgba(C1)).expect("píxel");
    img.set_pixel(1, 0, Pixel::Rgba(C2)).expect("píxel");
    Document::new(img)
}

fn doc_indexed() -> Document {
    let mut palette_2 = Palette::from_colors([
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 255, 0, 255),
        Rgba::new(0, 0, 255, 255),
    ])
    .expect("paleta");
    palette_2
        .set_transparent_index(Some(0))
        .expect("índice transparente");
    Document::new(Image::new_indexed(4, 3, palette_2).expect("imagen indexada"))
}

fn blend_2(layer: LayerId, blend: BlendMode) -> Command {
    Command::SetLayerBlend { layer, blend }
}

fn adds(doc: &mut Document, name_2: &str, at: usize) -> LayerId {
    let cmd = Command::AddLayer {
        name: name_2.to_string(),
        at,
    };
    assert_eq!(doc.apply(cmd), Ok(true), "añadir {name_2}");
    doc.layers()[at].id()
}

fn mode_of(doc: &Document, layer: LayerId) -> BlendMode {
    doc.layer(layer).expect("la capa existe").blend()
}

/// Todo lo observable que un comando fallido o sin cambios no puede tocar.
struct Snapshot {
    doc: Document,
    can_undo_2: bool,
    can_redo_2: bool,
    history: usize,
    state: u64,
}

impl Snapshot {
    fn of(doc: &Document) -> Snapshot {
        Snapshot {
            doc: doc.clone(),
            can_undo_2: doc.can_undo(),
            can_redo_2: doc.can_redo(),
            history: doc.history_bytes(),
            state: doc.state_id(),
        }
    }

    fn assert_equal(&self, doc: &Document, context: &str) {
        assert_eq!(*doc, self.doc, "{context}: el documento cambió");
        assert_eq!(doc.can_undo(), self.can_undo_2, "{context}: can_undo");
        assert_eq!(doc.can_redo(), self.can_redo_2, "{context}: can_redo");
        assert_eq!(doc.history_bytes(), self.history, "{context}: history");
        assert_eq!(doc.state_id(), self.state, "{context}: state_id");
    }
}

/// Aplica `cmd`, comprueba el resultado y que nada observable cambió.
fn without_effect(doc: &mut Document, cmd: Command, expected: Result<bool, DocumentError>) {
    let snapshot = Snapshot::of(doc);
    let context = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), expected, "{context}");
    snapshot.assert_equal(doc, &context);
}

// ---------------------------------------------------------------------------------------
// P-01, P-02 y P-05: el tipo, el valor por defecto y el mensaje.
// ---------------------------------------------------------------------------------------

#[test]
fn p01_blend_mode_values_default_and_message() {
    // `Default` es Normal y `ALL` lleva los ocho, sin repetir y con Normal el primero.
    assert_eq!(BlendMode::default(), BlendMode::Normal);
    assert_eq!(BlendMode::ALL.len(), 8);
    assert_eq!(BlendMode::ALL[0], BlendMode::Normal);
    let different: HashSet<BlendMode> = BlendMode::ALL.into_iter().collect();
    assert_eq!(different.len(), 8, "los ocho modos son distintos entre sí");
    for mode_2 in [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::Difference,
    ] {
        assert!(BlendMode::ALL.contains(&mode_2), "{mode_2:?} está en ALL");
        // `Debug` y `Clone`/`Copy`.
        let copy = mode_2;
        assert_eq!(copy, mode_2);
        assert!(!format!("{mode_2:?}").is_empty());
    }

    // Toda capa nueva (la del documento y las de `AddLayer`) es Normal.
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    assert_eq!(d.layers()[0].blend(), BlendMode::Normal);
    assert_eq!(mode_of(&d, b), BlendMode::Normal);

    // `IndexedBlend` tiene un mensaje en español, distinto del de `IndexedOpacity`.
    let message = DocumentError::IndexedBlend.to_string();
    assert!(!message.is_empty());
    assert_ne!(message, DocumentError::IndexedOpacity.to_string());
    let _: &dyn Error = &DocumentError::IndexedBlend;
}

// ---------------------------------------------------------------------------------------
// Fila 12: SetLayerBlend.
// ---------------------------------------------------------------------------------------

#[test]
fn row_12_a_mode_change_is_one_step() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let state = d.state_id();

    assert_eq!(d.apply(blend_2(a, BlendMode::Multiply)), Ok(true));
    assert_eq!(mode_of(&d, a), BlendMode::Multiply);
    assert_ne!(d.state_id(), state);
    assert!(d.can_undo());

    // Un solo paso: tras deshacer una vez no queda nada que deshacer.
    d.undo().expect("deshacer");
    assert_eq!(mode_of(&d, a), BlendMode::Normal);
    assert!(!d.can_undo(), "el cambio de modo es un único paso");
    assert!(d.can_redo());
}

#[test]
fn row_12_b_same_mode_is_ok_false_without_a_step() {
    let mut d = doc_d();
    let a = d.layers()[0].id();

    // Normal sobre una capa Normal.
    without_effect(&mut d, blend_2(a, BlendMode::Normal), Ok(false));
    assert!(!d.can_undo());

    // Y el mismo valor tras un cambio.
    assert_eq!(d.apply(blend_2(a, BlendMode::Screen)), Ok(true));
    without_effect(&mut d, blend_2(a, BlendMode::Screen), Ok(false));
    d.undo().expect("deshacer");
    assert_eq!(mode_of(&d, a), BlendMode::Normal);
    assert!(!d.can_undo(), "el segundo Screen no dejó paso");
}

#[test]
fn row_12_c_unknown_layer() {
    let mut d = doc_d();
    let false_value = LayerId::from_raw(999);
    without_effect(
        &mut d,
        blend_2(false_value, BlendMode::Multiply),
        Err(DocumentError::UnknownLayer(false_value)),
    );

    // También en indexado: `UnknownLayer` va antes que `IndexedBlend`.
    let mut indexed = doc_indexed();
    without_effect(
        &mut indexed,
        blend_2(false_value, BlendMode::Multiply),
        Err(DocumentError::UnknownLayer(false_value)),
    );
}

#[test]
fn row_12_d_in_indexed_only_normal_is_valid() {
    let mut d = doc_indexed();
    let a = d.layers()[0].id();

    for mode_2 in BlendMode::ALL {
        if mode_2 == BlendMode::Normal {
            continue;
        }
        without_effect(&mut d, blend_2(a, mode_2), Err(DocumentError::IndexedBlend));
    }
    without_effect(&mut d, blend_2(a, BlendMode::Normal), Ok(false));
    assert_eq!(mode_of(&d, a), BlendMode::Normal);
}

#[test]
fn row_12_e_converting_to_indexed_with_a_multiply_layer_fails_intact() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let cel = d.layers()[0].cel();
    let convert = Command::ConvertMode {
        image: cel,
        target: Mode::Indexed,
        allowed: Allowed::NOTHING,
    };

    assert_eq!(d.apply(blend_2(a, BlendMode::Multiply)), Ok(true));
    without_effect(&mut d, convert.clone(), Err(DocumentError::IndexedBlend));
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert_eq!(mode_of(&d, a), BlendMode::Multiply);

    // Con la capa otra vez en Normal, la misma conversión sí se hace (el modo era el motivo).
    assert_eq!(d.apply(blend_2(a, BlendMode::Normal)), Ok(true));
    assert_eq!(d.apply(convert), Ok(true));
    assert_eq!(d.mode(), Mode::Indexed);
}

#[test]
fn row_12_f_undo_and_redo_are_exact() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let s0 = d.state_id();
    let d0 = d.clone();

    assert_eq!(d.apply(blend_2(a, BlendMode::Multiply)), Ok(true));
    let s1 = d.state_id();
    let d1 = d.clone();
    assert_eq!(d.apply(blend_2(a, BlendMode::Difference)), Ok(true));
    let s2 = d.state_id();
    let d2 = d.clone();

    d.undo().expect("deshacer 1");
    assert_eq!(mode_of(&d, a), BlendMode::Multiply);
    assert_eq!((d.state_id(), &d), (s1, &d1));
    d.undo().expect("deshacer 2");
    assert_eq!(mode_of(&d, a), BlendMode::Normal);
    assert_eq!((d.state_id(), &d), (s0, &d0));
    assert!(!d.can_undo());

    d.redo().expect("rehacer 1");
    assert_eq!(mode_of(&d, a), BlendMode::Multiply);
    assert_eq!((d.state_id(), &d), (s1, &d1));
    d.redo().expect("rehacer 2");
    assert_eq!(mode_of(&d, a), BlendMode::Difference);
    assert_eq!((d.state_id(), &d), (s2, &d2));
    assert!(!d.can_redo());
}

#[test]
fn row_12_g_equality_distinguishes_the_mode() {
    let d = doc_d();
    let a = d.layers()[0].id();
    let mut other = d.clone();
    assert_eq!(d, other);

    assert_eq!(other.apply(blend_2(a, BlendMode::Add)), Ok(true));
    assert_ne!(
        d, other,
        "dos documentos con distinto modo de capa no son iguales"
    );
    assert_ne!(
        d.layer(a),
        other.layer(a),
        "`Layer` también compara el modo"
    );

    // Dos modos distintos entre sí tampoco.
    let mut third = d.clone();
    assert_eq!(third.apply(blend_2(a, BlendMode::Lighten)), Ok(true));
    assert_ne!(other, third);

    // Deshacer devuelve la igualdad.
    other.undo().expect("deshacer");
    assert_eq!(d, other);
}

#[test]
fn row_12_h_new_state_id_only_with_ok_true_and_never_reused() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let s0 = d.state_id();

    // Sin cambios y con error: el mismo identificador (lo comprueba `without_effect`).
    without_effect(&mut d, blend_2(a, BlendMode::Normal), Ok(false));
    without_effect(
        &mut d,
        blend_2(LayerId::from_raw(999), BlendMode::Add),
        Err(DocumentError::UnknownLayer(LayerId::from_raw(999))),
    );
    assert_eq!(d.state_id(), s0);

    // Con cambio: uno nuevo; deshacer vuelve al anterior y un cambio distinto no reutiliza el
    // del paso deshecho.
    assert_eq!(d.apply(blend_2(a, BlendMode::Multiply)), Ok(true));
    let s1 = d.state_id();
    assert_ne!(s1, s0);
    d.undo().expect("deshacer");
    assert_eq!(d.state_id(), s0);
    assert_eq!(d.apply(blend_2(a, BlendMode::Screen)), Ok(true));
    let s2 = d.state_id();
    assert_ne!(s2, s0);
    assert_ne!(s2, s1, "un identificador de estado nunca se reutiliza");
}

#[test]
fn row_12_i_inside_a_batch_it_is_not_batchable() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let cel = d.layers()[0].cel();

    without_effect(
        &mut d,
        Command::Batch(vec![blend_2(a, BlendMode::Multiply)]),
        Err(DocumentError::InBatch {
            index: 0,
            source: Box::new(DocumentError::NotBatchable),
        }),
    );

    // Detrás de una escritura válida, que se revierte con el resto del lote.
    let writing = Command::SetPixels {
        image: cel,
        pixels: vec![PixelEdit {
            x: 3,
            y: 2,
            pixel: Pixel::Rgba(C2),
        }],
    };
    without_effect(
        &mut d,
        Command::Batch(vec![writing, blend_2(a, BlendMode::Multiply)]),
        Err(DocumentError::InBatch {
            index: 1,
            source: Box::new(DocumentError::NotBatchable),
        }),
    );
    assert_eq!(mode_of(&d, a), BlendMode::Normal);
}

#[test]
fn row_12_j_the_mode_is_per_layer_and_survives_moving_the_layer() {
    // Una prueba corta de que `SetLayerBlend` toca solo su capa y que el modo viaja con ella.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    assert_eq!(d.apply(blend_2(b, BlendMode::Overlay)), Ok(true));
    assert_eq!(mode_of(&d, a), BlendMode::Normal);
    assert_eq!(mode_of(&d, b), BlendMode::Overlay);

    assert_eq!(d.apply(Command::MoveLayer { layer: b, to: 0 }), Ok(true));
    assert_eq!(mode_of(&d, b), BlendMode::Overlay);
    assert_eq!(d.layers()[0].id(), b);
    assert_eq!(d.layers()[0].blend(), BlendMode::Overlay);
}
