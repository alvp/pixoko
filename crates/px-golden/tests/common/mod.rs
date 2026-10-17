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

pub const FIRMA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// CRC-32 de la especificación PNG (polinomio 0xEDB88320, bit a bit).
pub fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in datos {
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
pub fn adler32(datos: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in datos {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flujo zlib con bloques deflate «stored» (sin comprimir).
pub fn zlib_stored(datos: &[u8]) -> Vec<u8> {
    let mut v = vec![0x78, 0x01];
    if datos.is_empty() {
        v.extend([1, 0, 0, 0xFF, 0xFF]);
    } else {
        let bloques = datos.chunks(65535).count();
        for (i, bloque) in datos.chunks(65535).enumerate() {
            v.push(u8::from(i + 1 == bloques));
            let n = bloque.len() as u16;
            v.extend(n.to_le_bytes());
            v.extend((!n).to_le_bytes());
            v.extend(bloque);
        }
    }
    v.extend(adler32(datos).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
pub fn trozo(tipo: &[u8; 4], datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + datos.len());
    v.extend((datos.len() as u32).to_be_bytes());
    v.extend(tipo);
    v.extend(datos);
    let mut cuerpo = tipo.to_vec();
    cuerpo.extend(datos);
    v.extend(crc32(&cuerpo).to_be_bytes());
    v
}

pub fn plte(colores: &[(u8, u8, u8)]) -> Vec<u8> {
    let datos: Vec<u8> = colores.iter().flat_map(|&(r, g, b)| [r, g, b]).collect();
    trozo(b"PLTE", &datos)
}

pub fn trns(datos: &[u8]) -> Vec<u8> {
    trozo(b"tRNS", datos)
}

/// Canales por píxel de cada tipo de color PNG.
pub fn canales(color: u8) -> usize {
    match color {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    }
}

/// Empaqueta las muestras de una fila: sub-8 bits de más a menos significativo con relleno
/// a byte, 8 bits tal cual y 16 bits en big-endian.
pub fn empaqueta(fila: &[u16], prof: u8) -> Vec<u8> {
    match prof {
        16 => fila.iter().flat_map(|v| v.to_be_bytes()).collect(),
        8 => fila.iter().map(|&v| v as u8).collect(),
        _ => {
            let p = usize::from(prof);
            fila.chunks(8 / p)
                .map(|grupo| {
                    let mut byte = 0u8;
                    for (i, &v) in grupo.iter().enumerate() {
                        byte |= (v as u8) << (8 - p * (i + 1));
                    }
                    byte
                })
                .collect()
        }
    }
}

/// Datos sin comprimir del IDAT: cada fila con su byte de filtro 0 y las muestras empaquetadas.
pub fn escaneos(ancho: usize, alto: usize, canales: usize, prof: u8, m: &[u16]) -> Vec<u8> {
    let mut salida = Vec::new();
    for y in 0..alto {
        let fila = &m[y * ancho * canales..(y + 1) * ancho * canales];
        salida.push(0);
        salida.extend(empaqueta(fila, prof));
    }
    salida
}

/// Fichero PNG en construcción, sin entrelazado. Con `muestras` vacío no se genera ningún
/// dato de imagen (así se hacen ficheros de cabecera enorme y datos mínimos).
#[derive(Clone)]
pub struct Png {
    ancho: u32,
    alto: u32,
    prof: u8,
    color: u8,
    /// Muestras por píxel y canal, fila a fila, en el orden de la imagen final.
    muestras: Vec<u16>,
    /// Chunks ya serializados entre IHDR e IDAT.
    antes: Vec<Vec<u8>>,
}

impl Png {
    pub fn new(ancho: u32, alto: u32, prof: u8, color: u8) -> Png {
        Png {
            ancho,
            alto,
            prof,
            color,
            muestras: Vec::new(),
            antes: Vec::new(),
        }
    }

    pub fn muestras(mut self, m: &[u16]) -> Png {
        self.muestras = m.to_vec();
        self
    }

    pub fn antes(mut self, chunk: Vec<u8>) -> Png {
        self.antes.push(chunk);
        self
    }

    fn crudo(&self) -> Vec<u8> {
        if self.muestras.is_empty() {
            return Vec::new();
        }
        escaneos(
            self.ancho as usize,
            self.alto as usize,
            canales(self.color),
            self.prof,
            &self.muestras,
        )
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut ihdr = Vec::new();
        ihdr.extend(self.ancho.to_be_bytes());
        ihdr.extend(self.alto.to_be_bytes());
        ihdr.extend([self.prof, self.color, 0, 0, 0]);
        let mut v = FIRMA.to_vec();
        v.extend(trozo(b"IHDR", &ihdr));
        for c in &self.antes {
            v.extend(c);
        }
        v.extend(trozo(b"IDAT", &zlib_stored(&self.crudo())));
        v.extend(trozo(b"IEND", &[]));
        v
    }
}

/// Un chunk encontrado por el recorredor: tipo, posición del campo de longitud y datos.
pub struct Trozo {
    pub tipo: [u8; 4],
    pub inicio: usize,
    pub datos: Vec<u8>,
}

/// Recorre la estructura de chunks de un fichero bien formado: firma, longitud, tipo, datos
/// y CRC de cada chunk, hasta un IEND que termina exactamente en el último byte.
pub fn recorre(bytes: &[u8]) -> Result<Vec<Trozo>, String> {
    if bytes.len() < 8 || bytes[..8] != FIRMA {
        return Err("firma incorrecta".into());
    }
    let mut pos = 8usize;
    let mut trozos = Vec::new();
    loop {
        let cabecera = bytes
            .get(pos..pos + 8)
            .ok_or_else(|| format!("cabecera truncada en {pos}"))?;
        let largo = u32::from_be_bytes(cabecera[..4].try_into().unwrap()) as usize;
        let tipo: [u8; 4] = cabecera[4..8].try_into().unwrap();
        let datos = bytes
            .get(pos + 8..pos + 8 + largo)
            .ok_or_else(|| format!("datos truncados en {pos}"))?;
        let crc = bytes
            .get(pos + 8 + largo..pos + 12 + largo)
            .ok_or_else(|| format!("CRC truncado en {pos}"))?;
        let mut cuerpo = tipo.to_vec();
        cuerpo.extend(datos);
        if u32::from_be_bytes(crc.try_into().unwrap()) != crc32(&cuerpo) {
            return Err(format!("CRC incorrecto en {pos}"));
        }
        trozos.push(Trozo {
            tipo,
            inicio: pos,
            datos: datos.to_vec(),
        });
        pos += 12 + largo;
        if tipo == *b"IEND" {
            if pos != bytes.len() {
                return Err(format!("sobran {} bytes tras IEND", bytes.len() - pos));
            }
            return Ok(trozos);
        }
    }
}

/// Posición del chunk IDAT y longitud de sus datos.
pub fn posicion_idat(bytes: &[u8]) -> (usize, usize) {
    let t = recorre(bytes)
        .unwrap()
        .into_iter()
        .find(|t| t.tipo == *b"IDAT")
        .unwrap();
    (t.inicio, t.datos.len())
}
