//! Contenido de las texturas tal como egui lo entrega a la GPU, para los bancos sin ventana.
//!
//! `egui` no deja leer una textura desde el `Context`; lo que se ve es el `TexturesDelta` de cada
//! fotograma. `Textures` los aplica **en orden** (completos y parciales, `free` incluidos) y deja
//! leer el resultado: es la «textura compuesta» de la spec de la unidad 14a
//! (specs/hito-2-zoom-baldosas.md, «Oráculo de la textura»). Los texels son `Color32`
//! (premultiplicados), como los guarda egui.

use std::collections::HashMap;

use egui::{Color32, ImageData, TextureId, TexturesDelta};

/// Una textura compuesta: tamaño y texels por filas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Texture {
    pub size_2: [usize; 2],
    pub texels: Vec<Color32>,
}

impl Texture {
    pub fn texel(&self, x: usize, y: usize) -> Option<Color32> {
        (x < self.size_2[0] && y < self.size_2[1]).then(|| self.texels[y * self.size_2[0] + x])
    }
}

/// Lo que cada `TextureId` contiene tras aplicar los deltas recibidos.
#[derive(Clone, Debug, Default)]
pub struct Textures {
    map: HashMap<TextureId, Texture>,
}

impl Textures {
    pub fn new() -> Textures {
        Textures::default()
    }

    /// Aplica un `TexturesDelta`: `set` en orden (un delta con `pos == None` sustituye la textura
    /// entera; con `pos == Some` pega el parche, que debe caber en la textura) y luego `free`.
    pub fn applies(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for delta in deltas {
                let ImageData::Color(image_2) = &delta.image;
                match delta.pos {
                    None => {
                        self.map.insert(
                            *id,
                            Texture {
                                size_2: image_2.size,
                                texels: image_2.pixels.clone(),
                            },
                        );
                    }
                    Some([x0, y0]) => {
                        // El atlas de la fuente de egui (`Managed(0)`) se crea en los
                        // fotogramas de calentamiento de `context()`, cuyos deltas se
                        // descartan: sus parches no se siguen (no es del lienzo).
                        let Some(texture_2) = self.map.get_mut(id) else {
                            assert_eq!(
                                *id,
                                TextureId::Managed(0),
                                "un parche llega a una textura ya creada"
                            );
                            continue;
                        };
                        let [w, h] = image_2.size;
                        assert!(
                            x0 + w <= texture_2.size_2[0] && y0 + h <= texture_2.size_2[1],
                            "el parche {w}×{h} en ({x0}, {y0}) se sale de la textura {:?}",
                            texture_2.size_2
                        );
                        for y in 0..h {
                            let target_2 = (y0 + y) * texture_2.size_2[0] + x0;
                            texture_2.texels[target_2..target_2 + w]
                                .copy_from_slice(&image_2.pixels[y * w..(y + 1) * w]);
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.map.remove(id);
        }
    }

    pub fn get(&self, id: TextureId) -> Option<&Texture> {
        self.map.get(&id)
    }

    /// Texel `(x, y)` de la textura `id`, si existe.
    pub fn texel(&self, id: TextureId, x: usize, y: usize) -> Option<Color32> {
        self.get(id)?.texel(x, y)
    }

    /// Muestreo `NEAREST` con envoltura `ClampToEdge`, como la GPU, en la coordenada `(u, v)`
    /// (0 a 1 sobre la textura entera).
    pub fn samples(&self, id: TextureId, u: f64, v: f64) -> Option<Color32> {
        let texture_2 = self.get(id)?;
        let [w, h] = texture_2.size_2;
        let tx = ((u * w as f64).floor() as i64).clamp(0, w as i64 - 1) as usize;
        let ty = ((v * h as f64).floor() as i64).clamp(0, h as i64 - 1) as usize;
        texture_2.texel(tx, ty)
    }

    pub fn ids(&self) -> impl Iterator<Item = TextureId> + '_ {
        self.map.keys().copied()
    }
}

/// Texels que `egui` guarda para un píxel RGBA no premultiplicado: el oráculo de la unidad 14a
/// (`Color32::from_rgba_unmultiplied`).
pub fn texel_de(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
