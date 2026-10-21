//! Los 14 iconos propios de la interfaz (specs/interfaz-iconos.md): PNG de 16 × 16 incrustados con
//! `include_bytes!`, decodificados una vez al arrancar con `px-formats` y subidos como texturas de
//! vecino más cercano. `px-ui` los pinta tintados y a escala entera (`IconSet`).
//!
//! Se pueden retocar con Pixoko: vale cualquier PNG que `read_png` lea (RGBA, indexado con
//! transparencia, gris con alfa) y solo cuenta el alfa: el color del dibujo se normaliza a blanco
//! para que la tinta del tema sea la que se ve.

use std::fmt;

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use px_formats::read_png;
use px_ui::icons::{ICON_PX, Icon, IconSet};
use px_ui::texture::to_rgba8;

/// Un icono que no se puede cargar (un PNG corrupto o de otro tamaño).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconError {
    /// `px-formats` no pudo leer el PNG.
    Png { icon: String, message: String },
    /// El PNG no mide 16 × 16.
    Size {
        icon: String,
        width: u32,
        height: u32,
    },
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IconError::Png { icon, message } => {
                write!(f, "icono «{icon}»: PNG no válido ({message})")
            }
            IconError::Size {
                icon,
                width,
                height,
            } => write!(
                f,
                "icono «{icon}»: mide {width} × {height} y debe medir {ICON_PX} × {ICON_PX}"
            ),
        }
    }
}

impl std::error::Error for IconError {}

/// Los bytes del PNG incrustado de un icono. Un `match` exhaustivo: añadir un `Icon` sin su PNG no
/// compila.
pub fn embedded(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::Pencil => include_bytes!("../../../assets/icons/lapiz.png"),
        Icon::Line => include_bytes!("../../../assets/icons/linea.png"),
        Icon::Fill => include_bytes!("../../../assets/icons/relleno.png"),
        Icon::Dropper => include_bytes!("../../../assets/icons/cuentagotas.png"),
        Icon::Eraser => include_bytes!("../../../assets/icons/borrador.png"),
        Icon::ZoomIn => include_bytes!("../../../assets/icons/acercar.png"),
        Icon::ZoomOut => include_bytes!("../../../assets/icons/alejar.png"),
        Icon::ZoomFit => include_bytes!("../../../assets/icons/ajustar.png"),
        Icon::ZoomActual => include_bytes!("../../../assets/icons/uno-a-uno.png"),
        Icon::Add => include_bytes!("../../../assets/icons/anadir.png"),
        Icon::Remove => include_bytes!("../../../assets/icons/quitar.png"),
        Icon::RotateLeft => include_bytes!("../../../assets/icons/rotar-izquierda.png"),
        Icon::RotateRight => include_bytes!("../../../assets/icons/rotar-derecha.png"),
        Icon::Transparent => include_bytes!("../../../assets/icons/transparente.png"),
    }
}

/// Decodifica el PNG de `stem` a la imagen de la textura: 16 × 16, el color a blanco donde hay
/// alfa y a nada donde no (solo cuenta el alfa).
pub fn decode(stem: &str, bytes: &[u8]) -> Result<ColorImage, IconError> {
    let png = read_png(bytes).map_err(|e| IconError::Png {
        icon: stem.to_string(),
        message: e.to_string(),
    })?;
    let (width, height) = (png.image.width(), png.image.height());
    if i64::from(width) != ICON_PX || i64::from(height) != ICON_PX {
        return Err(IconError::Size {
            icon: stem.to_string(),
            width,
            height,
        });
    }
    let mut rgba = to_rgba8(&png.image);
    for px in rgba.as_chunks_mut::<4>().0 {
        let value = if px[3] > 0 { 255 } else { 0 };
        px[..3].fill(value);
    }
    let side = ICON_PX as usize;
    Ok(ColorImage::from_rgba_unmultiplied([side, side], &rgba))
}

/// Las texturas de los iconos y el `IconSet` que las nombra. Las texturas viven mientras viva esto.
pub struct Icons {
    pub set: IconSet,
    _handles: Vec<TextureHandle>,
}

impl Icons {
    /// Sin ninguna textura: los botones se pintan sin icono (pruebas y `App::new`).
    pub fn empty() -> Icons {
        Icons {
            set: IconSet::default(),
            _handles: Vec::new(),
        }
    }
}

/// Carga los 14 iconos incrustados.
pub fn load(ctx: &egui::Context) -> Result<Icons, IconError> {
    load_with(ctx, &|icon| embedded(icon).to_vec())
}

/// Como [`load`] con los bytes que da `source` (las pruebas meten PNG rotos o de otro tamaño).
pub fn load_with(
    ctx: &egui::Context,
    source: &dyn Fn(Icon) -> Vec<u8>,
) -> Result<Icons, IconError> {
    let mut handles = Vec::with_capacity(Icon::ALL.len());
    for icon in Icon::ALL {
        let stem = icon.file_stem();
        let image = decode(stem, &source(icon))?;
        handles.push(ctx.load_texture(format!("icono-{stem}"), image, TextureOptions::NEAREST));
    }
    let set = IconSet::new(|icon| handles[icon.index()].id());
    Ok(Icons {
        set,
        _handles: handles,
    })
}

#[cfg(test)]
mod tests;
