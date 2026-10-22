//! Filas 1 a 4 de specs/hito-3-px-app-lib.md (unidad 3.0): la estructura de la biblioteca y el
//! binario, y la salida del arranque fuera de la ventana. Solo usa `std`; las filas 3 y 4 lanzan el
//! binario real (`CARGO_BIN_EXE_px-app`) con argumentos que terminan antes de abrir ventana.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn src(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
}

fn third_party() -> Vec<u8> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/THIRD-PARTY.md");
    fs::read(&path).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
}

fn launch(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_px-app"))
        .args(args)
        .output()
        .expect("no se pudo lanzar px-app")
}

/// Fila 1: `lib.rs` y `main.rs` existen; `main.rs` no declara módulos, solo llama a `px_app::run`
/// y tiene como mucho 12 líneas de código (sin comentarios ni vacías).
#[test]
fn main_es_minimo_y_lib_existe() {
    assert!(!src("lib.rs").is_empty(), "src/lib.rs está vacío");
    let main = src("main.rs");
    let code: Vec<&str> = main
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .collect();
    assert!(
        code.len() <= 12,
        "main.rs tiene {} líneas de código",
        code.len()
    );
    assert!(
        code.iter()
            .all(|l| !l.starts_with("mod ") && !l.starts_with("pub mod ")),
        "main.rs declara módulos"
    );
    assert!(
        code.iter().any(|l| l.contains("px_app::run(")),
        "main.rs no llama a px_app::run"
    );
}

/// Fila 2: la lib exporta una sola cosa. Ninguna línea de nivel 0 de `lib.rs` es un `pub` salvo
/// `pub fn run`.
#[test]
fn la_lib_solo_exporta_run() {
    let lib = src("lib.rs");
    let public: Vec<&str> = lib.lines().filter(|l| l.starts_with("pub")).collect();
    assert_eq!(public.len(), 1, "exportado en lib.rs: {public:?}");
    assert!(
        public[0].starts_with("pub fn run("),
        "exportado: {}",
        public[0]
    );
}

/// Fila 3: `--licencias` escribe `docs/THIRD-PARTY.md` tal cual y sale con 0.
#[test]
fn licencias_imprime_y_sale_con_cero() {
    let out = launch(&["--licencias"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, third_party());
}

/// Fila 4: dos argumentos dan el uso en la salida de errores, nada en la estándar y el código 2.
#[test]
fn dos_argumentos_dan_uso_y_codigo_dos() {
    let out = launch(&["a.png", "b.png"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "uso: px-app [archivo]\n"
    );
}
