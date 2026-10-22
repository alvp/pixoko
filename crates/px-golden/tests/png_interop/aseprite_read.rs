//! Lectura de los `.aseprite` de `golden/aseprite/` (specs/hito-3-aseprite.md, unidad 3.8, filas 3
//! y 8 de «Casos»; specs/corpus-hito-3.md para los casos).
//!
//! - Fila 3: los 29 casos se leen con `px_formats::read_aseprite` y se componen con
//!   `px_raster::compose` contra su PNG aplanado (el oráculo). **Tolerancia 0** fuera del dominio
//!   de la fórmula empírica de 3.2 (fondo ya compuesto translúcido, `αs > 0` y modo distinto de
//!   Normal); el RGB bajo alfa 0 no se exige cuando los dos alfa son 0. Dentro del dominio, alfa
//!   exacto y error de canal como mucho `TOLERANCIA_EMPIRICA`, con un suelo de 1 por caso agregado
//!   (una tira por modo en `mezcla-ocho-fondo-translucido`, un solo grupo en los demás). Los
//!   indexados se comparan por su paleta resuelta. **No se ensancha ninguna tolerancia**: es la
//!   misma versión de `evalua_imagen` y `en_dominio` de `mezcla.rs`, sobre un `Document` leído.
//! - Fila 8: la celda del modelo es el recorte exacto de la celda del fichero sobre el relleno;
//!   las celdas del todo fuera quedan vacías.
//!
//! Este módulo es la fase de pruebas en rojo: `read_aseprite` no existe todavía.

use std::fs;
use std::path::Path;

use px_core::{BlendMode, Document, Image, Layer, Mode, Pixel, Rgba};
use px_formats::{AsepriteLimits, read_aseprite, read_png};
use px_raster::{compose, composite_pixel};

use super::aseprite_corpus::CASOS;

/// Techo del error, en niveles de canal, de la fórmula empírica de 3.2 dentro de su dominio. Es el
/// mismo de `mezcla.rs`: no se ensancha.
pub(super) const TOLERANCIA_EMPIRICA: i32 = 2;

/// Un fichero de `golden/aseprite/` (datos, no código).
pub(super) fn golden(nombre: &str) -> Vec<u8> {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(nombre);
    fs::read(&ruta).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()))
}

/// El documento de un caso, leído con los límites por defecto.
pub(super) fn lee_documento(caso: &str) -> Result<Document, String> {
    let bytes = golden(&format!("{caso}.aseprite"));
    read_aseprite(&bytes, &AsepriteLimits::default())
        .map(|r| r.document)
        .map_err(|e| format!("{caso}: read_aseprite dio error: {e}"))
}

/// El PNG aplanado de un caso, tal como lo lee `read_png`.
pub(super) fn lee_png(caso: &str) -> Image {
    read_png(&golden(&format!("{caso}.png")))
        .unwrap_or_else(|e| panic!("{caso}.png: read_png dio error: {e}"))
        .image
}

/// El píxel como RGBA resuelto: los indexados, por su paleta (con el índice transparente en
/// alfa 0).
pub(super) fn rgba_de(imagen: &Image, x: u32, y: u32) -> Rgba {
    match imagen.get_pixel(x, y) {
        Ok(Pixel::Rgba(c)) => c,
        Ok(Pixel::Indexed(i)) => imagen
            .palette()
            .and_then(|p| p.resolved(i).ok())
            .unwrap_or_else(|| panic!("índice {i} sin entrada en la paleta")),
        otro => panic!("se esperaba RGBA o indexado, hay {otro:?}"),
    }
}

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición.
fn redondea_255(x: u32) -> u32 {
    (2 * x + 255) / 510
}

/// El píxel `(x, y)` de la celda de una capa; una celda vacía es transparente.
fn pixel_de_capa(doc: &Document, capa: &Layer, x: u32, y: u32) -> Rgba {
    match doc.image(capa.cel()) {
        None => Rgba::TRANSPARENT,
        Some(imagen) => rgba_de(imagen, x, y),
    }
}

/// `true` si el píxel `(x, y)` está en el dominio de la fórmula empírica: en alguna capa visible,
/// el fondo ya compuesto debajo tiene `0 < αb < 255`, el origen tiene `αs > 0` (con `αs =
/// div255(a · opacidad)`) y el modo no es Normal. El fondo se pliega con `composite_pixel` capa a
/// capa. En indexado no hay mezcla ni opacidad: nunca.
fn en_dominio(doc: &Document, x: u32, y: u32) -> bool {
    if doc.mode() != Mode::Rgba {
        return false;
    }
    let mut fondo = Rgba::TRANSPARENT;
    let mut dentro = false;
    for capa in doc.layers().iter().filter(|c| c.visible()) {
        let origen = pixel_de_capa(doc, capa, x, y);
        let alfa_origen = redondea_255(u32::from(origen.a) * u32::from(capa.opacity()));
        if fondo.a > 0 && fondo.a < 255 && alfa_origen > 0 && capa.blend() != BlendMode::Normal {
            dentro = true;
        }
        fondo = composite_pixel(fondo, origen, capa.opacity(), capa.blend());
    }
    dentro
}

/// Evalúa un caso: `Err` con el nombre del caso y el primer píxel que incumple.
fn evalua_caso(caso: &str) -> Result<(), String> {
    let doc = lee_documento(caso)?;
    let compuesto = compose(&doc).map_err(|e| format!("{caso}: compose dio error: {e:?}"))?;
    evalua_imagen(caso, &doc, &compuesto, &lee_png(caso))
}

/// La comparación de `evalua_caso` sobre una imagen ya compuesta y un PNG dados (la usa también la
/// puerta del hito, specs/hito-3-puerta.md, con la imagen de un documento aplanado o reabierto).
/// `doc` es el documento **original**: de él sale el dominio de la fórmula empírica de 3.2.
pub(super) fn evalua_imagen(
    caso: &str,
    doc: &Document,
    compuesto: &Image,
    png: &Image,
) -> Result<(), String> {
    if (compuesto.width(), compuesto.height()) != (png.width(), png.height()) {
        return Err(format!(
            "{caso}: la composición mide {}×{} y el PNG {}×{}",
            compuesto.width(),
            compuesto.height(),
            png.width(),
            png.height()
        ));
    }
    // Un grupo por tira en el caso de las ocho tiras (como en `mezcla.rs`) y uno solo en los demás.
    let grupos = if caso == "mezcla-ocho-fondo-translucido" {
        8
    } else {
        1
    };
    let grupo = |x: u32| if grupos == 8 { (x / 8) as usize } else { 0 };
    let mut max_error = vec![0i32; grupos];
    let mut en_el_dominio = vec![0usize; grupos];
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (nuestro, esperado) = (rgba_de(compuesto, x, y), rgba_de(png, x, y));
            let dentro = en_dominio(doc, x, y);
            let mut error_pixel = 0;
            let mut motivo = None;
            if nuestro.a != esperado.a {
                motivo = Some("el alfa no es exacto");
            } else if dentro {
                let diferencias = [
                    i32::from(nuestro.r) - i32::from(esperado.r),
                    i32::from(nuestro.g) - i32::from(esperado.g),
                    i32::from(nuestro.b) - i32::from(esperado.b),
                ];
                error_pixel = diferencias.iter().map(|d| d.abs()).max().unwrap_or(0);
                if error_pixel > TOLERANCIA_EMPIRICA {
                    motivo = Some("el error pasa del techo de la fórmula empírica");
                }
            } else if !(nuestro.a == 0 && esperado.a == 0) && nuestro != esperado {
                motivo = Some("fuera del dominio la tolerancia es 0");
            }
            if let Some(motivo) = motivo {
                return Err(format!(
                    "{caso}: píxel ({x}, {y}) {}: {motivo}; compose da {nuestro:?} y el PNG {esperado:?}",
                    if dentro {
                        "del dominio"
                    } else {
                        "fuera del dominio"
                    },
                ));
            }
            if dentro {
                let g = grupo(x);
                en_el_dominio[g] += 1;
                max_error[g] = max_error[g].max(error_pixel);
            }
        }
    }
    // El techo y el suelo por grupo: si el error máximo baja a 0, la exención sobra.
    for (g, &maximo) in max_error.iter().enumerate() {
        if en_el_dominio[g] > 0 && maximo == 0 {
            return Err(format!(
                "{caso} (grupo {g}): el error máximo dentro del dominio es 0: la exención es \
                 innecesaria: usa tolerancia 0 en este caso"
            ));
        }
    }
    Ok(())
}

#[test]
fn fila_03_los_29_aseprite_componen_igual_que_su_png() {
    assert_eq!(CASOS.len(), 29);
    let fallos: Vec<String> = CASOS
        .iter()
        .filter_map(|caso| evalua_caso(caso).err())
        .collect();
    assert!(
        fallos.is_empty(),
        "{} casos no componen como su PNG:\n{}",
        fallos.len(),
        fallos.join("\n")
    );
}

// ---------------------------------------------------------------------------------------
// Fila 8: celdas desplazadas, recortadas y del todo fuera.
// ---------------------------------------------------------------------------------------

/// Una celda esperada: capa, rectángulo del lienzo `[x0, y0, x1, y1)` que cubre la celda del
/// fichero recortada, y cuántos píxeles no transparentes caen dentro (contados al decodificar los
/// datos del fichero con las tablas de la especificación, no con el lector). `None` en el
/// rectángulo: la celda queda del todo fuera y la del modelo, vacía.
type Esperada = (usize, Option<[u32; 4]>, usize);

fn exige_celdas(caso: &str, esperadas: &[Esperada]) {
    let doc = lee_documento(caso).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        doc.layers().len(),
        esperadas.len(),
        "{caso}: número de capas"
    );
    for &(i, rect, visibles) in esperadas {
        let capa = &doc.layers()[i];
        let nombre = format!("{caso}, capa {i} ({})", capa.name());
        let Some([x0, y0, x1, y1]) = rect else {
            assert!(
                doc.image(capa.cel()).is_none(),
                "{nombre}: del todo fuera ⇒ celda vacía"
            );
            continue;
        };
        let imagen = doc
            .image(capa.cel())
            .unwrap_or_else(|| panic!("{nombre}: debía tener celda"));
        assert_eq!(
            (imagen.width(), imagen.height()),
            (doc.width(), doc.height()),
            "{nombre}"
        );
        // El relleno de una celda nueva: RGBA (0, 0, 0, 0) o el índice transparente.
        let relleno = match doc.palette() {
            Some(paleta) => {
                Pixel::Indexed(paleta.transparent_index().expect("índice transparente"))
            }
            None => Pixel::Rgba(Rgba::TRANSPARENT),
        };
        let mut visibles_leidos = 0;
        for y in 0..imagen.height() {
            for x in 0..imagen.width() {
                let pixel = imagen.get_pixel(x, y).expect("píxel dentro");
                let dentro = (x0..x1).contains(&x) && (y0..y1).contains(&y);
                if !dentro {
                    assert_eq!(pixel, relleno, "{nombre}: ({x}, {y}) fuera del recorte");
                } else if pixel != relleno && rgba_de(imagen, x, y).a != 0 {
                    visibles_leidos += 1;
                }
            }
        }
        assert_eq!(
            visibles_leidos, visibles,
            "{nombre}: píxeles visibles del recorte"
        );
    }
}

#[test]
fn fila_08_celdas_del_corpus_son_el_recorte_exacto_y_las_de_fuera_quedan_vacias() {
    // `celda-recortada` (32 × 32): una que cubre todo, una a la izquierda y arriba, una por la
    // derecha, una por abajo y dos del todo fuera.
    exige_celdas(
        "celda-recortada",
        &[
            (0, Some([0, 0, 32, 32]), 1024),
            (1, Some([0, 0, 8, 8]), 64),
            (2, Some([24, 10, 32, 26]), 128),
            (3, Some([6, 26, 26, 32]), 120),
            (4, None, 0),
            (5, None, 0),
        ],
    );
    // `indexado-celdas` (32 × 32, indexado): desplazada y recortada por ambos lados.
    exige_celdas(
        "indexado-celdas",
        &[
            (0, Some([5, 7, 17, 19]), 90),
            (1, Some([0, 0, 8, 10]), 80),
            (2, Some([27, 25, 32, 32]), 35),
        ],
    );
    // `celda-desplazada` (32 × 32): celdas pequeñas, una pegada al borde y una de 1 × 1.
    exige_celdas(
        "celda-desplazada",
        &[
            (0, Some([3, 5, 13, 13]), 80),
            (1, Some([16, 4, 28, 16]), 81),
            (2, Some([24, 24, 32, 32]), 64),
            (3, Some([31, 31, 32, 32]), 1),
        ],
    );
}
