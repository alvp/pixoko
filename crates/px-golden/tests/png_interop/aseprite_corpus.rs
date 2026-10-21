//! Corpus `golden/aseprite/` (specs/corpus-hito-3.md, «Prueba del corpus»): cada `X.aseprite`
//! tiene su `X.png` aplanado y su `X.png.sha256`, con el mismo criterio que `golden/png/` (SHA-256
//! de la RGBA de 8 bits expandida, calculado con otra herramienta).
//!
//! Aquí no se lee el formato `.aseprite` (eso es la unidad 3.8): solo se comprueba de cada fichero
//! la cabecera de 128 bytes de la especificación pública (tamaño, magia `0xA5E0`, un fotograma,
//! dimensiones y profundidad de color) y que sus dimensiones sean las del PNG.
//!
//! Si `golden/aseprite/` no existe, el test de la carpeta real avisa y pasa: el corpus lo genera el
//! usuario con `tools/corpus-h3/generar.sh`. Con la carpeta presente es estricto como el de
//! `golden/png/` (carpeta vacía = error).

use px_formats::read_png;

use super::common::sha256::sha256_hex;
use super::{Fichero, bytes_rgba, fichero, hash_bien_formado, lee_carpeta, raiz_del_repo};

/// Tope de casos de la spec.
const MAX_CASOS: usize = 25;

/// `a-z`, `0-9` y guiones, sin guiones al principio, al final ni dobles.
fn nombre_valido(nombre: &str) -> bool {
    !nombre.is_empty()
        && nombre.split('-').all(|t| {
            !t.is_empty()
                && t.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn u16_le(b: &[u8], pos: usize) -> usize {
    usize::from(u16::from_le_bytes([b[pos], b[pos + 1]]))
}

/// Comprueba la cabecera de un `.aseprite` y devuelve (ancho, alto).
fn cabecera(bytes: &[u8]) -> Result<(usize, usize), String> {
    if bytes.len() < 128 {
        return Err(format!(
            "solo tiene {} bytes (la cabecera es de 128)",
            bytes.len()
        ));
    }
    let tamano = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if tamano != bytes.len() {
        return Err(format!(
            "la cabecera dice {tamano} bytes y el fichero tiene {}",
            bytes.len()
        ));
    }
    if u16_le(bytes, 4) != 0xA5E0 {
        return Err("la magia no es 0xA5E0".to_string());
    }
    if u16_le(bytes, 6) != 1 {
        return Err(format!(
            "el corpus del hito 3 tiene un fotograma y hay {}",
            u16_le(bytes, 6)
        ));
    }
    let (ancho, alto) = (u16_le(bytes, 8), u16_le(bytes, 10));
    if !(16..=64).contains(&ancho) || !(16..=64).contains(&alto) {
        return Err(format!("tamaño {ancho}x{alto} fuera de 16x16 a 64x64"));
    }
    match u16_le(bytes, 12) {
        32 | 8 => Ok((ancho, alto)),
        otra => Err(format!(
            "profundidad de color {otra}: solo RGBA (32) o indexado (8)"
        )),
    }
}

/// Todos los problemas juntos, uno por línea; `Ok` con los nombres de los casos comprobados.
fn verificar(ficheros: &[Fichero]) -> Result<Vec<String>, String> {
    let buscar = |nombre: &str| ficheros.iter().find(|f| f.nombre == nombre);
    let mut problemas = Vec::new();
    let mut casos = Vec::new();

    if buscar("README.md").is_none() {
        problemas.push("falta README.md (versión de Aseprite, fecha y origen)".to_string());
    }
    for f in ficheros {
        let n = f.nombre.as_str();
        if n == "README.md" {
            continue;
        }
        if let Some(caso) = n.strip_suffix(".aseprite") {
            casos.push(caso.to_string());
        } else if let Some(caso) = n.strip_suffix(".png.sha256") {
            if buscar(&format!("{caso}.png")).is_none() {
                problemas.push(format!("{n}: no hay {caso}.png al lado"));
            }
        } else if let Some(caso) = n.strip_suffix(".png") {
            if buscar(&format!("{caso}.aseprite")).is_none() {
                problemas.push(format!("{n}: no hay {caso}.aseprite al lado"));
            }
        } else {
            problemas.push(format!(
                "{n}: fichero ajeno (solo README.md, *.aseprite, *.png y *.png.sha256)"
            ));
        }
    }

    if casos.is_empty() {
        problemas.insert(
            0,
            "golden/aseprite no contiene ningún .aseprite: el corpus no puede estar vacío"
                .to_string(),
        );
    }
    if casos.len() > MAX_CASOS {
        problemas.push(format!(
            "hay {} casos y la spec admite {MAX_CASOS} como máximo",
            casos.len()
        ));
    }

    for caso in &casos {
        if !nombre_valido(caso) {
            problemas.push(format!(
                "{caso}.aseprite: el nombre debe ser minúsculas, dígitos y guiones"
            ));
        }
        let dims = match cabecera(&buscar(&format!("{caso}.aseprite")).unwrap().bytes) {
            Ok(d) => Some(d),
            Err(e) => {
                problemas.push(format!("{caso}.aseprite: {e}"));
                None
            }
        };
        let Some(png) = buscar(&format!("{caso}.png")) else {
            problemas.push(format!("{caso}.aseprite: falta {caso}.png"));
            continue;
        };
        let Some(hash) = buscar(&format!("{caso}.png.sha256")) else {
            problemas.push(format!("{caso}.png: falta {caso}.png.sha256"));
            continue;
        };
        let esperado = match hash_bien_formado(&hash.bytes) {
            Ok(h) => h,
            Err(e) => {
                problemas.push(format!("{caso}.png.sha256: hash mal formado: {e}"));
                continue;
            }
        };
        let leido = match read_png(&png.bytes) {
            Ok(r) => r,
            Err(e) => {
                problemas.push(format!("{caso}.png: read_png dio error: {e}"));
                continue;
            }
        };
        let (w, h) = (leido.image.width() as usize, leido.image.height() as usize);
        if let Some((aw, ah)) = dims
            && (w, h) != (aw, ah)
        {
            problemas.push(format!(
                "{caso}: el .aseprite es de {aw}x{ah} y el PNG de {w}x{h}"
            ));
        }
        let calculado = sha256_hex(&bytes_rgba(&leido.image));
        if calculado != esperado {
            problemas.push(format!(
                "{caso}.png: el hash no coincide: esperado {esperado}, calculado {calculado}"
            ));
        }
    }

    if problemas.is_empty() {
        casos.sort();
        Ok(casos)
    } else {
        Err(problemas.join("\n"))
    }
}

// ---------------------------------------------------------------------------------------
// Filas 1 y 2: el verificador con ficheros en memoria.
// ---------------------------------------------------------------------------------------

/// Cabecera mínima válida de 128 bytes: 1 fotograma, `ancho` x `alto`, color de 32 bits.
fn aseprite(ancho: u16, alto: u16) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[0..4].copy_from_slice(&128u32.to_le_bytes());
    b[4..6].copy_from_slice(&0xA5E0u16.to_le_bytes());
    b[6..8].copy_from_slice(&1u16.to_le_bytes());
    b[8..10].copy_from_slice(&ancho.to_le_bytes());
    b[10..12].copy_from_slice(&alto.to_le_bytes());
    b[12..14].copy_from_slice(&32u16.to_le_bytes());
    b
}

/// PNG RGBA de 8 bits, liso, y su SHA-256 como lo escribiría el generador.
fn png_liso(ancho: u32, alto: u32, valor: u8) -> (Vec<u8>, String) {
    let crudo = vec![valor; (ancho * alto * 4) as usize];
    let muestras: Vec<u16> = crudo.iter().map(|&b| u16::from(b)).collect();
    let png = super::common::Png::new(ancho, alto, 8, 6)
        .muestras(&muestras)
        .bytes();
    (png, sha256_hex(&crudo))
}

/// Un caso correcto: `.aseprite`, `.png` y `.png.sha256`, más el README.
fn corpus_bueno(nombre: &str) -> Vec<Fichero> {
    let (png, hash) = png_liso(16, 16, 200);
    vec![
        fichero("README.md", "# golden/aseprite"),
        fichero(&format!("{nombre}.aseprite"), aseprite(16, 16)),
        fichero(&format!("{nombre}.png"), png),
        fichero(&format!("{nombre}.png.sha256"), format!("{hash}\n")),
    ]
}

fn sin(mut v: Vec<Fichero>, nombre: &str) -> Vec<Fichero> {
    v.retain(|f| f.nombre != nombre);
    v
}

#[test]
fn aseprite_fila_1_un_corpus_correcto_pasa() {
    assert_eq!(
        verificar(&corpus_bueno("rgba-una-capa")),
        Ok(vec!["rgba-una-capa".to_string()])
    );
}

#[test]
fn aseprite_fila_2_cada_defecto_da_error_con_su_mensaje() {
    let (png_otro, hash_otro) = png_liso(16, 16, 7);
    let mut hash_malo = corpus_bueno("a");
    hash_malo[3] = fichero("a.png.sha256", hash_otro);
    let mut magia = corpus_bueno("a");
    magia[1].bytes[4] = 0;
    let mut tamano = corpus_bueno("a");
    tamano[1].bytes.push(0);
    let mut dimensiones = corpus_bueno("a");
    dimensiones[1] = fichero("a.aseprite", aseprite(32, 16));
    let mut fuera_de_rango = corpus_bueno("a");
    fuera_de_rango[1] = fichero("a.aseprite", aseprite(8, 8));
    let mut dos_fotogramas = corpus_bueno("a");
    dos_fotogramas[1].bytes[6] = 2;
    let mut ajeno = corpus_bueno("a");
    ajeno.push(fichero("notas.txt", "x"));
    let mayusculas = corpus_bueno("Capa-Uno");
    let mut otro_png = corpus_bueno("a");
    otro_png[2] = fichero("a.png", png_otro);
    let mut demasiados = corpus_bueno("caso-0");
    for i in 1..=MAX_CASOS {
        demasiados.extend(corpus_bueno(&format!("caso-{i}")).into_iter().skip(1));
    }

    let casos: Vec<(&str, Vec<Fichero>, &str)> = vec![
        ("vacía", Vec::new(), "no puede estar vacío"),
        (
            "sin README",
            sin(corpus_bueno("a"), "README.md"),
            "falta README.md",
        ),
        ("sin PNG", sin(corpus_bueno("a"), "a.png"), "falta a.png"),
        (
            "sin .sha256",
            sin(corpus_bueno("a"), "a.png.sha256"),
            "falta a.png.sha256",
        ),
        (
            "PNG sin .aseprite",
            sin(corpus_bueno("a"), "a.aseprite"),
            "no hay a.aseprite",
        ),
        ("hash de otra imagen", hash_malo, "el hash no coincide"),
        ("magia", magia, "magia"),
        ("tamaño de cabecera", tamano, "la cabecera dice"),
        ("dimensiones distintas", dimensiones, "el PNG de 16x16"),
        ("fuera de 16 a 64", fuera_de_rango, "fuera de 16x16 a 64x64"),
        ("dos fotogramas", dos_fotogramas, "un fotograma"),
        ("fichero ajeno", ajeno, "fichero ajeno"),
        (
            "nombre con mayúsculas",
            mayusculas,
            "minúsculas, dígitos y guiones",
        ),
        ("PNG de otra imagen", otro_png, "el hash no coincide"),
        ("más de 25", demasiados, "como máximo"),
    ];
    for (descripcion, ficheros, fragmento) in casos {
        match verificar(&ficheros) {
            Ok(_) => panic!("{descripcion}: debía dar error"),
            Err(e) => assert!(
                e.contains(fragmento),
                "{descripcion}: {e:?} no contiene {fragmento:?}"
            ),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: la carpeta real.
// ---------------------------------------------------------------------------------------

#[test]
fn aseprite_fila_3_cada_aseprite_de_golden_tiene_su_png_y_su_sha256() {
    let dir = raiz_del_repo().join("golden/aseprite");
    if !dir.is_dir() {
        eprintln!(
            "golden/aseprite no existe: corpus pendiente (tools/corpus-h3/generar.sh, specs/corpus-hito-3.md)"
        );
        return;
    }
    match verificar(&lee_carpeta(&dir)) {
        Ok(casos) => println!("golden/aseprite: {} casos comprobados", casos.len()),
        Err(e) => panic!("corpus de golden/aseprite con problemas:\n{e}"),
    }
}
