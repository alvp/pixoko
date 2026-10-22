//! Tests de `Command::ConvertAll`, `convert_cels` y `analyze_cels` (specs/hito-3-combinar.md,
//! unidad 3.5, cambios `C-02`, `C-08`, `C-09`, `C-10`, `C-11` y `C-13`): convertir el modo y la
//! paleta del lienzo y de todas las celdas con imagen en un solo paso.
//!
//! Filas 3 y 4 de «Casos» (`fila_NN_x`, un caso por prueba) y las propiedades 3 (`ConvertAll` y
//! las paletas) y 2 (ida y vuelta, en lo que toca a `ConvertAll`; la secuencia que mezcla
//! combinar está en `merge.rs`). Las cifras (8 píxeles perdidos, 257 colores, 96 bytes...) son
//! las de la spec, calculadas a mano. Ninguna prueba reserva más de unos pocos KiB: las
//! imágenes miden como mucho 20 x 10 y el caso de `TooLarge` usa solo `size`, sin imágenes.
//!
//! Este fichero es la fase de pruebas en rojo: `ConvertAll`, `convert_cels` y `analyze_cels` no
//! existen todavía, así que no compila hasta la lógica de `px-core` de la spec.
//!
//! Los `// SPEC?:` marcan dudas que la spec no resuelve; ahí la prueba solo comprueba lo que la
//! spec dice y evita lo demás.

use std::ops::RangeInclusive;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, BlendMode, Command, ConversionReport, ConvertError, Document, DocumentError, Image,
    ImageId, LayerId, Mode, Palette, Pixel, PixelEdit, Rgba, Span, analyze_cels, convert_cels,
    convert_image, luma,
};

// ---------------------------------------------------------------------------------------
// Colores, imágenes y documentos de apoyo.
// ---------------------------------------------------------------------------------------

const T: Rgba = Rgba::TRANSPARENT;
const RED: Rgba = Rgba::new(255, 0, 0, 255);
const GREEN: Rgba = Rgba::new(0, 255, 0, 255);
const BLUE: Rgba = Rgba::new(0, 0, 255, 255);

fn img_rgba(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Rgba).expect("imagen RGBA")
}

fn img_indexed(w: u32, h: u32, colors_2: &[Rgba], t: Option<u8>) -> Image {
    let mut palette_2 = Palette::from_colors(colors_2.iter().copied()).expect("paleta");
    palette_2
        .set_transparent_index(t)
        .expect("índice transparente");
    Image::new_indexed(w, h, palette_2).expect("imagen indexada")
}

fn colors_different(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

/// Todo permitido: `luma` y descartar el alfa sobre negro.
fn all() -> Allowed {
    Allowed {
        luma: true,
        drop_alpha: Some([0, 0, 0]),
    }
}

fn only_luma() -> Allowed {
    Allowed {
        luma: true,
        drop_alpha: None,
    }
}

fn adds(doc: &mut Document, name_2: &str, at: usize) -> LayerId {
    let cmd = Command::AddLayer {
        name: name_2.to_string(),
        at,
    };
    assert_eq!(doc.apply(cmd), Ok(true), "añadir {name_2}");
    doc.layers()[at].id()
}

fn cel_2(doc: &Document, layer_2: LayerId) -> ImageId {
    doc.layer(layer_2).expect("la capa existe").cel()
}

fn ids(doc: &Document) -> Vec<(LayerId, ImageId)> {
    doc.layers().iter().map(|l| (l.id(), l.cel())).collect()
}

fn set_px(image: ImageId, edits: &[(u32, u32, Pixel)]) -> Command {
    Command::SetPixels {
        image,
        pixels: edits
            .iter()
            .map(|&(x, y, pixel)| PixelEdit { x, y, pixel })
            .collect(),
    }
}

fn sets(image: ImageId, edits: &[(u32, u32, Rgba)]) -> Command {
    let v: Vec<(u32, u32, Pixel)> = edits
        .iter()
        .map(|&(x, y, c)| (x, y, Pixel::Rgba(c)))
        .collect();
    set_px(image, &v)
}

/// Rellena de `c` toda una celda RGBA de 4 x 3.
fn fills_all(image: ImageId, c: Rgba) -> Command {
    Command::FillSpans {
        image,
        spans: (0..3)
            .map(|y| Span {
                y,
                x_start: 0,
                x_end: 4,
            })
            .collect(),
        pixel: Pixel::Rgba(c),
    }
}

fn adjusts(doc: &mut Document, cmd: Command) {
    doc.apply(cmd).expect("propiedad de capa");
}

/// Los píxeles de una imagen, fila a fila.
fn pixels(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).expect("píxel dentro"));
        }
    }
    v
}

/// La celda de la capa, que ha de tener imagen.
fn image_2(doc: &Document, layer_2: LayerId) -> &Image {
    doc.image(cel_2(doc, layer_2))
        .expect("la celda tiene imagen")
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

    /// Contenido: documento, ids, modo y paleta del lienzo, `state_id` y bytes materializados.
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

    /// Además, la historia y las pilas.
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
fn without_effect(doc: &mut Document, cmd: Command, expected_2: Result<bool, DocumentError>) {
    let snapshot = Snapshot::de(doc);
    let context = format!("{cmd:?}");
    assert_eq!(doc.apply(cmd), expected_2, "{context}");
    snapshot.assert_equal(doc, &context);
}

fn convert_all(target: Mode, allowed: Allowed) -> Command {
    Command::ConvertAll { target, allowed }
}

/// Tras un `ConvertAll` con `Ok(true)`: un solo `undo` devuelve lo de antes (documento, ids,
/// modo y paleta del lienzo, `state_id`) y un `redo` lo de después.
fn undoes_and_redoes(doc: &mut Document, before: &Snapshot) {
    let after = Snapshot::de(doc);
    doc.undo().expect("deshacer");
    assert_eq!(before.difference_state(doc), None, "tras deshacer");
    assert!(doc.can_redo());
    doc.redo().expect("rehacer");
    assert_eq!(after.difference_state(doc), None, "tras rehacer");
    assert!(!doc.can_redo());
}

/// El error de un `Result` sin exigir `Debug` al caso `Ok`.
fn error_de<T>(r: Result<T, ConvertError>) -> ConvertError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("se esperaba un error"),
    }
}

/// `[A, B, C]`, RGBA 4 x 3; `A` y `C` con imagen y `B` con la celda vacía.
/// `A`: (0,0) = (200,100,50,255), (1,0) = gris, (2,0) = (200,100,50,128).
/// `C`: (3,2) = azul, (0,0) = gris. Con `props`: `A` opacidad 200, `B` oculta, `C` Multiplicar y
/// bloqueada.
fn doc_three(props: bool) -> (Document, [LayerId; 3]) {
    let mut img = img_rgba(4, 3);
    for (x, y, c) in [
        (0, 0, Rgba::new(200, 100, 50, 255)),
        (1, 0, Rgba::new(10, 10, 10, 255)),
        (2, 0, Rgba::new(200, 100, 50, 128)),
    ] {
        img.set_pixel(x, y, Pixel::Rgba(c)).expect("píxel");
    }
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    let cc = cel_2(&d, c);
    let edits = [(3, 2, BLUE), (0, 0, Rgba::new(10, 10, 10, 255))];
    assert_eq!(d.apply(sets(cc, &edits)), Ok(true));
    if props {
        adjusts(
            &mut d,
            Command::SetLayerOpacity {
                layer: a,
                opacity: 200,
            },
        );
        adjusts(
            &mut d,
            Command::SetLayerVisible {
                layer: b,
                visible: false,
            },
        );
        adjusts(
            &mut d,
            Command::SetLayerBlend {
                layer: c,
                blend: BlendMode::Multiply,
            },
        );
        adjusts(
            &mut d,
            Command::SetLayerLocked {
                layer: c,
                locked: true,
            },
        );
    }
    (d, [a, b, c])
}

/// `[A, B, C]` en grises con alfa, con `B` vacía: `A` (0,0) = (100, 255) y (1,0) = (50, 128);
/// `C` (2,2) = (200, 255).
fn doc_gray_alpha() -> (Document, [LayerId; 3]) {
    let mut img = Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa");
    img.set_pixel(
        0,
        0,
        Pixel::GrayAlpha {
            gray: 100,
            alpha: 255,
        },
    )
    .expect("píxel");
    img.set_pixel(
        1,
        0,
        Pixel::GrayAlpha {
            gray: 50,
            alpha: 128,
        },
    )
    .expect("píxel");
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    let editing = [(
        2,
        2,
        Pixel::GrayAlpha {
            gray: 200,
            alpha: 255,
        },
    )];
    assert_eq!(d.apply(set_px(cel_2(&d, c), &editing)), Ok(true));
    (d, [a, b, c])
}

// ---------------------------------------------------------------------------------------
// Fila 3: ConvertAll con tres capas.
// ---------------------------------------------------------------------------------------

#[test]
fn row_03_a_rgba_to_gray_alpha_with_luma_in_one_step() {
    let (mut d, [a, b, c]) = doc_three(true);
    let before = Snapshot::de(&d);
    let layers_2 = d.layers().to_vec();
    let original_2: Vec<Option<Image>> =
        layers_2.iter().map(|l| d.image(l.cel()).cloned()).collect();

    assert_eq!(d.apply(convert_all(Mode::GrayAlpha, only_luma())), Ok(true));
    assert_eq!(d.mode(), Mode::GrayAlpha);
    assert_eq!(d.palette(), None);
    assert_eq!((d.width(), d.height()), (4, 3));
    assert_eq!(d.layers().len(), 3);
    for (new, old) in d.layers().iter().zip(&layers_2) {
        // Las propiedades de la capa no cambian: solo la imagen de su celda.
        assert_eq!(new.id(), old.id());
        assert_eq!(new.cel(), old.cel());
        assert_eq!(new.name(), old.name());
        assert_eq!(new.visible(), old.visible());
        assert_eq!(new.opacity(), old.opacity());
        assert_eq!(new.blend(), old.blend());
        assert_eq!(new.locked(), old.locked());
    }
    // La celda vacía sigue vacía; las otras tienen el modo del lienzo y el gris por `luma`.
    assert!(d.image(cel_2(&d, b)).is_none());
    for (layer_2, original) in [(a, &original_2[0]), (c, &original_2[2])] {
        let new = image_2(&d, layer_2);
        assert_eq!((new.width(), new.height()), (4, 3));
        assert_eq!(new.mode(), Mode::GrayAlpha);
        assert_eq!(new.palette(), None);
        let expected_2: Vec<Pixel> = pixels(original.as_ref().expect("original"))
            .into_iter()
            .map(|p| match p {
                Pixel::Rgba(c) if c.a == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
                Pixel::Rgba(c) => Pixel::GrayAlpha {
                    gray: luma(c.r, c.g, c.b),
                    alpha: c.a,
                },
                other => panic!("se esperaba RGBA, hay {other:?}"),
            })
            .collect();
        assert_eq!(pixels(new), expected_2, "{layer_2:?}");
    }
    // Dos celdas de 4 x 3 x 2 bytes.
    assert_eq!(d.materialized_bytes(), 48);
    assert_ne!(d.state_id(), before.state);

    undoes_and_redoes(&mut d, &before);
}

#[test]
fn row_03_b_to_indexed_one_palette_for_all_with_the_transparent_at_0() {
    // [A, B vacía, C]. Se recorre de abajo arriba y cada celda por filas: primero todo A (ROJO,
    // VERDE) y luego C (AZUL), aunque el AZUL de C esté en (0, 0), antes que lo de A en el
    // recorrido de arriba abajo.
    let mut img = img_rgba(4, 3);
    for (x, c) in [(0, RED), (1, GREEN), (2, RED)] {
        img.set_pixel(x, 0, Pixel::Rgba(c)).expect("píxel");
    }
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    assert_eq!(
        d.apply(sets(cel_2(&d, c), &[(0, 0, BLUE), (1, 1, GREEN)])),
        Ok(true)
    );
    let before = Snapshot::de(&d);

    assert_eq!(
        d.apply(convert_all(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::Indexed);
    let palette_2 = d.palette().expect("el lienzo tiene paleta").clone();
    assert_eq!(palette_2.colors(), &[T, RED, GREEN, BLUE]);
    assert_eq!(palette_2.transparent_index(), Some(0));
    // Una sola paleta, idéntica, para todas las celdas con imagen; la vacía sigue vacía.
    assert!(d.image(cel_2(&d, b)).is_none());
    for layer_2 in [a, c] {
        assert_eq!(
            image_2(&d, layer_2).palette(),
            Some(&palette_2),
            "{layer_2:?}"
        );
    }
    let indices = |layer_2: LayerId| -> Vec<Pixel> { pixels(image_2(&d, layer_2)) };
    let expected_2 = |v: [u8; 12]| -> Vec<Pixel> { v.into_iter().map(Pixel::Indexed).collect() };
    assert_eq!(indices(a), expected_2([1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    assert_eq!(indices(c), expected_2([3, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0]));
    undoes_and_redoes(&mut d, &before);

    // Con más de una capa el transparente va en el 0 aunque ningún píxel sea transparente.
    let opaque_2 = Image::filled(4, 3, Pixel::Rgba(RED)).expect("imagen");
    let mut d = Document::new(opaque_2);
    let b = adds(&mut d, "B", 1);
    assert_eq!(d.apply(fills_all(cel_2(&d, b), BLUE)), Ok(true));
    assert_eq!(
        d.apply(convert_all(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    let palette_2 = d.palette().expect("paleta");
    assert_eq!(palette_2.colors(), &[T, RED, BLUE]);
    assert_eq!(palette_2.transparent_index(), Some(0));

    // Con una sola capa opaca y con imagen, no hay transparente (como `ConvertMode`).
    let opaque_2 = Image::filled(4, 3, Pixel::Rgba(RED)).expect("imagen");
    let mut d = Document::new(opaque_2);
    assert_eq!(
        d.apply(convert_all(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    let palette_2 = d.palette().expect("paleta");
    assert_eq!(palette_2.colors(), &[RED]);
    assert_eq!(palette_2.transparent_index(), None);
}

#[test]
fn row_03_c_gray_alpha_and_indexed_to_rgba() {
    // Grises con alfa -> RGBA.
    let (mut d, [a, b, c]) = doc_gray_alpha();
    let before = Snapshot::de(&d);
    assert_eq!(d.apply(convert_all(Mode::Rgba, Allowed::NOTHING)), Ok(true));
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert!(d.image(cel_2(&d, b)).is_none());
    let mut expected_a = vec![Pixel::Rgba(T); 12];
    expected_a[0] = Pixel::Rgba(Rgba::new(100, 100, 100, 255));
    expected_a[1] = Pixel::Rgba(Rgba::new(50, 50, 50, 128));
    assert_eq!(pixels(image_2(&d, a)), expected_a);
    let mut expected_c = vec![Pixel::Rgba(T); 12];
    expected_c[10] = Pixel::Rgba(Rgba::new(200, 200, 200, 255));
    assert_eq!(pixels(image_2(&d, c)), expected_c);
    // Dos celdas de 4 x 3 x 4 bytes.
    assert_eq!(d.materialized_bytes(), 96);
    undoes_and_redoes(&mut d, &before);

    // Indexado -> RGBA: el lienzo pierde la paleta y `undo` se la devuelve.
    let mut img = img_indexed(4, 3, &[RED, GREEN, BLUE], Some(0));
    img.set_pixel(0, 0, Pixel::Indexed(1)).expect("píxel");
    img.set_pixel(1, 0, Pixel::Indexed(2)).expect("píxel");
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    let c = adds(&mut d, "C", 2);
    assert_eq!(
        d.apply(set_px(cel_2(&d, c), &[(3, 2, Pixel::Indexed(1))])),
        Ok(true)
    );
    let before = Snapshot::de(&d);
    assert!(before.palette_2.is_some());
    assert_eq!(d.apply(convert_all(Mode::Rgba, Allowed::NOTHING)), Ok(true));
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert!(d.image(cel_2(&d, b)).is_none());
    let mut expected_a = vec![Pixel::Rgba(T); 12];
    expected_a[0] = Pixel::Rgba(GREEN);
    expected_a[1] = Pixel::Rgba(BLUE);
    assert_eq!(pixels(image_2(&d, a)), expected_a);
    let mut expected_c = vec![Pixel::Rgba(T); 12];
    expected_c[11] = Pixel::Rgba(GREEN);
    assert_eq!(pixels(image_2(&d, c)), expected_c);
    assert_eq!(image_2(&d, a).palette(), None);
    undoes_and_redoes(&mut d, &before);
    // Tras el `redo` de arriba el lienzo vuelve a estar en RGBA; tras un `undo`, con su paleta.
    d.undo().expect("deshacer");
    assert_eq!(d.palette(), before.palette_2.as_ref());
    assert_eq!(image_2(&d, a).palette(), before.palette_2.as_ref());
}

#[test]
fn row_03_d_prior_errors_same_mode_and_convert_mode_with_several_layers() {
    // A gris con más de una capa (con todos los permisos): no hay valor transparente.
    let (mut d, [a, _, c]) = doc_three(false);
    without_effect(
        &mut d,
        convert_all(Mode::Gray, all()),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );
    let (mut g, _) = doc_gray_alpha();
    without_effect(
        &mut g,
        convert_all(Mode::Gray, all()),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );

    // A indexado con opacidad o con mezcla distintas de las de siempre.
    adjusts(
        &mut d,
        Command::SetLayerOpacity {
            layer: c,
            opacity: 128,
        },
    );
    without_effect(
        &mut d,
        convert_all(Mode::Indexed, all()),
        Err(DocumentError::IndexedOpacity),
    );
    adjusts(
        &mut d,
        Command::SetLayerBlend {
            layer: a,
            blend: BlendMode::Multiply,
        },
    );
    // Con las dos cosas, gana la opacidad.
    without_effect(
        &mut d,
        convert_all(Mode::Indexed, all()),
        Err(DocumentError::IndexedOpacity),
    );
    adjusts(
        &mut d,
        Command::SetLayerOpacity {
            layer: c,
            opacity: 255,
        },
    );
    without_effect(
        &mut d,
        convert_all(Mode::Indexed, all()),
        Err(DocumentError::IndexedBlend),
    );

    // Mismo modo: `Ok(false)` y nada cambia.
    without_effect(&mut d, convert_all(Mode::Rgba, all()), Ok(false));

    // `ConvertMode` con varias capas sigue dando `MultiLayerConvert`.
    let cel = cel_2(&d, a);
    without_effect(
        &mut d,
        Command::ConvertMode {
            image: cel,
            target: Mode::GrayAlpha,
            allowed: all(),
        },
        Err(DocumentError::MultiLayerConvert { layers: 3 }),
    );

    // Solo como comando suelto.
    without_effect(
        &mut d,
        Command::Batch(vec![convert_all(Mode::GrayAlpha, all())]),
        Err(DocumentError::InBatch {
            index: 0,
            source: Box::new(DocumentError::NotBatchable),
        }),
    );
}

#[test]
fn row_03_e_with_one_layer_convert_all_and_convert_mode_give_equal_documents() {
    let mut img = img_rgba(4, 3);
    for (x, c) in [
        (0, Rgba::new(40, 50, 60, 255)),
        (1, Rgba::new(70, 80, 90, 255)),
        (2, Rgba::new(255, 0, 0, 128)),
    ] {
        img.set_pixel(x, 0, Pixel::Rgba(c)).expect("píxel");
    }
    let original = Document::new(img);
    let cel = original.layers()[0].cel();
    for (target, allowed) in [
        (Mode::GrayAlpha, only_luma()),
        (Mode::Indexed, Allowed::NOTHING),
        (Mode::Gray, all()),
    ] {
        let (mut with_mode, mut with_all) = (original.clone(), original.clone());
        let r1 = with_mode.apply(Command::ConvertMode {
            image: cel,
            target,
            allowed,
        });
        let r2 = with_all.apply(convert_all(target, allowed));
        assert_eq!(r1, Ok(true), "{target:?}");
        assert_eq!(r2, r1, "{target:?}");
        assert_eq!(with_all, with_mode, "{target:?}");
        assert_eq!(with_all.mode(), with_mode.mode(), "{target:?}");
        assert_eq!(with_all.palette(), with_mode.palette(), "{target:?}");
        // `undo` exacto en el lienzo.
        with_all.undo().expect("deshacer");
        assert_eq!(with_all, original);
        assert_eq!(with_all.mode(), Mode::Rgba);
        assert_eq!(with_all.palette(), None);
    }

    // La única celda vacía (el caso que `ConvertMode` no admite): `ConvertAll` convierte el
    // lienzo y la celda sigue vacía.
    let mut d = Document::new(img_rgba(4, 3));
    let a = d.layers()[0].id();
    let b = adds(&mut d, "B", 1);
    assert_eq!(d.apply(Command::RemoveLayer { layer: a }), Ok(true));
    assert_eq!(d.layers().len(), 1);
    let before = Snapshot::de(&d);
    assert_eq!(
        d.apply(convert_all(Mode::GrayAlpha, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::GrayAlpha);
    assert_eq!(d.palette(), None);
    assert!(d.image(cel_2(&d, b)).is_none());
    undoes_and_redoes(&mut d, &before);
    assert_eq!(
        d.apply(convert_all(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    // Las celdas vacías valen transparente: la paleta lleva el transparente en el 0.
    // (Aquí el lienzo ya estaba en GrayAlpha tras el `redo`.)
    let palette_2 = d.palette().expect("paleta");
    assert_eq!(palette_2.colors(), &[T]);
    assert_eq!(palette_2.transparent_index(), Some(0));
    assert!(d.image(cel_2(&d, b)).is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 4: informe y errores.
// ---------------------------------------------------------------------------------------

/// Las celdas con imagen del documento, en orden de capas (de abajo arriba).
fn cels_2(doc: &Document) -> Vec<Option<&Image>> {
    doc.layers().iter().map(|l| doc.image(l.cel())).collect()
}

#[test]
fn row_04_a_the_report_sums_all_the_cels() {
    // Dos capas con 3 y 5 píxeles de color (más un gris y un transparente que no cuentan).
    let mut img = img_rgba(4, 3);
    for (x, y, c) in [
        (0, 0, RED),
        (1, 0, GREEN),
        (2, 0, BLUE),
        (3, 2, Rgba::new(9, 9, 9, 255)),
    ] {
        img.set_pixel(x, y, Pixel::Rgba(c)).expect("píxel");
    }
    let mut d = Document::new(img);
    let b = adds(&mut d, "B", 1);
    let color = [
        (0, 0, Rgba::new(10, 20, 30, 255)),
        (1, 0, Rgba::new(30, 20, 10, 255)),
        (2, 0, Rgba::new(1, 2, 3, 255)),
        (3, 0, Rgba::new(255, 0, 0, 128)),
        (0, 1, Rgba::new(0, 99, 0, 255)),
    ];
    assert_eq!(d.apply(sets(cel_2(&d, b), &color)), Ok(true));
    let sum = ConversionReport {
        color_lost: 8,
        alpha_lost: 0,
    };

    // Sin permiso: el informe sumado y el documento intacto.
    without_effect(
        &mut d,
        convert_all(Mode::GrayAlpha, Allowed::NOTHING),
        Err(DocumentError::Convert(ConvertError::LossNotAllowed {
            report: sum,
        })),
    );

    // `analyze_cels` da el mismo informe, y `convert_cels` también al convertir con permiso.
    let size = (d.width(), d.height());
    let cels = cels_2(&d);
    let e = error_de(analyze_cels(
        &cels,
        size,
        Mode::Rgba,
        Mode::GrayAlpha,
        Allowed::NOTHING,
    ));
    assert_eq!(e, ConvertError::LossNotAllowed { report: sum });
    let e = error_de(convert_cels(
        &cels,
        size,
        Mode::Rgba,
        Mode::GrayAlpha,
        Allowed::NOTHING,
    ));
    assert_eq!(e, ConvertError::LossNotAllowed { report: sum });
    let analysis =
        analyze_cels(&cels, size, Mode::Rgba, Mode::GrayAlpha, only_luma()).expect("con permiso");
    assert_eq!(analysis.report, sum);
    assert_eq!(analysis.palette, None);
    let conversion =
        convert_cels(&cels, size, Mode::Rgba, Mode::GrayAlpha, only_luma()).expect("con permiso");
    assert_eq!(conversion.report, sum);
    assert_eq!(conversion.palette, None);
    assert_eq!(conversion.images.len(), 2);
    assert!(
        conversion
            .images
            .iter()
            .all(|i| i.as_ref().is_some_and(|i| i.mode() == Mode::GrayAlpha))
    );
    let images = conversion.images;

    // Con permiso, `ConvertAll` aplica exactamente esas imágenes.
    assert_eq!(d.apply(convert_all(Mode::GrayAlpha, only_luma())), Ok(true));
    for (layer_2, expected) in d.layers().iter().zip(&images) {
        assert_eq!(d.image(layer_2.cel()), expected.as_ref());
    }
}

#[test]
fn row_04_b_the_union_of_colors_of_all_cels_counts() {
    // Dos capas de 20 x 10 con 200 colores distintos cada una: cada una cabe (con el
    // transparente, 201), la unión (400) no.
    let color = |i: usize, k: u8| Rgba::new(i as u8, k, 2, 255);
    let mut a = img_rgba(20, 10);
    let mut b = img_rgba(20, 10);
    for i in 0..200usize {
        let (x, y) = ((i % 20) as u32, (i / 20) as u32);
        a.set_pixel(x, y, Pixel::Rgba(color(i, 1))).expect("píxel");
        b.set_pixel(x, y, Pixel::Rgba(color(i, 9))).expect("píxel");
    }
    let size = (20, 10);
    let excess = ConvertError::TooManyColors { at_least: 257 };

    let only_a: Vec<Option<&Image>> = vec![Some(&a)];
    convert_cels(&only_a, size, Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
        .expect("cada una cabe");
    let both: Vec<Option<&Image>> = vec![Some(&a), Some(&b)];
    assert_eq!(
        error_de(analyze_cels(
            &both,
            size,
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        excess
    );
    assert_eq!(
        error_de(convert_cels(
            &both,
            size,
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        excess
    );

    // Y en el documento: error y nada cambia.
    let mut d = Document::new(a.clone());
    let layer_b = adds(&mut d, "B", 1);
    let edits: Vec<(u32, u32, Pixel)> = (0..200usize)
        .map(|i| ((i % 20) as u32, (i / 20) as u32, Pixel::Rgba(color(i, 9))))
        .collect();
    assert_eq!(d.apply(set_px(cel_2(&d, layer_b), &edits)), Ok(true));
    without_effect(
        &mut d,
        convert_all(Mode::Indexed, Allowed::NOTHING),
        Err(DocumentError::Convert(excess.clone())),
    );

    // Es la unión, no la suma: dos celdas con los mismos 200 colores caben (200 + transparente).
    let same = inverted(&a);
    let cels: Vec<Option<&Image>> = vec![Some(&a), Some(&same)];
    let conversion = convert_cels(&cels, size, Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
        .expect("la unión cabe");
    let palette_2 = conversion.palette.expect("paleta común");
    assert_eq!(palette_2.len(), 201);
    assert_eq!(palette_2.transparent_index(), Some(0));

    // La entrada transparente cuenta entre las 256: 255 colores y el transparente caben, 256 y
    // el transparente no (la segunda celda vacía obliga a tener transparente).
    let square = |different: usize| -> Image {
        let mut img = img_rgba(16, 16);
        for i in 0..256usize {
            let c = Rgba::new((i % different) as u8, 1, 2, 255);
            img.set_pixel((i % 16) as u32, (i / 16) as u32, Pixel::Rgba(c))
                .expect("píxel");
        }
        img
    };
    let (with_256, with_255) = (square(256), square(255));
    let cels: Vec<Option<&Image>> = vec![Some(&with_255), None];
    let conversion = convert_cels(&cels, (16, 16), Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
        .expect("255 + transparente = 256");
    assert_eq!(conversion.palette.expect("paleta").len(), 256);
    let cels: Vec<Option<&Image>> = vec![Some(&with_256), None];
    assert_eq!(
        error_de(convert_cels(
            &cels,
            (16, 16),
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        excess
    );
    assert_eq!(
        error_de(analyze_cels(
            &cels,
            (16, 16),
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        excess
    );
}

/// Copia de `a` con los mismos colores en otro orden de píxeles (girada 180 grados).
fn inverted(a: &Image) -> Image {
    let mut copy = a.clone();
    let (w, h) = (a.width(), a.height());
    for y in 0..h {
        for x in 0..w {
            let source = a.get_pixel(w - 1 - x, h - 1 - y).expect("píxel");
            copy.set_pixel(x, y, source).expect("píxel");
        }
    }
    copy
}

#[test]
fn row_04_c_cels_too_large_before_reserving() {
    // Grises con alfa -> RGBA duplica: dos celdas de 24 bytes pasan a 48 cada una (la vacía no
    // cuenta).
    let mut img = Image::new(4, 3, Mode::GrayAlpha).expect("gris con alfa");
    img.set_pixel(
        0,
        0,
        Pixel::GrayAlpha {
            gray: 100,
            alpha: 255,
        },
    )
    .expect("píxel");
    let mut d = Document::new(img);
    let b = adds(&mut d, "B", 1);
    adds(&mut d, "C", 2);
    let editing = [(
        1,
        1,
        Pixel::GrayAlpha {
            gray: 7,
            alpha: 255,
        },
    )];
    assert_eq!(d.apply(set_px(cel_2(&d, b), &editing)), Ok(true));
    assert_eq!(d.materialized_bytes(), 48);

    // Justo por debajo de 96: error con los datos de 3.1, sin tocar nada.
    d.set_cells_limit(95);
    without_effect(
        &mut d,
        convert_all(Mode::Rgba, Allowed::NOTHING),
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 95,
        }),
    );
    // Justo en 96 vale.
    d.set_cells_limit(96);
    assert_eq!(d.apply(convert_all(Mode::Rgba, Allowed::NOTHING)), Ok(true));
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn row_04_d_convert_cels_and_analyze_cels_direct() {
    // `TooLarge` por el tamaño del destino, antes de mirar ninguna celda: 32768 x 32768 x 4
    // bytes supera `Image::MAX_BYTES`, y no hay imágenes que reservar.
    let nothing: Vec<Option<&Image>> = vec![None, None];
    let large = ConvertError::TooLarge {
        width: 32768,
        height: 32768,
    };
    assert_eq!(
        error_de(convert_cels(
            &nothing,
            (32768, 32768),
            Mode::GrayAlpha,
            Mode::Rgba,
            Allowed::NOTHING
        )),
        large
    );
    assert_eq!(
        error_de(analyze_cels(
            &nothing,
            (32768, 32768),
            Mode::GrayAlpha,
            Mode::Rgba,
            Allowed::NOTHING
        )),
        large
    );

    // `images[i]` es `None` si `cels[i]` lo es; `palette` solo con destino indexado.
    let mut color = img_rgba(4, 3);
    color
        .set_pixel(1, 1, Pixel::Rgba(Rgba::new(200, 100, 50, 255)))
        .expect("píxel");
    let cels: Vec<Option<&Image>> = vec![None, Some(&color), None];
    let conversion =
        convert_cels(&cels, (4, 3), Mode::Rgba, Mode::GrayAlpha, only_luma()).expect("convertir");
    assert_eq!(conversion.images.len(), 3);
    assert!(conversion.images[0].is_none() && conversion.images[2].is_none());
    assert_eq!(conversion.palette, None);
    // Con una sola celda con imagen devuelve lo mismo que `convert_image`.
    let expected = convert_image(&color, Mode::GrayAlpha, only_luma()).expect("convert_image");
    assert_eq!(conversion.images[1].as_ref(), Some(&expected.image));
    assert_eq!(conversion.report, expected.report);

    // A indexado, con una celda con transparentes y con otra totalmente opaca.
    let opaque = Image::filled(4, 3, Pixel::Rgba(RED)).expect("imagen");
    for img in [&color, &opaque] {
        let a: Vec<Option<&Image>> = vec![Some(img)];
        let conversion = convert_cels(&a, (4, 3), Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
            .expect("convertir");
        let expected = convert_image(img, Mode::Indexed, Allowed::NOTHING).expect("convert_image");
        assert_eq!(conversion.images[0].as_ref(), Some(&expected.image));
        assert_eq!(conversion.palette.as_ref(), expected.image.palette());
        assert_eq!(conversion.report, expected.report);
    }

    // Con una celda vacía de por medio, el transparente va en el 0 aunque todo sea opaco.
    let with_empty: Vec<Option<&Image>> = vec![Some(&opaque), None];
    let conversion = convert_cels(
        &with_empty,
        (4, 3),
        Mode::Rgba,
        Mode::Indexed,
        Allowed::NOTHING,
    )
    .expect("convertir");
    let palette_2 = conversion.palette.expect("paleta");
    assert_eq!(palette_2.colors(), &[T, RED]);
    assert_eq!(palette_2.transparent_index(), Some(0));
    assert!(conversion.images[1].is_none());
    let analysis = analyze_cels(
        &with_empty,
        (4, 3),
        Mode::Rgba,
        Mode::Indexed,
        Allowed::NOTHING,
    )
    .expect("analizar");
    assert_eq!(analysis.palette.as_ref(), Some(&palette_2));
}

// ---------------------------------------------------------------------------------------
// Generadores para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct PlanLayer {
    empty: bool,
    pixels: Vec<(u8, u8, u8)>,
    opacity_2: u8,
    blend_2: u8,
    visible: bool,
    locked_2: bool,
}

/// `kind`: 0 RGBA, 1 grises con alfa, 2 indexado con índice transparente, 3 grises (una capa).
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
        2 => Mode::Indexed,
        _ => Mode::Gray,
    }
}

fn mode_any(m: u8) -> Mode {
    match m % 4 {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        2 => Mode::Gray,
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
            |(empty, pixels, opacity_2, blend_2, visible, locked_2)| PlanLayer {
                empty,
                pixels,
                opacity_2,
                blend_2,
                visible,
                locked_2,
            },
        );
    (
        0u8..4,
        1u32..=4,
        1u32..=4,
        2usize..=8,
        prop::collection::vec(layer_2, layers_2),
    )
        .prop_map(|(kind, w, h, palette_2, mut layers_2)| {
            // Sin valor transparente en grises no se puede tener más de una capa.
            if kind == 3 {
                layers_2.truncate(1);
            }
            PlanDoc {
                kind,
                w,
                h,
                palette_2,
                layers_2,
            }
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

/// Construye el documento del plan con `Document::new`, `AddLayer`, escrituras y ajustes.
fn builds(plan: &PlanDoc) -> Document {
    let mode_2 = mode_of(plan.kind);
    let mut img = match mode_2 {
        Mode::Indexed => img_indexed(plan.w, plan.h, &colors_different(plan.palette_2), Some(0)),
        other => Image::new(plan.w, plan.h, other).expect("imagen"),
    };
    for &(x, y, s) in &plan.layers_2[0].pixels {
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
        if !layer_2.empty && !layer_2.pixels.is_empty() {
            let edits: Vec<(u32, u32, Pixel)> = layer_2
                .pixels
                .iter()
                .map(|&(x, y, s)| {
                    (
                        u32::from(x) % plan.w,
                        u32::from(y) % plan.h,
                        pixel_de(mode_2, s, plan.palette_2),
                    )
                })
                .collect();
            let image = d.layers()[i].cel();
            d.apply(set_px(image, &edits)).expect("escribir en la capa");
        }
    }
    let indexed = mode_2 == Mode::Indexed;
    for (i, layer_2) in plan.layers_2.iter().enumerate() {
        let layer = d.layers()[i].id();
        let (opacity, blend) = if indexed {
            (255, BlendMode::Normal)
        } else {
            (
                layer_2.opacity_2,
                BlendMode::ALL[usize::from(layer_2.blend_2 % 8)],
            )
        };
        adjusts(&mut d, Command::SetLayerOpacity { layer, opacity });
        adjusts(&mut d, Command::SetLayerBlend { layer, blend });
        adjusts(
            &mut d,
            Command::SetLayerVisible {
                layer,
                visible: layer_2.visible,
            },
        );
        adjusts(
            &mut d,
            Command::SetLayerLocked {
                layer,
                locked: layer_2.locked_2,
            },
        );
    }
    d
}

/// Invariante de 3.1: toda celda con imagen tiene el ancho, alto y modo del lienzo, y todas las
/// paletas son idénticas entre sí y a `palette()`.
fn invariants(doc: &Document) -> Result<(), TestCaseError> {
    prop_assert_eq!(doc.palette().is_some(), doc.mode() == Mode::Indexed);
    for layer_2 in doc.layers() {
        if let Some(img) = doc.image(layer_2.cel()) {
            prop_assert_eq!(
                (img.width(), img.height(), img.mode()),
                (doc.width(), doc.height(), doc.mode()),
                "{:?}",
                layer_2.id()
            );
            prop_assert_eq!(img.palette(), doc.palette(), "paleta de {:?}", layer_2.id());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: ConvertAll y las paletas.
// ---------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 3: tras cada `ConvertAll`, `undo` y `redo`, toda celda con imagen tiene el
    /// ancho, alto y modo del lienzo y todas las paletas son idénticas entre sí y a `palette()`;
    /// `analyze_cels` da el mismo informe y la misma paleta que `convert_cels` (que a su vez es
    /// lo que `ConvertAll` aplica); y con una capa con imagen, `ConvertAll` y `ConvertMode` dan
    /// documentos `==`.
    #[test]
    fn property_3_convert_all_and_the_palettes(
        plan in plan_strategy(1..=3),
        steps in prop::collection::vec(
            (any::<u8>(), any::<bool>(), any::<bool>(), any::<u8>()),
            1..=4,
        ),
    ) {
        let mut doc = builds(&plan);
        invariants(&doc)?;

        // Una capa: `ConvertAll` y `ConvertMode` dan el mismo documento.
        if doc.layers().len() == 1 {
            let (m, with_luma, alpha_2, background) = steps[0];
            let target = mode_any(m);
            let allowed = Allowed { luma: with_luma, drop_alpha: alpha_2.then_some([background; 3]) };
            let cel = doc.layers()[0].cel();
            let (mut with_mode, mut with_all) = (doc.clone(), doc.clone());
            let r1 = with_mode.apply(Command::ConvertMode { image: cel, target, allowed });
            let r2 = with_all.apply(convert_all(target, allowed));
            prop_assert_eq!(r1.is_ok(), r2.is_ok(), "{:?} vs {:?}", r1, r2);
            if r1.is_ok() {
                prop_assert_eq!(&r1, &r2);
                prop_assert_eq!(&with_all, &with_mode);
                prop_assert_eq!(with_all.mode(), with_mode.mode());
                prop_assert_eq!(with_all.palette(), with_mode.palette());
            }
        }

        for (m, with_luma, alpha_2, background) in steps {
            let target = mode_any(m);
            let allowed = Allowed { luma: with_luma, drop_alpha: alpha_2.then_some([background; 3]) };
            let before = Snapshot::de(&doc);

            // `analyze_cels` y `convert_cels` sobre las celdas de ahora (no se prueba el mismo
            // modo de origen y destino).
            let size = (doc.width(), doc.height());
            let compared = if target == doc.mode() {
                None
            } else {
                let cels = cels_2(&doc);
                let analysis = analyze_cels(&cels, size, doc.mode(), target, allowed)
                    .map(|a| (a.report, a.palette));
                let conversion = convert_cels(&cels, size, doc.mode(), target, allowed)
                    .map(|c| (c.report, c.palette, c.images));
                Some((analysis, conversion))
            };
            if let Some((analysis, conversion)) = &compared {
                match (analysis, conversion) {
                    (Ok((r1, p1)), Ok((r2, p2, _))) => {
                        prop_assert_eq!(r1, r2);
                        prop_assert_eq!(p1, p2);
                    }
                    (Err(e1), Err(e2)) => prop_assert_eq!(e1, e2),
                    (a, c) => prop_assert!(
                        false,
                        "analyze y convert discrepan: {:?} / {:?}",
                        a.as_ref().map(|_| ()),
                        c.as_ref().map(|_| ())
                    ),
                }
            }

            let result = doc.apply(convert_all(target, allowed));
            match (&result, &compared) {
                (Ok(true), Some((_, Ok((_, palette, images))))) => {
                    prop_assert_eq!(doc.mode(), target);
                    prop_assert_eq!(doc.palette(), palette.as_ref());
                    for (layer_2, expected) in doc.layers().iter().zip(images) {
                        prop_assert_eq!(doc.image(layer_2.cel()), expected.as_ref());
                    }
                    invariants(&doc)?;
                    let after = Snapshot::de(&doc);
                    doc.undo().map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                    if let Some(reason) = before.difference_state(&doc) {
                        return Err(TestCaseError::fail(format!("tras undo: {reason}")));
                    }
                    invariants(&doc)?;
                    doc.redo().map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                    if let Some(reason) = after.difference_state(&doc) {
                        return Err(TestCaseError::fail(format!("tras redo: {reason}")));
                    }
                    invariants(&doc)?;
                }
                (Ok(true), _) => prop_assert!(false, "ConvertAll aplicó lo que convert_cels rechaza"),
                (Ok(false), _) => {
                    prop_assert_eq!(target, before.mode_2);
                    before.requires(&doc, "Ok(false)")?;
                }
                (Err(error), compared) => {
                    before.requires(&doc, &format!("{error:?}"))?;
                    if let (DocumentError::Convert(e), Some((_, Err(expected_2)))) = (error, compared) {
                        prop_assert_eq!(e, expected_2);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2 (en lo que toca a ConvertAll): ida y vuelta.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    /// Modo y si se permite todo lo que se pueda perder.
    ConvertAll(u8, bool),
    ConvertMode(usize, u8),
    Opacity(usize, u8),
    Blend(usize, u8),
    Visible(usize, bool),
    Paint(usize, Vec<(u8, u8, u8)>),
    Pal(u8, usize, u8),
    Add(usize),
    Remove(usize),
}

fn op() -> BoxedStrategy<Op> {
    prop_oneof![
        8 => (0u8..4, any::<bool>()).prop_map(|(m, p)| Op::ConvertAll(m, p)),
        2 => (any::<usize>(), 0u8..4).prop_map(|(k, m)| Op::ConvertMode(k, m)),
        2 => (any::<usize>(), any::<u8>()).prop_map(|(k, o)| Op::Opacity(k, o)),
        2 => (any::<usize>(), 0u8..8).prop_map(|(k, m)| Op::Blend(k, m)),
        1 => (any::<usize>(), any::<bool>()).prop_map(|(k, v)| Op::Visible(k, v)),
        4 => (any::<usize>(), prop::collection::vec((0u8..4, 0u8..4, any::<u8>()), 0..4))
            .prop_map(|(k, p)| Op::Paint(k, p)),
        2 => (any::<u8>(), any::<usize>(), any::<u8>()).prop_map(|(p, k, s)| Op::Pal(p, k, s)),
        1 => any::<usize>().prop_map(Op::Add),
        1 => any::<usize>().prop_map(Op::Remove),
    ]
    .boxed()
}

fn translates(doc: &Document, op: &Op) -> Command {
    let n = doc.layers().len();
    let layer_2 = |k: &usize| doc.layers()[k % n].id();
    let cel = |k: &usize| doc.layers()[k % n].cel();
    match op {
        Op::ConvertAll(m, permission) => convert_all(
            mode_any(*m),
            if *permission { all() } else { Allowed::NOTHING },
        ),
        Op::ConvertMode(k, m) => Command::ConvertMode {
            image: cel(k),
            target: mode_any(*m),
            allowed: all(),
        },
        Op::Opacity(k, opacity) => Command::SetLayerOpacity {
            layer: layer_2(k),
            opacity: *opacity,
        },
        Op::Blend(k, m) => Command::SetLayerBlend {
            layer: layer_2(k),
            blend: BlendMode::ALL[usize::from(*m % 8)],
        },
        Op::Visible(k, visible) => Command::SetLayerVisible {
            layer: layer_2(k),
            visible: *visible,
        },
        Op::Paint(k, p) => {
            let palette_2 = doc.palette().map_or(1, Palette::len);
            let edits: Vec<(u32, u32, Pixel)> = p
                .iter()
                .map(|&(x, y, s)| {
                    (
                        u32::from(x) % doc.width(),
                        u32::from(y) % doc.height(),
                        pixel_de(doc.mode(), s, palette_2),
                    )
                })
                .collect();
            set_px(cel(k), &edits)
        }
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
        Op::Add(at) => Command::AddLayer {
            name: "n".to_string(),
            at: at % (n + 1),
        },
        Op::Remove(k) => Command::RemoveLayer { layer: layer_2(k) },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 2: tras una secuencia con `ConvertAll`, `ConvertMode`, capas, paleta y pintura
    /// (con fallos: permisos que faltan, destino gris con varias capas, opacidad en indexado),
    /// deshacer cada `Ok(true)` devuelve el documento de partida con los mismos ids, modo y
    /// paleta del lienzo, `state_id` e historia; rehacerlos, el final; y todo `Err` u `Ok(false)`
    /// deja documento, `state_id`, historia y pilas iguales.
    #[test]
    fn property_2_convert_all_round_trip(
        plan in plan_strategy(1..=3),
        ops in prop::collection::vec(op(), 0..=20),
    ) {
        let mut doc = builds(&plan);
        doc.set_history_limit(usize::MAX);
        let initial = Snapshot::de(&doc);
        let mut oks = 0usize;

        for order in &ops {
            let before = Snapshot::de(&doc);
            let result = doc.apply(translates(&doc, order));
            match &result {
                Ok(true) => {
                    oks += 1;
                    prop_assert_ne!(doc.state_id(), before.state, "{:?}", order);
                    invariants(&doc)?;
                }
                _ => before.requires(&doc, &format!("{order:?} -> {result:?}"))?,
            }
        }

        let final_ = Snapshot::de(&doc);
        for i in 0..oks {
            prop_assert_eq!(doc.undo(), Ok(()), "deshacer {}", i);
        }
        if let Some(reason) = initial.difference_state(&doc) {
            return Err(TestCaseError::fail(format!("tras deshacer todo: {reason}")));
        }
        prop_assert_eq!(doc.history_bytes(), initial.history);
        invariants(&doc)?;
        for i in 0..oks {
            prop_assert_eq!(doc.redo(), Ok(()), "rehacer {}", i);
        }
        if let Some(reason) = final_.difference_state(&doc) {
            return Err(TestCaseError::fail(format!("tras rehacer todo: {reason}")));
        }
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    }
}
