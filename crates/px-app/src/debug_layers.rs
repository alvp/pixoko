//! Ayuda de depuración **descartable** (specs/hito-3-capas-vista.md, decisión 8): hasta que el
//! panel de capas (3.4) permita crearlas, `PIXOKO_CAPAS=N[,K]` crea `N` capas al abrir un
//! documento (al arrancar, al abrir un fichero y con `Nueva imagen…`) y deja activa la `K`
//! (de 1 a `N`, de abajo arriba; por defecto la 1).
//!
//! - `N` va de 2 a 256: `Capa 2` a `Capa N` encima de la que ya había.
//! - En RGBA, la capa `i` lleva una banda de un cuarto del ancho (desplazada un cuarto por capa;
//!   rojo, verde y azul por turnos) y las capas pares, opacidad 128: así se ve la composición sin
//!   panel. En los demás modos las capas quedan vacías.
//! - Se aplica con comandos reales: deshacer las quita. El documento queda sin modificar.
//! - Un valor no válido se ignora con un aviso al arrancar; un modo sin valor transparente
//!   (gris, indexado sin índice transparente) avisa y deja el documento intacto.
//!
//! Se borra entero (este módulo y sus dos llamadas en `lib.rs`) con la unidad 3.4.

use px_core::{Command, Mode, Pixel, Rgba, Span};

use crate::session::Session;

pub const ENV_LAYERS: &str = "PIXOKO_CAPAS";

/// Lo que pide la variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayersRequest {
    /// Sin variable (o vacía).
    None,
    /// Un valor que no vale: el aviso de arranque.
    Invalid(String),
    /// `count` capas en total y la `active` (desde 1) activa.
    Create { count: usize, active: usize },
}

/// Lee el valor de [`ENV_LAYERS`].
pub fn parse_layers(value: Option<&str>) -> LayersRequest {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return LayersRequest::None;
    };
    let invalid = || {
        LayersRequest::Invalid(format!(
            "{ENV_LAYERS}={text} no es válido (N de 2 a 256 y K de 1 a N): se ignora"
        ))
    };
    let (count, active) = match text.split_once(',') {
        Some((count, active)) => (count, Some(active)),
        None => (text, None),
    };
    let Ok(count) = count.parse::<usize>() else {
        return invalid();
    };
    let active = match active {
        Some(active) => match active.parse::<usize>() {
            Ok(active) => active,
            Err(_) => return invalid(),
        },
        None => 1,
    };
    if (2..=px_core::Document::MAX_LAYERS).contains(&count) && (1..=count).contains(&active) {
        LayersRequest::Create { count, active }
    } else {
        invalid()
    }
}

/// Crea hasta `count` capas y deja activa la `active` (desde 1). `Err` con el aviso si el modo no
/// admite capas: entonces el documento no cambia.
pub fn apply_debug_layers(
    session: &mut Session,
    count: usize,
    active: usize,
) -> Result<(), String> {
    let rgba = session.document.mode() == Mode::Rgba;
    let (width, height) = (session.document.width(), session.document.height());
    let colors = [
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
    ];
    while session.document.layers().len() < count {
        let at = session.document.layers().len();
        let number = at + 1;
        session
            .apply_layer(Command::AddLayer {
                name: format!("Capa {number}"),
                at,
            })
            .map_err(|error| error.to_string())?;
        if !rgba {
            continue;
        }
        let layer = &session.document.layers()[at];
        let (id, cel) = (layer.id(), layer.cel());
        let band = (width / 4).max(1);
        let x_start = ((number - 2) as u32 % 4) * band;
        let x_end = (x_start + band).min(width);
        if x_start < x_end {
            let spans = (0..height).map(|y| Span { y, x_start, x_end }).collect();
            let color = colors[(number - 2) % 3];
            session
                .apply_layer(Command::FillSpans {
                    image: cel,
                    spans,
                    pixel: Pixel::Rgba(color),
                })
                .map_err(|error| error.to_string())?;
        }
        if number.is_multiple_of(2) {
            session
                .apply_layer(Command::SetLayerOpacity {
                    layer: id,
                    opacity: 128,
                })
                .map_err(|error| error.to_string())?;
        }
    }
    if let Some(layer) = session.document.layers().get(active - 1) {
        let id = layer.id();
        session
            .set_active_layer(id)
            .map_err(|error| error.to_string())?;
    }
    session.mark_unmodified();
    Ok(())
}
