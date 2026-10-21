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

static CONTADOR: AtomicU64 = AtomicU64::new(0);

struct Carpeta(PathBuf);

impl Carpeta {
    fn nueva() -> Carpeta {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/pixoko-test-config")
            .join(format!("{}-{n}", std::process::id()));
        fs::create_dir_all(&ruta).unwrap();
        Carpeta(ruta)
    }

    fn ruta(&self) -> &Path {
        &self.0
    }
}

impl Drop for Carpeta {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn entorno(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |nombre| {
        vars.iter()
            .find(|(clave, _)| *clave == nombre)
            .map(|(_, valor)| (*valor).to_string())
    }
}

fn defecto() -> Keymap {
    Keymap::default_scheme()
}

fn lapiz_con_x() -> Keymap {
    let mut e = px_ui::shortcut_editor::ShortcutEditor::new(
        AppAction::SelectTool(Tool::Pencil),
        &defecto(),
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

fn sin_archivos_temporales(dir: &Path) {
    for entrada in fs::read_dir(dir).unwrap() {
        let nombre = entrada.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            !nombre.contains("pixoko-tmp"),
            "temporal sin borrar: {nombre}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Carpeta de configuración
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_la_carpeta_de_configuracion_por_sistema() {
    // Linux: XDG_CONFIG_HOME si es absoluta y no vacía; si no, $HOME/.config.
    let linux = |vars| config_dir(Platform::Linux, &entorno(vars));
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
    let windows = |vars| config_dir(Platform::Windows, &entorno(vars));
    assert_eq!(
        windows(&[("APPDATA", "C:\\Users\\a\\AppData\\Roaming")]),
        Some(PathBuf::from("C:\\Users\\a\\AppData\\Roaming"))
    );
    assert_eq!(windows(&[("HOME", "/home/a")]), None);
    // macOS: ~/Library/Application Support.
    let mac = |vars| config_dir(Platform::Mac, &entorno(vars));
    assert_eq!(
        mac(&[("HOME", "/Users/a")]),
        Some(PathBuf::from("/Users/a/Library/Application Support"))
    );
    assert_eq!(mac(&[]), None);
    // El fichero cuelga de `pixoko/atajos.txt`.
    assert_eq!(
        shortcuts_path(Platform::Linux, &entorno(&[("HOME", "/home/a")])),
        Some(PathBuf::from("/home/a/.config/pixoko/atajos.txt"))
    );
    assert_eq!(shortcuts_path(Platform::Linux, &entorno(&[])), None);
}

// ---------------------------------------------------------------------------------------------
// Lectura y escritura
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_guardar_crea_la_carpeta_escribe_atomico_y_se_lee_igual() {
    let dir = Carpeta::nueva();
    let ruta = dir.ruta().join("pixoko").join("atajos.txt");
    let mut tienda = ShortcutStore::new(Some(ruta.clone()));
    // Sin fichero: la tabla por defecto, sin aviso.
    let cargado = tienda.load(&defecto());
    assert_eq!(cargado.keymap, defecto());
    assert!(cargado.fatal.is_none());
    let tabla = lapiz_con_x();
    tienda.save(&tabla, &defecto(), false).expect("guarda");
    assert!(ruta.is_file());
    sin_archivos_temporales(ruta.parent().unwrap());
    assert_eq!(
        fs::read_to_string(&ruta).unwrap(),
        to_text(&tabla, &defecto())
    );
    let mut otra = ShortcutStore::new(Some(ruta.clone()));
    assert_eq!(otra.load(&defecto()).keymap, tabla);
    // Nada ignorado: sin copia `.bak`.
    assert!(!ruta.with_file_name("atajos.txt.bak").exists());
    // Sobrescribir otra vez tampoco la crea.
    tienda.save(&defecto(), &defecto(), false).unwrap();
    assert!(!ruta.with_file_name("atajos.txt.bak").exists());
    sin_archivos_temporales(ruta.parent().unwrap());
}

#[test]
fn condicion_3_si_la_lectura_ignoro_algo_la_primera_escritura_copia_el_original() {
    let dir = Carpeta::nueva();
    let ruta = dir.ruta().join("atajos.txt");
    let bak = dir.ruta().join("atajos.txt.bak");
    let original = "pixoko-atajos 1\ntool.pencil 1 x\nlinea mala\n";
    fs::write(&ruta, original).unwrap();
    // Un `.bak` anterior se sustituye.
    fs::write(&bak, "viejo").unwrap();
    let mut tienda = ShortcutStore::new(Some(ruta.clone()));
    let cargado = tienda.load(&defecto());
    assert_eq!(cargado.ignored_lines, 1);
    assert_eq!(
        fs::read_to_string(&ruta).unwrap(),
        original,
        "leer no toca el fichero"
    );
    tienda.save(&lapiz_con_x(), &defecto(), false).unwrap();
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        original,
        "copia de lo que había"
    );
    assert_ne!(fs::read_to_string(&ruta).unwrap(), original);
    // Solo la primera escritura de la sesión: una segunda no vuelve a copiar.
    fs::write(&bak, "marca").unwrap();
    tienda.save(&defecto(), &defecto(), false).unwrap();
    assert_eq!(fs::read_to_string(&bak).unwrap(), "marca");
}

#[test]
fn condicion_3_un_fichero_ilegible_se_ignora_y_su_copia_precede_a_la_escritura() {
    let dir = Carpeta::nueva();
    let ruta = dir.ruta().join("atajos.txt");
    let bak = dir.ruta().join("atajos.txt.bak");
    fs::write(&ruta, [0xffu8, 0xfe, 0x80]).unwrap();
    let mut tienda = ShortcutStore::new(Some(ruta.clone()));
    let cargado = tienda.load(&defecto());
    assert_eq!(cargado.fatal, Some(Fatal::Unreadable));
    assert_eq!(
        fs::read(&ruta).unwrap(),
        [0xff, 0xfe, 0x80],
        "sin borrar ni tocar"
    );
    tienda.save(&lapiz_con_x(), &defecto(), false).unwrap();
    assert_eq!(fs::read(&bak).unwrap(), [0xff, 0xfe, 0x80]);
}

#[test]
fn condicion_3_restablecer_todo_siempre_copia_y_deja_solo_la_cabecera() {
    let dir = Carpeta::nueva();
    let ruta = dir.ruta().join("atajos.txt");
    let bak = dir.ruta().join("atajos.txt.bak");
    let tabla = lapiz_con_x();
    let guardado = to_text(&tabla, &defecto());
    fs::write(&ruta, &guardado).unwrap();
    fs::write(&bak, "viejo").unwrap();
    let mut tienda = ShortcutStore::new(Some(ruta.clone()));
    let cargado = tienda.load(&defecto());
    assert_eq!((cargado.ignored_lines, cargado.dropped_rows), (0, 0));
    tienda.save(&defecto(), &defecto(), true).unwrap();
    assert_eq!(
        fs::read_to_string(&bak).unwrap(),
        guardado,
        "sustituye cualquier .bak"
    );
    assert_eq!(
        fs::read_to_string(&ruta).unwrap(),
        to_text(&defecto(), &defecto())
    );
    // Sin fichero previo, restablecer escribe la cabecera y no falla por no haber qué copiar.
    let otra = Carpeta::nueva();
    let nueva = otra.ruta().join("a").join("atajos.txt");
    let mut t2 = ShortcutStore::new(Some(nueva.clone()));
    t2.save(&defecto(), &defecto(), true).unwrap();
    assert!(nueva.is_file());
    assert!(!nueva.with_file_name("atajos.txt.bak").exists());
}

#[test]
fn fila_11_los_fallos_son_errores_y_sin_carpeta_no_se_guarda() {
    let mut sin = ShortcutStore::new(None);
    assert!(matches!(
        sin.save(&defecto(), &defecto(), false),
        Err(SaveError::NoDirectory)
    ));
    assert_eq!(sin.load(&defecto()).keymap, defecto());
    // El destino es un directorio.
    let dir = Carpeta::nueva();
    let ruta = dir.ruta().join("atajos.txt");
    fs::create_dir_all(&ruta).unwrap();
    let mut t = ShortcutStore::new(Some(ruta.clone()));
    assert!(matches!(
        t.save(&lapiz_con_x(), &defecto(), false),
        Err(SaveError::File(_))
    ));
    assert!(ruta.is_dir());
    sin_archivos_temporales(dir.ruta());
    // La carpeta padre es un fichero.
    let fichero = dir.ruta().join("fichero");
    fs::write(&fichero, "x").unwrap();
    let mut t = ShortcutStore::new(Some(fichero.join("atajos.txt")));
    assert!(matches!(
        t.save(&lapiz_con_x(), &defecto(), false),
        Err(SaveError::File(_))
    ));
    assert_eq!(fs::read_to_string(&fichero).unwrap(), "x");
}
