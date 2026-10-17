//! Interoperabilidad con los PNG de `golden/png/` (specs/hito-1-png-corpus.md, punto 4).
//!
//! Los PNG de la carpeta los guardaron Grafx2 y LibreSprite; cada `X.png` lleva al lado
//! `X.png.sha256`, el SHA-256 de la imagen expandida a RGBA de 8 bits (alfa directo, sin
//! cabecera ni dimensiones) que calculó una herramienta independiente. Aquí se lee cada PNG
//! con `read_png` y se compara el hash de su vista RGBA.
//!
//! Piezas:
//! - `verificar_corpus`: función pura sobre una lista de ficheros en memoria. Tiene sus
//!   propios tests (fila 14) para demostrar que el corpus sabe fallar.
//! - El test de la carpeta real (fila 13) solo lee el directorio y llama al verificador.
//! - El SHA-256 propio (`common::sha256`) se valida con sus vectores en un único test
//!   (fila 15), aparte de la fila 13: si falla el hash se sabe si es el SHA-256 o el PNG.
//! - La fila 16 comprueba la estructura del repositorio (`.gitattributes` y el README).

mod common;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use common::sha256::{sha256_hex, vectores};
use common::{Png, plte, trns};
use px_core::{Image, Pixel};
use px_formats::{Conversion, read_png};

// ---------------------------------------------------------------------------------------
// El verificador del corpus.
// ---------------------------------------------------------------------------------------

/// Mensaje cuando no hay ningún PNG: el test falla, nunca pasa en vacío.
const MENSAJE_VACIO: &str = "golden/png no contiene ningún PNG: el corpus de interoperabilidad no puede estar vacío (ver golden/png/README.md)";

/// Un fichero de la carpeta: nombre (sin ruta) y contenido.
struct Fichero {
    nombre: String,
    bytes: Vec<u8>,
}

fn fichero(nombre: &str, bytes: impl Into<Vec<u8>>) -> Fichero {
    Fichero {
        nombre: nombre.to_string(),
        bytes: bytes.into(),
    }
}

/// Lo que dejó una verificación correcta.
struct Resumen {
    /// Informe de conversiones de cada PNG leído (`read_png` con `Ok`).
    informes: Vec<(String, Vec<Conversion>)>,
    /// PNG cuyo hash coincide con su `.sha256`.
    comprobados: Vec<String>,
    /// PNG de 16 bits: se leen, pero no se hashean.
    omitidos: Vec<String>,
}

/// Vista RGBA de una imagen: `ancho × alto × 4` bytes, fila a fila de arriba abajo y de
/// izquierda a derecha. `Rgba(c)` da `[r, g, b, a]`; `Indexed(i)`, la entrada resuelta de la
/// paleta; `Gray(g)`, `[g, g, g, 255]`; `GrayAlpha`, `[gray, gray, gray, alpha]`. El RGB bajo
/// alfa 0 se conserva.
fn bytes_rgba(img: &Image) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            match img.get_pixel(x, y).unwrap() {
                Pixel::Rgba(c) => v.extend([c.r, c.g, c.b, c.a]),
                Pixel::Indexed(i) => {
                    let c = img.palette().unwrap().resolved(i).unwrap();
                    v.extend([c.r, c.g, c.b, c.a]);
                }
                Pixel::Gray(g) => v.extend([g, g, g, 255]),
                Pixel::GrayAlpha { gray, alpha } => v.extend([gray, gray, gray, alpha]),
            }
        }
    }
    v
}

/// Contenido de un `.sha256`: recortados los espacios y saltos de línea de los extremos,
/// exactamente 64 dígitos hexadecimales. Devuelve el hash en minúsculas.
fn hash_bien_formado(bytes: &[u8]) -> Result<String, String> {
    let texto = std::str::from_utf8(bytes).map_err(|_| "no es texto UTF-8".to_string())?;
    let hash = texto.trim();
    if hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(hash.to_ascii_lowercase())
    } else {
        Err(format!(
            "se esperaban exactamente 64 dígitos hexadecimales y hay {hash:?}"
        ))
    }
}

/// Comprueba el corpus y devuelve **todos** los problemas juntos, uno por línea.
///
/// Es un problema: no haber ningún PNG; un PNG sin `.sha256` (salvo los de 16 bits) o un
/// `.sha256` sin PNG; un fichero que no es `README.md`, `*.png` ni `*.png.sha256`; un
/// `.sha256` mal formado, un `read_png` con `Err` o un hash que no coincide; y que no quede
/// ningún PNG comprobado porque todos son de 16 bits.
fn verificar_corpus(ficheros: &[Fichero]) -> Result<Resumen, String> {
    let buscar = |nombre: &str| ficheros.iter().find(|f| f.nombre == nombre);
    let pngs: Vec<&Fichero> = ficheros
        .iter()
        .filter(|f| f.nombre.ends_with(".png"))
        .collect();
    let mut problemas = Vec::new();

    for f in ficheros {
        if f.nombre == "README.md" || f.nombre.ends_with(".png") {
            continue;
        }
        match f.nombre.strip_suffix(".sha256") {
            Some(png) if png.ends_with(".png") => {
                if buscar(png).is_none() {
                    problemas.push(format!("{}: no hay {png} al lado", f.nombre));
                }
            }
            _ => problemas.push(format!(
                "{}: fichero ajeno (solo se admiten README.md, *.png y *.png.sha256)",
                f.nombre
            )),
        }
    }

    let mut informes = Vec::new();
    let mut comprobados = Vec::new();
    let mut omitidos = Vec::new();
    for png in &pngs {
        let nombre = &png.nombre;
        // Profundidad de bits: byte 24 del fichero (8 de firma, 4 de longitud, 4 de tipo,
        // 4 de ancho y 4 de alto).
        let de_16_bits = png.bytes.get(24) == Some(&16);
        let leido = match read_png(&png.bytes) {
            Ok(r) => r,
            Err(e) => {
                problemas.push(format!("{nombre}: read_png dio error: {e}"));
                continue;
            }
        };
        // Se imprime al leer (no al final) para que se vea también si el corpus falla.
        println!("{nombre}: informe de conversiones {:?}", leido.report);
        informes.push((nombre.clone(), leido.report.clone()));

        if de_16_bits {
            if !leido.report.contains(&Conversion::Depth16Reduced) {
                problemas.push(format!(
                    "{nombre}: es de 16 bits y el informe no incluye Depth16Reduced: {:?}",
                    leido.report
                ));
            }
            omitidos.push(nombre.clone());
            continue;
        }

        let nombre_hash = format!("{nombre}.sha256");
        let Some(hash) = buscar(&nombre_hash) else {
            problemas.push(format!("{nombre}: falta {nombre_hash}"));
            continue;
        };
        let esperado = match hash_bien_formado(&hash.bytes) {
            Ok(h) => h,
            Err(e) => {
                problemas.push(format!("{nombre_hash}: hash mal formado: {e}"));
                continue;
            }
        };
        let calculado = sha256_hex(&bytes_rgba(&leido.image));
        if calculado == esperado {
            comprobados.push(nombre.clone());
        } else {
            problemas.push(format!(
                "{nombre}: el hash no coincide: esperado {esperado}, calculado {calculado}"
            ));
        }
    }

    if pngs.is_empty() {
        problemas.insert(0, MENSAJE_VACIO.to_string());
    } else if comprobados.is_empty() && omitidos.len() == pngs.len() {
        problemas.push(
            "ningún PNG comprobado: todos los PNG de la carpeta son de 16 bits (omitidos del hash)"
                .to_string(),
        );
    }

    if problemas.is_empty() {
        Ok(Resumen {
            informes,
            comprobados,
            omitidos,
        })
    } else {
        Err(problemas.join("\n"))
    }
}

// ---------------------------------------------------------------------------------------
// Fila 13: la carpeta real.
// ---------------------------------------------------------------------------------------

fn raiz_del_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn carpeta_golden_png() -> PathBuf {
    raiz_del_repo().join("golden/png")
}

/// Nombres de los ficheros de una carpeta, ordenados. Una carpeta que no existe es una
/// carpeta vacía (el verificador la rechaza con el mensaje del corpus vacío).
fn nombres_en(dir: &Path) -> Vec<String> {
    let entradas = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => panic!("no se pudo leer el directorio {}: {e}", dir.display()),
    };
    let mut nombres: Vec<String> = entradas
        .map(|e| e.expect("entrada de directorio ilegible"))
        .map(|e| {
            let nombre = e.file_name().to_string_lossy().into_owned();
            // Un subdirectorio se anota con barra final: no es README, ni PNG ni .sha256.
            if e.path().is_dir() {
                format!("{nombre}/")
            } else {
                nombre
            }
        })
        .collect();
    nombres.sort();
    nombres
}

/// Lee la carpeta entera como lista de ficheros en memoria.
fn lee_carpeta(dir: &Path) -> Vec<Fichero> {
    nombres_en(dir)
        .into_iter()
        .map(|nombre| {
            let bytes = if nombre.ends_with('/') {
                Vec::new()
            } else {
                let ruta = dir.join(&nombre);
                fs::read(&ruta)
                    .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()))
            };
            Fichero { nombre, bytes }
        })
        .collect()
}

#[test]
fn fila_13_cada_png_de_golden_tiene_el_hash_de_su_sha256() {
    let ficheros = lee_carpeta(&carpeta_golden_png());
    match verificar_corpus(&ficheros) {
        Ok(resumen) => {
            // Visible con `cargo test -- --nocapture`, para depurar.
            for nombre in &resumen.comprobados {
                println!("{nombre}: hash coincide");
            }
            for nombre in &resumen.omitidos {
                println!("{nombre}: omitido del hash (16 bits)");
            }
        }
        Err(problemas) => panic!("el corpus de golden/png no pasa:\n{problemas}"),
    }
}

// ---------------------------------------------------------------------------------------
// Fila 14: el verificador con listas en memoria (el corpus sabe fallar).
// ---------------------------------------------------------------------------------------

/// Cuatro PNG de 8 bits de 2 × 1 (uno por modo) con la vista RGBA escrita a mano, sin
/// pasar por `bytes_rgba`: (nombre, PNG, bytes de la vista RGBA).
fn pngs_de_ejemplo() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
    vec![
        (
            "gris.png",
            Png::new(2, 1, 8, 0).muestras(&[10, 200]).bytes(),
            vec![10, 10, 10, 255, 200, 200, 200, 255],
        ),
        (
            "gris-alfa.png",
            Png::new(2, 1, 8, 4).muestras(&[77, 0, 200, 255]).bytes(),
            vec![77, 77, 77, 0, 200, 200, 200, 255],
        ),
        (
            "rgba.png",
            Png::new(2, 1, 8, 6)
                .muestras(&[10, 20, 30, 0, 40, 50, 60, 255])
                .bytes(),
            vec![10, 20, 30, 0, 40, 50, 60, 255],
        ),
        (
            // Paleta de 2 entradas con tRNS: los píxeles son los índices 1 y 0.
            "indexado.png",
            Png::new(2, 1, 8, 3)
                .muestras(&[1, 0])
                .antes(plte(&[(1, 2, 3), (4, 5, 6)]))
                .antes(trns(&[0, 128]))
                .bytes(),
            vec![4, 5, 6, 128, 1, 2, 3, 0],
        ),
    ]
}

/// Un corpus válido: los PNG de ejemplo, cada uno con su `.sha256` correcto.
fn corpus_valido() -> Vec<Fichero> {
    let mut v = vec![fichero("README.md", "# golden/png\n")];
    for (nombre, png, vista) in pngs_de_ejemplo() {
        v.push(fichero(
            &format!("{nombre}.sha256"),
            format!("{}\n", sha256_hex(&vista)),
        ));
        v.push(fichero(nombre, png));
    }
    v
}

/// Hash correcto de `gris.png`.
fn hash_de_gris() -> String {
    sha256_hex(&[10, 10, 10, 255, 200, 200, 200, 255])
}

/// El corpus válido con el contenido de un fichero cambiado.
fn con_contenido(nombre: &str, bytes: impl Into<Vec<u8>>) -> Vec<Fichero> {
    let bytes = bytes.into();
    let mut v = corpus_valido();
    v.iter_mut().find(|f| f.nombre == nombre).unwrap().bytes = bytes;
    v
}

/// PNG de 16 bits (gris, 2 × 1) y su nombre.
fn png_de_16_bits() -> Vec<u8> {
    Png::new(2, 1, 16, 0).muestras(&[0x0000, 0xFFFF]).bytes()
}

/// Error esperado: el texto debe contener todos los fragmentos.
fn assert_err_con(r: Result<Resumen, String>, fragmentos: &[&str]) {
    match r {
        Ok(_) => panic!("se esperaba Err con {fragmentos:?} y fue Ok"),
        Err(msg) => {
            for f in fragmentos {
                assert!(msg.contains(f), "falta {f:?} en el mensaje:\n{msg}");
            }
        }
    }
}

#[test]
fn fila_14_ayudante_los_pngs_de_ejemplo_son_validos_y_el_de_16_bits_se_detecta() {
    let corpus = corpus_valido();
    let resumen = verificar_corpus(&corpus).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(resumen.comprobados.len(), 4);
    assert!(resumen.omitidos.is_empty());
    // Ninguno de los de ejemplo tiene conversiones: las cuatro vistas son las del fichero.
    assert!(resumen.informes.iter().all(|(_, r)| r.is_empty()));
    // El byte 24 del PNG de 16 bits es la profundidad.
    assert_eq!(png_de_16_bits()[24], 16);
    assert_eq!(pngs_de_ejemplo()[0].1[24], 8);
}

#[test]
fn fila_14_lista_vacia_da_el_mensaje_del_corpus_vacio() {
    assert_eq!(verificar_corpus(&[]).err().as_deref(), Some(MENSAJE_VACIO));
    // Solo el README: tampoco hay ningún PNG.
    let solo_readme = [fichero("README.md", "# golden/png\n")];
    assert_eq!(
        verificar_corpus(&solo_readme).err().as_deref(),
        Some(MENSAJE_VACIO)
    );
}

#[test]
fn fila_14_png_sin_sha256() {
    let mut corpus = corpus_valido();
    corpus.retain(|f| f.nombre != "rgba.png.sha256");
    assert_err_con(
        verificar_corpus(&corpus),
        &["rgba.png", "falta rgba.png.sha256"],
    );
}

#[test]
fn fila_14_sha256_sin_png() {
    let mut corpus = corpus_valido();
    corpus.push(fichero(
        "huerfano.png.sha256",
        format!("{}\n", hash_de_gris()),
    ));
    let err = verificar_corpus(&corpus).err().expect("debía ser Err");
    assert!(err.contains("huerfano.png.sha256"), "{err}");
    assert!(err.contains("no hay huerfano.png"), "{err}");
    assert!(!err.contains(MENSAJE_VACIO), "{err}");

    // Un PNG al que se le quita su .png pero se deja el .sha256.
    let mut sin_png = corpus_valido();
    sin_png.retain(|f| f.nombre != "indexado.png");
    assert_err_con(
        verificar_corpus(&sin_png),
        &["indexado.png.sha256", "no hay indexado.png"],
    );
}

#[test]
fn fila_14_fichero_ajeno() {
    for ajeno in [
        "notas.txt",
        "gris.sha256",
        "gris.png.sha256.bak",
        "subcarpeta/",
    ] {
        let mut corpus = corpus_valido();
        corpus.push(fichero(ajeno, "x"));
        assert_err_con(verificar_corpus(&corpus), &[ajeno, "ajeno"]);
    }
}

#[test]
fn fila_14_hash_mal_formado() {
    let bueno = hash_de_gris();
    let otro = sha256_hex(b"otro");
    let casos: Vec<(&str, Vec<u8>)> = vec![
        ("63 dígitos", bueno.as_bytes()[..63].to_vec()),
        ("65 dígitos", format!("{bueno}0").into_bytes()),
        (
            "64 caracteres no hex",
            format!("{}g", &bueno[..63]).into_bytes(),
        ),
        (
            "un nombre detrás del hash",
            format!("{bueno}  gris.png\n").into_bytes(),
        ),
        (
            "otro hash en la segunda línea",
            format!("{bueno}\n{otro}\n").into_bytes(),
        ),
        (
            "dos hashes en la misma línea",
            format!("{bueno} {otro}").into_bytes(),
        ),
        ("vacío", Vec::new()),
        ("solo espacios", b"  \n\r\n".to_vec()),
        ("no UTF-8", vec![0xFF; 64]),
    ];
    for (nombre, contenido) in casos {
        let r = verificar_corpus(&con_contenido("gris.png.sha256", contenido));
        match r {
            Ok(_) => panic!("{nombre}: se esperaba Err y fue Ok"),
            Err(msg) => {
                assert!(msg.contains("gris.png.sha256"), "{nombre}: {msg}");
                assert!(msg.contains("hash mal formado"), "{nombre}: {msg}");
            }
        }
    }
}

#[test]
fn fila_14_el_sha256_se_lee_recortando_extremos_y_con_mayusculas_o_minusculas() {
    let bueno = hash_de_gris();
    let validos: Vec<(&str, String)> = vec![
        ("sin salto de línea", bueno.clone()),
        ("con LF", format!("{bueno}\n")),
        ("con CRLF", format!("{bueno}\r\n")),
        (
            "con espacios y saltos a ambos lados",
            format!(" \n\t{bueno}  \r\n\n"),
        ),
        ("en mayúsculas", bueno.to_ascii_uppercase()),
        (
            "con mayúsculas y minúsculas mezcladas",
            format!("{}{}", bueno[..32].to_ascii_uppercase(), &bueno[32..]),
        ),
    ];
    for (nombre, contenido) in validos {
        let corpus = con_contenido("gris.png.sha256", contenido.into_bytes());
        if let Err(e) = verificar_corpus(&corpus) {
            panic!("{nombre}: se esperaba Ok y fue Err:\n{e}");
        }
    }
}

#[test]
fn fila_14_hash_con_un_digito_cambiado_nombra_el_fichero_y_los_dos_hashes() {
    let bueno = hash_de_gris();
    let ultimo = bueno.chars().last().unwrap();
    let cambiado_ultimo = if ultimo == '0' { '1' } else { '0' };
    let cambiado = format!("{}{cambiado_ultimo}", &bueno[..63]);
    assert_ne!(cambiado, bueno);
    let corpus = con_contenido("gris.png.sha256", format!("{cambiado}\n").into_bytes());
    // El esperado es el del .sha256 (cambiado) y el calculado es el del PNG (bueno).
    assert_err_con(
        verificar_corpus(&corpus),
        &[
            "gris.png",
            "no coincide",
            &format!("esperado {cambiado}"),
            &format!("calculado {bueno}"),
        ],
    );
}

#[test]
fn fila_14_un_pixel_alterado_cambia_el_hash() {
    // Mismo hash que el de `gris.png`, pero con el primer píxel alterado en el PNG.
    let alterado = Png::new(2, 1, 8, 0).muestras(&[11, 200]).bytes();
    let corpus = con_contenido("gris.png", alterado);
    assert_err_con(verificar_corpus(&corpus), &["gris.png", "no coincide"]);
}

#[test]
fn fila_14_png_ilegible_nombra_el_fichero() {
    // 8 bits con el hash correcto, pero el PNG truncado: `read_png` da Err.
    let png = pngs_de_ejemplo()[0].1.clone();
    let corpus = con_contenido("gris.png", png[..png.len() - 1].to_vec());
    assert_err_con(verificar_corpus(&corpus), &["gris.png", "read_png"]);

    // Lo mismo con un fichero de 16 bits truncado: se lee aunque no se hashee.
    let p16 = png_de_16_bits();
    let corpus = [
        fichero("profundo.png", p16[..p16.len() - 1].to_vec()),
        fichero("profundo.png.sha256", "x"),
    ];
    assert_err_con(verificar_corpus(&corpus), &["profundo.png", "read_png"]);
}

#[test]
fn fila_14_solo_pngs_de_16_bits_no_deja_ninguno_comprobado() {
    let corpus = [fichero("profundo.png", png_de_16_bits())];
    let err = verificar_corpus(&corpus).err().expect("debía ser Err");
    assert!(err.contains("ningún PNG comprobado"), "{err}");
    assert!(!err.contains(MENSAJE_VACIO), "{err}");

    // Aunque tenga su .sha256 al lado: sigue sin comprobarse nada.
    let con_hash = [
        fichero("profundo.png", png_de_16_bits()),
        fichero("profundo.png.sha256", format!("{}\n", hash_de_gris())),
    ];
    assert_err_con(verificar_corpus(&con_hash), &["ningún PNG comprobado"]);
}

#[test]
fn fila_14_png_de_16_bits_sin_sha256_junto_a_uno_valido_es_ok_y_se_lista_como_omitido() {
    let mut corpus = corpus_valido();
    corpus.push(fichero("profundo.png", png_de_16_bits()));
    let resumen = verificar_corpus(&corpus).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(resumen.omitidos, ["profundo.png"]);
    assert_eq!(resumen.comprobados.len(), 4);
    assert!(!resumen.comprobados.iter().any(|n| n == "profundo.png"));
    // Se lee, y el informe trae la reducción de 16 a 8 bits.
    let informe = resumen
        .informes
        .iter()
        .find(|(n, _)| n == "profundo.png")
        .map(|(_, r)| r.clone());
    assert_eq!(informe, Some(vec![Conversion::Depth16Reduced]));
}

#[test]
fn fila_14_todos_los_problemas_salen_juntos() {
    let mut corpus = corpus_valido();
    corpus.retain(|f| f.nombre != "rgba.png.sha256");
    corpus.push(fichero("notas.txt", "x"));
    corpus.push(fichero("huerfano.png.sha256", "x"));
    let err = verificar_corpus(&corpus).err().expect("debía ser Err");
    assert!(err.contains("falta rgba.png.sha256"), "{err}");
    assert!(err.contains("notas.txt"), "{err}");
    assert!(err.contains("huerfano.png.sha256"), "{err}");
    assert_eq!(err.lines().count(), 3, "{err}");
}

// ---------------------------------------------------------------------------------------
// Fila 15: el SHA-256 propio.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_15_sha256_con_los_vectores_del_estandar_y_las_fronteras_del_relleno() {
    let mut fallos = Vec::new();
    for (nombre, mensaje, esperado) in vectores() {
        let obtenido = sha256_hex(&mensaje);
        if obtenido != esperado {
            fallos.push(format!(
                "{nombre} ({} bytes): esperado {esperado}, obtenido {obtenido}",
                mensaje.len()
            ));
        }
    }
    assert!(fallos.is_empty(), "\n{}", fallos.join("\n"));
    // Los diez vectores del encargo: 4 del estándar y 6 longitudes de `a`.
    assert_eq!(vectores().len(), 10);
}

// ---------------------------------------------------------------------------------------
// Fila 16: estructura del repositorio.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_16_gitattributes_cubre_golden_con_binary_y_el_readme_va_despues() {
    let ruta = raiz_del_repo().join(".gitattributes");
    let texto = fs::read_to_string(&ruta)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
    // Líneas con regla: patrón y atributos (sin comentarios ni líneas en blanco).
    let reglas: Vec<(&str, Vec<&str>)> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut partes = l.split_whitespace();
            (partes.next().unwrap(), partes.collect())
        })
        .collect();

    let cubre_golden =
        |patron: &str| ["golden/**", "golden/", "/golden/", "/golden/**"].contains(&patron);
    let con_binary = reglas
        .iter()
        .position(|(p, atributos)| cubre_golden(p) && atributos.contains(&"binary"));
    let para_md = reglas.iter().position(|(p, _)| *p == "golden/**/*.md");

    let con_binary = con_binary.unwrap_or_else(|| {
        panic!(".gitattributes: ninguna línea cubre `golden/` con `binary`:\n{texto}")
    });
    let para_md = para_md
        .unwrap_or_else(|| panic!(".gitattributes: falta la línea de `golden/**/*.md`:\n{texto}"));
    assert!(
        para_md > con_binary,
        ".gitattributes: la línea de `golden/**/*.md` (regla {para_md}) debe ir después de la \
         de `binary` (regla {con_binary}), porque gana la última que casa:\n{texto}"
    );
}

#[test]
fn fila_16_el_readme_nombra_cada_png_y_dice_mit() {
    let dir = carpeta_golden_png();
    let pngs: Vec<String> = nombres_en(&dir)
        .into_iter()
        .filter(|n| n.ends_with(".png"))
        .collect();
    assert!(
        !pngs.is_empty(),
        "{} no contiene ningún PNG: el README no puede comprobarse en vacío",
        dir.display()
    );
    let ruta = dir.join("README.md");
    let readme = fs::read_to_string(&ruta)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
    let sin_nombrar: Vec<&String> = pngs
        .iter()
        .filter(|n| !readme.contains(n.as_str()))
        .collect();
    assert!(
        sin_nombrar.is_empty(),
        "el README no nombra {sin_nombrar:?}"
    );
    assert!(
        readme.contains("MIT"),
        "el README no menciona la licencia MIT"
    );
}
