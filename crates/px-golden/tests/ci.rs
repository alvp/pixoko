//! Test de estructura del workflow de CI (specs/hito-0-ci.md, secciones
//! "Comportamiento" y "Tests").
//!
//! Solo usa `std`. `.github/workflows/ci.yml` y el `Cargo.toml` raiz se leen
//! como texto, linea a linea: no hace falta un parser de YAML ni de TOML para
//! las pocas cosas que se comprueban. Es una red de seguridad contra ediciones
//! accidentales, no una validacion de la semantica de GitHub Actions.

use std::fs;
use std::path::{Path, PathBuf};

/// Sistemas operativos que debe cubrir la matriz.
const EXPECTED_OS: [&str; 3] = ["ubuntu-latest", "windows-latest", "macos-latest"];

/// Los cuatro comandos de comprobacion, en el orden del punto 6 de la spec.
const EXPECTED_COMMANDS: [&str; 4] = [
    "cargo fmt --all --check",
    "cargo clippy --workspace --all-targets --locked -- -D warnings",
    "cargo test --workspace --locked",
    "cargo deny --locked check",
];

/// Acciones de terceros permitidas en `uses:`.
const ALLOWED_ACTIONS: [&str; 2] = ["actions/checkout", "taiki-e/install-action"];

const WORKFLOW_PATH: &str = ".github/workflows/ci.yml";

/// Raiz del workspace: dos niveles por encima de `crates/px-golden`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Lee un archivo de texto de la raiz; si falla, el mensaje dice cual falta.
fn read_text(relative: &str) -> String {
    let path = workspace_root().join(relative);
    match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => panic!(
            "no se pudo leer {relative} ({}): {e}. Hay que crearlo segun specs/hito-0-ci.md",
            path.display()
        ),
    }
}

fn read_workflow() -> String {
    read_text(WORKFLOW_PATH)
}

/// Quita un comentario YAML (`# ...` al principio o tras un espacio).
fn strip_comment(line: &str) -> &str {
    if line.trim_start().starts_with('#') {
        return "";
    }
    match line.find(" #") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Lineas sin comentario ni espacios en los extremos, sin las vacias.
fn code_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| strip_comment(l).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Quita el guion de elemento de lista (`- `) del principio de una linea.
fn strip_list_dash(line: &str) -> &str {
    line.strip_prefix("- ").map_or(line, str::trim_start)
}

/// Comandos de todos los `run`, ya recortados.
///
/// Admite `run: comando` (con o sin `- ` delante) y bloques `run: |` o
/// `run: >`, cuyas lineas, mas indentadas que `run`, cuentan como comandos.
fn run_commands(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    // Indentacion de la clave `run` del bloque abierto, si hay uno.
    let mut block_indent: Option<usize> = None;
    for raw in text.lines() {
        let indent = raw.len() - raw.trim_start().len();
        let code = strip_comment(raw).trim();
        if let Some(parent) = block_indent {
            if code.is_empty() {
                continue;
            }
            if indent > parent {
                commands.push(code.to_string());
                continue;
            }
            block_indent = None;
        }
        let item = strip_list_dash(code);
        let Some(rest) = item.strip_prefix("run:") else {
            continue;
        };
        let rest = rest.trim();
        if rest.starts_with('|') || rest.starts_with('>') {
            // La clave `run` esta tras el guion si lo hay: sumamos su ancho.
            let dash = code.len() - item.len();
            block_indent = Some(indent + dash);
        } else if !rest.is_empty() {
            commands.push(rest.to_string());
        }
    }
    commands
}

/// Valor de `rust-version` en el `Cargo.toml` raiz, sin comillas.
fn workspace_rust_version() -> String {
    let text = read_text("Cargo.toml");
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "rust-version" {
            return value.trim().trim_matches(['"', '\'']).to_string();
        }
    }
    panic!("no se encontro `rust-version` en el Cargo.toml raiz");
}

fn assert_no_failures(failures: &[String]) {
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn matrix_covers_linux_windows_and_macos() {
    let lines = code_lines(&read_workflow());
    let missing: Vec<&str> = EXPECTED_OS
        .iter()
        .copied()
        .filter(|os| {
            !lines.iter().any(|l| {
                l.split(|c: char| !(c.is_alphanumeric() || c == '-'))
                    .any(|w| w == *os)
            })
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{WORKFLOW_PATH}: la matriz `os` debe incluir {EXPECTED_OS:?}; faltan {missing:?}"
    );
}

#[test]
fn matrix_does_not_fail_fast() {
    let lines = code_lines(&read_workflow());
    assert!(
        lines.iter().any(|l| l == "fail-fast: false"),
        "{WORKFLOW_PATH}: falta `fail-fast: false` en la estrategia de la matriz"
    );
}

#[test]
fn run_steps_include_the_four_commands() {
    let commands = run_commands(&read_workflow());
    let missing: Vec<&str> = EXPECTED_COMMANDS
        .iter()
        .copied()
        .filter(|expected| !commands.iter().any(|c| c == expected))
        .collect();
    assert!(
        missing.is_empty(),
        "{WORKFLOW_PATH}: faltan como lineas `run` estos comandos: {missing:?}; \
         comandos `run` encontrados: {commands:?}"
    );
}

#[test]
fn run_commands_appear_in_the_specified_order() {
    let commands = run_commands(&read_workflow());
    let positions: Vec<Option<usize>> = EXPECTED_COMMANDS
        .iter()
        .map(|expected| commands.iter().position(|c| c == expected))
        .collect();
    assert!(
        positions.iter().all(Option::is_some),
        "{WORKFLOW_PATH}: no estan los cuatro comandos {EXPECTED_COMMANDS:?}; \
         comandos `run` encontrados: {commands:?}"
    );
    let found: Vec<usize> = positions.into_iter().flatten().collect();
    assert!(
        found.windows(2).all(|w| w[0] < w[1]),
        "{WORKFLOW_PATH}: los comandos deben ir en este orden (fmt, clippy, test, deny): \
         {EXPECTED_COMMANDS:?}; posiciones encontradas: {found:?}"
    );
}

#[test]
fn clippy_denies_warnings() {
    let commands = run_commands(&read_workflow());
    let clippy: Vec<&String> = commands
        .iter()
        .filter(|c| c.starts_with("cargo clippy"))
        .collect();
    assert!(
        !clippy.is_empty(),
        "{WORKFLOW_PATH}: no hay ninguna linea `run` con `cargo clippy`"
    );
    let without: Vec<&&String> = clippy
        .iter()
        .filter(|c| !c.contains("-D warnings"))
        .collect();
    assert!(
        without.is_empty(),
        "{WORKFLOW_PATH}: la linea de clippy debe llevar `-D warnings`; sin el: {without:?}"
    );
}

#[test]
fn permissions_are_declared_and_read_only() {
    let text = read_workflow();
    let lines = code_lines(&text);
    let mut failures = Vec::new();
    if !lines.iter().any(|l| l == "permissions:") {
        failures.push(format!("{WORKFLOW_PATH}: falta `permissions:`"));
    }
    if !lines.iter().any(|l| l == "contents: read") {
        failures.push(format!(
            "{WORKFLOW_PATH}: falta `contents: read` bajo `permissions:`"
        ));
    }
    // Se mira el archivo entero, comentarios incluidos: no debe haber `write`.
    for (n, line) in text.lines().enumerate() {
        if line.to_lowercase().contains("write") {
            failures.push(format!(
                "{WORKFLOW_PATH}:{}: no debe aparecer `write` en el archivo: `{}`",
                n + 1,
                line.trim()
            ));
        }
    }
    assert_no_failures(&failures);
}

#[test]
fn toolchain_version_matches_workspace_rust_version() {
    let prefix = workspace_rust_version();
    let commands = run_commands(&read_workflow());
    let Some(install) = commands
        .iter()
        .find(|c| c.starts_with("rustup toolchain install"))
    else {
        panic!("{WORKFLOW_PATH}: falta una linea `run` con `rustup toolchain install <version>`");
    };
    let Some(version) = install
        .strip_prefix("rustup toolchain install")
        .and_then(|rest| rest.split_whitespace().next())
    else {
        panic!("{WORKFLOW_PATH}: `{install}` no indica la version del toolchain");
    };
    let is_patch_of_prefix = version
        .strip_prefix(prefix.as_str())
        .and_then(|rest| rest.strip_prefix('.'))
        .is_some_and(|patch| !patch.is_empty() && patch.chars().all(|c| c.is_ascii_digit()));
    assert!(
        is_patch_of_prefix,
        "{WORKFLOW_PATH}: la version del toolchain `{version}` debe ser `{prefix}.x` \
         (rust-version = \"{prefix}\" en el Cargo.toml raiz)"
    );
}

#[test]
fn actions_are_allowed_and_pinned_by_sha_with_version_comment() {
    let text = read_workflow();
    let mut failures = Vec::new();
    let mut count = 0;
    for (n, raw) in text.lines().enumerate() {
        let item = strip_list_dash(raw.trim());
        let Some(rest) = item.strip_prefix("uses:") else {
            continue;
        };
        count += 1;
        let at = format!("{WORKFLOW_PATH}:{}", n + 1);
        let (spec, comment) = match rest.split_once('#') {
            Some((spec, comment)) => (spec.trim(), Some(comment.trim())),
            None => (rest.trim(), None),
        };
        let Some((action, reference)) = spec.split_once('@') else {
            failures.push(format!(
                "{at}: `uses: {spec}` debe ir fijada con `@<sha de 40 hex>`"
            ));
            continue;
        };
        if !ALLOWED_ACTIONS.contains(&action) {
            failures.push(format!(
                "{at}: la accion `{action}` no esta permitida; solo {ALLOWED_ACTIONS:?}"
            ));
        }
        if reference.len() != 40 || !reference.chars().all(|c| c.is_ascii_hexdigit()) {
            failures.push(format!(
                "{at}: `{action}@{reference}` debe fijarse por SHA de commit de 40 caracteres hexadecimales"
            ));
        }
        let tag_ok = comment.is_some_and(|c| {
            let mut parts = c.strip_prefix('v').unwrap_or("").split('.');
            (0..3).all(|_| {
                parts
                    .next()
                    .is_some_and(|p| !p.is_empty() && p.chars().all(|ch| ch.is_ascii_digit()))
            }) && parts.next().is_none()
        });
        if !tag_ok {
            failures.push(format!(
                "{at}: `{action}` necesita un comentario con la etiqueta de version, por ejemplo `# v1.2.3`"
            ));
        }
    }
    if count == 0 {
        failures.push(format!(
            "{WORKFLOW_PATH}: no hay ninguna accion `uses:` (se espera al menos actions/checkout)"
        ));
    }
    assert_no_failures(&failures);
}

#[test]
fn cargo_deny_is_installed_with_exact_version() {
    let lines = code_lines(&read_workflow());
    assert!(
        lines.iter().any(|l| l == "tool: cargo-deny@0.20.2"),
        "{WORKFLOW_PATH}: falta `tool: cargo-deny@0.20.2` (version exacta, no flotante)"
    );
}
