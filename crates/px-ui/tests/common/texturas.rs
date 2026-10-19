//! Contenido de las texturas tal como egui lo entrega a la GPU, para los bancos sin ventana.
//!
//! `egui` no deja leer una textura desde el `Context`; lo que se ve es el `TexturesDelta` de cada
//! fotograma. `Texturas` los aplica **en orden** (completos y parciales, `free` incluidos) y deja
//! leer el resultado: es la «textura compuesta» de la spec de la unidad 14a
//! (specs/hito-2-zoom-baldosas.md, «Oráculo de la textura»). Los texels son `Color32`
//! (premultiplicados), como los guarda egui.

use std::collections::HashMap;

use egui::{Color32, ImageData, TextureId, TexturesDelta};

/// Una textura compuesta: tamaño y texels por filas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Textura {
    pub tamano: [usize; 2],
    pub texels: Vec<Color32>,
}

impl Textura {
    pub fn texel(&self, x: usize, y: usize) -> Option<Color32> {
        (x < self.tamano[0] && y < self.tamano[1]).then(|| self.texels[y * self.tamano[0] + x])
    }
}

/// Lo que cada `TextureId` contiene tras aplicar los deltas recibidos.
#[derive(Clone, Debug, Default)]
pub struct Texturas {
    mapa: HashMap<TextureId, Textura>,
}

impl Texturas {
    pub fn new() -> Texturas {
        Texturas::default()
    }

    /// Aplica un `TexturesDelta`: `set` en orden (un delta con `pos == None` sustituye la textura
    /// entera; con `pos == Some` pega el parche, que debe caber en la textura) y luego `free`.
    pub fn aplica(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for delta in deltas {
                let ImageData::Color(imagen) = &delta.image;
                match delta.pos {
                    None => {
                        self.mapa.insert(
                            *id,
                            Textura {
                                tamano: imagen.size,
                                texels: imagen.pixels.clone(),
                            },
                        );
                    }
                    Some([x0, y0]) => {
                        // El atlas de la fuente de egui (`Managed(0)`) se crea en los
                        // fotogramas de calentamiento de `contexto()`, cuyos deltas se
                        // descartan: sus parches no se siguen (no es del lienzo).
                        let Some(textura) = self.mapa.get_mut(id) else {
                            assert_eq!(
                                *id,
                                TextureId::Managed(0),
                                "un parche llega a una textura ya creada"
                            );
                            continue;
                        };
                        let [w, h] = imagen.size;
                        assert!(
                            x0 + w <= textura.tamano[0] && y0 + h <= textura.tamano[1],
                            "el parche {w}×{h} en ({x0}, {y0}) se sale de la textura {:?}",
                            textura.tamano
                        );
                        for y in 0..h {
                            let destino = (y0 + y) * textura.tamano[0] + x0;
                            textura.texels[destino..destino + w]
                                .copy_from_slice(&imagen.pixels[y * w..(y + 1) * w]);
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.mapa.remove(id);
        }
    }

    pub fn get(&self, id: TextureId) -> Option<&Textura> {
        self.mapa.get(&id)
    }

    /// Texel `(x, y)` de la textura `id`, si existe.
    pub fn texel(&self, id: TextureId, x: usize, y: usize) -> Option<Color32> {
        self.get(id)?.texel(x, y)
    }

    /// Muestreo `NEAREST` con envoltura `ClampToEdge`, como la GPU, en la coordenada `(u, v)`
    /// (0 a 1 sobre la textura entera).
    pub fn muestrea(&self, id: TextureId, u: f64, v: f64) -> Option<Color32> {
        let textura = self.get(id)?;
        let [w, h] = textura.tamano;
        let tx = ((u * w as f64).floor() as i64).clamp(0, w as i64 - 1) as usize;
        let ty = ((v * h as f64).floor() as i64).clamp(0, h as i64 - 1) as usize;
        textura.texel(tx, ty)
    }

    pub fn ids(&self) -> impl Iterator<Item = TextureId> + '_ {
        self.mapa.keys().copied()
    }
}

/// Texels que `egui` guarda para un píxel RGBA no premultiplicado: el oráculo de la unidad 14a
/// (`Color32::from_rgba_unmultiplied`).
pub fn texel_de(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
