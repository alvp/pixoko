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
const ROJO: Rgba = Rgba::new(255, 0, 0, 255);
const VERDE: Rgba = Rgba::new(0, 255, 0, 255);
const AZUL: Rgba = Rgba::new(0, 0, 255, 255);

fn img_rgba(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Rgba).expect("imagen RGBA")
}

fn img_indexada(w: u32, h: u32, colores: &[Rgba], t: Option<u8>) -> Image {
    let mut paleta = Palette::from_colors(colores.iter().copied()).expect("paleta");
    paleta
        .set_transparent_index(t)
        .expect("índice transparente");
    Image::new_indexed(w, h, paleta).expect("imagen indexada")
}

fn colores_distintos(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

/// Todo permitido: `luma` y descartar el alfa sobre negro.
fn todo() -> Allowed {
    Allowed {
        luma: true,
        drop_alpha: Some([0, 0, 0]),
    }
}

fn solo_luma() -> Allowed {
    Allowed {
        luma: true,
        drop_alpha: None,
    }
}

fn anade(doc: &mut Document, nombre: &str, at: usize) -> LayerId {
    let cmd = Command::AddLayer {
        name: nombre.to_string(),
        at,
    };
    assert_eq!(doc.apply(cmd), Ok(true), "añadir {nombre}");
    doc.layers()[at].id()
}

fn celda(doc: &Document, capa: LayerId) -> ImageId {
    doc.layer(capa).expect("la capa existe").cel()
}

fn ids(doc: &Document) -> Vec<(LayerId, ImageId)> {
    doc.layers().iter().map(|l| (l.id(), l.cel())).collect()
}

fn pone_px(image: ImageId, ediciones: &[(u32, u32, Pixel)]) -> Command {
    Command::SetPixels {
        image,
        pixels: ediciones
            .iter()
            .map(|&(x, y, pixel)| PixelEdit { x, y, pixel })
            .collect(),
    }
}

fn pone(image: ImageId, ediciones: &[(u32, u32, Rgba)]) -> Command {
    let v: Vec<(u32, u32, Pixel)> = ediciones
        .iter()
        .map(|&(x, y, c)| (x, y, Pixel::Rgba(c)))
        .collect();
    pone_px(image, &v)
}

/// Rellena de `c` toda una celda RGBA de 4 x 3.
fn rellena_todo(image: ImageId, c: Rgba) -> Command {
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

fn ajusta(doc: &mut Document, cmd: Command) {
    doc.apply(cmd).expect("propiedad de capa");
}

/// Los píxeles de una imagen, fila a fila.
fn pixeles(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).expect("píxel dentro"));
        }
    }
    v
}

/// La celda de la capa, que ha de tener imagen.
fn imagen(doc: &Document, capa: LayerId) -> &Image {
    doc.image(celda(doc, capa)).expect("la celda tiene imagen")
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

    /// Contenido: documento, ids, modo y paleta del lienzo, `state_id` y bytes materializados.
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

    /// Además, la historia y las pilas.
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

fn convertir_todo(target: Mode, allowed: Allowed) -> Command {
    Command::ConvertAll { target, allowed }
}

/// Tras un `ConvertAll` con `Ok(true)`: un solo `undo` devuelve lo de antes (documento, ids,
/// modo y paleta del lienzo, `state_id`) y un `redo` lo de después.
fn deshace_y_rehace(doc: &mut Document, antes: &Foto) {
    let despues = Foto::de(doc);
    doc.undo().expect("deshacer");
    assert_eq!(antes.diferencia_estado(doc), None, "tras deshacer");
    assert!(doc.can_redo());
    doc.redo().expect("rehacer");
    assert_eq!(despues.diferencia_estado(doc), None, "tras rehacer");
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
fn doc_tres(props: bool) -> (Document, [LayerId; 3]) {
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
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    let cc = celda(&d, c);
    let ediciones = [(3, 2, AZUL), (0, 0, Rgba::new(10, 10, 10, 255))];
    assert_eq!(d.apply(pone(cc, &ediciones)), Ok(true));
    if props {
        ajusta(
            &mut d,
            Command::SetLayerOpacity {
                layer: a,
                opacity: 200,
            },
        );
        ajusta(
            &mut d,
            Command::SetLayerVisible {
                layer: b,
                visible: false,
            },
        );
        ajusta(
            &mut d,
            Command::SetLayerBlend {
                layer: c,
                blend: BlendMode::Multiply,
            },
        );
        ajusta(
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
fn doc_gris_alfa() -> (Document, [LayerId; 3]) {
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
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    let edicion = [(
        2,
        2,
        Pixel::GrayAlpha {
            gray: 200,
            alpha: 255,
        },
    )];
    assert_eq!(d.apply(pone_px(celda(&d, c), &edicion)), Ok(true));
    (d, [a, b, c])
}

// ---------------------------------------------------------------------------------------
// Fila 3: ConvertAll con tres capas.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_03_a_rgba_a_gris_con_alfa_con_luma_en_un_paso() {
    let (mut d, [a, b, c]) = doc_tres(true);
    let antes = Foto::de(&d);
    let capas = d.layers().to_vec();
    let originales: Vec<Option<Image>> = capas.iter().map(|l| d.image(l.cel()).cloned()).collect();

    assert_eq!(
        d.apply(convertir_todo(Mode::GrayAlpha, solo_luma())),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::GrayAlpha);
    assert_eq!(d.palette(), None);
    assert_eq!((d.width(), d.height()), (4, 3));
    assert_eq!(d.layers().len(), 3);
    for (nueva, vieja) in d.layers().iter().zip(&capas) {
        // Las propiedades de la capa no cambian: solo la imagen de su celda.
        assert_eq!(nueva.id(), vieja.id());
        assert_eq!(nueva.cel(), vieja.cel());
        assert_eq!(nueva.name(), vieja.name());
        assert_eq!(nueva.visible(), vieja.visible());
        assert_eq!(nueva.opacity(), vieja.opacity());
        assert_eq!(nueva.blend(), vieja.blend());
        assert_eq!(nueva.locked(), vieja.locked());
    }
    // La celda vacía sigue vacía; las otras tienen el modo del lienzo y el gris por `luma`.
    assert!(d.image(celda(&d, b)).is_none());
    for (capa, original) in [(a, &originales[0]), (c, &originales[2])] {
        let nueva = imagen(&d, capa);
        assert_eq!((nueva.width(), nueva.height()), (4, 3));
        assert_eq!(nueva.mode(), Mode::GrayAlpha);
        assert_eq!(nueva.palette(), None);
        let esperado: Vec<Pixel> = pixeles(original.as_ref().expect("original"))
            .into_iter()
            .map(|p| match p {
                Pixel::Rgba(c) if c.a == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
                Pixel::Rgba(c) => Pixel::GrayAlpha {
                    gray: luma(c.r, c.g, c.b),
                    alpha: c.a,
                },
                otro => panic!("se esperaba RGBA, hay {otro:?}"),
            })
            .collect();
        assert_eq!(pixeles(nueva), esperado, "{capa:?}");
    }
    // Dos celdas de 4 x 3 x 2 bytes.
    assert_eq!(d.materialized_bytes(), 48);
    assert_ne!(d.state_id(), antes.estado);

    deshace_y_rehace(&mut d, &antes);
}

#[test]
fn fila_03_b_a_indexado_una_paleta_para_todas_con_el_transparente_en_el_0() {
    // [A, B vacía, C]. Se recorre de abajo arriba y cada celda por filas: primero todo A (ROJO,
    // VERDE) y luego C (AZUL), aunque el AZUL de C esté en (0, 0), antes que lo de A en el
    // recorrido de arriba abajo.
    let mut img = img_rgba(4, 3);
    for (x, c) in [(0, ROJO), (1, VERDE), (2, ROJO)] {
        img.set_pixel(x, 0, Pixel::Rgba(c)).expect("píxel");
    }
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    assert_eq!(
        d.apply(pone(celda(&d, c), &[(0, 0, AZUL), (1, 1, VERDE)])),
        Ok(true)
    );
    let antes = Foto::de(&d);

    assert_eq!(
        d.apply(convertir_todo(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::Indexed);
    let paleta = d.palette().expect("el lienzo tiene paleta").clone();
    assert_eq!(paleta.colors(), &[T, ROJO, VERDE, AZUL]);
    assert_eq!(paleta.transparent_index(), Some(0));
    // Una sola paleta, idéntica, para todas las celdas con imagen; la vacía sigue vacía.
    assert!(d.image(celda(&d, b)).is_none());
    for capa in [a, c] {
        assert_eq!(imagen(&d, capa).palette(), Some(&paleta), "{capa:?}");
    }
    let indices = |capa: LayerId| -> Vec<Pixel> { pixeles(imagen(&d, capa)) };
    let esperado = |v: [u8; 12]| -> Vec<Pixel> { v.into_iter().map(Pixel::Indexed).collect() };
    assert_eq!(indices(a), esperado([1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    assert_eq!(indices(c), esperado([3, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0]));
    deshace_y_rehace(&mut d, &antes);

    // Con más de una capa el transparente va en el 0 aunque ningún píxel sea transparente.
    let opaco = Image::filled(4, 3, Pixel::Rgba(ROJO)).expect("imagen");
    let mut d = Document::new(opaco);
    let b = anade(&mut d, "B", 1);
    assert_eq!(d.apply(rellena_todo(celda(&d, b), AZUL)), Ok(true));
    assert_eq!(
        d.apply(convertir_todo(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    let paleta = d.palette().expect("paleta");
    assert_eq!(paleta.colors(), &[T, ROJO, AZUL]);
    assert_eq!(paleta.transparent_index(), Some(0));

    // Con una sola capa opaca y con imagen, no hay transparente (como `ConvertMode`).
    let opaco = Image::filled(4, 3, Pixel::Rgba(ROJO)).expect("imagen");
    let mut d = Document::new(opaco);
    assert_eq!(
        d.apply(convertir_todo(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    let paleta = d.palette().expect("paleta");
    assert_eq!(paleta.colors(), &[ROJO]);
    assert_eq!(paleta.transparent_index(), None);
}

#[test]
fn fila_03_c_gris_con_alfa_e_indexado_a_rgba() {
    // Grises con alfa -> RGBA.
    let (mut d, [a, b, c]) = doc_gris_alfa();
    let antes = Foto::de(&d);
    assert_eq!(
        d.apply(convertir_todo(Mode::Rgba, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert!(d.image(celda(&d, b)).is_none());
    let mut esperado_a = vec![Pixel::Rgba(T); 12];
    esperado_a[0] = Pixel::Rgba(Rgba::new(100, 100, 100, 255));
    esperado_a[1] = Pixel::Rgba(Rgba::new(50, 50, 50, 128));
    assert_eq!(pixeles(imagen(&d, a)), esperado_a);
    let mut esperado_c = vec![Pixel::Rgba(T); 12];
    esperado_c[10] = Pixel::Rgba(Rgba::new(200, 200, 200, 255));
    assert_eq!(pixeles(imagen(&d, c)), esperado_c);
    // Dos celdas de 4 x 3 x 4 bytes.
    assert_eq!(d.materialized_bytes(), 96);
    deshace_y_rehace(&mut d, &antes);

    // Indexado -> RGBA: el lienzo pierde la paleta y `undo` se la devuelve.
    let mut img = img_indexada(4, 3, &[ROJO, VERDE, AZUL], Some(0));
    img.set_pixel(0, 0, Pixel::Indexed(1)).expect("píxel");
    img.set_pixel(1, 0, Pixel::Indexed(2)).expect("píxel");
    let mut d = Document::new(img);
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    let c = anade(&mut d, "C", 2);
    assert_eq!(
        d.apply(pone_px(celda(&d, c), &[(3, 2, Pixel::Indexed(1))])),
        Ok(true)
    );
    let antes = Foto::de(&d);
    assert!(antes.paleta.is_some());
    assert_eq!(
        d.apply(convertir_todo(Mode::Rgba, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::Rgba);
    assert_eq!(d.palette(), None);
    assert!(d.image(celda(&d, b)).is_none());
    let mut esperado_a = vec![Pixel::Rgba(T); 12];
    esperado_a[0] = Pixel::Rgba(VERDE);
    esperado_a[1] = Pixel::Rgba(AZUL);
    assert_eq!(pixeles(imagen(&d, a)), esperado_a);
    let mut esperado_c = vec![Pixel::Rgba(T); 12];
    esperado_c[11] = Pixel::Rgba(VERDE);
    assert_eq!(pixeles(imagen(&d, c)), esperado_c);
    assert_eq!(imagen(&d, a).palette(), None);
    deshace_y_rehace(&mut d, &antes);
    // Tras el `redo` de arriba el lienzo vuelve a estar en RGBA; tras un `undo`, con su paleta.
    d.undo().expect("deshacer");
    assert_eq!(d.palette(), antes.paleta.as_ref());
    assert_eq!(imagen(&d, a).palette(), antes.paleta.as_ref());
}

#[test]
fn fila_03_d_errores_previos_mismo_modo_y_convert_mode_con_varias_capas() {
    // A gris con más de una capa (con todos los permisos): no hay valor transparente.
    let (mut d, [a, _, c]) = doc_tres(false);
    sin_efecto(
        &mut d,
        convertir_todo(Mode::Gray, todo()),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );
    let (mut g, _) = doc_gris_alfa();
    sin_efecto(
        &mut g,
        convertir_todo(Mode::Gray, todo()),
        Err(DocumentError::NoTransparency { mode: Mode::Gray }),
    );

    // A indexado con opacidad o con mezcla distintas de las de siempre.
    ajusta(
        &mut d,
        Command::SetLayerOpacity {
            layer: c,
            opacity: 128,
        },
    );
    sin_efecto(
        &mut d,
        convertir_todo(Mode::Indexed, todo()),
        Err(DocumentError::IndexedOpacity),
    );
    ajusta(
        &mut d,
        Command::SetLayerBlend {
            layer: a,
            blend: BlendMode::Multiply,
        },
    );
    // Con las dos cosas, gana la opacidad.
    sin_efecto(
        &mut d,
        convertir_todo(Mode::Indexed, todo()),
        Err(DocumentError::IndexedOpacity),
    );
    ajusta(
        &mut d,
        Command::SetLayerOpacity {
            layer: c,
            opacity: 255,
        },
    );
    sin_efecto(
        &mut d,
        convertir_todo(Mode::Indexed, todo()),
        Err(DocumentError::IndexedBlend),
    );

    // Mismo modo: `Ok(false)` y nada cambia.
    sin_efecto(&mut d, convertir_todo(Mode::Rgba, todo()), Ok(false));

    // `ConvertMode` con varias capas sigue dando `MultiLayerConvert`.
    let cel = celda(&d, a);
    sin_efecto(
        &mut d,
        Command::ConvertMode {
            image: cel,
            target: Mode::GrayAlpha,
            allowed: todo(),
        },
        Err(DocumentError::MultiLayerConvert { layers: 3 }),
    );

    // Solo como comando suelto.
    sin_efecto(
        &mut d,
        Command::Batch(vec![convertir_todo(Mode::GrayAlpha, todo())]),
        Err(DocumentError::InBatch {
            index: 0,
            source: Box::new(DocumentError::NotBatchable),
        }),
    );
}

#[test]
fn fila_03_e_con_una_capa_convert_all_y_convert_mode_dan_documentos_iguales() {
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
        (Mode::GrayAlpha, solo_luma()),
        (Mode::Indexed, Allowed::NOTHING),
        (Mode::Gray, todo()),
    ] {
        let (mut con_mode, mut con_all) = (original.clone(), original.clone());
        let r1 = con_mode.apply(Command::ConvertMode {
            image: cel,
            target,
            allowed,
        });
        let r2 = con_all.apply(convertir_todo(target, allowed));
        assert_eq!(r1, Ok(true), "{target:?}");
        assert_eq!(r2, r1, "{target:?}");
        assert_eq!(con_all, con_mode, "{target:?}");
        assert_eq!(con_all.mode(), con_mode.mode(), "{target:?}");
        assert_eq!(con_all.palette(), con_mode.palette(), "{target:?}");
        // `undo` exacto en el lienzo.
        con_all.undo().expect("deshacer");
        assert_eq!(con_all, original);
        assert_eq!(con_all.mode(), Mode::Rgba);
        assert_eq!(con_all.palette(), None);
    }

    // La única celda vacía (el caso que `ConvertMode` no admite): `ConvertAll` convierte el
    // lienzo y la celda sigue vacía.
    let mut d = Document::new(img_rgba(4, 3));
    let a = d.layers()[0].id();
    let b = anade(&mut d, "B", 1);
    assert_eq!(d.apply(Command::RemoveLayer { layer: a }), Ok(true));
    assert_eq!(d.layers().len(), 1);
    let antes = Foto::de(&d);
    assert_eq!(
        d.apply(convertir_todo(Mode::GrayAlpha, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.mode(), Mode::GrayAlpha);
    assert_eq!(d.palette(), None);
    assert!(d.image(celda(&d, b)).is_none());
    deshace_y_rehace(&mut d, &antes);
    assert_eq!(
        d.apply(convertir_todo(Mode::Indexed, Allowed::NOTHING)),
        Ok(true)
    );
    // Las celdas vacías valen transparente: la paleta lleva el transparente en el 0.
    // (Aquí el lienzo ya estaba en GrayAlpha tras el `redo`.)
    let paleta = d.palette().expect("paleta");
    assert_eq!(paleta.colors(), &[T]);
    assert_eq!(paleta.transparent_index(), Some(0));
    assert!(d.image(celda(&d, b)).is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 4: informe y errores.
// ---------------------------------------------------------------------------------------

/// Las celdas con imagen del documento, en orden de capas (de abajo arriba).
fn celdas(doc: &Document) -> Vec<Option<&Image>> {
    doc.layers().iter().map(|l| doc.image(l.cel())).collect()
}

#[test]
fn fila_04_a_el_informe_suma_todas_las_celdas() {
    // Dos capas con 3 y 5 píxeles de color (más un gris y un transparente que no cuentan).
    let mut img = img_rgba(4, 3);
    for (x, y, c) in [
        (0, 0, ROJO),
        (1, 0, VERDE),
        (2, 0, AZUL),
        (3, 2, Rgba::new(9, 9, 9, 255)),
    ] {
        img.set_pixel(x, y, Pixel::Rgba(c)).expect("píxel");
    }
    let mut d = Document::new(img);
    let b = anade(&mut d, "B", 1);
    let color = [
        (0, 0, Rgba::new(10, 20, 30, 255)),
        (1, 0, Rgba::new(30, 20, 10, 255)),
        (2, 0, Rgba::new(1, 2, 3, 255)),
        (3, 0, Rgba::new(255, 0, 0, 128)),
        (0, 1, Rgba::new(0, 99, 0, 255)),
    ];
    assert_eq!(d.apply(pone(celda(&d, b), &color)), Ok(true));
    let suma = ConversionReport {
        color_lost: 8,
        alpha_lost: 0,
    };

    // Sin permiso: el informe sumado y el documento intacto.
    sin_efecto(
        &mut d,
        convertir_todo(Mode::GrayAlpha, Allowed::NOTHING),
        Err(DocumentError::Convert(ConvertError::LossNotAllowed {
            report: suma,
        })),
    );

    // `analyze_cels` da el mismo informe, y `convert_cels` también al convertir con permiso.
    let size = (d.width(), d.height());
    let cels = celdas(&d);
    let e = error_de(analyze_cels(
        &cels,
        size,
        Mode::Rgba,
        Mode::GrayAlpha,
        Allowed::NOTHING,
    ));
    assert_eq!(e, ConvertError::LossNotAllowed { report: suma });
    let e = error_de(convert_cels(
        &cels,
        size,
        Mode::Rgba,
        Mode::GrayAlpha,
        Allowed::NOTHING,
    ));
    assert_eq!(e, ConvertError::LossNotAllowed { report: suma });
    let analisis =
        analyze_cels(&cels, size, Mode::Rgba, Mode::GrayAlpha, solo_luma()).expect("con permiso");
    assert_eq!(analisis.report, suma);
    assert_eq!(analisis.palette, None);
    let conversion =
        convert_cels(&cels, size, Mode::Rgba, Mode::GrayAlpha, solo_luma()).expect("con permiso");
    assert_eq!(conversion.report, suma);
    assert_eq!(conversion.palette, None);
    assert_eq!(conversion.images.len(), 2);
    assert!(
        conversion
            .images
            .iter()
            .all(|i| i.as_ref().is_some_and(|i| i.mode() == Mode::GrayAlpha))
    );
    let imagenes = conversion.images;

    // Con permiso, `ConvertAll` aplica exactamente esas imágenes.
    assert_eq!(
        d.apply(convertir_todo(Mode::GrayAlpha, solo_luma())),
        Ok(true)
    );
    for (capa, esperada) in d.layers().iter().zip(&imagenes) {
        assert_eq!(d.image(capa.cel()), esperada.as_ref());
    }
}

#[test]
fn fila_04_b_la_union_de_colores_de_todas_las_celdas_cuenta() {
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
    let exceso = ConvertError::TooManyColors { at_least: 257 };

    let solo_a: Vec<Option<&Image>> = vec![Some(&a)];
    convert_cels(&solo_a, size, Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
        .expect("cada una cabe");
    let ambas: Vec<Option<&Image>> = vec![Some(&a), Some(&b)];
    assert_eq!(
        error_de(analyze_cels(
            &ambas,
            size,
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        exceso
    );
    assert_eq!(
        error_de(convert_cels(
            &ambas,
            size,
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        exceso
    );

    // Y en el documento: error y nada cambia.
    let mut d = Document::new(a.clone());
    let capa_b = anade(&mut d, "B", 1);
    let ediciones: Vec<(u32, u32, Pixel)> = (0..200usize)
        .map(|i| ((i % 20) as u32, (i / 20) as u32, Pixel::Rgba(color(i, 9))))
        .collect();
    assert_eq!(d.apply(pone_px(celda(&d, capa_b), &ediciones)), Ok(true));
    sin_efecto(
        &mut d,
        convertir_todo(Mode::Indexed, Allowed::NOTHING),
        Err(DocumentError::Convert(exceso.clone())),
    );

    // Es la unión, no la suma: dos celdas con los mismos 200 colores caben (200 + transparente).
    let misma = invertida(&a);
    let cels: Vec<Option<&Image>> = vec![Some(&a), Some(&misma)];
    let conversion = convert_cels(&cels, size, Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
        .expect("la unión cabe");
    let paleta = conversion.palette.expect("paleta común");
    assert_eq!(paleta.len(), 201);
    assert_eq!(paleta.transparent_index(), Some(0));

    // La entrada transparente cuenta entre las 256: 255 colores y el transparente caben, 256 y
    // el transparente no (la segunda celda vacía obliga a tener transparente).
    let cuadrada = |distintos: usize| -> Image {
        let mut img = img_rgba(16, 16);
        for i in 0..256usize {
            let c = Rgba::new((i % distintos) as u8, 1, 2, 255);
            img.set_pixel((i % 16) as u32, (i / 16) as u32, Pixel::Rgba(c))
                .expect("píxel");
        }
        img
    };
    let (con_256, con_255) = (cuadrada(256), cuadrada(255));
    let cels: Vec<Option<&Image>> = vec![Some(&con_255), None];
    let conversion = convert_cels(&cels, (16, 16), Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
        .expect("255 + transparente = 256");
    assert_eq!(conversion.palette.expect("paleta").len(), 256);
    let cels: Vec<Option<&Image>> = vec![Some(&con_256), None];
    assert_eq!(
        error_de(convert_cels(
            &cels,
            (16, 16),
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        exceso
    );
    assert_eq!(
        error_de(analyze_cels(
            &cels,
            (16, 16),
            Mode::Rgba,
            Mode::Indexed,
            Allowed::NOTHING
        )),
        exceso
    );
}

/// Copia de `a` con los mismos colores en otro orden de píxeles (girada 180 grados).
fn invertida(a: &Image) -> Image {
    let mut copia = a.clone();
    let (w, h) = (a.width(), a.height());
    for y in 0..h {
        for x in 0..w {
            let origen = a.get_pixel(w - 1 - x, h - 1 - y).expect("píxel");
            copia.set_pixel(x, y, origen).expect("píxel");
        }
    }
    copia
}

#[test]
fn fila_04_c_cels_too_large_antes_de_reservar() {
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
    let b = anade(&mut d, "B", 1);
    anade(&mut d, "C", 2);
    let edicion = [(
        1,
        1,
        Pixel::GrayAlpha {
            gray: 7,
            alpha: 255,
        },
    )];
    assert_eq!(d.apply(pone_px(celda(&d, b), &edicion)), Ok(true));
    assert_eq!(d.materialized_bytes(), 48);

    // Justo por debajo de 96: error con los datos de 3.1, sin tocar nada.
    d.set_cells_limit(95);
    sin_efecto(
        &mut d,
        convertir_todo(Mode::Rgba, Allowed::NOTHING),
        Err(DocumentError::CelsTooLarge {
            needed: 96,
            limit: 95,
        }),
    );
    // Justo en 96 vale.
    d.set_cells_limit(96);
    assert_eq!(
        d.apply(convertir_todo(Mode::Rgba, Allowed::NOTHING)),
        Ok(true)
    );
    assert_eq!(d.materialized_bytes(), 96);
}

#[test]
fn fila_04_d_convert_cels_y_analyze_cels_directos() {
    // `TooLarge` por el tamaño del destino, antes de mirar ninguna celda: 32768 x 32768 x 4
    // bytes supera `Image::MAX_BYTES`, y no hay imágenes que reservar.
    let nada: Vec<Option<&Image>> = vec![None, None];
    let grande = ConvertError::TooLarge {
        width: 32768,
        height: 32768,
    };
    assert_eq!(
        error_de(convert_cels(
            &nada,
            (32768, 32768),
            Mode::GrayAlpha,
            Mode::Rgba,
            Allowed::NOTHING
        )),
        grande
    );
    assert_eq!(
        error_de(analyze_cels(
            &nada,
            (32768, 32768),
            Mode::GrayAlpha,
            Mode::Rgba,
            Allowed::NOTHING
        )),
        grande
    );

    // `images[i]` es `None` si `cels[i]` lo es; `palette` solo con destino indexado.
    let mut color = img_rgba(4, 3);
    color
        .set_pixel(1, 1, Pixel::Rgba(Rgba::new(200, 100, 50, 255)))
        .expect("píxel");
    let cels: Vec<Option<&Image>> = vec![None, Some(&color), None];
    let conversion =
        convert_cels(&cels, (4, 3), Mode::Rgba, Mode::GrayAlpha, solo_luma()).expect("convertir");
    assert_eq!(conversion.images.len(), 3);
    assert!(conversion.images[0].is_none() && conversion.images[2].is_none());
    assert_eq!(conversion.palette, None);
    // Con una sola celda con imagen devuelve lo mismo que `convert_image`.
    let esperada = convert_image(&color, Mode::GrayAlpha, solo_luma()).expect("convert_image");
    assert_eq!(conversion.images[1].as_ref(), Some(&esperada.image));
    assert_eq!(conversion.report, esperada.report);

    // A indexado, con una celda con transparentes y con otra totalmente opaca.
    let opaca = Image::filled(4, 3, Pixel::Rgba(ROJO)).expect("imagen");
    for img in [&color, &opaca] {
        let una: Vec<Option<&Image>> = vec![Some(img)];
        let conversion = convert_cels(&una, (4, 3), Mode::Rgba, Mode::Indexed, Allowed::NOTHING)
            .expect("convertir");
        let esperada = convert_image(img, Mode::Indexed, Allowed::NOTHING).expect("convert_image");
        assert_eq!(conversion.images[0].as_ref(), Some(&esperada.image));
        assert_eq!(conversion.palette.as_ref(), esperada.image.palette());
        assert_eq!(conversion.report, esperada.report);
    }

    // Con una celda vacía de por medio, el transparente va en el 0 aunque todo sea opaco.
    let con_vacia: Vec<Option<&Image>> = vec![Some(&opaca), None];
    let conversion = convert_cels(
        &con_vacia,
        (4, 3),
        Mode::Rgba,
        Mode::Indexed,
        Allowed::NOTHING,
    )
    .expect("convertir");
    let paleta = conversion.palette.expect("paleta");
    assert_eq!(paleta.colors(), &[T, ROJO]);
    assert_eq!(paleta.transparent_index(), Some(0));
    assert!(conversion.images[1].is_none());
    let analisis = analyze_cels(
        &con_vacia,
        (4, 3),
        Mode::Rgba,
        Mode::Indexed,
        Allowed::NOTHING,
    )
    .expect("analizar");
    assert_eq!(analisis.palette.as_ref(), Some(&paleta));
}

// ---------------------------------------------------------------------------------------
// Generadores para las propiedades.
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

/// `tipo`: 0 RGBA, 1 grises con alfa, 2 indexado con índice transparente, 3 grises (una capa).
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
        2 => Mode::Indexed,
        _ => Mode::Gray,
    }
}

fn modo_cualquiera(m: u8) -> Mode {
    match m % 4 {
        0 => Mode::Rgba,
        1 => Mode::GrayAlpha,
        2 => Mode::Gray,
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
        0u8..4,
        1u32..=4,
        1u32..=4,
        2usize..=8,
        prop::collection::vec(capa, capas),
    )
        .prop_map(|(tipo, w, h, paleta, mut capas)| {
            // Sin valor transparente en grises no se puede tener más de una capa.
            if tipo == 3 {
                capas.truncate(1);
            }
            PlanDoc {
                tipo,
                w,
                h,
                paleta,
                capas,
            }
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

/// Construye el documento del plan con `Document::new`, `AddLayer`, escrituras y ajustes.
fn construye(plan: &PlanDoc) -> Document {
    let modo = modo_de(plan.tipo);
    let mut img = match modo {
        Mode::Indexed => img_indexada(plan.w, plan.h, &colores_distintos(plan.paleta), Some(0)),
        otro => Image::new(plan.w, plan.h, otro).expect("imagen"),
    };
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
            let ediciones: Vec<(u32, u32, Pixel)> = capa
                .pixeles
                .iter()
                .map(|&(x, y, s)| {
                    (
                        u32::from(x) % plan.w,
                        u32::from(y) % plan.h,
                        pixel_de(modo, s, plan.paleta),
                    )
                })
                .collect();
            let image = d.layers()[i].cel();
            d.apply(pone_px(image, &ediciones))
                .expect("escribir en la capa");
        }
    }
    let indexado = modo == Mode::Indexed;
    for (i, capa) in plan.capas.iter().enumerate() {
        let layer = d.layers()[i].id();
        let (opacity, blend) = if indexado {
            (255, BlendMode::Normal)
        } else {
            (capa.opacidad, BlendMode::ALL[usize::from(capa.mezcla % 8)])
        };
        ajusta(&mut d, Command::SetLayerOpacity { layer, opacity });
        ajusta(&mut d, Command::SetLayerBlend { layer, blend });
        ajusta(
            &mut d,
            Command::SetLayerVisible {
                layer,
                visible: capa.visible,
            },
        );
        ajusta(
            &mut d,
            Command::SetLayerLocked {
                layer,
                locked: capa.bloqueada,
            },
        );
    }
    d
}

/// Invariante de 3.1: toda celda con imagen tiene el ancho, alto y modo del lienzo, y todas las
/// paletas son idénticas entre sí y a `palette()`.
fn invariantes(doc: &Document) -> Result<(), TestCaseError> {
    prop_assert_eq!(doc.palette().is_some(), doc.mode() == Mode::Indexed);
    for capa in doc.layers() {
        if let Some(img) = doc.image(capa.cel()) {
            prop_assert_eq!(
                (img.width(), img.height(), img.mode()),
                (doc.width(), doc.height(), doc.mode()),
                "{:?}",
                capa.id()
            );
            prop_assert_eq!(img.palette(), doc.palette(), "paleta de {:?}", capa.id());
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
    fn propiedad_3_convert_all_y_las_paletas(
        plan in plan_estrategia(1..=3),
        pasos in prop::collection::vec(
            (any::<u8>(), any::<bool>(), any::<bool>(), any::<u8>()),
            1..=4,
        ),
    ) {
        let mut doc = construye(&plan);
        invariantes(&doc)?;

        // Una capa: `ConvertAll` y `ConvertMode` dan el mismo documento.
        if doc.layers().len() == 1 {
            let (m, con_luma, alfa, fondo) = pasos[0];
            let target = modo_cualquiera(m);
            let allowed = Allowed { luma: con_luma, drop_alpha: alfa.then_some([fondo; 3]) };
            let cel = doc.layers()[0].cel();
            let (mut con_mode, mut con_all) = (doc.clone(), doc.clone());
            let r1 = con_mode.apply(Command::ConvertMode { image: cel, target, allowed });
            let r2 = con_all.apply(convertir_todo(target, allowed));
            prop_assert_eq!(r1.is_ok(), r2.is_ok(), "{:?} vs {:?}", r1, r2);
            if r1.is_ok() {
                prop_assert_eq!(&r1, &r2);
                prop_assert_eq!(&con_all, &con_mode);
                prop_assert_eq!(con_all.mode(), con_mode.mode());
                prop_assert_eq!(con_all.palette(), con_mode.palette());
            }
        }

        for (m, con_luma, alfa, fondo) in pasos {
            let target = modo_cualquiera(m);
            let allowed = Allowed { luma: con_luma, drop_alpha: alfa.then_some([fondo; 3]) };
            let antes = Foto::de(&doc);

            // `analyze_cels` y `convert_cels` sobre las celdas de ahora (no se prueba el mismo
            // modo de origen y destino).
            let size = (doc.width(), doc.height());
            let comparadas = if target == doc.mode() {
                None
            } else {
                let cels = celdas(&doc);
                let analisis = analyze_cels(&cels, size, doc.mode(), target, allowed)
                    .map(|a| (a.report, a.palette));
                let conversion = convert_cels(&cels, size, doc.mode(), target, allowed)
                    .map(|c| (c.report, c.palette, c.images));
                Some((analisis, conversion))
            };
            if let Some((analisis, conversion)) = &comparadas {
                match (analisis, conversion) {
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

            let resultado = doc.apply(convertir_todo(target, allowed));
            match (&resultado, &comparadas) {
                (Ok(true), Some((_, Ok((_, palette, images))))) => {
                    prop_assert_eq!(doc.mode(), target);
                    prop_assert_eq!(doc.palette(), palette.as_ref());
                    for (capa, esperada) in doc.layers().iter().zip(images) {
                        prop_assert_eq!(doc.image(capa.cel()), esperada.as_ref());
                    }
                    invariantes(&doc)?;
                    let despues = Foto::de(&doc);
                    doc.undo().map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                    if let Some(motivo) = antes.diferencia_estado(&doc) {
                        return Err(TestCaseError::fail(format!("tras undo: {motivo}")));
                    }
                    invariantes(&doc)?;
                    doc.redo().map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
                    if let Some(motivo) = despues.diferencia_estado(&doc) {
                        return Err(TestCaseError::fail(format!("tras redo: {motivo}")));
                    }
                    invariantes(&doc)?;
                }
                (Ok(true), _) => prop_assert!(false, "ConvertAll aplicó lo que convert_cels rechaza"),
                (Ok(false), _) => {
                    prop_assert_eq!(target, antes.modo);
                    antes.exige(&doc, "Ok(false)")?;
                }
                (Err(error), comparadas) => {
                    antes.exige(&doc, &format!("{error:?}"))?;
                    if let (DocumentError::Convert(e), Some((_, Err(esperado)))) = (error, comparadas) {
                        prop_assert_eq!(e, esperado);
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

fn traduce(doc: &Document, op: &Op) -> Command {
    let n = doc.layers().len();
    let capa = |k: &usize| doc.layers()[k % n].id();
    let cel = |k: &usize| doc.layers()[k % n].cel();
    match op {
        Op::ConvertAll(m, permiso) => convertir_todo(
            modo_cualquiera(*m),
            if *permiso { todo() } else { Allowed::NOTHING },
        ),
        Op::ConvertMode(k, m) => Command::ConvertMode {
            image: cel(k),
            target: modo_cualquiera(*m),
            allowed: todo(),
        },
        Op::Opacity(k, opacity) => Command::SetLayerOpacity {
            layer: capa(k),
            opacity: *opacity,
        },
        Op::Blend(k, m) => Command::SetLayerBlend {
            layer: capa(k),
            blend: BlendMode::ALL[usize::from(*m % 8)],
        },
        Op::Visible(k, visible) => Command::SetLayerVisible {
            layer: capa(k),
            visible: *visible,
        },
        Op::Paint(k, p) => {
            let paleta = doc.palette().map_or(1, Palette::len);
            let ediciones: Vec<(u32, u32, Pixel)> = p
                .iter()
                .map(|&(x, y, s)| {
                    (
                        u32::from(x) % doc.width(),
                        u32::from(y) % doc.height(),
                        pixel_de(doc.mode(), s, paleta),
                    )
                })
                .collect();
            pone_px(cel(k), &ediciones)
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
        Op::Remove(k) => Command::RemoveLayer { layer: capa(k) },
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
    fn propiedad_2_ida_y_vuelta_de_convert_all(
        plan in plan_estrategia(1..=3),
        ops in prop::collection::vec(op(), 0..=20),
    ) {
        let mut doc = construye(&plan);
        doc.set_history_limit(usize::MAX);
        let inicial = Foto::de(&doc);
        let mut oks = 0usize;

        for orden in &ops {
            let antes = Foto::de(&doc);
            let resultado = doc.apply(traduce(&doc, orden));
            match &resultado {
                Ok(true) => {
                    oks += 1;
                    prop_assert_ne!(doc.state_id(), antes.estado, "{:?}", orden);
                    invariantes(&doc)?;
                }
                _ => antes.exige(&doc, &format!("{orden:?} -> {resultado:?}"))?,
            }
        }

        let final_ = Foto::de(&doc);
        for i in 0..oks {
            prop_assert_eq!(doc.undo(), Ok(()), "deshacer {}", i);
        }
        if let Some(motivo) = inicial.diferencia_estado(&doc) {
            return Err(TestCaseError::fail(format!("tras deshacer todo: {motivo}")));
        }
        prop_assert_eq!(doc.history_bytes(), inicial.historia);
        invariantes(&doc)?;
        for i in 0..oks {
            prop_assert_eq!(doc.redo(), Ok(()), "rehacer {}", i);
        }
        if let Some(motivo) = final_.diferencia_estado(&doc) {
            return Err(TestCaseError::fail(format!("tras rehacer todo: {motivo}")));
        }
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    }
}
