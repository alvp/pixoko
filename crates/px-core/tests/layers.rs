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
        for i in 1..paletas.len() {
            d.redo().expect("rehacer");
            coincide(&d, &format!("redo {i}"));
            assert_eq!(d.canvas_image().palette(), paletas[i].as_ref());
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
