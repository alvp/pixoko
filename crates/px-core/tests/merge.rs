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
const RED: Rgba = Rgba::new(255, 0, 0, 255);
const GREEN: Rgba = Rgba::new(0, 255, 0, 255);
const BLUE: Rgba = Rgba::new(0, 0, 255, 255);

fn img_rgba(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Rgba).expect("imagen RGBA")
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

/// Documento RGBA de 4 x 3 recién creado.
fn doc_d() -> Document {
    Document::new(img_rgba(4, 3))
}

fn adds_cmd(name_2: &str, at: usize) -> Command {
    Command::AddLayer {
        name: name_2.to_string(),
        at,
    }
}

/// Añade una capa en `at` y devuelve su `LayerId`.
fn adds(doc: &mut Document, name_2: &str, at: usize) -> LayerId {
    assert_eq!(doc.apply(adds_cmd(name_2, at)), Ok(true), "añadir {name_2}");
    doc.layers()[at].id()
}

fn cel(doc: &Document, layer_2: LayerId) -> ImageId {
    doc.layer(layer_2).expect("la capa existe").cel()
}

fn ids(doc: &Document) -> Vec<(LayerId, ImageId)> {
    doc.layers().iter().map(|l| (l.id(), l.cel())).collect()
}

fn sets(image: ImageId, x: u32, y: u32, c: Rgba) -> Command {
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
fn row_0(image: ImageId, c: Rgba) -> Command {
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

fn renames(layer: LayerId, name: &str) -> Command {
    Command::RenameLayer {
        layer,
        name: name.to_string(),
    }
}

/// Fija opacidad, mezcla, visibilidad y bloqueo de una capa.
fn adjusts(
    doc: &mut Document,
    layer_2: LayerId,
    opacity: u8,
    blend: BlendMode,
    visible: bool,
    locked: bool,
) {
    for cmd in [
        Command::SetLayerOpacity {
            layer: layer_2,
            opacity,
        },
        Command::SetLayerBlend {
            layer: layer_2,
            blend,
        },
        Command::SetLayerVisible {
            layer: layer_2,
            visible,
        },
        Command::SetLayerLocked {
            layer: layer_2,
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
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    let (cb, cc) = (cel(&d, b), cel(&d, c));
    assert_eq!(d.apply(row_0(cb, C2)), Ok(true));
    assert_eq!(d.apply(sets(cc, 1, 1, RED)), Ok(true));
    adjusts(&mut d, a, 200, BlendMode::Multiply, true, false);
    adjusts(&mut d, b, 128, BlendMode::Screen, false, true);
    adjusts(&mut d, c, 90, BlendMode::Difference, true, true);
    (d, [a, b, c])
}

/// Imagen RGBA 4 x 3 de un solo color: la «composición» que viaja en el comando.
fn image_new(tag: u8) -> Image {
    Image::filled(4, 3, Pixel::Rgba(Rgba::new(tag, 20, 30, 200))).expect("imagen")
}

fn combines(doc: &Document, target: LayerId, remove: &[LayerId], image: Option<Image>) -> Command {
    Command::MergeLayers {
        target,
        remove: remove.to_vec(),
        image,
        based_on: doc.state_id(),
    }
}

/// Todo lo observable que un comando fallido o sin cambios no puede tocar.
struct Snapshot {
    doc: Document,
    can_undo_2: bool,
    can_redo_2: bool,
    history: usize,
    state: u64,
    materialized: usize,
    ids: Vec<(LayerId, ImageId)>,
    mode_2: Mode,
    palette_2: Option<Palette>,
}

impl Snapshot {
    fn de(doc: &Document) -> Snapshot {
        Snapshot {
            doc: doc.clone(),
            can_undo_2: doc.can_undo(),
            can_redo_2: doc.can_redo(),
            history: doc.history_bytes(),
            state: doc.state_id(),
            materialized: doc.materialized_bytes(),
            ids: ids(doc),
            mode_2: doc.mode(),
            palette_2: doc.palette().cloned(),
        }
    }

    /// Solo el contenido: documento, ids, modo y paleta del lienzo, `state_id` y bytes
    /// materializados.
    fn difference_state(&self, doc: &Document) -> Option<&'static str> {
        if *doc != self.doc {
            Some("el documento cambió")
        } else if ids(doc) != self.ids {
            Some("los ids o su orden cambiaron")
        } else if doc.mode() != self.mode_2 {
            Some("el modo del lienzo cambió")
        } else if doc.palette() != self.palette_2.as_ref() {
            Some("la paleta del lienzo cambió")
        } else if doc.state_id() != self.state {
            Some("state_id cambió")
        } else if doc.materialized_bytes() != self.materialized {
            Some("materialized_bytes cambió")
        } else {
            None
        }
    }

    /// Además de lo anterior, la historia y las pilas.
    fn difference(&self, doc: &Document) -> Option<&'static str> {
        self.difference_state(doc).or_else(|| {
            if doc.can_undo() != self.can_undo_2 {
                Some("can_undo cambió")
            } else if doc.can_redo() != self.can_redo_2 {
                Some("can_redo cambió")
            } else if doc.history_bytes() != self.history {
                Some("history_bytes cambió")
            } else {
                None
            }
        })
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
}

/// Aplica `cmd`, comprueba el resultado y que nada observable cambió.
fn without_effect(doc: &mut Document, cmd: Command, expected: Result<bool, DocumentError>) {
    let snapshot = Snapshot::de(doc);
    let context = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), expected, "{context}");
    snapshot.assert_equal(doc, &context);
}

fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

macro_rules! requires {
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
fn applies_and_checks(
    doc: &mut Document,
    target: LayerId,
    remove: &[LayerId],
    image: Option<Image>,
) -> Result<(), String> {
    let before = Snapshot::de(doc);
    let layers_before = doc.layers().to_vec();
    let saved: usize = layers_before
        .iter()
        .filter(|l| l.id() == target || remove.contains(&l.id()))
        .filter_map(|l| doc.image(l.cel()))
        .map(|i| i.bytes().len())
        .sum();
    let new = image.as_ref().map_or(0, |i| i.bytes().len());

    let result = doc.apply(combines(doc, target, remove, image.clone()));
    requires!(result == Ok(true), "apply dio {result:?}");
    requires!(doc.state_id() != before.state, "state_id no es nuevo");
    requires!(doc.can_undo() && !doc.can_redo(), "pilas tras aplicar");

    // Las capas que quedan, en el mismo orden relativo; las quitadas desaparecen con su celda.
    let expected_2: Vec<LayerId> = layers_before
        .iter()
        .map(Layer::id)
        .filter(|id| !remove.contains(id))
        .collect();
    let real: Vec<LayerId> = doc.layers().iter().map(Layer::id).collect();
    requires!(
        real == expected_2,
        "capas {real:?} en vez de {expected_2:?}"
    );
    for removed in remove {
        let old = layers_before.iter().find(|l| l.id() == *removed);
        requires!(doc.layer(*removed).is_none(), "{removed:?} sigue");
        if let Some(old) = old {
            requires!(
                doc.image(old.cel()).is_none(),
                "{removed:?}: su celda sigue"
            );
        }
    }
    // Las demás capas, intactas.
    for layer_2 in doc.layers() {
        if layer_2.id() == target {
            continue;
        }
        let old = layers_before.iter().find(|l| l.id() == layer_2.id());
        requires!(old == Some(layer_2), "{:?} cambió", layer_2.id());
    }
    // `target`: mismas identidad, nombre y visibilidad; opacidad 255, Normal y desbloqueada; la
    // imagen del comando.
    let old = layers_before
        .iter()
        .find(|l| l.id() == target)
        .ok_or_else(|| "target no estaba".to_string())?;
    let new_layer = doc
        .layer(target)
        .ok_or_else(|| "target desapareció".to_string())?;
    requires!(new_layer.cel() == old.cel(), "ImageId de target cambió");
    requires!(new_layer.name() == old.name(), "nombre de target cambió");
    requires!(
        new_layer.visible() == old.visible(),
        "visibilidad de target cambió"
    );
    requires!(
        new_layer.opacity() == 255 && new_layer.blend() == BlendMode::Normal && !new_layer.locked(),
        "target no quedó con 255, Normal y desbloqueada"
    );
    requires!(
        doc.image(new_layer.cel()) == image.as_ref(),
        "la celda de target no es la imagen del comando"
    );
    // Memoria y coste de historia.
    requires!(
        doc.materialized_bytes() == before.materialized - saved + new,
        "materialized_bytes {} (antes {}, guardado {saved}, nueva {new})",
        doc.materialized_bytes(),
        before.materialized
    );
    requires!(
        doc.history_bytes() >= before.history + saved,
        "history_bytes {} < {} + {saved}",
        doc.history_bytes(),
        before.history
    );

    // Un solo paso: un `undo` devuelve las capas tal cual, y un `redo`, la combinación.
    let after = Snapshot::de(doc);
    doc.undo().map_err(|e| format!("undo: {e:?}"))?;
    requires!(
        doc.layers() == layers_before.as_slice(),
        "tras undo las capas no son las de antes"
    );
    if let Some(reason) = before.difference_state(doc) {
        return Err(format!("tras undo: {reason}"));
    }
    doc.redo().map_err(|e| format!("redo: {e:?}"))?;
    if let Some(reason) = after.difference_state(doc) {
        return Err(format!("tras redo: {reason}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Fila 1: validación de MergeLayers y plan_merge.
// ---------------------------------------------------------------------------------------

#[test]
fn row_01_a_unknown_layers_invalid_remove_and_batch() {
    let (mut d, [a, b, c]) = doc_abc();
    let (false_value, other) = (LayerId::from_raw(999), LayerId::from_raw(998));
    // La combinación de partida es válida (se comprueba sobre una copia).
    let mut copy = d.clone();
    let valid = combines(&d, a, &[b, c], Some(image_new(1)));
    assert_eq!(copy.apply(valid), Ok(true));

    let with = |target: LayerId, remove: &[LayerId], based_on: u64| Command::MergeLayers {
        target,
        remove: remove.to_vec(),
        image: Some(image_new(1)),
        based_on,
    };
    let state = d.state_id();

    // `UnknownLayer`: primero `target`, luego `remove` en orden.
    without_effect(
        &mut d,
        with(false_value, &[b], state),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    without_effect(
        &mut d,
        with(a, &[b, false_value], state),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    without_effect(
        &mut d,
        with(a, &[other, false_value], state),
        Err(DocumentError::UnknownLayer(other)),
    );
    without_effect(
        &mut d,
        with(false_value, &[other], state),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    // `UnknownLayer` gana a `InvalidMerge` (remove vacío, o con `target`).
    without_effect(
        &mut d,
        with(false_value, &[], state),
        Err(DocumentError::UnknownLayer(false_value)),
    );
    without_effect(
        &mut d,
        with(a, &[a, false_value], state),
        Err(DocumentError::UnknownLayer(false_value)),
    );

    // `InvalidMerge`: remove vacío, con repetidas o con `target`; gana a `StaleImage`.
    without_effect(
        &mut d,
        with(a, &[], state),
        Err(DocumentError::InvalidMerge),
    );
    without_effect(
        &mut d,
        with(a, &[b, b], state),
        Err(DocumentError::InvalidMerge),
    );
    without_effect(
        &mut d,
        with(a, &[b, a], state),
        Err(DocumentError::InvalidMerge),
    );
    without_effect(
        &mut d,
        with(a, &[a], state + 1000),
        Err(DocumentError::InvalidMerge),
    );

    // Dentro de un `Batch` no se puede, sola o detrás de una escritura válida.
    let ca = cel(&d, a);
    without_effect(
        &mut d,
        Command::Batch(vec![with(a, &[b, c], state)]),
        Err(in_batch(0, DocumentError::NotBatchable)),
    );
    without_effect(
        &mut d,
        Command::Batch(vec![sets(ca, 2, 2, C2), with(a, &[b, c], state)]),
        Err(in_batch(1, DocumentError::NotBatchable)),
    );
}

#[test]
fn row_01_b_an_old_image_gives_stale_image_without_touching_anything() {
    let (mut d, [a, b, c]) = doc_abc();
    let based_on = d.state_id();
    let cmd = combines(&d, a, &[b, c], Some(image_new(1)));

    // El documento cambia entre calcular el comando y aplicarlo.
    assert_eq!(d.apply(renames(a, "otro nombre")), Ok(true));
    let current = d.state_id();
    assert_ne!(current, based_on);
    without_effect(
        &mut d,
        cmd.clone(),
        Err(DocumentError::StaleImage { based_on, current }),
    );

    // Lo mismo con una escritura de píxeles, y con una imagen que además no encaja: el estado
    // viejo se detecta antes que el desajuste de la imagen.
    let cb = cel(&d, b);
    assert_eq!(d.apply(sets(cb, 3, 2, RED)), Ok(true));
    let current = d.state_id();
    without_effect(
        &mut d,
        cmd.clone(),
        Err(DocumentError::StaleImage { based_on, current }),
    );
    let mismatched = Command::MergeLayers {
        target: a,
        remove: vec![b, c],
        image: Some(img_rgba(9, 9)),
        based_on,
    };
    without_effect(
        &mut d,
        mismatched,
        Err(DocumentError::StaleImage { based_on, current }),
    );

    // Volver al estado del que salió el comando lo hace válido otra vez.
    d.undo().expect("deshacer la escritura");
    d.undo().expect("deshacer el nombre");
    assert_eq!(d.state_id(), based_on);
    assert_eq!(d.apply(cmd), Ok(true));
}

#[test]
fn row_01_c_the_image_must_have_the_canvas_width_height_mode_and_palette() {
    // RGBA: otro ancho, otro alto y otros modos.
    let (mut d, [a, b, c]) = doc_abc();
    let with = |d: &Document, image_2: Option<Image>| combines(d, a, &[b, c], image_2);
    let bad_2 = [
        img_rgba(5, 3),
        img_rgba(4, 4),
        img_rgba(1, 1),
        Image::new(4, 3, Mode::Gray).expect("gris"),
        Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa"),
        img_indexed(4, 3, &[RED, GREEN], Some(0)),
    ];
    for bad in bad_2 {
        let cmd = with(&d, Some(bad));
        without_effect(&mut d, cmd, Err(DocumentError::MergeImageMismatch));
    }

    // Indexado: la paleta ha de ser idéntica, índice transparente incluido.
    let palette_2 = [RED, GREEN, BLUE];
    let mut di = Document::new(img_indexed(4, 3, &palette_2, Some(0)));
    let (ia, ib) = (di.layers()[0].id(), adds(&mut di, "B", 1));
    let with_i = |d: &Document, image_2: Option<Image>| combines(d, ia, &[ib], image_2);
    let bad_2 = [
        img_indexed(4, 3, &[RED, GREEN, C2], Some(0)),
        img_indexed(4, 3, &palette_2, Some(1)),
        img_indexed(4, 3, &palette_2, None),
        img_indexed(4, 3, &[RED, GREEN], Some(0)),
        img_indexed(4, 3, &[RED, GREEN, BLUE, T], Some(0)),
        img_rgba(4, 3),
    ];
    for bad in bad_2 {
        let cmd = with_i(&di, Some(bad));
        without_effect(&mut di, cmd, Err(DocumentError::MergeImageMismatch));
    }

    // Con la paleta idéntica vale, y `None` (celda vacía) vale siempre y no se comprueba.
    let good = Image::filled_indexed(4, 3, di.palette().expect("paleta").clone(), 2)
        .expect("imagen indexada");
    let cmd = with_i(&di, Some(good));
    assert_eq!(di.apply(cmd), Ok(true));
    let (mut d2, [a2, b2, _]) = doc_abc();
    let cmd = combines(&d2, a2, &[b2], None);
    assert_eq!(d2.apply(cmd), Ok(true));
}

#[test]
fn row_01_d_cels_too_large_looks_at_the_result_and_only_if_it_grows() {
    // Un documento al tope de celdas SÍ puede aplanarse: el resultado (48) cabe aunque el pico
    // (144 + 48) no.
    let (mut d, [a, b, c]) = doc_abc();
    assert_eq!(d.materialized_bytes(), 144);
    d.set_cells_limit(144);
    applies_and_checks(&mut d, a, &[b, c], Some(image_new(1))).expect("aplanar al tope");

    // El resultado supera el tope y crece: A tiene imagen (48), B y C están vacías, y la imagen
    // nueva de B suma 48 más.
    let mut d = doc_d();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    d.set_cells_limit(48);
    let cmd = combines(&d, b, &[c], Some(image_new(1)));
    without_effect(
        &mut d,
        cmd,
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 48,
        }),
    );
    // El desajuste de la imagen se detecta antes que el tope.
    let cmd = combines(&d, b, &[c], Some(img_rgba(9, 9)));
    without_effect(&mut d, cmd, Err(DocumentError::MergeImageMismatch));
    // Con el tope justo en el resultado, vale.
    d.set_cells_limit(96);
    let cmd = combines(&d, b, &[c], Some(image_new(1)));
    assert_eq!(d.apply(cmd), Ok(true));

    // Pasarse del tope sin crecer respecto a hoy vale: hay 144 bytes, el resultado es 96 y el
    // tope es 48.
    let (mut d, [a, b, _]) = doc_abc();
    d.set_cells_limit(48);
    applies_and_checks(&mut d, a, &[b], Some(image_new(1))).expect("no crece");

    // Un resultado igual a lo que hay hoy (48 - 48 + 48) con el tope en 0 tampoco crece.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    d.set_cells_limit(0);
    assert_eq!(d.materialized_bytes(), 48);
    let cmd = combines(&d, b, &[a], Some(image_new(1)));
    assert_eq!(d.apply(cmd), Ok(true));
    assert_eq!(d.materialized_bytes(), 48);
}

#[test]
fn row_01_e_plan_merge_is_read_only_and_gives_empty_bytes_and_peak() {
    // [A con imagen, B vacía, C con imagen, D vacía]: dos celdas de 48 bytes.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    let dd = adds(&mut d, "D", 3);
    let cc = cel(&d, c);
    assert_eq!(d.apply(sets(cc, 0, 0, RED)), Ok(true));
    let snapshot = Snapshot::de(&d);
    let peak = d.materialized_bytes() + d.cel_bytes();
    assert_eq!(peak, 96 + 48);

    let plan = d.plan_merge(a, &[b]).expect("plan");
    assert_eq!(
        plan,
        MergePlan {
            stored_bytes: 48,
            all_empty: false,
            peak_bytes: peak,
        }
    );
    let copy = plan; // `Copy`
    assert_eq!(copy, plan);
    // Cuenta las celdas de `target` y las de `remove`, no las demás.
    assert_eq!(d.plan_merge(b, &[c]).expect("plan").stored_bytes, 48);
    assert_eq!(d.plan_merge(a, &[b, c]).expect("plan").stored_bytes, 96);
    assert_eq!(d.plan_merge(b, &[dd]).expect("plan").stored_bytes, 0);
    // `all_empty`: ninguna de las celdas de `target` y `remove` tiene imagen.
    let empty = d.plan_merge(b, &[dd]).expect("plan");
    assert!(empty.all_empty);
    assert_eq!(empty.peak_bytes, peak);
    assert!(!d.plan_merge(b, &[c]).expect("plan").all_empty);
    assert!(!d.plan_merge(a, &[dd]).expect("plan").all_empty);

    // Mismos errores que `apply`, en el mismo orden.
    let (false_value, other) = (LayerId::from_raw(999), LayerId::from_raw(998));
    assert_eq!(
        d.plan_merge(false_value, &[b]),
        Err(DocumentError::UnknownLayer(false_value))
    );
    assert_eq!(
        d.plan_merge(a, &[other, false_value]),
        Err(DocumentError::UnknownLayer(other))
    );
    assert_eq!(d.plan_merge(a, &[]), Err(DocumentError::InvalidMerge));
    assert_eq!(d.plan_merge(a, &[b, b]), Err(DocumentError::InvalidMerge));
    assert_eq!(d.plan_merge(a, &[b, a]), Err(DocumentError::InvalidMerge));
    assert_eq!(
        d.plan_merge(false_value, &[]),
        Err(DocumentError::UnknownLayer(false_value))
    );
    // No tocó nada.
    snapshot.assert_equal(&d, "plan_merge");
}

// ---------------------------------------------------------------------------------------
// Fila 2: MergeLayers sobre [A, B, C].
// ---------------------------------------------------------------------------------------

#[test]
fn row_02_a_removing_the_top_one_and_ids_keep_growing() {
    let (mut d, [_, b, c]) = doc_abc();
    let cc = cel(&d, c);
    applies_and_checks(&mut d, b, &[c], Some(image_new(1))).expect("combinar C en B");

    // Lo que dice la spec, explícito: quedan A y B; B conserva id, celda, nombre y que estaba
    // oculta (aunque tenía 128, Pantalla y bloqueo).
    assert_eq!(d.layers().len(), 2);
    let layer_b = d.layer(b).expect("B sigue");
    assert_eq!(layer_b.name(), "B");
    assert!(!layer_b.visible());
    assert_eq!(
        (layer_b.opacity(), layer_b.blend(), layer_b.locked()),
        (255, BlendMode::Normal, false)
    );
    assert_eq!(d.image(layer_b.cel()), Some(&image_new(1)));
    assert!(d.layer(c).is_none());

    // Los ids de la quitada no se reutilizan, tampoco tras deshacer y rehacer.
    let new = adds(&mut d, "Z", 0);
    let z = d.layer(new).expect("Z");
    assert!(z.id().raw() > c.raw(), "LayerId reutilizado");
    assert!(z.cel().raw() > cc.raw(), "ImageId reutilizado");
}

#[test]
fn row_02_b_remove_two_or_the_bottom_one_at_any_position() {
    // La de abajo se queda y se quitan B y C.
    let (mut d, [a, b, c]) = doc_abc();
    applies_and_checks(&mut d, a, &[b, c], Some(image_new(2))).expect("A <- B y C");
    assert_eq!(d.layers().len(), 1);
    assert_eq!(d.layers()[0].name(), "Capa 1");

    // El orden de `remove` no importa.
    let (mut d, [a, b, c]) = doc_abc();
    applies_and_checks(&mut d, a, &[c, b], Some(image_new(2))).expect("A <- C y B");

    // Se queda la de arriba y se quita la de abajo (posiciones reinsertadas al deshacer).
    let (mut d, [a, _, c]) = doc_abc();
    applies_and_checks(&mut d, c, &[a], Some(image_new(3))).expect("C <- A");
    assert_eq!(d.layers().len(), 2);

    // Se queda la oculta y bloqueada B (conserva su visibilidad y queda desbloqueada) y se
    // quita la de abajo; luego se queda A y se quita la de arriba, con B en medio de las dos
    // al deshacer.
    let (mut d, [a, b, _]) = doc_abc();
    applies_and_checks(&mut d, b, &[a], Some(image_new(4))).expect("B <- A");
    assert!(!d.layer(b).expect("B").visible());
    assert_eq!(d.layers()[0].id(), b);
    let (mut d, [a, _, c]) = doc_abc();
    applies_and_checks(&mut d, a, &[c], Some(image_new(5))).expect("A <- C");
    assert_eq!(d.layers().len(), 2);
}

#[test]
fn row_02_c_image_none_leaves_the_cel_empty() {
    // Todas las celdas vacías: [B, C] sin imagen.
    let mut d = doc_d();
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    assert_eq!(d.apply(Command::RemoveLayer { layer: a }), Ok(true));
    assert_eq!(d.materialized_bytes(), 0);
    assert!(d.plan_merge(b, &[c]).expect("plan").all_empty);
    applies_and_checks(&mut d, b, &[c], None).expect("vacías");
    assert!(d.image(cel(&d, b)).is_none(), "la celda sigue vacía");
    assert_eq!(d.materialized_bytes(), 0);

    // `None` con celdas con imagen: el resultado es una celda vacía y la memoria baja.
    let (mut d, [a, b, c]) = doc_abc();
    applies_and_checks(&mut d, a, &[b, c], None).expect("aplanar a vacío");
    assert!(d.image(cel(&d, a)).is_none());
    assert_eq!(d.materialized_bytes(), 0);
}

// ---------------------------------------------------------------------------------------
// Generadores de documentos y de imágenes válidas para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct PlanLayer {
    empty: bool,
    pixels_2: Vec<(u8, u8, u8)>,
    opacity_2: u8,
    blend_2: u8,
    visible: bool,
    locked_2: bool,
}

/// `kind`: 0 RGBA, 1 grises con alfa, 2 indexado con índice transparente.
#[derive(Clone, Debug)]
struct PlanDoc {
    kind: u8,
    w: u32,
    h: u32,
    palette_2: usize,
    layers_2: Vec<PlanLayer>,
}

fn mode_of(kind: u8) -> Mode {
    match kind {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        _ => Mode::Indexed,
    }
}

fn plan_strategy(layers_2: RangeInclusive<usize>) -> impl Strategy<Value = PlanDoc> {
    let layer_2 = (
        any::<bool>(),
        prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..5),
        prop_oneof![2 => Just(255u8), 1 => any::<u8>()],
        0u8..8,
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(empty, pixels_2, opacity_2, blend_2, visible, locked_2)| PlanLayer {
                empty,
                pixels_2,
                opacity_2,
                blend_2,
                visible,
                locked_2,
            },
        );
    (
        0u8..3,
        1u32..=4,
        1u32..=4,
        2usize..=8,
        prop::collection::vec(layer_2, layers_2),
    )
        .prop_map(|(kind, w, h, palette_2, layers_2)| PlanDoc {
            kind,
            w,
            h,
            palette_2,
            layers_2,
        })
}

fn pixel_de(mode_2: Mode, s: u8, palette_2: usize) -> Pixel {
    let alpha_2 = [0u8, 128, 255, 255][usize::from(s % 4)];
    match mode_2 {
        Mode::Rgba if alpha_2 == 0 => Pixel::Rgba(T),
        Mode::Rgba => Pixel::Rgba(Rgba::new(s, s.wrapping_mul(3), s.wrapping_add(50), alpha_2)),
        Mode::GrayAlpha if alpha_2 == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: s,
            alpha: alpha_2,
        },
        Mode::Gray => Pixel::Gray(s),
        Mode::Indexed => Pixel::Indexed(u8::try_from(usize::from(s) % palette_2).unwrap_or(0)),
    }
}

fn image_base(kind: u8, w: u32, h: u32, palette_2: usize) -> Image {
    match kind {
        0 | 1 => Image::new(w, h, mode_of(kind)).expect("imagen"),
        _ => img_indexed(w, h, &colors_different(palette_2), Some(0)),
    }
}

/// Construye el documento del plan con `Document::new`, `AddLayer`, escrituras y ajustes.
fn builds(plan: &PlanDoc) -> Document {
    let mode_2 = mode_of(plan.kind);
    let mut img = image_base(plan.kind, plan.w, plan.h, plan.palette_2);
    for &(x, y, s) in &plan.layers_2[0].pixels_2 {
        img.set_pixel(
            u32::from(x) % plan.w,
            u32::from(y) % plan.h,
            pixel_de(mode_2, s, plan.palette_2),
        )
        .expect("píxel de la capa 0");
    }
    let mut d = Document::new(img);
    for (i, layer_2) in plan.layers_2.iter().enumerate().skip(1) {
        adds(&mut d, &format!("Capa {}", i + 1), i);
        if !layer_2.empty && !layer_2.pixels_2.is_empty() {
            let image = d.layers()[i].cel();
            let pixels = layer_2
                .pixels_2
                .iter()
                .map(|&(x, y, s)| PixelEdit {
                    x: u32::from(x) % plan.w,
                    y: u32::from(y) % plan.h,
                    pixel: pixel_de(mode_2, s, plan.palette_2),
                })
                .collect();
            d.apply(Command::SetPixels { image, pixels })
                .expect("escribir en la capa");
        }
    }
    let indexed = mode_2 == Mode::Indexed;
    for (i, layer_2) in plan.layers_2.iter().enumerate() {
        let id = d.layers()[i].id();
        let (opacity_2, blend_2) = if indexed {
            (255, BlendMode::Normal)
        } else {
            (
                layer_2.opacity_2,
                BlendMode::ALL[usize::from(layer_2.blend_2 % 8)],
            )
        };
        adjusts(
            &mut d,
            id,
            opacity_2,
            blend_2,
            layer_2.visible,
            layer_2.locked_2,
        );
    }
    d
}

/// Imagen válida (tamaño, modo y paleta del lienzo) de un solo valor.
fn image_valid(doc: &Document, s: u8) -> Image {
    let (w, h) = (doc.width(), doc.height());
    match doc.palette() {
        Some(p) => {
            let index = u8::try_from(usize::from(s) % p.len()).unwrap_or(0);
            Image::filled_indexed(w, h, p.clone(), index).expect("imagen indexada")
        }
        None => Image::filled(w, h, pixel_de(doc.mode(), s, 1)).expect("imagen"),
    }
}

/// Imagen del tamaño del lienzo pero de otro modo (nunca válida para combinar).
fn image_of_other_mode(doc: &Document) -> Image {
    let mode_2 = if doc.mode() == Mode::Rgba {
        Mode::Gray
    } else {
        Mode::Rgba
    };
    Image::new(doc.width(), doc.height(), mode_2).expect("imagen")
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
    /// `applies_and_checks`).
    #[test]
    fn property_5_undoing_a_merge_returns_the_layers_as_they_were(
        plan in plan_strategy(2..=5),
        choice in any::<usize>(),
        mask in any::<u8>(),
        image_2 in 0u8..3,
        seed in any::<u8>(),
    ) {
        let mut doc = builds(&plan);
        let n = doc.layers().len();
        let t = choice % n;
        let target = doc.layers()[t].id();
        let mut remove: Vec<LayerId> = (0..n)
            .filter(|&i| i != t && (mask & (1u8 << i)) != 0)
            .map(|i| doc.layers()[i].id())
            .collect();
        if remove.is_empty() {
            remove.push(doc.layers()[(t + 1) % n].id());
        }
        let image_2 = match image_2 {
            0 => None,
            1 => Some(image_valid(&doc, seed)),
            _ => doc.image(doc.layers()[t].cel()).cloned(),
        };
        applies_and_checks(&mut doc, target, &remove, image_2)
            .map_err(TestCaseError::fail)?;
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: ida y vuelta de secuencias con combinar, ConvertAll, ConvertMode, capas,
// paleta y pintura, con fallos inyectados.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    /// `failure % 6`: 1 `based_on` viejo, 2 imagen de otro modo, 3 `target` desconocido, 4 una
    /// capa desconocida en `remove`; 0 y 5, sin fallo.
    Merge {
        t: usize,
        mask: u8,
        image_2: u8,
        seed: u8,
        failure: u8,
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
            |(t, mask, image_2, seed, failure)| Op::Merge { t, mask, image_2, seed, failure }
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

fn mode_any(m: u8) -> Mode {
    match m % 4 {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        2 => Mode::Gray,
        _ => Mode::Indexed,
    }
}

fn paints(doc: &Document, k: usize, pixels_2: &[(u8, u8, u8)]) -> Command {
    let mode_2 = doc.mode();
    let palette_2 = doc.palette().map_or(1, Palette::len);
    let n = doc.layers().len();
    Command::SetPixels {
        image: doc.layers()[k % n].cel(),
        pixels: pixels_2
            .iter()
            .map(|&(x, y, s)| PixelEdit {
                x: u32::from(x) % doc.width(),
                y: u32::from(y) % doc.height(),
                pixel: pixel_de(mode_2, s, palette_2),
            })
            .collect(),
    }
}

/// Traduce una orden a `Command` con el documento vivo delante, para que los ids sean los del
/// momento.
fn translates(doc: &Document, op: &Op) -> Command {
    let n = doc.layers().len();
    let layer_2 = |k: &usize| doc.layers()[k % n].id();
    let cel = |k: &usize| doc.layers()[k % n].cel();
    match op {
        Op::Merge {
            t,
            mask,
            image_2,
            seed,
            failure,
        } => {
            let ti = t % n;
            let mut target = doc.layers()[ti].id();
            let mut remove: Vec<LayerId> = (0..n)
                .filter(|&i| i != ti && (mask & (1u8 << (i % 8))) != 0)
                .map(|i| doc.layers()[i].id())
                .collect();
            if remove.is_empty() && n > 1 {
                remove.push(doc.layers()[(ti + 1) % n].id());
            }
            let mut image = match image_2 % 3 {
                0 => None,
                1 => Some(image_valid(doc, *seed)),
                _ => doc.image(doc.layers()[ti].cel()).cloned(),
            };
            let mut based_on = doc.state_id();
            match failure % 6 {
                1 => based_on = based_on.wrapping_add(1),
                2 => image = Some(image_of_other_mode(doc)),
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
        Op::ConvertAll(m, all) => Command::ConvertAll {
            target: mode_any(*m),
            allowed: if *all {
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
            target: mode_any(*m),
            allowed: Allowed {
                luma: true,
                drop_alpha: Some([0, 0, 0]),
            },
        },
        Op::Add(at) => adds_cmd("n", at % (n + 1)),
        Op::Remove(k) => Command::RemoveLayer { layer: layer_2(k) },
        Op::Move(k, to) => Command::MoveLayer {
            layer: layer_2(k),
            to: to % (n + 1),
        },
        Op::Rename(k, name_2) => renames(layer_2(k), name_2),
        Op::Opacity(k, opacity) => Command::SetLayerOpacity {
            layer: layer_2(k),
            opacity: *opacity,
        },
        Op::Visible(k, visible) => Command::SetLayerVisible {
            layer: layer_2(k),
            visible: *visible,
        },
        Op::Blend(k, m) => Command::SetLayerBlend {
            layer: layer_2(k),
            blend: BlendMode::ALL[usize::from(*m % 8)],
        },
        Op::Locked(k, locked) => Command::SetLayerLocked {
            layer: layer_2(k),
            locked: *locked,
        },
        Op::Duplicate(k) => Command::DuplicateLayer {
            layer: layer_2(k),
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
        Op::Paint(k, pixels_2) => paints(doc, *k, pixels_2),
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
    fn property_2_round_trip_of_sequences_with_merge(
        kind in 0u8..3,
        w in 1u32..=4,
        h in 1u32..=4,
        palette_2 in 2usize..=8,
        new_2 in 1usize..=3,
        initial_2 in prop::collection::vec(
            prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..4),
            4,
        ),
        ops in prop::collection::vec(op(), 0..=20),
    ) {
        let mut doc = Document::new(image_base(kind, w, h, palette_2));
        doc.set_history_limit(usize::MAX);
        let initial = Snapshot::de(&doc);
        prop_assert_eq!(initial.history, 0);
        prop_assert_eq!(initial.state, 0);
        let mut oks = 0usize;

        // La preparación (capas y píxeles) es parte de la secuencia que se deshace.
        for (i, initial) in initial_2.iter().enumerate().take(new_2 + 1) {
            if i > 0 {
                prop_assert_eq!(doc.apply(adds_cmd(&format!("Capa {}", i + 1), i)), Ok(true));
                oks += 1;
            }
            if !initial.is_empty() {
                let cmd = paints(&doc, i, initial);
                match doc.apply(cmd) {
                    Ok(true) => oks += 1,
                    Ok(false) => {}
                    Err(e) => return Err(TestCaseError::fail(format!("preparar: {e:?}"))),
                }
            }
        }

        for order in &ops {
            let before = Snapshot::de(&doc);
            let cmd = translates(&doc, order);
            let result = doc.apply(cmd);
            match &result {
                Ok(true) => {
                    oks += 1;
                    prop_assert_ne!(doc.state_id(), before.state, "{:?}", order);
                }
                _ => before.requires(&doc, &format!("{order:?} -> {result:?}"))?,
            }
            if let Op::Merge { failure, .. } = order
                && matches!(failure % 6, 1..=4)
            {
                prop_assert_ne!(result, Ok(true), "un fallo inyectado no puede aplicarse");
            }
        }

        let final_ = Snapshot::de(&doc);
        let mut undone = 0usize;
        while doc.undo().is_ok() {
            undone += 1;
            prop_assert!(undone <= oks, "más deshacer que pasos");
        }
        prop_assert_eq!(undone, oks);
        if let Some(reason) = initial.difference_state(&doc) {
            return Err(TestCaseError::fail(format!("tras deshacer todo: {reason}")));
        }
        prop_assert_eq!(doc.history_bytes(), 0);
        prop_assert!(!doc.can_undo());

        let mut redone = 0usize;
        while doc.redo().is_ok() {
            redone += 1;
            prop_assert!(redone <= oks, "más rehacer que pasos");
        }
        prop_assert_eq!(redone, oks);
        if let Some(reason) = final_.difference_state(&doc) {
            return Err(TestCaseError::fail(format!("tras rehacer todo: {reason}")));
        }
        prop_assert!(!doc.can_redo());
    }
}
