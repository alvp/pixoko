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

fn doc_indexado() -> Document {
    let mut paleta = Palette::from_colors([
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 255, 0, 255),
        Rgba::new(0, 0, 255, 255),
    ])
    .expect("paleta");
    paleta
        .set_transparent_index(Some(0))
        .expect("índice transparente");
    Document::new(Image::new_indexed(4, 3, paleta).expect("imagen indexada"))
}

fn mezcla(layer: LayerId, blend: BlendMode) -> Command {
    Command::SetLayerBlend { layer, blend }
}

fn anade(doc: &mut Document, nombre: &str, at: usize) -> LayerId {
    let cmd = Command::AddLayer {
        name: nombre.to_string(),
        at,
    };
    assert_eq!(doc.apply(cmd), Ok(true), "añadir {nombre}");
    doc.layers()[at].id()
}

fn modo_de(doc: &Document, capa: LayerId) -> BlendMode {
    doc.layer(capa).expect("la capa existe").blend()
}

/// Todo lo observable que un comando fallido o sin cambios no puede tocar.
struct Foto {
    doc: Document,
    puede_deshacer: bool,
    puede_rehacer: bool,
    historia: usize,
    estado: u64,
}

impl Foto {
    fn de(doc: &Document) -> Foto {
        Foto {
            doc: doc.clone(),
            puede_deshacer: doc.can_undo(),
            puede_rehacer: doc.can_redo(),
            historia: doc.history_bytes(),
            estado: doc.state_id(),
        }
    }

    fn assert_igual(&self, doc: &Document, contexto: &str) {
        assert_eq!(*doc, self.doc, "{contexto}: el documento cambió");
        assert_eq!(doc.can_undo(), self.puede_deshacer, "{contexto}: can_undo");
        assert_eq!(doc.can_redo(), self.puede_rehacer, "{contexto}: can_redo");
        assert_eq!(doc.history_bytes(), self.historia, "{contexto}: history");
        assert_eq!(doc.state_id(), self.estado, "{contexto}: state_id");
    }
}

/// Aplica `cmd`, comprueba el resultado y que nada observable cambió.
fn sin_efecto(doc: &mut Document, cmd: Command, esperado: Result<bool, DocumentError>) {
    let foto = Foto::de(doc);
    let contexto = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), esperado, "{contexto}");
    foto.assert_igual(doc, &contexto);
}

// ---------------------------------------------------------------------------------------
// P-01, P-02 y P-05: el tipo, el valor por defecto y el mensaje.
// ---------------------------------------------------------------------------------------

#[test]
fn p01_blend_mode_valores_defecto_y_mensaje() {
    // `Default` es Normal y `ALL` lleva los ocho, sin repetir y con Normal el primero.
    assert_eq!(BlendMode::default(), BlendMode::Normal);
    assert_eq!(BlendMode::ALL.len(), 8);
    assert_eq!(BlendMode::ALL[0], BlendMode::Normal);
    let distintos: HashSet<BlendMode> = BlendMode::ALL.into_iter().collect();
    assert_eq!(distintos.len(), 8, "los ocho modos son distintos entre sí");
    for modo in [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::Difference,
    ] {
        assert!(BlendMode::ALL.contains(&modo), "{modo:?} está en ALL");
        // `Debug` y `Clone`/`Copy`.
        let copia = modo;
        assert_eq!(copia, modo);
        assert!(!format!("{modo:?}").is_empty());
    }

    // Toda capa nueva (la del documento y las de `AddLayer`) es Normal.
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    assert_eq!(d.layers()[0].blend(), BlendMode::Normal);
    assert_eq!(modo_de(&d, b), BlendMode::Normal);

    // `IndexedBlend` tiene un mensaje en español, distinto del de `IndexedOpacity`.
    let mensaje = DocumentError::IndexedBlend.to_string();
    assert!(!mensaje.is_empty());
    assert_ne!(mensaje, DocumentError::IndexedOpacity.to_string());
    let _: &dyn Error = &DocumentError::IndexedBlend;
}

// ---------------------------------------------------------------------------------------
// Fila 12: SetLayerBlend.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_12_a_cambio_de_modo_es_un_solo_paso() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let estado = d.state_id();

    assert_eq!(d.apply(mezcla(a, BlendMode::Multiply)), Ok(true));
    assert_eq!(modo_de(&d, a), BlendMode::Multiply);
    assert_ne!(d.state_id(), estado);
    assert!(d.can_undo());

    // Un solo paso: tras deshacer una vez no queda nada que deshacer.
    d.undo().expect("deshacer");
    assert_eq!(modo_de(&d, a), BlendMode::Normal);
    assert!(!d.can_undo(), "el cambio de modo es un único paso");
    assert!(d.can_redo());
}

#[test]
fn fila_12_b_mismo_modo_es_ok_false_sin_paso() {
    let mut d = doc_d();
    let a = d.layers()[0].id();

    // Normal sobre una capa Normal.
    sin_efecto(&mut d, mezcla(a, BlendMode::Normal), Ok(false));
    assert!(!d.can_undo());

    // Y el mismo valor tras un cambio.
    assert_eq!(d.apply(mezcla(a, BlendMode::Screen)), Ok(true));
    sin_efecto(&mut d, mezcla(a, BlendMode::Screen), Ok(false));
    d.undo().expect("deshacer");
    assert_eq!(modo_de(&d, a), BlendMode::Normal);
    assert!(!d.can_undo(), "el segundo Screen no dejó paso");
}

#[test]
fn fila_12_c_capa_desconocida() {
    let mut d = doc_d();
    let falsa = LayerId::from_raw(999);
    sin_efecto(
        &mut d,
        mezcla(falsa, BlendMode::Multiply),
        Err(DocumentError::UnknownLayer(falsa)),
    );

    // También en indexado: `UnknownLayer` va antes que `IndexedBlend`.
    let mut indexado = doc_indexado();
    sin_efecto(
        &mut indexado,
        mezcla(falsa, BlendMode::Multiply),
        Err(DocumentError::UnknownLayer(falsa)),
    );
}

#[test]
fn fila_12_d_en_indexado_solo_vale_normal() {
    let mut d = doc_indexado();
    let a = d.layers()[0].id();

    for modo in BlendMode::ALL {
        if modo == BlendMode::Normal {
            continue;
        }
        sin_efecto(&mut d, mezcla(a, modo), Err(DocumentError::IndexedBlend));
    }
    sin_efecto(&mut d, mezcla(a, BlendMode::Normal), Ok(false));
    assert_eq!(modo_de(&d, a), BlendMode::Normal);
}

#[test]
fn fila_12_e_convertir_a_indexado_con_una_capa_multiplicar_falla_intacto() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let cel = d.layers()[0].cel();
    let convertir = Command::ConvertMode {
        image: cel,
        target: Mode::Indexed,
        allowed: Allowed::NOTHING,
    };

    assert_eq!(d.apply(mezcla(a, BlendMode::Multiply)), Ok(true));
    sin_efecto(&mut d, convertir.clone(), Err(DocumentError::IndexedBlend));
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert_eq!(modo_de(&d, a), BlendMode::Multiply);

    // Con la capa otra vez en Normal, la misma conversión sí se hace (el modo era el motivo).
    assert_eq!(d.apply(mezcla(a, BlendMode::Normal)), Ok(true));
    assert_eq!(d.apply(convertir), Ok(true));
    assert_eq!(d.mode(), Mode::Indexed);
}

#[test]
fn fila_12_f_deshacer_y_rehacer_son_exactos() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let s0 = d.state_id();
    let d0 = d.clone();

    assert_eq!(d.apply(mezcla(a, BlendMode::Multiply)), Ok(true));
    let s1 = d.state_id();
    let d1 = d.clone();
    assert_eq!(d.apply(mezcla(a, BlendMode::Difference)), Ok(true));
    let s2 = d.state_id();
    let d2 = d.clone();

    d.undo().expect("deshacer 1");
    assert_eq!(modo_de(&d, a), BlendMode::Multiply);
    assert_eq!((d.state_id(), &d), (s1, &d1));
    d.undo().expect("deshacer 2");
    assert_eq!(modo_de(&d, a), BlendMode::Normal);
    assert_eq!((d.state_id(), &d), (s0, &d0));
    assert!(!d.can_undo());

    d.redo().expect("rehacer 1");
    assert_eq!(modo_de(&d, a), BlendMode::Multiply);
    assert_eq!((d.state_id(), &d), (s1, &d1));
    d.redo().expect("rehacer 2");
    assert_eq!(modo_de(&d, a), BlendMode::Difference);
    assert_eq!((d.state_id(), &d), (s2, &d2));
    assert!(!d.can_redo());
}

#[test]
fn fila_12_g_la_igualdad_distingue_el_modo() {
    let d = doc_d();
    let a = d.layers()[0].id();
    let mut otro = d.clone();
    assert_eq!(d, otro);

    assert_eq!(otro.apply(mezcla(a, BlendMode::Add)), Ok(true));
    assert_ne!(
        d, otro,
        "dos documentos con distinto modo de capa no son iguales"
    );
    assert_ne!(d.layer(a), otro.layer(a), "`Layer` también compara el modo");

    // Dos modos distintos entre sí tampoco.
    let mut tercero = d.clone();
    assert_eq!(tercero.apply(mezcla(a, BlendMode::Lighten)), Ok(true));
    assert_ne!(otro, tercero);

    // Deshacer devuelve la igualdad.
    otro.undo().expect("deshacer");
    assert_eq!(d, otro);
}

#[test]
fn fila_12_h_state_id_nuevo_solo_con_ok_true_y_nunca_reutilizado() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let s0 = d.state_id();

    // Sin cambios y con error: el mismo identificador (lo comprueba `sin_efecto`).
    sin_efecto(&mut d, mezcla(a, BlendMode::Normal), Ok(false));
    sin_efecto(
        &mut d,
        mezcla(LayerId::from_raw(999), BlendMode::Add),
        Err(DocumentError::UnknownLayer(LayerId::from_raw(999))),
    );
    assert_eq!(d.state_id(), s0);

    // Con cambio: uno nuevo; deshacer vuelve al anterior y un cambio distinto no reutiliza el
    // del paso deshecho.
    assert_eq!(d.apply(mezcla(a, BlendMode::Multiply)), Ok(true));
    let s1 = d.state_id();
    assert_ne!(s1, s0);
    d.undo().expect("deshacer");
    assert_eq!(d.state_id(), s0);
    assert_eq!(d.apply(mezcla(a, BlendMode::Screen)), Ok(true));
    let s2 = d.state_id();
    assert_ne!(s2, s0);
    assert_ne!(s2, s1, "un identificador de estado nunca se reutiliza");
}

#[test]
fn fila_12_i_dentro_de_un_lote_es_not_batchable() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let cel = d.layers()[0].cel();

    sin_efecto(
        &mut d,
        Command::Batch(vec![mezcla(a, BlendMode::Multiply)]),
        Err(DocumentError::InBatch {
            index: 0,
            source: Box::new(DocumentError::NotBatchable),
        }),
    );

    // Detrás de una escritura válida, que se revierte con el resto del lote.
    let escritura = Command::SetPixels {
        image: cel,
        pixels: vec![PixelEdit {
            x: 3,
            y: 2,
            pixel: Pixel::Rgba(C2),
        }],
    };
    sin_efecto(
        &mut d,
        Command::Batch(vec![escritura, mezcla(a, BlendMode::Multiply)]),
        Err(DocumentError::InBatch {
            index: 1,
            source: Box::new(DocumentError::NotBatchable),
        }),
    );
    assert_eq!(modo_de(&d, a), BlendMode::Normal);
}

#[test]
fn fila_12_j_el_modo_es_por_capa_y_sobrevive_a_mover_la_capa() {
    // Una prueba corta de que `SetLayerBlend` toca solo su capa y que el modo viaja con ella.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    assert_eq!(d.apply(mezcla(b, BlendMode::Overlay)), Ok(true));
    assert_eq!(modo_de(&d, a), BlendMode::Normal);
    assert_eq!(modo_de(&d, b), BlendMode::Overlay);

    assert_eq!(d.apply(Command::MoveLayer { layer: b, to: 0 }), Ok(true));
    assert_eq!(modo_de(&d, b), BlendMode::Overlay);
    assert_eq!(d.layers()[0].id(), b);
    assert_eq!(d.layers()[0].blend(), BlendMode::Overlay);
}
