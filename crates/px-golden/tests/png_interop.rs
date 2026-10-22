//! Interoperabilidad con los PNG de `golden/png/` (specs/hito-1-png-corpus.md, punto 4).
//!
//! Los PNG de la carpeta los guardaron Grafx2 y LibreSprite; cada `X.png` lleva al lado
//! `X.png.sha256`, el SHA-256 de la imagen expandida a RGBA de 8 bits (alfa directo, sin
//! cabecera ni dimensiones) que calculó una herramienta independiente. Aquí se lee cada PNG
//! con `read_png` y se compara el hash de su vista RGBA.
//!
//! Piezas:
//! - `verify_corpus`: función pura sobre una lista de ficheros en memoria. Tiene sus
//!   propios tests (fila 14) para demostrar que el corpus sabe fallar.
//! - El test de la carpeta real (fila 13) solo lee el directorio y llama al verificador.
//! - El SHA-256 propio (`common::sha256`) se valida con sus vectores en un único test
//!   (fila 15), aparte de la fila 13: si falla el hash se sabe si es el SHA-256 o el PNG.
//! - La fila 16 comprueba la estructura del repositorio (`.gitattributes` y el README).
//! - `aseprite_corpus` hace lo mismo con `golden/aseprite/` (specs/corpus-hito-3.md).
//! - `aseprite_gate` es la parte de `px-golden` de la puerta del hito 3 (specs/hito-3-puerta.md).
//! - `aseprite_read` lee esos `.aseprite` con `read_aseprite` (specs/hito-3-aseprite.md, filas 3 y 8).

#[path = "png_interop/aseprite_corpus.rs"]
mod aseprite_corpus;
#[path = "png_interop/aseprite_gate.rs"]
mod aseprite_gate;
#[path = "png_interop/aseprite_read.rs"]
mod aseprite_read;
mod common;
#[path = "png_interop/gate_oracle.rs"]
mod gate_oracle;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use common::sha256::{sha256_hex, vectors};
use common::{Png, plte, trns};
use px_core::{Image, Pixel};
use px_formats::{Conversion, read_png};

// ---------------------------------------------------------------------------------------
// El verificador del corpus.
// ---------------------------------------------------------------------------------------

/// Mensaje cuando no hay ningún PNG: el test falla, nunca pasa en vacío.
const MESSAGE_EMPTY: &str = "golden/png no contiene ningún PNG: el corpus de interoperabilidad no puede estar vacío (ver golden/png/README.md)";

/// Un fichero de la carpeta: nombre (sin ruta) y contenido.
#[derive(Clone)]
struct File {
    name_2: String,
    bytes: Vec<u8>,
}

fn file(name_2: &str, bytes: impl Into<Vec<u8>>) -> File {
    File {
        name_2: name_2.to_string(),
        bytes: bytes.into(),
    }
}

/// Lo que dejó una verificación correcta.
struct Summary {
    /// Informe de conversiones de cada PNG leído (`read_png` con `Ok`).
    reports: Vec<(String, Vec<Conversion>)>,
    /// PNG cuyo hash coincide con su `.sha256`.
    checked: Vec<String>,
    /// PNG de 16 bits: se leen, pero no se hashean.
    omitted: Vec<String>,
}

/// Vista RGBA de una imagen: `width × height × 4` bytes, fila a fila de arriba abajo y de
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
fn hash_well_formed(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "no es texto UTF-8".to_string())?;
    let hash = text.trim();
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
fn verify_corpus(files: &[File]) -> Result<Summary, String> {
    let search = |name_2: &str| files.iter().find(|f| f.name_2 == name_2);
    let pngs: Vec<&File> = files
        .iter()
        .filter(|f| f.name_2.ends_with(".png"))
        .collect();
    let mut problems = Vec::new();

    for f in files {
        if f.name_2 == "README.md" || f.name_2.ends_with(".png") {
            continue;
        }
        match f.name_2.strip_suffix(".sha256") {
            Some(png) if png.ends_with(".png") => {
                if search(png).is_none() {
                    problems.push(format!("{}: no hay {png} al lado", f.name_2));
                }
            }
            _ => problems.push(format!(
                "{}: fichero ajeno (solo se admiten README.md, *.png y *.png.sha256)",
                f.name_2
            )),
        }
    }

    let mut reports = Vec::new();
    let mut checked = Vec::new();
    let mut omitted = Vec::new();
    for png in &pngs {
        let name_2 = &png.name_2;
        // Profundidad de bits: byte 24 del fichero (8 de firma, 4 de longitud, 4 de tipo,
        // 4 de ancho y 4 de alto).
        let de_16_bits = png.bytes.get(24) == Some(&16);
        let read = match read_png(&png.bytes) {
            Ok(r) => r,
            Err(e) => {
                problems.push(format!("{name_2}: read_png dio error: {e}"));
                continue;
            }
        };
        // Se imprime al leer (no al final) para que se vea también si el corpus falla.
        println!("{name_2}: informe de conversiones {:?}", read.report);
        reports.push((name_2.clone(), read.report.clone()));

        if de_16_bits {
            if !read.report.contains(&Conversion::Depth16Reduced) {
                problems.push(format!(
                    "{name_2}: es de 16 bits y el informe no incluye Depth16Reduced: {:?}",
                    read.report
                ));
            }
            omitted.push(name_2.clone());
            continue;
        }

        let name_hash = format!("{name_2}.sha256");
        let Some(hash) = search(&name_hash) else {
            problems.push(format!("{name_2}: falta {name_hash}"));
            continue;
        };
        let expected = match hash_well_formed(&hash.bytes) {
            Ok(h) => h,
            Err(e) => {
                problems.push(format!("{name_hash}: hash mal formado: {e}"));
                continue;
            }
        };
        let computed = sha256_hex(&bytes_rgba(&read.image));
        if computed == expected {
            checked.push(name_2.clone());
        } else {
            problems.push(format!(
                "{name_2}: el hash no coincide: esperado {expected}, calculado {computed}"
            ));
        }
    }

    if pngs.is_empty() {
        problems.insert(0, MESSAGE_EMPTY.to_string());
    } else if checked.is_empty() && omitted.len() == pngs.len() {
        problems.push(
            "ningún PNG comprobado: todos los PNG de la carpeta son de 16 bits (omitidos del hash)"
                .to_string(),
        );
    }

    if problems.is_empty() {
        Ok(Summary {
            reports,
            checked,
            omitted,
        })
    } else {
        Err(problems.join("\n"))
    }
}

// ---------------------------------------------------------------------------------------
// Fila 13: la carpeta real.
// ---------------------------------------------------------------------------------------

fn root_of_the_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn folder_golden_png() -> PathBuf {
    root_of_the_repo().join("golden/png")
}

/// Nombres de los ficheros de una carpeta, ordenados. Una carpeta que no existe es una
/// carpeta vacía (el verificador la rechaza con el mensaje del corpus vacío).
fn names_in(dir: &Path) -> Vec<String> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(e) => panic!("no se pudo leer el directorio {}: {e}", dir.display()),
    };
    let mut names: Vec<String> = entries
        .map(|e| e.expect("entrada de directorio ilegible"))
        .map(|e| {
            let name_2 = e.file_name().to_string_lossy().into_owned();
            // Un subdirectorio se anota con barra final: no es README, ni PNG ni .sha256.
            if e.path().is_dir() {
                format!("{name_2}/")
            } else {
                name_2
            }
        })
        .collect();
    names.sort();
    names
}

/// Lee la carpeta entera como lista de ficheros en memoria.
fn reads_folder(dir: &Path) -> Vec<File> {
    names_in(dir)
        .into_iter()
        .map(|name_2| {
            let bytes = if name_2.ends_with('/') {
                Vec::new()
            } else {
                let path = dir.join(&name_2);
                fs::read(&path)
                    .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
            };
            File { name_2, bytes }
        })
        .collect()
}

#[test]
fn row_13_each_golden_png_has_the_hash_of_its_sha256() {
    let files = reads_folder(&folder_golden_png());
    match verify_corpus(&files) {
        Ok(summary) => {
            // Visible con `cargo test -- --nocapture`, para depurar.
            for name_2 in &summary.checked {
                println!("{name_2}: hash coincide");
            }
            for name_2 in &summary.omitted {
                println!("{name_2}: omitido del hash (16 bits)");
            }
        }
        Err(problems) => panic!("el corpus de golden/png no pasa:\n{problems}"),
    }
}

// ---------------------------------------------------------------------------------------
// Fila 14: el verificador con listas en memoria (el corpus sabe fallar).
// ---------------------------------------------------------------------------------------

/// Cuatro PNG de 8 bits de 2 × 1 (uno por modo) con la vista RGBA escrita a mano, sin
/// pasar por `bytes_rgba`: (nombre, PNG, bytes de la vista RGBA).
fn pngs_of_example() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
    vec![
        (
            "gris.png",
            Png::new(2, 1, 8, 0).swatches(&[10, 200]).bytes(),
            vec![10, 10, 10, 255, 200, 200, 200, 255],
        ),
        (
            "gris-alfa.png",
            Png::new(2, 1, 8, 4).swatches(&[77, 0, 200, 255]).bytes(),
            vec![77, 77, 77, 0, 200, 200, 200, 255],
        ),
        (
            "rgba.png",
            Png::new(2, 1, 8, 6)
                .swatches(&[10, 20, 30, 0, 40, 50, 60, 255])
                .bytes(),
            vec![10, 20, 30, 0, 40, 50, 60, 255],
        ),
        (
            // Paleta de 2 entradas con tRNS: los píxeles son los índices 1 y 0.
            "indexado.png",
            Png::new(2, 1, 8, 3)
                .swatches(&[1, 0])
                .antes(plte(&[(1, 2, 3), (4, 5, 6)]))
                .antes(trns(&[0, 128]))
                .bytes(),
            vec![4, 5, 6, 128, 1, 2, 3, 0],
        ),
    ]
}

/// Un corpus válido: los PNG de ejemplo, cada uno con su `.sha256` correcto.
fn corpus_valid() -> Vec<File> {
    let mut v = vec![file("README.md", "# golden/png\n")];
    for (name_2, png, vista) in pngs_of_example() {
        v.push(file(
            &format!("{name_2}.sha256"),
            format!("{}\n", sha256_hex(&vista)),
        ));
        v.push(file(name_2, png));
    }
    v
}

/// Hash correcto de `gray.png`.
fn hash_of_gray() -> String {
    sha256_hex(&[10, 10, 10, 255, 200, 200, 200, 255])
}

/// El corpus válido con el contenido de un fichero cambiado.
fn with_content(name_2: &str, bytes: impl Into<Vec<u8>>) -> Vec<File> {
    let bytes = bytes.into();
    let mut v = corpus_valid();
    v.iter_mut().find(|f| f.name_2 == name_2).unwrap().bytes = bytes;
    v
}

/// PNG de 16 bits (gris, 2 × 1) y su nombre.
fn png_de_16_bits() -> Vec<u8> {
    Png::new(2, 1, 16, 0).swatches(&[0x0000, 0xFFFF]).bytes()
}

/// Error esperado: el texto debe contener todos los fragmentos.
fn assert_err_with(r: Result<Summary, String>, fragments: &[&str]) {
    match r {
        Ok(_) => panic!("se esperaba Err con {fragments:?} y fue Ok"),
        Err(msg) => {
            for f in fragments {
                assert!(msg.contains(f), "falta {f:?} en el mensaje:\n{msg}");
            }
        }
    }
}

#[test]
fn row_14_helper_the_example_pngs_are_valid_and_the_16_bit_one_is_detected() {
    let corpus = corpus_valid();
    let summary = verify_corpus(&corpus).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(summary.checked.len(), 4);
    assert!(summary.omitted.is_empty());
    // Ninguno de los de ejemplo tiene conversiones: las cuatro vistas son las del fichero.
    assert!(summary.reports.iter().all(|(_, r)| r.is_empty()));
    // El byte 24 del PNG de 16 bits es la profundidad.
    assert_eq!(png_de_16_bits()[24], 16);
    assert_eq!(pngs_of_example()[0].1[24], 8);
}

#[test]
fn row_14_empty_list_gives_the_empty_corpus_message() {
    assert_eq!(verify_corpus(&[]).err().as_deref(), Some(MESSAGE_EMPTY));
    // Solo el README: tampoco hay ningún PNG.
    let only_readme = [file("README.md", "# golden/png\n")];
    assert_eq!(
        verify_corpus(&only_readme).err().as_deref(),
        Some(MESSAGE_EMPTY)
    );
}

#[test]
fn row_14_png_without_sha256() {
    let mut corpus = corpus_valid();
    corpus.retain(|f| f.name_2 != "rgba.png.sha256");
    assert_err_with(
        verify_corpus(&corpus),
        &["rgba.png", "falta rgba.png.sha256"],
    );
}

#[test]
fn row_14_sha256_without_png() {
    let mut corpus = corpus_valid();
    corpus.push(file("huerfano.png.sha256", format!("{}\n", hash_of_gray())));
    let err = verify_corpus(&corpus).err().expect("debía ser Err");
    assert!(err.contains("huerfano.png.sha256"), "{err}");
    assert!(err.contains("no hay huerfano.png"), "{err}");
    assert!(!err.contains(MESSAGE_EMPTY), "{err}");

    // Un PNG al que se le quita su .png pero se deja el .sha256.
    let mut without_png = corpus_valid();
    without_png.retain(|f| f.name_2 != "indexado.png");
    assert_err_with(
        verify_corpus(&without_png),
        &["indexado.png.sha256", "no hay indexado.png"],
    );
}

#[test]
fn row_14_foreign_file() {
    for foreign in [
        "notas.txt",
        "gris.sha256",
        "gris.png.sha256.bak",
        "subcarpeta/",
    ] {
        let mut corpus = corpus_valid();
        corpus.push(file(foreign, "x"));
        assert_err_with(verify_corpus(&corpus), &[foreign, "ajeno"]);
    }
}

#[test]
fn row_14_malformed_hash() {
    let good = hash_of_gray();
    let other_2 = sha256_hex(b"otro");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("63 dígitos", good.as_bytes()[..63].to_vec()),
        ("65 dígitos", format!("{good}0").into_bytes()),
        (
            "64 caracteres no hex",
            format!("{}g", &good[..63]).into_bytes(),
        ),
        (
            "un nombre detrás del hash",
            format!("{good}  gris.png\n").into_bytes(),
        ),
        (
            "otro hash en la segunda línea",
            format!("{good}\n{other_2}\n").into_bytes(),
        ),
        (
            "dos hashes en la misma línea",
            format!("{good} {other_2}").into_bytes(),
        ),
        ("vacío", Vec::new()),
        ("solo espacios", b"  \n\r\n".to_vec()),
        ("no UTF-8", vec![0xFF; 64]),
    ];
    for (name_2, content) in cases {
        let r = verify_corpus(&with_content("gris.png.sha256", content));
        match r {
            Ok(_) => panic!("{name_2}: se esperaba Err y fue Ok"),
            Err(msg) => {
                assert!(msg.contains("gris.png.sha256"), "{name_2}: {msg}");
                assert!(msg.contains("hash mal formado"), "{name_2}: {msg}");
            }
        }
    }
}

#[test]
fn row_14_the_sha256_is_read_trimming_ends_and_with_upper_or_lower_case() {
    let good = hash_of_gray();
    let valid: Vec<(&str, String)> = vec![
        ("sin salto de línea", good.clone()),
        ("con LF", format!("{good}\n")),
        ("con CRLF", format!("{good}\r\n")),
        (
            "con espacios y saltos a ambos lados",
            format!(" \n\t{good}  \r\n\n"),
        ),
        ("en mayúsculas", good.to_ascii_uppercase()),
        (
            "con mayúsculas y minúsculas mezcladas",
            format!("{}{}", good[..32].to_ascii_uppercase(), &good[32..]),
        ),
    ];
    for (name_2, content) in valid {
        let corpus = with_content("gris.png.sha256", content.into_bytes());
        if let Err(e) = verify_corpus(&corpus) {
            panic!("{name_2}: se esperaba Ok y fue Err:\n{e}");
        }
    }
}

#[test]
fn row_14_hash_with_a_changed_digit_names_the_file_and_the_two_hashes() {
    let good = hash_of_gray();
    let ultimo = good.chars().last().unwrap();
    let changed_last = if ultimo == '0' { '1' } else { '0' };
    let changed = format!("{}{changed_last}", &good[..63]);
    assert_ne!(changed, good);
    let corpus = with_content("gris.png.sha256", format!("{changed}\n").into_bytes());
    // El esperado es el del .sha256 (cambiado) y el calculado es el del PNG (bueno).
    assert_err_with(
        verify_corpus(&corpus),
        &[
            "gris.png",
            "no coincide",
            &format!("esperado {changed}"),
            &format!("calculado {good}"),
        ],
    );
}

#[test]
fn row_14_an_altered_pixel_changes_the_hash() {
    // Mismo hash que el de `gray.png`, pero con el primer píxel alterado en el PNG.
    let altered = Png::new(2, 1, 8, 0).swatches(&[11, 200]).bytes();
    let corpus = with_content("gris.png", altered);
    assert_err_with(verify_corpus(&corpus), &["gris.png", "no coincide"]);
}

#[test]
fn row_14_unreadable_png_names_the_file() {
    // 8 bits con el hash correcto, pero el PNG truncado: `read_png` da Err.
    let png = pngs_of_example()[0].1.clone();
    let corpus = with_content("gris.png", png[..png.len() - 1].to_vec());
    assert_err_with(verify_corpus(&corpus), &["gris.png", "read_png"]);

    // Lo mismo con un fichero de 16 bits truncado: se lee aunque no se hashee.
    let p16 = png_de_16_bits();
    let corpus = [
        file("profundo.png", p16[..p16.len() - 1].to_vec()),
        file("profundo.png.sha256", "x"),
    ];
    assert_err_with(verify_corpus(&corpus), &["profundo.png", "read_png"]);
}

#[test]
fn row_14_only_16_bit_pngs_leaves_none_checked() {
    let corpus = [file("profundo.png", png_de_16_bits())];
    let err = verify_corpus(&corpus).err().expect("debía ser Err");
    assert!(err.contains("ningún PNG comprobado"), "{err}");
    assert!(!err.contains(MESSAGE_EMPTY), "{err}");

    // Aunque tenga su .sha256 al lado: sigue sin comprobarse nada.
    let with_hash = [
        file("profundo.png", png_de_16_bits()),
        file("profundo.png.sha256", format!("{}\n", hash_of_gray())),
    ];
    assert_err_with(verify_corpus(&with_hash), &["ningún PNG comprobado"]);
}

#[test]
fn row_14_16_bit_png_without_sha256_next_to_a_valid_one_is_ok_and_listed_as_skipped() {
    let mut corpus = corpus_valid();
    corpus.push(file("profundo.png", png_de_16_bits()));
    let summary = verify_corpus(&corpus).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(summary.omitted, ["profundo.png"]);
    assert_eq!(summary.checked.len(), 4);
    assert!(!summary.checked.iter().any(|n| n == "profundo.png"));
    // Se lee, y el informe trae la reducción de 16 a 8 bits.
    let report = summary
        .reports
        .iter()
        .find(|(n, _)| n == "profundo.png")
        .map(|(_, r)| r.clone());
    assert_eq!(report, Some(vec![Conversion::Depth16Reduced]));
}

#[test]
fn row_14_all_the_problems_come_out_together() {
    let mut corpus = corpus_valid();
    corpus.retain(|f| f.name_2 != "rgba.png.sha256");
    corpus.push(file("notas.txt", "x"));
    corpus.push(file("huerfano.png.sha256", "x"));
    let err = verify_corpus(&corpus).err().expect("debía ser Err");
    assert!(err.contains("falta rgba.png.sha256"), "{err}");
    assert!(err.contains("notas.txt"), "{err}");
    assert!(err.contains("huerfano.png.sha256"), "{err}");
    assert_eq!(err.lines().count(), 3, "{err}");
}

// ---------------------------------------------------------------------------------------
// Fila 15: el SHA-256 propio.
// ---------------------------------------------------------------------------------------

#[test]
fn row_15_sha256_with_the_standard_vectors_and_the_padding_boundaries() {
    let mut failures = Vec::new();
    for (name_2, message, expected) in vectors() {
        let obtained = sha256_hex(&message);
        if obtained != expected {
            failures.push(format!(
                "{name_2} ({} bytes): esperado {expected}, obtenido {obtained}",
                message.len()
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    // Los diez vectores del encargo: 4 del estándar y 6 longitudes de `a`.
    assert_eq!(vectors().len(), 10);
}

// ---------------------------------------------------------------------------------------
// Fila 16: estructura del repositorio.
// ---------------------------------------------------------------------------------------

#[test]
fn row_16_gitattributes_covers_golden_with_binary_and_the_readme_goes_after() {
    let path = root_of_the_repo().join(".gitattributes");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
    // Líneas con regla: patrón y atributos (sin comentarios ni líneas en blanco).
    let rules: Vec<(&str, Vec<&str>)> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut parts = l.split_whitespace();
            (parts.next().unwrap(), parts.collect())
        })
        .collect();

    let covers_golden =
        |patron: &str| ["golden/**", "golden/", "/golden/", "/golden/**"].contains(&patron);
    let with_binary = rules
        .iter()
        .position(|(p, attributes)| covers_golden(p) && attributes.contains(&"binary"));
    let para_md = rules.iter().position(|(p, _)| *p == "golden/**/*.md");

    let with_binary = with_binary.unwrap_or_else(|| {
        panic!(".gitattributes: ninguna línea cubre `golden/` con `binary`:\n{text}")
    });
    let para_md = para_md
        .unwrap_or_else(|| panic!(".gitattributes: falta la línea de `golden/**/*.md`:\n{text}"));
    assert!(
        para_md > with_binary,
        ".gitattributes: la línea de `golden/**/*.md` (regla {para_md}) debe ir después de la \
         de `binary` (regla {with_binary}), porque gana la última que casa:\n{text}"
    );
}

#[test]
fn row_16_the_readme_names_each_png_and_says_mit() {
    let dir = folder_golden_png();
    let pngs: Vec<String> = names_in(&dir)
        .into_iter()
        .filter(|n| n.ends_with(".png"))
        .collect();
    assert!(
        !pngs.is_empty(),
        "{} no contiene ningún PNG: el README no puede comprobarse en vacío",
        dir.display()
    );
    let path = dir.join("README.md");
    let readme = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
    let without_name: Vec<&String> = pngs
        .iter()
        .filter(|n| !readme.contains(n.as_str()))
        .collect();
    assert!(
        without_name.is_empty(),
        "el README no nombra {without_name:?}"
    );
    assert!(
        readme.contains("MIT"),
        "el README no menciona la licencia MIT"
    );
}
