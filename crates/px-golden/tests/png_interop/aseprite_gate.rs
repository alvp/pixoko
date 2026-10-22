//! Puerta del hito 3, parte de `px-golden` (specs/hito-3-puerta.md, unidad 3.9, filas 1 a 4 y
//! propiedad 4): lo que no necesita `Session`, con los 29 `.aseprite` de `golden/aseprite/`.
//!
//! - Fila 1: aplanar y combinar visibles sobre el documento leído dan la misma composición y el
//!   PNG de Aseprite (tolerancias de 3.2, sin ensancharlas).
//! - Fila 2: escribir, leer y escribir otra vez (contenido igual sin ids, bytes estables,
//!   composición igual salvo el RGB de los píxeles con alfa 0, decisión 3).
//! - Fila 3: la estructura del fichero que escribe Pixoko frente a la del que escribió Aseprite,
//!   con un lector de cabecera, chunks, capas y celdas escrito **solo con las tablas de la
//!   especificación pública** (sin descomprimir y sin `px-formats`).
//! - Fila 4: controles negativos (el corpus pierde un caso, un píxel de un PNG cambia, un campo
//!   del fichero escrito cambia): la prueba sabe fallar.
//! - Propiedad 4: la composición sobrevive a guardar y reabrir.
//!
//! **Todo en memoria: este módulo no escribe nada en disco.** Los PNG y `.aseprite` de
//! `golden/aseprite/` solo se leen. No hay código de producción nuevo: las pruebas nacen en verde.

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{BlendMode, Document, Image, Layer, LayerSpec, Mode, Palette, Pixel, Rgba};
use px_formats::{AsepriteLimits, read_aseprite, write_aseprite, write_png};
use px_raster::{MergeError, compose, flatten_command, merge_visible_command};

use super::aseprite_corpus::{CASES, compare_with_list, verify};
use super::aseprite_read::{evaluates_image, golden, read_golden_png, reads_document, rgba_of};
use super::{reads_folder, root_of_the_repo};

/// Los casos de una sola capa: no hay nada que aplanar ni combinar (`NothingToMerge`).
const OF_A_LAYER: [&str; 2] = ["rgba-one-layer", "indexed-palette-256"];

fn limits() -> AsepriteLimits {
    AsepriteLimits::default()
}

fn read_bytes(bytes: &[u8]) -> Result<px_formats::AsepriteRead, String> {
    read_aseprite(bytes, &limits()).map_err(|e| format!("read_aseprite dio error: {e}"))
}

// ---------------------------------------------------------------------------------------
// Ayudas: igualdad sin ids y composición igual salvo el RGB de alfa 0.
// ---------------------------------------------------------------------------------------

/// El valor de un píxel de una capa: `None` si es transparente (celda vacía, alfa 0 en RGBA o
/// índice transparente en indexado).
fn value(doc: &Document, layer_2: &Layer, x: u32, y: u32) -> Option<Pixel> {
    let image_2 = doc.image(layer_2.cel())?;
    let pixel = image_2.get_pixel(x, y).expect("píxel dentro del lienzo");
    let transparent = match pixel {
        Pixel::Rgba(c) => c.a == 0,
        Pixel::Indexed(i) => doc.palette().and_then(Palette::transparent_index) == Some(i),
        Pixel::GrayAlpha { alpha, .. } => alpha == 0,
        _ => false,
    };
    (!transparent).then_some(pixel)
}

/// La igualdad de ida y vuelta de 3.8 (condición 1), sin ids: mismas capas en el mismo orden con
/// nombre, visibilidad, opacidad, mezcla y bloqueo iguales, mismo lienzo, misma paleta e índice
/// transparente y mismo contenido de píxeles por capa (una celda vacía equivale a una
/// transparente; dos píxeles transparentes son iguales aunque su RGB difiera; los demás, byte a
/// byte).
fn same_content(a: &Document, b: &Document) -> Result<(), String> {
    if (a.width(), a.height(), a.mode()) != (b.width(), b.height(), b.mode()) {
        return Err("lienzo distinto".to_string());
    }
    if a.palette() != b.palette() {
        return Err(format!(
            "paleta distinta: {:?} y {:?}",
            a.palette(),
            b.palette()
        ));
    }
    if a.layers().len() != b.layers().len() {
        return Err(format!(
            "{} capas y {} capas",
            a.layers().len(),
            b.layers().len()
        ));
    }
    for (i, (ca, cb)) in a.layers().iter().zip(b.layers()).enumerate() {
        let properties = |c: &Layer| {
            (
                c.name().to_string(),
                c.visible(),
                c.opacity(),
                c.blend(),
                c.locked(),
            )
        };
        if properties(ca) != properties(cb) {
            return Err(format!(
                "capa {i}: propiedades {:?} y {:?}",
                properties(ca),
                properties(cb)
            ));
        }
        for y in 0..a.height() {
            for x in 0..a.width() {
                let (value_a, value_b) = (value(a, ca, x, y), value(b, cb, x, y));
                if value_a != value_b {
                    return Err(format!(
                        "capa {i}, píxel ({x}, {y}): {value_a:?} y {value_b:?}"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Dos composiciones iguales con la exención **exacta** de la decisión 3: el alfa es siempre
/// exacto y un píxel con alfa mayor que 0 que difiera en cualquier canal falla; solo se exime el
/// RGB cuando los dos píxeles tienen alfa 0.
fn equal_except_rgb_with_alpha_0(a: &Image, b: &Image) -> Result<(), String> {
    if (a.width(), a.height(), a.mode()) != (b.width(), b.height(), b.mode()) {
        return Err("composiciones de distinto tamaño o modo".to_string());
    }
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (pa, pb) = (rgba_of(a, x, y), rgba_of(b, x, y));
            if pa.a != pb.a {
                return Err(format!("píxel ({x}, {y}): alfa {} y {}", pa.a, pb.a));
            }
            if pa.a > 0 && pa != pb {
                return Err(format!("píxel ({x}, {y}) con alfa > 0: {pa:?} y {pb:?}"));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Fila 1: aplanar y combinar visibles sobre los 29.
// ---------------------------------------------------------------------------------------

fn row_01(case: &str, flatten: bool) -> Result<Option<&'static str>, String> {
    let original = reads_document(case)?;
    let before = compose(&original).map_err(|e| format!("{case}: compose dio error: {e:?}"))?;
    let mut doc = reads_document(case)?;
    let order = if flatten {
        flatten_command(&doc)
    } else {
        merge_visible_command(&doc)
    };
    let order = match order {
        Ok(order) => order,
        Err(MergeError::NothingToMerge) => return Ok(Some("NothingToMerge")),
        Err(e) => return Err(format!("{case}: la orden dio {e:?}")),
    };
    let hidden = doc.layers().iter().filter(|c| !c.visible()).count();
    if doc.apply(order) != Ok(true) {
        return Err(format!("{case}: la orden no dio Ok(true)"));
    }
    let expected_2 = if flatten { 1 } else { 1 + hidden };
    if doc.layers().len() != expected_2 {
        return Err(format!(
            "{case}: tras la orden hay {} capas y se esperaban {expected_2}",
            doc.layers().len()
        ));
    }
    let after_2 = compose(&doc).map_err(|e| format!("{case}: compose dio error: {e:?}"))?;
    if after_2 != before {
        return Err(format!("{case}: componer tras la orden no da lo de antes"));
    }
    // La composición tras la orden es la del PNG de Aseprite con las tolerancias de 3.2.
    evaluates_image(case, &original, &after_2, &read_golden_png(case))?;
    doc.undo().map_err(|e| format!("{case}: undo dio {e:?}"))?;
    if doc.layers().len() != original.layers().len()
        || compose(&doc).map_err(|e| format!("{e:?}"))? != before
    {
        return Err(format!("{case}: deshacer no devuelve las capas"));
    }
    Ok(None)
}

#[test]
fn row_01_flatten_and_merge_visible_give_the_composition_and_the_png() {
    for flatten in [true, false] {
        let name_2 = if flatten {
            "aplanar"
        } else {
            "combinar visibles"
        };
        let mut without_order = Vec::new();
        let mut failures = Vec::new();
        for case in CASES {
            match row_01(case, flatten) {
                Ok(Some(_)) => without_order.push(case),
                Ok(None) => {}
                Err(e) => failures.push(e),
            }
        }
        assert!(failures.is_empty(), "{name_2}:\n{}", failures.join("\n"));
        assert_eq!(
            without_order, OF_A_LAYER,
            "{name_2}: only the one-layer cases give NothingToMerge"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2: escribir y reabrir los 29.
// ---------------------------------------------------------------------------------------

fn row_02(case: &str) -> Result<(), String> {
    let read = read_bytes(&golden(&format!("{case}.aseprite")))?;
    let original = read.document;
    let bytes = write_aseprite(&original).map_err(|e| format!("{case}: write dio error: {e}"))?;
    let reopened = read_bytes(&bytes).map_err(|e| format!("{case}: {e}"))?;
    if reopened.report.lossy || reopened.report.frames_in_file != 1 {
        return Err(format!(
            "{case}: lo escrito se relee con lossy {} y {} fotogramas",
            reopened.report.lossy, reopened.report.frames_in_file
        ));
    }
    same_content(&original, &reopened.document).map_err(|e| format!("{case}: {e}"))?;
    let other = write_aseprite(&reopened.document).map_err(|e| format!("{case}: {e}"))?;
    if other != bytes {
        return Err(format!(
            "{case}: la segunda escritura no da los mismos bytes"
        ));
    }
    let before = compose(&original).map_err(|e| format!("{case}: {e:?}"))?;
    let after_2 = compose(&reopened.document).map_err(|e| format!("{case}: {e:?}"))?;
    equal_except_rgb_with_alpha_0(&before, &after_2).map_err(|e| format!("{case}: {e}"))?;
    evaluates_image(case, &original, &after_2, &read_golden_png(case))
}

#[test]
fn row_02_the_29_are_written_and_reopened_with_the_same_content() {
    let failures: Vec<String> = CASES.iter().filter_map(|c| row_02(c).err()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------------------------------------------------------------------------------------
// Fila 3: estructura del fichero escrito frente a la del que escribió Aseprite.
//
// Tablas de la especificación pública del formato: §Header, §Frames, §Layer Chunk (0x2004) y
// §Cel Chunk (0x2005). Solo se leen campos; nada se descomprime.
// ---------------------------------------------------------------------------------------

fn u16_le(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| format!("el fichero acaba antes del desplazamiento {at}"))
}

fn u32_le(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| format!("el fichero acaba antes del desplazamiento {at}"))
}

#[derive(Debug)]
struct LayerF {
    /// Desplazamiento del primer byte de los datos del chunk.
    at: usize,
    flags: u16,
    kind: u16,
    level: u16,
    blend: u16,
    opacity_2: u8,
    name_2: String,
}

#[derive(Debug)]
struct CelF {
    at: usize,
    layer_2: u16,
    opacity_2: u8,
    kind: u16,
    z: i16,
}

#[derive(Debug)]
struct Structure {
    width: u16,
    height: u16,
    depth: u16,
    flags: u32,
    frames: u16,
    speed: u16,
    transparent: u8,
    colors: u16,
    pixel: (u8, u8),
    grid: (u16, u16, u16, u16),
    chunks: Vec<u16>,
    layers: Vec<LayerF>,
    cels: Vec<CelF>,
}

fn structure(b: &[u8]) -> Result<Structure, String> {
    if b.len() < 144 {
        return Err("magia: el fichero no llega a la cabecera y el primer fotograma".to_string());
    }
    if u32_le(b, 0)? as usize != b.len() {
        return Err("tamaño: la cabecera no dice la longitud del fichero".to_string());
    }
    if u16_le(b, 4)? != 0xA5E0 {
        return Err("magia: no es 0xA5E0".to_string());
    }
    if u16_le(b, 128 + 4)? != 0xF1FA {
        return Err("magia del fotograma: no es 0xF1FA".to_string());
    }
    let new = u32_le(b, 128 + 12)?;
    let count = if new == 0 {
        u32::from(u16_le(b, 128 + 6)?)
    } else {
        new
    };
    let mut e = Structure {
        width: u16_le(b, 8)?,
        height: u16_le(b, 10)?,
        depth: u16_le(b, 12)?,
        flags: u32_le(b, 14)?,
        frames: u16_le(b, 6)?,
        speed: u16_le(b, 18)?,
        transparent: b[28],
        colors: u16_le(b, 32)?,
        pixel: (b[34], b[35]),
        grid: (
            u16_le(b, 36)?,
            u16_le(b, 38)?,
            u16_le(b, 40)?,
            u16_le(b, 42)?,
        ),
        chunks: Vec::new(),
        layers: Vec::new(),
        cels: Vec::new(),
    };
    let mut pos = 128 + 16;
    for _ in 0..count {
        let size = u32_le(b, pos)? as usize;
        let kind = u16_le(b, pos + 4)?;
        if size < 6 || pos + size > b.len() {
            return Err(format!("chunk en {pos}: tamaño {size} fuera del fichero"));
        }
        let data = pos + 6;
        e.chunks.push(kind);
        match kind {
            // §Layer Chunk: banderas 0, tipo 2, nivel hijo 4, ancho y alto 6 y 8, mezcla 10,
            // opacidad 12, tres bytes, nombre (WORD de longitud) en 16.
            0x2004 => {
                let length = usize::from(u16_le(b, data + 16)?);
                let name_2 = b
                    .get(data + 18..data + 18 + length)
                    .ok_or("nombre de capa fuera del fichero")?;
                e.layers.push(LayerF {
                    at: data,
                    flags: u16_le(b, data)?,
                    kind: u16_le(b, data + 2)?,
                    level: u16_le(b, data + 4)?,
                    blend: u16_le(b, data + 10)?,
                    opacity_2: *b.get(data + 12).ok_or("capa truncada")?,
                    name_2: String::from_utf8_lossy(name_2).into_owned(),
                });
            }
            // §Cel Chunk: capa 0, x 2, y 4, opacidad 6, tipo 7, z-index 9.
            0x2005 => e.cels.push(CelF {
                at: data,
                layer_2: u16_le(b, data)?,
                opacity_2: *b.get(data + 6).ok_or("celda truncada")?,
                kind: u16_le(b, data + 7)?,
                z: u16_le(b, data + 9)? as i16,
            }),
            _ => {}
        }
        pos += size;
    }
    Ok(e)
}

/// Cuántas celdas del original **no** escribe Pixoko (3.8, decisión 9: una celda del todo fuera
/// del lienzo o toda transparente no escribe chunk). Lista cerrada, por caso.
fn cels_that_are_not_written(case: &str) -> usize {
    match case {
        "cel-clipped" => 2,
        "cel-empty" => 1,
        _ => 0,
    }
}

/// Compara la estructura escrita por Pixoko con la que escribió Aseprite. Diferencias permitidas
/// (lista cerrada, declarada en 3.8 y medida en la spec): sin chunk de paleta antigua `0x0004`
/// (la paleta va en `0x2019` solo en indexado), número de colores 0 en RGBA y las celdas de
/// `cels_that_are_not_written`. Cualquier otra diferencia es `Err` con el campo.
fn compares(case: &str, aseprite: &Structure, pixoko: &Structure) -> Result<(), String> {
    macro_rules! equal {
        ($field:expr, $a:expr, $p:expr) => {
            if $a != $p {
                return Err(format!(
                    "{case}: {}: Aseprite {:?} y Pixoko {:?}",
                    $field, $a, $p
                ));
            }
        };
    }
    equal!("cabecera: ancho", aseprite.width, pixoko.width);
    equal!("cabecera: alto", aseprite.height, pixoko.height);
    equal!("cabecera: profundidad", aseprite.depth, pixoko.depth);
    equal!("cabecera: banderas", aseprite.flags, pixoko.flags);
    equal!("cabecera: fotogramas", aseprite.frames, pixoko.frames);
    equal!("cabecera: velocidad", aseprite.speed, pixoko.speed);
    equal!(
        "cabecera: índice transparente",
        aseprite.transparent,
        pixoko.transparent
    );
    equal!("cabecera: razón de píxel", aseprite.pixel, pixoko.pixel);
    equal!("cabecera: cuadrícula", aseprite.grid, pixoko.grid);
    if aseprite.depth == 32 {
        equal!("cabecera: número de colores (RGBA)", 0u16, pixoko.colors);
    } else {
        equal!(
            "cabecera: número de colores (indexado)",
            aseprite.colors,
            pixoko.colors
        );
    }
    // Orden de chunks de lo escrito: perfil, paleta solo si es indexado, capas y celdas.
    let mut expected = vec![0x2007u16];
    if aseprite.depth == 8 {
        expected.push(0x2019);
    }
    expected.extend(std::iter::repeat_n(0x2004, pixoko.layers.len()));
    expected.extend(std::iter::repeat_n(0x2005, pixoko.cels.len()));
    equal!("orden de chunks", expected, pixoko.chunks);
    let of_the_original: Vec<u16> = aseprite
        .chunks
        .iter()
        .copied()
        .filter(|&t| t != 0x0004)
        .collect();
    let of_the_original_without_cels: Vec<u16> = of_the_original
        .iter()
        .copied()
        .filter(|&t| t != 0x2005 && t != 0x2019)
        .collect();
    let of_the_written_without_cels: Vec<u16> = pixoko
        .chunks
        .iter()
        .copied()
        .filter(|&t| t != 0x2005 && t != 0x2019)
        .collect();
    equal!(
        "chunks de perfil y capas",
        of_the_original_without_cels,
        of_the_written_without_cels
    );
    equal!(
        "número de capas",
        aseprite.layers.len(),
        pixoko.layers.len()
    );
    for (i, (a, p)) in aseprite.layers.iter().zip(&pixoko.layers).enumerate() {
        equal!(format!("capa {i}: banderas"), a.flags, p.flags);
        equal!(format!("capa {i}: tipo"), a.kind, p.kind);
        equal!(format!("capa {i}: nivel hijo"), a.level, p.level);
        equal!(format!("capa {i}: mezcla"), a.blend, p.blend);
        equal!(format!("capa {i}: opacidad"), a.opacity_2, p.opacity_2);
        equal!(format!("capa {i}: nombre"), a.name_2, p.name_2);
    }
    let missing = aseprite
        .cels
        .iter()
        .filter(|c| !pixoko.cels.iter().any(|p| p.layer_2 == c.layer_2))
        .count();
    equal!(
        "celdas del original que no se escriben",
        cels_that_are_not_written(case),
        missing
    );
    for (i, p) in pixoko.cels.iter().enumerate() {
        let Some(a) = aseprite.cels.iter().find(|c| c.layer_2 == p.layer_2) else {
            return Err(format!(
                "{case}: celda {i}: Pixoko escribe una celda de la capa {} que Aseprite no tiene",
                p.layer_2
            ));
        };
        equal!(format!("celda {i}: opacidad"), a.opacity_2, p.opacity_2);
        equal!(format!("celda {i}: tipo"), a.kind, p.kind);
        equal!(format!("celda {i}: z-index"), a.z, p.z);
    }
    Ok(())
}

fn row_03(case: &str) -> Result<(), String> {
    let original = golden(&format!("{case}.aseprite"));
    let document = read_bytes(&original)
        .map_err(|e| format!("{case}: {e}"))?
        .document;
    let written = write_aseprite(&document).map_err(|e| format!("{case}: {e}"))?;
    compares(case, &structure(&original)?, &structure(&written)?)
}

#[test]
fn row_03_the_written_structure_is_aseprites_except_the_closed_list() {
    let failures: Vec<String> = CASES.iter().filter_map(|c| row_03(c).err()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------------------------------------------------------------------------------------
// Fila 4: controles negativos.
// ---------------------------------------------------------------------------------------

#[test]
fn row_04_a_the_corpus_missing_a_case_or_with_an_extra_one_fails() {
    let folder = root_of_the_repo().join("golden/aseprite");
    let files = reads_folder(&folder);
    // La carpeta real está bien.
    let cases = verify(&files).expect("golden/aseprite es válida");
    compare_with_list(&cases, &CASES).expect("los 29 casos de la lista");
    // Sin un caso (sus tres ficheros): el verificador no se queja (queda coherente), la lista sí.
    for remove in [CASES[0], "mixed-rgba", "layer-locked"] {
        let without: Vec<_> = files
            .iter()
            .filter(|f| !f.name_2.starts_with(&format!("{remove}.")))
            .cloned()
            .collect();
        let cases = verify(&without).expect("sin un caso queda coherente");
        let error = compare_with_list(&cases, &CASES).expect_err("falta un caso");
        assert!(error.contains(remove), "{error}");
    }
    // Con uno de más: tres ficheros nuevos (copias con otro nombre): 30 casos.
    let mut with = files.clone();
    for suffix in [".aseprite", ".png", ".png.sha256"] {
        let source_2 = files
            .iter()
            .find(|f| f.name_2 == format!("mixed-rgba{suffix}"))
            .expect("fichero de origen");
        let mut copy = source_2.clone();
        copy.name_2 = format!("caso-extra{suffix}");
        with.push(copy);
    }
    let error = verify(&with).expect_err("30 casos pasan del máximo");
    assert!(error.contains("hay 30 casos"), "{error}");
}

/// Un PNG con un canal de un píxel con alfa > 0 cambiado.
fn with_one_pixel_changed(png: &Image) -> (Image, (u32, u32)) {
    for y in 0..png.height() {
        for x in 0..png.width() {
            let c = rgba_of(png, x, y);
            if c.a > 0 {
                let mut other_2 = png.clone();
                other_2
                    .set_pixel(x, y, Pixel::Rgba(Rgba::new(c.r ^ 1, c.g, c.b, c.a)))
                    .expect("píxel dentro");
                return (other_2, (x, y));
            }
        }
    }
    panic!("the PNG has no pixel with alpha > 0");
}

#[test]
fn row_04_b_a_changed_pixel_of_a_png_makes_the_composition_and_the_hash_fail() {
    let case = "mixed-rgba";
    let doc = reads_document(case).expect("documento");
    let composite = compose(&doc).expect("compose");
    let png = read_golden_png(case);
    evaluates_image(case, &doc, &composite, &png).expect("el PNG original pasa");
    let (changed, (x, y)) = with_one_pixel_changed(&png);
    let error = evaluates_image(case, &doc, &composite, &changed).expect_err("un píxel distinto");
    assert!(
        error.contains(case) && error.contains(&format!("({x}, {y})")),
        "{error}"
    );
    // El corpus también lo ve: el `.sha256` es el de la imagen original.
    let mut files = reads_folder(&root_of_the_repo().join("golden/aseprite"));
    let png_new = write_png(&changed).expect("write_png");
    files
        .iter_mut()
        .find(|f| f.name_2 == format!("{case}.png"))
        .expect("el PNG del caso")
        .bytes = png_new;
    let error = verify(&files).expect_err("el hash ya no coincide");
    assert!(
        error.contains(&format!("{case}.png: el hash no coincide")),
        "{error}"
    );
    // Un `.sha256` de otra imagen, lo mismo.
    let mut files = reads_folder(&root_of_the_repo().join("golden/aseprite"));
    let other_2 = files
        .iter()
        .find(|f| f.name_2 == "blend-normal.png.sha256")
        .expect("otro hash")
        .bytes
        .clone();
    files
        .iter_mut()
        .find(|f| f.name_2 == format!("{case}.png.sha256"))
        .expect("su hash")
        .bytes = other_2;
    assert!(verify(&files).is_err());
}

/// Cambia un byte del fichero escrito y exige que la comparación de la fila 3 diga qué campo.
#[test]
fn row_04_c_a_changed_field_of_the_written_file_makes_the_structure_fail() {
    let case = "mixed-rgba";
    let original = golden(&format!("{case}.aseprite"));
    let document = read_bytes(&original).expect("lee").document;
    let written = write_aseprite(&document).expect("write");
    let from_aseprite = structure(&original).expect("estructura del original");
    let e = structure(&written).expect("estructura de lo escrito");
    compares(case, &from_aseprite, &e).expect("sin mutar, coincide");
    assert!(e.layers.len() >= 4 && !e.cels.is_empty());

    // (campo esperado en el mensaje, desplazamiento, máscara que se aplica con XOR)
    let mutations: Vec<(&str, usize, u8)> = vec![
        ("capa 1: banderas", e.layers[1].at, 1),
        ("capa 1: banderas", e.layers[1].at, 2),
        ("capa 0: tipo", e.layers[0].at + 2, 1),
        ("capa 0: nivel hijo", e.layers[0].at + 4, 1),
        ("capa 2: mezcla", e.layers[2].at + 10, 1),
        ("capa 2: opacidad", e.layers[2].at + 12, 1),
        ("capa 3: nombre", e.layers[3].at + 18, 1),
        ("celda 0: opacidad", e.cels[0].at + 6, 0x80),
        ("celda 0: tipo", e.cels[0].at + 7, 1),
        ("celda 0: z-index", e.cels[0].at + 9, 1),
        ("cabecera: ancho", 8, 1),
        ("cabecera: profundidad", 12, 8),
        ("cabecera: banderas", 14, 1),
        ("cabecera: velocidad", 18, 1),
        ("cabecera: cuadrícula", 40, 1),
        ("magia", 4, 1),
    ];
    for (field, pan, mask) in mutations {
        let mut broken = written.clone();
        broken[pan] ^= mask;
        let error = match structure(&broken) {
            Err(e) => e,
            Ok(other) => compares(case, &from_aseprite, &other).expect_err(field),
        };
        assert!(
            error.contains(field),
            "changing a byte in {pan} should give «{field}» and gives: {error}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: la composición sobrevive a guardar y reabrir.
// ---------------------------------------------------------------------------------------

fn name_of_layer() -> impl Strategy<Value = String> {
    "[a-zñ0-9]{1,12}"
}

/// Píxel RGBA con alfa de un conjunto que incluye los extremos y los translúcidos.
fn pixel_rgba() -> impl Strategy<Value = [u8; 4]> {
    (
        any::<[u8; 3]>(),
        prop_oneof![
            3 => Just(0u8),
            2 => Just(255u8),
            3 => prop::sample::select(vec![1u8, 40, 77, 128, 150, 200]),
        ],
    )
        .prop_map(|(c, a)| [c[0], c[1], c[2], a])
}

fn cel_rgba(n: usize) -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        5 => vec(pixel_rgba(), n).prop_map(|px| Some(px.concat())),
    ]
}

fn layer_2(
    name_2: &str,
    visible: bool,
    opacity: u8,
    blend: BlendMode,
    locked: bool,
    image: Option<Image>,
) -> LayerSpec {
    LayerSpec {
        name: name_2.to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    }
}

fn document_rgba(width: u32, height: u32) -> BoxedStrategy<Document> {
    let n = (width * height) as usize;
    let a = (
        name_of_layer(),
        any::<bool>(),
        prop_oneof![1 => Just(255u8), 1 => any::<u8>()],
        0usize..8,
        any::<bool>(),
        cel_rgba(n),
    );
    vec(a, 1..=6)
        .prop_map(move |layers| {
            let specs = layers
                .into_iter()
                .map(|(name_2, visible, opacity_2, mode, locked, cel_2)| {
                    let image_2 = cel_2
                        .map(|b| Image::from_bytes(width, height, Mode::Rgba, b).expect("imagen"));
                    layer_2(
                        &name_2,
                        visible,
                        opacity_2,
                        BlendMode::ALL[mode],
                        locked,
                        image_2,
                    )
                })
                .collect();
            Document::from_layers(width, height, Mode::Rgba, None, specs).expect("documento RGBA")
        })
        .boxed()
}

fn document_indexed(width: u32, height: u32) -> BoxedStrategy<Document> {
    let n = (width * height) as usize;
    (vec(any::<[u8; 4]>(), 2..=16), any::<u8>())
        .prop_flat_map(move |(colors, t)| {
            let length = colors.len();
            let transparent = (usize::from(t) % length) as u8;
            let cel_2 = prop_oneof![1 => Just(None), 5 => vec(any::<u8>(), n).prop_map(Some)];
            let a = (name_of_layer(), any::<bool>(), any::<bool>(), cel_2);
            vec(a, 1..=6).prop_map(move |layers| {
                let mut palette =
                    Palette::from_colors(colors.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
                        .expect("paleta");
                palette
                    .set_transparent_index(Some(transparent))
                    .expect("índice transparente");
                let specs = layers
                    .into_iter()
                    .map(|(name_2, visible, locked, cel_2)| {
                        let image_2 = cel_2.map(|b| {
                            let indices = b
                                .iter()
                                .map(|&v| {
                                    if v % 3 == 0 {
                                        transparent
                                    } else {
                                        (usize::from(v) % length) as u8
                                    }
                                })
                                .collect();
                            Image::from_bytes_indexed(width, height, palette.clone(), indices)
                                .expect("imagen indexada")
                        });
                        layer_2(&name_2, visible, 255, BlendMode::Normal, locked, image_2)
                    })
                    .collect();
                Document::from_layers(width, height, Mode::Indexed, Some(palette.clone()), specs)
                    .expect("documento indexado")
            })
        })
        .boxed()
}

fn document_any() -> impl Strategy<Value = Document> {
    (1u32..=12, 1u32..=12, any::<bool>()).prop_flat_map(|(width, height, indexed)| {
        if indexed {
            document_indexed(width, height)
        } else {
            document_rgba(width, height)
        }
    })
}

fn fail<T, E: std::fmt::Display>(r: Result<T, E>) -> Result<T, TestCaseError> {
    r.map_err(|e| TestCaseError::fail(e.to_string()))
}

fn reopens(doc: &Document) -> Result<Document, TestCaseError> {
    let bytes = fail(write_aseprite(doc))?;
    Ok(fail(read_bytes(&bytes))?.document)
}

/// La composición (o el error) tras aplicar `order` a `doc`; `None` si no hay nada que hacer.
fn after(doc: &Document, flatten: bool) -> Result<Option<Image>, TestCaseError> {
    let mut doc = doc.clone();
    let order = if flatten {
        flatten_command(&doc)
    } else {
        merge_visible_command(&doc)
    };
    match order {
        Err(MergeError::NothingToMerge) => Ok(None),
        Err(e) => Err(TestCaseError::fail(format!("la orden dio {e:?}"))),
        Ok(order) => {
            fail(doc.apply(order))?;
            Ok(Some(fail(compose(&doc).map_err(|e| format!("{e:?}")))?))
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// P4. Guardar y reabrir no cambia la composición salvo el RGB de los píxeles con alfa 0, y
    /// aplanar y combinar visibles conmutan con guardar y reabrir con la misma exención.
    #[test]
    fn property_4_the_composition_survives_saving_and_reopening(doc in document_any()) {
        let reopened = reopens(&doc)?;
        fail(same_content(&doc, &reopened))?;
        let before = fail(compose(&doc).map_err(|e| format!("{e:?}")))?;
        let after_2 = fail(compose(&reopened).map_err(|e| format!("{e:?}")))?;
        fail(equal_except_rgb_with_alpha_0(&before, &after_2))?;
        for flatten in [true, false] {
            let a = after(&doc, flatten)?;
            let b = after(&reopened, flatten)?;
            match (a, b) {
                (None, None) => {}
                (Some(a), Some(b)) => fail(equal_except_rgb_with_alpha_0(&a, &b))?,
                (a, b) => prop_assert!(false, "one command gives {} and the other {}", a.is_some(), b.is_some()),
            }
            // Escribir lo aplanado y reabrirlo es lo mismo que aplanar lo reabierto.
            let mut plane = doc.clone();
            let order = if flatten { flatten_command(&plane) } else { merge_visible_command(&plane) };
            if let Ok(order) = order {
                fail(plane.apply(order))?;
                let plane_reopened = reopens(&plane)?;
                let (x, y) = (
                    fail(compose(&plane).map_err(|e| format!("{e:?}")))?,
                    fail(compose(&plane_reopened).map_err(|e| format!("{e:?}")))?,
                );
                fail(equal_except_rgb_with_alpha_0(&x, &y))?;
            }
        }
    }
}
