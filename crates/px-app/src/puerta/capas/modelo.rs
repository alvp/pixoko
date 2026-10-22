//! Modelo independiente de la pila de capas de la puerta del hito 3 (specs/hito-3-puerta.md,
//! «Modelo independiente de la pila»): capas con sus propiedades y una rejilla de celdas por capa
//! (RGBA `[r, g, b, a]`; indexado `[índice, 0, 0, 0]`), la capa activa y órdenes que se aplican con
//! código ingenuo (relleno por cola, líneas solo horizontales, verticales o a 45°).
//!
//! **No reimplementa la composición**: las fórmulas de mezcla las fijan 3.2 y 3.5 (17 + 29 goldens
//! de Aseprite). La composición esperada sale de `compose` sobre un documento que el modelo
//! construye con `Document::from_layers`, un camino distinto del de la sesión (sin historia ni
//! comandos); lo demás (propiedades, orden, contenido de cada celda, capa activa) sale solo del
//! modelo. Los vectores a mano de 3.2 se comprueban aparte con `Orden::Vector`.

use px_core::{BlendMode, Document, Image, LayerSpec, Mode, Palette, Pixel, Rgba};
use px_raster::compose;

use crate::merge_flow::MergeKind;

/// Un píxel del modelo: RGBA `[r, g, b, a]` o indexado `[índice, 0, 0, 0]`.
pub type Celda4 = [u8; 4];

#[derive(Clone, Debug)]
pub struct CapaM {
    pub nombre: String,
    pub visible: bool,
    pub opacidad: u8,
    pub mezcla: BlendMode,
    pub bloqueada: bool,
    /// `None`: celda vacía (sin memoria). Con rejilla: `ancho × alto` celdas.
    pub celda: Option<Vec<Celda4>>,
}

#[derive(Clone, Debug)]
pub struct Pila {
    pub ancho: usize,
    pub alto: usize,
    pub modo: Mode,
    pub paleta: Option<Palette>,
    pub capas: Vec<CapaM>,
    /// Posición (de abajo arriba) de la capa activa.
    pub activa: usize,
}

/// Lo que una orden hizo en el modelo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Efecto {
    /// Hay paso de historia nuevo.
    pub cambia: bool,
    /// La orden cambia la estructura de capas (nueva, duplicar, quitar, combinar, convertir): la
    /// capa activa tras deshacer o rehacer se sabe.
    pub estructural: bool,
}

const NADA: Efecto = Efecto {
    cambia: false,
    estructural: false,
};
const CAMBIA: Efecto = Efecto {
    cambia: true,
    estructural: false,
};
const ESTRUCTURA: Efecto = Efecto {
    cambia: true,
    estructural: true,
};

#[derive(Clone, Debug)]
pub enum Orden {
    Nueva,
    Duplicar,
    Quitar,
    /// `+1` sube un puesto y `-1` baja (la activa sigue siendo la misma).
    Mover(i32),
    /// Elegir la capa activa: no es un paso de historia.
    Activar(usize),
    Visible(usize, bool),
    Bloqueo(usize, bool),
    Renombrar(usize, String),
    Opacidad(usize, u8),
    Mezcla(usize, BlendMode),
    Lapiz(Vec<(i32, i32)>, Celda4),
    Linea((i32, i32), (i32, i32), Celda4),
    Relleno((i32, i32), Celda4),
    Borrador(Vec<(i32, i32)>),
    Combinar(MergeKind),
    /// Conversión de **todas** las capas, exacta (sin permisos para perder datos).
    Convertir(Mode),
    /// No cambia nada: exige que el píxel `(x, y)` de la composición sea este valor escrito a mano.
    Vector((u32, u32), Celda4),
}

/// Los píxeles de un segmento horizontal, vertical o a 45° (de `a` a `b`, ambos incluidos).
pub fn segmento(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    assert!(
        dx == 0 || dy == 0 || dx.abs() == dy.abs(),
        "el modelo solo traza líneas horizontales, verticales o a 45°: {a:?} {b:?}"
    );
    let pasos = dx.abs().max(dy.abs());
    (0..=pasos)
        .map(|i| (a.0 + dx.signum() * i, a.1 + dy.signum() * i))
        .collect()
}

/// Un trazo: la unión de los segmentos entre puntos consecutivos, sin repetir.
pub fn trazo(ruta: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut puntos = vec![ruta[0]];
    for par in ruta.windows(2) {
        puntos.extend(segmento(par[0], par[1]).into_iter().skip(1));
    }
    puntos
}

impl Pila {
    /// El modelo sembrado con lo que hay en un documento (se usa al abrir y para leer la sesión).
    pub fn de_documento(doc: &Document, activa: usize) -> Pila {
        let (ancho, alto) = (doc.width() as usize, doc.height() as usize);
        let capas = doc
            .layers()
            .iter()
            .map(|capa| CapaM {
                nombre: capa.name().to_string(),
                visible: capa.visible(),
                opacidad: capa.opacity(),
                mezcla: capa.blend(),
                bloqueada: capa.locked(),
                celda: doc.image(capa.cel()).map(expandir),
            })
            .collect();
        Pila {
            ancho,
            alto,
            modo: doc.mode(),
            paleta: doc.palette().cloned(),
            capas,
            activa,
        }
    }

    /// El píxel transparente del modo (el de una celda nueva).
    pub fn transparente(&self) -> Celda4 {
        match self.modo {
            Mode::Indexed => [
                self.paleta
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
    fn transparente_opt(&self) -> Option<u8> {
        self.paleta.as_ref().and_then(Palette::transparent_index)
    }

    /// `Nueva` y `Duplicar` se rechazan en un indexado sin índice transparente (3.4).
    fn puede_crear(&self) -> bool {
        !self.indexado() || self.transparente_opt().is_some()
    }

    /// El borrador necesita un valor de borrado: en indexado, el índice transparente.
    pub fn puede_borrar(&self) -> bool {
        !self.indexado() || self.transparente_opt().is_some()
    }

    /// La celda de la capa `i` con las vacías expandidas a transparente (la igualdad por contenido).
    pub fn rejilla(&self, i: usize) -> Vec<Celda4> {
        match &self.capas[i].celda {
            Some(celda) => celda.clone(),
            None => vec![self.transparente(); self.ancho * self.alto],
        }
    }

    /// Igualdad por contenido (una celda vacía equivale a una transparente), con mensaje.
    pub fn igual(&self, otra: &Pila) -> Result<(), String> {
        if (self.ancho, self.alto, self.modo) != (otra.ancho, otra.alto, otra.modo) {
            return Err(format!(
                "lienzo {}×{} {:?} y {}×{} {:?}",
                self.ancho, self.alto, self.modo, otra.ancho, otra.alto, otra.modo
            ));
        }
        if self.paleta != otra.paleta {
            return Err("paleta distinta".to_string());
        }
        if self.capas.len() != otra.capas.len() {
            return Err(format!("{} capas y {}", self.capas.len(), otra.capas.len()));
        }
        for (i, (a, b)) in self.capas.iter().zip(&otra.capas).enumerate() {
            let props = |c: &CapaM| {
                (
                    c.nombre.clone(),
                    c.visible,
                    c.opacidad,
                    c.mezcla,
                    c.bloqueada,
                )
            };
            if props(a) != props(b) {
                return Err(format!(
                    "capa {i}: propiedades {:?} y {:?}",
                    props(a),
                    props(b)
                ));
            }
            let (ra, rb) = (self.rejilla(i), otra.rejilla(i));
            if let Some(k) = ra.iter().zip(&rb).position(|(x, y)| x != y) {
                return Err(format!(
                    "capa {i}, píxel ({}, {}): {:?} y {:?}",
                    k % self.ancho,
                    k / self.ancho,
                    ra[k],
                    rb[k]
                ));
            }
        }
        if self.activa != otra.activa {
            return Err(format!("activa {} y {}", self.activa, otra.activa));
        }
        Ok(())
    }

    /// Igualdad por contenido sin mirar la capa activa.
    pub fn igual_contenido(&self, otra: &Pila) -> Result<(), String> {
        let mut otra = otra.clone();
        otra.activa = self.activa;
        self.igual(&otra)
    }

    /// El modelo con todo píxel RGBA de alfa 0 reducido a `(0, 0, 0, 0)`: guardar un `.aseprite`
    /// solo escribe el rectángulo mínimo y no conserva el RGB bajo alfa 0 fuera de él (límite
    /// conocido, `docs/DISENO-INTERFAZ.md`); un píxel con alfa > 0 nunca se exime.
    pub fn canonica(&self) -> Pila {
        let mut pila = self.clone();
        if pila.modo == Mode::Rgba {
            for capa in &mut pila.capas {
                if let Some(celda) = &mut capa.celda {
                    for c in celda.iter_mut().filter(|c| c[3] == 0) {
                        *c = [0, 0, 0, 0];
                    }
                }
            }
        }
        pila
    }

    /// Un documento con estas capas (sin historia), por `Document::from_layers`.
    pub fn documento_de(&self, capas: &[usize]) -> Document {
        let specs: Vec<LayerSpec> = capas
            .iter()
            .map(|&i| {
                let c = &self.capas[i];
                LayerSpec {
                    name: c.nombre.clone(),
                    visible: c.visible,
                    opacity: c.opacidad,
                    blend: c.mezcla,
                    locked: c.bloqueada,
                    image: c.celda.as_ref().map(|celda| self.imagen(celda)),
                }
            })
            .collect();
        Document::from_layers(
            self.ancho as u32,
            self.alto as u32,
            self.modo,
            self.paleta.clone(),
            specs,
        )
        .expect("el modelo construye un documento válido")
    }

    pub fn documento(&self) -> Document {
        let todas: Vec<usize> = (0..self.capas.len()).collect();
        self.documento_de(&todas)
    }

    /// La composición esperada de todas las capas (o el error de `compose`, como texto: un
    /// indexado sin índice transparente con todas las capas ocultas no tiene qué mostrar).
    pub fn componer(&self) -> Result<Image, String> {
        compose(&self.documento()).map_err(|e| format!("{e:?}"))
    }

    fn imagen(&self, celda: &[Celda4]) -> Image {
        let (ancho, alto) = (self.ancho as u32, self.alto as u32);
        match self.modo {
            Mode::Indexed => {
                let indices = celda.iter().map(|c| c[0]).collect();
                Image::from_bytes_indexed(
                    ancho,
                    alto,
                    self.paleta.clone().expect("paleta del indexado"),
                    indices,
                )
                .expect("imagen indexada del modelo")
            }
            _ => Image::from_bytes(ancho, alto, Mode::Rgba, celda.concat())
                .expect("imagen RGBA del modelo"),
        }
    }

    /// Una imagen (la de una composición) convertida a rejilla del modelo.
    pub fn rejilla_de(imagen: &Image) -> Vec<Celda4> {
        expandir(imagen)
    }

    fn pixel_de(&self, c: Celda4) -> Pixel {
        match self.modo {
            Mode::Indexed => Pixel::Indexed(c[0]),
            _ => Pixel::Rgba(Rgba::new(c[0], c[1], c[2], c[3])),
        }
    }

    /// El píxel de sesión de un color del modelo (para las herramientas).
    pub fn pixel(&self, c: Celda4) -> Pixel {
        self.pixel_de(c)
    }

    fn dentro(&self, (x, y): (i32, i32)) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.ancho && (y as usize) < self.alto
    }

    /// Escribe `valor` en los puntos dentro del lienzo de la celda activa; devuelve si algo cambió.
    /// Una celda vacía solo se materializa si algún píxel difiere del transparente.
    fn pintar(&mut self, puntos: &[(i32, i32)], valor: Celda4) -> bool {
        let activa = self.activa;
        let mut rejilla = self.rejilla(activa);
        let mut cambio = false;
        for &p in puntos.iter().filter(|&&p| self.dentro(p)) {
            let k = p.1 as usize * self.ancho + p.0 as usize;
            if rejilla[k] != valor {
                rejilla[k] = valor;
                cambio = true;
            }
        }
        if cambio {
            self.capas[activa].celda = Some(rejilla);
        }
        cambio
    }

    /// Relleno por cola de 4 vecinos sobre la celda activa.
    fn rellenar(&mut self, semilla: (i32, i32), valor: Celda4) -> bool {
        if !self.dentro(semilla) {
            return false;
        }
        let activa = self.activa;
        let mut rejilla = self.rejilla(activa);
        let (ancho, alto) = (self.ancho as i32, self.alto as i32);
        let origen = rejilla[(semilla.1 * ancho + semilla.0) as usize];
        if origen == valor {
            return false;
        }
        let mut cola = std::collections::VecDeque::from([semilla]);
        while let Some((x, y)) = cola.pop_front() {
            let k = (y * ancho + x) as usize;
            if rejilla[k] != origen {
                continue;
            }
            rejilla[k] = valor;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && nx < ancho && ny < alto {
                    cola.push_back((nx, ny));
                }
            }
        }
        self.capas[activa].celda = Some(rejilla);
        true
    }

    fn borrar(&mut self, puntos: &[(i32, i32)]) -> bool {
        let activa = self.activa;
        let Some(celda) = &self.capas[activa].celda else {
            return false;
        };
        let transparente = self.transparente();
        let mut celda = celda.clone();
        let mut cambio = false;
        for &p in puntos.iter().filter(|&&p| self.dentro(p)) {
            let k = p.1 as usize * self.ancho + p.0 as usize;
            if celda[k] != transparente {
                celda[k] = transparente;
                cambio = true;
            }
        }
        if cambio {
            self.capas[activa].celda = Some(celda);
        }
        cambio
    }

    /// Se puede pintar en la capa activa (ni oculta ni bloqueada).
    pub fn puede_pintar(&self) -> bool {
        let c = &self.capas[self.activa];
        c.visible && !c.bloqueada
    }

    fn indexado(&self) -> bool {
        self.modo == Mode::Indexed
    }

    /// Aplica la orden con las reglas documentadas en 3.1 a 3.5 y devuelve su efecto.
    pub fn aplicar(&mut self, orden: &Orden) -> Efecto {
        match orden {
            Orden::Nueva => {
                if !self.puede_crear() {
                    return NADA;
                }
                let at = self.activa + 1;
                let nombre = format!("Capa {}", self.capas.len() + 1);
                self.capas.insert(
                    at,
                    CapaM {
                        nombre,
                        visible: true,
                        opacidad: 255,
                        mezcla: BlendMode::Normal,
                        bloqueada: false,
                        celda: None,
                    },
                );
                self.activa = at;
                ESTRUCTURA
            }
            Orden::Duplicar => {
                if !self.puede_crear() {
                    return NADA;
                }
                let mut copia = self.capas[self.activa].clone();
                copia.nombre = format!("{} copia", copia.nombre);
                self.capas.insert(self.activa + 1, copia);
                self.activa += 1;
                ESTRUCTURA
            }
            Orden::Quitar => {
                if self.capas.len() < 2 {
                    return NADA;
                }
                self.capas.remove(self.activa);
                self.activa = self.activa.min(self.capas.len() - 1);
                ESTRUCTURA
            }
            Orden::Mover(delta) => {
                let to = self.activa as i64 + i64::from(*delta);
                if to < 0 || to >= self.capas.len() as i64 {
                    return NADA;
                }
                let capa = self.capas.remove(self.activa);
                self.capas.insert(to as usize, capa);
                self.activa = to as usize;
                CAMBIA
            }
            Orden::Activar(i) => {
                self.activa = *i;
                NADA
            }
            Orden::Visible(i, v) => cambia_si(&mut self.capas[*i].visible, *v),
            Orden::Bloqueo(i, v) => cambia_si(&mut self.capas[*i].bloqueada, *v),
            Orden::Renombrar(i, nombre) => cambia_si(&mut self.capas[*i].nombre, nombre.clone()),
            Orden::Opacidad(i, o) => {
                if self.indexado() && *o != 255 {
                    return NADA;
                }
                cambia_si(&mut self.capas[*i].opacidad, *o)
            }
            Orden::Mezcla(i, m) => {
                if self.indexado() && *m != BlendMode::Normal {
                    return NADA;
                }
                cambia_si(&mut self.capas[*i].mezcla, *m)
            }
            Orden::Lapiz(ruta, color) => {
                if !self.puede_pintar() {
                    return NADA;
                }
                efecto(self.pintar(&trazo(ruta), *color))
            }
            Orden::Linea(a, b, color) => {
                if !self.puede_pintar() {
                    return NADA;
                }
                efecto(self.pintar(&segmento(*a, *b), *color))
            }
            Orden::Relleno(semilla, color) => {
                if !self.puede_pintar() {
                    return NADA;
                }
                efecto(self.rellenar(*semilla, *color))
            }
            Orden::Borrador(ruta) => {
                if !self.puede_pintar() || !self.puede_borrar() {
                    return NADA;
                }
                efecto(self.borrar(&trazo(ruta)))
            }
            Orden::Combinar(tipo) => self.combinar(*tipo),
            Orden::Convertir(destino) => self.convertir(*destino),
            Orden::Vector(..) => NADA,
        }
    }

    /// Qué capas se unen y en cuál (posiciones), o `None` si no se puede (reglas de 3.5).
    fn plan_de_combinar(&self, tipo: MergeKind) -> Option<(usize, Vec<usize>)> {
        let visibles: Vec<usize> = (0..self.capas.len())
            .filter(|&i| self.capas[i].visible)
            .collect();
        match tipo {
            MergeKind::Down => {
                let abajo = self.activa.checked_sub(1)?;
                let (arriba, debajo) = (&self.capas[self.activa], &self.capas[abajo]);
                (arriba.visible && debajo.visible && !debajo.bloqueada)
                    .then(|| (abajo, vec![abajo, self.activa]))
            }
            MergeKind::Visible => (visibles.len() >= 2).then(|| (visibles[0], visibles.clone())),
            MergeKind::Flatten => (self.capas.len() >= 2 && !visibles.is_empty())
                .then(|| (visibles[0], visibles.clone())),
        }
    }

    fn combinar(&mut self, tipo: MergeKind) -> Efecto {
        let Some((destino, unir)) = self.plan_de_combinar(tipo) else {
            return NADA;
        };
        // La celda resultante es la composición de las capas que se unen (como `merge_*_command`).
        let compuesta = compose(&self.documento_de(&unir)).expect("compose de lo que se une");
        let rejilla = Pila::rejilla_de(&compuesta);
        let quitar: Vec<usize> = match tipo {
            MergeKind::Flatten => (0..self.capas.len()).filter(|&i| i != destino).collect(),
            _ => unir.iter().copied().filter(|&i| i != destino).collect(),
        };
        {
            let capa = &mut self.capas[destino];
            capa.celda = Some(rejilla);
            capa.opacidad = 255;
            capa.mezcla = BlendMode::Normal;
            capa.bloqueada = false;
        }
        let nuevas: Vec<CapaM> = self
            .capas
            .iter()
            .enumerate()
            .filter(|(i, _)| !quitar.contains(i))
            .map(|(_, capa)| capa.clone())
            .collect();
        // El destino es la capa de abajo o la visible más baja: nada quitado queda por debajo.
        self.activa = (0..destino).filter(|i| !quitar.contains(i)).count();
        self.capas = nuevas;
        ESTRUCTURA
    }

    /// Indexado → RGBA: cada índice por su color resuelto (el transparente, `(0, 0, 0, 0)`).
    /// RGBA → indexado: lo decide la paleta que dé la sesión (`adopta_paleta`).
    fn convertir(&mut self, destino: Mode) -> Efecto {
        match (self.modo, destino) {
            (Mode::Indexed, Mode::Rgba) => {
                let paleta = self.paleta.clone().expect("paleta");
                let resolver = |c: Celda4| {
                    let color = paleta.resolved(c[0]).expect("entrada");
                    if color.a == 0 {
                        [0, 0, 0, 0]
                    } else {
                        [color.r, color.g, color.b, color.a]
                    }
                };
                for capa in &mut self.capas {
                    if let Some(celda) = &mut capa.celda {
                        for c in celda.iter_mut() {
                            *c = resolver(*c);
                        }
                    }
                }
                self.modo = Mode::Rgba;
                self.paleta = None;
                CAMBIA
            }
            (de, a) => panic!("el modelo no convierte {de:?} a {a:?} por su cuenta"),
        }
    }

    /// RGBA → indexado: el modelo adopta la paleta de la sesión **comprobando que es exacta**:
    /// cada píxel del modelo (alfa 0 → el transparente) debe tener en ella su color resuelto.
    pub fn adopta_paleta(&mut self, paleta: Palette) -> Result<(), String> {
        // Sin índice transparente (un documento de una capa sin ningún píxel de alfa 0) no puede
        // haber píxeles transparentes.
        let transparente = paleta.transparent_index();
        for (i, capa) in self.capas.iter_mut().enumerate() {
            let Some(celda) = &mut capa.celda else {
                continue;
            };
            for (k, c) in celda.iter_mut().enumerate() {
                *c = if c[3] == 0 {
                    let t = transparente.ok_or_else(|| {
                        format!("capa {i}, celda {k}: píxel transparente y la paleta no tiene índice transparente")
                    })?;
                    [t, 0, 0, 0]
                } else {
                    let color = Rgba::new(c[0], c[1], c[2], c[3]);
                    let idx = (0..paleta.len())
                        .find(|&j| paleta.resolved(j as u8) == Ok(color))
                        .ok_or_else(|| {
                            format!("capa {i}, celda {k}: {color:?} no está en la paleta nueva")
                        })?;
                    [idx as u8, 0, 0, 0]
                };
            }
        }
        self.modo = Mode::Indexed;
        self.paleta = Some(paleta);
        Ok(())
    }
}

fn efecto(cambio: bool) -> Efecto {
    if cambio { CAMBIA } else { NADA }
}

fn cambia_si<T: PartialEq>(campo: &mut T, valor: T) -> Efecto {
    if *campo == valor {
        NADA
    } else {
        *campo = valor;
        CAMBIA
    }
}

/// Una imagen del documento como rejilla del modelo.
fn expandir(imagen: &Image) -> Vec<Celda4> {
    let mut salida = Vec::with_capacity((imagen.width() * imagen.height()) as usize);
    for y in 0..imagen.height() {
        for x in 0..imagen.width() {
            salida.push(match imagen.get_pixel(x, y).expect("píxel dentro") {
                Pixel::Rgba(c) => [c.r, c.g, c.b, c.a],
                Pixel::Indexed(i) => [i, 0, 0, 0],
                otro => panic!("la puerta de capas solo usa RGBA e indexado, hay {otro:?}"),
            });
        }
    }
    salida
}
