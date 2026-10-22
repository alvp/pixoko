//! Fila 11 de la puerta del hito 2 (specs/hito-2-puerta.md, unidad 22): el **oráculo SHA-256** de
//! los 4 PNG reales de `golden/png/` editados.
//!
//! `px-golden` no puede usar `Session` (`px-app` es un binario sin `lib.rs`; decisión D1), así que
//! aquí se edita el `Document` directamente con comandos de `px-core` (`SetPixels`, `FillSpans`,
//! `SetPaletteColor`, `MovePaletteEntry` y `SetTransparentIndex`), se escribe con `write_png`, se
//! relee con `read_png` y se compara el **SHA-256 de la RGBA expandida** de la relectura con el de un
//! modelo ingenuo que aplica las mismas ediciones sobre una rejilla de celdas y una paleta (la misma
//! técnica del corpus: `.sha256` de la RGBA expandida). Un modelo con un píxel cambiado debe dar otro
//! SHA-256: es el control de que el oráculo sabe fallar.
//!
//! Solo usa `px-core` y `px-formats`. No escribe nada en disco: los PNG reales solo se leen y el
//! resultado de `write_png` se queda en memoria.
//!
//! **Suposiciones y ambigüedades:**
//! - «Un cambio de paleta» son dos comandos (cambiar el color de una entrada y mover otra), solo en
//!   las indexadas; en el RGBA real no aplican y quedan los de píxeles.
//! - El punto de partida de cada real es la RGBA que dice su `.sha256` (la de otra herramienta); a
//!   partir de lo leído se siembra el modelo.
//! - Un indexado con índice transparente vuelve de `write_png` más `read_png` con la entrada resuelta
//!   (alfa 0) y sin índice: la RGBA expandida es la misma, que es lo que se compara.

use px_core::ImageId;
use std::fs;

use px_core::{Command, Document, Image, Mode, Pixel, PixelEdit, Rgba, Span};
use px_formats::{read_png, write_png};

use super::common::sha256::sha256_hex;
use super::{bytes_rgba, folder_golden_png, names_in};

/// Rejilla de celdas de 4 bytes (RGBA `[r, g, b, a]`, gris con alfa `[g, a, 0, 0]`, gris `[g, 0, 0,
/// 0]` e indexed `[índice, 0, 0, 0]`) y palette: el model ingenuo de las ediciones.
#[derive(Clone)]
struct Model {
    mode: Mode,
    width: usize,
    height: usize,
    cels: Vec<[u8; 4]>,
    palette: Vec<[u8; 4]>,
    transparent: Option<u8>,
}

impl Model {
    /// Siembra el modelo con una imagen cuya RGBA ya se comprobó contra su `.sha256`.
    fn de(image_2: &Image) -> Model {
        let mode = image_2.mode();
        let n = match mode {
            Mode::Rgba => 4,
            Mode::GrayAlpha => 2,
            Mode::Gray | Mode::Indexed => 1,
        };
        let cels = image_2
            .bytes()
            .chunks_exact(n)
            .map(|piece| {
                let mut cel_2 = [0u8; 4];
                cel_2[..n].copy_from_slice(piece);
                cel_2
            })
            .collect();
        let palette = image_2
            .palette()
            .map(|p| p.colors().iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
            .unwrap_or_default();
        Model {
            mode,
            width: image_2.width() as usize,
            height: image_2.height() as usize,
            cels,
            palette,
            transparent: image_2.palette().and_then(|p| p.transparent_index()),
        }
    }

    /// RGBA de 8 bits expandida: fila a fila, de arriba abajo; el índice transparente, con alfa 0.
    fn rgba(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.cels.len() * 4);
        for c in &self.cels {
            match self.mode {
                Mode::Rgba => v.extend_from_slice(c),
                Mode::GrayAlpha => v.extend([c[0], c[0], c[0], c[1]]),
                Mode::Gray => v.extend([c[0], c[0], c[0], 255]),
                Mode::Indexed => {
                    let mut color = self.palette[usize::from(c[0])];
                    if self.transparent == Some(c[0]) {
                        color[3] = 0;
                    }
                    v.extend(color);
                }
            }
        }
        v
    }

    fn put(&mut self, x: usize, y: usize, cel_2: [u8; 4]) {
        self.cels[y * self.width + x] = cel_2;
    }

    /// Mueve la entrada `de` a la posición `a` (como quitar e insertar) y remapea los píxeles y el
    /// índice transparente.
    fn move_pointer(&mut self, de: usize, a: usize) {
        let mut order: Vec<usize> = (0..self.palette.len()).collect();
        let moved = order.remove(de);
        order.insert(a, moved);
        let mut forward = vec![0u8; order.len()];
        for (new_2, &old) in order.iter().enumerate() {
            forward[old] = new_2 as u8;
        }
        self.palette = order.iter().map(|&old| self.palette[old]).collect();
        for cel_2 in &mut self.cels {
            cel_2[0] = forward[usize::from(cel_2[0])];
        }
        self.transparent = self.transparent.map(|t| forward[usize::from(t)]);
    }
}

/// El píxel de `Document` y la celda del modelo para el valor número `k` de ese modo.
fn value(mode: Mode, k: usize, entries: usize) -> (Pixel, [u8; 4]) {
    let k8 = (k * 37 % 251) as u8;
    match mode {
        Mode::Rgba => {
            let (r, g, b, a) = (k8, 255 - k8, k8 ^ 0x5A, [255, 128, 0][k % 3]);
            (Pixel::Rgba(Rgba::new(r, g, b, a)), [r, g, b, a])
        }
        Mode::Indexed => {
            let index = (k * 3 % entries) as u8;
            (Pixel::Indexed(index), [index, 0, 0, 0])
        }
        Mode::Gray => (Pixel::Gray(k8), [k8, 0, 0, 0]),
        Mode::GrayAlpha => {
            let alpha_2 = [255, 128, 0][k % 3];
            (
                Pixel::GrayAlpha {
                    gray: k8,
                    alpha: alpha_2,
                },
                [k8, alpha_2, 0, 0],
            )
        }
    }
}

/// Edita el documento con comandos de `px-core` y el modelo con código ingenuo; devuelve el modelo.
fn edit(document: &mut Document, mut model: Model) -> Model {
    let id = ImageId::from_raw(0);
    let (mode, width, height) = (model.mode, model.width, model.height);
    let entries = model.palette.len().max(1);

    // `SetPixels`: si una coordenada se repite, gana la última.
    let points = [(1, 1), (2, 1), (3, 2), (15, 15), (0, 15), (1, 1)];
    let mut pixels_2 = Vec::new();
    for (k, (x, y)) in points.iter().enumerate() {
        let (x, y) = (x % width, y % height);
        let (pixel, cel_2) = value(mode, k + 1, entries);
        pixels_2.push(PixelEdit {
            x: x as u32,
            y: y as u32,
            pixel,
        });
        model.put(x, y, cel_2);
    }
    document
        .apply(Command::SetPixels {
            image: id,
            pixels: pixels_2,
        })
        .expect("SetPixels");

    // `FillSpans`: tramos estrictamente ordenados, sin solapes.
    let (pixel, cel_2) = value(mode, 10, entries);
    let spans_2 = [(5, 2, 9), (6, 2, 9), (7, 0, width)];
    let mut spans = Vec::new();
    for (y, x0, x1) in spans_2 {
        let y = y % height;
        spans.push(Span {
            y: y as u32,
            x_start: x0 as u32,
            x_end: x1 as u32,
        });
        for x in x0..x1 {
            model.put(x, y, cel_2);
        }
    }
    document
        .apply(Command::FillSpans {
            image: id,
            spans,
            pixel,
        })
        .expect("FillSpans");

    // Cambio de paleta (solo indexadas): el color de una entrada, mover otra y fijar el transparente.
    if mode == Mode::Indexed {
        document
            .apply(Command::SetPaletteColor {
                image: id,
                index: 0,
                color: Rgba::new(12, 34, 56, 255),
            })
            .expect("SetPaletteColor");
        model.palette[0] = [12, 34, 56, 255];

        let last = model.palette.len() - 1;
        if last >= 1 {
            document
                .apply(Command::MovePaletteEntry {
                    image: id,
                    from: 0,
                    to: last as u8,
                })
                .expect("MovePaletteEntry");
            model.move_pointer(0, last);
        }

        let transparent = 1.min(last) as u8;
        document
            .apply(Command::SetTransparentIndex {
                image: id,
                index: Some(transparent),
            })
            .expect("SetTransparentIndex");
        model.transparent = Some(transparent);
    }
    model
}

/// Los PNG de `golden/png/` con su `.sha256` (el hash que calculó otra herramienta).
fn real() -> Vec<(String, Vec<u8>, String)> {
    let dir = folder_golden_png();
    names_in(&dir)
        .into_iter()
        .filter(|name_2| name_2.ends_with(".png"))
        .map(|name_2| {
            let bytes = fs::read(dir.join(&name_2)).expect("leer el PNG real");
            let hash = fs::read_to_string(dir.join(format!("{name_2}.sha256")))
                .expect("leer el .sha256")
                .trim()
                .to_ascii_lowercase();
            (name_2, bytes, hash)
        })
        .collect()
}

#[test]
fn row_11_the_sha256_of_the_reread_is_the_models_in_the_four_real_ones() {
    let real = real();
    assert_eq!(real.len(), 4, "golden/png/ debía tener 4 PNG reales");
    for (name_2, bytes, hash) in &real {
        let read = read_png(bytes).unwrap_or_else(|e| panic!("{name_2}: read_png: {e}"));
        // El punto de partida es la RGBA que dice el .sha256.
        let initial = sha256_hex(&bytes_rgba(&read.image));
        assert_eq!(&initial, hash, "{name_2}: la lectura no es la del .sha256");

        let model = Model::de(&read.image);
        let mut document = Document::new(read.image);
        let model = edit(&mut document, model);
        let expected = sha256_hex(&model.rgba());
        assert_ne!(
            expected, initial,
            "{name_2}: las ediciones no cambiaron nada: la prueba estaría vacía"
        );

        // El documento editado es el modelo.
        let edited = document
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");
        assert_eq!(
            sha256_hex(&bytes_rgba(edited)),
            expected,
            "{name_2}: el documento editado no es el modelo"
        );

        // `write_png`, `read_png` y el SHA-256 de la RGBA expandida de la relectura.
        let written = write_png(edited).unwrap_or_else(|e| panic!("{name_2}: write_png: {e}"));
        let reread = read_png(&written).unwrap_or_else(|e| panic!("{name_2}: relectura: {e}"));
        assert!(reread.is_exact(), "{name_2}: la relectura se convirtió");
        assert_eq!(
            sha256_hex(&bytes_rgba(&reread.image)),
            expected,
            "{name_2}: la relectura no es el modelo"
        );
    }
}

#[test]
fn row_11_control_a_model_with_one_pixel_changed_gives_another_sha256() {
    for (name_2, bytes, _) in &real() {
        let read = read_png(bytes).unwrap_or_else(|e| panic!("{name_2}: read_png: {e}"));
        let model = Model::de(&read.image);
        let mut document = Document::new(read.image);
        let model = edit(&mut document, model);
        let edited = document
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");
        let reread = read_png(&write_png(edited).expect("write_png")).expect("read_png");
        let read_sha = sha256_hex(&bytes_rgba(&reread.image));
        assert_eq!(sha256_hex(&model.rgba()), read_sha, "{name_2}: partida");

        // Un píxel cambiado en el modelo (un canal, en cada uno de varios sitios): otro hash.
        for pixel in [0, model.cels.len() / 2, model.cels.len() - 1] {
            for channel in 0..4 {
                let mut view = model.rgba();
                view[pixel * 4 + channel] ^= 1;
                assert_ne!(
                    sha256_hex(&view),
                    read_sha,
                    "{name_2}: el modelo con el píxel {pixel} (canal {channel}) cambiado dio el mismo SHA-256"
                );
            }
        }
        // Y a nivel del modelo: otra celda, a un valor que cambia el color resuelto.
        let mut other_2 = model.clone();
        let new = match other_2.mode {
            Mode::Indexed => {
                // Una entrada de la paleta distinta del píxel 0, con otro color resuelto.
                let actual = other_2.cels[0][0];
                let color = |i: u8| {
                    let mut c = other_2.palette[usize::from(i)];
                    if other_2.transparent == Some(i) {
                        c[3] = 0;
                    }
                    c
                };
                let alternative = (0..other_2.palette.len())
                    .map(|i| i as u8)
                    .find(|&i| color(i) != color(actual))
                    .expect("alguna entrada con otro color");
                [alternative, 0, 0, 0]
            }
            Mode::Rgba => {
                let [r, g, b, a] = other_2.cels[0];
                [r ^ 1, g, b, a]
            }
            Mode::Gray => [other_2.cels[0][0] ^ 1, 0, 0, 0],
            Mode::GrayAlpha => [other_2.cels[0][0] ^ 1, other_2.cels[0][1], 0, 0],
        };
        other_2.cels[0] = new;
        assert_ne!(
            sha256_hex(&other_2.rgba()),
            read_sha,
            "{name_2}: el modelo con una celda cambiada dio el mismo SHA-256"
        );
    }
}
