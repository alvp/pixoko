//! Lectura de los `.aseprite` de `golden/aseprite/` (specs/hito-3-aseprite.md, unidad 3.8, filas 3
//! y 8 de «Casos»; specs/corpus-hito-3.md para los casos).
//!
//! - Fila 3: los 29 casos se leen con `px_formats::read_aseprite` y se componen con
//!   `px_raster::compose` contra su PNG aplanado (el oráculo). **Tolerancia 0** fuera del dominio
//!   de la fórmula empírica de 3.2 (fondo ya compuesto translúcido, `αs > 0` y modo distinto de
//!   Normal); el RGB bajo alfa 0 no se exige cuando los dos alfa son 0. Dentro del dominio, alfa
//!   exacto y error de canal como mucho `TOLERANCE_EMPIRICAL`, con un suelo de 1 por caso agregado
//!   (una tira por modo en `blend-eight-background-translucent`, un solo grupo en los demás). Los
//!   indexados se comparan por su paleta resuelta. **No se ensancha ninguna tolerancia**: es la
//!   misma versión de `evaluates_image` y `in_domain` de `blend.rs`, sobre un `Document` leído.
//! - Fila 8: la celda del modelo es el recorte exacto de la celda del fichero sobre el relleno;
//!   las celdas del todo fuera quedan vacías.
//!
//! Este módulo es la fase de pruebas en rojo: `read_aseprite` no existe todavía.

use std::fs;
use std::path::Path;

use px_core::{BlendMode, Document, Image, Layer, Mode, Pixel, Rgba};
use px_formats::{AsepriteLimits, read_aseprite, read_png};
use px_raster::{compose, composite_pixel};

use super::aseprite_corpus::CASES;

/// Techo del error, en niveles de canal, de la fórmula empírica de 3.2 dentro de su dominio. Es el
/// mismo de `blend.rs`: no se ensancha.
pub(super) const TOLERANCE_EMPIRICAL: i32 = 2;

/// Un fichero de `golden/aseprite/` (datos, no código).
pub(super) fn golden(name_2: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(name_2);
    fs::read(&path).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
}

/// El documento de un caso, leído con los límites por defecto.
pub(super) fn reads_document(case: &str) -> Result<Document, String> {
    let bytes = golden(&format!("{case}.aseprite"));
    read_aseprite(&bytes, &AsepriteLimits::default())
        .map(|r| r.document)
        .map_err(|e| format!("{case}: read_aseprite dio error: {e}"))
}

/// El PNG aplanado de un caso, tal como lo lee `read_png`.
pub(super) fn lee_png(case: &str) -> Image {
    read_png(&golden(&format!("{case}.png")))
        .unwrap_or_else(|e| panic!("{case}.png: read_png dio error: {e}"))
        .image
}

/// El píxel como RGBA resuelto: los indexados, por su paleta (con el índice transparente en
/// alfa 0).
pub(super) fn rgba_de(image_2: &Image, x: u32, y: u32) -> Rgba {
    match image_2.get_pixel(x, y) {
        Ok(Pixel::Rgba(c)) => c,
        Ok(Pixel::Indexed(i)) => image_2
            .palette()
            .and_then(|p| p.resolved(i).ok())
            .unwrap_or_else(|| panic!("índice {i} sin entrada en la paleta")),
        other_2 => panic!("se esperaba RGBA o indexado, hay {other_2:?}"),
    }
}

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición.
fn rounds_255(x: u32) -> u32 {
    (2 * x + 255) / 510
}

/// El píxel `(x, y)` de la celda de una capa; una celda vacía es transparente.
fn pixel_of_layer(doc: &Document, layer_2: &Layer, x: u32, y: u32) -> Rgba {
    match doc.image(layer_2.cel()) {
        None => Rgba::TRANSPARENT,
        Some(image_2) => rgba_de(image_2, x, y),
    }
}

/// `true` si el píxel `(x, y)` está en el dominio de la fórmula empírica: en alguna capa visible,
/// el fondo ya compuesto debajo tiene `0 < αb < 255`, el origen tiene `αs > 0` (con `αs =
/// div255(a · opacidad)`) y el mode no es Normal. El background se pliega with `composite_pixel` layer_2 a
/// capa. En indexado no hay mezcla ni opacidad: nunca.
fn in_domain(doc: &Document, x: u32, y: u32) -> bool {
    if doc.mode() != Mode::Rgba {
        return false;
    }
    let mut background = Rgba::TRANSPARENT;
    let mut inside = false;
    for layer_2 in doc.layers().iter().filter(|c| c.visible()) {
        let source_2 = pixel_of_layer(doc, layer_2, x, y);
        let alpha_source_2 = rounds_255(u32::from(source_2.a) * u32::from(layer_2.opacity()));
        if background.a > 0
            && background.a < 255
            && alpha_source_2 > 0
            && layer_2.blend() != BlendMode::Normal
        {
            inside = true;
        }
        background = composite_pixel(background, source_2, layer_2.opacity(), layer_2.blend());
    }
    inside
}

/// Evalúa un caso: `Err` con el nombre del caso y el primer píxel que incumple.
fn evaluates_case(case: &str) -> Result<(), String> {
    let doc = reads_document(case)?;
    let composite = compose(&doc).map_err(|e| format!("{case}: compose dio error: {e:?}"))?;
    evaluates_image(case, &doc, &composite, &lee_png(case))
}

/// La comparación de `evaluates_case` sobre una imagen ya compuesta y un PNG dados (la usa también la
/// puerta del hito, specs/hito-3-puerta.md, con la imagen de un documento aplanado o reabierto).
/// `doc` es el documento **original**: de él sale el dominio de la fórmula empírica de 3.2.
pub(super) fn evaluates_image(
    case: &str,
    doc: &Document,
    composite: &Image,
    png: &Image,
) -> Result<(), String> {
    if (composite.width(), composite.height()) != (png.width(), png.height()) {
        return Err(format!(
            "{case}: la composición mide {}×{} y el PNG {}×{}",
            composite.width(),
            composite.height(),
            png.width(),
            png.height()
        ));
    }
    // Un grupo por tira en el caso de las ocho tiras (como en `blend.rs`) y uno solo en los demás.
    let groups = if case == "blend-eight-background-translucent" {
        8
    } else {
        1
    };
    let group = |x: u32| if groups == 8 { (x / 8) as usize } else { 0 };
    let mut max_error = vec![0i32; groups];
    let mut in_the_domain = vec![0usize; groups];
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (our, expected) = (rgba_de(composite, x, y), rgba_de(png, x, y));
            let inside = in_domain(doc, x, y);
            let mut error_pixel = 0;
            let mut reason = None;
            if our.a != expected.a {
                reason = Some("el alfa no es exacto");
            } else if inside {
                let differences = [
                    i32::from(our.r) - i32::from(expected.r),
                    i32::from(our.g) - i32::from(expected.g),
                    i32::from(our.b) - i32::from(expected.b),
                ];
                error_pixel = differences.iter().map(|d| d.abs()).max().unwrap_or(0);
                if error_pixel > TOLERANCE_EMPIRICAL {
                    reason = Some("el error pasa del techo de la fórmula empírica");
                }
            } else if !(our.a == 0 && expected.a == 0) && our != expected {
                reason = Some("fuera del dominio la tolerancia es 0");
            }
            if let Some(reason) = reason {
                return Err(format!(
                    "{case}: píxel ({x}, {y}) {}: {reason}; compose da {our:?} y el PNG {expected:?}",
                    if inside {
                        "del dominio"
                    } else {
                        "fuera del dominio"
                    },
                ));
            }
            if inside {
                let g = group(x);
                in_the_domain[g] += 1;
                max_error[g] = max_error[g].max(error_pixel);
            }
        }
    }
    // El techo y el suelo por grupo: si el error máximo baja a 0, la exención sobra.
    for (g, &maximum) in max_error.iter().enumerate() {
        if in_the_domain[g] > 0 && maximum == 0 {
            return Err(format!(
                "{case} (grupo {g}): el error máximo dentro del dominio es 0: la exención es \
                 innecesaria: usa tolerancia 0 en este caso"
            ));
        }
    }
    Ok(())
}

#[test]
fn row_03_the_29_aseprite_compose_the_same_as_their_png() {
    assert_eq!(CASES.len(), 29);
    let failures: Vec<String> = CASES
        .iter()
        .filter_map(|case| evaluates_case(case).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} casos no componen como su PNG:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------------------
// Fila 8: celdas desplazadas, recortadas y del todo fuera.
// ---------------------------------------------------------------------------------------

/// Una celda esperada: capa, rectángulo del lienzo `[x0, y0, x1, y1)` que cubre la celda del
/// fichero recortada, y cuántos píxeles no transparentes caen dentro (contados al decodificar los
/// datos del fichero con las tablas de la especificación, no con el lector). `None` en el
/// rectángulo: la celda queda del todo fuera y la del modelo, vacía.
type Expected = (usize, Option<[u32; 4]>, usize);

fn requires_cels(case: &str, expected_2: &[Expected]) {
    let doc = reads_document(case).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        doc.layers().len(),
        expected_2.len(),
        "{case}: número de capas"
    );
    for &(i, rect, visible) in expected_2 {
        let layer_2 = &doc.layers()[i];
        let name_2 = format!("{case}, capa {i} ({})", layer_2.name());
        let Some([x0, y0, x1, y1]) = rect else {
            assert!(
                doc.image(layer_2.cel()).is_none(),
                "{name_2}: del todo fuera ⇒ celda vacía"
            );
            continue;
        };
        let image_2 = doc
            .image(layer_2.cel())
            .unwrap_or_else(|| panic!("{name_2}: debía tener celda"));
        assert_eq!(
            (image_2.width(), image_2.height()),
            (doc.width(), doc.height()),
            "{name_2}"
        );
        // El relleno de una celda nueva: RGBA (0, 0, 0, 0) o el índice transparente.
        let fill = match doc.palette() {
            Some(palette) => {
                Pixel::Indexed(palette.transparent_index().expect("índice transparente"))
            }
            None => Pixel::Rgba(Rgba::TRANSPARENT),
        };
        let mut visible_read = 0;
        for y in 0..image_2.height() {
            for x in 0..image_2.width() {
                let pixel = image_2.get_pixel(x, y).expect("píxel dentro");
                let inside = (x0..x1).contains(&x) && (y0..y1).contains(&y);
                if !inside {
                    assert_eq!(pixel, fill, "{name_2}: ({x}, {y}) fuera del recorte");
                } else if pixel != fill && rgba_de(image_2, x, y).a != 0 {
                    visible_read += 1;
                }
            }
        }
        assert_eq!(
            visible_read, visible,
            "{name_2}: píxeles visibles del recorte"
        );
    }
}

#[test]
fn row_08_corpus_cels_are_the_exact_clip_and_the_outside_ones_stay_empty() {
    // `cel-clipped` (32 × 32): una que cubre todo, una a la izquierda y arriba, una por la
    // derecha, una por abajo y dos del todo fuera.
    requires_cels(
        "cel-clipped",
        &[
            (0, Some([0, 0, 32, 32]), 1024),
            (1, Some([0, 0, 8, 8]), 64),
            (2, Some([24, 10, 32, 26]), 128),
            (3, Some([6, 26, 26, 32]), 120),
            (4, None, 0),
            (5, None, 0),
        ],
    );
    // `indexed-cels` (32 × 32, indexado): desplazada y recortada por ambos lados.
    requires_cels(
        "indexed-cels",
        &[
            (0, Some([5, 7, 17, 19]), 90),
            (1, Some([0, 0, 8, 10]), 80),
            (2, Some([27, 25, 32, 32]), 35),
        ],
    );
    // `cel-shifted` (32 × 32): celdas pequeñas, una pegada al borde y una de 1 × 1.
    requires_cels(
        "cel-shifted",
        &[
            (0, Some([3, 5, 13, 13]), 80),
            (1, Some([16, 4, 28, 16]), 81),
            (2, Some([24, 24, 32, 32]), 64),
            (3, Some([31, 31, 32, 32]), 1),
        ],
    );
}
