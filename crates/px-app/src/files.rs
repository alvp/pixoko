//! Ficheros de la app: argumentos, lectura con topes, escritura atómica y nombres de copia
//! (specs/hito-2-abrir-guardar.md). Sin egui: se prueba sin ventana.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use px_core::{Document, Image, Mode};
use px_formats::{
    AsepriteError, AsepriteLimits, AsepriteRead, AsepriteReport, AuxChunk, AuxReport, Conversion,
    FileFormat, Ignored, PngError, PngRead, aseprite_dimensions, detect_format, png_dimensions,
    read_aseprite, read_png,
};

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
    /// Error de lectura o escritura de un `.aseprite` (el texto dice qué y que el fichero no se ha
    /// modificado).
    Aseprite(AsepriteError),
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
    /// Se añadió `.png` al nombre elegido y ese fichero ya existe.
    Exists(PathBuf),
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
            FileError::Aseprite(error) => write!(f, "{error}"),
            FileError::TooLargeToShow {
                width,
                height,
                max_side,
            } => write!(
                f,
                "la imagen de {width}×{height} supera el lado máximo mostrable ({max_side})"
            ),
            FileError::TooManyPixels { width, height, max } => write!(
                f,
                "la imagen de {width}×{height} supera el máximo temporal de {max} píxeles"
            ),
            FileError::Unsaved => write!(f, "hay cambios sin guardar"),
            FileError::ReadOnly(path) => write!(f, "{} es de solo lectura", path.display()),
            FileError::Write(msg) => write!(f, "no se pudo escribir: {msg}"),
            FileError::Exists(path) => {
                write!(f, "ya existe {}; elige otro nombre", file_label(path))
            }
        }
    }
}

impl std::error::Error for FileError {}

/// Argumentos de la línea de comandos, sin el nombre del programa.
pub fn parse_args(args: &[OsString]) -> Result<Option<PathBuf>, String> {
    match args {
        [] => Ok(None),
        [path] => Ok(Some(PathBuf::from(path))),
        _ => Err("uso: px-app [archivo]".to_string()),
    }
}

/// El primer fichero soltado con ruta y cuántos más con ruta se ignoran.
pub fn pick_dropped(files: &[Option<PathBuf>]) -> Option<(PathBuf, usize)> {
    let mut with_path = files.iter().flatten();
    let first = with_path.next()?;
    Some((first.clone(), with_path.count()))
}

/// Lee un fichero normal de hasta `max_bytes` entero en memoria.
fn read_file_bytes(path: &Path, max_bytes: u64) -> Result<Vec<u8>, FileError> {
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
    Ok(bytes)
}

/// Lee un PNG con topes: fichero normal de hasta `max_bytes`, y dimensiones que quepan
/// (`check_fits`) **antes** de decodificar. Solo lo usan las pruebas (la app llama a `load_image`).
#[cfg(test)]
pub fn load_png(path: &Path, max_bytes: u64, max_side: usize) -> Result<PngRead, FileError> {
    let bytes = read_file_bytes(path, max_bytes)?;
    let (width, height) = png_dimensions(&bytes).map_err(FileError::Png)?;
    check_fits(width, height, max_side)?;
    read_png(&bytes).map_err(FileError::Png)
}

/// Lo que se abrió: un PNG o un `.aseprite`.
pub enum Opened {
    Png(PngRead),
    Aseprite(AsepriteRead),
}

/// Lee un PNG o un `.aseprite` con topes (el formato se decide por el contenido, no por la
/// extensión; lo que no es un `.aseprite` va al lector de PNG, que da su error). El lienzo debe
/// caber (`check_fits`) **antes** de decodificar; las celdas, dentro del tope del núcleo.
pub fn load_image(path: &Path, max_bytes: u64, max_side: usize) -> Result<Opened, FileError> {
    let bytes = read_file_bytes(path, max_bytes)?;
    if detect_format(&bytes) == Some(FileFormat::Aseprite) {
        let (width, height) = aseprite_dimensions(&bytes).map_err(FileError::Aseprite)?;
        check_fits(width, height, max_side)?;
        let limits = AsepriteLimits {
            max_pixels: MAX_OPEN_PIXELS,
            ..AsepriteLimits::default()
        };
        return read_aseprite(&bytes, &limits)
            .map(Opened::Aseprite)
            .map_err(FileError::Aseprite);
    }
    let (width, height) = png_dimensions(&bytes).map_err(FileError::Png)?;
    check_fits(width, height, max_side)?;
    read_png(&bytes).map(Opened::Png).map_err(FileError::Png)
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

/// Primer nombre libre `{name_2}-copy.png`, `{name_2}-copy-2.png`… junto al original; un
/// `.aseprite` conserva su extensión (`{name_2}-copy.aseprite`).
pub fn copy_path(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsString::from(UNTITLED));
    let extension = if crate::dialogs::is_aseprite_path(original) {
        "aseprite"
    } else {
        "png"
    };
    first_free(|n| {
        let mut name = stem.clone();
        name.push(match n {
            1 => format!("-copia.{extension}"),
            n => format!("-copia-{n}.{extension}"),
        });
        original.with_file_name(name)
    })
}

/// Primer nombre libre `without-title.png`, `without-title-2.png`… en `dir`.
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

/// Coletilla de las causas que fuerzan una copia (specs/hito-2-chunks-auxiliares.md, D4).
const COPY_TAIL: &str = "; Ctrl+S guardará una copia";

/// Causas por las que guardar sobre el original perdería algo que se ve distinto.
const AUX_CAUSES: [AuxChunk; 5] = [
    AuxChunk::Animation,
    AuxChunk::Icc,
    AuxChunk::CodePoints,
    AuxChunk::Gamma,
    AuxChunk::Chromaticities,
];

/// Texto de una causa de `AuxReport::appearance`; cualquier otro chunk, `chunk ignorado`.
pub fn aux_cause(chunk: AuxChunk) -> &'static str {
    match chunk {
        AuxChunk::Animation => "APNG: solo el 1.er fotograma",
        AuxChunk::Icc => "perfil de color ICC ignorado",
        AuxChunk::CodePoints => "espacio de color cICP ignorado",
        AuxChunk::Gamma => "gamma no sRGB ignorada",
        AuxChunk::Chromaticities => "primarios no sRGB ignorados",
        _ => "chunk ignorado",
    }
}

/// Causa de la copia de un `.aseprite` con datos que Pixoko no guarda (specs/hito-3-aseprite.md).
const ASEPRITE_CAUSE: &str = "datos del .aseprite que Pixoko no guarda";

/// `true` si `reason` es exactamente una de las causas (sin conversiones), o la de un `.aseprite`
/// con datos que Pixoko no guarda: se avisa en la forma corta.
pub(crate) fn is_aux_cause(reason: &str) -> bool {
    AUX_CAUSES.iter().any(|&chunk| aux_cause(chunk) == reason) || reason.starts_with(ASEPRITE_CAUSE)
}

/// Rótulo de lo que se ignoró al leer un `.aseprite`; `None` para lo que no se avisa.
fn ignored_label(kind: Ignored) -> Option<&'static str> {
    Some(match kind {
        Ignored::Tags => "etiquetas",
        Ignored::UserData => "datos de usuario",
        Ignored::Slices => "cortes",
        Ignored::ExternalFiles => "ficheros externos",
        Ignored::CelExtra => "extra de celda",
        Ignored::Mask => "máscara",
        Ignored::Path => "trazo",
        Ignored::Tileset => "conjunto de tiles",
        Ignored::ColorProfile => "perfil de color",
        Ignored::BackgroundTransparent => "índice transparente en la capa de fondo",
        Ignored::Unknown => "chunks desconocidos",
        // La paleta de un RGBA no se pierde nada que se vea: no se avisa.
        _ => return None,
    })
}

/// Lo ignorado de un `.aseprite` en texto (`labels (2), data de usuario`), sin lo que no se
/// avisa. Vacío si no hay nada.
fn describe_ignored(report: &AsepriteReport) -> String {
    report
        .ignored
        .iter()
        .filter_map(|&(kind, count)| {
            let label = ignored_label(kind)?;
            Some(if count > 1 {
                format!("{label} ({count})")
            } else {
                label.to_string()
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// La razón de la copia de un `.aseprite` con `lossy` (`None` si se entendió entero).
pub fn aseprite_copy_reason(report: &AsepriteReport) -> Option<String> {
    if !report.lossy {
        return None;
    }
    let mut parts = Vec::new();
    let ignored = describe_ignored(report);
    if !ignored.is_empty() {
        parts.push(ignored);
    }
    if report.frames_in_file > 1 {
        parts.push(format!("solo el fotograma 1 de {}", report.frames_in_file));
    }
    Some(if parts.is_empty() {
        ASEPRITE_CAUSE.to_string()
    } else {
        format!("{ASEPRITE_CAUSE}: {}", parts.join(", "))
    })
}

/// Aviso al abrir un `.aseprite`: nombre, tamaño, capas y modo; y, si se descartó algo, qué y que
/// `Ctrl+S` guardará una copia.
pub fn describe_open_aseprite(name: &str, document: &Document, report: &AsepriteReport) -> String {
    let mode = match document.mode() {
        Mode::Indexed => "indexado",
        Mode::Gray => "gris",
        Mode::GrayAlpha => "gris con alfa",
        Mode::Rgba => "RGBA",
    };
    let layers = document.layers().len();
    let mut text = format!(
        "abierto {name} · {}×{} · {layers} {} · {mode}",
        document.width(),
        document.height(),
        if layers == 1 { "capa" } else { "capas" },
    );
    let ignored = describe_ignored(report);
    if !ignored.is_empty() {
        text.push_str(&format!(" · se ignoran: {ignored}"));
    }
    if report.frames_in_file > 1 {
        text.push_str(&format!(
            " · solo se abre el fotograma 1 de {}",
            report.frames_in_file
        ));
    }
    if report.lossy {
        text.push_str(" · Ctrl+S guardará una copia");
    }
    text
}

/// Rótulos de los chunks que el aviso nombra, en el orden de `AuxChunk`: `Animation` solo si no es
/// ya la causa (el color, la resolución y la fecha se quedan en `AuxReport`).
fn aux_labels(aux: &AuxReport) -> Vec<&'static str> {
    aux.found
        .iter()
        .filter_map(|&chunk| match chunk {
            AuxChunk::Text => Some("texto"),
            AuxChunk::Exif => Some("Exif"),
            AuxChunk::Animation if aux.appearance != Some(AuxChunk::Animation) => Some("animación"),
            AuxChunk::Other => Some("otros"),
            _ => None,
        })
        .collect()
}

/// Aviso al abrir: nombre, tamaño, modo y, si hubo conversiones, cuáles y que `Ctrl+S` guardará
/// una copia. Solo lo usan las pruebas (la app llama a `describe_open_aux`).
#[cfg(test)]
pub fn describe_open(name: &str, image: &Image, report: &[Conversion]) -> String {
    describe_open_aux(name, image, report, &AuxReport::default())
}

/// `describe_open` con los chunks auxiliares ignorados: con una causa de copia, va delante
/// (`{cause}; Ctrl+S guardará a_2 copy · …`) y sustituye la coletilla de las conversiones; los
/// chunks de texto, Exif, animación y otros se nombran al final (` · ignorados: text_2, Exif`).
pub fn describe_open_aux(
    name: &str,
    image: &Image,
    report: &[Conversion],
    aux: &AuxReport,
) -> String {
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
        text.push_str(&format!(" · convertido: {}", describe_report(report)));
        if aux.appearance.is_none() {
            text.push_str(" · Ctrl+S guardará una copia");
        }
    }
    let labels = aux_labels(aux);
    if !labels.is_empty() {
        text.push_str(&format!(" · ignorados: {}", labels.join(", ")));
    }
    match aux.appearance {
        Some(cause) => format!("{}{COPY_TAIL} · {text}", aux_cause(cause)),
        None => text,
    }
}

#[cfg(test)]
mod tests;
