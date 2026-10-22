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

const ROJO: Rgba = Rgba::new(255, 0, 0, 255);
const VERDE: Rgba = Rgba::new(0, 255, 0, 255);
const AZUL: Rgba = Rgba::new(0, 0, 255, 255);
const NEGRO: Rgba = Rgba::new(0, 0, 0, 255);
const AMARILLO: Rgba = Rgba::new(255, 255, 0, 255);
const BLANCO: Rgba = Rgba::new(255, 255, 255, 255);

fn img_rgba(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Rgba).expect("imagen RGBA")
}

/// `D` de la spec: documento RGBA de 4 x 3 recién creado.
fn doc_d() -> Document {
    Document::new(img_rgba(4, 3))
}

/// Imagen indexada con la paleta y el índice transparente dados (todos los píxeles en 0).
fn img_indexada(w: u32, h: u32, colores: &[Rgba], t: Option<u8>) -> Image {
    let mut paleta = Palette::from_colors(colores.iter().copied()).expect("paleta");
    paleta
        .set_transparent_index(t)
        .expect("índice transparente");
    Image::new_indexed(w, h, paleta).expect("imagen indexada")
}

/// `n` colores opacos distintos (n <= 256).
fn colores_distintos(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

// Comandos de apoyo.

fn pone_px(image: ImageId, x: u32, y: u32, pixel: Pixel) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit { x, y, pixel }],
    }
}

/// `S(c, [(x, y, color)])` de la spec.
fn pone(image: ImageId, x: u32, y: u32, c: Rgba) -> Command {
    pone_px(image, x, y, Pixel::Rgba(c))
}

/// `F(c, fila 0, color)` de la spec, en una celda de `ancho` píxeles de ancho.
fn fila0(image: ImageId, ancho: u32, c: Rgba) -> Command {
    Command::FillSpans {
        image,
        spans: vec![Span {
            y: 0,
            x_start: 0,
            x_end: ancho,
        }],
        pixel: Pixel::Rgba(c),
    }
}

fn anade_cmd(nombre: &str, at: usize) -> Command {
    Command::AddLayer {
        name: nombre.to_string(),
        at,
    }
}

fn renombra(layer: LayerId, nombre: &str) -> Command {
    Command::RenameLayer {
        layer,
        name: nombre.to_string(),
    }
}

fn opacidad(layer: LayerId, opacity: u8) -> Command {
    Command::SetLayerOpacity { layer, opacity }
}

fn visible(layer: LayerId, visible: bool) -> Command {
    Command::SetLayerVisible { layer, visible }
}

fn mueve(layer: LayerId, to: usize) -> Command {
    Command::MoveLayer { layer, to }
}

fn quita(layer: LayerId) -> Command {
    Command::RemoveLayer { layer }
}

fn sin_transparente(image: ImageId) -> Command {
    Command::SetTransparentIndex { image, index: None }
}

// Lectura de documentos.

/// Añade una capa en `at` y devuelve su `LayerId` (se lee de `layers()[at]` tras el `Ok(true)`).
fn anade(doc: &mut Document, nombre: &str, at: usize) -> LayerId {
    assert_eq!(
        doc.apply(anade_cmd(nombre, at)),
        Ok(true),
        "añadir {nombre}"
    );
    doc.layers()[at].id()
}

/// `cel(X)` de la spec.
fn celda(doc: &Document, capa: LayerId) -> ImageId {
    doc.layer(capa).expect("la capa existe").cel()
}

fn ids(doc: &Document) -> Vec<(LayerId, ImageId)> {
    doc.layers().iter().map(|l| (l.id(), l.cel())).collect()
}

fn nombres(doc: &Document) -> Vec<String> {
    doc.layers().iter().map(|l| l.name().to_string()).collect()
}

fn pixel_rgba(doc: &Document, cel: ImageId, x: u32, y: u32) -> Rgba {
    let imagen = doc.image(cel).expect("la celda tiene imagen");
    match imagen.get_pixel(x, y).expect("píxel dentro") {
        Pixel::Rgba(c) => c,
        otro => panic!("se esperaba RGBA, hay {otro:?}"),
    }
}

/// Índices de paleta de todos los píxeles de la celda, fila a fila.
fn indices(doc: &Document, cel: ImageId) -> Vec<u8> {
    let imagen = doc.image(cel).expect("la celda tiene imagen");
    let mut v = Vec::new();
    for y in 0..imagen.height() {
        for x in 0..imagen.width() {
            match imagen.get_pixel(x, y).expect("píxel dentro") {
                Pixel::Indexed(i) => v.push(i),
                otro => panic!("se esperaba indexado, hay {otro:?}"),
            }
        }
    }
    v
}

/// `[A, B, C]`: `D` con dos capas vacías encima.
fn tres_capas() -> (Document, [LayerId; 3]) {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    (d, [a, b, c])
}

/// Todo lo observable que un comando fallido o sin cambios no puede tocar.
struct Foto {
    doc: Document,
    puede_deshacer: bool,
    puede_rehacer: bool,
    historia: usize,
    estado: u64,
    materializado: usize,
}

impl Foto {
    fn de(doc: &Document) -> Foto {
        Foto {
            doc: doc.clone(),
            puede_deshacer: doc.can_undo(),
            puede_rehacer: doc.can_redo(),
            historia: doc.history_bytes(),
            estado: doc.state_id(),
            materializado: doc.materialized_bytes(),
        }
    }

    fn diferencia(&self, doc: &Document) -> Option<&'static str> {
        if *doc != self.doc {
            Some("el documento cambió")
        } else if doc.can_undo() != self.puede_deshacer {
            Some("can_undo cambió")
        } else if doc.can_redo() != self.puede_rehacer {
            Some("can_redo cambió")
        } else if doc.history_bytes() != self.historia {
            Some("history_bytes cambió")
        } else if doc.state_id() != self.estado {
            Some("state_id cambió")
        } else if doc.materialized_bytes() != self.materializado {
            Some("materialized_bytes cambió")
        } else {
            None
        }
    }

    fn assert_igual(&self, doc: &Document, contexto: &str) {
        assert_eq!(self.diferencia(doc), None, "{contexto}");
    }

    fn exige(&self, doc: &Document, contexto: &str) -> Result<(), TestCaseError> {
        match self.diferencia(doc) {
            None => Ok(()),
            Some(motivo) => Err(TestCaseError::fail(format!("{contexto}: {motivo}"))),
        }
    }

    /// Solo el documento, su `state_id` y los bytes materializados (lo que dicen las
    /// propiedades 1 y 5 al comparar dos estados).
    fn exige_estado(&self, doc: &Document, contexto: &str) -> Result<(), TestCaseError> {
        if *doc != self.doc {
            return Err(TestCaseError::fail(format!(
                "{contexto}: el documento no es igual"
            )));
        }
        if doc.state_id() != self.estado {
            return Err(TestCaseError::fail(format!(
                "{contexto}: state_id {} en vez de {}",
                doc.state_id(),
                self.estado
            )));
        }
        if doc.materialized_bytes() != self.materializado {
            return Err(TestCaseError::fail(format!(
                "{contexto}: materialized_bytes {} en vez de {}",
                doc.materialized_bytes(),
                self.materializado
            )));
        }
        Ok(())
    }
}

/// Aplica `cmd`, comprueba el resultado y que el documento, la historia y el `state_id` no
/// cambiaron.
fn sin_efecto(doc: &mut Document, cmd: Command, esperado: Result<bool, DocumentError>) {
    let foto = Foto::de(doc);
    let contexto = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), esperado, "{contexto}");
    foto.assert_igual(doc, &contexto);
}

/// `InBatch { index, source }`.
fn en_lote(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

/// Los seis comandos de capa, todos con `Ok(true)` en un documento con la capa `a` y alguna más.
fn seis_comandos(a: LayerId) -> Vec<Command> {
    vec![
        anade_cmd("Z", 0),
        quita(a),
        mueve(a, 1),
        renombra(a, "Z"),
        opacidad(a, 128),
        visible(a, false),
    ]
}

// Comprobaciones en tiempo de compilación de los traits exigidos (C-01, C-02).
fn assert_id_traits<X: Clone + Copy + Debug + PartialEq + Eq + Hash + PartialOrd + Ord>() {}
fn assert_dato_traits<X: Clone + Debug + PartialEq + Eq>() {}

#[test]
fn traits_ids_y_mensajes_de_los_errores_nuevos() {
    // C-01, C-02: traits derivados; C-29: un mensaje por variante nueva y `source` vacío.
    assert_id_traits::<LayerId>();
    assert_dato_traits::<Layer>();
    assert_eq!(LayerId::from_raw(7).raw(), 7);
    let errores = [
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
    let mut mensajes = HashSet::new();
    for error in &errores {
        let mensaje = error.to_string();
        assert!(!mensaje.is_empty(), "{error:?}");
        assert!(error.source().is_none(), "{error:?}");
        mensajes.insert(mensaje);
    }
    assert_eq!(mensajes.len(), errores.len(), "los mensajes son distintos");
}

// ---------------------------------------------------------------------------------------
// Fila 1: `Document::new`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_01_a_documento_nuevo_tiene_una_capa() {
    let mut img = img_rgba(4, 3);
    img.set_pixel(1, 1, Pixel::Rgba(C1)).expect("píxel");
    let d = Document::new(img.clone());

    assert_eq!(d.layers().len(), 1);
    let a = &d.layers()[0];
    assert_eq!(a.name(), "Capa 1");
    assert_eq!(a.id(), LayerId::from_raw(0));
    assert!(a.visible());
    assert_eq!(a.opacity(), 255);
    assert_eq!(a.cel(), d.main_image());
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
fn fila_01_b_el_lienzo_indexado_toma_la_paleta_de_la_imagen() {
    let img = img_indexada(4, 3, &[ROJO, VERDE, AZUL, NEGRO], Some(1));
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
    assert_eq!(d.image(d.main_image()), Some(&img));
}

// ---------------------------------------------------------------------------------------
// Fila 2: AddLayer en varias posiciones; deshacer y rehacer.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_02_anadir_capas_en_posiciones_y_deshacer_rehacer() {
    let mut d = doc_d();
    let original = d.clone();
    let mut celdas_vistas = vec![d.main_image()];
    let mut capas = vec![d.layers().to_vec()];
    let mut documentos = vec![d.clone()];

    // `at == len` (encima de todas), el fondo y el medio.
    for (nombre, at) in [("B", 1usize), ("N", 0), ("M", 2)] {
        let antes = d.materialized_bytes();
        assert_eq!(d.apply(anade_cmd(nombre, at)), Ok(true), "{nombre}");
        let nueva = &d.layers()[at];
        assert_eq!(nueva.name(), nombre);
        assert!(nueva.visible());
        assert_eq!(nueva.opacity(), 255);
        assert!(d.image(nueva.cel()).is_none(), "{nombre} nace vacía");
        assert!(
            !celdas_vistas.contains(&nueva.cel()),
            "{nombre}: ImageId nuevo"
        );
        celdas_vistas.push(nueva.cel());
        assert_eq!(d.materialized_bytes(), antes, "{nombre}");
        capas.push(d.layers().to_vec());
        documentos.push(d.clone());
    }
    assert_eq!(d.layers().len(), 4);
    let ids_capa: HashSet<LayerId> = d.layers().iter().map(Layer::id).collect();
    assert_eq!(ids_capa.len(), 4, "LayerId distintos");

    // Un `undo` quita la última añadida; `redo` la repone con los mismos ids.
    d.undo().expect("deshacer");
    assert_eq!(d.layers(), capas[2].as_slice());
    assert_eq!(d, documentos[2]);
    d.redo().expect("rehacer");
    assert_eq!(d.layers(), capas[3].as_slice());
    assert_eq!(d, documentos[3]);

    for (i, esperadas) in capas.iter().enumerate().take(3).rev() {
        d.undo().expect("deshacer");
        assert_eq!(d.layers(), esperadas.as_slice(), "tras deshacer hasta {i}");
    }
    assert_eq!(d, original);
    assert_eq!(d.state_id(), 0);
    assert!(!d.can_undo());
}

// ---------------------------------------------------------------------------------------
// Fila 3: escribir en una celda vacía la materializa.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_03_a_set_pixels_materializa_y_undo_la_vacia() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    let antes = d.clone();
    let estado = d.state_id();

    assert_eq!(d.apply(pone(cb, 0, 0, C1)), Ok(true));
    assert!(d.image(cb).is_some());
    for y in 0..3 {
        for x in 0..4 {
            let esperado = if (x, y) == (0, 0) { C1 } else { T };
            assert_eq!(pixel_rgba(&d, cb, x, y), esperado, "({x}, {y})");
        }
    }
    assert_eq!(d.materialized_bytes(), 96);
    let despues = d.clone();

    d.undo().expect("deshacer");
    assert_eq!(d, antes);
    assert!(d.image(cb).is_none(), "la celda vuelve a estar vacía");
    assert_eq!(d.materialized_bytes(), 48);
    assert_eq!(d.state_id(), estado);

    d.redo().expect("rehacer");
    assert_eq!(d, despues);
    assert_eq!(pixel_rgba(&d, cb, 0, 0), C1);
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn fila_03_b_fill_spans_materializa_y_undo_la_vacia() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    let antes = d.clone();

    assert_eq!(d.apply(fila0(cb, 4, C1)), Ok(true));
    for x in 0..4 {
        assert_eq!(pixel_rgba(&d, cb, x, 0), C1, "fila 0, x {x}");
        assert_eq!(pixel_rgba(&d, cb, x, 1), T, "fila 1, x {x}");
        assert_eq!(pixel_rgba(&d, cb, x, 2), T, "fila 2, x {x}");
    }
    assert_eq!(d.materialized_bytes(), 96);
    let despues = d.clone();

    d.undo().expect("deshacer");
    assert_eq!(d, antes);
    assert!(d.image(cb).is_none());
    assert_eq!(d.materialized_bytes(), 48);

    d.redo().expect("rehacer");
    assert_eq!(d, despues);
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn fila_03_c_un_lote_materializa_varias_celdas_en_un_solo_paso() {
    let (mut d, [a, b, c]) = tres_capas();
    let (ca, cb, cc) = (celda(&d, a), celda(&d, b), celda(&d, c));
    let antes = d.clone();
    let estado = d.state_id();

    let lote = Command::Batch(vec![
        pone(ca, 1, 1, C1),
        fila0(cb, 4, C2),
        pone(cc, 2, 2, C2),
    ]);
    assert_eq!(d.apply(lote), Ok(true));
    assert!(d.image(cb).is_some());
    assert!(d.image(cc).is_some());
    // Una celda ya con imagen más dos materializadas.
    assert_eq!(d.materialized_bytes(), 48 * 3);
    assert_eq!(pixel_rgba(&d, ca, 1, 1), C1);
    assert_eq!(pixel_rgba(&d, cb, 3, 0), C2);
    assert_eq!(pixel_rgba(&d, cc, 2, 2), C2);
    let despues = d.clone();

    // Un solo `undo` deshace el lote entero y deja las dos celdas vacías.
    d.undo().expect("deshacer");
    assert_eq!(d, antes);
    assert!(d.image(cb).is_none());
    assert!(d.image(cc).is_none());
    assert_eq!(d.materialized_bytes(), 48);
    assert_eq!(d.state_id(), estado);

    d.redo().expect("rehacer");
    assert_eq!(d, despues);
}

#[test]
fn fila_03_d_escribir_transparente_en_celda_vacia_no_la_materializa() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);

    sin_efecto(&mut d, pone(cb, 0, 0, T), Ok(false));
    assert!(d.image(cb).is_none());
    assert_eq!(d.materialized_bytes(), 48);

    sin_efecto(&mut d, fila0(cb, 4, T), Ok(false));
    assert!(d.image(cb).is_none());
    assert_eq!(d.materialized_bytes(), 48);
}

#[test]
fn fila_03_e_escribir_fuera_del_lienzo_no_deja_celda_a_medias() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    let foto = Foto::de(&d);

    let resultado = d.apply(pone(cb, 9, 9, C1));
    assert!(
        matches!(resultado, Err(DocumentError::Pixel { .. })),
        "{resultado:?}"
    );
    foto.assert_igual(&d, "píxel fuera");
    assert!(d.image(cb).is_none(), "la celda sigue vacía");
}

// ---------------------------------------------------------------------------------------
// Fila 4: RemoveLayer.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_04_a_quitar_una_capa_con_imagen_y_deshacer_devuelve_lo_mismo() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let ca = d.main_image();
    assert_eq!(d.apply(pone(ca, 1, 1, C1)), Ok(true));
    anade(&mut d, "B", 1);

    let antes = d.clone();
    let capas = d.layers().to_vec();
    let imagen = d.image(ca).expect("imagen de A").clone();
    let estado = d.state_id();
    let materializado = d.materialized_bytes();
    let historia = d.history_bytes();

    assert_eq!(d.apply(quita(a)), Ok(true));
    assert_eq!(d.layers().len(), 1);
    assert!(d.layer(a).is_none());
    assert!(d.image(ca).is_none());
    assert_eq!(d.materialized_bytes(), 0);
    // La celda guardada (48 bytes de píxeles) cuenta en la historia.
    assert!(d.history_bytes() >= historia + 48, "{}", d.history_bytes());
    let despues = d.clone();

    d.undo().expect("deshacer");
    assert_eq!(d, antes);
    assert_eq!(d.layers(), capas.as_slice());
    assert_eq!(d.layers()[0].id(), a);
    assert_eq!(d.layers()[0].cel(), ca);
    assert_eq!(d.image(ca), Some(&imagen));
    assert_eq!(d.materialized_bytes(), materializado);
    assert_eq!(d.state_id(), estado);

    d.redo().expect("rehacer");
    assert_eq!(d, despues);
}

#[test]
fn fila_04_b_quitar_la_unica_capa_o_una_desconocida_es_error() {
    // La única capa: `LastLayer`; el fondo no es especial pero tampoco se queda sin capas.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    sin_efecto(&mut d, quita(a), Err(DocumentError::LastLayer));

    // Un `LayerId` que no existe, con dos capas.
    // SPEC?: con un `LayerId` desconocido en un documento de una sola capa, la spec no dice si
    // gana `UnknownLayer` o `LastLayer`; aquí solo se prueba con dos capas.
    anade(&mut d, "B", 1);
    let falsa = LayerId::from_raw(999);
    sin_efecto(
        &mut d,
        quita(falsa),
        Err(DocumentError::UnknownLayer(falsa)),
    );
}

#[test]
fn fila_04_c_los_ids_no_se_reutilizan_tras_quitar() {
    fn maximos(d: &Document, max: &mut (u32, u32)) {
        for capa in d.layers() {
            max.0 = max.0.max(capa.id().raw());
            max.1 = max.1.max(capa.cel().raw());
        }
    }

    let mut d = doc_d();
    let mut max = (0, 0);
    maximos(&d, &mut max);
    anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    maximos(&d, &mut max);

    // Quitar la de arriba (la de ids mayores) y añadir otra: ids mayores que cualquiera visto.
    assert_eq!(d.apply(quita(c)), Ok(true));
    let nueva = anade(&mut d, "D", 2);
    let capa = d.layer(nueva).expect("la capa nueva existe");
    assert!(capa.id().raw() > max.0, "LayerId reutilizado");
    assert!(capa.cel().raw() > max.1, "ImageId reutilizado");
    maximos(&d, &mut max);

    // Deshacer el añadir y el quitar, y volver a añadir: sigue sin reutilizar nada.
    d.undo().expect("deshacer añadir");
    d.undo().expect("deshacer quitar");
    assert!(d.layer(c).is_some(), "C vuelve con su id");
    let otra = anade(&mut d, "E", 3);
    let capa = d.layer(otra).expect("la capa nueva existe");
    assert!(capa.id().raw() > max.0, "LayerId reutilizado tras undo");
    assert!(capa.cel().raw() > max.1, "ImageId reutilizado tras undo");
}

// ---------------------------------------------------------------------------------------
// Fila 5: MoveLayer.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_05_a_mover_deshacer_y_rehacer() {
    let (mut d, [a, b, c]) = tres_capas();
    let celdas: Vec<ImageId> = [a, b, c].iter().map(|&x| celda(&d, x)).collect();
    let abc = d.layers().to_vec();

    assert_eq!(d.apply(mueve(a, 2)), Ok(true));
    let bca = d.layers().to_vec();
    let orden: Vec<LayerId> = d.layers().iter().map(Layer::id).collect();
    assert_eq!(orden, vec![b, c, a]);
    // Las celdas no cambian de id al mover.
    assert_eq!(celda(&d, a), celdas[0]);
    assert_eq!(celda(&d, b), celdas[1]);
    assert_eq!(celda(&d, c), celdas[2]);

    // La segunda vez no hay cambio y no hay paso.
    sin_efecto(&mut d, mueve(a, 2), Ok(false));

    d.undo().expect("deshacer");
    assert_eq!(d.layers(), abc.as_slice());
    d.redo().expect("rehacer");
    assert_eq!(d.layers(), bca.as_slice());
}

#[test]
fn fila_05_b_mover_fuera_de_rango_o_capa_desconocida_es_error() {
    let (mut d, [a, ..]) = tres_capas();
    // `to` es la posición final: con 3 capas, la mayor válida es 2.
    sin_efecto(
        &mut d,
        mueve(a, 3),
        Err(DocumentError::LayerIndex { index: 3, max: 2 }),
    );
    let falsa = LayerId::from_raw(999);
    sin_efecto(
        &mut d,
        mueve(falsa, 0),
        Err(DocumentError::UnknownLayer(falsa)),
    );
}

// ---------------------------------------------------------------------------------------
// Fila 6: RenameLayer, SetLayerOpacity y SetLayerVisible.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_06_a_cada_propiedad_es_un_paso_y_repetir_no_hace_nada() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let inicial = d.layers().to_vec();
    let mut capas = vec![inicial.clone()];
    let mut estados = vec![d.state_id()];

    assert_eq!(d.apply(renombra(a, "Otra")), Ok(true));
    assert_eq!(d.layers()[0].name(), "Otra");
    capas.push(d.layers().to_vec());
    estados.push(d.state_id());

    assert_eq!(d.apply(opacidad(a, 128)), Ok(true));
    assert_eq!(d.layers()[0].opacity(), 128);
    capas.push(d.layers().to_vec());
    estados.push(d.state_id());

    assert_eq!(d.apply(visible(a, false)), Ok(true));
    assert!(!d.layers()[0].visible());
    capas.push(d.layers().to_vec());
    estados.push(d.state_id());

    // Cada uno es un paso (cada `state_id` es nuevo)...
    let distintos: HashSet<u64> = estados.iter().copied().collect();
    assert_eq!(distintos.len(), 4);

    // ...y repetir el mismo valor no es un paso.
    sin_efecto(&mut d, renombra(a, "Otra"), Ok(false));
    sin_efecto(&mut d, opacidad(a, 128), Ok(false));
    sin_efecto(&mut d, visible(a, false), Ok(false));

    // Tres `undo` y ninguno más; `redo` los repone.
    for (i, esperadas) in capas.iter().enumerate().take(3).rev() {
        d.undo().expect("deshacer");
        assert_eq!(d.layers(), esperadas.as_slice(), "tras deshacer hasta {i}");
        assert_eq!(d.state_id(), estados[i]);
    }
    assert_eq!(d.undo(), Err(DocumentError::NothingToUndo));
    for (i, esperadas) in capas.iter().enumerate().skip(1) {
        d.redo().expect("rehacer");
        assert_eq!(d.layers(), esperadas.as_slice(), "tras rehacer hasta {i}");
    }
    assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));
}

#[test]
fn fila_06_b_el_nombre_mide_bytes_utf8() {
    let mut d = doc_d();
    let a = d.layers()[0].id();

    // 255 bytes valen.
    let justo = "a".repeat(255);
    assert_eq!(d.apply(renombra(a, &justo)), Ok(true));
    assert_eq!(d.layers()[0].name(), justo);
    // 255 bytes con caracteres de dos bytes (127 `é` y una `a`).
    let mixto = format!("{}a", "é".repeat(127));
    assert_eq!(mixto.len(), 255);
    assert_eq!(d.apply(renombra(a, &mixto)), Ok(true));
    assert_eq!(d.layers()[0].name(), mixto);

    // 256 bytes no, ni de `a` ni de 128 `é` (128 caracteres, 256 bytes).
    for largo in ["a".repeat(256), "é".repeat(128)] {
        sin_efecto(
            &mut d,
            renombra(a, &largo),
            Err(DocumentError::LayerName {
                len: 256,
                limit: 255,
            }),
        );
    }
    assert_eq!(d.layers()[0].name(), mixto);
}

#[test]
fn fila_06_c_capa_desconocida_en_los_tres_comandos() {
    let mut d = doc_d();
    let falsa = LayerId::from_raw(999);
    let error = Err(DocumentError::UnknownLayer(falsa));
    sin_efecto(&mut d, renombra(falsa, "X"), error.clone());
    sin_efecto(&mut d, opacidad(falsa, 128), error.clone());
    sin_efecto(&mut d, visible(falsa, false), error);
}

#[test]
fn fila_06_d_en_indexado_la_opacidad_es_siempre_255() {
    let mut d = Document::new(img_indexada(4, 3, &[ROJO, VERDE, AZUL, NEGRO], Some(0)));
    let a = d.layers()[0].id();

    sin_efecto(&mut d, opacidad(a, 128), Err(DocumentError::IndexedOpacity));
    sin_efecto(&mut d, opacidad(a, 255), Ok(false));
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
fn fila_07_a_sin_valor_transparente_no_se_puede_anadir() {
    let mut gris = Document::new(Image::new(4, 3, Mode::Gray).expect("imagen gris"));
    sin_efecto(
        &mut gris,
        anade_cmd("B", 1),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );

    let mut indexado = Document::new(img_indexada(4, 3, &[ROJO, VERDE], None));
    sin_efecto(
        &mut indexado,
        anade_cmd("B", 1),
        Err(DocumentError::NoTransparency {
            mode: Mode::Indexed,
        }),
    );
}

#[test]
fn fila_07_b_con_valor_transparente_se_puede_anadir() {
    let mut gris_alfa = Document::new(Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"));
    let b = anade(&mut gris_alfa, "B", 1);
    assert!(gris_alfa.image(celda(&gris_alfa, b)).is_none());

    let mut indexado = Document::new(img_indexada(4, 3, &[ROJO, VERDE], Some(1)));
    let b = anade(&mut indexado, "B", 1);
    assert!(indexado.image(celda(&indexado, b)).is_none());
    assert_eq!(indexado.layers()[1].opacity(), 255);
}

/// Documento de 1 x 1 con 256 capas (RGBA o indexado con índice transparente).
fn doc_256(indexado: bool) -> Document {
    let img = if indexado {
        img_indexada(1, 1, &[ROJO, VERDE], Some(1))
    } else {
        img_rgba(1, 1)
    };
    let mut d = Document::new(img);
    for i in 1..Document::MAX_LAYERS {
        assert_eq!(
            d.apply(anade_cmd(&format!("c{i}"), i)),
            Ok(true),
            "capa {i}"
        );
    }
    assert_eq!(d.layers().len(), 256);
    d
}

#[test]
fn fila_07_c_con_256_capas_no_cabe_otra() {
    let mut d = doc_256(false);
    assert_eq!(d.worst_case_bytes(), 256 * d.cel_bytes());
    sin_efecto(
        &mut d,
        anade_cmd("una más", 0),
        Err(DocumentError::TooManyLayers { limit: 256 }),
    );
    assert_eq!(d.layers().len(), 256);
}

#[test]
fn fila_07_d_posicion_mayor_que_len_es_error() {
    let mut d = doc_d();
    // Con una capa, la mayor posición válida es 1.
    sin_efecto(
        &mut d,
        anade_cmd("B", 2),
        Err(DocumentError::LayerIndex { index: 2, max: 1 }),
    );
    sin_efecto(
        &mut d,
        anade_cmd("B", usize::MAX),
        Err(DocumentError::LayerIndex {
            index: usize::MAX,
            max: 1,
        }),
    );
    assert_eq!(d.apply(anade_cmd("B", 1)), Ok(true));
}

#[test]
fn fila_07_e_nombre_de_256_bytes_es_error() {
    let mut d = doc_d();
    sin_efecto(
        &mut d,
        anade_cmd(&"a".repeat(256), 1),
        Err(DocumentError::LayerName {
            len: 256,
            limit: 255,
        }),
    );
    // 255 bytes valen.
    assert_eq!(d.apply(anade_cmd(&"a".repeat(255), 1)), Ok(true));
}

#[test]
fn fila_07_f_orden_de_comprobacion_nombre_transparencia_numero_posicion() {
    let largo = "a".repeat(256);
    let error_nombre = DocumentError::LayerName {
        len: 256,
        limit: 255,
    };

    // Nombre antes que transparencia (y que posición).
    let mut gris = Document::new(Image::new(4, 3, Mode::Gray).expect("imagen gris"));
    sin_efecto(&mut gris, anade_cmd(&largo, 99), Err(error_nombre.clone()));
    // Transparencia antes que posición.
    sin_efecto(
        &mut gris,
        anade_cmd("B", 99),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );

    // Nombre antes que número de capas; número de capas antes que posición.
    let mut lleno = doc_256(false);
    sin_efecto(&mut lleno, anade_cmd(&largo, 999), Err(error_nombre));
    sin_efecto(
        &mut lleno,
        anade_cmd("B", 999),
        Err(DocumentError::TooManyLayers { limit: 256 }),
    );

    // Transparencia antes que número de capas: 256 capas en indexado y luego sin índice
    // transparente.
    let mut indexado = doc_256(true);
    let cel = indexado.layers()[0].cel();
    assert_eq!(indexado.apply(sin_transparente(cel)), Ok(true));
    sin_efecto(
        &mut indexado,
        anade_cmd("B", 999),
        Err(DocumentError::NoTransparency {
            mode: Mode::Indexed,
        }),
    );
}

// ---------------------------------------------------------------------------------------
// Fila 8: los comandos de paleta editan todo el documento en un solo paso.
// ---------------------------------------------------------------------------------------

/// Indexado de 4 x 3, paleta `[ROJO, VERDE, AZUL, NEGRO]` con el índice 0 transparente y tres
/// capas: `A` (píxeles de la entrada 2 en (0,0) y (1,1), y de la 1 en (3,2)), `B` (entrada 2 en
/// (0,0), 1 en (2,1) y 3 en (3,0)) y `C` vacía. Devuelve las tres celdas.
fn doc_paleta() -> (Document, [ImageId; 3]) {
    let mut img = img_indexada(4, 3, &[ROJO, VERDE, AZUL, NEGRO], Some(0));
    for (x, y, i) in [(0, 0, 2), (1, 1, 2), (3, 2, 1)] {
        img.set_pixel(x, y, Pixel::Indexed(i)).expect("píxel");
    }
    let mut d = Document::new(img);
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    let cb = celda(&d, b);
    let comando = Command::SetPixels {
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
    assert_eq!(d.apply(comando), Ok(true));
    let celdas = [d.layers()[0].cel(), cb, celda(&d, c)];
    (d, celdas)
}

/// Toda celda con imagen tiene la paleta del lienzo (`Some`).
fn paletas_iguales(d: &Document, contexto: &str) {
    let lienzo = d.palette().expect("el lienzo indexado tiene paleta");
    for capa in d.layers() {
        if let Some(imagen) = d.image(capa.cel()) {
            assert_eq!(
                imagen.palette(),
                Some(lienzo),
                "{contexto}: {}",
                capa.name()
            );
        }
    }
}

/// Aplica el comando de paleta (con `image` = la celda `A`) y comprueba la fila 8.
fn comprueba_paleta(nombre: &str, comando: impl Fn(ImageId) -> Command, remapea: fn(u8) -> u8) {
    let (mut d, [ca, cb, cc]) = doc_paleta();
    let antes = d.clone();
    let estado = d.state_id();
    let indices_a = indices(&d, ca);
    let indices_b = indices(&d, cb);

    assert_eq!(d.apply(comando(ca)), Ok(true), "{nombre}");
    let despues = d.clone();
    paletas_iguales(&d, nombre);
    // Los píxeles de las dos celdas se remapean igual.
    let esperado_a: Vec<u8> = indices_a.iter().map(|&i| remapea(i)).collect();
    let esperado_b: Vec<u8> = indices_b.iter().map(|&i| remapea(i)).collect();
    assert_eq!(indices(&d, ca), esperado_a, "{nombre}: celda A");
    assert_eq!(indices(&d, cb), esperado_b, "{nombre}: celda B");
    assert!(d.image(cc).is_none(), "{nombre}: C sigue vacía");

    // Un solo paso: un `undo` devuelve exactamente el documento y el `state_id` de antes.
    d.undo().expect("deshacer");
    assert_eq!(d, antes, "{nombre}: undo");
    assert_eq!(d.state_id(), estado, "{nombre}: undo");
    assert!(d.image(cc).is_none(), "{nombre}: C vacía tras undo");
    d.redo().expect("rehacer");
    assert_eq!(d, despues, "{nombre}: redo");
    assert!(d.image(cc).is_none(), "{nombre}: C vacía tras redo");

    // Al materializarla, la celda vacía toma la paleta que hay entonces.
    assert_eq!(
        d.apply(pone_px(cc, 0, 0, Pixel::Indexed(2))),
        Ok(true),
        "{nombre}: materializar C"
    );
    paletas_iguales(&d, &format!("{nombre} con C materializada"));
    assert_eq!(
        d.image(cc).expect("C con imagen").palette(),
        d.palette(),
        "{nombre}: paleta de C"
    );
}

#[test]
fn fila_08_a_cada_comando_de_paleta_edita_todas_las_celdas_con_imagen() {
    comprueba_paleta(
        "SetPaletteColor",
        |image| Command::SetPaletteColor {
            image,
            index: 2,
            color: BLANCO,
        },
        |i| i,
    );
    comprueba_paleta(
        "SetTransparentIndex",
        |image| Command::SetTransparentIndex {
            image,
            index: Some(1),
        },
        |i| i,
    );
    comprueba_paleta(
        "AddPaletteEntry",
        |image| Command::AddPaletteEntry {
            image,
            color: AMARILLO,
        },
        |i| i,
    );
    // La entrada 2 pasa a la posición 0 y las de en medio se desplazan.
    comprueba_paleta(
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
            otro => otro,
        },
    );
    // `order[nuevo] = antiguo` con `[3, 2, 1, 0]`: el índice antiguo `i` pasa a `3 - i`.
    comprueba_paleta(
        "PermutePalette",
        |image| Command::PermutePalette {
            image,
            order: vec![3, 2, 1, 0],
        },
        |i| 3 - i,
    );
    // Los píxeles de la entrada 2 pasan a la 0 y los índices mayores bajan uno.
    comprueba_paleta(
        "RemovePaletteEntry",
        |image| Command::RemovePaletteEntry {
            image,
            index: 2,
            into: 0,
        },
        |i| match i {
            2 => 0,
            3 => 2,
            otro => otro,
        },
    );
    // Los píxeles conservan su índice (cambian los colores de las ranuras).
    comprueba_paleta(
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
fn fila_08_b_con_una_capa_el_coste_de_cada_paso_de_paleta_es_el_de_hoy() {
    // Salvo `RemovePaletteEntry` (que además guarda una máscara por celda), un paso de
    // paleta de un documento de una capa cuesta como mucho 512 bytes.
    let comandos = |image: ImageId| {
        vec![
            Command::SetPaletteColor {
                image,
                index: 2,
                color: BLANCO,
            },
            Command::SetTransparentIndex {
                image,
                index: Some(1),
            },
            Command::AddPaletteEntry {
                image,
                color: AMARILLO,
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
    let imagen = img_indexada(4, 3, &[ROJO, VERDE, AZUL, NEGRO], Some(0));
    for comando in comandos(ImageId::from_raw(0)) {
        let mut d = Document::new(imagen.clone());
        assert_eq!(d.history_bytes(), 0);
        let contexto = format!("{comando:?}");
        assert_eq!(d.apply(comando), Ok(true), "{contexto}");
        assert!(
            d.history_bytes() <= 512,
            "{contexto}: {} bytes",
            d.history_bytes()
        );
    }
}

#[test]
fn fila_08_c_el_campo_image_solo_se_valida() {
    let (mut d, [_, _, cc]) = doc_paleta();
    // Una celda vacía vale como `image`: se edita igualmente el documento entero.
    let comando = Command::SetPaletteColor {
        image: cc,
        index: 2,
        color: BLANCO,
    };
    assert_eq!(d.apply(comando), Ok(true));
    assert_eq!(d.palette().expect("paleta").colors()[2], BLANCO);
    paletas_iguales(&d, "SetPaletteColor con image vacía");
    assert!(d.image(cc).is_none());

    // Un id que no es de ninguna celda es `UnknownImage`.
    let falsa = ImageId::from_raw(999);
    sin_efecto(
        &mut d,
        Command::SetPaletteColor {
            image: falsa,
            index: 0,
            color: AMARILLO,
        },
        Err(DocumentError::UnknownImage(falsa)),
    );
}

// ---------------------------------------------------------------------------------------
// Fila 9: ConvertMode.
// ---------------------------------------------------------------------------------------

fn convertir(image: ImageId, target: Mode) -> Command {
    Command::ConvertMode {
        image,
        target,
        allowed: Allowed::NOTHING,
    }
}

/// Documento RGBA de 4 x 3 con dos colores opacos y el resto transparente.
fn doc_con_colores() -> Document {
    let mut img = img_rgba(4, 3);
    img.set_pixel(0, 0, Pixel::Rgba(C1)).expect("píxel");
    img.set_pixel(1, 0, Pixel::Rgba(C2)).expect("píxel");
    Document::new(img)
}

#[test]
fn fila_09_a_con_una_capa_el_lienzo_sigue_a_la_conversion() {
    let mut d = doc_con_colores();
    let cel = d.layers()[0].cel();
    let rgba = d.clone();

    // RGBA a indexado: el lienzo sigue a la imagen en `apply`, `undo` y `redo`.
    assert_eq!(d.apply(convertir(cel, Mode::Indexed)), Ok(true));
    let indexado = d.clone();
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
    assert_eq!(d, indexado);
    assert_eq!(d.mode(), Mode::Indexed);
    assert_eq!(d.palette(), d.image(cel).expect("imagen").palette());

    // Y de vuelta a RGBA.
    assert_eq!(d.apply(convertir(cel, Mode::Rgba)), Ok(true));
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    d.undo().expect("deshacer");
    assert_eq!(d.mode(), Mode::Indexed);
    assert_eq!(d.palette(), d.image(cel).expect("imagen").palette());
    assert_eq!(d, indexado);
}

#[test]
fn fila_09_b_con_dos_capas_no_se_convierte() {
    let mut d = doc_con_colores();
    anade(&mut d, "B", 1);
    let cel = d.layers()[0].cel();
    sin_efecto(
        &mut d,
        convertir(cel, Mode::Indexed),
        Err(DocumentError::MultiLayerConvert { layers: 2 }),
    );
    assert_eq!(d.mode(), Mode::Rgba);
}

#[test]
fn fila_09_c_a_indexado_con_opacidad_distinta_de_255_no_se_convierte() {
    let mut d = doc_con_colores();
    let a = d.layers()[0].id();
    let cel = d.layers()[0].cel();
    assert_eq!(d.apply(opacidad(a, 128)), Ok(true));
    sin_efecto(
        &mut d,
        convertir(cel, Mode::Indexed),
        Err(DocumentError::IndexedOpacity),
    );
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
}

// ---------------------------------------------------------------------------------------
// Fila 10: Batch con celdas vacías y con órdenes de capa.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_a_un_lote_que_materializa_es_un_solo_paso() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let (ca, cb) = (d.main_image(), celda(&d, b));
    let antes = d.clone();
    let estado = d.state_id();

    let lote = Command::Batch(vec![pone(ca, 0, 0, C1), fila0(cb, 4, C2)]);
    assert_eq!(d.apply(lote), Ok(true));
    assert!(d.image(cb).is_some());
    assert_eq!(pixel_rgba(&d, ca, 0, 0), C1);
    assert_eq!(pixel_rgba(&d, cb, 2, 0), C2);

    d.undo().expect("deshacer");
    assert_eq!(d, antes);
    assert!(d.image(cb).is_none(), "B vuelve a estar vacía");
    assert_eq!(d.state_id(), estado);
}

#[test]
fn fila_10_b_los_comandos_de_capa_no_van_en_un_lote() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let ca = d.main_image();
    anade(&mut d, "B", 1);

    for comando in seis_comandos(a) {
        let contexto = format!("{comando:?}");
        sin_efecto(
            &mut d,
            Command::Batch(vec![comando.clone()]),
            Err(en_lote(0, DocumentError::NotBatchable)),
        );
        // También detrás de una escritura válida, que se revierte.
        sin_efecto(
            &mut d,
            Command::Batch(vec![pone(ca, 0, 0, C1), comando]),
            Err(en_lote(1, DocumentError::NotBatchable)),
        );
        assert_eq!(pixel_rgba(&d, ca, 0, 0), T, "{contexto}");
    }
}

#[test]
fn fila_10_c_un_lote_que_falla_no_deja_celdas_materializadas() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let (ca, cb) = (d.main_image(), celda(&d, b));

    let lote = Command::Batch(vec![fila0(cb, 4, C1), pone(ca, 9, 9, C2)]);
    let foto = Foto::de(&d);
    let resultado = d.apply(lote);
    assert!(
        matches!(
            &resultado,
            Err(DocumentError::InBatch { index: 1, source })
                if matches!(**source, DocumentError::Pixel { .. })
        ),
        "{resultado:?}"
    );
    foto.assert_igual(&d, "lote fallido");
    assert!(d.image(cb).is_none(), "B no queda materializada");
    assert_eq!(d.materialized_bytes(), 48);
}

#[test]
fn fila_10_d_un_id_desconocido_en_un_lote_es_unknown_image_en_su_turno() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    let falsa = ImageId::from_raw(999);

    let lote = Command::Batch(vec![fila0(cb, 4, C1), pone(falsa, 0, 0, C2)]);
    sin_efecto(
        &mut d,
        lote,
        Err(en_lote(1, DocumentError::UnknownImage(falsa))),
    );
    assert!(d.image(cb).is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 11: límite de celdas materializadas, `worst_case_bytes` y constantes.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_11_a_materializar_por_encima_del_limite_es_celstoolarge() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    d.set_cells_limit(d.cel_bytes());
    assert_eq!(d.cells_limit(), 48);

    sin_efecto(
        &mut d,
        pone(cb, 0, 0, C1),
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        }),
    );
    assert!(d.image(cb).is_none(), "ninguna celda a medias");
    assert_eq!(d.materialized_bytes(), 48);
    // También con un relleno.
    sin_efecto(
        &mut d,
        fila0(cb, 4, C1),
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        }),
    );
}

#[test]
fn fila_11_b_rehacer_sobre_un_limite_mas_bajo_falla_y_se_puede_reintentar() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    assert_eq!(d.apply(pone(cb, 0, 0, C1)), Ok(true));
    let con_celda = d.clone();
    d.undo().expect("deshacer");

    d.set_cells_limit(48);
    let foto = Foto::de(&d);
    let resultado = d.redo();
    assert!(
        matches!(resultado, Err(DocumentError::CelsTooLarge { .. })),
        "{resultado:?}"
    );
    // Nada cambió y el paso sigue en la pila de rehacer.
    foto.assert_igual(&d, "redo fallido");
    assert!(d.can_redo());
    assert!(d.image(cb).is_none());

    d.set_cells_limit(96);
    d.redo().expect("rehacer con el límite subido");
    assert_eq!(d, con_celda);
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn fila_11_c_bajar_el_limite_no_descarta_nada() {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let cb = celda(&d, b);
    assert_eq!(d.apply(pone(cb, 0, 0, C1)), Ok(true));
    let antes = d.clone();

    d.set_cells_limit(10);
    assert_eq!(d.cells_limit(), 10);
    assert_eq!(d, antes);
    assert_eq!(d.materialized_bytes(), 96);
    // Escribir en una celda que ya tiene imagen no materializa nada (C-09: el límite «solo
    // impide materializar más»).
    assert_eq!(d.apply(pone(cb, 1, 0, C2)), Ok(true));
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn fila_11_d_peor_caso_y_constantes() {
    let mut d = doc_d();
    assert_eq!(d.worst_case_bytes(), d.layers().len() * d.cel_bytes());
    for i in 1..4 {
        anade(&mut d, &format!("c{i}"), i);
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
fn fila_11_e_un_lote_comprueba_todas_las_celdas_vacias_antes_de_ejecutar() {
    let (mut d, [a, b, c]) = tres_capas();
    let (ca, cb, cc) = (celda(&d, a), celda(&d, b), celda(&d, c));
    // Cabe una celda más (48 + 48 = 96), pero el lote nombra dos vacías.
    d.set_cells_limit(96);
    let foto = Foto::de(&d);

    let lote = Command::Batch(vec![
        pone(ca, 0, 0, C1),
        fila0(cb, 4, C1),
        pone(cc, 0, 0, C2),
    ]);
    let resultado = d.apply(lote);
    // SPEC?: la spec no dice si este error sale envuelto en `InBatch` ni cuál es `needed` con
    // varias celdas (el texto dice `materialized_bytes() + cel_bytes() × celdas vacías
    // nombradas`; C-28, `materialized_bytes() + cel_bytes()`). Se quita el envoltorio y solo se
    // comprueba `limit`.
    let error = match resultado {
        Err(DocumentError::InBatch { source, .. }) => *source,
        otro => otro.expect_err("el lote debe fallar"),
    };
    assert!(
        matches!(error, DocumentError::CelsTooLarge { limit: 96, .. }),
        "{error:?}"
    );
    foto.assert_igual(&d, "lote que no cabe");
    // Ni la escritura sobre la celda con imagen, que va antes en el lote, se aplicó.
    assert_eq!(pixel_rgba(&d, ca, 0, 0), T);
    assert!(d.image(cb).is_none());
    assert!(d.image(cc).is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 12: `state_id` e igualdad con los comandos de capa.
// ---------------------------------------------------------------------------------------

/// Aplica `cmd` (`Ok(true)`) y comprueba que el `state_id` es nuevo.
fn aplica_nuevo(d: &mut Document, cmd: Command, vistos: &mut Vec<u64>) {
    let contexto = format!("{cmd:?}");
    assert_eq!(d.apply(cmd), Ok(true), "{contexto}");
    let id = d.state_id();
    assert!(!vistos.contains(&id), "{contexto}: state_id {id} repetido");
    vistos.push(id);
}

#[test]
fn fila_12_a_cada_comando_de_capa_da_un_state_id_nuevo_y_undo_redo_lo_recorren() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let mut vistos = vec![d.state_id()];
    assert_eq!(vistos[0], 0);

    aplica_nuevo(&mut d, anade_cmd("B", 1), &mut vistos);
    let b = d.layers()[1].id();
    aplica_nuevo(&mut d, renombra(b, "Beta"), &mut vistos);
    aplica_nuevo(&mut d, opacidad(b, 128), &mut vistos);
    aplica_nuevo(&mut d, visible(b, false), &mut vistos);
    aplica_nuevo(&mut d, mueve(b, 0), &mut vistos);
    aplica_nuevo(&mut d, quita(a), &mut vistos);
    assert_eq!(vistos.len(), 7);

    for (i, estado) in vistos.iter().enumerate().take(6).rev() {
        d.undo().expect("deshacer");
        assert_eq!(d.state_id(), *estado, "undo hasta {i}");
    }
    for (i, estado) in vistos.iter().enumerate().skip(1) {
        d.redo().expect("rehacer");
        assert_eq!(d.state_id(), *estado, "redo hasta {i}");
    }
}

#[test]
fn fila_12_b_sin_cambios_y_con_error_no_tocan_el_state_id() {
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let falsa = LayerId::from_raw(999);

    // `Ok(false)`.
    sin_efecto(&mut d, renombra(b, "B"), Ok(false));
    sin_efecto(&mut d, opacidad(b, 255), Ok(false));
    sin_efecto(&mut d, visible(b, true), Ok(false));
    sin_efecto(&mut d, mueve(b, 1), Ok(false));

    // Errores.
    sin_efecto(
        &mut d,
        anade_cmd("X", 99),
        Err(DocumentError::LayerIndex { index: 99, max: 2 }),
    );
    sin_efecto(
        &mut d,
        quita(falsa),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    sin_efecto(
        &mut d,
        mueve(a, 99),
        Err(DocumentError::LayerIndex { index: 99, max: 1 }),
    );
    sin_efecto(
        &mut d,
        renombra(falsa, "X"),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    sin_efecto(
        &mut d,
        opacidad(falsa, 1),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    sin_efecto(
        &mut d,
        visible(falsa, false),
        Err(DocumentError::UnknownLayer(falsa)),
    );
}

/// `D` con una capa `B` vacía encima. Los ids son siempre los mismos.
fn base() -> (Document, LayerId) {
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    (d, b)
}

#[test]
fn fila_12_c_la_igualdad_distingue_nombre_orden_opacidad_visibilidad_ids_y_vacia() {
    let (referencia, b) = base();
    assert_eq!(
        base().0,
        referencia,
        "dos construcciones iguales son iguales"
    );

    // Nombre, opacidad, visibilidad y orden.
    for (que, cmd) in [
        ("nombre", renombra(b, "Z")),
        ("opacidad", opacidad(b, 128)),
        ("visibilidad", visible(b, false)),
        ("orden", mueve(b, 0)),
    ] {
        let (mut d, _) = base();
        assert_eq!(d.apply(cmd), Ok(true), "{que}");
        assert_ne!(d, referencia, "{que}");
    }

    // Mismo contenido y nombres, otros ids: se añadió y quitó otra capa antes.
    let mut otros_ids = doc_d();
    let x = anade(&mut otros_ids, "X", 1);
    assert_eq!(otros_ids.apply(quita(x)), Ok(true));
    anade(&mut otros_ids, "B", 1);
    assert_eq!(nombres(&otros_ids), nombres(&referencia));
    assert_ne!(otros_ids, referencia, "ids distintos");

    // Celda vacía frente a celda con una imagen toda transparente.
    let (mut transparente, b2) = base();
    let cb = celda(&transparente, b2);
    assert_eq!(transparente.apply(pone(cb, 0, 0, C1)), Ok(true));
    assert_eq!(transparente.apply(pone(cb, 0, 0, T)), Ok(true));
    assert!(transparente.image(cb).is_some());
    assert_ne!(transparente, referencia, "vacía no es transparente");
}

#[test]
fn fila_12_d_la_igualdad_no_mira_la_historia() {
    // Mismo contenido e ids con historias distintas.
    let (mut uno, b) = base();
    assert_eq!(uno.apply(renombra(b, "X")), Ok(true));
    let (mut dos, _) = base();
    assert_eq!(dos.apply(renombra(b, "Y")), Ok(true));
    assert_eq!(dos.apply(renombra(b, "X")), Ok(true));
    assert_ne!(uno.history_bytes(), dos.history_bytes());
    assert_eq!(uno, dos);

    // Un documento al que se le deshizo todo es igual a uno recién construido, aunque tenga
    // pasos por rehacer.
    let (nuevo, b) = base();
    let mut deshecho = nuevo.clone();
    let cb = celda(&deshecho, b);
    assert_eq!(deshecho.apply(pone(cb, 0, 0, C1)), Ok(true));
    assert_eq!(deshecho.apply(pone(cb, 0, 0, T)), Ok(true));
    deshecho.undo().expect("deshacer");
    deshecho.undo().expect("deshacer");
    assert!(deshecho.can_redo());
    assert_eq!(deshecho, nuevo);
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

fn modo_de(tipo: u8) -> Mode {
    match tipo {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        2 => Mode::Gray,
        _ => Mode::Indexed,
    }
}

/// Un píxel válido del modo (en indexado, el índice es `semilla % modulo`, que cabe en la
/// paleta cuando `modulo` es su longitud).
fn pixel_valido(modo: Mode, semilla: u8, modulo: usize) -> Pixel {
    match modo {
        Mode::Rgba => Pixel::Rgba(color(semilla)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: (semilla % 3) * 100,
            alpha: if semilla.is_multiple_of(2) { 0 } else { 255 },
        },
        Mode::Gray => Pixel::Gray(semilla),
        Mode::Indexed => {
            Pixel::Indexed(u8::try_from(usize::from(semilla) % modulo).expect("cabe en u8"))
        }
    }
}

/// Un píxel de una orden generada: casi siempre válido, a veces de otro modo o (en indexado)
/// fuera de la paleta, para inyectar fallos.
fn pixel_op(modo: Mode, semilla: u8, paleta: usize) -> Pixel {
    if semilla.is_multiple_of(11) {
        return if modo == Mode::Rgba {
            Pixel::Gray(semilla)
        } else {
            Pixel::Rgba(color(semilla))
        };
    }
    pixel_valido(modo, semilla, paleta + 1)
}

/// Descripción de un documento inicial: de 1 a 4 capas, algunas con imagen y otras vacías.
#[derive(Clone, Debug)]
struct Plan {
    /// 0 RGBA, 1 grises con alfa, 2 grises, 3 indexado con índice transparente, 4 indexado al
    /// que se le quita el índice transparente después de añadir las capas.
    tipo: u8,
    w: u32,
    h: u32,
    paleta: usize,
    transparente: usize,
    capas: usize,
    /// Por capa: `None` vacía (la 0 siempre tiene imagen), `Some` píxeles `(x, y, semilla)`.
    contenido: Vec<Option<Vec<(u8, u8, u8)>>>,
}

fn plan_estrategia(
    capas: std::ops::RangeInclusive<usize>,
    con_grises: bool,
) -> impl Strategy<Value = Plan> {
    (
        0u8..5,
        1u32..=4,
        1u32..=4,
        1usize..=8,
        0usize..8,
        capas,
        prop::collection::vec(
            prop::option::of(prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..4)),
            4,
        ),
    )
        .prop_map(
            move |(tipo, w, h, paleta, transparente, capas, contenido)| {
                let tipo = if !con_grises && tipo == 2 { 3 } else { tipo };
                Plan {
                    tipo,
                    w,
                    h,
                    paleta,
                    transparente,
                    // En grises no se puede añadir capas (no hay valor transparente).
                    capas: if tipo == 2 { 1 } else { capas },
                    contenido,
                }
            },
        )
}

fn imagen_base(plan: &Plan) -> Image {
    match plan.tipo {
        0..=2 => {
            Image::new(plan.w, plan.h, modo_de(plan.tipo)).expect("imagen de un modo sin paleta")
        }
        _ => {
            let t = u8::try_from(plan.transparente % plan.paleta).expect("cabe en u8");
            img_indexada(plan.w, plan.h, &colores_distintos(plan.paleta), Some(t))
        }
    }
}

/// Construye el documento del plan solo con `Document::new`, `AddLayer` y escrituras.
fn construye(plan: &Plan) -> Document {
    let modo = modo_de(plan.tipo);
    let mut imagen = imagen_base(plan);
    if let Some(Some(pixeles)) = plan.contenido.first() {
        for &(x, y, semilla) in pixeles {
            let pixel = pixel_valido(modo, semilla, plan.paleta);
            imagen
                .set_pixel(u32::from(x) % plan.w, u32::from(y) % plan.h, pixel)
                .expect("píxel de la capa 0");
        }
    }
    let mut d = Document::new(imagen);
    for i in 1..plan.capas {
        assert_eq!(d.apply(anade_cmd(&format!("Capa {}", i + 1), i)), Ok(true));
        if let Some(Some(pixeles)) = plan.contenido.get(i)
            && !pixeles.is_empty()
        {
            let image = d.layers()[i].cel();
            let pixels = pixeles
                .iter()
                .map(|&(x, y, semilla)| PixelEdit {
                    x: u32::from(x) % plan.w,
                    y: u32::from(y) % plan.h,
                    pixel: pixel_valido(modo, semilla, plan.paleta),
                })
                .collect();
            d.apply(Command::SetPixels { image, pixels })
                .expect("escribir en la capa");
        }
    }
    if plan.tipo == 4 {
        let image = d.layers()[0].cel();
        assert_eq!(d.apply(sin_transparente(image)), Ok(true));
    }
    d
}

/// A qué se refiere una orden generada: la capa (o su celda) número `n % len`, o un id que no
/// existe.
#[derive(Clone, Debug)]
enum Obj {
    Nth(usize),
    Falso(u32),
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
    /// Celda, tramos `(y, x, largo)`, semilla del píxel y si se deja sin ordenar.
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
    LimiteHistoria(u8),
    LimiteCeldas(u8, usize),
}

fn obj(salvaje: bool) -> BoxedStrategy<Obj> {
    let falso = if salvaje {
        any::<u32>().boxed()
    } else {
        (1000u32..1010).boxed()
    };
    prop_oneof![
        6 => (0usize..8).prop_map(Obj::Nth),
        1 => falso.prop_map(Obj::Falso),
    ]
    .boxed()
}

fn posicion(salvaje: bool) -> BoxedStrategy<usize> {
    if salvaje {
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

fn nombre(salvaje: bool) -> BoxedStrategy<String> {
    let base = prop_oneof![
        6 => "[a-z]{0,8}",
        1 => Just("a".repeat(255)),
        1 => Just("a".repeat(256)),
        1 => Just("é".repeat(128)),
    ];
    if salvaje {
        prop_oneof![
            3 => base,
            1 => "\\PC{0,300}",
        ]
        .boxed()
    } else {
        base.boxed()
    }
}

fn opacidad_estrategia() -> BoxedStrategy<u8> {
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
fn hoja(salvaje: bool) -> BoxedStrategy<Op> {
    let pixeles = || prop::collection::vec((0u8..6, 0u8..6, any::<u8>()), 0..4);
    let tramos = || prop::collection::vec((0u8..5, 0u8..5, 1u8..4), 0..3);
    prop_oneof![
        6 => (obj(salvaje), pixeles()).prop_map(|(o, p)| Op::Set(o, p)),
        4 => (obj(salvaje), tramos(), any::<u8>(), prop::bool::weighted(0.15))
            .prop_map(|(o, t, s, d)| Op::Fill(o, t, s, d)),
        3 => (nombre(salvaje), posicion(salvaje)).prop_map(|(n, a)| Op::Add(n, a)),
        2 => obj(salvaje).prop_map(Op::Remove),
        2 => (obj(salvaje), posicion(salvaje)).prop_map(|(o, t)| Op::Move(o, t)),
        2 => (obj(salvaje), nombre(salvaje)).prop_map(|(o, n)| Op::Rename(o, n)),
        2 => (obj(salvaje), opacidad_estrategia()).prop_map(|(o, p)| Op::Opacity(o, p)),
        2 => (obj(salvaje), any::<bool>()).prop_map(|(o, v)| Op::Visible(o, v)),
        4 => (pal_op(), obj(salvaje)).prop_map(|(p, o)| Op::Pal(p, o)),
        1 => (obj(salvaje), 0u8..4).prop_map(|(o, m)| Op::Convert(o, m)),
    ]
    .boxed()
}

/// Una orden de nivel superior. `salvaje`: ids, posiciones y nombres arbitrarios, `Batch`
/// anidados. `con_historia`: añade `undo`, `redo` y los dos límites.
fn op(salvaje: bool, con_historia: bool) -> BoxedStrategy<Op> {
    let interior = if salvaje {
        prop_oneof![
            5 => hoja(true),
            1 => prop::collection::vec(hoja(true), 0..3).prop_map(Op::Batch),
        ]
        .boxed()
    } else {
        hoja(false)
    };
    let lote = prop::collection::vec(interior, 0..4).prop_map(Op::Batch);
    if con_historia {
        prop_oneof![
            14 => hoja(salvaje),
            3 => lote,
            2 => Just(Op::Undo),
            2 => Just(Op::Redo),
            1 => any::<u8>().prop_map(Op::LimiteHistoria),
            1 => (any::<u8>(), any::<usize>()).prop_map(|(k, v)| Op::LimiteCeldas(k, v)),
        ]
        .boxed()
    } else {
        prop_oneof![
            14 => hoja(salvaje),
            3 => lote,
        ]
        .boxed()
    }
}

fn celda_de(doc: &Document, o: &Obj) -> ImageId {
    match o {
        Obj::Nth(n) => doc
            .layers()
            .get(n % doc.layers().len().max(1))
            .map_or(ImageId::from_raw(0), Layer::cel),
        Obj::Falso(raw) => ImageId::from_raw(*raw),
    }
}

fn capa_de(doc: &Document, o: &Obj) -> LayerId {
    match o {
        Obj::Nth(n) => doc
            .layers()
            .get(n % doc.layers().len().max(1))
            .map_or(LayerId::from_raw(0), Layer::id),
        Obj::Falso(raw) => LayerId::from_raw(*raw),
    }
}

fn longitud_paleta(doc: &Document) -> usize {
    doc.palette().map_or(1, Palette::len)
}

fn comando_paleta(doc: &Document, p: &PalOp, image: ImageId) -> Command {
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
        PalOp::Permute(semilla) => {
            let n = longitud_paleta(doc);
            let mut order: Vec<u8> = (0..n).map(|i| u8::try_from(i).unwrap_or(0)).collect();
            if semilla % 2 == 1 {
                order.reverse();
            } else if !order.is_empty() {
                let k = usize::from(*semilla) % order.len();
                order.rotate_left(k);
            }
            if semilla % 7 == 0 {
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
fn a_comando(doc: &Document, orden: &Op) -> Option<Command> {
    let modo = doc.mode();
    let paleta = longitud_paleta(doc);
    Some(match orden {
        Op::Set(o, pixeles) => Command::SetPixels {
            image: celda_de(doc, o),
            pixels: pixeles
                .iter()
                .map(|&(x, y, s)| PixelEdit {
                    x: u32::from(x),
                    y: u32::from(y),
                    pixel: pixel_op(modo, s, paleta),
                })
                .collect(),
        },
        Op::Fill(o, tramos, s, sin_ordenar) => {
            let mut v: Vec<(u32, u32, u32)> = tramos
                .iter()
                .map(|&(y, x, largo)| (u32::from(y), u32::from(x), u32::from(x) + u32::from(largo)))
                .collect();
            if !*sin_ordenar {
                // Ordenados y sin solapes, salvo lo que se salga del lienzo.
                v.sort_unstable();
                let mut fin: Option<(u32, u32)> = None;
                v.retain(|&(y, x0, x1)| {
                    let vale = fin.is_none_or(|(py, px)| y > py || (y == py && x0 >= px));
                    if vale {
                        fin = Some((y, x1));
                    }
                    vale
                });
            }
            Command::FillSpans {
                image: celda_de(doc, o),
                spans: v
                    .into_iter()
                    .map(|(y, x0, x1)| Span {
                        y,
                        x_start: x0,
                        x_end: x1,
                    })
                    .collect(),
                pixel: pixel_op(modo, *s, paleta),
            }
        }
        Op::Batch(v) => Command::Batch(v.iter().filter_map(|o| a_comando(doc, o)).collect()),
        Op::Add(nom, at) => Command::AddLayer {
            name: nom.clone(),
            at: *at,
        },
        Op::Remove(o) => quita(capa_de(doc, o)),
        Op::Move(o, to) => mueve(capa_de(doc, o), *to),
        Op::Rename(o, nom) => renombra(capa_de(doc, o), nom),
        Op::Opacity(o, p) => opacidad(capa_de(doc, o), *p),
        Op::Visible(o, v) => visible(capa_de(doc, o), *v),
        Op::Pal(p, o) => comando_paleta(doc, p, celda_de(doc, o)),
        Op::Convert(o, m) => Command::ConvertMode {
            image: celda_de(doc, o),
            target: modo_de(*m % 4),
            allowed: Allowed {
                luma: true,
                drop_alpha: Some([0, 0, 0]),
            },
        },
        Op::Undo | Op::Redo | Op::LimiteHistoria(_) | Op::LimiteCeldas(..) => return None,
    })
}

enum Paso {
    Apply(Result<bool, DocumentError>),
    Deshacer(Result<(), DocumentError>),
    Rehacer(Result<(), DocumentError>),
    Limite,
}

fn ejecuta(doc: &mut Document, orden: &Op) -> Paso {
    match orden {
        Op::Undo => Paso::Deshacer(doc.undo()),
        Op::Redo => Paso::Rehacer(doc.redo()),
        Op::LimiteHistoria(k) => {
            doc.set_history_limit(match k % 4 {
                0 => 0,
                1 => 1,
                2 => 4096,
                _ => Document::DEFAULT_HISTORY_LIMIT,
            });
            Paso::Limite
        }
        Op::LimiteCeldas(k, valor) => {
            let celda = doc.cel_bytes();
            doc.set_cells_limit(match k % 6 {
                0 => 0,
                1 => celda,
                2 => celda.saturating_mul(2),
                3 => celda.saturating_mul(3),
                4 => usize::MAX,
                _ => *valor,
            });
            Paso::Limite
        }
        _ => match a_comando(doc, orden) {
            Some(comando) => Paso::Apply(doc.apply(comando)),
            None => Paso::Limite,
        },
    }
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

/// Lo que ha salido en algún `layers()` anterior, para la invariante (c).
#[derive(Default)]
struct Vistos {
    capa_a_celda: HashMap<LayerId, ImageId>,
    celda_a_capa: HashMap<ImageId, LayerId>,
    /// Se ha bajado `cells_limit()` alguna vez: (e) deja de exigirse.
    limite_bajado: bool,
}

/// Las invariantes (a) a (e) de la propiedad 2.
fn invariantes(doc: &Document, vistos: &mut Vistos) -> Result<(), TestCaseError> {
    let capas = doc.layers();
    // (d)
    prop_assert!(
        (1..=Document::MAX_LAYERS).contains(&capas.len()),
        "{} capas",
        capas.len()
    );
    let (mut ids_capa, mut ids_celda) = (HashSet::new(), HashSet::new());
    let mut suma = 0usize;
    for capa in capas {
        prop_assert!(capa.name().len() <= Document::MAX_LAYER_NAME_BYTES);
        // (c) únicos ahora y de la misma capa para siempre.
        prop_assert!(ids_capa.insert(capa.id()), "LayerId repetido");
        prop_assert!(ids_celda.insert(capa.cel()), "ImageId repetido");
        if let Some(antes) = vistos.capa_a_celda.insert(capa.id(), capa.cel()) {
            prop_assert_eq!(antes, capa.cel(), "la capa {:?} cambió de celda", capa.id());
        }
        if let Some(antes) = vistos.celda_a_capa.insert(capa.cel(), capa.id()) {
            prop_assert_eq!(antes, capa.id(), "la celda {:?} cambió de capa", capa.cel());
        }
        prop_assert_eq!(doc.layer(capa.id()), Some(capa));
        // (b)
        if doc.mode() == Mode::Indexed {
            prop_assert_eq!(capa.opacity(), 255, "opacidad en indexado");
        }
        // (a)
        if let Some(imagen) = doc.image(capa.cel()) {
            prop_assert_eq!(
                (imagen.width(), imagen.height(), imagen.mode()),
                (doc.width(), doc.height(), doc.mode())
            );
            prop_assert_eq!(
                imagen.palette(),
                doc.palette(),
                "paleta distinta del lienzo"
            );
            suma += imagen.bytes().len();
        }
    }
    prop_assert_eq!(doc.palette().is_some(), doc.mode() == Mode::Indexed);
    // (e)
    prop_assert_eq!(doc.materialized_bytes(), suma);
    prop_assert!(
        suma <= doc.cells_limit() || vistos.limite_bajado,
        "{suma} bytes materializados con límite {}",
        doc.cells_limit()
    );
    prop_assert_eq!(
        Ok(doc.cel_bytes()),
        Image::byte_len(doc.width(), doc.height(), doc.mode())
    );
    prop_assert_eq!(doc.worst_case_bytes(), capas.len() * doc.cel_bytes());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 1: tras una secuencia de comandos, deshacer cada `Ok(true)` devuelve el
    /// documento inicial y rehacerlos el final.
    #[test]
    fn propiedad_1_deshacer_y_rehacer_con_capas(
        plan in plan_estrategia(1..=4, true),
        ordenes in prop::collection::vec(op(false, false), 0..=30),
    ) {
        let mut doc = construye(&plan);
        // SPEC?: la propiedad dice «`undo()` hasta `Err`». Para tener varias capas el documento
        // inicial se construye con `AddLayer`, así que su historia no está vacía; se deshacen
        // exactamente los `Ok(true)` de la secuencia y se compara con el estado inicial (y
        // `history_bytes()` con el inicial, que es 0 si el plan no necesitó pasos).
        let inicial = Foto::de(&doc);
        let mut pasos = 0usize;
        for orden in &ordenes {
            if let Paso::Apply(Ok(true)) = ejecuta(&mut doc, orden) {
                pasos += 1;
            }
        }
        let final_ = Foto::de(&doc);

        for i in 0..pasos {
            prop_assert_eq!(doc.undo(), Ok(()), "deshacer {}", i);
        }
        inicial.exige_estado(&doc, "tras deshacer todo")?;
        prop_assert_eq!(doc.history_bytes(), inicial.historia);
        if !inicial.puede_deshacer {
            prop_assert_eq!(doc.history_bytes(), 0);
            prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
        }

        for i in 0..pasos {
            prop_assert_eq!(doc.redo(), Ok(()), "rehacer {}", i);
        }
        final_.exige_estado(&doc, "tras rehacer todo")?;
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    }

    /// Propiedad 2: las invariantes (a) a (e) valen tras cada operación, también las fallidas.
    #[test]
    fn propiedad_2_invariantes_tras_cualquier_operacion(
        plan in plan_estrategia(1..=4, true),
        ordenes in prop::collection::vec(op(false, true), 0..=30),
    ) {
        let mut doc = construye(&plan);
        let mut vistos = Vistos::default();
        invariantes(&doc, &mut vistos)?;
        for orden in &ordenes {
            let limite = doc.cells_limit();
            ejecuta(&mut doc, orden);
            if doc.cells_limit() < limite {
                vistos.limite_bajado = true;
            }
            invariantes(&doc, &mut vistos)?;
        }
    }

    /// Propiedad 3: ni un pánico, y lo que falla o no cambia no toca nada.
    #[test]
    fn propiedad_3_atomicidad_y_ausencia_de_panicos(
        plan in plan_estrategia(1..=4, true),
        ordenes in prop::collection::vec(op(true, true), 0..=30),
    ) {
        let mut doc = construye(&plan);
        for orden in &ordenes {
            let antes = Foto::de(&doc);
            let sin_cambio = match ejecuta(&mut doc, orden) {
                Paso::Apply(Ok(true)) | Paso::Limite => false,
                Paso::Apply(_) | Paso::Deshacer(Err(_)) | Paso::Rehacer(Err(_)) => true,
                Paso::Deshacer(Ok(())) | Paso::Rehacer(Ok(())) => false,
            };
            if sin_cambio {
                antes.exige(&doc, &format!("{orden:?}"))?;
            }
        }
    }

    /// Propiedad 5: quitar una capa que no es la última y deshacer devuelve el mismo
    /// documento, con los mismos ids.
    #[test]
    fn propiedad_5_quitar_una_capa_y_deshacer_devuelve_el_mismo_documento(
        plan in plan_estrategia(2..=4, false),
        eleccion in any::<usize>(),
    ) {
        let mut doc = construye(&plan);
        // SPEC?: en grises no se puede tener más de una capa (no hay valor transparente), así que
        // el plan nunca genera grises para esta propiedad.
        prop_assert!(doc.layers().len() >= 2);
        let antes = Foto::de(&doc);
        let ids_antes = ids(&doc);
        let capa = doc.layers()[eleccion % (doc.layers().len() - 1)].id();

        prop_assert_eq!(doc.apply(quita(capa)), Ok(true));
        prop_assert_eq!(doc.layers().len(), ids_antes.len() - 1);
        let tras = Foto::de(&doc);

        prop_assert_eq!(doc.undo(), Ok(()));
        antes.exige_estado(&doc, "tras deshacer el borrado")?;
        prop_assert_eq!(ids(&doc), ids_antes);

        prop_assert_eq!(doc.redo(), Ok(()));
        tras.exige_estado(&doc, "tras rehacer el borrado")?;
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: la tabla de capas contra un modelo independiente.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct CapaModelo {
    id: LayerId,
    cel: ImageId,
    nombre: String,
    visible: bool,
    opacidad: u8,
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
        3 => (nombre(false), posicion(false)).prop_map(|(n, a)| MOp::Add(n, a)),
        2 => obj(false).prop_map(MOp::Remove),
        2 => (obj(false), posicion(false)).prop_map(|(o, t)| MOp::Move(o, t)),
        2 => (obj(false), nombre(false)).prop_map(|(o, n)| MOp::Rename(o, n)),
        2 => (obj(false), opacidad_estrategia()).prop_map(|(o, p)| MOp::Opacity(o, p)),
        2 => (obj(false), any::<bool>()).prop_map(|(o, v)| MOp::Visible(o, v)),
        2 => Just(MOp::Undo),
        2 => Just(MOp::Redo),
    ]
    .boxed()
}

/// Documento de una capa 2 x 2 sin historia: 0 RGBA, 1 grises con alfa, 2 grises, 3 indexado
/// con índice transparente, 4 indexado sin él.
fn doc_modelo(tipo: u8) -> Document {
    Document::new(match tipo {
        0..=2 => Image::new(2, 2, modo_de(tipo)).expect("imagen"),
        3 => img_indexada(2, 2, &[ROJO, VERDE, AZUL], Some(1)),
        _ => img_indexada(2, 2, &[ROJO, VERDE, AZUL], None),
    })
}

struct Modelo {
    capas: Vec<CapaModelo>,
    /// Instantáneas de la tabla tras cada estado, y cuál es la actual.
    historia: Vec<Vec<CapaModelo>>,
    cursor: usize,
    /// Mayor id de capa y de celda visto hasta ahora (el contador de ids).
    max_capa: u32,
    max_celda: u32,
    modo: Mode,
    transparencia: bool,
}

impl Modelo {
    fn nuevo(tipo: u8) -> Modelo {
        let capas = vec![CapaModelo {
            id: LayerId::from_raw(0),
            cel: ImageId::from_raw(0),
            nombre: "Capa 1".to_string(),
            visible: true,
            opacidad: 255,
        }];
        Modelo {
            historia: vec![capas.clone()],
            capas,
            cursor: 0,
            max_capa: 0,
            max_celda: 0,
            modo: modo_de(tipo),
            // Grises e indexado sin índice transparente no tienen valor transparente.
            transparencia: !matches!(tipo, 2 | 4),
        }
    }

    /// Posición de la capa a la que se refiere `o` y su `LayerId`; `None` si es desconocida.
    fn objetivo(&self, o: &Obj) -> (LayerId, Option<usize>) {
        match o {
            Obj::Nth(n) => {
                let pos = n % self.capas.len();
                (self.capas[pos].id, Some(pos))
            }
            Obj::Falso(raw) => (LayerId::from_raw(*raw), None),
        }
    }

    /// Anota el cambio (si lo hubo) y compara la tabla del documento con el modelo.
    fn cierra(&mut self, doc: &Document, cambio: bool) -> Result<(), TestCaseError> {
        if cambio {
            self.historia.truncate(self.cursor + 1);
            self.historia.push(self.capas.clone());
            self.cursor += 1;
        }
        let reales = doc.layers();
        prop_assert_eq!(reales.len(), self.capas.len());
        for (real, esperada) in reales.iter().zip(&self.capas) {
            prop_assert_eq!(real.id(), esperada.id);
            prop_assert_eq!(real.cel(), esperada.cel);
            prop_assert_eq!(real.name(), esperada.nombre.as_str());
            prop_assert_eq!(real.visible(), esperada.visible);
            prop_assert_eq!(real.opacity(), esperada.opacidad);
        }
        Ok(())
    }

    fn paso(&mut self, doc: &mut Document, orden: &MOp) -> Result<(), TestCaseError> {
        let mut cambio = false;
        match orden {
            MOp::Add(nom, at) => {
                let len = self.capas.len();
                let esperado = if nom.len() > Document::MAX_LAYER_NAME_BYTES {
                    Err(DocumentError::LayerName {
                        len: nom.len(),
                        limit: 255,
                    })
                } else if !self.transparencia {
                    Err(DocumentError::NoTransparency { mode: self.modo })
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
                let real = doc.apply(anade_cmd(nom, *at));
                prop_assert_eq!(&real, &esperado, "{:?}", orden);
                if real == Ok(true) {
                    let nueva = doc.layers().get(*at).ok_or_else(|| {
                        TestCaseError::fail(format!("no hay capa en la posición {at}"))
                    })?;
                    // Los ids nuevos los reparte el documento, mayores que cualquiera visto.
                    prop_assert!(nueva.id().raw() > self.max_capa, "LayerId reutilizado");
                    prop_assert!(nueva.cel().raw() > self.max_celda, "ImageId reutilizado");
                    self.max_capa = nueva.id().raw();
                    self.max_celda = nueva.cel().raw();
                    self.capas.insert(
                        *at,
                        CapaModelo {
                            id: nueva.id(),
                            cel: nueva.cel(),
                            nombre: nom.clone(),
                            visible: true,
                            opacidad: 255,
                        },
                    );
                    cambio = true;
                }
            }
            MOp::Remove(o) => {
                let (id, pos) = self.objetivo(o);
                // SPEC?: con un id desconocido y una sola capa, la spec no dice si gana
                // `UnknownLayer` o `LastLayer`; esa combinación no se genera.
                if pos.is_none() && self.capas.len() == 1 {
                    return Ok(());
                }
                let esperado = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if self.capas.len() == 1 => Err(DocumentError::LastLayer),
                    Some(_) => Ok(true),
                };
                let real = doc.apply(quita(id));
                prop_assert_eq!(&real, &esperado, "{:?}", orden);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.capas.remove(pos);
                    cambio = true;
                }
            }
            MOp::Move(o, to) => {
                let (id, pos) = self.objetivo(o);
                // SPEC?: con un id desconocido y una posición fuera de rango, la spec no dice
                // qué error gana; con un id desconocido la posición se normaliza a 0.
                let to = if pos.is_none() { 0 } else { *to };
                let len = self.capas.len();
                let esperado = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if to >= len => Err(DocumentError::LayerIndex {
                        index: to,
                        max: len - 1,
                    }),
                    Some(pos) => Ok(pos != to),
                };
                let real = doc.apply(mueve(id, to));
                prop_assert_eq!(&real, &esperado, "{:?}", orden);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    let capa = self.capas.remove(pos);
                    self.capas.insert(to, capa);
                    cambio = true;
                }
            }
            MOp::Rename(o, nom) => {
                let (id, pos) = self.objetivo(o);
                // SPEC?: con un id desconocido y un nombre largo, la spec no dice qué error
                // gana; con un id desconocido el nombre se normaliza a uno corto.
                let nom = if pos.is_none() { "x" } else { nom.as_str() };
                let esperado = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if nom.len() > Document::MAX_LAYER_NAME_BYTES => {
                        Err(DocumentError::LayerName {
                            len: nom.len(),
                            limit: 255,
                        })
                    }
                    Some(pos) => Ok(self.capas[pos].nombre != nom),
                };
                let real = doc.apply(renombra(id, nom));
                prop_assert_eq!(&real, &esperado, "{:?}", orden);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.capas[pos].nombre = nom.to_string();
                    cambio = true;
                }
            }
            MOp::Opacity(o, opacidad_nueva) => {
                let (id, pos) = self.objetivo(o);
                let indexado = self.modo == Mode::Indexed;
                // SPEC?: con un id desconocido y opacidad ≠ 255 en indexado, la spec no dice qué
                // error gana; con un id desconocido en indexado la opacidad se normaliza a 255.
                let valor = if pos.is_none() && indexado {
                    255
                } else {
                    *opacidad_nueva
                };
                let esperado = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(_) if indexado && valor != 255 => Err(DocumentError::IndexedOpacity),
                    Some(pos) => Ok(self.capas[pos].opacidad != valor),
                };
                let real = doc.apply(opacidad(id, valor));
                prop_assert_eq!(&real, &esperado, "{:?}", orden);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.capas[pos].opacidad = valor;
                    cambio = true;
                }
            }
            MOp::Visible(o, v) => {
                let (id, pos) = self.objetivo(o);
                let esperado = match pos {
                    None => Err(DocumentError::UnknownLayer(id)),
                    Some(pos) => Ok(self.capas[pos].visible != *v),
                };
                let real = doc.apply(visible(id, *v));
                prop_assert_eq!(&real, &esperado, "{:?}", orden);
                if let (Ok(true), Some(pos)) = (&real, pos) {
                    self.capas[pos].visible = *v;
                    cambio = true;
                }
            }
            MOp::Undo => {
                let esperado = if self.cursor > 0 {
                    self.cursor -= 1;
                    self.capas = self.historia[self.cursor].clone();
                    Ok(())
                } else {
                    Err(DocumentError::NothingToUndo)
                };
                prop_assert_eq!(doc.undo(), esperado);
            }
            MOp::Redo => {
                let esperado = if self.cursor + 1 < self.historia.len() {
                    self.cursor += 1;
                    self.capas = self.historia[self.cursor].clone();
                    Ok(())
                } else {
                    Err(DocumentError::NothingToRedo)
                };
                prop_assert_eq!(doc.redo(), esperado);
            }
        }
        self.cierra(doc, cambio)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 4: la tabla de capas del documento coincide con un modelo independiente tras
    /// cada orden, con los mismos resultados y errores, y `undo`/`redo` siguen al cursor.
    #[test]
    fn propiedad_4_la_tabla_de_capas_contra_un_modelo(
        tipo in 0u8..5,
        ordenes in prop::collection::vec(mop(), 0..=30),
    ) {
        let mut doc = doc_modelo(tipo);
        let mut modelo = Modelo::nuevo(tipo);
        modelo.cierra(&doc, false)?;
        for orden in &ordenes {
            modelo.paso(&mut doc, orden)?;
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
    fn coincide(d: &Document, contexto: &str) {
        let plantilla = d.canvas_image();
        assert_eq!(plantilla.mode(), d.mode(), "{contexto}: modo");
        assert_eq!(plantilla.palette(), d.palette(), "{contexto}: paleta");
        assert_eq!(
            (plantilla.width(), plantilla.height()),
            (1, 1),
            "{contexto}: tamaño"
        );
    }

    /// (a) En los cuatro modos: modo y paleta del lienzo (`None` salvo indexado), 1 x 1.
    #[test]
    fn a_tiene_el_modo_y_la_paleta_del_lienzo_en_los_cuatro_modos() {
        let imagenes = [
            img_rgba(4, 3),
            Image::new(4, 3, Mode::Gray).expect("imagen de grises"),
            Image::new(4, 3, Mode::GrayAlpha).expect("imagen de grises con alfa"),
            img_indexada(4, 3, &[ROJO, VERDE, AZUL], Some(0)),
        ];
        let modos = [Mode::Rgba, Mode::Gray, Mode::GrayAlpha, Mode::Indexed];
        for (imagen, modo) in imagenes.into_iter().zip(modos) {
            let d = Document::new(imagen);
            assert_eq!(d.mode(), modo);
            coincide(&d, &format!("{modo:?}"));
            assert_eq!(d.canvas_image().palette().is_some(), modo == Mode::Indexed);
        }
    }

    /// (b) Sigue al lienzo en `apply`, `undo` y `redo`: comandos de paleta en indexado y
    /// `ConvertMode` de una capa (RGBA a indexado y vuelta).
    #[test]
    fn b_sigue_a_los_cambios_del_lienzo_en_apply_undo_y_redo() {
        // Paleta: color, entrada nueva e índice transparente.
        let mut d = Document::new(img_indexada(4, 3, &[ROJO, VERDE, AZUL], Some(0)));
        let cel = d.layers()[0].cel();
        let comandos = [
            Command::SetPaletteColor {
                image: cel,
                index: 1,
                color: BLANCO,
            },
            Command::AddPaletteEntry {
                image: cel,
                color: AMARILLO,
            },
            Command::SetTransparentIndex {
                image: cel,
                index: Some(2),
            },
        ];
        let mut paletas = vec![d.palette().cloned()];
        for (i, comando) in comandos.into_iter().enumerate() {
            assert_eq!(d.apply(comando), Ok(true), "comando {i}");
            coincide(&d, &format!("apply {i}"));
            assert_eq!(d.canvas_image().palette(), d.palette());
            paletas.push(d.palette().cloned());
        }
        assert!(paletas.windows(2).all(|p| p[0] != p[1]), "cada paso cambia");
        for i in (0..comandos_len(&paletas)).rev() {
            d.undo().expect("deshacer");
            coincide(&d, &format!("undo {i}"));
            assert_eq!(d.canvas_image().palette(), paletas[i].as_ref());
        }
        for (i, esperada) in paletas.iter().enumerate().skip(1) {
            d.redo().expect("rehacer");
            coincide(&d, &format!("redo {i}"));
            assert_eq!(d.canvas_image().palette(), esperada.as_ref());
        }

        // Conversión de modo de una capa: RGBA -> indexado y de vuelta.
        let mut d = doc_con_colores();
        let cel = d.layers()[0].cel();
        coincide(&d, "rgba");
        assert_eq!(d.canvas_image().mode(), Mode::Rgba);
        assert_eq!(d.apply(convertir(cel, Mode::Indexed)), Ok(true));
        coincide(&d, "apply a indexado");
        assert_eq!(d.canvas_image().mode(), Mode::Indexed);
        assert!(d.canvas_image().palette().is_some());
        assert_eq!(d.apply(convertir(cel, Mode::Rgba)), Ok(true));
        coincide(&d, "apply a rgba");
        assert_eq!(d.canvas_image().palette(), None);
        d.undo().expect("deshacer");
        coincide(&d, "undo a indexado");
        assert_eq!(d.canvas_image().mode(), Mode::Indexed);
        d.undo().expect("deshacer");
        coincide(&d, "undo a rgba");
        assert_eq!(d.canvas_image().mode(), Mode::Rgba);
        d.redo().expect("rehacer");
        coincide(&d, "redo a indexado");
        assert_eq!(d.canvas_image().mode(), Mode::Indexed);
        d.redo().expect("rehacer");
        coincide(&d, "redo a rgba");
        assert_eq!(d.canvas_image().mode(), Mode::Rgba);
    }

    fn comandos_len(paletas: &[Option<Palette>]) -> usize {
        paletas.len() - 1
    }

    /// (c) Sigue válida con todas las celdas vacías y no es ninguna celda: `image(cel)` es
    /// `None` aunque `canvas_image()` exista.
    #[test]
    fn c_sigue_valida_con_las_celdas_vacias() {
        // RGBA: dos capas y se quita la que tenía imagen (queda solo la vacía).
        let mut d = doc_d();
        let a = d.layers()[0].id();
        let b = anade(&mut d, "B", 1);
        assert_eq!(d.apply(quita(a)), Ok(true));
        assert_eq!(d.layers().len(), 1);
        assert!(
            d.image(celda(&d, b)).is_none(),
            "la celda restante es vacía"
        );
        coincide(&d, "rgba sin imagen");
        d.undo().expect("deshacer");
        coincide(&d, "rgba tras undo");

        // Indexado: capas vacías encima y la paleta cambia con todas las celdas vacías.
        let mut d = Document::new(img_indexada(4, 3, &[ROJO, VERDE, AZUL], Some(0)));
        let a = d.layers()[0].id();
        let b = anade(&mut d, "B", 1);
        let c = anade(&mut d, "C", 2);
        coincide(&d, "indexado con capas vacías encima");
        assert_eq!(d.apply(quita(a)), Ok(true));
        for capa in [b, c] {
            assert!(d.image(celda(&d, capa)).is_none());
        }
        coincide(&d, "indexado sin imágenes");
        let cb = celda(&d, b);
        assert_eq!(
            d.apply(Command::SetPaletteColor {
                image: cb,
                index: 1,
                color: BLANCO,
            }),
            Ok(true)
        );
        coincide(&d, "paleta editada con celdas vacías");
        assert_eq!(
            d.canvas_image().palette().expect("paleta").get(1),
            Ok(BLANCO)
        );
        assert!(d.image(cb).is_none(), "editar la paleta no materializa");
        d.undo().expect("deshacer");
        coincide(&d, "paleta deshecha con celdas vacías");
        assert_eq!(
            d.canvas_image().palette().expect("paleta").get(1),
            Ok(VERDE)
        );
    }

    /// (d) Es de solo lectura: no cambia `state_id`, `materialized_bytes()`, la historia ni la
    /// igualdad del documento (llamarla antes y después es inocuo).
    #[test]
    fn d_es_inocua_no_toca_estado_bytes_historia_ni_igualdad() {
        let (mut d, [_, b, _]) = tres_capas();
        assert_eq!(d.apply(pone(celda(&d, b), 1, 1, C1)), Ok(true));
        d.undo().expect("deshacer");
        let foto = Foto::de(&d);
        let copia = d.clone();

        let _ = d.canvas_image();
        foto.assert_igual(&d, "tras llamarla una vez");
        let _ = (d.canvas_image(), d.canvas_image());
        foto.assert_igual(&d, "tras llamarla varias veces");
        assert_eq!(d, copia);
        assert_eq!(copia, d);

        // Y el documento sigue evolucionando igual que su copia, sin haberla llamado.
        let mut sin_llamar = copia.clone();
        let comando = pone(celda(&d, b), 2, 2, C2);
        assert_eq!(d.apply(comando.clone()), Ok(true));
        assert_eq!(sin_llamar.apply(comando), Ok(true));
        let _ = d.canvas_image();
        assert_eq!(d, sin_llamar);
        assert_eq!(d.state_id(), sin_llamar.state_id());
        assert_eq!(d.materialized_bytes(), sin_llamar.materialized_bytes());
        assert_eq!(d.history_bytes(), sin_llamar.history_bytes());
    }
}

// ---------------------------------------------------------------------------------------
// Unidad 3.4, cambios P-01 a P-04 (specs/hito-3-capas-interfaz.md, «Lista exacta de cambios en
// `px-core`»): `Layer::locked()`, `Command::SetLayerLocked` y `Command::DuplicateLayer`. Filas 1
// y 2 de «Casos», propiedad 1 y condición 1 del visto bueno. Fase de pruebas en rojo: no
// compilan hasta que existan los dos comandos y `locked()`.
//
// Las cifras de bytes son las de la spec de 3.1 (RGBA 4 x 3: celda de 48 bytes, `tres_capas()`
// tiene la celda de `A` materializada y las de `B` y `C` vacías). Los errores de
// `DuplicateLayer` se comprueban con `sin_efecto`: ni documento, ni historia, ni `state_id`, ni
// `materialized_bytes()` cambian (es la forma observable de «antes de reservar»).
//
// SPEC?: «rehacer repone la copia sin copiar ni reservar celdas» no se prueba bajando el límite
// de celdas antes del `redo` (la spec no dice si `redo` de un paso con celda lo vuelve a
// comprobar, como hace `redo` de una escritura en la fila 11 b de 3.1); solo se prueba que
// rehacer deja el documento, el `state_id` y los bytes materializados del instante posterior.
// ---------------------------------------------------------------------------------------

mod duplicar_y_bloquear {
    use super::*;
    use px_core::BlendMode;

    fn bloquea(layer: LayerId, locked: bool) -> Command {
        Command::SetLayerLocked { layer, locked }
    }

    fn duplica(layer: LayerId, name: &str) -> Command {
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
    fn fila_01_a_bloquear_desbloquear_y_repetir_son_un_paso_cada_uno() {
        let (mut d, b) = base();
        let a = d.layers()[0].id();
        assert!(!d.layer(a).expect("A").locked(), "Document::new");
        assert!(!d.layer(b).expect("B").locked(), "AddLayer");

        let inicial = d.clone();
        let estado0 = d.state_id();
        let materializado = d.materialized_bytes();

        assert_eq!(d.apply(bloquea(b, true)), Ok(true));
        let estado1 = d.state_id();
        assert_ne!(estado1, estado0, "state_id nuevo al bloquear");
        assert!(d.layer(b).expect("B").locked());
        assert!(!d.layer(a).expect("A").locked(), "solo la pedida");
        assert_eq!(d.materialized_bytes(), materializado);
        // Dos documentos que solo difieren en el bloqueo no son iguales.
        assert_ne!(d, inicial);
        assert_ne!(inicial, d);
        let bloqueado = d.clone();

        // Repetir: `Ok(false)` sin tocar nada (ni historia ni `state_id`).
        sin_efecto(&mut d, bloquea(b, true), Ok(false));

        assert_eq!(d.apply(bloquea(b, false)), Ok(true));
        let estado2 = d.state_id();
        assert!(estado2 != estado1 && estado2 != estado0, "state_id nuevo");
        assert!(!d.layer(b).expect("B").locked());
        assert_eq!(d, inicial, "desbloquear deja un documento igual");
        sin_efecto(&mut d, bloquea(b, false), Ok(false));

        // Un paso por comando: dos `undo` y dos `redo`.
        d.undo().expect("deshacer el desbloqueo");
        assert_eq!((&d, d.state_id()), (&bloqueado, estado1));
        d.undo().expect("deshacer el bloqueo");
        assert_eq!((&d, d.state_id()), (&inicial, estado0));
        d.redo().expect("rehacer el bloqueo");
        assert_eq!((&d, d.state_id()), (&bloqueado, estado1));
        d.redo().expect("rehacer el desbloqueo");
        assert_eq!((&d, d.state_id()), (&inicial, estado2));
        assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));

        // Sirve en todos los modos, incluido gris (una sola capa).
        let imagenes = [
            img_rgba(4, 3),
            Image::new(4, 3, Mode::Gray).expect("gris"),
            Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"),
            img_indexada(4, 3, &[ROJO, VERDE], Some(1)),
        ];
        for imagen in imagenes {
            let modo = imagen.mode();
            let mut m = Document::new(imagen);
            let capa = m.layers()[0].id();
            let antes = m.clone();
            assert_eq!(m.apply(bloquea(capa, true)), Ok(true), "{modo:?}");
            assert!(m.layer(capa).expect("capa").locked(), "{modo:?}");
            assert_ne!(m, antes, "{modo:?}");
            m.undo().expect("deshacer");
            assert_eq!(m, antes, "{modo:?}");
            m.redo().expect("rehacer");
            assert!(m.layer(capa).expect("capa").locked(), "{modo:?}");
        }
    }

    /// (b) Capa desconocida (también una ya quitada) y dentro de un `Batch`.
    #[test]
    fn fila_01_b_capa_desconocida_y_en_un_lote_no_cambian_nada() {
        let (mut d, b) = base();
        let ca = d.main_image();
        let falsa = LayerId::from_raw(999);
        sin_efecto(
            &mut d,
            bloquea(falsa, true),
            Err(DocumentError::UnknownLayer(falsa)),
        );
        sin_efecto(
            &mut d,
            bloquea(falsa, false),
            Err(DocumentError::UnknownLayer(falsa)),
        );

        sin_efecto(
            &mut d,
            Command::Batch(vec![bloquea(b, true)]),
            Err(en_lote(0, DocumentError::NotBatchable)),
        );
        // También detrás de una escritura válida, que se revierte.
        sin_efecto(
            &mut d,
            Command::Batch(vec![pone(ca, 0, 0, C1), bloquea(b, true)]),
            Err(en_lote(1, DocumentError::NotBatchable)),
        );
        assert_eq!(pixel_rgba(&d, ca, 0, 0), T);
        assert!(!d.layer(b).expect("B").locked());

        // Una capa quitada ya no existe para el comando.
        assert_eq!(d.apply(quita(b)), Ok(true));
        sin_efecto(
            &mut d,
            bloquea(b, true),
            Err(DocumentError::UnknownLayer(b)),
        );
    }

    // -----------------------------------------------------------------------------------
    // Fila 2: `DuplicateLayer`.
    // -----------------------------------------------------------------------------------

    /// (a) Copia de una capa con píxeles en medio de `[A, B, C]`: va en `posición + 1`, con
    /// `LayerId` e `ImageId` nuevos (tampoco los de una capa ya quitada), imagen `==` en otra
    /// memoria, un solo paso, y editar una no toca la otra. También arriba del todo y abajo.
    #[test]
    fn fila_02_a_la_copia_va_encima_con_ids_nuevos_y_sin_compartir_memoria() {
        let (mut d, [a, b, c]) = tres_capas();
        assert_eq!(d.cel_bytes(), 48);
        let (ca, cb, cc) = (celda(&d, a), celda(&d, b), celda(&d, c));
        // Una capa añadida y quitada: sus ids no se reutilizan.
        let retirada = anade(&mut d, "X", 3);
        let celda_retirada = celda(&d, retirada);
        assert_eq!(d.apply(quita(retirada)), Ok(true));
        assert_eq!(d.apply(pone(cb, 1, 1, C1)), Ok(true));
        assert_eq!(d.apply(pone(cb, 3, 2, C2)), Ok(true));

        let antes = d.clone();
        let estado = d.state_id();
        let materializado = d.materialized_bytes();
        assert_eq!(materializado, 96, "A y B tienen celda");

        assert_eq!(d.apply(duplica(b, "B copia")), Ok(true));
        assert_eq!(d.layers().len(), 4);
        assert_eq!(d.layers()[0].id(), a);
        assert_eq!(d.layers()[1].id(), b);
        assert_eq!(d.layers()[3].id(), c);
        let copia = d.layers()[2].id();
        let cel_copia = celda(&d, copia);
        assert_eq!(d.layer(copia).expect("copia").name(), "B copia");
        assert!(![a, b, c, retirada].contains(&copia), "LayerId nuevo");
        assert!(
            ![ca, cb, cc, celda_retirada].contains(&cel_copia),
            "ImageId nuevo"
        );
        // Las demás capas siguen como estaban.
        assert_eq!(celda(&d, b), cb);
        assert_eq!(d.layer(b).expect("B").name(), "B");

        let original = d.image(cb).expect("B tiene imagen");
        let imagen_copia = d.image(cel_copia).expect("la copia tiene imagen");
        assert_eq!(original, imagen_copia);
        assert_ne!(
            original.bytes().as_ptr(),
            imagen_copia.bytes().as_ptr(),
            "no comparte memoria"
        );
        assert_eq!(d.materialized_bytes(), materializado + d.cel_bytes());

        // Un solo paso: deshacer deja el documento igual, y rehacer repone la copia.
        let despues = d.clone();
        let estado_despues = d.state_id();
        assert_ne!(estado_despues, estado);
        d.undo().expect("deshacer la copia");
        assert_eq!(d, antes);
        assert_eq!(d.state_id(), estado);
        assert_eq!(d.materialized_bytes(), materializado);
        assert!(d.image(cel_copia).is_none() && d.layer(copia).is_none());
        d.redo().expect("rehacer la copia");
        assert_eq!(d, despues);
        assert_eq!(d.state_id(), estado_despues);
        assert_eq!(d.materialized_bytes(), materializado + d.cel_bytes());
        assert_eq!(celda(&d, copia), cel_copia, "rehacer repone los mismos ids");

        // Editar una no toca la otra, en los dos sentidos.
        assert_eq!(d.apply(pone(cel_copia, 0, 0, C2)), Ok(true));
        assert_eq!(pixel_rgba(&d, cb, 0, 0), T);
        assert_eq!(d.apply(pone(cb, 2, 0, C1)), Ok(true));
        assert_eq!(pixel_rgba(&d, cel_copia, 2, 0), T);
        assert_eq!(pixel_rgba(&d, cel_copia, 1, 1), C1, "lo copiado sigue");

        // Arriba del todo: `posición + 1 == len`; y de la de abajo: va en la posición 1.
        assert_eq!(d.apply(duplica(c, "C copia")), Ok(true));
        assert_eq!(d.layers().len(), 5);
        assert_eq!(d.layers()[3].id(), c);
        assert_eq!(d.layers()[4].name(), "C copia");
        assert_eq!(d.apply(duplica(a, "A copia")), Ok(true));
        assert_eq!(d.layers().len(), 6);
        assert_eq!(d.layers()[0].id(), a);
        assert_eq!(d.layers()[1].name(), "A copia");
        assert_eq!(d.layers()[2].id(), b);
    }

    /// (b) La copia hereda visible, opacidad, mezcla y bloqueo; cambiarlos en una no toca la
    /// otra; una capa corriente da una copia corriente.
    #[test]
    fn fila_02_b_la_copia_hereda_opacidad_mezcla_visibilidad_y_bloqueo() {
        let (mut d, [_a, b, _c]) = tres_capas();
        let cb = celda(&d, b);
        assert_eq!(d.apply(pone(cb, 0, 0, C1)), Ok(true));
        assert_eq!(d.apply(opacidad(b, 128)), Ok(true));
        assert_eq!(
            d.apply(Command::SetLayerBlend {
                layer: b,
                blend: BlendMode::Multiply,
            }),
            Ok(true)
        );
        assert_eq!(d.apply(visible(b, false)), Ok(true));
        assert_eq!(d.apply(bloquea(b, true)), Ok(true));
        let original = d.layer(b).expect("B").clone();

        assert_eq!(d.apply(duplica(b, "B2")), Ok(true));
        let copia = d.layers()[2].id();
        {
            let l = d.layer(copia).expect("copia");
            assert_eq!(l.name(), "B2");
            assert!(!l.visible());
            assert_eq!(l.opacity(), 128);
            assert_eq!(l.blend(), BlendMode::Multiply);
            assert!(l.locked(), "la copia conserva el bloqueo");
        }
        assert_eq!(d.layer(b).expect("B"), &original, "el original no cambia");

        // Desbloquear la copia no desbloquea el original, y al revés.
        assert_eq!(d.apply(bloquea(copia, false)), Ok(true));
        assert!(d.layer(b).expect("B").locked());
        assert_eq!(d.apply(opacidad(b, 10)), Ok(true));
        assert_eq!(d.layer(copia).expect("copia").opacity(), 128);

        // Una capa corriente da una copia corriente.
        assert_eq!(d.apply(duplica(copia, "B3")), Ok(true));
        let tercera = d.layers()[3].id();
        let l = d.layer(tercera).expect("tercera");
        assert!(!l.locked(), "hereda el desbloqueo");
        assert_eq!(
            (l.opacity(), l.blend(), l.visible()),
            (128, BlendMode::Multiply, false)
        );
    }

    /// (c) Una celda vacía sigue vacía: `materialized_bytes()` no sube, aunque no quepa otra
    /// celda; deshacer y rehacer la dejan vacía; escribir en la copia no materializa la otra.
    #[test]
    fn fila_02_c_una_celda_vacia_sigue_vacia_y_no_reserva() {
        let (mut d, [_a, b, c]) = tres_capas();
        assert_eq!(d.apply(opacidad(b, 100)), Ok(true));
        assert_eq!(d.apply(bloquea(b, true)), Ok(true));
        let cb = celda(&d, b);
        let materializado = d.materialized_bytes();
        assert_eq!(materializado, 48);
        assert_eq!(d.worst_case_bytes(), 3 * 48);
        // El límite justo no deja materializar ninguna otra celda: la copia vacía cabe igual.
        d.set_cells_limit(materializado);
        let antes = d.clone();
        let estado = d.state_id();

        assert_eq!(d.apply(duplica(b, "vacía")), Ok(true));
        let copia = d.layers()[2].id();
        let cel_copia = celda(&d, copia);
        assert_ne!(cel_copia, cb);
        assert!(d.image(cb).is_none() && d.image(cel_copia).is_none());
        assert_eq!(d.materialized_bytes(), materializado);
        assert_eq!(d.worst_case_bytes(), 4 * 48);
        let l = d.layer(copia).expect("copia");
        assert_eq!((l.opacity(), l.locked()), (100, true));
        assert_eq!(d.layers()[3].id(), c);

        let despues = d.clone();
        d.undo().expect("deshacer");
        assert_eq!((&d, d.state_id()), (&antes, estado));
        d.redo().expect("rehacer");
        assert_eq!(d, despues);
        assert!(d.image(cel_copia).is_none());
        assert_eq!(d.materialized_bytes(), materializado);

        // Con sitio, escribir en la copia materializa solo la copia.
        d.set_cells_limit(1 << 20);
        assert_eq!(d.apply(pone(cel_copia, 0, 0, C1)), Ok(true));
        assert!(d.image(cb).is_none(), "el original sigue vacío");
        assert_eq!(d.materialized_bytes(), materializado + 48);
    }

    /// (d) Errores, en este orden y sin tocar nada: `UnknownLayer`, `LayerName`,
    /// `TooManyLayers`, `CelsTooLarge`; y `NotBatchable` dentro de un lote.
    #[test]
    fn fila_02_d_los_errores_salen_en_orden_y_sin_tocar_nada() {
        let falsa = LayerId::from_raw(999);
        let largo = "a".repeat(256);

        // Con 256 capas: UnknownLayer gana a LayerName y a TooManyLayers; LayerName a
        // TooManyLayers; y con un nombre bueno, TooManyLayers (con celda o sin ella, y aunque
        // tampoco quepa otra celda: gana a CelsTooLarge).
        let mut d = doc_256(false);
        let primera = d.layers()[0].id();
        let ultima = d.layers()[255].id();
        sin_efecto(
            &mut d,
            duplica(falsa, &largo),
            Err(DocumentError::UnknownLayer(falsa)),
        );
        sin_efecto(
            &mut d,
            duplica(primera, &largo),
            Err(DocumentError::LayerName {
                len: 256,
                limit: 255,
            }),
        );
        let con_celda = Err(DocumentError::TooManyLayers { limit: 256 });
        sin_efecto(&mut d, duplica(primera, "copia"), con_celda.clone());
        sin_efecto(&mut d, duplica(ultima, "copia"), con_celda.clone());
        d.set_cells_limit(d.materialized_bytes());
        sin_efecto(&mut d, duplica(primera, "copia"), con_celda);
        assert_eq!(d.layers().len(), 256);

        // LayerName antes que CelsTooLarge, con 255 bytes sí vale.
        let (mut d, [a, b, c]) = tres_capas();
        let cb = celda(&d, b);
        assert_eq!(d.apply(pone(cb, 1, 1, C1)), Ok(true));
        let materializado = d.materialized_bytes();
        assert_eq!(materializado, 96);
        d.set_cells_limit(materializado);
        sin_efecto(
            &mut d,
            duplica(b, &largo),
            Err(DocumentError::LayerName {
                len: 256,
                limit: 255,
            }),
        );
        // CelsTooLarge: `needed = materialized_bytes() + cel_bytes()`, solo si la celda tiene
        // imagen; una capa vacía sí se copia aunque no quepa otra celda.
        let sin_sitio = Err(DocumentError::CelsTooLarge {
            needed: 96 + 48,
            limit: 96,
        });
        sin_efecto(&mut d, duplica(b, "copia"), sin_sitio.clone());
        sin_efecto(&mut d, duplica(a, "copia"), sin_sitio);
        // Un byte menos de lo que hace falta sigue sin caber; justo lo que hace falta, sí.
        d.set_cells_limit(96 + 48 - 1);
        sin_efecto(
            &mut d,
            duplica(b, "copia"),
            Err(DocumentError::CelsTooLarge {
                needed: 144,
                limit: 143,
            }),
        );
        assert_eq!(
            d.apply(duplica(c, &"a".repeat(255))),
            Ok(true),
            "vacía y 255 bytes"
        );
        assert_eq!(
            d.apply(duplica(b, "copia")),
            Err(DocumentError::CelsTooLarge {
                needed: 144,
                limit: 143,
            })
        );
        d.set_cells_limit(144);
        assert_eq!(d.apply(duplica(b, "copia")), Ok(true));
        assert_eq!(d.materialized_bytes(), 144);

        // Dentro de un lote: NotBatchable, también detrás de una escritura que se revierte.
        let (mut d, [_a, b, _c]) = tres_capas();
        let ca = d.main_image();
        sin_efecto(
            &mut d,
            Command::Batch(vec![duplica(b, "copia")]),
            Err(en_lote(0, DocumentError::NotBatchable)),
        );
        sin_efecto(
            &mut d,
            Command::Batch(vec![pone(ca, 0, 0, C1), duplica(b, "copia")]),
            Err(en_lote(1, DocumentError::NotBatchable)),
        );
        assert_eq!(pixel_rgba(&d, ca, 0, 0), T);
        assert_eq!(d.layers().len(), 3);
    }

    /// Copia `origen` en `d` y comprueba la copia: imagen `==` en otra memoria, mismas
    /// propiedades, `+ cel_bytes()` y un solo paso que se deshace y se rehace.
    fn copia_exacta(mut d: Document, origen: LayerId, etiqueta: &str) {
        let antes = d.clone();
        let estado = d.state_id();
        let materializado = d.materialized_bytes();
        let p = d
            .layers()
            .iter()
            .position(|l| l.id() == origen)
            .expect("el origen existe");
        assert_eq!(d.apply(duplica(origen, "Copia")), Ok(true), "{etiqueta}");
        let (o, c) = (&d.layers()[p], &d.layers()[p + 1]);
        assert_eq!(c.name(), "Copia", "{etiqueta}");
        assert!(c.id() != o.id() && c.cel() != o.cel(), "{etiqueta}: ids");
        assert_eq!(
            (c.visible(), c.opacity(), c.blend(), c.locked()),
            (o.visible(), o.opacity(), o.blend(), o.locked()),
            "{etiqueta}: propiedades"
        );
        let io = d.image(o.cel()).expect("el original tiene imagen");
        let ic = d.image(c.cel()).expect("la copia tiene imagen");
        assert_eq!(io, ic, "{etiqueta}: imagen");
        assert_eq!(io.palette(), ic.palette(), "{etiqueta}: paleta");
        assert_ne!(
            io.bytes().as_ptr(),
            ic.bytes().as_ptr(),
            "{etiqueta}: memoria"
        );
        assert_eq!(d.materialized_bytes(), materializado + d.cel_bytes());
        let despues = d.clone();
        d.undo().expect("deshacer");
        assert_eq!((&d, d.state_id()), (&antes, estado), "{etiqueta}: undo");
        d.redo().expect("rehacer");
        assert_eq!(d, despues, "{etiqueta}: redo");
    }

    /// (e) Funciona en RGBA, grises con alfa, indexado (con transparente) y grises.
    #[test]
    fn fila_02_e_duplicar_funciona_en_rgba_gris_con_alfa_indexado_y_gris() {
        // RGBA.
        let (mut d, [_a, b, _c]) = tres_capas();
        let cb = celda(&d, b);
        assert_eq!(d.apply(pone(cb, 2, 1, C2)), Ok(true));
        copia_exacta(d, b, "RGBA");

        // Grises con alfa.
        let mut d = Document::new(Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"));
        let b = anade(&mut d, "B", 1);
        let pixel = Pixel::GrayAlpha {
            gray: 9,
            alpha: 200,
        };
        assert_eq!(d.apply(pone_px(celda(&d, b), 1, 2, pixel)), Ok(true));
        copia_exacta(d, b, "GrayAlpha");

        // Indexado con color transparente, con la capa bloqueada.
        let mut d = Document::new(img_indexada(4, 3, &[ROJO, VERDE, AZUL], Some(2)));
        let b = anade(&mut d, "B", 1);
        assert_eq!(
            d.apply(pone_px(celda(&d, b), 2, 1, Pixel::Indexed(1))),
            Ok(true)
        );
        assert_eq!(d.apply(bloquea(b, true)), Ok(true));
        copia_exacta(d, b, "Indexed");

        // Grises: no admite capas nuevas (no hay valor transparente), pero sí copiar.
        let mut d = Document::new(Image::new(4, 3, Mode::Gray).expect("gris"));
        let a = d.layers()[0].id();
        assert_eq!(
            d.apply(pone_px(d.main_image(), 1, 1, Pixel::Gray(77))),
            Ok(true)
        );
        assert!(
            d.apply(anade_cmd("B", 1)).is_err(),
            "gris no admite AddLayer"
        );
        copia_exacta(d, a, "Gray");
    }

    // -----------------------------------------------------------------------------------
    // Propiedad 1: duplicar y bloquear se deshacen exactamente.
    // -----------------------------------------------------------------------------------

    #[derive(Clone, Debug)]
    struct CapaPlan {
        opacidad: u8,
        mezcla: u8,
        visible: bool,
        bloqueada: bool,
        /// `None`: celda vacía; `Some`: píxeles `(x, y, semilla)` (puede seguir vacía si todos
        /// son transparentes).
        contenido: Option<Vec<(u8, u8, u8)>>,
    }

    #[derive(Clone, Debug)]
    struct DocPlan {
        /// 0 RGBA, 1 grises con alfa, 2 indexado con índice transparente.
        tipo: u8,
        w: u32,
        h: u32,
        paleta: usize,
        transparente: usize,
        /// Celdas que caben además de las ya materializadas.
        holgura: usize,
        capas: Vec<CapaPlan>,
    }

    fn capa_plan() -> impl Strategy<Value = CapaPlan> {
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
                |(opacidad, mezcla, visible, bloqueada, contenido)| CapaPlan {
                    opacidad,
                    mezcla,
                    visible,
                    bloqueada,
                    contenido,
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
            prop::collection::vec(capa_plan(), 1..=8),
        )
            .prop_map(
                |(tipo, w, h, paleta, transparente, holgura, capas)| DocPlan {
                    tipo,
                    w,
                    h,
                    paleta,
                    transparente,
                    holgura,
                    capas,
                },
            )
    }

    /// Lo que la secuencia sabe del documento: modo, tamaño y longitud de la paleta.
    struct Cx {
        modo: Mode,
        w: u32,
        h: u32,
        paleta: usize,
    }

    fn construye_doc(plan: &DocPlan) -> (Document, Cx) {
        let (imagen, modo) = match plan.tipo {
            0 => (img_rgba(plan.w, plan.h), Mode::Rgba),
            1 => (
                Image::new(plan.w, plan.h, Mode::GrayAlpha).expect("gris con alfa"),
                Mode::GrayAlpha,
            ),
            _ => {
                let t = u8::try_from(plan.transparente % plan.paleta).expect("cabe en u8");
                (
                    img_indexada(plan.w, plan.h, &colores_distintos(plan.paleta), Some(t)),
                    Mode::Indexed,
                )
            }
        };
        let cx = Cx {
            modo,
            w: plan.w,
            h: plan.h,
            paleta: plan.paleta,
        };
        let mut d = Document::new(imagen);
        for i in 1..plan.capas.len() {
            assert_eq!(d.apply(anade_cmd(&format!("Capa {}", i + 1), i)), Ok(true));
        }
        for (i, capa) in plan.capas.iter().enumerate() {
            let (id, image) = (d.layers()[i].id(), d.layers()[i].cel());
            if let Some(pixeles) = &capa.contenido {
                let pixels = pixeles
                    .iter()
                    .map(|&(x, y, semilla)| PixelEdit {
                        x: u32::from(x) % plan.w,
                        y: u32::from(y) % plan.h,
                        pixel: pixel_valido(modo, semilla, plan.paleta),
                    })
                    .collect();
                assert!(d.apply(Command::SetPixels { image, pixels }).is_ok());
            }
            if modo != Mode::Indexed {
                if capa.opacidad != 255 {
                    assert_eq!(d.apply(opacidad(id, capa.opacidad)), Ok(true));
                }
                let blend = BlendMode::ALL[usize::from(capa.mezcla) % 8];
                if blend != BlendMode::Normal {
                    assert_eq!(
                        d.apply(Command::SetLayerBlend { layer: id, blend }),
                        Ok(true)
                    );
                }
            }
            if !capa.visible {
                assert_eq!(d.apply(visible(id, false)), Ok(true));
            }
            if capa.bloqueada {
                assert_eq!(d.apply(bloquea(id, true)), Ok(true));
            }
        }
        // Un límite que a veces no deja materializar más celdas.
        d.set_cells_limit(d.materialized_bytes() + plan.holgura * d.cel_bytes());
        (d, cx)
    }

    #[derive(Clone, Debug)]
    enum Orden {
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

    fn orden() -> impl Strategy<Value = Orden> {
        (
            0u8..20,
            any::<usize>(),
            any::<usize>(),
            any::<u8>(),
            any::<bool>(),
            prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>()), 1..5),
        )
            .prop_map(|(tipo, k, t, v, b, px)| match tipo {
                0..=1 => Orden::Add(t),
                2..=6 => Orden::Dup(k),
                7..=8 => Orden::Remove(k),
                9..=10 => Orden::Move(k, t),
                11 => Orden::Rename(k, v),
                12 => Orden::Opacity(k, v),
                13 => Orden::Visible(k, b),
                14 => Orden::Blend(k, v),
                15..=16 => Orden::Locked(k, b),
                17..=18 => Orden::Set(k, px),
                _ => Orden::Fill(k, px[0].0, px[0].1, px[0].2, v),
            })
    }

    /// El comando de la orden con el documento vivo delante (los índices valen `% len`).
    fn traduce(doc: &Document, o: &Orden, cx: &Cx) -> Command {
        let capas = doc.layers();
        let n = capas.len();
        let id = |k: usize| capas[k % n].id();
        match o {
            Orden::Add(at) => anade_cmd(&format!("N{at}"), at % (n + 1)),
            Orden::Dup(k) => duplica(id(*k), &format!("Copia {}", k % 1000)),
            Orden::Remove(k) => quita(id(*k)),
            Orden::Move(k, to) => mueve(id(*k), to % n),
            Orden::Rename(k, s) => renombra(id(*k), &format!("R{s}")),
            Orden::Opacity(k, v) => opacidad(id(*k), *v),
            Orden::Visible(k, v) => visible(id(*k), *v),
            Orden::Blend(k, m) => Command::SetLayerBlend {
                layer: id(*k),
                blend: BlendMode::ALL[usize::from(*m) % 8],
            },
            Orden::Locked(k, v) => bloquea(id(*k), *v),
            Orden::Set(k, pixeles) => Command::SetPixels {
                image: capas[k % n].cel(),
                pixels: pixeles
                    .iter()
                    .map(|&(x, y, semilla)| PixelEdit {
                        x: u32::from(x) % cx.w,
                        y: u32::from(y) % cx.h,
                        pixel: pixel_valido(cx.modo, semilla, cx.paleta),
                    })
                    .collect(),
            },
            Orden::Fill(k, y, x0, largo, semilla) => {
                let x_start = u32::from(*x0) % cx.w;
                Command::FillSpans {
                    image: capas[k % n].cel(),
                    spans: vec![Span {
                        y: u32::from(*y) % cx.h,
                        x_start,
                        x_end: (x_start + 1 + u32::from(*largo)).min(cx.w),
                    }],
                    pixel: pixel_valido(cx.modo, *semilla, cx.paleta),
                }
            }
        }
    }

    /// Ids de capa y de celda que ya existieron alguna vez (los ids no se reutilizan).
    #[derive(Default)]
    struct Vistos {
        capas: HashSet<LayerId>,
        celdas: HashSet<ImageId>,
    }

    impl Vistos {
        fn registra(&mut self, doc: &Document) {
            for l in doc.layers() {
                self.capas.insert(l.id());
                self.celdas.insert(l.cel());
            }
        }
    }

    /// `materialized_bytes()` es la suma real de las celdas con imagen y cabe en el límite; los
    /// ids de capa y de celda no se repiten.
    fn invariantes(doc: &Document) -> Result<(), TestCaseError> {
        let real: usize = doc
            .layers()
            .iter()
            .filter_map(|l| doc.image(l.cel()))
            .map(|i| i.bytes().len())
            .sum();
        prop_assert_eq!(doc.materialized_bytes(), real);
        prop_assert!(doc.materialized_bytes() <= doc.cells_limit());
        let capas: HashSet<LayerId> = doc.layers().iter().map(Layer::id).collect();
        let celdas: HashSet<ImageId> = doc.layers().iter().map(Layer::cel).collect();
        prop_assert_eq!(capas.len(), doc.layers().len());
        prop_assert_eq!(celdas.len(), doc.layers().len());
        Ok(())
    }

    type Resumen<'a> = (
        LayerId,
        ImageId,
        &'a str,
        bool,
        u8,
        BlendMode,
        bool,
        Option<&'a Image>,
    );

    fn resumen<'a>(doc: &'a Document, l: &'a Layer) -> Resumen<'a> {
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
    fn otro_pixel(cx: &Cx, actual: Pixel) -> Option<Pixel> {
        (1..=8u8)
            .map(|s| pixel_valido(cx.modo, s, cx.paleta))
            .find(|p| *p != actual)
    }

    /// Tras una copia `Ok(true)`: el original y las demás capas no cambian, la copia tiene ids
    /// nuevos, el nombre dado, las propiedades del original y su imagen en otra memoria; y
    /// editar una no toca la otra.
    fn comprueba_copia(
        doc: &mut Document,
        antes: &Document,
        origen: LayerId,
        nombre: &str,
        cx: &Cx,
        vistos: &Vistos,
        estados: &mut Vec<u64>,
    ) -> Result<(), TestCaseError> {
        let p = antes
            .layers()
            .iter()
            .position(|l| l.id() == origen)
            .expect("el origen existía");
        prop_assert_eq!(doc.layers().len(), antes.layers().len() + 1);
        {
            let previas: Vec<_> = antes.layers().iter().map(|l| resumen(antes, l)).collect();
            let resto: Vec<_> = doc
                .layers()
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != p + 1)
                .map(|(_, l)| resumen(doc, l))
                .collect();
            prop_assert_eq!(
                previas,
                resto,
                "la copia no cambia al original ni a las demás"
            );
        }
        let (original, copia) = (&doc.layers()[p], &doc.layers()[p + 1]);
        prop_assert_eq!(copia.name(), nombre);
        prop_assert!(!vistos.capas.contains(&copia.id()), "LayerId nuevo");
        prop_assert!(!vistos.celdas.contains(&copia.cel()), "ImageId nuevo");
        prop_assert_eq!(
            (
                copia.visible(),
                copia.opacity(),
                copia.blend(),
                copia.locked()
            ),
            (
                original.visible(),
                original.opacity(),
                original.blend(),
                original.locked()
            )
        );
        let (cel_o, cel_c) = (original.cel(), copia.cel());
        let (io, ic) = (doc.image(cel_o), doc.image(cel_c));
        match (io, ic) {
            (None, None) => {
                prop_assert_eq!(doc.materialized_bytes(), antes.materialized_bytes());
                return Ok(());
            }
            (Some(io), Some(ic)) => {
                prop_assert_eq!(io, ic);
                prop_assert_ne!(io.bytes().as_ptr(), ic.bytes().as_ptr());
                prop_assert_eq!(
                    doc.materialized_bytes(),
                    antes.materialized_bytes() + doc.cel_bytes()
                );
            }
            otro => {
                return Err(TestCaseError::fail(format!(
                    "la celda vacía y la copia deben coincidir: {otro:?}"
                )));
            }
        }

        // Editar la copia no toca al original; editar el original no toca a la copia.
        let actual = doc
            .image(cel_c)
            .expect("copia")
            .get_pixel(0, 0)
            .expect("(0, 0)");
        if let Some(pixel) = otro_pixel(cx, actual) {
            let original_antes = doc.image(cel_o).expect("original").clone();
            let edicion = pone_px(cel_c, 0, 0, pixel);
            if doc.apply(edicion) == Ok(true) {
                estados.push(doc.state_id());
            }
            prop_assert_eq!(doc.image(cel_o), Some(&original_antes));
            let copia_antes = doc.image(cel_c).expect("copia").clone();
            let actual = doc
                .image(cel_o)
                .expect("original")
                .get_pixel(0, 0)
                .expect("(0, 0)");
            if let Some(pixel) = otro_pixel(cx, actual) {
                let edicion = pone_px(cel_o, 0, 0, pixel);
                if doc.apply(edicion) == Ok(true) {
                    estados.push(doc.state_id());
                }
                prop_assert_eq!(doc.image(cel_c), Some(&copia_antes));
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
        fn propiedad_1_duplicar_y_bloquear_se_deshacen_exactamente(
            plan in doc_plan(),
            ordenes in prop::collection::vec(orden(), 0..=25),
        ) {
            let (mut doc, cx) = construye_doc(&plan);
            invariantes(&doc)?;
            let inicial = doc.clone();
            let mut estados = vec![doc.state_id()];
            let mut vistos = Vistos::default();
            vistos.registra(&doc);

            for o in &ordenes {
                let antes = doc.clone();
                let comando = traduce(&doc, o, &cx);
                let copiada = match &comando {
                    Command::DuplicateLayer { layer, name } => Some((*layer, name.clone())),
                    _ => None,
                };
                let un_paso = copiada.is_some() || matches!(comando, Command::SetLayerLocked { .. });
                let resultado = doc.apply(comando);
                invariantes(&doc)?;
                match resultado {
                    Ok(true) => {
                        prop_assert!(!estados.contains(&doc.state_id()), "state_id nuevo");
                        estados.push(doc.state_id());
                        if un_paso {
                            let mut c = doc.clone();
                            prop_assert_eq!(c.undo(), Ok(()));
                            prop_assert!(c == antes, "un solo paso de historia");
                            prop_assert_eq!(c.state_id(), antes.state_id());
                        }
                        if let Some((origen, nombre)) = copiada {
                            comprueba_copia(
                                &mut doc, &antes, origen, &nombre, &cx, &vistos, &mut estados,
                            )?;
                        }
                        vistos.registra(&doc);
                    }
                    Ok(false) | Err(_) => {
                        prop_assert!(doc == antes, "sin cambios: {:?}", resultado);
                        prop_assert_eq!(doc.state_id(), antes.state_id());
                        prop_assert_eq!(doc.history_bytes(), antes.history_bytes());
                        prop_assert_eq!(doc.can_redo(), antes.can_redo());
                    }
                }
            }

            let final_ = doc.clone();
            let pasos = estados.len() - 1;
            for i in (0..pasos).rev() {
                prop_assert_eq!(doc.undo(), Ok(()), "deshacer {}", i);
                prop_assert_eq!(doc.state_id(), estados[i], "state_id al deshacer a {}", i);
                invariantes(&doc)?;
            }
            prop_assert!(doc == inicial, "deshacer hasta el principio");
            prop_assert_eq!(doc.materialized_bytes(), inicial.materialized_bytes());

            for (i, estado) in estados.iter().enumerate().take(pasos + 1).skip(1) {
                prop_assert_eq!(doc.redo(), Ok(()), "rehacer {}", i);
                prop_assert_eq!(doc.state_id(), *estado, "state_id al rehacer a {}", i);
                invariantes(&doc)?;
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
        nombre: &str,
        visible: bool,
        opacity: u8,
        blend: BlendMode,
        locked: bool,
        image: Option<Image>,
    ) -> LayerSpec {
        LayerSpec {
            name: nombre.to_string(),
            visible,
            opacity,
            blend,
            locked,
            image,
        }
    }

    /// Capa visible, opaca, Normal y desbloqueada.
    fn simple(nombre: &str, image: Option<Image>) -> LayerSpec {
        spec(nombre, true, 255, BlendMode::Normal, false, image)
    }

    fn rgba_con(w: u32, h: u32, pixeles: &[(u32, u32, Rgba)]) -> Image {
        let mut imagen = img_rgba(w, h);
        for &(x, y, c) in pixeles {
            imagen.set_pixel(x, y, Pixel::Rgba(c)).expect("píxel");
        }
        imagen
    }

    /// (a) Un documento RGBA correcto: ids `0..n`, historia vacía, `state_id` 0, propiedades y
    /// celdas tal cual, y `==` con el construido a mano con comandos.
    #[test]
    fn fila_01_a_rgba_ids_historia_vacia_state_id_0_e_igual_al_construido_a_mano() {
        let a = rgba_con(4, 3, &[(0, 0, ROJO), (3, 2, VERDE)]);
        let c = rgba_con(4, 3, &[(1, 1, AZUL)]);
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
        for (i, capa) in d.layers().iter().enumerate() {
            assert_eq!(capa.id(), LayerId::from_raw(i as u32), "LayerId({i})");
            assert_eq!(capa.cel(), ImageId::from_raw(i as u32), "ImageId({i})");
        }
        let nombres: Vec<&str> = d.layers().iter().map(Layer::name).collect();
        assert_eq!(nombres, ["fondo", "vacía", "arriba"]);
        let tercera = &d.layers()[2];
        assert_eq!(
            (
                tercera.visible(),
                tercera.opacity(),
                tercera.blend(),
                tercera.locked()
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
        let mut a_mano = Document::new(a);
        let (id0, id1, id2) = (
            LayerId::from_raw(0),
            LayerId::from_raw(1),
            LayerId::from_raw(2),
        );
        for orden in [
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
            pone_px(ImageId::from_raw(2), 1, 1, Pixel::Rgba(AZUL)),
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
            assert!(a_mano.apply(orden).is_ok());
        }
        assert_eq!(a_mano.layer(id1).map(Layer::name), Some("vacía"));
        assert!(a_mano == d, "el construido a mano es igual");
        assert!(d == a_mano);
    }

    /// (b) Indexado con paleta, y los límites exactos se aceptan: 256 capas, nombre de 255 bytes.
    #[test]
    fn fila_01_b_indexado_y_limites_exactos_se_aceptan() {
        let colores = [ROJO, VERDE, AZUL];
        let paleta = |t: Option<u8>| {
            let mut p = Palette::from_colors(colores).expect("paleta");
            p.set_transparent_index(t).expect("índice transparente");
            p
        };
        let p = paleta(Some(0));
        let imagen = Image::filled_indexed(4, 3, p.clone(), 2).expect("imagen");
        let d = Document::from_layers(
            4,
            3,
            Mode::Indexed,
            Some(p.clone()),
            vec![simple("a", Some(imagen.clone())), simple("b", None)],
        )
        .expect("indexado correcto");
        assert_eq!(d.mode(), Mode::Indexed);
        assert_eq!(d.palette(), Some(&p));
        assert_eq!(d.image(d.layers()[0].cel()), Some(&imagen));
        assert_eq!(d.image(d.layers()[1].cel()), None);
        assert_eq!(
            (d.state_id(), d.can_undo(), d.history_bytes()),
            (0, false, 0)
        );

        // 256 capas de 1 × 1 y un nombre de 255 bytes.
        let nombre = "ñ".repeat(127) + "a";
        assert_eq!(nombre.len(), 255);
        let mut capas: Vec<LayerSpec> = (0..255).map(|i| simple(&format!("c{i}"), None)).collect();
        capas.push(simple(&nombre, Some(img_rgba(1, 1))));
        let d = Document::from_layers(1, 1, Mode::Rgba, None, capas).expect("256 capas");
        assert_eq!(d.layers().len(), 256);
        assert_eq!(d.layers()[255].name(), nombre);
        assert_eq!(d.layers()[255].id(), LayerId::from_raw(255));
    }

    /// (c) Cada defecto aislado da su error y no se construye ningún documento.
    #[test]
    fn fila_01_c_cada_defecto_da_su_error_y_ningun_documento() {
        let p3 = |t: Option<u8>| {
            let mut p = Palette::from_colors([ROJO, VERDE, AZUL]).expect("paleta");
            p.set_transparent_index(t).expect("índice transparente");
            p
        };
        let indexada =
            |paleta: &Palette| Image::filled_indexed(4, 3, paleta.clone(), 1).expect("imagen");
        let otra = Palette::from_colors([ROJO, VERDE, BLANCO]).expect("otra paleta");
        let ok_rgba = || simple("ok", Some(img_rgba(4, 3)));

        #[allow(clippy::type_complexity)]
        let casos: Vec<(&str, Result<Document, DocumentError>, DocumentError)> = vec![
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
                    vec![simple("x", Some(indexada(&otra)))],
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
                        Some(indexada(&p3(Some(0)))),
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
                        Some(indexada(&p3(Some(0)))),
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
        for (nombre, resultado, esperado) in casos {
            assert_eq!(resultado, Err(esperado), "{nombre}");
        }
    }

    /// Unidad L1, fila 6: un lienzo imposible (ancho o alto 0, más de `Image::MAX_BYTES`) o una
    /// paleta que no corresponde al modo es `InvalidCanvas`, aunque las capas sean válidas; una
    /// celda que no encaja sigue siendo `CelMismatch`, y los dos errores se leen distinto.
    #[test]
    fn fila_06_lienzo_imposible_es_invalid_canvas_y_celda_que_no_encaja_es_cel_mismatch() {
        let paleta = Palette::from_colors([ROJO, VERDE, AZUL]).expect("paleta");
        let casos: Vec<(&str, Result<Document, DocumentError>)> = vec![
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
                Document::from_layers(4, 3, Mode::Rgba, Some(paleta), vec![simple("a", None)]),
            ),
            (
                "indexado sin paleta",
                Document::from_layers(4, 3, Mode::Indexed, None, vec![simple("a", None)]),
            ),
        ];
        for (nombre, resultado) in casos {
            assert_eq!(
                resultado.err(),
                Some(DocumentError::InvalidCanvas),
                "{nombre}"
            );
        }
        let celda = Document::from_layers(
            4,
            3,
            Mode::Rgba,
            None,
            vec![simple("a", Some(img_rgba(3, 3)))],
        )
        .err()
        .expect("la celda no encaja");
        assert_eq!(celda, DocumentError::CelMismatch { layer: 0 });
        assert_ne!(
            DocumentError::InvalidCanvas.to_string(),
            celda.to_string(),
            "los dos mensajes son distintos"
        );
    }

    // -----------------------------------------------------------------------------------
    // P2: `from_layers` no construye documentos inválidos.
    // -----------------------------------------------------------------------------------

    /// Una celda de 1 a 4 píxeles de lado y de cualquiera de los cuatro modos; `variante` decide
    /// cuánto se parece a lo que pide el lienzo.
    fn celda_de(variante: u8, w: u32, h: u32, modo: Mode, paleta: Option<&Palette>) -> Image {
        let otra = Palette::from_colors([NEGRO, BLANCO]).expect("otra paleta");
        let resultado = match variante {
            // Coincide en todo (si se puede).
            0 => match (modo, paleta) {
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
                if modo == Mode::Gray {
                    Mode::GrayAlpha
                } else {
                    Mode::Gray
                },
            ),
            // Otra paleta.
            _ => Image::new_indexed(w, h, otra),
        };
        resultado.expect("celda de la prueba")
    }

    /// Las invariantes de un documento válido (spec, P2).
    fn invariantes_p2(d: &Document) -> Result<(), TestCaseError> {
        let n = d.layers().len();
        prop_assert!((1..=Document::MAX_LAYERS).contains(&n), "{} capas", n);
        for (i, capa) in d.layers().iter().enumerate() {
            prop_assert!(
                capa.name().len() <= Document::MAX_LAYER_NAME_BYTES,
                "nombre de {}",
                i
            );
            prop_assert_eq!(capa.id(), LayerId::from_raw(i as u32));
            prop_assert_eq!(capa.cel(), ImageId::from_raw(i as u32));
            if d.mode() == Mode::Indexed {
                prop_assert_eq!(capa.opacity(), 255);
                prop_assert_eq!(capa.blend(), BlendMode::Normal);
            }
            if let Some(imagen) = d.image(capa.cel()) {
                prop_assert_eq!(
                    (imagen.width(), imagen.height(), imagen.mode()),
                    (d.width(), d.height(), d.mode())
                );
                prop_assert_eq!(imagen.palette(), d.palette());
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
        fn propiedad_2_from_layers_no_construye_documentos_invalidos(
            (w, h) in (1u32..=4, 1u32..=4),
            modo in prop::sample::select(vec![Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha]),
            con_paleta in 0u8..4,
            transparente in prop::option::of(0u8..2),
            cantidad in prop_oneof![8 => 0usize..=5, 1 => Just(256usize), 1 => Just(257usize), 1 => Just(300usize)],
            semillas in prop::collection::vec(
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
            // `con_paleta`: 0 no hay paleta, 1 y 2 una de dos colores, 3 otra de un color.
            let paleta = match con_paleta {
                0 => None,
                3 => Some(Palette::from_colors([ROJO]).expect("paleta")),
                _ => {
                    let mut p = Palette::from_colors([ROJO, VERDE]).expect("paleta");
                    p.set_transparent_index(transparente).expect("transparente");
                    Some(p)
                }
            };
            let capas: Vec<LayerSpec> = semillas
                .iter()
                .take(cantidad)
                .map(|(largo, opacidad, mezcla, (visible, bloqueada), variante)| {
                    // Las muy numerosas llevan celdas vacías o de 1 × 1 para no pesar.
                    let imagen = variante.map(|v| celda_de(v, w, h, modo, paleta.as_ref()));
                    spec(&"a".repeat(*largo), *visible, *opacidad, BlendMode::ALL[*mezcla], *bloqueada, imagen)
                })
                .collect();
            if let Ok(d) = Document::from_layers(w, h, modo, paleta, capas) {
                invariantes_p2(&d)?;
            }
        }
    }
}
