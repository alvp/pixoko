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

use super::aseprite_corpus::{CASOS, comparar_con_lista, verificar};
use super::aseprite_read::{evalua_imagen, golden, lee_documento, lee_png, rgba_de};
use super::{lee_carpeta, raiz_del_repo};

/// Los casos de una sola capa: no hay nada que aplanar ni combinar (`NothingToMerge`).
const DE_UNA_CAPA: [&str; 2] = ["rgba-una-capa", "indexado-paleta-256"];

fn limites() -> AsepriteLimits {
    AsepriteLimits::default()
}

fn lee(bytes: &[u8]) -> Result<px_formats::AsepriteRead, String> {
    read_aseprite(bytes, &limites()).map_err(|e| format!("read_aseprite dio error: {e}"))
}

// ---------------------------------------------------------------------------------------
// Ayudas: igualdad sin ids y composición igual salvo el RGB de alfa 0.
// ---------------------------------------------------------------------------------------

/// El valor de un píxel de una capa: `None` si es transparente (celda vacía, alfa 0 en RGBA o
/// índice transparente en indexado).
fn valor(doc: &Document, capa: &Layer, x: u32, y: u32) -> Option<Pixel> {
    let imagen = doc.image(capa.cel())?;
    let pixel = imagen.get_pixel(x, y).expect("píxel dentro del lienzo");
    let transparente = match pixel {
        Pixel::Rgba(c) => c.a == 0,
        Pixel::Indexed(i) => doc.palette().and_then(Palette::transparent_index) == Some(i),
        Pixel::GrayAlpha { alpha, .. } => alpha == 0,
        _ => false,
    };
    (!transparente).then_some(pixel)
}

/// La igualdad de ida y vuelta de 3.8 (condición 1), sin ids: mismas capas en el mismo orden con
/// nombre, visibilidad, opacidad, mezcla y bloqueo iguales, mismo lienzo, misma paleta e índice
/// transparente y mismo contenido de píxeles por capa (una celda vacía equivale a una
/// transparente; dos píxeles transparentes son iguales aunque su RGB difiera; los demás, byte a
/// byte).
fn mismo_contenido(a: &Document, b: &Document) -> Result<(), String> {
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
        let propiedades = |c: &Layer| {
            (
                c.name().to_string(),
                c.visible(),
                c.opacity(),
                c.blend(),
                c.locked(),
            )
        };
        if propiedades(ca) != propiedades(cb) {
            return Err(format!(
                "capa {i}: propiedades {:?} y {:?}",
                propiedades(ca),
                propiedades(cb)
            ));
        }
        for y in 0..a.height() {
            for x in 0..a.width() {
                let (va, vb) = (valor(a, ca, x, y), valor(b, cb, x, y));
                if va != vb {
                    return Err(format!("capa {i}, píxel ({x}, {y}): {va:?} y {vb:?}"));
                }
            }
        }
    }
    Ok(())
}

/// Dos composiciones iguales con la exención **exacta** de la decisión 3: el alfa es siempre
/// exacto y un píxel con alfa mayor que 0 que difiera en cualquier canal falla; solo se exime el
/// RGB cuando los dos píxeles tienen alfa 0.
fn igual_salvo_rgb_con_alfa_0(a: &Image, b: &Image) -> Result<(), String> {
    if (a.width(), a.height(), a.mode()) != (b.width(), b.height(), b.mode()) {
        return Err("composiciones de distinto tamaño o modo".to_string());
    }
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (pa, pb) = (rgba_de(a, x, y), rgba_de(b, x, y));
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

fn fila_01(caso: &str, aplanar: bool) -> Result<Option<&'static str>, String> {
    let original = lee_documento(caso)?;
    let antes = compose(&original).map_err(|e| format!("{caso}: compose dio error: {e:?}"))?;
    let mut doc = lee_documento(caso)?;
    let orden = if aplanar {
        flatten_command(&doc)
    } else {
        merge_visible_command(&doc)
    };
    let orden = match orden {
        Ok(orden) => orden,
        Err(MergeError::NothingToMerge) => return Ok(Some("NothingToMerge")),
        Err(e) => return Err(format!("{caso}: la orden dio {e:?}")),
    };
    let ocultas = doc.layers().iter().filter(|c| !c.visible()).count();
    if doc.apply(orden) != Ok(true) {
        return Err(format!("{caso}: la orden no dio Ok(true)"));
    }
    let esperadas = if aplanar { 1 } else { 1 + ocultas };
    if doc.layers().len() != esperadas {
        return Err(format!(
            "{caso}: tras la orden hay {} capas y se esperaban {esperadas}",
            doc.layers().len()
        ));
    }
    let despues = compose(&doc).map_err(|e| format!("{caso}: compose dio error: {e:?}"))?;
    if despues != antes {
        return Err(format!("{caso}: componer tras la orden no da lo de antes"));
    }
    // La composición tras la orden es la del PNG de Aseprite con las tolerancias de 3.2.
    evalua_imagen(caso, &original, &despues, &lee_png(caso))?;
    doc.undo().map_err(|e| format!("{caso}: undo dio {e:?}"))?;
    if doc.layers().len() != original.layers().len()
        || compose(&doc).map_err(|e| format!("{e:?}"))? != antes
    {
        return Err(format!("{caso}: deshacer no devuelve las capas"));
    }
    Ok(None)
}

#[test]
fn fila_01_aplanar_y_combinar_visibles_dan_la_composicion_y_el_png() {
    for aplanar in [true, false] {
        let nombre = if aplanar {
            "aplanar"
        } else {
            "combinar visibles"
        };
        let mut sin_orden = Vec::new();
        let mut fallos = Vec::new();
        for caso in CASOS {
            match fila_01(caso, aplanar) {
                Ok(Some(_)) => sin_orden.push(caso),
                Ok(None) => {}
                Err(e) => fallos.push(e),
            }
        }
        assert!(fallos.is_empty(), "{nombre}:\n{}", fallos.join("\n"));
        assert_eq!(
            sin_orden, DE_UNA_CAPA,
            "{nombre}: solo los casos de una capa dan NothingToMerge"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2: escribir y reabrir los 29.
// ---------------------------------------------------------------------------------------

fn fila_02(caso: &str) -> Result<(), String> {
    let leido = lee(&golden(&format!("{caso}.aseprite")))?;
    let original = leido.document;
    let bytes = write_aseprite(&original).map_err(|e| format!("{caso}: write dio error: {e}"))?;
    let reabierto = lee(&bytes).map_err(|e| format!("{caso}: {e}"))?;
    if reabierto.report.lossy || reabierto.report.frames_in_file != 1 {
        return Err(format!(
            "{caso}: lo escrito se relee con lossy {} y {} fotogramas",
            reabierto.report.lossy, reabierto.report.frames_in_file
        ));
    }
    mismo_contenido(&original, &reabierto.document).map_err(|e| format!("{caso}: {e}"))?;
    let otra = write_aseprite(&reabierto.document).map_err(|e| format!("{caso}: {e}"))?;
    if otra != bytes {
        return Err(format!(
            "{caso}: la segunda escritura no da los mismos bytes"
        ));
    }
    let antes = compose(&original).map_err(|e| format!("{caso}: {e:?}"))?;
    let despues = compose(&reabierto.document).map_err(|e| format!("{caso}: {e:?}"))?;
    igual_salvo_rgb_con_alfa_0(&antes, &despues).map_err(|e| format!("{caso}: {e}"))?;
    evalua_imagen(caso, &original, &despues, &lee_png(caso))
}

#[test]
fn fila_02_los_29_se_escriben_y_se_reabren_con_el_mismo_contenido() {
    let fallos: Vec<String> = CASOS.iter().filter_map(|c| fila_02(c).err()).collect();
    assert!(fallos.is_empty(), "{}", fallos.join("\n"));
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
struct CapaF {
    /// Desplazamiento del primer byte de los datos del chunk.
    at: usize,
    banderas: u16,
    tipo: u16,
    nivel: u16,
    mezcla: u16,
    opacidad: u8,
    nombre: String,
}

#[derive(Debug)]
struct CeldaF {
    at: usize,
    capa: u16,
    opacidad: u8,
    tipo: u16,
    z: i16,
}

#[derive(Debug)]
struct Estructura {
    ancho: u16,
    alto: u16,
    profundidad: u16,
    banderas: u32,
    fotogramas: u16,
    velocidad: u16,
    transparente: u8,
    colores: u16,
    pixel: (u8, u8),
    cuadricula: (u16, u16, u16, u16),
    chunks: Vec<u16>,
    capas: Vec<CapaF>,
    celdas: Vec<CeldaF>,
}

fn estructura(b: &[u8]) -> Result<Estructura, String> {
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
    let nuevos = u32_le(b, 128 + 12)?;
    let cuenta = if nuevos == 0 {
        u32::from(u16_le(b, 128 + 6)?)
    } else {
        nuevos
    };
    let mut e = Estructura {
        ancho: u16_le(b, 8)?,
        alto: u16_le(b, 10)?,
        profundidad: u16_le(b, 12)?,
        banderas: u32_le(b, 14)?,
        fotogramas: u16_le(b, 6)?,
        velocidad: u16_le(b, 18)?,
        transparente: b[28],
        colores: u16_le(b, 32)?,
        pixel: (b[34], b[35]),
        cuadricula: (
            u16_le(b, 36)?,
            u16_le(b, 38)?,
            u16_le(b, 40)?,
            u16_le(b, 42)?,
        ),
        chunks: Vec::new(),
        capas: Vec::new(),
        celdas: Vec::new(),
    };
    let mut pos = 128 + 16;
    for _ in 0..cuenta {
        let tamano = u32_le(b, pos)? as usize;
        let tipo = u16_le(b, pos + 4)?;
        if tamano < 6 || pos + tamano > b.len() {
            return Err(format!("chunk en {pos}: tamaño {tamano} fuera del fichero"));
        }
        let datos = pos + 6;
        e.chunks.push(tipo);
        match tipo {
            // §Layer Chunk: banderas 0, tipo 2, nivel hijo 4, ancho y alto 6 y 8, mezcla 10,
            // opacidad 12, tres bytes, nombre (WORD de longitud) en 16.
            0x2004 => {
                let largo = usize::from(u16_le(b, datos + 16)?);
                let nombre = b
                    .get(datos + 18..datos + 18 + largo)
                    .ok_or("nombre de capa fuera del fichero")?;
                e.capas.push(CapaF {
                    at: datos,
                    banderas: u16_le(b, datos)?,
                    tipo: u16_le(b, datos + 2)?,
                    nivel: u16_le(b, datos + 4)?,
                    mezcla: u16_le(b, datos + 10)?,
                    opacidad: *b.get(datos + 12).ok_or("capa truncada")?,
                    nombre: String::from_utf8_lossy(nombre).into_owned(),
                });
            }
            // §Cel Chunk: capa 0, x 2, y 4, opacidad 6, tipo 7, z-index 9.
            0x2005 => e.celdas.push(CeldaF {
                at: datos,
                capa: u16_le(b, datos)?,
                opacidad: *b.get(datos + 6).ok_or("celda truncada")?,
                tipo: u16_le(b, datos + 7)?,
                z: u16_le(b, datos + 9)? as i16,
            }),
            _ => {}
        }
        pos += tamano;
    }
    Ok(e)
}

/// Cuántas celdas del original **no** escribe Pixoko (3.8, decisión 9: una celda del todo fuera
/// del lienzo o toda transparente no escribe chunk). Lista cerrada, por caso.
fn celdas_que_no_se_escriben(caso: &str) -> usize {
    match caso {
        "celda-recortada" => 2,
        "celda-vacia" => 1,
        _ => 0,
    }
}

/// Compara la estructura escrita por Pixoko con la que escribió Aseprite. Diferencias permitidas
/// (lista cerrada, declarada en 3.8 y medida en la spec): sin chunk de paleta antigua `0x0004`
/// (la paleta va en `0x2019` solo en indexado), número de colores 0 en RGBA y las celdas de
/// `celdas_que_no_se_escriben`. Cualquier otra diferencia es `Err` con el campo.
fn compara(caso: &str, aseprite: &Estructura, pixoko: &Estructura) -> Result<(), String> {
    macro_rules! igual {
        ($campo:expr, $a:expr, $p:expr) => {
            if $a != $p {
                return Err(format!(
                    "{caso}: {}: Aseprite {:?} y Pixoko {:?}",
                    $campo, $a, $p
                ));
            }
        };
    }
    igual!("cabecera: ancho", aseprite.ancho, pixoko.ancho);
    igual!("cabecera: alto", aseprite.alto, pixoko.alto);
    igual!(
        "cabecera: profundidad",
        aseprite.profundidad,
        pixoko.profundidad
    );
    igual!("cabecera: banderas", aseprite.banderas, pixoko.banderas);
    igual!(
        "cabecera: fotogramas",
        aseprite.fotogramas,
        pixoko.fotogramas
    );
    igual!("cabecera: velocidad", aseprite.velocidad, pixoko.velocidad);
    igual!(
        "cabecera: índice transparente",
        aseprite.transparente,
        pixoko.transparente
    );
    igual!("cabecera: razón de píxel", aseprite.pixel, pixoko.pixel);
    igual!(
        "cabecera: cuadrícula",
        aseprite.cuadricula,
        pixoko.cuadricula
    );
    if aseprite.profundidad == 32 {
        igual!("cabecera: número de colores (RGBA)", 0u16, pixoko.colores);
    } else {
        igual!(
            "cabecera: número de colores (indexado)",
            aseprite.colores,
            pixoko.colores
        );
    }
    // Orden de chunks de lo escrito: perfil, paleta solo si es indexado, capas y celdas.
    let mut esperado = vec![0x2007u16];
    if aseprite.profundidad == 8 {
        esperado.push(0x2019);
    }
    esperado.extend(std::iter::repeat_n(0x2004, pixoko.capas.len()));
    esperado.extend(std::iter::repeat_n(0x2005, pixoko.celdas.len()));
    igual!("orden de chunks", esperado, pixoko.chunks);
    let del_original: Vec<u16> = aseprite
        .chunks
        .iter()
        .copied()
        .filter(|&t| t != 0x0004)
        .collect();
    let del_original_sin_celdas: Vec<u16> = del_original
        .iter()
        .copied()
        .filter(|&t| t != 0x2005 && t != 0x2019)
        .collect();
    let del_escrito_sin_celdas: Vec<u16> = pixoko
        .chunks
        .iter()
        .copied()
        .filter(|&t| t != 0x2005 && t != 0x2019)
        .collect();
    igual!(
        "chunks de perfil y capas",
        del_original_sin_celdas,
        del_escrito_sin_celdas
    );
    igual!("número de capas", aseprite.capas.len(), pixoko.capas.len());
    for (i, (a, p)) in aseprite.capas.iter().zip(&pixoko.capas).enumerate() {
        igual!(format!("capa {i}: banderas"), a.banderas, p.banderas);
        igual!(format!("capa {i}: tipo"), a.tipo, p.tipo);
        igual!(format!("capa {i}: nivel hijo"), a.nivel, p.nivel);
        igual!(format!("capa {i}: mezcla"), a.mezcla, p.mezcla);
        igual!(format!("capa {i}: opacidad"), a.opacidad, p.opacidad);
        igual!(format!("capa {i}: nombre"), a.nombre, p.nombre);
    }
    let faltan = aseprite
        .celdas
        .iter()
        .filter(|c| !pixoko.celdas.iter().any(|p| p.capa == c.capa))
        .count();
    igual!(
        "celdas del original que no se escriben",
        celdas_que_no_se_escriben(caso),
        faltan
    );
    for (i, p) in pixoko.celdas.iter().enumerate() {
        let Some(a) = aseprite.celdas.iter().find(|c| c.capa == p.capa) else {
            return Err(format!(
                "{caso}: celda {i}: Pixoko escribe una celda de la capa {} que Aseprite no tiene",
                p.capa
            ));
        };
        igual!(format!("celda {i}: opacidad"), a.opacidad, p.opacidad);
        igual!(format!("celda {i}: tipo"), a.tipo, p.tipo);
        igual!(format!("celda {i}: z-index"), a.z, p.z);
    }
    Ok(())
}

fn fila_03(caso: &str) -> Result<(), String> {
    let original = golden(&format!("{caso}.aseprite"));
    let documento = lee(&original).map_err(|e| format!("{caso}: {e}"))?.document;
    let escrito = write_aseprite(&documento).map_err(|e| format!("{caso}: {e}"))?;
    compara(caso, &estructura(&original)?, &estructura(&escrito)?)
}

#[test]
fn fila_03_la_estructura_escrita_es_la_de_aseprite_salvo_la_lista_cerrada() {
    let fallos: Vec<String> = CASOS.iter().filter_map(|c| fila_03(c).err()).collect();
    assert!(fallos.is_empty(), "{}", fallos.join("\n"));
}

// ---------------------------------------------------------------------------------------
// Fila 4: controles negativos.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_04_a_el_corpus_sin_un_caso_o_con_uno_de_mas_falla() {
    let carpeta = raiz_del_repo().join("golden/aseprite");
    let ficheros = lee_carpeta(&carpeta);
    // La carpeta real está bien.
    let casos = verificar(&ficheros).expect("golden/aseprite es válida");
    comparar_con_lista(&casos, &CASOS).expect("los 29 casos de la lista");
    // Sin un caso (sus tres ficheros): el verificador no se queja (queda coherente), la lista sí.
    for quitar in [CASOS[0], "mixto-rgba", "capa-bloqueada"] {
        let sin: Vec<_> = ficheros
            .iter()
            .filter(|f| !f.nombre.starts_with(&format!("{quitar}.")))
            .cloned()
            .collect();
        let casos = verificar(&sin).expect("sin un caso queda coherente");
        let error = comparar_con_lista(&casos, &CASOS).expect_err("falta un caso");
        assert!(error.contains(quitar), "{error}");
    }
    // Con uno de más: tres ficheros nuevos (copias con otro nombre): 30 casos.
    let mut con = ficheros.clone();
    for sufijo in [".aseprite", ".png", ".png.sha256"] {
        let origen = ficheros
            .iter()
            .find(|f| f.nombre == format!("mixto-rgba{sufijo}"))
            .expect("fichero de origen");
        let mut copia = origen.clone();
        copia.nombre = format!("caso-extra{sufijo}");
        con.push(copia);
    }
    let error = verificar(&con).expect_err("30 casos pasan del máximo");
    assert!(error.contains("hay 30 casos"), "{error}");
}

/// Un PNG con un canal de un píxel con alfa > 0 cambiado.
fn con_un_pixel_cambiado(png: &Image) -> (Image, (u32, u32)) {
    for y in 0..png.height() {
        for x in 0..png.width() {
            let c = rgba_de(png, x, y);
            if c.a > 0 {
                let mut otro = png.clone();
                otro.set_pixel(x, y, Pixel::Rgba(Rgba::new(c.r ^ 1, c.g, c.b, c.a)))
                    .expect("píxel dentro");
                return (otro, (x, y));
            }
        }
    }
    panic!("el PNG no tiene ningún píxel con alfa > 0");
}

#[test]
fn fila_04_b_un_pixel_cambiado_de_un_png_hace_fallar_la_composicion_y_el_hash() {
    let caso = "mixto-rgba";
    let doc = lee_documento(caso).expect("documento");
    let compuesto = compose(&doc).expect("compose");
    let png = lee_png(caso);
    evalua_imagen(caso, &doc, &compuesto, &png).expect("el PNG original pasa");
    let (cambiado, (x, y)) = con_un_pixel_cambiado(&png);
    let error = evalua_imagen(caso, &doc, &compuesto, &cambiado).expect_err("un píxel distinto");
    assert!(
        error.contains(caso) && error.contains(&format!("({x}, {y})")),
        "{error}"
    );
    // El corpus también lo ve: el `.sha256` es el de la imagen original.
    let mut ficheros = lee_carpeta(&raiz_del_repo().join("golden/aseprite"));
    let png_nuevo = write_png(&cambiado).expect("write_png");
    ficheros
        .iter_mut()
        .find(|f| f.nombre == format!("{caso}.png"))
        .expect("el PNG del caso")
        .bytes = png_nuevo;
    let error = verificar(&ficheros).expect_err("el hash ya no coincide");
    assert!(
        error.contains(&format!("{caso}.png: el hash no coincide")),
        "{error}"
    );
    // Un `.sha256` de otra imagen, lo mismo.
    let mut ficheros = lee_carpeta(&raiz_del_repo().join("golden/aseprite"));
    let otro = ficheros
        .iter()
        .find(|f| f.nombre == "mezcla-normal.png.sha256")
        .expect("otro hash")
        .bytes
        .clone();
    ficheros
        .iter_mut()
        .find(|f| f.nombre == format!("{caso}.png.sha256"))
        .expect("su hash")
        .bytes = otro;
    assert!(verificar(&ficheros).is_err());
}

/// Cambia un byte del fichero escrito y exige que la comparación de la fila 3 diga qué campo.
#[test]
fn fila_04_c_un_campo_cambiado_del_fichero_escrito_hace_fallar_la_estructura() {
    let caso = "mixto-rgba";
    let original = golden(&format!("{caso}.aseprite"));
    let documento = lee(&original).expect("lee").document;
    let escrito = write_aseprite(&documento).expect("write");
    let de_aseprite = estructura(&original).expect("estructura del original");
    let e = estructura(&escrito).expect("estructura de lo escrito");
    compara(caso, &de_aseprite, &e).expect("sin mutar, coincide");
    assert!(e.capas.len() >= 4 && !e.celdas.is_empty());

    // (campo esperado en el mensaje, desplazamiento, máscara que se aplica con XOR)
    let mutaciones: Vec<(&str, usize, u8)> = vec![
        ("capa 1: banderas", e.capas[1].at, 1),
        ("capa 1: banderas", e.capas[1].at, 2),
        ("capa 0: tipo", e.capas[0].at + 2, 1),
        ("capa 0: nivel hijo", e.capas[0].at + 4, 1),
        ("capa 2: mezcla", e.capas[2].at + 10, 1),
        ("capa 2: opacidad", e.capas[2].at + 12, 1),
        ("capa 3: nombre", e.capas[3].at + 18, 1),
        ("celda 0: opacidad", e.celdas[0].at + 6, 0x80),
        ("celda 0: tipo", e.celdas[0].at + 7, 1),
        ("celda 0: z-index", e.celdas[0].at + 9, 1),
        ("cabecera: ancho", 8, 1),
        ("cabecera: profundidad", 12, 8),
        ("cabecera: banderas", 14, 1),
        ("cabecera: velocidad", 18, 1),
        ("cabecera: cuadrícula", 40, 1),
        ("magia", 4, 1),
    ];
    for (campo, desplazamiento, mascara) in mutaciones {
        let mut roto = escrito.clone();
        roto[desplazamiento] ^= mascara;
        let error = match estructura(&roto) {
            Err(e) => e,
            Ok(otra) => compara(caso, &de_aseprite, &otra).expect_err(campo),
        };
        assert!(
            error.contains(campo),
            "cambiar un byte en {desplazamiento} debía dar «{campo}» y da: {error}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: la composición sobrevive a guardar y reabrir.
// ---------------------------------------------------------------------------------------

fn nombre_de_capa() -> impl Strategy<Value = String> {
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

fn celda_rgba(n: usize) -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        5 => vec(pixel_rgba(), n).prop_map(|px| Some(px.concat())),
    ]
}

fn capa(
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

fn documento_rgba(ancho: u32, alto: u32) -> BoxedStrategy<Document> {
    let n = (ancho * alto) as usize;
    let una = (
        nombre_de_capa(),
        any::<bool>(),
        prop_oneof![1 => Just(255u8), 1 => any::<u8>()],
        0usize..8,
        any::<bool>(),
        celda_rgba(n),
    );
    vec(una, 1..=6)
        .prop_map(move |capas| {
            let specs = capas
                .into_iter()
                .map(|(nombre, visible, opacidad, modo, bloqueada, celda)| {
                    let imagen = celda
                        .map(|b| Image::from_bytes(ancho, alto, Mode::Rgba, b).expect("imagen"));
                    capa(
                        &nombre,
                        visible,
                        opacidad,
                        BlendMode::ALL[modo],
                        bloqueada,
                        imagen,
                    )
                })
                .collect();
            Document::from_layers(ancho, alto, Mode::Rgba, None, specs).expect("documento RGBA")
        })
        .boxed()
}

fn documento_indexado(ancho: u32, alto: u32) -> BoxedStrategy<Document> {
    let n = (ancho * alto) as usize;
    (vec(any::<[u8; 4]>(), 2..=16), any::<u8>())
        .prop_flat_map(move |(colores, t)| {
            let largo = colores.len();
            let transparente = (usize::from(t) % largo) as u8;
            let celda = prop_oneof![1 => Just(None), 5 => vec(any::<u8>(), n).prop_map(Some)];
            let una = (nombre_de_capa(), any::<bool>(), any::<bool>(), celda);
            vec(una, 1..=6).prop_map(move |capas| {
                let mut paleta =
                    Palette::from_colors(colores.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
                        .expect("paleta");
                paleta
                    .set_transparent_index(Some(transparente))
                    .expect("índice transparente");
                let specs = capas
                    .into_iter()
                    .map(|(nombre, visible, bloqueada, celda)| {
                        let imagen = celda.map(|b| {
                            let indices = b
                                .iter()
                                .map(|&v| {
                                    if v % 3 == 0 {
                                        transparente
                                    } else {
                                        (usize::from(v) % largo) as u8
                                    }
                                })
                                .collect();
                            Image::from_bytes_indexed(ancho, alto, paleta.clone(), indices)
                                .expect("imagen indexada")
                        });
                        capa(&nombre, visible, 255, BlendMode::Normal, bloqueada, imagen)
                    })
                    .collect();
                Document::from_layers(ancho, alto, Mode::Indexed, Some(paleta.clone()), specs)
                    .expect("documento indexado")
            })
        })
        .boxed()
}

fn documento_cualquiera() -> impl Strategy<Value = Document> {
    (1u32..=12, 1u32..=12, any::<bool>()).prop_flat_map(|(ancho, alto, indexado)| {
        if indexado {
            documento_indexado(ancho, alto)
        } else {
            documento_rgba(ancho, alto)
        }
    })
}

fn falla<T, E: std::fmt::Display>(r: Result<T, E>) -> Result<T, TestCaseError> {
    r.map_err(|e| TestCaseError::fail(e.to_string()))
}

fn reabre(doc: &Document) -> Result<Document, TestCaseError> {
    let bytes = falla(write_aseprite(doc))?;
    Ok(falla(lee(&bytes))?.document)
}

/// La composición (o el error) tras aplicar `orden` a `doc`; `None` si no hay nada que hacer.
fn tras(doc: &Document, aplanar: bool) -> Result<Option<Image>, TestCaseError> {
    let mut doc = doc.clone();
    let orden = if aplanar {
        flatten_command(&doc)
    } else {
        merge_visible_command(&doc)
    };
    match orden {
        Err(MergeError::NothingToMerge) => Ok(None),
        Err(e) => Err(TestCaseError::fail(format!("la orden dio {e:?}"))),
        Ok(orden) => {
            falla(doc.apply(orden))?;
            Ok(Some(falla(compose(&doc).map_err(|e| format!("{e:?}")))?))
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// P4. Guardar y reabrir no cambia la composición salvo el RGB de los píxeles con alfa 0, y
    /// aplanar y combinar visibles conmutan con guardar y reabrir con la misma exención.
    #[test]
    fn propiedad_4_la_composicion_sobrevive_a_guardar_y_reabrir(doc in documento_cualquiera()) {
        let reabierto = reabre(&doc)?;
        falla(mismo_contenido(&doc, &reabierto))?;
        let antes = falla(compose(&doc).map_err(|e| format!("{e:?}")))?;
        let despues = falla(compose(&reabierto).map_err(|e| format!("{e:?}")))?;
        falla(igual_salvo_rgb_con_alfa_0(&antes, &despues))?;
        for aplanar in [true, false] {
            let a = tras(&doc, aplanar)?;
            let b = tras(&reabierto, aplanar)?;
            match (a, b) {
                (None, None) => {}
                (Some(a), Some(b)) => falla(igual_salvo_rgb_con_alfa_0(&a, &b))?,
                (a, b) => prop_assert!(false, "una orden da {} y la otra {}", a.is_some(), b.is_some()),
            }
            // Escribir lo aplanado y reabrirlo es lo mismo que aplanar lo reabierto.
            let mut plano = doc.clone();
            let orden = if aplanar { flatten_command(&plano) } else { merge_visible_command(&plano) };
            if let Ok(orden) = orden {
                falla(plano.apply(orden))?;
                let plano_reabierto = reabre(&plano)?;
                let (x, y) = (
                    falla(compose(&plano).map_err(|e| format!("{e:?}")))?,
                    falla(compose(&plano_reabierto).map_err(|e| format!("{e:?}")))?,
                );
                falla(igual_salvo_rgb_con_alfa_0(&x, &y))?;
            }
        }
    }
}
