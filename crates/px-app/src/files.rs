//! Ficheros de la app: argumentos, lectura con topes, escritura atómica y nombres de copia
//! (specs/hito-2-abrir-guardar.md). Sin egui: se prueba sin ventana.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use px_core::{Image, Mode};
use px_formats::{Conversion, PngError, PngRead, png_dimensions, read_png};

/// Tope del fichero de entrada. TEMPORAL hasta la unidad 14.
pub const MAX_FILE_BYTES: u64 = 256 << 20;

/// Tope de píxeles de una imagen abierta (8192×8192). TEMPORAL hasta la unidad 14, que traerá
/// texturas por teselas y un tope de memoria: `Canvas::set_image` y `to_rgba8` reservan sin
/// comprobar, y con este tope el pico queda en torno a 1 GiB.
pub const MAX_OPEN_PIXELS: u64 = 1 << 26;

/// Nombre (sin extensión) del fichero de un documento que aún no tiene ruta.
const UNTITLED: &str = "sin-titulo";

/// Error de una operación de fichero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileError {
    /// No es un fichero normal (directorio, FIFO, dispositivo).
    NotAFile,
    TooBigFile {
        bytes: u64,
        max: u64,
    },
    Read(String),
    Png(PngError),
    /// Un lado supera el lado máximo de textura de la GPU.
    TooLargeToShow {
        width: u32,
        height: u32,
        max_side: usize,
    },
    /// Más píxeles que `MAX_OPEN_PIXELS`.
    TooManyPixels {
        width: u32,
        height: u32,
        max: u64,
    },
    /// Hay cambios sin guardar y no se pidió descartarlos.
    Unsaved,
    ReadOnly(PathBuf),
    Write(String),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileError::NotAFile => write!(f, "no es un fichero normal"),
            FileError::TooBigFile { bytes, max } => {
                write!(
                    f,
                    "el fichero ({bytes} bytes) supera el máximo de {max} bytes"
                )
            }
            FileError::Read(msg) => write!(f, "no se pudo leer: {msg}"),
            FileError::Png(error) => write!(f, "{error}"),
            FileError::TooLargeToShow {
                width,
                height,
                max_side,
            } => write!(
                f,
                "la imagen de {width}×{height} no cabe en una textura (lado máximo {max_side})"
            ),
            FileError::TooManyPixels { width, height, max } => write!(
                f,
                "la imagen de {width}×{height} supera el máximo temporal de {max} píxeles"
            ),
            FileError::Unsaved => write!(f, "hay cambios sin guardar"),
            FileError::ReadOnly(path) => write!(f, "{} es de solo lectura", path.display()),
            FileError::Write(msg) => write!(f, "no se pudo escribir: {msg}"),
        }
    }
}

impl std::error::Error for FileError {}

/// Argumentos de la línea de comandos, sin el nombre del programa.
pub fn parse_args(args: &[OsString]) -> Result<Option<PathBuf>, String> {
    match args {
        [] => Ok(None),
        [path] => Ok(Some(PathBuf::from(path))),
        _ => Err("uso: px-app [archivo.png]".to_string()),
    }
}

/// El primer fichero soltado con ruta y cuántos más con ruta se ignoran.
pub fn pick_dropped(files: &[Option<PathBuf>]) -> Option<(PathBuf, usize)> {
    let mut with_path = files.iter().flatten();
    let first = with_path.next()?;
    Some((first.clone(), with_path.count()))
}

/// Lee un PNG con topes: fichero normal de hasta `max_bytes`, y dimensiones que quepan
/// (`check_fits`) **antes** de decodificar.
pub fn load_png(path: &Path, max_bytes: u64, max_side: usize) -> Result<PngRead, FileError> {
    let read_error =
        |error: std::io::Error| FileError::Read(format!("{}: {error}", path.display()));
    let metadata = fs::metadata(path).map_err(read_error)?;
    if !metadata.is_file() {
        return Err(FileError::NotAFile);
    }
    if metadata.len() > max_bytes {
        return Err(FileError::TooBigFile {
            bytes: metadata.len(),
            max: max_bytes,
        });
    }
    let mut bytes = Vec::new();
    let expected = usize::try_from(metadata.len()).unwrap_or(0);
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| FileError::Read("sin memoria para leer el fichero".to_string()))?;
    File::open(path)
        .map_err(read_error)?
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() as u64 > max_bytes {
        return Err(FileError::TooBigFile {
            bytes: bytes.len() as u64,
            max: max_bytes,
        });
    }
    let (width, height) = png_dimensions(&bytes).map_err(FileError::Png)?;
    check_fits(width, height, max_side)?;
    read_png(&bytes).map_err(FileError::Png)
}

/// `Ok` si la imagen se puede mostrar: ningún lado mayor que `max_side` y no más de
/// `MAX_OPEN_PIXELS` píxeles.
pub fn check_fits(width: u32, height: u32, max_side: usize) -> Result<(), FileError> {
    let side = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
    if side(width) > max_side || side(height) > max_side {
        return Err(FileError::TooLargeToShow {
            width,
            height,
            max_side,
        });
    }
    if u64::from(width) * u64::from(height) > MAX_OPEN_PIXELS {
        return Err(FileError::TooManyPixels {
            width,
            height,
            max: MAX_OPEN_PIXELS,
        });
    }
    Ok(())
}

/// Escribe `bytes` en `path` sin dejar nunca un destino a medias: temporal en el mismo
/// directorio, `sync_all` y `rename`. Un enlace simbólico se resuelve (se escribe en su
/// destino) y los permisos del destino existente se conservan. Un destino de solo lectura o un
/// directorio se rechaza antes de escribir. Si algo falla, el temporal se borra y el destino
/// queda como estaba.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), FileError> {
    let write_error =
        |error: std::io::Error| FileError::Write(format!("{}: {error}", path.display()));
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let existing = fs::metadata(&target).ok();
    if let Some(metadata) = &existing {
        if metadata.is_dir() {
            return Err(FileError::Write(format!(
                "{} es un directorio",
                path.display()
            )));
        }
        if metadata.permissions().readonly() {
            return Err(FileError::ReadOnly(path.to_path_buf()));
        }
    }
    let name = target.file_name().ok_or_else(|| {
        FileError::Write(format!("{} no es un nombre de fichero", path.display()))
    })?;
    let mut temp_name = OsString::from(".");
    temp_name.push(name);
    temp_name.push(format!(".pixoko-tmp-{}", std::process::id()));
    let temp = target.with_file_name(temp_name);

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if let Some(metadata) = &existing {
            fs::set_permissions(&temp, metadata.permissions())?;
        }
        drop(file);
        fs::rename(&temp, &target)
    })();
    result.map_err(|error| {
        let _ = fs::remove_file(&temp);
        write_error(error)
    })
}

/// Primer nombre libre `{nombre}-copia.png`, `{nombre}-copia-2.png`… junto al original.
pub fn copy_path(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsString::from(UNTITLED));
    first_free(|n| {
        let mut name = stem.clone();
        name.push(match n {
            1 => "-copia.png".to_string(),
            n => format!("-copia-{n}.png"),
        });
        original.with_file_name(name)
    })
}

/// Primer nombre libre `sin-titulo.png`, `sin-titulo-2.png`… en `dir`.
pub fn untitled_path(dir: &Path) -> PathBuf {
    first_free(|n| match n {
        1 => dir.join(format!("{UNTITLED}.png")),
        n => dir.join(format!("{UNTITLED}-{n}.png")),
    })
}

/// El primer candidato (numerados desde 1) que no existe, ni siquiera como enlace roto.
fn first_free(candidate: impl Fn(u64) -> PathBuf) -> PathBuf {
    (1..)
        .map(candidate)
        .find(|path| fs::symlink_metadata(path).is_err())
        .unwrap_or_default()
}

/// Solo el nombre del fichero, sin directorios.
pub fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Conversiones del informe de `read_png` en texto, separadas por comas.
pub fn describe_report(report: &[Conversion]) -> String {
    report
        .iter()
        .map(|conversion| match conversion {
            Conversion::DepthExpanded { from_bits } => {
                format!("gris de {from_bits} bits ampliado a 8")
            }
            Conversion::Depth16Reduced => "16 bits reducido a 8".to_string(),
            Conversion::RgbToRgba => "RGB convertido a RGBA".to_string(),
            Conversion::ColorKeyToAlpha => "color clave convertido en alfa".to_string(),
            _ => "conversión".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Aviso al abrir: nombre, tamaño, modo y, si hubo conversiones, cuáles y que `Ctrl+S` guardará
/// una copia.
pub fn describe_open(name: &str, image: &Image, report: &[Conversion]) -> String {
    let mode = match image.mode() {
        Mode::Indexed => "indexado",
        Mode::Gray => "gris",
        Mode::GrayAlpha => "gris con alfa",
        Mode::Rgba => "RGBA",
    };
    let mut text = format!(
        "abierto {name} · {}×{} · {mode}",
        image.width(),
        image.height()
    );
    if !report.is_empty() {
        text.push_str(&format!(
            " · convertido: {} · Ctrl+S guardará una copia",
            describe_report(report)
        ));
    }
    text
}

#[cfg(test)]
mod tests;
