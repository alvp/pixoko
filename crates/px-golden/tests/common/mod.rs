//! Ayudantes compartidos por los tests de `px-golden` (specs/hito-1-png-corpus.md).
//!
//! Constructor de PNG a mano: chunks, CRC-32, Adler-32 y deflate «stored», todo escrito sin
//! `flate2`, `crc32fast` ni `png`. Es una copia deliberada del ayudante de
//! `crates/px-formats/tests/png.rs` (decisión 3 de la spec), sin las pasadas Adam7 ni los
//! mecanismos que aquí no se usan (Adler-32 erróneo, datos crudos fijos, chunks tras el IDAT).
//! Si hace falta un tercer uso, se unifican.
//!
//! Cada fichero de `tests/*.rs` es un binario de test distinto y solo usa una parte de este
//! módulo, así que lo que uno no necesita le saldría como código muerto; de ahí el `allow`.
#![allow(dead_code)]

pub mod sha256;

pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// CRC-32 de la especificación PNG (polinomio 0xEDB88320, bit a bit).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Adler-32 de la especificación zlib (RFC 1950).
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flujo zlib con bloques deflate «stored» (sin comprimir).
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut v = vec![0x78, 0x01];
    if data.is_empty() {
        v.extend([1, 0, 0, 0xFF, 0xFF]);
    } else {
        let blocks = data.chunks(65535).count();
        for (i, block) in data.chunks(65535).enumerate() {
            v.push(u8::from(i + 1 == blocks));
            let n = block.len() as u16;
            v.extend(n.to_le_bytes());
            v.extend((!n).to_le_bytes());
            v.extend(block);
        }
    }
    v.extend(adler32(data).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
pub fn piece(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + data.len());
    v.extend((data.len() as u32).to_be_bytes());
    v.extend(kind);
    v.extend(data);
    let mut body = kind.to_vec();
    body.extend(data);
    v.extend(crc32(&body).to_be_bytes());
    v
}

pub fn plte(colors: &[(u8, u8, u8)]) -> Vec<u8> {
    let data: Vec<u8> = colors.iter().flat_map(|&(r, g, b)| [r, g, b]).collect();
    piece(b"PLTE", &data)
}

pub fn trns(data: &[u8]) -> Vec<u8> {
    piece(b"tRNS", data)
}

/// Canales por píxel de cada tipo de color PNG.
pub fn channels(color: u8) -> usize {
    match color {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    }
}

/// Empaqueta las muestras de una fila: sub-8 bits de más a menos significativo con relleno
/// a byte, 8 bits tal cual y 16 bits en big-endian.
pub fn packs(row: &[u16], prof: u8) -> Vec<u8> {
    match prof {
        16 => row.iter().flat_map(|v| v.to_be_bytes()).collect(),
        8 => row.iter().map(|&v| v as u8).collect(),
        _ => {
            let p = usize::from(prof);
            row.chunks(8 / p)
                .map(|group| {
                    let mut byte = 0u8;
                    for (i, &v) in group.iter().enumerate() {
                        byte |= (v as u8) << (8 - p * (i + 1));
                    }
                    byte
                })
                .collect()
        }
    }
}

/// Datos sin comprimir del IDAT: cada fila con su byte de filtro 0 y las muestras empaquetadas.
pub fn scans(width: usize, height: usize, channels: usize, prof: u8, m: &[u16]) -> Vec<u8> {
    let mut output = Vec::new();
    for y in 0..height {
        let row = &m[y * width * channels..(y + 1) * width * channels];
        output.push(0);
        output.extend(packs(row, prof));
    }
    output
}

/// Fichero PNG en construcción, sin entrelazado. Con `swatches` vacío no se genera ningún
/// dato de imagen (así se hacen ficheros de cabecera enorme y datos mínimos).
#[derive(Clone)]
pub struct Png {
    width: u32,
    height: u32,
    prof: u8,
    color: u8,
    /// Muestras por píxel y canal, fila a fila, en el orden de la imagen final.
    swatches: Vec<u16>,
    /// Chunks ya serializados entre IHDR e IDAT.
    antes: Vec<Vec<u8>>,
}

impl Png {
    pub fn new(width: u32, height: u32, prof: u8, color: u8) -> Png {
        Png {
            width,
            height,
            prof,
            color,
            swatches: Vec::new(),
            antes: Vec::new(),
        }
    }

    pub fn swatches(mut self, m: &[u16]) -> Png {
        self.swatches = m.to_vec();
        self
    }

    pub fn antes(mut self, chunk: Vec<u8>) -> Png {
        self.antes.push(chunk);
        self
    }

    fn raw(&self) -> Vec<u8> {
        if self.swatches.is_empty() {
            return Vec::new();
        }
        scans(
            self.width as usize,
            self.height as usize,
            channels(self.color),
            self.prof,
            &self.swatches,
        )
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut ihdr = Vec::new();
        ihdr.extend(self.width.to_be_bytes());
        ihdr.extend(self.height.to_be_bytes());
        ihdr.extend([self.prof, self.color, 0, 0, 0]);
        let mut v = SIGNATURE.to_vec();
        v.extend(piece(b"IHDR", &ihdr));
        for c in &self.antes {
            v.extend(c);
        }
        v.extend(piece(b"IDAT", &zlib_stored(&self.raw())));
        v.extend(piece(b"IEND", &[]));
        v
    }
}

/// Un chunk encontrado por el recorredor: tipo, posición del campo de longitud y datos.
pub struct Piece {
    pub kind: [u8; 4],
    pub start: usize,
    pub data: Vec<u8>,
}

/// Recorre la estructura de chunks de un fichero bien formado: firma, longitud, tipo, datos
/// y CRC de cada chunk, hasta un IEND que termina exactamente en el último byte.
pub fn walks(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return Err("firma incorrecta".into());
    }
    let mut pos = 8usize;
    let mut pieces = Vec::new();
    loop {
        let header = bytes
            .get(pos..pos + 8)
            .ok_or_else(|| format!("cabecera truncada en {pos}"))?;
        let largo = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = header[4..8].try_into().unwrap();
        let data = bytes
            .get(pos + 8..pos + 8 + largo)
            .ok_or_else(|| format!("datos truncados en {pos}"))?;
        let crc = bytes
            .get(pos + 8 + largo..pos + 12 + largo)
            .ok_or_else(|| format!("CRC truncado en {pos}"))?;
        let mut body = kind.to_vec();
        body.extend(data);
        if u32::from_be_bytes(crc.try_into().unwrap()) != crc32(&body) {
            return Err(format!("CRC incorrecto en {pos}"));
        }
        pieces.push(Piece {
            kind,
            start: pos,
            data: data.to_vec(),
        });
        pos += 12 + largo;
        if kind == *b"IEND" {
            if pos != bytes.len() {
                return Err(format!("sobran {} bytes tras IEND", bytes.len() - pos));
            }
            return Ok(pieces);
        }
    }
}

/// Posición del chunk IDAT y longitud de sus datos.
pub fn position_idat(bytes: &[u8]) -> (usize, usize) {
    let t = walks(bytes)
        .unwrap()
        .into_iter()
        .find(|t| t.kind == *b"IDAT")
        .unwrap();
    (t.start, t.data.len())
}
