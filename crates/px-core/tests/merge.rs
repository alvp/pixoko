//! Tests de `Command::MergeLayers` y `Document::plan_merge` (specs/hito-3-combinar.md, unidad
//! 3.5, cambios `C-01` a `C-08`, `C-13` y `C-07`): quitar capas, poner la imagen de la celda de
//! la que se queda e imponerle opacidad 255, mezcla Normal y desbloqueada, todo en un solo paso
//! de deshacer.
//!
//! Filas 1 y 2 de «Casos» (`fila_NN_x`, un caso por prueba) y las propiedades 2 (ida y vuelta,
//! parte de combinar; también mezcla `ConvertAll` y `ConvertMode` en las secuencias) y 5
//! (deshacer una combinación devuelve las capas tal cual). La imagen compuesta la calcula
//! `px-raster` en producción; aquí es una imagen válida cualquiera del lienzo (`px-core` no
//! comprueba que sea la composición, solo su tamaño, modo y paleta). Las cifras de las filas son
//! calculadas a mano (RGBA 4 x 3: una celda son 48 bytes).
//!
//! Este fichero es la fase de pruebas en rojo: `MergeLayers`, `ConvertAll`, `plan_merge`,
//! `MergePlan` y los tres errores nuevos no existen todavía, así que no compila hasta la lógica
//! de `px-core` de la spec.
//!
//! Los `// SPEC?:` marcan dudas que la spec no resuelve; ahí la prueba solo comprueba lo que la
//! spec dice y evita lo demás.

use std::ops::RangeInclusive;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, BlendMode, Command, Document, DocumentError, Image, ImageId, Layer, LayerId,
    MergePlan, Mode, Palette, Pixel, PixelEdit, Rgba, Span,
};

// ---------------------------------------------------------------------------------------
// Colores, imágenes y documentos de apoyo.
// ---------------------------------------------------------------------------------------

const T: Rgba = Rgba::TRANSPARENT;
const C1: Rgba = Rgba::new(40, 50, 60, 255);
const C2: Rgba = Rgba::new(70, 80, 90, 255);
const ROJO: Rgba = Rgba::new(255, 0, 0, 255);
const VERDE: Rgba = Rgba::new(0, 255, 0, 255);
const AZUL: Rgba = Rgba::new(0, 0, 255, 255);

fn img_rgba(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Rgba).expect("imagen RGBA")
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

/// Documento RGBA de 4 x 3 recién creado.
fn doc_d() -> Document {
    Document::new(img_rgba(4, 3))
}

fn anade_cmd(nombre: &str, at: usize) -> Command {
    Command::AddLayer {
        name: nombre.to_string(),
        at,
    }
}

/// Añade una capa en `at` y devuelve su `LayerId`.
fn anade(doc: &mut Document, nombre: &str, at: usize) -> LayerId {
    assert_eq!(
        doc.apply(anade_cmd(nombre, at)),
        Ok(true),
        "añadir {nombre}"
    );
    doc.layers()[at].id()
}

fn celda(doc: &Document, capa: LayerId) -> ImageId {
    doc.layer(capa).expect("la capa existe").cel()
}

fn ids(doc: &Document) -> Vec<(LayerId, ImageId)> {
    doc.layers().iter().map(|l| (l.id(), l.cel())).collect()
}

fn pone(image: ImageId, x: u32, y: u32, c: Rgba) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit {
            x,
            y,
            pixel: Pixel::Rgba(c),
        }],
    }
}

/// Rellena la fila 0 de una celda de 4 de ancho.
fn fila0(image: ImageId, c: Rgba) -> Command {
    Command::FillSpans {
        image,
        spans: vec![Span {
            y: 0,
            x_start: 0,
            x_end: 4,
        }],
        pixel: Pixel::Rgba(c),
    }
}

fn renombra(layer: LayerId, name: &str) -> Command {
    Command::RenameLayer {
        layer,
        name: name.to_string(),
    }
}

/// Fija opacidad, mezcla, visibilidad y bloqueo de una capa.
fn ajusta(
    doc: &mut Document,
    capa: LayerId,
    opacity: u8,
    blend: BlendMode,
    visible: bool,
    locked: bool,
) {
    for cmd in [
        Command::SetLayerOpacity {
            layer: capa,
            opacity,
        },
        Command::SetLayerBlend { layer: capa, blend },
        Command::SetLayerVisible {
            layer: capa,
            visible,
        },
        Command::SetLayerLocked {
            layer: capa,
            locked,
        },
    ] {
        doc.apply(cmd).expect("propiedad de capa");
    }
}

/// `[A, B, C]`, RGBA 4 x 3, las tres con imagen y propiedades todas distintas:
/// `A` opacidad 200, Multiplicar, visible, desbloqueada; `B` 128, Pantalla, oculta, bloqueada;
/// `C` 90, Diferencia, visible, bloqueada. Sus nombres son «Capa 1», «B» y «C».
fn doc_abc() -> (Document, [LayerId; 3]) {
    let mut img = img_rgba(4, 3);
    img.set_pixel(0, 0, Pixel::Rgba(C1)).expect("píxel");
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    let (cb, cc) = (celda(&d, b), celda(&d, c));
    assert_eq!(d.apply(fila0(cb, C2)), Ok(true));
    assert_eq!(d.apply(pone(cc, 1, 1, ROJO)), Ok(true));
    ajusta(&mut d, a, 200, BlendMode::Multiply, true, false);
    ajusta(&mut d, b, 128, BlendMode::Screen, false, true);
    ajusta(&mut d, c, 90, BlendMode::Difference, true, true);
    (d, [a, b, c])
}

/// Imagen RGBA 4 x 3 de un solo color: la «composición» que viaja en el comando.
fn imagen_nueva(tag: u8) -> Image {
    Image::filled(4, 3, Pixel::Rgba(Rgba::new(tag, 20, 30, 200))).expect("imagen")
}

fn combina(doc: &Document, target: LayerId, remove: &[LayerId], image: Option<Image>) -> Command {
    Command::MergeLayers {
        target,
        remove: remove.to_vec(),
        image,
        based_on: doc.state_id(),
    }
}

/// Todo lo observable que un comando fallido o sin cambios no puede tocar.
struct Foto {
    doc: Document,
    puede_deshacer: bool,
    puede_rehacer: bool,
    historia: usize,
    estado: u64,
    materializado: usize,
    ids: Vec<(LayerId, ImageId)>,
    modo: Mode,
    paleta: Option<Palette>,
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
            ids: ids(doc),
            modo: doc.mode(),
            paleta: doc.palette().cloned(),
        }
    }

    /// Solo el contenido: documento, ids, modo y paleta del lienzo, `state_id` y bytes
    /// materializados.
    fn diferencia_estado(&self, doc: &Document) -> Option<&'static str> {
        if *doc != self.doc {
            Some("el documento cambió")
        } else if ids(doc) != self.ids {
            Some("los ids o su orden cambiaron")
        } else if doc.mode() != self.modo {
            Some("el modo del lienzo cambió")
        } else if doc.palette() != self.paleta.as_ref() {
            Some("la paleta del lienzo cambió")
        } else if doc.state_id() != self.estado {
            Some("state_id cambió")
        } else if doc.materialized_bytes() != self.materializado {
            Some("materialized_bytes cambió")
        } else {
            None
        }
    }

    /// Además de lo anterior, la historia y las pilas.
    fn diferencia(&self, doc: &Document) -> Option<&'static str> {
        self.diferencia_estado(doc).or_else(|| {
            if doc.can_undo() != self.puede_deshacer {
                Some("can_undo cambió")
            } else if doc.can_redo() != self.puede_rehacer {
                Some("can_redo cambió")
            } else if doc.history_bytes() != self.historia {
                Some("history_bytes cambió")
            } else {
                None
            }
        })
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
}

/// Aplica `cmd`, comprueba el resultado y que nada observable cambió.
fn sin_efecto(doc: &mut Document, cmd: Command, esperado: Result<bool, DocumentError>) {
    let foto = Foto::de(doc);
    let contexto = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), esperado, "{contexto}");
    foto.assert_igual(doc, &contexto);
}

fn en_lote(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

macro_rules! exige {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(format!($($arg)+));
        }
    };
}

/// Aplica una `MergeLayers` válida y comprueba todo lo que dice la spec (filas 1 y 2 y
/// propiedad 5): estructura tras aplicar, propiedades de `target`, memoria, historia, y que
/// `undo` y `redo` devuelven exactamente las capas de antes y de después. El documento acaba en
/// el estado posterior a la combinación (tras el `redo`).
fn aplica_y_comprueba(
    doc: &mut Document,
    target: LayerId,
    remove: &[LayerId],
    image: Option<Image>,
) -> Result<(), String> {
    let antes = Foto::de(doc);
    let capas_antes = doc.layers().to_vec();
    let guardado: usize = capas_antes
        .iter()
        .filter(|l| l.id() == target || remove.contains(&l.id()))
        .filter_map(|l| doc.image(l.cel()))
        .map(|i| i.bytes().len())
        .sum();
    let nueva = image.as_ref().map_or(0, |i| i.bytes().len());

    let resultado = doc.apply(combina(doc, target, remove, image.clone()));
    exige!(resultado == Ok(true), "apply dio {resultado:?}");
    exige!(doc.state_id() != antes.estado, "state_id no es nuevo");
    exige!(doc.can_undo() && !doc.can_redo(), "pilas tras aplicar");

    // Las capas que quedan, en el mismo orden relativo; las quitadas desaparecen con su celda.
    let esperadas: Vec<LayerId> = capas_antes
        .iter()
        .map(Layer::id)
        .filter(|id| !remove.contains(id))
        .collect();
    let reales: Vec<LayerId> = doc.layers().iter().map(Layer::id).collect();
    exige!(
        reales == esperadas,
        "capas {reales:?} en vez de {esperadas:?}"
    );
    for quitada in remove {
        let vieja = capas_antes.iter().find(|l| l.id() == *quitada);
        exige!(doc.layer(*quitada).is_none(), "{quitada:?} sigue");
        if let Some(vieja) = vieja {
            exige!(
                doc.image(vieja.cel()).is_none(),
                "{quitada:?}: su celda sigue"
            );
        }
    }
    // Las demás capas, intactas.
    for capa in doc.layers() {
        if capa.id() == target {
            continue;
        }
        let vieja = capas_antes.iter().find(|l| l.id() == capa.id());
        exige!(vieja == Some(capa), "{:?} cambió", capa.id());
    }
    // `target`: mismas identidad, nombre y visibilidad; opacidad 255, Normal y desbloqueada; la
    // imagen del comando.
    let vieja = capas_antes
        .iter()
        .find(|l| l.id() == target)
        .ok_or_else(|| "target no estaba".to_string())?;
    let nueva_capa = doc
        .layer(target)
        .ok_or_else(|| "target desapareció".to_string())?;
    exige!(nueva_capa.cel() == vieja.cel(), "ImageId de target cambió");
    exige!(nueva_capa.name() == vieja.name(), "nombre de target cambió");
    exige!(
        nueva_capa.visible() == vieja.visible(),
        "visibilidad de target cambió"
    );
    exige!(
        nueva_capa.opacity() == 255
            && nueva_capa.blend() == BlendMode::Normal
            && !nueva_capa.locked(),
        "target no quedó con 255, Normal y desbloqueada"
    );
    exige!(
        doc.image(nueva_capa.cel()) == image.as_ref(),
        "la celda de target no es la imagen del comando"
    );
    // Memoria y coste de historia.
    exige!(
        doc.materialized_bytes() == antes.materializado - guardado + nueva,
        "materialized_bytes {} (antes {}, guardado {guardado}, nueva {nueva})",
        doc.materialized_bytes(),
        antes.materializado
    );
    exige!(
        doc.history_bytes() >= antes.historia + guardado,
        "history_bytes {} < {} + {guardado}",
        doc.history_bytes(),
        antes.historia
    );

    // Un solo paso: un `undo` devuelve las capas tal cual, y un `redo`, la combinación.
    let despues = Foto::de(doc);
    doc.undo().map_err(|e| format!("undo: {e:?}"))?;
    exige!(
        doc.layers() == capas_antes.as_slice(),
        "tras undo las capas no son las de antes"
    );
    if let Some(motivo) = antes.diferencia_estado(doc) {
        return Err(format!("tras undo: {motivo}"));
    }
    doc.redo().map_err(|e| format!("redo: {e:?}"))?;
    if let Some(motivo) = despues.diferencia_estado(doc) {
        return Err(format!("tras redo: {motivo}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Fila 1: validación de MergeLayers y plan_merge.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_01_a_capas_desconocidas_remove_invalido_y_lote() {
    let (mut d, [a, b, c]) = doc_abc();
    let (falsa, otra) = (LayerId::from_raw(999), LayerId::from_raw(998));
    // La combinación de partida es válida (se comprueba sobre una copia).
    let mut copia = d.clone();
    let valida = combina(&d, a, &[b, c], Some(imagen_nueva(1)));
    assert_eq!(copia.apply(valida), Ok(true));

    let con = |target: LayerId, remove: &[LayerId], based_on: u64| Command::MergeLayers {
        target,
        remove: remove.to_vec(),
        image: Some(imagen_nueva(1)),
        based_on,
    };
    let estado = d.state_id();

    // `UnknownLayer`: primero `target`, luego `remove` en orden.
    sin_efecto(
        &mut d,
        con(falsa, &[b], estado),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    sin_efecto(
        &mut d,
        con(a, &[b, falsa], estado),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    sin_efecto(
        &mut d,
        con(a, &[otra, falsa], estado),
        Err(DocumentError::UnknownLayer(otra)),
    );
    sin_efecto(
        &mut d,
        con(falsa, &[otra], estado),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    // `UnknownLayer` gana a `InvalidMerge` (remove vacío, o con `target`).
    sin_efecto(
        &mut d,
        con(falsa, &[], estado),
        Err(DocumentError::UnknownLayer(falsa)),
    );
    sin_efecto(
        &mut d,
        con(a, &[a, falsa], estado),
        Err(DocumentError::UnknownLayer(falsa)),
    );

    // `InvalidMerge`: remove vacío, con repetidas o con `target`; gana a `StaleImage`.
    sin_efecto(
        &mut d,
        con(a, &[], estado),
        Err(DocumentError::InvalidMerge),
    );
    sin_efecto(
        &mut d,
        con(a, &[b, b], estado),
        Err(DocumentError::InvalidMerge),
    );
    sin_efecto(
        &mut d,
        con(a, &[b, a], estado),
        Err(DocumentError::InvalidMerge),
    );
    sin_efecto(
        &mut d,
        con(a, &[a], estado + 1000),
        Err(DocumentError::InvalidMerge),
    );

    // Dentro de un `Batch` no se puede, sola o detrás de una escritura válida.
    let ca = celda(&d, a);
    sin_efecto(
        &mut d,
        Command::Batch(vec![con(a, &[b, c], estado)]),
        Err(en_lote(0, DocumentError::NotBatchable)),
    );
    sin_efecto(
        &mut d,
        Command::Batch(vec![pone(ca, 2, 2, C2), con(a, &[b, c], estado)]),
        Err(en_lote(1, DocumentError::NotBatchable)),
    );
}

#[test]
fn fila_01_b_una_imagen_vieja_da_stale_image_sin_tocar_nada() {
    let (mut d, [a, b, c]) = doc_abc();
    let based_on = d.state_id();
    let cmd = combina(&d, a, &[b, c], Some(imagen_nueva(1)));

    // El documento cambia entre calcular el comando y aplicarlo.
    assert_eq!(d.apply(renombra(a, "otro nombre")), Ok(true));
    let current = d.state_id();
    assert_ne!(current, based_on);
    sin_efecto(
        &mut d,
        cmd.clone(),
        Err(DocumentError::StaleImage { based_on, current }),
    );

    // Lo mismo con una escritura de píxeles, y con una imagen que además no encaja: el estado
    // viejo se detecta antes que el desajuste de la imagen.
    let cb = celda(&d, b);
    assert_eq!(d.apply(pone(cb, 3, 2, ROJO)), Ok(true));
    let current = d.state_id();
    sin_efecto(
        &mut d,
        cmd.clone(),
        Err(DocumentError::StaleImage { based_on, current }),
    );
    let desajustada = Command::MergeLayers {
        target: a,
        remove: vec![b, c],
        image: Some(img_rgba(9, 9)),
        based_on,
    };
    sin_efecto(
        &mut d,
        desajustada,
        Err(DocumentError::StaleImage { based_on, current }),
    );

    // Volver al estado del que salió el comando lo hace válido otra vez.
    d.undo().expect("deshacer la escritura");
    d.undo().expect("deshacer el nombre");
    assert_eq!(d.state_id(), based_on);
    assert_eq!(d.apply(cmd), Ok(true));
}

#[test]
fn fila_01_c_la_imagen_debe_tener_el_ancho_alto_modo_y_paleta_del_lienzo() {
    // RGBA: otro ancho, otro alto y otros modos.
    let (mut d, [a, b, c]) = doc_abc();
    let con = |d: &Document, imagen: Option<Image>| combina(d, a, &[b, c], imagen);
    let malas = [
        img_rgba(5, 3),
        img_rgba(4, 4),
        img_rgba(1, 1),
        Image::new(4, 3, Mode::Gray).expect("gris"),
        Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"),
        img_indexada(4, 3, &[ROJO, VERDE], Some(0)),
    ];
    for mala in malas {
        let cmd = con(&d, Some(mala));
        sin_efecto(&mut d, cmd, Err(DocumentError::MergeImageMismatch));
    }

    // Indexado: la paleta ha de ser idéntica, índice transparente incluido.
    let paleta = [ROJO, VERDE, AZUL];
    let mut di = Document::new(img_indexada(4, 3, &paleta, Some(0)));
    let (ia, ib) = (di.layers()[0].id(), anade(&mut di, "B", 1));
    let con_i = |d: &Document, imagen: Option<Image>| combina(d, ia, &[ib], imagen);
    let malas = [
        img_indexada(4, 3, &[ROJO, VERDE, C2], Some(0)),
        img_indexada(4, 3, &paleta, Some(1)),
        img_indexada(4, 3, &paleta, None),
        img_indexada(4, 3, &[ROJO, VERDE], Some(0)),
        img_indexada(4, 3, &[ROJO, VERDE, AZUL, T], Some(0)),
        img_rgba(4, 3),
    ];
    for mala in malas {
        let cmd = con_i(&di, Some(mala));
        sin_efecto(&mut di, cmd, Err(DocumentError::MergeImageMismatch));
    }

    // Con la paleta idéntica vale, y `None` (celda vacía) vale siempre y no se comprueba.
    let buena = Image::filled_indexed(4, 3, di.palette().expect("paleta").clone(), 2)
        .expect("imagen indexada");
    let cmd = con_i(&di, Some(buena));
    assert_eq!(di.apply(cmd), Ok(true));
    let (mut d2, [a2, b2, _]) = doc_abc();
    let cmd = combina(&d2, a2, &[b2], None);
    assert_eq!(d2.apply(cmd), Ok(true));
}

#[test]
fn fila_01_d_cels_too_large_mira_el_resultado_y_solo_si_crece() {
    // Un documento al tope de celdas SÍ puede aplanarse: el resultado (48) cabe aunque el pico
    // (144 + 48) no.
    let (mut d, [a, b, c]) = doc_abc();
    assert_eq!(d.materialized_bytes(), 144);
    d.set_cells_limit(144);
    aplica_y_comprueba(&mut d, a, &[b, c], Some(imagen_nueva(1))).expect("aplanar al tope");

    // El resultado supera el tope y crece: A tiene imagen (48), B y C están vacías, y la imagen
    // nueva de B suma 48 más.
    let mut d = doc_d();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    d.set_cells_limit(48);
    let cmd = combina(&d, b, &[c], Some(imagen_nueva(1)));
    sin_efecto(
        &mut d,
        cmd,
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        }),
    );
    // El desajuste de la imagen se detecta antes que el tope.
    let cmd = combina(&d, b, &[c], Some(img_rgba(9, 9)));
    sin_efecto(&mut d, cmd, Err(DocumentError::MergeImageMismatch));
    // Con el tope justo en el resultado, vale.
    d.set_cells_limit(96);
    let cmd = combina(&d, b, &[c], Some(imagen_nueva(1)));
    assert_eq!(d.apply(cmd), Ok(true));

    // Pasarse del tope sin crecer respecto a hoy vale: hay 144 bytes, el resultado es 96 y el
    // tope es 48.
    let (mut d, [a, b, _]) = doc_abc();
    d.set_cells_limit(48);
    aplica_y_comprueba(&mut d, a, &[b], Some(imagen_nueva(1))).expect("no crece");

    // Un resultado igual a lo que hay hoy (48 - 48 + 48) con el tope en 0 tampoco crece.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    d.set_cells_limit(0);
    assert_eq!(d.materialized_bytes(), 48);
    let cmd = combina(&d, b, &[a], Some(imagen_nueva(1)));
    assert_eq!(d.apply(cmd), Ok(true));
    assert_eq!(d.materialized_bytes(), 48);
}

#[test]
fn fila_01_e_plan_merge_es_de_solo_lectura_y_da_bytes_vacia_y_pico() {
    // [A con imagen, B vacía, C con imagen, D vacía]: dos celdas de 48 bytes.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    let dd = anade(&mut d, "D", 3);
    let cc = celda(&d, c);
    assert_eq!(d.apply(pone(cc, 0, 0, ROJO)), Ok(true));
    let foto = Foto::de(&d);
    let pico = d.materialized_bytes() + d.cel_bytes();
    assert_eq!(pico, 96 + 48);

    let plan = d.plan_merge(a, &[b]).expect("plan");
    assert_eq!(
        plan,
        MergePlan {
            stored_bytes: 48,
            all_empty: false,
            peak_bytes: pico,
        }
    );
    let copia = plan; // `Copy`
    assert_eq!(copia, plan);
    // Cuenta las celdas de `target` y las de `remove`, no las demás.
    assert_eq!(d.plan_merge(b, &[c]).expect("plan").stored_bytes, 48);
    assert_eq!(d.plan_merge(a, &[b, c]).expect("plan").stored_bytes, 96);
    assert_eq!(d.plan_merge(b, &[dd]).expect("plan").stored_bytes, 0);
    // `all_empty`: ninguna de las celdas de `target` y `remove` tiene imagen.
    let vacia = d.plan_merge(b, &[dd]).expect("plan");
    assert!(vacia.all_empty);
    assert_eq!(vacia.peak_bytes, pico);
    assert!(!d.plan_merge(b, &[c]).expect("plan").all_empty);
    assert!(!d.plan_merge(a, &[dd]).expect("plan").all_empty);

    // Mismos errores que `apply`, en el mismo orden.
    let (falsa, otra) = (LayerId::from_raw(999), LayerId::from_raw(998));
    assert_eq!(
        d.plan_merge(falsa, &[b]),
        Err(DocumentError::UnknownLayer(falsa))
    );
    assert_eq!(
        d.plan_merge(a, &[otra, falsa]),
        Err(DocumentError::UnknownLayer(otra))
    );
    assert_eq!(d.plan_merge(a, &[]), Err(DocumentError::InvalidMerge));
    assert_eq!(d.plan_merge(a, &[b, b]), Err(DocumentError::InvalidMerge));
    assert_eq!(d.plan_merge(a, &[b, a]), Err(DocumentError::InvalidMerge));
    assert_eq!(
        d.plan_merge(falsa, &[]),
        Err(DocumentError::UnknownLayer(falsa))
    );
    // No tocó nada.
    foto.assert_igual(&d, "plan_merge");
}

// ---------------------------------------------------------------------------------------
// Fila 2: MergeLayers sobre [A, B, C].
// ---------------------------------------------------------------------------------------

#[test]
fn fila_02_a_quitar_la_de_arriba_y_los_ids_siguen_creciendo() {
    let (mut d, [_, b, c]) = doc_abc();
    let cc = celda(&d, c);
    aplica_y_comprueba(&mut d, b, &[c], Some(imagen_nueva(1))).expect("combinar C en B");

    // Lo que dice la spec, explícito: quedan A y B; B conserva id, celda, nombre y que estaba
    // oculta (aunque tenía 128, Pantalla y bloqueo).
    assert_eq!(d.layers().len(), 2);
    let capa_b = d.layer(b).expect("B sigue");
    assert_eq!(capa_b.name(), "B");
    assert!(!capa_b.visible());
    assert_eq!(
        (capa_b.opacity(), capa_b.blend(), capa_b.locked()),
        (255, BlendMode::Normal, false)
    );
    assert_eq!(d.image(capa_b.cel()), Some(&imagen_nueva(1)));
    assert!(d.layer(c).is_none());

    // Los ids de la quitada no se reutilizan, tampoco tras deshacer y rehacer.
    let nueva = anade(&mut d, "Z", 0);
    let z = d.layer(nueva).expect("Z");
    assert!(z.id().raw() > c.raw(), "LayerId reutilizado");
    assert!(z.cel().raw() > cc.raw(), "ImageId reutilizado");
}

#[test]
fn fila_02_b_quitar_dos_o_la_de_abajo_en_cualquier_posicion() {
    // La de abajo se queda y se quitan B y C.
    let (mut d, [a, b, c]) = doc_abc();
    aplica_y_comprueba(&mut d, a, &[b, c], Some(imagen_nueva(2))).expect("A <- B y C");
    assert_eq!(d.layers().len(), 1);
    assert_eq!(d.layers()[0].name(), "Capa 1");

    // El orden de `remove` no importa.
    let (mut d, [a, b, c]) = doc_abc();
    aplica_y_comprueba(&mut d, a, &[c, b], Some(imagen_nueva(2))).expect("A <- C y B");

    // Se queda la de arriba y se quita la de abajo (posiciones reinsertadas al deshacer).
    let (mut d, [a, _, c]) = doc_abc();
    aplica_y_comprueba(&mut d, c, &[a], Some(imagen_nueva(3))).expect("C <- A");
    assert_eq!(d.layers().len(), 2);

    // Se queda la oculta y bloqueada B (conserva su visibilidad y queda desbloqueada) y se
    // quita la de abajo; luego se queda A y se quita la de arriba, con B en medio de las dos
    // al deshacer.
    let (mut d, [a, b, _]) = doc_abc();
    aplica_y_comprueba(&mut d, b, &[a], Some(imagen_nueva(4))).expect("B <- A");
    assert!(!d.layer(b).expect("B").visible());
    assert_eq!(d.layers()[0].id(), b);
    let (mut d, [a, _, c]) = doc_abc();
    aplica_y_comprueba(&mut d, a, &[c], Some(imagen_nueva(5))).expect("A <- C");
    assert_eq!(d.layers().len(), 2);
}

#[test]
fn fila_02_c_image_none_deja_la_celda_vacia() {
    // Todas las celdas vacías: [B, C] sin imagen.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    assert_eq!(d.apply(Command::RemoveLayer { layer: a }), Ok(true));
    assert_eq!(d.materialized_bytes(), 0);
    assert!(d.plan_merge(b, &[c]).expect("plan").all_empty);
    aplica_y_comprueba(&mut d, b, &[c], None).expect("vacías");
    assert!(d.image(celda(&d, b)).is_none(), "la celda sigue vacía");
    assert_eq!(d.materialized_bytes(), 0);

    // `None` con celdas con imagen: el resultado es una celda vacía y la memoria baja.
    let (mut d, [a, b, c]) = doc_abc();
    aplica_y_comprueba(&mut d, a, &[b, c], None).expect("aplanar a vacío");
    assert!(d.image(celda(&d, a)).is_none());
    assert_eq!(d.materialized_bytes(), 0);
}

// ---------------------------------------------------------------------------------------
// Generadores de documentos y de imágenes válidas para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct PlanCapa {
    vacia: bool,
    pixeles: Vec<(u8, u8, u8)>,
    opacidad: u8,
    mezcla: u8,
    visible: bool,
    bloqueada: bool,
}

/// `tipo`: 0 RGBA, 1 grises con alfa, 2 indexado con índice transparente.
#[derive(Clone, Debug)]
struct PlanDoc {
    tipo: u8,
    w: u32,
    h: u32,
    paleta: usize,
    capas: Vec<PlanCapa>,
}

fn modo_de(tipo: u8) -> Mode {
    match tipo {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        _ => Mode::Indexed,
    }
}

fn plan_estrategia(capas: RangeInclusive<usize>) -> impl Strategy<Value = PlanDoc> {
    let capa = (
        any::<bool>(),
        prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..5),
        prop_oneof![2 => Just(255u8), 1 => any::<u8>()],
        0u8..8,
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(vacia, pixeles, opacidad, mezcla, visible, bloqueada)| PlanCapa {
                vacia,
                pixeles,
                opacidad,
                mezcla,
                visible,
                bloqueada,
            },
        );
    (
        0u8..3,
        1u32..=4,
        1u32..=4,
        2usize..=8,
        prop::collection::vec(capa, capas),
    )
        .prop_map(|(tipo, w, h, paleta, capas)| PlanDoc {
            tipo,
            w,
            h,
            paleta,
            capas,
        })
}

fn pixel_de(modo: Mode, s: u8, paleta: usize) -> Pixel {
    let alfa = [0u8, 128, 255, 255][usize::from(s % 4)];
    match modo {
        Mode::Rgba if alfa == 0 => Pixel::Rgba(T),
        Mode::Rgba => Pixel::Rgba(Rgba::new(s, s.wrapping_mul(3), s.wrapping_add(50), alfa)),
        Mode::GrayAlpha if alfa == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: s,
            alpha: alfa,
        },
        Mode::Gray => Pixel::Gray(s),
        Mode::Indexed => Pixel::Indexed(u8::try_from(usize::from(s) % paleta).unwrap_or(0)),
    }
}

fn imagen_base(tipo: u8, w: u32, h: u32, paleta: usize) -> Image {
    match tipo {
        0 | 1 => Image::new(w, h, modo_de(tipo)).expect("imagen"),
        _ => img_indexada(w, h, &colores_distintos(paleta), Some(0)),
    }
}

/// Construye el documento del plan con `Document::new`, `AddLayer`, escrituras y ajustes.
fn construye(plan: &PlanDoc) -> Document {
    let modo = modo_de(plan.tipo);
    let mut img = imagen_base(plan.tipo, plan.w, plan.h, plan.paleta);
    for &(x, y, s) in &plan.capas[0].pixeles {
        img.set_pixel(
            u32::from(x) % plan.w,
            u32::from(y) % plan.h,
            pixel_de(modo, s, plan.paleta),
        )
        .expect("píxel de la capa 0");
    }
    let mut d = Document::new(img);
    for (i, capa) in plan.capas.iter().enumerate().skip(1) {
        anade(&mut d, &format!("Capa {}", i + 1), i);
        if !capa.vacia && !capa.pixeles.is_empty() {
            let image = d.layers()[i].cel();
            let pixels = capa
                .pixeles
                .iter()
                .map(|&(x, y, s)| PixelEdit {
                    x: u32::from(x) % plan.w,
                    y: u32::from(y) % plan.h,
                    pixel: pixel_de(modo, s, plan.paleta),
                })
                .collect();
            d.apply(Command::SetPixels { image, pixels })
                .expect("escribir en la capa");
        }
    }
    let indexado = modo == Mode::Indexed;
    for (i, capa) in plan.capas.iter().enumerate() {
        let id = d.layers()[i].id();
        let (opacidad, mezcla) = if indexado {
            (255, BlendMode::Normal)
        } else {
            (capa.opacidad, BlendMode::ALL[usize::from(capa.mezcla % 8)])
        };
        ajusta(&mut d, id, opacidad, mezcla, capa.visible, capa.bloqueada);
    }
    d
}

/// Imagen válida (tamaño, modo y paleta del lienzo) de un solo valor.
fn imagen_valida(doc: &Document, s: u8) -> Image {
    let (w, h) = (doc.width(), doc.height());
    match doc.palette() {
        Some(p) => {
            let indice = u8::try_from(usize::from(s) % p.len()).unwrap_or(0);
            Image::filled_indexed(w, h, p.clone(), indice).expect("imagen indexada")
        }
        None => Image::filled(w, h, pixel_de(doc.mode(), s, 1)).expect("imagen"),
    }
}

/// Imagen del tamaño del lienzo pero de otro modo (nunca válida para combinar).
fn imagen_de_otro_modo(doc: &Document) -> Image {
    let modo = if doc.mode() == Mode::Rgba {
        Mode::Gray
    } else {
        Mode::Rgba
    };
    Image::new(doc.width(), doc.height(), modo).expect("imagen")
}

// ---------------------------------------------------------------------------------------
// Propiedad 5: deshacer una combinación devuelve las capas tal cual.
// ---------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 5: para cualquier documento de 2 a 5 capas y cualquier `MergeLayers` válido,
    /// `undo` devuelve las quitadas con sus ids, posición, nombre, visibilidad, bloqueo,
    /// opacidad, mezcla e imagen, la que se queda recupera las suyas, el documento es `==` al de
    /// antes con el mismo `state_id`, y `redo` repite el resultado (lo comprueba
    /// `aplica_y_comprueba`).
    #[test]
    fn propiedad_5_deshacer_una_combinacion_devuelve_las_capas_tal_cual(
        plan in plan_estrategia(2..=5),
        eleccion in any::<usize>(),
        mascara in any::<u8>(),
        imagen in 0u8..3,
        semilla in any::<u8>(),
    ) {
        let mut doc = construye(&plan);
        let n = doc.layers().len();
        let t = eleccion % n;
        let target = doc.layers()[t].id();
        let mut quitar: Vec<LayerId> = (0..n)
            .filter(|&i| i != t && (mascara & (1u8 << i)) != 0)
            .map(|i| doc.layers()[i].id())
            .collect();
        if quitar.is_empty() {
            quitar.push(doc.layers()[(t + 1) % n].id());
        }
        let imagen = match imagen {
            0 => None,
            1 => Some(imagen_valida(&doc, semilla)),
            _ => doc.image(doc.layers()[t].cel()).cloned(),
        };
        aplica_y_comprueba(&mut doc, target, &quitar, imagen)
            .map_err(TestCaseError::fail)?;
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: ida y vuelta de secuencias con combinar, ConvertAll, ConvertMode, capas,
// paleta y pintura, con fallos inyectados.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    /// `fallo % 6`: 1 `based_on` viejo, 2 imagen de otro modo, 3 `target` desconocido, 4 una
    /// capa desconocida en `remove`; 0 y 5, sin fallo.
    Merge {
        t: usize,
        mascara: u8,
        imagen: u8,
        semilla: u8,
        fallo: u8,
    },
    /// Modo, y si se permite todo lo que se pueda perder.
    ConvertAll(u8, bool),
    ConvertMode(usize, u8),
    Add(usize),
    Remove(usize),
    Move(usize, usize),
    Rename(usize, String),
    Opacity(usize, u8),
    Visible(usize, bool),
    Blend(usize, u8),
    Locked(usize, bool),
    Duplicate(usize),
    Pal(u8, usize, u8),
    Paint(usize, Vec<(u8, u8, u8)>),
}

fn op() -> BoxedStrategy<Op> {
    prop_oneof![
        6 => (any::<usize>(), any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()).prop_map(
            |(t, mascara, imagen, semilla, fallo)| Op::Merge { t, mascara, imagen, semilla, fallo }
        ),
        2 => (0u8..4, any::<bool>()).prop_map(|(m, p)| Op::ConvertAll(m, p)),
        1 => (any::<usize>(), 0u8..4).prop_map(|(k, m)| Op::ConvertMode(k, m)),
        2 => any::<usize>().prop_map(Op::Add),
        2 => any::<usize>().prop_map(Op::Remove),
        1 => (any::<usize>(), any::<usize>()).prop_map(|(k, to)| Op::Move(k, to)),
        1 => (any::<usize>(), "[a-z]{0,6}").prop_map(|(k, n)| Op::Rename(k, n)),
        2 => (any::<usize>(), any::<u8>()).prop_map(|(k, o)| Op::Opacity(k, o)),
        1 => (any::<usize>(), any::<bool>()).prop_map(|(k, v)| Op::Visible(k, v)),
        2 => (any::<usize>(), 0u8..8).prop_map(|(k, m)| Op::Blend(k, m)),
        1 => (any::<usize>(), any::<bool>()).prop_map(|(k, l)| Op::Locked(k, l)),
        1 => any::<usize>().prop_map(Op::Duplicate),
        2 => (any::<u8>(), any::<usize>(), any::<u8>()).prop_map(|(p, k, s)| Op::Pal(p, k, s)),
        4 => (any::<usize>(), prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..4))
            .prop_map(|(k, p)| Op::Paint(k, p)),
    ]
    .boxed()
}

fn modo_cualquiera(m: u8) -> Mode {
    match m % 4 {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        2 => Mode::Gray,
        _ => Mode::Indexed,
    }
}

fn pinta(doc: &Document, k: usize, pixeles: &[(u8, u8, u8)]) -> Command {
    let modo = doc.mode();
    let paleta = doc.palette().map_or(1, Palette::len);
    let n = doc.layers().len();
    Command::SetPixels {
        image: doc.layers()[k % n].cel(),
        pixels: pixeles
            .iter()
            .map(|&(x, y, s)| PixelEdit {
                x: u32::from(x) % doc.width(),
                y: u32::from(y) % doc.height(),
                pixel: pixel_de(modo, s, paleta),
            })
            .collect(),
    }
}

/// Traduce una orden a `Command` con el documento vivo delante, para que los ids sean los del
/// momento.
fn traduce(doc: &Document, op: &Op) -> Command {
    let n = doc.layers().len();
    let capa = |k: &usize| doc.layers()[k % n].id();
    let cel = |k: &usize| doc.layers()[k % n].cel();
    match op {
        Op::Merge {
            t,
            mascara,
            imagen,
            semilla,
            fallo,
        } => {
            let ti = t % n;
            let mut target = doc.layers()[ti].id();
            let mut remove: Vec<LayerId> = (0..n)
                .filter(|&i| i != ti && (mascara & (1u8 << (i % 8))) != 0)
                .map(|i| doc.layers()[i].id())
                .collect();
            if remove.is_empty() && n > 1 {
                remove.push(doc.layers()[(ti + 1) % n].id());
            }
            let mut image = match imagen % 3 {
                0 => None,
                1 => Some(imagen_valida(doc, *semilla)),
                _ => doc.image(doc.layers()[ti].cel()).cloned(),
            };
            let mut based_on = doc.state_id();
            match fallo % 6 {
                1 => based_on = based_on.wrapping_add(1),
                2 => image = Some(imagen_de_otro_modo(doc)),
                3 => target = LayerId::from_raw(9999),
                4 => remove.push(LayerId::from_raw(9999)),
                _ => {}
            }
            Command::MergeLayers {
                target,
                remove,
                image,
                based_on,
            }
        }
        Op::ConvertAll(m, todo) => Command::ConvertAll {
            target: modo_cualquiera(*m),
            allowed: if *todo {
                Allowed {
                    luma: true,
                    drop_alpha: Some([0, 0, 0]),
                }
            } else {
                Allowed::NOTHING
            },
        },
        Op::ConvertMode(k, m) => Command::ConvertMode {
            image: cel(k),
            target: modo_cualquiera(*m),
            allowed: Allowed {
                luma: true,
                drop_alpha: Some([0, 0, 0]),
            },
        },
        Op::Add(at) => anade_cmd("n", at % (n + 1)),
        Op::Remove(k) => Command::RemoveLayer { layer: capa(k) },
        Op::Move(k, to) => Command::MoveLayer {
            layer: capa(k),
            to: to % (n + 1),
        },
        Op::Rename(k, nombre) => renombra(capa(k), nombre),
        Op::Opacity(k, opacity) => Command::SetLayerOpacity {
            layer: capa(k),
            opacity: *opacity,
        },
        Op::Visible(k, visible) => Command::SetLayerVisible {
            layer: capa(k),
            visible: *visible,
        },
        Op::Blend(k, m) => Command::SetLayerBlend {
            layer: capa(k),
            blend: BlendMode::ALL[usize::from(*m % 8)],
        },
        Op::Locked(k, locked) => Command::SetLayerLocked {
            layer: capa(k),
            locked: *locked,
        },
        Op::Duplicate(k) => Command::DuplicateLayer {
            layer: capa(k),
            name: "copia".to_string(),
        },
        Op::Pal(p, k, s) => {
            let image = cel(k);
            let color = Rgba::new(*s, *s / 2, 255 - *s, 255);
            match p % 4 {
                0 => Command::SetPaletteColor {
                    image,
                    index: *s % 4,
                    color,
                },
                1 => Command::AddPaletteEntry { image, color },
                2 => Command::SetTransparentIndex {
                    image,
                    index: if s % 3 == 0 { None } else { Some(*s % 4) },
                },
                _ => Command::RotateColors {
                    image,
                    start: 0,
                    len: 3,
                    steps: 1,
                },
            }
        }
        Op::Paint(k, pixeles) => pinta(doc, *k, pixeles),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 2: `undo` hasta `Err` deja un documento `==` al inicial (mismos ids en el mismo
    /// orden, mismo `state_id`, `materialized_bytes()` y `history_bytes() == 0`); `redo` hasta
    /// `Err` deja uno `==` al final; y todo `Err` u `Ok(false)` deja documento, `state_id`,
    /// historia y pilas iguales. Los fallos inyectados (`based_on` viejo, imagen de otro modo,
    /// capa desconocida) nunca dan `Ok(true)`.
    #[test]
    fn propiedad_2_ida_y_vuelta_de_secuencias_con_combinar(
        tipo in 0u8..3,
        w in 1u32..=4,
        h in 1u32..=4,
        paleta in 2usize..=8,
        nuevas in 1usize..=3,
        iniciales in prop::collection::vec(
            prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..4),
            4,
        ),
        ops in prop::collection::vec(op(), 0..=20),
    ) {
        let mut doc = Document::new(imagen_base(tipo, w, h, paleta));
        doc.set_history_limit(usize::MAX);
        let inicial = Foto::de(&doc);
        prop_assert_eq!(inicial.historia, 0);
        prop_assert_eq!(inicial.estado, 0);
        let mut oks = 0usize;

        // La preparación (capas y píxeles) es parte de la secuencia que se deshace.
        for i in 0..=nuevas {
            if i > 0 {
                prop_assert_eq!(doc.apply(anade_cmd(&format!("Capa {}", i + 1), i)), Ok(true));
                oks += 1;
            }
            if !iniciales[i].is_empty() {
                let cmd = pinta(&doc, i, &iniciales[i]);
                match doc.apply(cmd) {
                    Ok(true) => oks += 1,
                    Ok(false) => {}
                    Err(e) => return Err(TestCaseError::fail(format!("preparar: {e:?}"))),
                }
            }
        }

        for orden in &ops {
            let antes = Foto::de(&doc);
            let cmd = traduce(&doc, orden);
            let resultado = doc.apply(cmd);
            match &resultado {
                Ok(true) => {
                    oks += 1;
                    prop_assert_ne!(doc.state_id(), antes.estado, "{:?}", orden);
                }
                _ => antes.exige(&doc, &format!("{orden:?} -> {resultado:?}"))?,
            }
            if let Op::Merge { fallo, .. } = orden
                && matches!(fallo % 6, 1..=4)
            {
                prop_assert_ne!(resultado, Ok(true), "un fallo inyectado no puede aplicarse");
            }
        }

        let final_ = Foto::de(&doc);
        let mut deshechos = 0usize;
        while doc.undo().is_ok() {
            deshechos += 1;
            prop_assert!(deshechos <= oks, "más deshacer que pasos");
        }
        prop_assert_eq!(deshechos, oks);
        if let Some(motivo) = inicial.diferencia_estado(&doc) {
            return Err(TestCaseError::fail(format!("tras deshacer todo: {motivo}")));
        }
        prop_assert_eq!(doc.history_bytes(), 0);
        prop_assert!(!doc.can_undo());

        let mut rehechos = 0usize;
        while doc.redo().is_ok() {
            rehechos += 1;
            prop_assert!(rehechos <= oks, "más rehacer que pasos");
        }
        prop_assert_eq!(rehechos, oks);
        if let Some(motivo) = final_.diferencia_estado(&doc) {
            return Err(TestCaseError::fail(format!("tras rehacer todo: {motivo}")));
        }
        prop_assert!(!doc.can_redo());
    }
}
