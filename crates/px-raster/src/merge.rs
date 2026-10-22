//! Combinar y aplanar capas como comandos (specs/hito-3-combinar.md, unidad 3.5): los
//! constructores calculan la imagen con la misma composición que pinta el lienzo
//! ([`compose`] / [`compose_layers`]) y devuelven un `Command::MergeLayers` para
//! `Document::apply`, igual que `line_command` y `fill_command` devuelven el suyo. El núcleo
//! no puede componer (no depende de este crate): por eso la imagen viaja en el comando, con el
//! `state_id` sobre el que se calculó.

use std::error::Error;
use std::fmt;

use px_core::{Command, Document, DocumentError, LayerId};

use crate::compose::{ComposeError, compose, compose_layers};

/// Por qué no se pudo construir el comando.
#[derive(Debug, PartialEq)]
pub enum MergeError {
    /// La capa de arriba es la de más abajo: no hay con qué combinarla.
    NoLayerBelow,
    /// Combinar abajo exige las dos capas visibles; esta está oculta.
    Hidden(LayerId),
    /// No hay al menos dos capas que combinar (o ninguna visible que aplanar).
    NothingToMerge,
    /// Falló la composición.
    Compose(ComposeError),
    /// El documento rechazó las capas pedidas.
    Document(DocumentError),
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::NoLayerBelow => write!(f, "no hay capa debajo con la que combinar"),
            MergeError::Hidden(_) => write!(f, "una capa oculta no se combina"),
            MergeError::NothingToMerge => write!(f, "no hay capas que combinar"),
            MergeError::Compose(error) => write!(f, "{error}"),
            MergeError::Document(error) => write!(f, "{error}"),
        }
    }
}

impl Error for MergeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MergeError::Compose(error) => Some(error),
            MergeError::Document(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ComposeError> for MergeError {
    fn from(error: ComposeError) -> MergeError {
        MergeError::Compose(error)
    }
}

impl From<DocumentError> for MergeError {
    fn from(error: DocumentError) -> MergeError {
        MergeError::Document(error)
    }
}

/// Une la capa `upper` con la de justo debajo: la de abajo recibe la composición de las dos
/// (cada una con su opacidad y su modo, sobre vacío) y `upper` desaparece. Las dos tienen que
/// estar visibles; el bloqueo es política de la sesión y no se mira aquí.
pub fn merge_down_command(doc: &Document, upper: LayerId) -> Result<Command, MergeError> {
    let at = doc
        .layers()
        .iter()
        .position(|layer| layer.id() == upper)
        .ok_or(DocumentError::UnknownLayer(upper))?;
    let below = at.checked_sub(1).ok_or(MergeError::NoLayerBelow)?;
    let (Some(lower), Some(top)) = (doc.layers().get(below), doc.layers().get(at)) else {
        return Err(MergeError::NoLayerBelow);
    };
    for layer in [top, lower] {
        if !layer.visible() {
            return Err(MergeError::Hidden(layer.id()));
        }
    }
    let target = lower.id();
    let plan = doc.plan_merge(target, &[upper])?;
    let image = if plan.all_empty {
        None
    } else {
        Some(compose_layers(doc, &[target, upper])?)
    };
    Ok(Command::MergeLayers {
        target,
        remove: vec![upper],
        image,
        based_on: doc.state_id(),
    })
}

/// Une todas las capas visibles en la visible más baja; las ocultas se quedan como están.
pub fn merge_visible_command(doc: &Document) -> Result<Command, MergeError> {
    merge_visible_with(doc, false)
}

/// Deja una sola capa con lo que se ve: une las visibles en la visible más baja y quita las
/// ocultas.
pub fn flatten_command(doc: &Document) -> Result<Command, MergeError> {
    merge_visible_with(doc, true)
}

fn merge_visible_with(doc: &Document, drop_hidden: bool) -> Result<Command, MergeError> {
    let layers = doc.layers();
    let mut visible = layers.iter().filter(|layer| layer.visible());
    let target = visible.next().ok_or(MergeError::NothingToMerge)?.id();
    let others_visible = visible.count();
    let enough = if drop_hidden {
        layers.len() >= 2
    } else {
        others_visible >= 1
    };
    if !enough {
        return Err(MergeError::NothingToMerge);
    }
    let remove: Vec<LayerId> = layers
        .iter()
        .filter(|layer| layer.id() != target && (drop_hidden || layer.visible()))
        .map(|layer| layer.id())
        .collect();
    doc.plan_merge(target, &remove)?;
    let any_image = layers
        .iter()
        .any(|layer| layer.visible() && doc.image(layer.cel()).is_some());
    let image = if any_image { Some(compose(doc)?) } else { None };
    Ok(Command::MergeLayers {
        target,
        remove,
        image,
        based_on: doc.state_id(),
    })
}
