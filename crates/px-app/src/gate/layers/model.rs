//! Modelo independiente de la pila de capas de la puerta del hito 3 (specs/hito-3-puerta.md,
//! «Modelo independiente de la pila»): capas con sus propiedades y una rejilla de celdas por capa
//! (RGBA `[r, g, b, a]`; indexado `[índice, 0, 0, 0]`), la capa activa y órdenes que se aplican con
//! código ingenuo (relleno por cola, líneas solo horizontales, verticales o a 45°).
//!
//! **No reimplementa la composición**: las fórmulas de mezcla las fijan 3.2 y 3.5 (17 + 29 goldens
//! de Aseprite). La composición esperada sale de `compose` sobre un documento que el modelo
//! construye con `Document::from_layers`, un camino distinto del de la sesión (sin historia ni
//! comandos); lo demás (propiedades, orden, contenido de cada celda, capa activa) sale solo del
//! modelo. Los vectores a mano de 3.2 se comprueban aparte con `Order::Vector`.

use px_core::{BlendMode, Document, Image, LayerSpec, Mode, Palette, Pixel, Rgba};
use px_raster::compose;

use crate::merge_flow::MergeKind;

/// Un píxel del modelo: RGBA `[r, g, b, a]` o indexado `[índice, 0, 0, 0]`.
pub type Cel4 = [u8; 4];

#[derive(Clone, Debug)]
pub struct LayerM {
    pub name_2: String,
    pub visible: bool,
    pub opacity_2: u8,
    pub blend_2: BlendMode,
    pub locked_2: bool,
    /// `None`: celda vacía (sin memoria). Con rejilla: `width × height` celdas.
    pub cel: Option<Vec<Cel4>>,
}

#[derive(Clone, Debug)]
pub struct Stack {
    pub width: usize,
    pub height: usize,
    pub mode_2: Mode,
    pub palette: Option<Palette>,
    pub layers_2: Vec<LayerM>,
    /// Posición (de abajo arriba) de la capa activa.
    pub active_2: usize,
}

/// Lo que una orden hizo en el modelo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    /// Hay paso de historia nuevo.
    pub changes: bool,
    /// La orden cambia la estructura de capas (nueva, duplicar, quitar, combinar, convertir): la
    /// capa activa tras deshacer o rehacer se sabe.
    pub structural: bool,
}

const NOTHING: Effect = Effect {
    changes: false,
    structural: false,
};
const CHANGES: Effect = Effect {
    changes: true,
    structural: false,
};
const STRUCTURE: Effect = Effect {
    changes: true,
    structural: true,
};

#[derive(Clone, Debug)]
pub enum Order {
    New,
    Duplicate,
    Remove,
    /// `+1` sube un puesto y `-1` baja (la activa sigue siendo la misma).
    Mover(i32),
    /// Elegir la capa activa: no es un paso de historia.
    Activate(usize),
    Visible(usize, bool),
    Lock(usize, bool),
    Rename(usize, String),
    Opacity(usize, u8),
    Blend(usize, BlendMode),
    Pencil(Vec<(i32, i32)>, Cel4),
    Line((i32, i32), (i32, i32), Cel4),
    Fill((i32, i32), Cel4),
    Eraser(Vec<(i32, i32)>),
    Merge(MergeKind),
    /// Conversión de **todas** las capas, exacta (sin permisos para perder datos).
    Convert(Mode),
    /// No cambia nada: exige que el píxel `(x, y)` de la composición sea este valor escrito a mano.
    Vector((u32, u32), Cel4),
}

/// Los píxeles de un segmento horizontal, vertical o a 45° (de `a` a `b`, ambos incluidos).
pub fn segment(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    assert!(
        dx == 0 || dy == 0 || dx.abs() == dy.abs(),
        "el modelo solo traza líneas horizontales, verticales o a 45°: {a:?} {b:?}"
    );
    let steps_2 = dx.abs().max(dy.abs());
    (0..=steps_2)
        .map(|i| (a.0 + dx.signum() * i, a.1 + dy.signum() * i))
        .collect()
}

/// Un trazo: la unión de los segmentos entre puntos consecutivos, sin repetir.
pub fn stroke(path_2: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut points = vec![path_2[0]];
    for par in path_2.windows(2) {
        points.extend(segment(par[0], par[1]).into_iter().skip(1));
    }
    points
}

impl Stack {
    /// El modelo sembrado con lo que hay en un documento (se usa al abrir y para leer la sesión).
    pub fn of_document(doc: &Document, active_2: usize) -> Stack {
        let (width, height) = (doc.width() as usize, doc.height() as usize);
        let layers_2 = doc
            .layers()
            .iter()
            .map(|layer_2| LayerM {
                name_2: layer_2.name().to_string(),
                visible: layer_2.visible(),
                opacity_2: layer_2.opacity(),
                blend_2: layer_2.blend(),
                locked_2: layer_2.locked(),
                cel: doc.image(layer_2.cel()).map(expand),
            })
            .collect();
        Stack {
            width,
            height,
            mode_2: doc.mode(),
            palette: doc.palette().cloned(),
            layers_2,
            active_2,
        }
    }

    /// El píxel transparente del modo (el de una celda nueva).
    pub fn transparent(&self) -> Cel4 {
        match self.mode_2 {
            Mode::Indexed => [
                self.palette
                    .as_ref()
                    .and_then(Palette::transparent_index)
                    .expect("el indexado de la prueba tiene índice transparente"),
                0,
                0,
                0,
            ],
            _ => [0, 0, 0, 0],
        }
    }

    /// El índice transparente del indexado, si lo hay.
    fn transparent_opt(&self) -> Option<u8> {
        self.palette.as_ref().and_then(Palette::transparent_index)
    }

    /// `New` y `Duplicate` se rechazan en un indexado sin índice transparente (3.4).
    fn can_create(&self) -> bool {
        !self.indexed_2() || self.transparent_opt().is_some()
    }

    /// El borrador necesita un valor de borrado: en indexado, el índice transparente.
    pub fn can_delete(&self) -> bool {
        !self.indexed_2() || self.transparent_opt().is_some()
    }

    /// La celda de la capa `i` con las vacías expandidas a transparente (la igualdad por contenido).
    pub fn grid(&self, i: usize) -> Vec<Cel4> {
        match &self.layers_2[i].cel {
            Some(cel) => cel.clone(),
            None => vec![self.transparent(); self.width * self.height],
        }
    }

    /// Igualdad por contenido (una celda vacía equivale a una transparente), con mensaje.
    pub fn equal(&self, other: &Stack) -> Result<(), String> {
        if (self.width, self.height, self.mode_2) != (other.width, other.height, other.mode_2) {
            return Err(format!(
                "lienzo {}×{} {:?} y {}×{} {:?}",
                self.width, self.height, self.mode_2, other.width, other.height, other.mode_2
            ));
        }
        if self.palette != other.palette {
            return Err("paleta distinta".to_string());
        }
        if self.layers_2.len() != other.layers_2.len() {
            return Err(format!(
                "{} capas y {}",
                self.layers_2.len(),
                other.layers_2.len()
            ));
        }
        for (i, (a, b)) in self.layers_2.iter().zip(&other.layers_2).enumerate() {
            let props = |c: &LayerM| {
                (
                    c.name_2.clone(),
                    c.visible,
                    c.opacity_2,
                    c.blend_2,
                    c.locked_2,
                )
            };
            if props(a) != props(b) {
                return Err(format!(
                    "capa {i}: propiedades {:?} y {:?}",
                    props(a),
                    props(b)
                ));
            }
            let (ra, rb) = (self.grid(i), other.grid(i));
            if let Some(k) = ra.iter().zip(&rb).position(|(x, y)| x != y) {
                return Err(format!(
                    "capa {i}, píxel ({}, {}): {:?} y {:?}",
                    k % self.width,
                    k / self.width,
                    ra[k],
                    rb[k]
                ));
            }
        }
        if self.active_2 != other.active_2 {
            return Err(format!("activa {} y {}", self.active_2, other.active_2));
        }
        Ok(())
    }

    /// Igualdad por contenido sin mirar la capa activa.
    pub fn equal_content(&self, other: &Stack) -> Result<(), String> {
        let mut other = other.clone();
        other.active_2 = self.active_2;
        self.equal(&other)
    }

    /// El modelo con todo píxel RGBA de alfa 0 reducido a `(0, 0, 0, 0)`: guardar un `.aseprite`
    /// solo escribe el rectángulo mínimo y no conserva el RGB bajo alfa 0 fuera de él (límite
    /// conocido, `docs/DISENO-INTERFAZ.md`); un píxel con alfa > 0 nunca se exime.
    pub fn canonical(&self) -> Stack {
        let mut stack = self.clone();
        if stack.mode_2 == Mode::Rgba {
            for layer_2 in &mut stack.layers_2 {
                if let Some(cel) = &mut layer_2.cel {
                    for c in cel.iter_mut().filter(|c| c[3] == 0) {
                        *c = [0, 0, 0, 0];
                    }
                }
            }
        }
        stack
    }

    /// Un documento con estas capas (sin historia), por `Document::from_layers`.
    pub fn document_of(&self, layers_2: &[usize]) -> Document {
        let specs: Vec<LayerSpec> = layers_2
            .iter()
            .map(|&i| {
                let c = &self.layers_2[i];
                LayerSpec {
                    name: c.name_2.clone(),
                    visible: c.visible,
                    opacity: c.opacity_2,
                    blend: c.blend_2,
                    locked: c.locked_2,
                    image: c.cel.as_ref().map(|cel| self.image_2(cel)),
                }
            })
            .collect();
        Document::from_layers(
            self.width as u32,
            self.height as u32,
            self.mode_2,
            self.palette.clone(),
            specs,
        )
        .expect("el modelo construye un documento válido")
    }

    pub fn document(&self) -> Document {
        let all: Vec<usize> = (0..self.layers_2.len()).collect();
        self.document_of(&all)
    }

    /// La composición esperada de todas las capas (o el error de `compose`, como texto: un
    /// indexado sin índice transparente con todas las capas ocultas no tiene qué mostrar).
    pub fn compose_2(&self) -> Result<Image, String> {
        compose(&self.document()).map_err(|e| format!("{e:?}"))
    }

    fn image_2(&self, cel: &[Cel4]) -> Image {
        let (width, height) = (self.width as u32, self.height as u32);
        match self.mode_2 {
            Mode::Indexed => {
                let indices = cel.iter().map(|c| c[0]).collect();
                Image::from_bytes_indexed(
                    width,
                    height,
                    self.palette.clone().expect("paleta del indexado"),
                    indices,
                )
                .expect("imagen indexada del modelo")
            }
            _ => Image::from_bytes(width, height, Mode::Rgba, cel.concat())
                .expect("imagen RGBA del modelo"),
        }
    }

    /// Una imagen (la de una composición) convertida a rejilla del modelo.
    pub fn grid_of(image_2: &Image) -> Vec<Cel4> {
        expand(image_2)
    }

    fn pixel_de(&self, c: Cel4) -> Pixel {
        match self.mode_2 {
            Mode::Indexed => Pixel::Indexed(c[0]),
            _ => Pixel::Rgba(Rgba::new(c[0], c[1], c[2], c[3])),
        }
    }

    /// El píxel de sesión de un color del modelo (para las herramientas).
    pub fn pixel(&self, c: Cel4) -> Pixel {
        self.pixel_de(c)
    }

    fn inside(&self, (x, y): (i32, i32)) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    /// Escribe `valor` en los puntos dentro del lienzo de la celda activa; devuelve si algo cambió.
    /// Una celda vacía solo se materializa si algún píxel difiere del transparente.
    fn paint(&mut self, points: &[(i32, i32)], valor: Cel4) -> bool {
        let active_2 = self.active_2;
        let mut grid = self.grid(active_2);
        let mut change = false;
        for &p in points.iter().filter(|&&p| self.inside(p)) {
            let k = p.1 as usize * self.width + p.0 as usize;
            if grid[k] != valor {
                grid[k] = valor;
                change = true;
            }
        }
        if change {
            self.layers_2[active_2].cel = Some(grid);
        }
        change
    }

    /// Relleno por cola de 4 vecinos sobre la celda activa.
    fn fill(&mut self, seed: (i32, i32), valor: Cel4) -> bool {
        if !self.inside(seed) {
            return false;
        }
        let active_2 = self.active_2;
        let mut grid = self.grid(active_2);
        let (width, height) = (self.width as i32, self.height as i32);
        let source_2 = grid[(seed.1 * width + seed.0) as usize];
        if source_2 == valor {
            return false;
        }
        let mut queue = std::collections::VecDeque::from([seed]);
        while let Some((x, y)) = queue.pop_front() {
            let k = (y * width + x) as usize;
            if grid[k] != source_2 {
                continue;
            }
            grid[k] = valor;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && nx < width && ny < height {
                    queue.push_back((nx, ny));
                }
            }
        }
        self.layers_2[active_2].cel = Some(grid);
        true
    }

    fn delete(&mut self, points: &[(i32, i32)]) -> bool {
        let active_2 = self.active_2;
        let Some(cel) = &self.layers_2[active_2].cel else {
            return false;
        };
        let transparent = self.transparent();
        let mut cel = cel.clone();
        let mut change = false;
        for &p in points.iter().filter(|&&p| self.inside(p)) {
            let k = p.1 as usize * self.width + p.0 as usize;
            if cel[k] != transparent {
                cel[k] = transparent;
                change = true;
            }
        }
        if change {
            self.layers_2[active_2].cel = Some(cel);
        }
        change
    }

    /// Se puede pintar en la capa activa (ni oculta ni bloqueada).
    pub fn can_paint(&self) -> bool {
        let c = &self.layers_2[self.active_2];
        c.visible && !c.locked_2
    }

    fn indexed_2(&self) -> bool {
        self.mode_2 == Mode::Indexed
    }

    /// Aplica la orden con las reglas documentadas en 3.1 a 3.5 y devuelve su efecto.
    pub fn apply(&mut self, order: &Order) -> Effect {
        match order {
            Order::New => {
                if !self.can_create() {
                    return NOTHING;
                }
                let at = self.active_2 + 1;
                let name_2 = format!("Capa {}", self.layers_2.len() + 1);
                self.layers_2.insert(
                    at,
                    LayerM {
                        name_2,
                        visible: true,
                        opacity_2: 255,
                        blend_2: BlendMode::Normal,
                        locked_2: false,
                        cel: None,
                    },
                );
                self.active_2 = at;
                STRUCTURE
            }
            Order::Duplicate => {
                if !self.can_create() {
                    return NOTHING;
                }
                let mut copy = self.layers_2[self.active_2].clone();
                copy.name_2 = format!("{} copia", copy.name_2);
                self.layers_2.insert(self.active_2 + 1, copy);
                self.active_2 += 1;
                STRUCTURE
            }
            Order::Remove => {
                if self.layers_2.len() < 2 {
                    return NOTHING;
                }
                self.layers_2.remove(self.active_2);
                self.active_2 = self.active_2.min(self.layers_2.len() - 1);
                STRUCTURE
            }
            Order::Mover(delta) => {
                let to = self.active_2 as i64 + i64::from(*delta);
                if to < 0 || to >= self.layers_2.len() as i64 {
                    return NOTHING;
                }
                let layer_2 = self.layers_2.remove(self.active_2);
                self.layers_2.insert(to as usize, layer_2);
                self.active_2 = to as usize;
                CHANGES
            }
            Order::Activate(i) => {
                self.active_2 = *i;
                NOTHING
            }
            Order::Visible(i, v) => changes_if(&mut self.layers_2[*i].visible, *v),
            Order::Lock(i, v) => changes_if(&mut self.layers_2[*i].locked_2, *v),
            Order::Rename(i, name_2) => changes_if(&mut self.layers_2[*i].name_2, name_2.clone()),
            Order::Opacity(i, o) => {
                if self.indexed_2() && *o != 255 {
                    return NOTHING;
                }
                changes_if(&mut self.layers_2[*i].opacity_2, *o)
            }
            Order::Blend(i, m) => {
                if self.indexed_2() && *m != BlendMode::Normal {
                    return NOTHING;
                }
                changes_if(&mut self.layers_2[*i].blend_2, *m)
            }
            Order::Pencil(path_2, color) => {
                if !self.can_paint() {
                    return NOTHING;
                }
                effect(self.paint(&stroke(path_2), *color))
            }
            Order::Line(a, b, color) => {
                if !self.can_paint() {
                    return NOTHING;
                }
                effect(self.paint(&segment(*a, *b), *color))
            }
            Order::Fill(seed, color) => {
                if !self.can_paint() {
                    return NOTHING;
                }
                effect(self.fill(*seed, *color))
            }
            Order::Eraser(path_2) => {
                if !self.can_paint() || !self.can_delete() {
                    return NOTHING;
                }
                effect(self.delete(&stroke(path_2)))
            }
            Order::Merge(kind) => self.merge(*kind),
            Order::Convert(target_2) => self.convert(*target_2),
            Order::Vector(..) => NOTHING,
        }
    }

    /// Qué capas se unen y en cuál (posiciones), o `None` si no se puede (reglas de 3.5).
    fn plan_of_merge(&self, kind: MergeKind) -> Option<(usize, Vec<usize>)> {
        let visible: Vec<usize> = (0..self.layers_2.len())
            .filter(|&i| self.layers_2[i].visible)
            .collect();
        match kind {
            MergeKind::Down => {
                let below = self.active_2.checked_sub(1)?;
                let (top, below_2) = (&self.layers_2[self.active_2], &self.layers_2[below]);
                (top.visible && below_2.visible && !below_2.locked_2)
                    .then(|| (below, vec![below, self.active_2]))
            }
            MergeKind::Visible => (visible.len() >= 2).then(|| (visible[0], visible.clone())),
            MergeKind::Flatten => (self.layers_2.len() >= 2 && !visible.is_empty())
                .then(|| (visible[0], visible.clone())),
        }
    }

    fn merge(&mut self, kind: MergeKind) -> Effect {
        let Some((target_2, join)) = self.plan_of_merge(kind) else {
            return NOTHING;
        };
        // La celda resultante es la composición de las capas que se unen (como `merge_*_command`).
        let composite = compose(&self.document_of(&join)).expect("compose de lo que se une");
        let grid = Stack::grid_of(&composite);
        let remove: Vec<usize> = match kind {
            MergeKind::Flatten => (0..self.layers_2.len())
                .filter(|&i| i != target_2)
                .collect(),
            _ => join.iter().copied().filter(|&i| i != target_2).collect(),
        };
        {
            let layer_2 = &mut self.layers_2[target_2];
            layer_2.cel = Some(grid);
            layer_2.opacity_2 = 255;
            layer_2.blend_2 = BlendMode::Normal;
            layer_2.locked_2 = false;
        }
        let new_2_2: Vec<LayerM> = self
            .layers_2
            .iter()
            .enumerate()
            .filter(|(i, _)| !remove.contains(i))
            .map(|(_, layer_2)| layer_2.clone())
            .collect();
        // El destino es la capa de abajo o la visible más baja: nada quitado queda por debajo.
        self.active_2 = (0..target_2).filter(|i| !remove.contains(i)).count();
        self.layers_2 = new_2_2;
        STRUCTURE
    }

    /// Indexado → RGBA: cada índice por su color resuelto (el transparente, `(0, 0, 0, 0)`).
    /// RGBA → indexado: lo decide la paleta que dé la sesión (`adopts_palette`).
    fn convert(&mut self, target_2: Mode) -> Effect {
        match (self.mode_2, target_2) {
            (Mode::Indexed, Mode::Rgba) => {
                let palette = self.palette.clone().expect("paleta");
                let resolver = |c: Cel4| {
                    let color = palette.resolved(c[0]).expect("entrada");
                    if color.a == 0 {
                        [0, 0, 0, 0]
                    } else {
                        [color.r, color.g, color.b, color.a]
                    }
                };
                for layer_2 in &mut self.layers_2 {
                    if let Some(cel) = &mut layer_2.cel {
                        for c in cel.iter_mut() {
                            *c = resolver(*c);
                        }
                    }
                }
                self.mode_2 = Mode::Rgba;
                self.palette = None;
                CHANGES
            }
            (de, a) => panic!("el modelo no convierte {de:?} a {a:?} por su cuenta"),
        }
    }

    /// RGBA → indexado: el modelo adopta la paleta de la sesión **comprobando que es exacta**:
    /// cada píxel del modelo (alfa 0 → el transparente) debe tener en ella su color resuelto.
    pub fn adopts_palette(&mut self, palette: Palette) -> Result<(), String> {
        // Sin índice transparente (un documento de una capa sin ningún píxel de alfa 0) no puede
        // haber píxeles transparentes.
        let transparent = palette.transparent_index();
        for (i, layer_2) in self.layers_2.iter_mut().enumerate() {
            let Some(cel) = &mut layer_2.cel else {
                continue;
            };
            for (k, c) in cel.iter_mut().enumerate() {
                *c = if c[3] == 0 {
                    let t = transparent.ok_or_else(|| {
                        format!("capa {i}, celda {k}: píxel transparente y la paleta no tiene índice transparente")
                    })?;
                    [t, 0, 0, 0]
                } else {
                    let color = Rgba::new(c[0], c[1], c[2], c[3]);
                    let idx = (0..palette.len())
                        .find(|&j| palette.resolved(j as u8) == Ok(color))
                        .ok_or_else(|| {
                            format!("capa {i}, celda {k}: {color:?} no está en la paleta nueva")
                        })?;
                    [idx as u8, 0, 0, 0]
                };
            }
        }
        self.mode_2 = Mode::Indexed;
        self.palette = Some(palette);
        Ok(())
    }
}

fn effect(change: bool) -> Effect {
    if change { CHANGES } else { NOTHING }
}

fn changes_if<T: PartialEq>(field_2: &mut T, valor: T) -> Effect {
    if *field_2 == valor {
        NOTHING
    } else {
        *field_2 = valor;
        CHANGES
    }
}

/// Una imagen del documento como rejilla del modelo.
fn expand(image_2: &Image) -> Vec<Cel4> {
    let mut output = Vec::with_capacity((image_2.width() * image_2.height()) as usize);
    for y in 0..image_2.height() {
        for x in 0..image_2.width() {
            output.push(match image_2.get_pixel(x, y).expect("píxel dentro") {
                Pixel::Rgba(c) => [c.r, c.g, c.b, c.a],
                Pixel::Indexed(i) => [i, 0, 0, 0],
                other_2 => panic!("la puerta de capas solo usa RGBA e indexado, hay {other_2:?}"),
            });
        }
    }
    output
}
