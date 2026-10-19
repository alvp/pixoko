//! Región sucia por bloques de 64×64 y registro de cambios por `state_id`
//! (specs/hito-2-zoom-baldosas.md, decisión 6). Puro: sin egui.

use std::collections::{HashMap, VecDeque};

use px_core::Command;

use crate::tiles::{BLOCK, TileRect, block_rect};
use crate::view::Size;

/// Bloques de `BLOCK`² de una imagen que han cambiado: un mapa de bits sobre su rejilla (2 KiB a
/// 8192²).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockSet {
    size: Size,
    /// Bloques por fila.
    cols: u32,
    rows: u32,
    bits: Vec<u64>,
}

impl BlockSet {
    /// Vacío, con la rejilla de una imagen de `width` × `height`.
    pub fn new(width: u32, height: u32) -> BlockSet {
        let cols = width.div_ceil(BLOCK);
        let rows = height.div_ceil(BLOCK);
        let words = (cols as usize * rows as usize).div_ceil(64);
        BlockSet {
            size: Size::new(width, height),
            cols,
            rows,
            bits: vec![0; words],
        }
    }

    /// Todos los bloques de la imagen.
    pub fn full(width: u32, height: u32) -> BlockSet {
        let mut set = BlockSet::new(width, height);
        let total = set.cols as usize * set.rows as usize;
        for (index, word) in set.bits.iter_mut().enumerate() {
            let in_word = total.saturating_sub(index * 64).min(64);
            *word = if in_word == 64 {
                u64::MAX
            } else {
                (1u64 << in_word) - 1
            };
        }
        set
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|word| *word == 0)
    }

    /// Bloques sucios.
    pub fn len(&self) -> usize {
        self.bits
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    /// Bytes que ocupa el mapa.
    pub fn bytes(&self) -> usize {
        self.bits.len() * 8
    }

    fn index(&self, bx: u32, by: u32) -> Option<usize> {
        (bx < self.cols && by < self.rows).then(|| by as usize * self.cols as usize + bx as usize)
    }

    fn set(&mut self, bx: u32, by: u32) {
        self.insert(bx, by);
    }

    /// Marca el bloque `(bx, by)` (coordenadas de bloque); fuera de la rejilla no hace nada.
    pub fn insert(&mut self, bx: u32, by: u32) {
        if let Some(index) = self.index(bx, by) {
            self.bits[index / 64] |= 1 << (index % 64);
        }
    }

    /// Quita el bloque `(bx, by)`.
    pub fn remove(&mut self, bx: u32, by: u32) {
        if let Some(index) = self.index(bx, by) {
            self.bits[index / 64] &= !(1 << (index % 64));
        }
    }

    /// El bloque `(bx, by)` (coordenadas de bloque) está sucio.
    pub fn contains(&self, bx: u32, by: u32) -> bool {
        self.index(bx, by)
            .is_some_and(|index| self.bits[index / 64] & (1 << (index % 64)) != 0)
    }

    /// Marca el bloque del píxel; fuera de la imagen no hace nada.
    pub fn add_pixel(&mut self, x: u32, y: u32) {
        if x < self.size.width && y < self.size.height {
            self.set(x / BLOCK, y / BLOCK);
        }
    }

    /// Marca los bloques que corta el rectángulo, recortado a la imagen.
    pub fn add_rect(&mut self, x: i64, y: i64, width: u64, height: u64) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = i128::from(x) + i128::from(width);
        let y1 = i128::from(y) + i128::from(height);
        let x1 = x1.min(i128::from(self.size.width));
        let y1 = y1.min(i128::from(self.size.height));
        if i128::from(x0) >= x1 || i128::from(y0) >= y1 {
            return;
        }
        let (x0, y0, x1, y1) = (x0 as u64, y0 as u64, x1 as u64, y1 as u64);
        let block = u64::from(BLOCK);
        for by in (y0 / block)..y1.div_ceil(block) {
            for bx in (x0 / block)..x1.div_ceil(block) {
                self.set(bx as u32, by as u32);
            }
        }
    }

    /// Añade los bloques de `other`. Con otro tamaño no hace nada.
    pub fn union(&mut self, other: &BlockSet) {
        if self.size != other.size {
            return;
        }
        for (word, other) in self.bits.iter_mut().zip(&other.bits) {
            *word |= other;
        }
    }

    /// Deja solo los bloques que cortan `rect`.
    pub fn clip(&mut self, rect: TileRect) {
        for by in 0..self.rows {
            for bx in 0..self.cols {
                if self.contains(bx, by) && block_rect(bx, by, self.size).intersect(&rect).is_none()
                {
                    self.remove(bx, by);
                }
            }
        }
    }

    /// Para cada fila de bloques, cada serie máxima de bloques sucios contiguos que corta `rect`,
    /// en píxeles y recortada a `rect`. Orden por filas y por `x`.
    pub fn runs(&self, rect: TileRect) -> Vec<TileRect> {
        let mut runs = Vec::new();
        if rect.width == 0 || rect.height == 0 {
            return runs;
        }
        let block = u64::from(BLOCK);
        let bx0 = (u64::from(rect.x) / block) as u32;
        let bx1 = rect.right().div_ceil(block).min(u64::from(self.cols)) as u32;
        let by0 = (u64::from(rect.y) / block) as u32;
        let by1 = rect.bottom().div_ceil(block).min(u64::from(self.rows)) as u32;
        for by in by0..by1 {
            let mut bx = bx0;
            while bx < bx1 {
                if !self.contains(bx, by) {
                    bx += 1;
                    continue;
                }
                let start = bx;
                while bx < bx1 && self.contains(bx, by) {
                    bx += 1;
                }
                let first = block_rect(start, by, self.size);
                let last = block_rect(bx - 1, by, self.size);
                let run = TileRect {
                    x: first.x,
                    y: first.y,
                    width: (last.right() - u64::from(first.x)) as u32,
                    height: first.height,
                };
                runs.extend(run.intersect(&rect));
            }
        }
        runs
    }
}

/// Bloques que puede cambiar un comando, calculados **antes** de aplicarlo (superconjunto de lo
/// que cambia): `SetPixels` = los de sus píxeles; `FillSpans` = los de sus tramos; `Batch` = la
/// unión; mover, reordenar y añadir entradas de paleta = vacío (la imagen se ve igual); el resto
/// (y cualquier comando que no se conozca) = toda la imagen.
pub fn dirty_of(command: &Command, size: Size) -> BlockSet {
    let mut set = BlockSet::new(size.width, size.height);
    match command {
        Command::SetPixels { pixels, .. } => {
            for edit in pixels {
                set.add_pixel(edit.x, edit.y);
            }
        }
        Command::FillSpans { spans, .. } => {
            for span in spans {
                set.add_rect(
                    i64::from(span.x_start),
                    i64::from(span.y),
                    u64::from(span.x_end.saturating_sub(span.x_start)),
                    1,
                );
            }
        }
        Command::Batch(commands) => {
            for command in commands {
                set.union(&dirty_of(command, size));
            }
        }
        Command::MovePaletteEntry { .. }
        | Command::PermutePalette { .. }
        | Command::AddPaletteEntry { .. } => {}
        _ => return BlockSet::full(size.width, size.height),
    }
    set
}

/// Un cambio ya aplicado: el `state_id` resultante y los bloques que tocó.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub state: u64,
    pub blocks: BlockSet,
}

/// Qué bloques tocó cada paso de la historia, por el `state_id` que dejó. `undo` desde `s`
/// invalida `log[s]` y `redo` hacia `s'` invalida `log[s']`. Con un tope de memoria: olvida los
/// pasos más viejos (la clave ausente significa «toda la imagen»).
#[derive(Clone, Debug, Default)]
pub struct ChangeLog {
    entries: HashMap<u64, BlockSet>,
    order: VecDeque<u64>,
    bytes: usize,
}

impl ChangeLog {
    /// Tope de memoria del registro.
    pub const LIMIT_BYTES: usize = 4 << 20;

    pub fn new() -> ChangeLog {
        ChangeLog::default()
    }

    fn cost(blocks: &BlockSet) -> usize {
        blocks.bytes() + 32
    }

    /// Anota los bloques del paso que dejó el estado `state`; si ya había uno, lo sustituye.
    pub fn record(&mut self, state: u64, blocks: BlockSet) {
        if let Some(old) = self.entries.remove(&state) {
            self.bytes -= ChangeLog::cost(&old);
            self.order.retain(|id| *id != state);
        }
        self.bytes += ChangeLog::cost(&blocks);
        self.entries.insert(state, blocks);
        self.order.push_back(state);
        while self.bytes > ChangeLog::LIMIT_BYTES
            && let Some(oldest) = self.order.pop_front()
        {
            if let Some(old) = self.entries.remove(&oldest) {
                self.bytes -= ChangeLog::cost(&old);
            }
        }
    }

    pub fn get(&self, state: u64) -> Option<&BlockSet> {
        self.entries.get(&state)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}
