//! Edición de la paleta de una imagen indexada (specs/hito-2-paleta-editar.md).
//!
//! `check` valida y decide si el comando cambia algo; `build` reserva y construye el paso de
//! historia; `PaletteStep::redo` lo aplica (y rehace) y `PaletteStep::undo` lo deshace. Después
//! de `check` y `build`, aplicar y deshacer no reservan ni pueden fallar.

use std::error::Error;
use std::fmt;
use std::mem::size_of;

use crate::palette::is_permutation;
use crate::{Command, DocumentError, Image, ImageId, Mode, Palette, Rgba};

/// Error al editar la paleta de una imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PaletteEditError {
    /// La imagen no es indexada.
    NotIndexed { mode: Mode },
    /// Un índice es mayor o igual que la longitud de la paleta.
    IndexOutOfRange { index: u8, len: usize },
    /// `order` no tiene la longitud de la paleta, repite un índice o se sale de ella.
    BadPermutation,
    /// `AddPaletteEntry` con 256 entradas.
    PaletteFull,
    /// `RemovePaletteEntry` con `into` igual a la entrada quitada.
    RemoveIntoSelf { index: u8 },
    /// Quitar la única entrada de la paleta.
    LastEntry,
    /// `RotateColors` con un rango que se sale de la paleta.
    RangeOutOfPalette {
        start: u8,
        len: u16,
        palette_len: usize,
    },
}

impl fmt::Display for PaletteEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaletteEditError::NotIndexed { mode } => {
                write!(f, "la imagen es de modo {mode:?}, no indexada")
            }
            PaletteEditError::IndexOutOfRange { index, len } => {
                write!(f, "índice {index} fuera de una paleta de {len} colores")
            }
            PaletteEditError::BadPermutation => write!(
                f,
                "el orden no es una permutación de las entradas de la paleta"
            ),
            PaletteEditError::PaletteFull => write!(
                f,
                "la paleta ya tiene el máximo de {} colores",
                Palette::MAX_COLORS
            ),
            PaletteEditError::RemoveIntoSelf { index } => write!(
                f,
                "la entrada {index} no puede pasar sus píxeles a sí misma"
            ),
            PaletteEditError::LastEntry => {
                write!(f, "no se puede quitar la única entrada de la paleta")
            }
            PaletteEditError::RangeOutOfPalette {
                start,
                len,
                palette_len,
            } => write!(
                f,
                "el rango de {len} colores desde el {start} se sale de una paleta de {palette_len} colores"
            ),
        }
    }
}

impl Error for PaletteEditError {}

/// Paso de historia de un comando de paleta. Es del documento entero: se aplica al lienzo y a
/// cada celda con imagen (spec 3.1, N-04); lo que depende de los píxeles (`Remove`) guarda una
/// máscara por celda. Lo que guarda cada variante es lo mínimo para aplicar y deshacer sin
/// reservar (spec, punto 8).
#[derive(Clone, Debug)]
pub(crate) enum PaletteStep {
    /// Color de la entrada antes y después.
    Color {
        index: u8,
        before: Rgba,
        after: Rgba,
    },
    /// Índice transparente antes y después.
    Transparent {
        before: Option<u8>,
        after: Option<u8>,
    },
    /// Entrada añadida al final.
    Append { color: Rgba },
    /// Permutación `adelante[antiguo] = nuevo` (`move` y `permute`).
    Reorder { forward: Vec<u8> },
    /// Entrada quitada: su color, el índice transparente de antes y, por cada imagen que usaba
    /// la entrada, los píxeles que la usaban.
    Remove {
        index: u8,
        into: u8,
        color: Rgba,
        transparent: Option<u8>,
        masks: Vec<(ImageId, Vec<u64>)>,
    },
    /// Rotación de `len` entradas desde `start`, `k` posiciones hacia índices mayores.
    Rotate { start: u8, len: u16, k: u16 },
}

impl PaletteStep {
    /// `true` si deshacer el paso agranda la paleta (hace falta un hueco en su `Vec`).
    pub(crate) fn grows_on_undo(&self) -> bool {
        matches!(self, PaletteStep::Remove { .. })
    }

    /// `true` si aplicar o rehacer el paso agranda la paleta.
    pub(crate) fn grows_on_redo(&self) -> bool {
        matches!(self, PaletteStep::Append { .. })
    }

    /// Memoria del paso: su tamaño más lo que reserva en el montón (capacidad real).
    pub(crate) fn cost(&self) -> usize {
        let heap = match self {
            PaletteStep::Reorder { forward } => forward.capacity(),
            PaletteStep::Remove { masks, .. } => masks
                .capacity()
                .saturating_mul(size_of::<(ImageId, Vec<u64>)>())
                .saturating_add(masks.iter().fold(0usize, |total, (_, mask)| {
                    total.saturating_add(mask.capacity().saturating_mul(size_of::<u64>()))
                })),
            _ => 0,
        };
        size_of::<PaletteStep>().saturating_add(heap)
    }

    /// Aplica el paso a `image` (la primera vez, y al rehacer). No reserva ni falla.
    pub(crate) fn redo(&self, image: &mut Image) {
        match self {
            PaletteStep::Color { index, after, .. } => {
                image.set_palette_entry(*index, *after);
            }
            PaletteStep::Transparent { after, .. } => {
                image.set_transparent(*after);
            }
            PaletteStep::Append { color } => {
                image.push_palette_entry(*color);
            }
            PaletteStep::Reorder { forward } => {
                image.reorder_palette(forward);
            }
            PaletteStep::Remove { index, into, .. } => {
                image.remove_palette_entry(*index, *into);
            }
            PaletteStep::Rotate { start, len, k } => {
                image.rotate_palette_range(*start, *len, *k, true);
            }
        }
    }

    /// Deshace el paso en `image`, que en la historia es la de id `id`. No reserva ni falla.
    pub(crate) fn undo(&self, id: ImageId, image: &mut Image) {
        match self {
            PaletteStep::Color { index, before, .. } => {
                image.set_palette_entry(*index, *before);
            }
            PaletteStep::Transparent { before, .. } => {
                image.set_transparent(*before);
            }
            PaletteStep::Append { .. } => {
                image.pop_palette_entry();
            }
            PaletteStep::Reorder { forward } => {
                let mut inverse = [0u8; Palette::MAX_COLORS];
                for (old, &new) in forward.iter().enumerate() {
                    if let (Some(slot), Ok(old)) =
                        (inverse.get_mut(usize::from(new)), u8::try_from(old))
                    {
                        *slot = old;
                    }
                }
                if let Some(inverse) = inverse.get(..forward.len()) {
                    image.reorder_palette(inverse);
                }
            }
            PaletteStep::Remove {
                index,
                color,
                transparent,
                masks,
                ..
            } => {
                let mask: &[u64] = masks
                    .iter()
                    .find(|(mask_id, _)| *mask_id == id)
                    .map_or(&[], |(_, mask)| mask.as_slice());
                image.restore_palette_entry(*index, *color, *transparent, mask);
            }
            PaletteStep::Rotate { start, len, k } => {
                image.rotate_palette_range(*start, *len, *k, false);
            }
        }
    }
}

/// La imagen sobre la que actúa un comando de paleta, o `None` si no lo es.
pub(crate) fn command_image(command: &Command) -> Option<ImageId> {
    match command {
        Command::SetPaletteColor { image, .. }
        | Command::MovePaletteEntry { image, .. }
        | Command::PermutePalette { image, .. }
        | Command::AddPaletteEntry { image, .. }
        | Command::RemovePaletteEntry { image, .. }
        | Command::SetTransparentIndex { image, .. }
        | Command::RotateColors { image, .. } => Some(*image),
        _ => None,
    }
}

fn out_of_range(index: u8, len: usize) -> PaletteEditError {
    PaletteEditError::IndexOutOfRange { index, len }
}

fn check_index(index: u8, len: usize) -> Result<(), PaletteEditError> {
    if usize::from(index) < len {
        Ok(())
    } else {
        Err(out_of_range(index, len))
    }
}

/// `adelante` de `MovePaletteEntry(from, to)`: la entrada `from` acaba en `to`.
fn move_table(len: usize, from: u8, to: u8) -> [u8; Palette::MAX_COLORS] {
    let mut table = [0u8; Palette::MAX_COLORS];
    for (slot, value) in table.iter_mut().zip(0..=u8::MAX) {
        *slot = value;
    }
    let (from, to) = (usize::from(from), usize::from(to));
    for old in 0..len.min(Palette::MAX_COLORS) {
        let new = if old == from {
            to
        } else if from < to && old > from && old <= to {
            old - 1
        } else if from > to && old >= to && old < from {
            old + 1
        } else {
            old
        };
        if let (Some(slot), Ok(new)) = (table.get_mut(old), u8::try_from(new)) {
            *slot = new;
        }
    }
    table
}

/// `steps` módulo `len` (con `len > 0`), siempre en `0..len`.
fn rotation(steps: i32, len: u16) -> u16 {
    let len = i64::from(len.max(1));
    u16::try_from(i64::from(steps).rem_euclid(len)).unwrap_or(0)
}

/// Valida el comando contra la imagen, sin tocar nada. `Ok(true)` si cambia algo y `Ok(false)`
/// si no (spec, puntos 4 y 5). `command` debe ser un comando de paleta.
pub(crate) fn check(image: &Image, command: &Command) -> Result<bool, PaletteEditError> {
    let Some(palette) = image.palette() else {
        return Err(PaletteEditError::NotIndexed { mode: image.mode() });
    };
    let len = palette.len();
    match command {
        Command::SetPaletteColor { index, color, .. } => {
            check_index(*index, len)?;
            Ok(palette.get(*index).is_ok_and(|current| current != *color))
        }
        Command::SetTransparentIndex { index, .. } => {
            if let Some(index) = index {
                check_index(*index, len)?;
            }
            Ok(palette.transparent_index() != *index)
        }
        Command::AddPaletteEntry { .. } => {
            if len >= Palette::MAX_COLORS {
                return Err(PaletteEditError::PaletteFull);
            }
            Ok(true)
        }
        Command::MovePaletteEntry { from, to, .. } => {
            check_index(*from, len)?;
            check_index(*to, len)?;
            Ok(from != to)
        }
        Command::PermutePalette { order, .. } => {
            if !is_permutation(order, len) {
                return Err(PaletteEditError::BadPermutation);
            }
            Ok(order.iter().zip(0..=u8::MAX).any(|(&old, new)| old != new))
        }
        Command::RemovePaletteEntry { index, into, .. } => {
            check_index(*index, len)?;
            check_index(*into, len)?;
            if len < 2 {
                return Err(PaletteEditError::LastEntry);
            }
            if index == into {
                return Err(PaletteEditError::RemoveIntoSelf { index: *index });
            }
            Ok(true)
        }
        Command::RotateColors {
            start,
            len: span,
            steps,
            ..
        } => {
            let end = usize::from(*start) + usize::from(*span);
            if end > len {
                return Err(PaletteEditError::RangeOutOfPalette {
                    start: *start,
                    len: *span,
                    palette_len: len,
                });
            }
            if *span < 2 {
                return Ok(false);
            }
            let k = usize::from(rotation(*steps, *span));
            if k == 0 {
                return Ok(false);
            }
            let colors = palette.colors();
            let span = usize::from(*span);
            let first = usize::from(*start);
            // Rotar `k` a la derecha: la entrada `i` recibe la de `i − k` (módulo `span`).
            let changes = (0..span).any(|i| {
                let source = (i + span - k) % span;
                colors.get(first + i) != colors.get(first + source)
            });
            Ok(changes)
        }
        _ => Ok(false),
    }
}

/// Construye el paso de un comando ya validado por `check` que cambia algo. Toda la memoria
/// del paso (la tabla de `Move`/`Permute`, la máscara de `Remove`) se reserva aquí, antes de
/// modificar nada; el único error posible es `OutOfMemory`.
pub(crate) fn build(
    images: &[(ImageId, &Image)],
    command: &Command,
) -> Result<PaletteStep, DocumentError> {
    let image = images
        .first()
        .map(|(_, image)| *image)
        .ok_or(DocumentError::PaletteEdit(PaletteEditError::BadPermutation))?;
    let palette =
        image
            .palette()
            .ok_or(DocumentError::PaletteEdit(PaletteEditError::NotIndexed {
                mode: image.mode(),
            }))?;
    let len = palette.len();
    let invalid = || DocumentError::PaletteEdit(PaletteEditError::BadPermutation);
    match command {
        Command::SetPaletteColor { index, color, .. } => Ok(PaletteStep::Color {
            index: *index,
            before: palette.get(*index).unwrap_or_default(),
            after: *color,
        }),
        Command::SetTransparentIndex { index, .. } => Ok(PaletteStep::Transparent {
            before: palette.transparent_index(),
            after: *index,
        }),
        Command::AddPaletteEntry { color, .. } => Ok(PaletteStep::Append { color: *color }),
        Command::MovePaletteEntry { from, to, .. } => {
            let table = move_table(len, *from, *to);
            let forward = table.get(..len).ok_or_else(invalid)?;
            Ok(PaletteStep::Reorder {
                forward: exact_copy(forward)?,
            })
        }
        Command::PermutePalette { order, .. } => {
            let mut forward = [0u8; Palette::MAX_COLORS];
            for (new, &old) in order.iter().enumerate() {
                if let (Some(slot), Ok(new)) =
                    (forward.get_mut(usize::from(old)), u8::try_from(new))
                {
                    *slot = new;
                }
            }
            let forward = forward.get(..len).ok_or_else(invalid)?;
            Ok(PaletteStep::Reorder {
                forward: exact_copy(forward)?,
            })
        }
        Command::RemovePaletteEntry { index, into, .. } => {
            let mut masks: Vec<(ImageId, Vec<u64>)> = Vec::new();
            masks
                .try_reserve_exact(images.len())
                .map_err(|_| DocumentError::OutOfMemory)?;
            for (target, image) in images {
                if image.count_pixels_using(*index) == 0 {
                    continue;
                }
                let pixels = usize::try_from(image.width())
                    .ok()
                    .and_then(|w| w.checked_mul(usize::try_from(image.height()).ok()?))
                    .ok_or(DocumentError::OutOfMemory)?;
                let words = pixels.div_ceil(64);
                let mut mask: Vec<u64> = Vec::new();
                mask.try_reserve_exact(words)
                    .map_err(|_| DocumentError::OutOfMemory)?;
                mask.resize(words, 0);
                image.fill_mask(*index, &mut mask);
                masks.push((*target, mask));
            }
            Ok(PaletteStep::Remove {
                index: *index,
                into: *into,
                color: palette.get(*index).unwrap_or_default(),
                transparent: palette.transparent_index(),
                masks,
            })
        }
        Command::RotateColors {
            start,
            len: span,
            steps,
            ..
        } => Ok(PaletteStep::Rotate {
            start: *start,
            len: *span,
            k: rotation(*steps, *span),
        }),
        _ => Err(invalid()),
    }
}

/// Copia `source` en un `Vec` de capacidad exacta; `OutOfMemory` si no se puede reservar.
fn exact_copy(source: &[u8]) -> Result<Vec<u8>, DocumentError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(source.len())
        .map_err(|_| DocumentError::OutOfMemory)?;
    copy.extend_from_slice(source);
    Ok(copy)
}
