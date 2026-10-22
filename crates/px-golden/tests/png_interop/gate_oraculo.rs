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
use super::{bytes_rgba, carpeta_golden_png, nombres_en};

/// Rejilla de celdas de 4 bytes (RGBA `[r, g, b, a]`, gris con alfa `[g, a, 0, 0]`, gris `[g, 0, 0,
/// 0]` e indexado `[índice, 0, 0, 0]`) y paleta: el modelo ingenuo de las ediciones.
#[derive(Clone)]
struct Modelo {
    modo: Mode,
    ancho: usize,
    alto: usize,
    celdas: Vec<[u8; 4]>,
    paleta: Vec<[u8; 4]>,
    transparente: Option<u8>,
}

impl Modelo {
    /// Siembra el modelo con una imagen cuya RGBA ya se comprobó contra su `.sha256`.
    fn de(imagen: &Image) -> Modelo {
        let modo = imagen.mode();
        let n = match modo {
            Mode::Rgba => 4,
            Mode::GrayAlpha => 2,
            Mode::Gray | Mode::Indexed => 1,
        };
        let celdas = imagen
            .bytes()
            .chunks_exact(n)
            .map(|trozo| {
                let mut celda = [0u8; 4];
                celda[..n].copy_from_slice(trozo);
                celda
            })
            .collect();
        let paleta = imagen
            .palette()
            .map(|p| p.colors().iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
            .unwrap_or_default();
        Modelo {
            modo,
            ancho: imagen.width() as usize,
            alto: imagen.height() as usize,
            celdas,
            paleta,
            transparente: imagen.palette().and_then(|p| p.transparent_index()),
        }
    }

    /// RGBA de 8 bits expandida: fila a fila, de arriba abajo; el índice transparente, con alfa 0.
    fn rgba(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.celdas.len() * 4);
        for c in &self.celdas {
            match self.modo {
                Mode::Rgba => v.extend_from_slice(c),
                Mode::GrayAlpha => v.extend([c[0], c[0], c[0], c[1]]),
                Mode::Gray => v.extend([c[0], c[0], c[0], 255]),
                Mode::Indexed => {
                    let mut color = self.paleta[usize::from(c[0])];
                    if self.transparente == Some(c[0]) {
                        color[3] = 0;
                    }
                    v.extend(color);
                }
            }
        }
        v
    }

    fn poner(&mut self, x: usize, y: usize, celda: [u8; 4]) {
        self.celdas[y * self.ancho + x] = celda;
    }

    /// Mueve la entrada `de` a la posición `a` (como quitar e insertar) y remapea los píxeles y el
    /// índice transparente.
    fn mover(&mut self, de: usize, a: usize) {
        let mut orden: Vec<usize> = (0..self.paleta.len()).collect();
        let movida = orden.remove(de);
        orden.insert(a, movida);
        let mut adelante = vec![0u8; orden.len()];
        for (nuevo, &antiguo) in orden.iter().enumerate() {
            adelante[antiguo] = nuevo as u8;
        }
        self.paleta = orden.iter().map(|&antiguo| self.paleta[antiguo]).collect();
        for celda in &mut self.celdas {
            celda[0] = adelante[usize::from(celda[0])];
        }
        self.transparente = self.transparente.map(|t| adelante[usize::from(t)]);
    }
}

/// El píxel de `Document` y la celda del modelo para el valor número `k` de ese modo.
fn valor(modo: Mode, k: usize, entradas: usize) -> (Pixel, [u8; 4]) {
    let k8 = (k * 37 % 251) as u8;
    match modo {
        Mode::Rgba => {
            let (r, g, b, a) = (k8, 255 - k8, k8 ^ 0x5A, [255, 128, 0][k % 3]);
            (Pixel::Rgba(Rgba::new(r, g, b, a)), [r, g, b, a])
        }
        Mode::Indexed => {
            let indice = (k * 3 % entradas) as u8;
            (Pixel::Indexed(indice), [indice, 0, 0, 0])
        }
        Mode::Gray => (Pixel::Gray(k8), [k8, 0, 0, 0]),
        Mode::GrayAlpha => {
            let alfa = [255, 128, 0][k % 3];
            (
                Pixel::GrayAlpha {
                    gray: k8,
                    alpha: alfa,
                },
                [k8, alfa, 0, 0],
            )
        }
    }
}

/// Edita el documento con comandos de `px-core` y el modelo con código ingenuo; devuelve el modelo.
fn editar(documento: &mut Document, mut modelo: Modelo) -> Modelo {
    let id = ImageId::from_raw(0);
    let (modo, ancho, alto) = (modelo.modo, modelo.ancho, modelo.alto);
    let entradas = modelo.paleta.len().max(1);

    // `SetPixels`: si una coordenada se repite, gana la última.
    let puntos = [(1, 1), (2, 1), (3, 2), (15, 15), (0, 15), (1, 1)];
    let mut pixeles = Vec::new();
    for (k, (x, y)) in puntos.iter().enumerate() {
        let (x, y) = (x % ancho, y % alto);
        let (pixel, celda) = valor(modo, k + 1, entradas);
        pixeles.push(PixelEdit {
            x: x as u32,
            y: y as u32,
            pixel,
        });
        modelo.poner(x, y, celda);
    }
    documento
        .apply(Command::SetPixels {
            image: id,
            pixels: pixeles,
        })
        .expect("SetPixels");

    // `FillSpans`: tramos estrictamente ordenados, sin solapes.
    let (pixel, celda) = valor(modo, 10, entradas);
    let tramos = [(5, 2, 9), (6, 2, 9), (7, 0, ancho)];
    let mut spans = Vec::new();
    for (y, x0, x1) in tramos {
        let y = y % alto;
        spans.push(Span {
            y: y as u32,
            x_start: x0 as u32,
            x_end: x1 as u32,
        });
        for x in x0..x1 {
            modelo.poner(x, y, celda);
        }
    }
    documento
        .apply(Command::FillSpans {
            image: id,
            spans,
            pixel,
        })
        .expect("FillSpans");

    // Cambio de paleta (solo indexadas): el color de una entrada, mover otra y fijar el transparente.
    if modo == Mode::Indexed {
        documento
            .apply(Command::SetPaletteColor {
                image: id,
                index: 0,
                color: Rgba::new(12, 34, 56, 255),
            })
            .expect("SetPaletteColor");
        modelo.paleta[0] = [12, 34, 56, 255];

        let ultima = modelo.paleta.len() - 1;
        if ultima >= 1 {
            documento
                .apply(Command::MovePaletteEntry {
                    image: id,
                    from: 0,
                    to: ultima as u8,
                })
                .expect("MovePaletteEntry");
            modelo.mover(0, ultima);
        }

        let transparente = 1.min(ultima) as u8;
        documento
            .apply(Command::SetTransparentIndex {
                image: id,
                index: Some(transparente),
            })
            .expect("SetTransparentIndex");
        modelo.transparente = Some(transparente);
    }
    modelo
}

/// Los PNG de `golden/png/` con su `.sha256` (el hash que calculó otra herramienta).
fn reales() -> Vec<(String, Vec<u8>, String)> {
    let dir = carpeta_golden_png();
    nombres_en(&dir)
        .into_iter()
        .filter(|nombre| nombre.ends_with(".png"))
        .map(|nombre| {
            let bytes = fs::read(dir.join(&nombre)).expect("leer el PNG real");
            let hash = fs::read_to_string(dir.join(format!("{nombre}.sha256")))
                .expect("leer el .sha256")
                .trim()
                .to_ascii_lowercase();
            (nombre, bytes, hash)
        })
        .collect()
}

#[test]
fn fila_11_el_sha256_de_la_relectura_es_el_del_modelo_en_los_cuatro_reales() {
    let reales = reales();
    assert_eq!(reales.len(), 4, "golden/png/ debía tener 4 PNG reales");
    for (nombre, bytes, hash) in &reales {
        let leido = read_png(bytes).unwrap_or_else(|e| panic!("{nombre}: read_png: {e}"));
        // El punto de partida es la RGBA que dice el .sha256.
        let inicial = sha256_hex(&bytes_rgba(&leido.image));
        assert_eq!(&inicial, hash, "{nombre}: la lectura no es la del .sha256");

        let modelo = Modelo::de(&leido.image);
        let mut documento = Document::new(leido.image);
        let modelo = editar(&mut documento, modelo);
        let esperado = sha256_hex(&modelo.rgba());
        assert_ne!(
            esperado, inicial,
            "{nombre}: las ediciones no cambiaron nada: la prueba estaría vacía"
        );

        // El documento editado es el modelo.
        let editada = documento
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");
        assert_eq!(
            sha256_hex(&bytes_rgba(editada)),
            esperado,
            "{nombre}: el documento editado no es el modelo"
        );

        // `write_png`, `read_png` y el SHA-256 de la RGBA expandida de la relectura.
        let escrito = write_png(editada).unwrap_or_else(|e| panic!("{nombre}: write_png: {e}"));
        let relectura = read_png(&escrito).unwrap_or_else(|e| panic!("{nombre}: relectura: {e}"));
        assert!(relectura.is_exact(), "{nombre}: la relectura se convirtió");
        assert_eq!(
            sha256_hex(&bytes_rgba(&relectura.image)),
            esperado,
            "{nombre}: la relectura no es el modelo"
        );
    }
}

#[test]
fn fila_11_control_un_modelo_con_un_pixel_cambiado_da_otro_sha256() {
    for (nombre, bytes, _) in &reales() {
        let leido = read_png(bytes).unwrap_or_else(|e| panic!("{nombre}: read_png: {e}"));
        let modelo = Modelo::de(&leido.image);
        let mut documento = Document::new(leido.image);
        let modelo = editar(&mut documento, modelo);
        let editada = documento
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");
        let relectura = read_png(&write_png(editada).expect("write_png")).expect("read_png");
        let leido_sha = sha256_hex(&bytes_rgba(&relectura.image));
        assert_eq!(sha256_hex(&modelo.rgba()), leido_sha, "{nombre}: partida");

        // Un píxel cambiado en el modelo (un canal, en cada uno de varios sitios): otro hash.
        for pixel in [0, modelo.celdas.len() / 2, modelo.celdas.len() - 1] {
            for canal in 0..4 {
                let mut vista = modelo.rgba();
                vista[pixel * 4 + canal] ^= 1;
                assert_ne!(
                    sha256_hex(&vista),
                    leido_sha,
                    "{nombre}: el modelo con el píxel {pixel} (canal {canal}) cambiado dio el mismo SHA-256"
                );
            }
        }
        // Y a nivel del modelo: otra celda, a un valor que cambia el color resuelto.
        let mut otro = modelo.clone();
        let nueva = match otro.modo {
            Mode::Indexed => {
                // Una entrada de la paleta distinta del píxel 0, con otro color resuelto.
                let actual = otro.celdas[0][0];
                let color = |i: u8| {
                    let mut c = otro.paleta[usize::from(i)];
                    if otro.transparente == Some(i) {
                        c[3] = 0;
                    }
                    c
                };
                let alternativa = (0..otro.paleta.len())
                    .map(|i| i as u8)
                    .find(|&i| color(i) != color(actual))
                    .expect("alguna entrada con otro color");
                [alternativa, 0, 0, 0]
            }
            Mode::Rgba => {
                let [r, g, b, a] = otro.celdas[0];
                [r ^ 1, g, b, a]
            }
            Mode::Gray => [otro.celdas[0][0] ^ 1, 0, 0, 0],
            Mode::GrayAlpha => [otro.celdas[0][0] ^ 1, otro.celdas[0][1], 0, 0],
        };
        otro.celdas[0] = nueva;
        assert_ne!(
            sha256_hex(&otro.rgba()),
            leido_sha,
            "{nombre}: el modelo con una celda cambiada dio el mismo SHA-256"
        );
    }
}
