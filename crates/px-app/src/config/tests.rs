//! Tests of `config` (specs/interfaz-ayuda.md, unit I.5, row 11 and condition 3 of the sign-off):
//! per-system configuration folder, atomic read and write of `atajos.txt` and the
//! `atajos.txt.bak` copy.
//!
//! Each test uses its own folder under `target/` (on disk, not in `/tmp`), deleted at the end.

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
            .map(|(_, value)| (*value).to_string())
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
            "temporary file not deleted: {name_2}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Configuration folder
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_configuration_folder_per_system() {
    // `config_dir` asks the host whether XDG_CONFIG_HOME is absolute, and `join` uses the host
    // separator: the absolute path and the expected values are built per host so the Linux rows
    // also hold when the suite runs on Windows ("/cfg" has no drive letter there).
    const ABSOLUTE: &str = if cfg!(windows) { "C:\\cfg" } else { "/cfg" };
    let home_config = PathBuf::from("/home/a").join(".config");
    // Linux: XDG_CONFIG_HOME if absolute and non-empty; otherwise $HOME/.config.
    let linux = |vars| config_dir(Platform::Linux, &environment(vars));
    assert_eq!(
        linux(&[("XDG_CONFIG_HOME", ABSOLUTE), ("HOME", "/home/a")]),
        Some(PathBuf::from(ABSOLUTE))
    );
    assert_eq!(
        linux(&[("XDG_CONFIG_HOME", "relative"), ("HOME", "/home/a")]),
        Some(home_config.clone()),
        "a relative path is ignored"
    );
    assert_eq!(
        linux(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/a")]),
        Some(home_config.clone()),
        "an empty one is ignored"
    );
    assert_eq!(linux(&[("HOME", "/home/a")]), Some(home_config.clone()));
    assert_eq!(linux(&[]), None, "without HOME there is no folder");
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
        Some(
            PathBuf::from("/Users/a")
                .join("Library")
                .join("Application Support")
        )
    );
    assert_eq!(mac(&[]), None);
    // The file hangs from `pixoko/atajos.txt`.
    assert_eq!(
        shortcuts_path(Platform::Linux, &environment(&[("HOME", "/home/a")])),
        Some(home_config.join("pixoko").join("atajos.txt"))
    );
    assert_eq!(shortcuts_path(Platform::Linux, &environment(&[])), None);
}

// ---------------------------------------------------------------------------------------------
// Reading and writing
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_save_creates_the_folder_writes_atomically_and_reads_back_the_same() {
    let dir = Folder::new();
    let path_2 = dir.path_2().join("pixoko").join("atajos.txt");
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    // No file: the default table, without a warning.
    let loaded = store.load(&default());
    assert_eq!(loaded.keymap, default());
    assert!(loaded.fatal.is_none());
    let table = pencil_with_x();
    store.save(&table, &default(), false).expect("saves");
    assert!(path_2.is_file());
    without_files_temporary(path_2.parent().unwrap());
    assert_eq!(
        fs::read_to_string(&path_2).unwrap(),
        to_text(&table, &default())
    );
    let mut other = ShortcutStore::new(Some(path_2.clone()));
    assert_eq!(other.load(&default()).keymap, table);
    // Nothing ignored: no `.bak` copy.
    assert!(!path_2.with_file_name("atajos.txt.bak").exists());
    // Overwriting again does not create it either.
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
    // A previous `.bak` is replaced.
    fs::write(&bak, "viejo").unwrap();
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    let loaded = store.load(&default());
    assert_eq!(loaded.ignored_lines, 1);
    assert_eq!(
        fs::read_to_string(&path_2).unwrap(),
        original,
        "reading does not touch the file"
    );
    store.save(&pencil_with_x(), &default(), false).unwrap();
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        original,
        "copy of what was there"
    );
    assert_ne!(fs::read_to_string(&path_2).unwrap(), original);
    // Only the first write of the session: a second one does not copy again.
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
        "neither deleted nor touched"
    );
    store.save(&pencil_with_x(), &default(), false).unwrap();
    assert_eq!(fs::read(&bak).unwrap(), [0xff, 0xfe, 0x80]);
}

#[test]
fn condition_3_reset_all_always_copies_and_leaves_only_the_header() {
    let dir = Folder::new();
    let path_2 = dir.path_2().join("atajos.txt");
    let bak = dir.path_2().join("atajos.txt.bak");
    let table = pencil_with_x();
    let saved = to_text(&table, &default());
    fs::write(&path_2, &saved).unwrap();
    fs::write(&bak, "viejo").unwrap();
    let mut store = ShortcutStore::new(Some(path_2.clone()));
    let loaded = store.load(&default());
    assert_eq!((loaded.ignored_lines, loaded.dropped_rows), (0, 0));
    store.save(&default(), &default(), true).unwrap();
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        saved,
        "replaces any .bak"
    );
    assert_eq!(
        fs::read_to_string(&path_2).unwrap(),
        to_text(&default(), &default())
    );
    // With no previous file, resetting writes the header and does not fail for having nothing to copy.
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
    // The destination is a directory.
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
    // The parent folder is a file.
    let file = dir.path_2().join("fichero");
    fs::write(&file, "x").unwrap();
    let mut t = ShortcutStore::new(Some(file.join("atajos.txt")));
    assert!(matches!(
        t.save(&pencil_with_x(), &default(), false),
        Err(SaveError::File(_))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "x");
}
