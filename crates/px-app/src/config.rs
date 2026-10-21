//! La carpeta de configuración y el fichero de atajos (specs/interfaz-ayuda.md, decisiones 9 y 10 y
//! visto bueno 3): dónde vive `atajos.txt` en cada sistema, su lectura sin tocarlo y su escritura
//! atómica con la copia `atajos.txt.bak`. El formato es de `px_ui::keymap_file`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use px_ui::actions::Platform;
use px_ui::keymap::Keymap;
use px_ui::keymap_file::{Loaded, MAX_BYTES, parse, to_text};

use crate::files::{FileError, write_atomic};

/// Carpeta del programa dentro de la de configuración y nombre del fichero.
const APP_DIR: &str = "pixoko";
const SHORTCUTS_FILE: &str = "atajos.txt";

/// La carpeta de configuración del usuario: `XDG_CONFIG_HOME` (si es una ruta absoluta y no vacía)
/// o `~/.config` en Linux, `APPDATA` en Windows y `~/Library/Application Support` en macOS. `None`
/// si falta la variable que hace falta. `env` lee las variables de entorno.
pub fn config_dir(platform: Platform, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env(name).filter(|value| !value.is_empty());
    match platform {
        Platform::Linux => non_empty("XDG_CONFIG_HOME")
            .filter(|value| Path::new(value).is_absolute())
            .map(PathBuf::from)
            .or_else(|| non_empty("HOME").map(|home| PathBuf::from(home).join(".config"))),
        Platform::Windows => non_empty("APPDATA").map(PathBuf::from),
        Platform::Mac => non_empty("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        }),
    }
}

/// `<configuración>/pixoko/atajos.txt`.
pub fn shortcuts_path(platform: Platform, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    config_dir(platform, env).map(|dir| dir.join(APP_DIR).join(SHORTCUTS_FILE))
}

/// Por qué no se pudo guardar.
#[derive(Debug)]
pub enum SaveError {
    /// No hay carpeta de configuración (sin `HOME` ni `APPDATA`).
    NoDirectory,
    File(FileError),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NoDirectory => {
                write!(f, "atajos: no hay carpeta de configuración, no se guardan")
            }
            SaveError::File(error) => write!(f, "atajos: no se pudieron guardar ({error})"),
        }
    }
}

/// El fichero de atajos: su ruta y si la primera escritura de la sesión debe copiar antes el
/// original (porque la lectura ignoró algo).
#[derive(Debug)]
pub struct ShortcutStore {
    path: Option<PathBuf>,
    backup_pending: bool,
}

impl ShortcutStore {
    pub fn new(path: Option<PathBuf>) -> ShortcutStore {
        ShortcutStore {
            path,
            backup_pending: false,
        }
    }

    fn backup_path(path: &Path) -> PathBuf {
        let mut name = path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        name.push(".bak");
        path.with_file_name(name)
    }

    /// Lee el fichero sin tocarlo. Sin fichero (o sin carpeta), la tabla por defecto y ningún aviso;
    /// ilegible o de otra versión, la de fábrica con su `fatal`. Si ignoró algo, la primera
    /// escritura copiará antes el original.
    pub fn load(&mut self, default: &Keymap) -> Loaded {
        let Some(path) = &self.path else {
            return parse(b"pixoko-atajos 1\n", default);
        };
        let bytes = match fs::File::open(path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                match file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes) {
                    Ok(_) => bytes,
                    // No se pudo leer (un directorio, un permiso): se trata como ilegible.
                    Err(_) => vec![0xff],
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return parse(b"pixoko-atajos 1\n", default);
            }
            Err(_) => vec![0xff],
        };
        let loaded = parse(&bytes, default);
        self.backup_pending =
            loaded.ignored_lines > 0 || loaded.dropped_rows > 0 || loaded.fatal.is_some();
        loaded
    }

    /// Guarda `keymap` (solo lo que difiere de `default`) con escritura atómica, creando la carpeta.
    /// Antes copia el fichero que hubiera a `atajos.txt.bak`, sustituyendo cualquier copia anterior,
    /// si `reset_all` o si es la primera escritura tras una lectura que ignoró algo.
    pub fn save(
        &mut self,
        keymap: &Keymap,
        default: &Keymap,
        reset_all: bool,
    ) -> Result<(), SaveError> {
        let path = self.path.as_ref().ok_or(SaveError::NoDirectory)?;
        let write_error = |error: std::io::Error| {
            SaveError::File(FileError::Write(format!("{}: {error}", path.display())))
        };
        if (reset_all || self.backup_pending) && path.is_file() {
            fs::copy(path, Self::backup_path(path)).map_err(write_error)?;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(write_error)?;
        }
        write_atomic(path, to_text(keymap, default).as_bytes()).map_err(SaveError::File)?;
        self.backup_pending = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
