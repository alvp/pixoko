//! Test de estructura del workspace (specs/hito-0-workspace.md, seccion "Tests").
//!
//! Solo usa `std`. Los `Cargo.toml` se leen a mano, linea a linea: no hace
//! falta un parser de TOML completo para las pocas claves que se comprueban.

use std::fs;
use std::path::{Path, PathBuf};

/// Los 8 crates del hito 0, en orden alfabetico.
const EXPECTED_CRATES: [&str; 8] = [
    "px-app",
    "px-cli",
    "px-core",
    "px-formats",
    "px-golden",
    "px-raster",
    "px-render",
    "px-ui",
];

/// Crate con lints propios (`unsafe_code = "deny"`) en lugar de los del workspace.
const OWN_LINTS_CRATE: &str = "px-render";

/// Segmentos de ruta que indican una tabla de dependencias.
const DEPENDENCY_TABLES: [&str; 6] = [
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
    "dev_dependencies",
    "build_dependencies",
    "workspace-dependencies",
];

/// Raiz del workspace: dos niveles por encima de `crates/px-golden`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crates_dir() -> PathBuf {
    workspace_root().join("crates")
}

/// Nombres de los directorios que hay en `crates/`, ordenados.
fn crate_dirs() -> Vec<String> {
    let dir = crates_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("could not read the directory {}: {e}", dir.display()))
        .map(|entry| entry.expect("entrada de directorio ilegible"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Quita el comentario `# ...` de una linea respetando las comillas.
fn strip_comment(line: &str) -> &str {
    let mut in_string: Option<char> = None;
    for (i, c) in line.char_indices() {
        match in_string {
            Some(quote) if c == quote => in_string = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => in_string = Some(c),
            None if c == '#' => return &line[..i],
            None => {}
        }
    }
    line
}

/// Cargo.toml reducido a pares (ruta completa con puntos, valor).
///
/// Ejemplos: `[package]` + `license.workspace = true` da
/// `("package.license.workspace", "true")`; `[lints]` + `workspace = true`
/// da `("lints.workspace", "true")`. La forma `license = { workspace = true }`
/// se normaliza a la misma ruta que la forma con punto.
struct Manifest {
    entries: Vec<(String, String)>,
}

impl Manifest {
    fn parse(text: &str) -> Self {
        let mut section = String::new();
        let mut entries = Vec::new();
        for raw in text.lines() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') {
                section = line
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue; // continuacion de un valor multilinea
            };
            let key = key.trim();
            let value = value.trim();
            let path = if section.is_empty() {
                key.to_string()
            } else {
                format!("{section}.{key}")
            };
            let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
            if compact == "{workspace=true}" {
                entries.push((format!("{path}.workspace"), "true".to_string()));
            } else {
                entries.push((path, value.to_string()));
            }
        }
        Manifest { entries }
    }

    fn get(&self, path: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, v)| v.as_str())
    }

    fn is_true(&self, path: &str) -> bool {
        self.get(path) == Some("true")
    }

    /// Rutas que cuelgan de una tabla de dependencias (es decir, con entradas).
    fn dependency_entries(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|(p, _)| p.as_str())
            .filter(|p| p.split('.').any(|seg| DEPENDENCY_TABLES.contains(&seg)))
            .collect()
    }
}

fn read_manifest(crate_name: &str) -> Manifest {
    let path = crates_dir().join(crate_name).join("Cargo.toml");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    Manifest::parse(&text)
}

fn assert_no_failures(failures: &[String]) {
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn crates_dir_has_exactly_the_eight_crates() {
    let found = crate_dirs();
    let missing: Vec<&str> = EXPECTED_CRATES
        .iter()
        .copied()
        .filter(|name| !found.iter().any(|f| f == name))
        .collect();
    let extra: Vec<&String> = found
        .iter()
        .filter(|f| !EXPECTED_CRATES.contains(&f.as_str()))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "crates/ debe contener exactamente {EXPECTED_CRATES:?}; faltan {missing:?}, sobran {extra:?}"
    );
}

#[test]
fn every_crate_inherits_license_and_lints_from_workspace() {
    let mut failures = Vec::new();
    for name in EXPECTED_CRATES {
        let manifest = read_manifest(name);
        if !manifest.is_true("package.license.workspace") {
            failures.push(format!(
                "{name}: falta `license.workspace = true` en [package]"
            ));
        }
        if name != OWN_LINTS_CRATE && !manifest.is_true("lints.workspace") {
            failures.push(format!(
                "{name}: falta `lints.workspace = true` (seccion [lints])"
            ));
        }
    }
    assert_no_failures(&failures);
}

#[test]
fn px_render_declares_its_own_lints_with_unsafe_deny() {
    let manifest = read_manifest(OWN_LINTS_CRATE);
    let mut failures = Vec::new();
    if manifest.get("lints.rust.unsafe_code") != Some("\"deny\"") {
        failures.push(format!(
            "{OWN_LINTS_CRATE}: debe declarar `unsafe_code = \"deny\"` en [lints.rust]"
        ));
    }
    if manifest.get("lints.workspace").is_some() {
        failures.push(format!(
            "{OWN_LINTS_CRATE}: no debe usar `lints.workspace` (forbid impediria permitir unsafe)"
        ));
    }
    assert_no_failures(&failures);
}

/// Regla de `CLAUDE.md`: toda dependencia externa se anota en `docs/DEPENDENCIES.md`.
/// Las aristas entre crates `px-` no cuentan.
#[test]
fn every_external_dependency_is_documented() {
    let path = workspace_root().join("docs/DEPENDENCIES.md");
    let documented = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let mut failures = Vec::new();
    for name in EXPECTED_CRATES {
        let manifest = read_manifest(name);
        for entry in manifest.dependency_entries() {
            let mut segments = entry.split('.');
            let dep = segments
                .by_ref()
                .skip_while(|seg| !DEPENDENCY_TABLES.contains(seg))
                .nth(1);
            let Some(dep) = dep.filter(|d| !d.starts_with("px-")) else {
                continue;
            };
            if !documented.contains(&format!("`{dep}`")) {
                failures.push(format!(
                    "{name}: la dependencia `{dep}` no esta anotada en docs/DEPENDENCIES.md"
                ));
            }
        }
    }
    failures.dedup();
    assert_no_failures(&failures);
}
