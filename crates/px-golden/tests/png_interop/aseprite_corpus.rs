//! Corpus `golden/aseprite/` (specs/corpus-hito-3.md, «Prueba del corpus»): cada `X.aseprite`
//! tiene su `X.png` aplanado y su `X.png.sha256`, con el mismo criterio que `golden/png/` (SHA-256
//! de la RGBA de 8 bits expandida, calculado con otra herramienta).
//!
//! Aquí no se lee el formato `.aseprite` (eso es la unidad 3.8): solo se comprueba de cada fichero
//! la cabecera de 128 bytes de la especificación pública (tamaño, magia `0xA5E0`, un fotograma,
//! dimensiones y profundidad de color) y que sus dimensiones sean las del PNG.
//!
//! La prueba de la carpeta real es **siempre estricta**: falla si `golden/aseprite/` no existe,
//! está vacía o no tiene exactamente los 29 casos de `CASES` (los de la tabla de la spec).

use px_formats::read_png;

use super::common::sha256::sha256_hex;
use super::{File, bytes_rgba, file, hash_well_formed, reads_folder, root_of_the_repo};

/// Tope de casos de la spec (25, los tres de la ampliación de 3.2 y `layer-locked` de 3.8).
const MAX_CASES: usize = 29;

/// Los 29 casos de la tabla de specs/corpus-hito-3.md (misma lista que `tools/corpus-h3/generate.sh`).
pub(super) const CASES: [&str; MAX_CASES] = [
    "rgba-one-layer",
    "layers-normal-three",
    "layers-opacity",
    "layers-alpha-source",
    "layers-alpha-background",
    "blend-normal",
    "blend-multiply",
    "blend-pattern",
    "blend-overlay",
    "blend-darken",
    "blend-lighten",
    "blend-add",
    "blend-difference",
    "blend-eight-opacity",
    "blend-eight-alpha",
    "blend-eight-background-translucent",
    "indexed-layers",
    "indexed-transparent-7",
    "indexed-palette-256",
    "indexed-cels",
    "cel-shifted",
    "cel-clipped",
    "cel-empty",
    "layers-hidden",
    "mixed-rgba",
    "blend-alpha-grid-multiply",
    "blend-alpha-grid-overlay",
    "background-with-opacity",
    "layer-locked",
];

/// Falta o sobra algún caso respecto de `expected_2` (`cases` ordenados).
pub(super) fn compare_with_list(cases: &[String], expected_2: &[&str]) -> Result<(), String> {
    let missing: Vec<&str> = expected_2
        .iter()
        .copied()
        .filter(|e| !cases.iter().any(|c| c == e))
        .collect();
    let are_left: Vec<&String> = cases
        .iter()
        .filter(|c| !expected_2.contains(&c.as_str()))
        .collect();
    if missing.is_empty() && are_left.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "faltan los casos {missing:?} y sobran {are_left:?}"
        ))
    }
}

/// `a-z`, `0-9` y guiones, sin guiones al principio, al final ni dobles.
fn name_valid(name_2: &str) -> bool {
    !name_2.is_empty()
        && name_2.split('-').all(|t| {
            !t.is_empty()
                && t.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn u16_le(b: &[u8], pos: usize) -> usize {
    usize::from(u16::from_le_bytes([b[pos], b[pos + 1]]))
}

/// Comprueba la cabecera de un `.aseprite` y devuelve (ancho, alto).
fn header(bytes: &[u8]) -> Result<(usize, usize), String> {
    if bytes.len() < 128 {
        return Err(format!(
            "solo tiene {} bytes (la cabecera es de 128)",
            bytes.len()
        ));
    }
    let size = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if size != bytes.len() {
        return Err(format!(
            "la cabecera dice {size} bytes y el fichero tiene {}",
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
    let (width, height) = (u16_le(bytes, 8), u16_le(bytes, 10));
    if !(16..=64).contains(&width) || !(16..=64).contains(&height) {
        return Err(format!("tamaño {width}x{height} fuera de 16x16 a 64x64"));
    }
    match u16_le(bytes, 12) {
        32 | 8 => Ok((width, height)),
        other => Err(format!(
            "profundidad de color {other}: solo RGBA (32) o indexado (8)"
        )),
    }
}

/// Todos los problemas juntos, uno por línea; `Ok` con los nombres de los casos comprobados.
pub(super) fn verify(files: &[File]) -> Result<Vec<String>, String> {
    let search = |name_2: &str| files.iter().find(|f| f.name_2 == name_2);
    let mut problems = Vec::new();
    let mut cases = Vec::new();

    if search("README.md").is_none() {
        problems.push("falta README.md (versión de Aseprite, fecha y origen)".to_string());
    }
    for f in files {
        let n = f.name_2.as_str();
        if n == "README.md" {
            continue;
        }
        if let Some(case) = n.strip_suffix(".aseprite") {
            cases.push(case.to_string());
        } else if let Some(case) = n.strip_suffix(".png.sha256") {
            if search(&format!("{case}.png")).is_none() {
                problems.push(format!("{n}: no hay {case}.png al lado"));
            }
        } else if let Some(case) = n.strip_suffix(".png") {
            if search(&format!("{case}.aseprite")).is_none() {
                problems.push(format!("{n}: no hay {case}.aseprite al lado"));
            }
        } else {
            problems.push(format!(
                "{n}: fichero ajeno (solo README.md, *.aseprite, *.png y *.png.sha256)"
            ));
        }
    }

    if cases.is_empty() {
        problems.insert(
            0,
            "golden/aseprite no contiene ningún .aseprite: el corpus no puede estar vacío"
                .to_string(),
        );
    }
    if cases.len() > MAX_CASES {
        problems.push(format!(
            "hay {} casos y la spec admite {MAX_CASES} como máximo",
            cases.len()
        ));
    }

    for case in &cases {
        if !name_valid(case) {
            problems.push(format!(
                "{case}.aseprite: el nombre debe ser minúsculas, dígitos y guiones"
            ));
        }
        let dims = match header(&search(&format!("{case}.aseprite")).unwrap().bytes) {
            Ok(d) => Some(d),
            Err(e) => {
                problems.push(format!("{case}.aseprite: {e}"));
                None
            }
        };
        let Some(png) = search(&format!("{case}.png")) else {
            problems.push(format!("{case}.aseprite: falta {case}.png"));
            continue;
        };
        let Some(hash) = search(&format!("{case}.png.sha256")) else {
            problems.push(format!("{case}.png: falta {case}.png.sha256"));
            continue;
        };
        let expected = match hash_well_formed(&hash.bytes) {
            Ok(h) => h,
            Err(e) => {
                problems.push(format!("{case}.png.sha256: hash mal formado: {e}"));
                continue;
            }
        };
        let read = match read_png(&png.bytes) {
            Ok(r) => r,
            Err(e) => {
                problems.push(format!("{case}.png: read_png dio error: {e}"));
                continue;
            }
        };
        let (w, h) = (read.image.width() as usize, read.image.height() as usize);
        if let Some((aw, ah)) = dims
            && (w, h) != (aw, ah)
        {
            problems.push(format!(
                "{case}: el .aseprite es de {aw}x{ah} y el PNG de {w}x{h}"
            ));
        }
        let computed = sha256_hex(&bytes_rgba(&read.image));
        if computed != expected {
            problems.push(format!(
                "{case}.png: el hash no coincide: esperado {expected}, calculado {computed}"
            ));
        }
    }

    if problems.is_empty() {
        cases.sort();
        Ok(cases)
    } else {
        Err(problems.join("\n"))
    }
}

// ---------------------------------------------------------------------------------------
// Filas 1 y 2: el verificador con ficheros en memoria.
// ---------------------------------------------------------------------------------------

/// Cabecera mínima válida de 128 bytes: 1 fotograma, `width` x `height`, color de 32 bits.
fn aseprite(width: u16, height: u16) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[0..4].copy_from_slice(&128u32.to_le_bytes());
    b[4..6].copy_from_slice(&0xA5E0u16.to_le_bytes());
    b[6..8].copy_from_slice(&1u16.to_le_bytes());
    b[8..10].copy_from_slice(&width.to_le_bytes());
    b[10..12].copy_from_slice(&height.to_le_bytes());
    b[12..14].copy_from_slice(&32u16.to_le_bytes());
    b
}

/// PNG RGBA de 8 bits, liso, y su SHA-256 como lo escribiría el generador.
fn png_smooth(width: u32, height: u32, valor: u8) -> (Vec<u8>, String) {
    let raw = vec![valor; (width * height * 4) as usize];
    let swatches: Vec<u16> = raw.iter().map(|&b| u16::from(b)).collect();
    let png = super::common::Png::new(width, height, 8, 6)
        .swatches(&swatches)
        .bytes();
    (png, sha256_hex(&raw))
}

/// Un caso correcto: `.aseprite`, `.png` y `.png.sha256`, más el README.
fn corpus_good(name_2: &str) -> Vec<File> {
    let (png, hash) = png_smooth(16, 16, 200);
    vec![
        file("README.md", "# golden/aseprite"),
        file(&format!("{name_2}.aseprite"), aseprite(16, 16)),
        file(&format!("{name_2}.png"), png),
        file(&format!("{name_2}.png.sha256"), format!("{hash}\n")),
    ]
}

fn without(mut v: Vec<File>, name_2: &str) -> Vec<File> {
    v.retain(|f| f.name_2 != name_2);
    v
}

#[test]
fn aseprite_row_1_a_correct_corpus_passes() {
    assert_eq!(
        verify(&corpus_good("rgba-one-layer")),
        Ok(vec!["rgba-one-layer".to_string()])
    );
}

#[test]
fn aseprite_row_2_each_defect_gives_an_error_with_its_message() {
    let (png_other, hash_other) = png_smooth(16, 16, 7);
    let mut hash_bad = corpus_good("a");
    hash_bad[3] = file("a.png.sha256", hash_other);
    let mut magic = corpus_good("a");
    magic[1].bytes[4] = 0;
    let mut size = corpus_good("a");
    size[1].bytes.push(0);
    let mut dimensions = corpus_good("a");
    dimensions[1] = file("a.aseprite", aseprite(32, 16));
    let mut out_of_range = corpus_good("a");
    out_of_range[1] = file("a.aseprite", aseprite(8, 8));
    let mut two_frames = corpus_good("a");
    two_frames[1].bytes[6] = 2;
    let mut foreign = corpus_good("a");
    foreign.push(file("notas.txt", "x"));
    let capitals = corpus_good("Capa-Uno");
    let mut other_png = corpus_good("a");
    other_png[2] = file("a.png", png_other);
    let mut too_many = corpus_good("caso-0");
    for i in 1..=MAX_CASES {
        too_many.extend(corpus_good(&format!("caso-{i}")).into_iter().skip(1));
    }

    let cases: Vec<(&str, Vec<File>, &str)> = vec![
        ("vacía", Vec::new(), "no puede estar vacío"),
        (
            "sin README",
            without(corpus_good("a"), "README.md"),
            "falta README.md",
        ),
        ("sin PNG", without(corpus_good("a"), "a.png"), "falta a.png"),
        (
            "sin .sha256",
            without(corpus_good("a"), "a.png.sha256"),
            "falta a.png.sha256",
        ),
        (
            "PNG sin .aseprite",
            without(corpus_good("a"), "a.aseprite"),
            "no hay a.aseprite",
        ),
        ("hash de otra imagen", hash_bad, "el hash no coincide"),
        ("magia", magic, "magia"),
        ("tamaño de cabecera", size, "la cabecera dice"),
        ("dimensiones distintas", dimensions, "el PNG de 16x16"),
        ("fuera de 16 a 64", out_of_range, "fuera de 16x16 a 64x64"),
        ("dos fotogramas", two_frames, "un fotograma"),
        ("fichero ajeno", foreign, "fichero ajeno"),
        (
            "nombre con mayúsculas",
            capitals,
            "minúsculas, dígitos y guiones",
        ),
        ("PNG de otra imagen", other_png, "el hash no coincide"),
        ("más de 29", too_many, "como máximo"),
    ];
    for (description, files, fragment) in cases {
        match verify(&files) {
            Ok(_) => panic!("{description}: debía dar error"),
            Err(e) => assert!(
                e.contains(fragment),
                "{description}: {e:?} no contiene {fragment:?}"
            ),
        }
    }
}

#[test]
fn aseprite_row_3_each_golden_aseprite_has_its_png_and_its_sha256() {
    let dir = root_of_the_repo().join("golden/aseprite");
    let files = reads_folder(&dir); // una carpeta que no existe es una vacía: `verify` falla
    match verify(&files) {
        Ok(cases) => {
            if let Err(e) = compare_with_list(&cases, &CASES) {
                panic!("golden/aseprite no tiene los 29 casos de la spec: {e}");
            }
            println!("golden/aseprite: {} casos comprobados", cases.len());
        }
        Err(e) => panic!("corpus de golden/aseprite con problemas:\n{e}"),
    }
}

#[test]
fn aseprite_row_4_the_exact_list_detects_a_missing_or_extra_case() {
    let all: Vec<String> = CASES.iter().map(|c| c.to_string()).collect();
    assert_eq!(compare_with_list(&all, &CASES), Ok(()));
    let without_one = &all[1..];
    let e = compare_with_list(without_one, &CASES).unwrap_err();
    assert!(e.contains("rgba-one-layer"), "{e}");
    let mut with_one_more = all.clone();
    with_one_more.push("extra".to_string());
    let e = compare_with_list(&with_one_more, &CASES).unwrap_err();
    assert!(e.contains("extra"), "{e}");
}
