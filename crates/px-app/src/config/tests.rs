//! Tests de `config` (specs/interfaz-ayuda.md, unidad I.5, fila 11 y condición 3 del visto bueno):
//! carpeta de configuración por sistema, lectura y escritura atómica de `atajos.txt` y copia
//! `atajos.txt.bak`. En ROJO hasta la lógica: `config` no existe todavía.
//!
//! Cada test usa su propia carpeta en `target/` (en disco, no en `/tmp`), borrada al terminar.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use px_ui::actions::{AppAction, Platform};
use px_ui::keymap::Keymap;
use px_ui::keymap_file::{Fatal, to_text};
use px_ui::tool::Tool;

use super::{SaveError, ShortcutStore, config_dir, shortcuts_path};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Folder(PathBuf);

impl Folder {
    fn new() -> Folder {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path_2 = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/pixoko-test-config")
            .join(format!("{}-{n}", std::process::id()));
        fs::create_dir_all(&path_2).unwrap();
        Folder(path_2)
    }

    fn path_2(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn environment(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name_2| {
        vars.iter()
            .find(|(key, _)| *key == name_2)
            .map(|(_, valor)| (*valor).to_string())
    }
}

fn default() -> Keymap {
    Keymap::default_scheme()
}

fn pencil_with_x() -> Keymap {
    let mut e = px_ui::shortcut_editor::ShortcutEditor::new(
        AppAction::SelectTool(Tool::Pencil),
        &default(),
        Platform::Linux,
    )
    .unwrap();
    e.begin_capture(px_ui::shortcut_editor::Slot::Primary);
    e.capture(px_ui::key_events::Capture::Press(px_ui::keymap::Press {
        key: Some(px_ui::keymap::KeyId::Char('x')),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }));
    e.accept()
}

fn without_files_temporary(dir: &Path) {
    for input in fs::read_dir(dir).unwrap() {
        let name_2 = input.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            !name_2.contains("pixoko-tmp"),
            "temporal sin borrar: {name_2}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Carpeta de configuración
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_configuration_folder_per_system() {
    // Linux: XDG_CONFIG_HOME si es absoluta y no vacía; si no, $HOME/.config.
    let linux = |vars| config_dir(Platform::Linux, &environment(vars));
    assert_eq!(
        linux(&[("XDG_CONFIG_HOME", "/cfg"), ("HOME", "/home/a")]),
        Some(PathBuf::from("/cfg"))
    );
    assert_eq!(
        linux(&[("XDG_CONFIG_HOME", "relativa"), ("HOME", "/home/a")]),
        Some(PathBuf::from("/home/a/.config")),
        "una ruta relativa se ignora"
    );
    assert_eq!(
        linux(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/a")]),
        Some(PathBuf::from("/home/a/.config")),
        "vacía se ignora"
    );
    assert_eq!(
        linux(&[("HOME", "/home/a")]),
        Some(PathBuf::from("/home/a/.config"))
    );
    assert_eq!(linux(&[]), None, "sin HOME no hay carpeta");
    assert_eq!(linux(&[("HOME", "")]), None);
    // Windows: APPDATA.
    let windows = |vars| config_dir(Platform::Windows, &environment(vars));
    assert_eq!(
        windows(&[("APPDATA", "C:\\Users\\a\\AppData\\Roaming")]),
        Some(PathBuf::from("C:\\Users\\a\\AppData\\Roaming"))
    );
    assert_eq!(windows(&[("HOME", "/home/a")]), None);
    // macOS: ~/Library/Application Support.
    let mac = |vars| config_dir(Platform::Mac, &environment(vars));
    assert_eq!(
        mac(&[("HOME", "/Users/a")]),
        Some(PathBuf::from("/Users/a/Library/Application Support"))
    );
    assert_eq!(mac(&[]), None);
    // El fichero cuelga de `pixoko/atajos.txt`.
    assert_eq!(
        shortcuts_path(Platform::Linux, &environment(&[("HOME", "/home/a")])),
        Some(PathBuf::from("/home/a/.config/pixoko/atajos.txt"))
    );
    assert_eq!(shortcuts_path(Platform::Linux, &environment(&[])), None);
}

// ---------------------------------------------------------------------------------------------
// Lectura y escritura
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_save_creates_the_folder_writes_atomically_and_reads_back_the_same() {
    let dir = Folder::new();
    let path_2 = dir.path_2().join("pixoko").join("atajos.txt");
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    // Sin fichero: la tabla por defecto, sin aviso.
    let loaded = store.load(&default());
    assert_eq!(loaded.keymap, default());
    assert!(loaded.fatal.is_none());
    let tabla = pencil_with_x();
    store.save(&tabla, &default(), false).expect("guarda");
    assert!(path_2.is_file());
    without_files_temporary(path_2.parent().unwrap());
    assert_eq!(
        fs::read_to_string(&path_2).unwrap(),
        to_text(&tabla, &default())
    );
    let mut other = ShortcutStore::new(Some(path_2.clone()));
    assert_eq!(other.load(&default()).keymap, tabla);
    // Nada ignorado: sin copia `.bak`.
    assert!(!path_2.with_file_name("atajos.txt.bak").exists());
    // Sobrescribir otra vez tampoco la crea.
    store.save(&default(), &default(), false).unwrap();
    assert!(!path_2.with_file_name("atajos.txt.bak").exists());
    without_files_temporary(path_2.parent().unwrap());
}

#[test]
fn condition_3_if_the_read_ignored_something_the_first_write_copies_the_original() {
    let dir = Folder::new();
    let path_2 = dir.path_2().join("atajos.txt");
    let bak = dir.path_2().join("atajos.txt.bak");
    let original = "pixoko-atajos 1\ntool.pencil 1 x\nlinea mala\n";
    fs::write(&path_2, original).unwrap();
    // Un `.bak` anterior se sustituye.
    fs::write(&bak, "viejo").unwrap();
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    let loaded = store.load(&default());
    assert_eq!(loaded.ignored_lines, 1);
    assert_eq!(
        fs::read_to_string(&path_2).unwrap(),
        original,
        "leer no toca el fichero"
    );
    store.save(&pencil_with_x(), &default(), false).unwrap();
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        original,
        "copia de lo que había"
    );
    assert_ne!(fs::read_to_string(&path_2).unwrap(), original);
    // Solo la primera escritura de la sesión: una segunda no vuelve a copiar.
    fs::write(&bak, "marca").unwrap();
    store.save(&default(), &default(), false).unwrap();
    assert_eq!(fs::read_to_string(&bak).unwrap(), "marca");
}

#[test]
fn condition_3_an_unreadable_file_is_ignored_and_its_copy_precedes_the_write() {
    let dir = Folder::new();
    let path_2 = dir.path_2().join("atajos.txt");
    let bak = dir.path_2().join("atajos.txt.bak");
    fs::write(&path_2, [0xffu8, 0xfe, 0x80]).unwrap();
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    let loaded = store.load(&default());
    assert_eq!(loaded.fatal, Some(Fatal::Unreadable));
    assert_eq!(
        fs::read(&path_2).unwrap(),
        [0xff, 0xfe, 0x80],
        "sin borrar ni tocar"
    );
    store.save(&pencil_with_x(), &default(), false).unwrap();
    assert_eq!(fs::read(&bak).unwrap(), [0xff, 0xfe, 0x80]);
}

#[test]
fn condition_3_reset_all_always_copies_and_leaves_only_the_header() {
    let dir = Folder::new();
    let path_2 = dir.path_2().join("atajos.txt");
    let bak = dir.path_2().join("atajos.txt.bak");
    let tabla = pencil_with_x();
    let saved = to_text(&tabla, &default());
    fs::write(&path_2, &saved).unwrap();
    fs::write(&bak, "viejo").unwrap();
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    let loaded = store.load(&default());
    assert_eq!((loaded.ignored_lines, loaded.dropped_rows), (0, 0));
    store.save(&default(), &default(), true).unwrap();
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        saved,
        "sustituye cualquier .bak"
    );
    assert_eq!(
        fs::read_to_string(&path_2).unwrap(),
        to_text(&default(), &default())
    );
    // Sin fichero previo, restablecer escribe la cabecera y no falla por no haber qué copiar.
    let other = Folder::new();
    let new = other.path_2().join("a").join("atajos.txt");
    let mut t2 = ShortcutStore::new(Some(new.clone()));
    t2.save(&default(), &default(), true).unwrap();
    assert!(new.is_file());
    assert!(!new.with_file_name("atajos.txt.bak").exists());
}

#[test]
fn row_11_failures_are_errors_and_without_a_folder_it_is_not_saved() {
    let mut without = ShortcutStore::new(None);
    assert!(matches!(
        without.save(&default(), &default(), false),
        Err(SaveError::NoDirectory)
    ));
    assert_eq!(without.load(&default()).keymap, default());
    // El destino es un directorio.
    let dir = Folder::new();
    let path_2 = dir.path_2().join("atajos.txt");
    fs::create_dir_all(&path_2).unwrap();
    let mut t = ShortcutStore::new(Some(path_2.clone()));
    assert!(matches!(
        t.save(&pencil_with_x(), &default(), false),
        Err(SaveError::File(_))
    ));
    assert!(path_2.is_dir());
    without_files_temporary(dir.path_2());
    // La carpeta padre es un fichero.
    let file = dir.path_2().join("fichero");
    fs::write(&file, "x").unwrap();
    let mut t = ShortcutStore::new(Some(file.join("atajos.txt")));
    assert!(matches!(
        t.save(&pencil_with_x(), &default(), false),
        Err(SaveError::File(_))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "x");
}
