//! Escritor y lector mínimos de ficheros `.aseprite` para las pruebas de la unidad 3.8
//! (specs/hito-3-aseprite.md). Están hechos SOLO con las tablas de la especificación pública del
//! formato (apartados Header, Frames, Chunk Types y Notes): ninguna constante sale de un programa.
//! Cada constante lleva su apartado. Lo usan `aseprite.rs` y `aseprite_reserve.rs`.

use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::decompress_to_vec_zlib;

// ---------------------------------------------------------------------------------------
// Constantes de la especificación.
// ---------------------------------------------------------------------------------------

/// Spec: Header (tamaño de la cabecera).
pub const TAM_CABECERA: usize = 128;
/// Spec: Frames (tamaño de la cabecera de un fotograma).
pub const TAM_CABECERA_FOTOGRAMA: usize = 16;
/// Spec: Header («Magic number»).
pub const MAGIA_FICHERO: u16 = 0xA5E0;
/// Spec: Frames («Magic number»).
pub const MAGIA_FOTOGRAMA: u16 = 0xF1FA;

/// Spec: Header, desplazamientos de los campos (cabecera de 128 bytes).
pub const OFF_TAM_FICHERO: usize = 0;
pub const OFF_MAGIA: usize = 4;
pub const OFF_FOTOGRAMAS: usize = 6;
pub const OFF_ANCHO: usize = 8;
pub const OFF_ALTO: usize = 10;
pub const OFF_PROFUNDIDAD: usize = 12;
pub const OFF_VELOCIDAD: usize = 18;
pub const OFF_TRANSPARENTE: usize = 28;
pub const OFF_RAZON_PIXEL: usize = 34;
/// Spec: Header: `SHORT` x, `SHORT` y, `WORD` ancho y `WORD` alto de la cuadrícula, seguidos.
pub const OFF_REJILLA: usize = 36;
/// Spec: Frames, desplazamientos dentro del primer fotograma (que sigue a la cabecera).
pub const OFF_FOT_TAM: usize = 128;
pub const OFF_FOT_MAGIA: usize = 128 + 4;
pub const OFF_FOT_CHUNKS_ANTIGUO: usize = 128 + 6;
pub const OFF_FOT_DURACION: usize = 128 + 8;
pub const OFF_FOT_CHUNKS_NUEVO: usize = 128 + 12;

/// Spec: Header («Color depth»): 32 = RGBA, 16 = escala de grises, 8 = indexado.
pub const PROFUNDIDAD_RGBA: u16 = 32;
pub const PROFUNDIDAD_GRISES: u16 = 16;
pub const PROFUNDIDAD_INDEXADO: u16 = 8;
/// Spec: Header («Flags»): 1 = la opacidad de capa es válida; 4 = las capas llevan UUID.
pub const CABECERA_OPACIDAD_VALIDA: u32 = 1;
pub const CABECERA_UUID: u32 = 4;

/// Spec: Chunk Types (tipos de chunk).
pub const CHUNK_PALETA_ANTIGUA: u16 = 0x0004;
pub const CHUNK_PALETA_ANTIGUA_6: u16 = 0x0011;
pub const CHUNK_CAPA: u16 = 0x2004;
pub const CHUNK_CELDA: u16 = 0x2005;
pub const CHUNK_CELDA_EXTRA: u16 = 0x2006;
pub const CHUNK_PERFIL: u16 = 0x2007;
pub const CHUNK_FICHEROS: u16 = 0x2008;
pub const CHUNK_MASCARA: u16 = 0x2016;
pub const CHUNK_TRAZO: u16 = 0x2017;
pub const CHUNK_ETIQUETAS: u16 = 0x2018;
pub const CHUNK_PALETA: u16 = 0x2019;
pub const CHUNK_DATOS_USUARIO: u16 = 0x2020;
pub const CHUNK_CORTES: u16 = 0x2022;
pub const CHUNK_TILESET: u16 = 0x2023;

/// Spec: Layer Chunk («Flags»).
pub const CAPA_VISIBLE: u16 = 1;
pub const CAPA_EDITABLE: u16 = 2;
pub const CAPA_BLOQUEO_MOVIMIENTO: u16 = 4;
pub const CAPA_FONDO: u16 = 8;
pub const CAPA_ENLAZADAS: u16 = 16;
pub const CAPA_COLAPSADA: u16 = 32;
pub const CAPA_REFERENCIA: u16 = 64;
/// Spec: Layer Chunk («Layer type»).
pub const CAPA_TIPO_NORMAL: u16 = 0;
pub const CAPA_TIPO_GRUPO: u16 = 1;
pub const CAPA_TIPO_MAPA_TILES: u16 = 2;

/// Spec: Layer Chunk («Blend mode»). Los códigos 6 a 9, 11 a 15, 17 y 18 existen en la tabla
/// pero Pixoko no los admite todavía.
pub const MEZCLA_NORMAL: u16 = 0;
pub const MEZCLA_MULTIPLICAR: u16 = 1;
pub const MEZCLA_TRAMA: u16 = 2;
pub const MEZCLA_SUPERPONER: u16 = 3;
pub const MEZCLA_OSCURECER: u16 = 4;
pub const MEZCLA_ACLARAR: u16 = 5;
pub const MEZCLA_DIFERENCIA: u16 = 10;
pub const MEZCLA_SUMA: u16 = 16;

/// Spec: Cel Chunk («Cel Type»).
pub const CELDA_CRUDA: u16 = 0;
pub const CELDA_ENLAZADA: u16 = 1;
pub const CELDA_COMPRIMIDA: u16 = 2;
pub const CELDA_MAPA_TILES: u16 = 3;

// ---------------------------------------------------------------------------------------
// Escritura de campos (Spec: References, todo en little-endian).
// ---------------------------------------------------------------------------------------

pub fn w16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_le_bytes());
}

pub fn wi16(v: &mut Vec<u8>, x: i16) {
    v.extend(x.to_le_bytes());
}

pub fn w32(v: &mut Vec<u8>, x: u32) {
    v.extend(x.to_le_bytes());
}

/// Spec: References, `STRING`: `WORD` con la longitud en bytes y los caracteres UTF-8.
pub fn cadena(v: &mut Vec<u8>, texto: &[u8]) {
    w16(v, texto.len() as u16);
    v.extend(texto);
}

/// Spec: Frames, formato de un chunk: `DWORD` tamaño (incluye los 6 bytes de su cabecera),
/// `WORD` tipo y los datos.
pub fn chunk(tipo: u16, datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    w32(&mut v, (datos.len() + 6) as u32);
    w16(&mut v, tipo);
    v.extend(datos);
    v
}

/// zlib como lo pide Spec: NOTE.3.
pub fn zlib(datos: &[u8]) -> Vec<u8> {
    compress_to_vec_zlib(datos, 6)
}

/// Descomprime un flujo zlib (Spec: NOTE.3) en el lector de pruebas.
pub fn inflar(datos: &[u8]) -> Vec<u8> {
    decompress_to_vec_zlib(datos).expect("el flujo zlib de la prueba debe descomprimirse")
}

/// Cambia un `DWORD` de un fichero ya construido (para fabricar defectos).
pub fn pon_u32(bytes: &mut [u8], pos: usize, valor: u32) {
    bytes[pos..pos + 4].copy_from_slice(&valor.to_le_bytes());
}

/// Cambia un `WORD` de un fichero ya construido (para fabricar defectos).
pub fn pon_u16(bytes: &mut [u8], pos: usize, valor: u16) {
    bytes[pos..pos + 2].copy_from_slice(&valor.to_le_bytes());
}

// ---------------------------------------------------------------------------------------
// Chunks de capa y de celda.
// ---------------------------------------------------------------------------------------

/// Spec: Layer Chunk.
#[derive(Clone)]
pub struct Capa {
    pub flags: u16,
    pub tipo: u16,
    pub nivel: u16,
    pub mezcla: u16,
    pub opacidad: u8,
    pub nombre: Vec<u8>,
    /// Solo con `tipo == CAPA_TIPO_MAPA_TILES` (Spec: Layer Chunk, «Tileset index»).
    pub tileset: Option<u32>,
}

impl Capa {
    /// Capa normal, visible y editable, Normal, opacidad 255.
    pub fn nueva(nombre: &str) -> Capa {
        Capa {
            flags: CAPA_VISIBLE | CAPA_EDITABLE,
            tipo: CAPA_TIPO_NORMAL,
            nivel: 0,
            mezcla: MEZCLA_NORMAL,
            opacidad: 255,
            nombre: nombre.as_bytes().to_vec(),
            tileset: None,
        }
    }

    pub fn chunk(&self) -> Vec<u8> {
        self.chunk_con_uuid(false)
    }

    /// Con `uuid`, añade el `UUID` de 16 bytes del final (Spec: Layer Chunk, bandera de
    /// cabecera 4).
    pub fn chunk_con_uuid(&self, uuid: bool) -> Vec<u8> {
        let mut d = Vec::new();
        w16(&mut d, self.flags);
        w16(&mut d, self.tipo);
        w16(&mut d, self.nivel);
        w16(&mut d, 0); // ancho por defecto (ignorado)
        w16(&mut d, 0); // alto por defecto (ignorado)
        w16(&mut d, self.mezcla);
        d.push(self.opacidad);
        d.extend([0u8; 3]);
        cadena(&mut d, &self.nombre);
        if let Some(tileset) = self.tileset {
            w32(&mut d, tileset);
        }
        if uuid {
            d.extend([0x5A; 16]);
        }
        chunk(CHUNK_CAPA, &d)
    }
}

/// Spec: Cel Chunk.
#[derive(Clone)]
pub struct Celda {
    pub capa: u16,
    pub x: i16,
    pub y: i16,
    pub opacidad: u8,
    pub tipo: u16,
    pub z: i16,
    pub ancho: u16,
    pub alto: u16,
    /// Píxeles sin comprimir (`PIXEL[]`, fila a fila).
    pub pixeles: Vec<u8>,
    /// Si es `Some`, sustituye al flujo zlib que saldría de `pixeles` (tipo 2).
    pub comprimido: Option<Vec<u8>>,
}

impl Celda {
    /// Celda comprimida (tipo 2), opacidad 255 y z-index 0.
    pub fn nueva(capa: u16, x: i16, y: i16, ancho: u16, alto: u16, pixeles: Vec<u8>) -> Celda {
        Celda {
            capa,
            x,
            y,
            opacidad: 255,
            tipo: CELDA_COMPRIMIDA,
            z: 0,
            ancho,
            alto,
            pixeles,
            comprimido: None,
        }
    }

    pub fn chunk(&self) -> Vec<u8> {
        let mut d = Vec::new();
        w16(&mut d, self.capa);
        wi16(&mut d, self.x);
        wi16(&mut d, self.y);
        d.push(self.opacidad);
        w16(&mut d, self.tipo);
        wi16(&mut d, self.z);
        d.extend([0u8; 5]);
        match self.tipo {
            CELDA_CRUDA => {
                w16(&mut d, self.ancho);
                w16(&mut d, self.alto);
                d.extend(&self.pixeles);
            }
            CELDA_ENLAZADA => w16(&mut d, 0), // fotograma con el que se enlaza
            CELDA_MAPA_TILES => {
                w16(&mut d, self.ancho);
                w16(&mut d, self.alto);
                w16(&mut d, 32); // bits por tile
                for mascara in [0x1fff_ffffu32, 0x2000_0000, 0x4000_0000, 0x8000_0000] {
                    w32(&mut d, mascara);
                }
                d.extend([0u8; 10]);
                d.extend(zlib(&self.pixeles));
            }
            _ => {
                w16(&mut d, self.ancho);
                w16(&mut d, self.alto);
                match &self.comprimido {
                    Some(flujo) => d.extend(flujo),
                    None => d.extend(zlib(&self.pixeles)),
                }
            }
        }
        chunk(CHUNK_CELDA, &d)
    }
}

// ---------------------------------------------------------------------------------------
// Paleta, perfil de color y chunks que se ignoran.
// ---------------------------------------------------------------------------------------

/// Spec: Palette Chunk (0x2019). `entradas`: (R, G, B, A, nombre).
pub fn paleta_nueva(
    tam: u32,
    primero: u32,
    ultimo: u32,
    entradas: &[([u8; 4], Option<&str>)],
) -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, tam);
    w32(&mut d, primero);
    w32(&mut d, ultimo);
    d.extend([0u8; 8]);
    for (rgba, nombre) in entradas {
        w16(&mut d, u16::from(nombre.is_some())); // bandera 1 = lleva nombre
        d.extend(rgba);
        if let Some(nombre) = nombre {
            cadena(&mut d, nombre.as_bytes());
        }
    }
    chunk(CHUNK_PALETA, &d)
}

/// Paleta completa de `colores` en un solo chunk 0x2019, sin nombres.
pub fn paleta_completa(colores: &[[u8; 4]]) -> Vec<u8> {
    let entradas: Vec<([u8; 4], Option<&str>)> = colores.iter().map(|&c| (c, None)).collect();
    paleta_nueva(colores.len() as u32, 0, colores.len() as u32 - 1, &entradas)
}

/// Spec: Old palette chunk (0x0004 o 0x0011). `paquetes`: (saltar, cantidad escrita, colores);
/// la cantidad 0 significa 256.
pub fn paleta_antigua(tipo: u16, paquetes: &[(u8, u8, Vec<[u8; 3]>)]) -> Vec<u8> {
    let mut d = Vec::new();
    w16(&mut d, paquetes.len() as u16);
    for (saltar, cantidad, colores) in paquetes {
        d.push(*saltar);
        d.push(*cantidad);
        for c in colores {
            d.extend(c);
        }
    }
    chunk(tipo, &d)
}

/// Spec: Color Profile Chunk (0x2007). `tipo` 0 = ninguno, 1 = sRGB, 2 = ICC.
pub fn perfil(tipo: u16, flags: u16, gamma: u32, icc: &[u8]) -> Vec<u8> {
    let mut d = Vec::new();
    w16(&mut d, tipo);
    w16(&mut d, flags);
    w32(&mut d, gamma);
    d.extend([0u8; 8]);
    if tipo == 2 {
        w32(&mut d, icc.len() as u32);
        d.extend(icc);
    }
    chunk(CHUNK_PERFIL, &d)
}

pub fn perfil_srgb() -> Vec<u8> {
    perfil(1, 0, 0, &[])
}

/// Spec: Tags Chunk (0x2018), con una etiqueta.
pub fn etiquetas() -> Vec<u8> {
    let mut d = Vec::new();
    w16(&mut d, 1);
    d.extend([0u8; 8]);
    w16(&mut d, 0); // desde el fotograma
    w16(&mut d, 0); // hasta el fotograma
    d.push(0); // dirección
    w16(&mut d, 0); // repeticiones
    d.extend([0u8; 6]);
    d.extend([0u8; 3]); // color RGB (obsoleto)
    d.push(0);
    cadena(&mut d, b"anda");
    chunk(CHUNK_ETIQUETAS, &d)
}

/// Spec: User Data Chunk (0x2020), solo con texto.
pub fn datos_usuario() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 1);
    cadena(&mut d, b"nota");
    chunk(CHUNK_DATOS_USUARIO, &d)
}

/// Spec: Slice Chunk (0x2022), con una clave.
pub fn cortes() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 1);
    w32(&mut d, 0);
    w32(&mut d, 0);
    cadena(&mut d, b"corte");
    w32(&mut d, 0); // fotograma
    w32(&mut d, 0); // x
    w32(&mut d, 0); // y
    w32(&mut d, 2); // ancho
    w32(&mut d, 2); // alto
    chunk(CHUNK_CORTES, &d)
}

/// Spec: External Files Chunk (0x2008), con una entrada.
pub fn ficheros_externos() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 1);
    d.extend([0u8; 8]);
    w32(&mut d, 1); // id de la entrada
    d.push(0); // tipo: paleta externa
    d.extend([0u8; 7]);
    cadena(&mut d, b"paleta.gpl");
    chunk(CHUNK_FICHEROS, &d)
}

/// Spec: Cel Extra Chunk (0x2006).
pub fn celda_extra() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 0);
    d.extend([0u8; 16]); // cuatro FIXED
    d.extend([0u8; 16]);
    chunk(CHUNK_CELDA_EXTRA, &d)
}

/// Spec: Mask Chunk (0x2016), de 8 × 2 píxeles.
pub fn mascara() -> Vec<u8> {
    let mut d = Vec::new();
    wi16(&mut d, 0);
    wi16(&mut d, 0);
    w16(&mut d, 8);
    w16(&mut d, 2);
    d.extend([0u8; 8]);
    cadena(&mut d, b"mascara");
    d.extend([0xFF, 0x0F]); // alto * ((ancho + 7) / 8) bytes
    chunk(CHUNK_MASCARA, &d)
}

/// Spec: Path Chunk (0x2017): «Never used», sin estructura.
pub fn trazo() -> Vec<u8> {
    chunk(CHUNK_TRAZO, &[1, 2, 3])
}

/// Spec: Tileset Chunk (0x2023), sin ficheros externos ni tiles dentro.
pub fn tileset() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 0);
    w32(&mut d, 0);
    w32(&mut d, 0);
    w16(&mut d, 8);
    w16(&mut d, 8);
    wi16(&mut d, 1);
    d.extend([0u8; 14]);
    cadena(&mut d, b"tiles");
    chunk(CHUNK_TILESET, &d)
}

/// Un chunk de un tipo que la especificación no lista.
pub fn desconocido(tipo: u16) -> Vec<u8> {
    chunk(tipo, &[9, 8, 7])
}

// ---------------------------------------------------------------------------------------
// El fichero entero.
// ---------------------------------------------------------------------------------------

/// Un fichero con cabecera y UN fotograma (Spec: Header y Frames). `fotogramas` es lo que
/// declara la cabecera; `resto` son los bytes que siguen al primer fotograma.
#[derive(Clone)]
pub struct Fichero {
    pub ancho: u16,
    pub alto: u16,
    pub profundidad: u16,
    pub flags: u32,
    pub transparente: u8,
    pub colores: u16,
    pub fotogramas: u16,
    pub chunks: Vec<Vec<u8>>,
    pub resto: Vec<u8>,
}

impl Fichero {
    pub fn rgba(ancho: u16, alto: u16, chunks: Vec<Vec<u8>>) -> Fichero {
        Fichero {
            ancho,
            alto,
            profundidad: PROFUNDIDAD_RGBA,
            flags: CABECERA_OPACIDAD_VALIDA,
            transparente: 0,
            colores: 0,
            fotogramas: 1,
            chunks,
            resto: Vec::new(),
        }
    }

    pub fn indexado(
        ancho: u16,
        alto: u16,
        transparente: u8,
        colores: u16,
        chunks: Vec<Vec<u8>>,
    ) -> Fichero {
        Fichero {
            profundidad: PROFUNDIDAD_INDEXADO,
            transparente,
            colores,
            ..Fichero::rgba(ancho, alto, chunks)
        }
    }

    /// Los bytes del fichero, con los tamaños y contadores coherentes.
    pub fn bytes(&self) -> Vec<u8> {
        let cuerpo: Vec<u8> = self.chunks.iter().flatten().copied().collect();
        let tam_fotograma = TAM_CABECERA_FOTOGRAMA + cuerpo.len();
        let total = TAM_CABECERA + tam_fotograma + self.resto.len();
        let mut v = Vec::with_capacity(total);
        // Spec: Header.
        w32(&mut v, total as u32);
        w16(&mut v, MAGIA_FICHERO);
        w16(&mut v, self.fotogramas);
        w16(&mut v, self.ancho);
        w16(&mut v, self.alto);
        w16(&mut v, self.profundidad);
        w32(&mut v, self.flags);
        w16(&mut v, 100); // velocidad
        w32(&mut v, 0);
        w32(&mut v, 0);
        v.push(self.transparente);
        v.extend([0u8; 3]);
        w16(&mut v, self.colores);
        v.push(1); // ancho de píxel
        v.push(1); // alto de píxel
        wi16(&mut v, 0); // cuadrícula x
        wi16(&mut v, 0); // cuadrícula y
        w16(&mut v, 16); // ancho de la cuadrícula
        w16(&mut v, 16); // alto de la cuadrícula
        v.extend([0u8; 84]);
        assert_eq!(v.len(), TAM_CABECERA);
        // Spec: Frames.
        w32(&mut v, tam_fotograma as u32);
        w16(&mut v, MAGIA_FOTOGRAMA);
        w16(&mut v, self.chunks.len().min(0xFFFF) as u16);
        w16(&mut v, 100); // duración
        v.extend([0u8; 2]);
        w32(&mut v, self.chunks.len() as u32);
        v.extend(cuerpo);
        v.extend(&self.resto);
        v
    }
}

/// Píxeles RGBA fila a fila: `f(x, y)` da `[r, g, b, a]`.
pub fn pixeles_rgba(ancho: usize, alto: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> Vec<u8> {
    let mut v = Vec::with_capacity(ancho * alto * 4);
    for y in 0..alto {
        for x in 0..ancho {
            v.extend(f(x, y));
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// Lector mínimo (solo para comprobar lo que escribe `write_aseprite`).
// ---------------------------------------------------------------------------------------

/// Cursor sobre bytes; en la prueba un fin de datos inesperado es un fallo de la prueba.
pub struct Lector<'a> {
    datos: &'a [u8],
    pub pos: usize,
}

impl<'a> Lector<'a> {
    pub fn nuevo(datos: &'a [u8]) -> Lector<'a> {
        Lector { datos, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let trozo = self
            .datos
            .get(self.pos..self.pos + n)
            .expect("fin de datos inesperado en el lector de la prueba");
        self.pos += n;
        trozo
    }

    pub fn u8(&mut self) -> u8 {
        self.bytes(1)[0]
    }

    pub fn u16(&mut self) -> u16 {
        let b = self.bytes(2);
        u16::from_le_bytes([b[0], b[1]])
    }

    pub fn i16(&mut self) -> i16 {
        self.u16() as i16
    }

    pub fn u32(&mut self) -> u32 {
        let b = self.bytes(4);
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    pub fn resto(&self) -> usize {
        self.datos.len() - self.pos
    }
}

/// Spec: Header, campo a campo.
#[derive(Debug)]
pub struct CabeceraLeida {
    pub tam_fichero: u32,
    pub magia: u16,
    pub fotogramas: u16,
    pub ancho: u16,
    pub alto: u16,
    pub profundidad: u16,
    pub flags: u32,
    pub velocidad: u16,
    pub dwords_cero: [u32; 2],
    pub transparente: u8,
    pub ignorados: [u8; 3],
    pub colores: u16,
    pub pixel: (u8, u8),
    pub rejilla: (i16, i16, u16, u16),
    pub futuro_a_cero: bool,
}

/// Spec: Frames, cabecera del primer fotograma.
#[derive(Debug)]
pub struct FotogramaLeido {
    pub tam: u32,
    pub magia: u16,
    pub chunks_antiguo: u16,
    pub duracion: u16,
    pub futuro: [u8; 2],
    pub chunks_nuevo: u32,
}

pub struct FicheroLeido {
    pub cabecera: CabeceraLeida,
    pub fotograma: FotogramaLeido,
    /// (tipo, datos) de cada chunk del primer fotograma, en orden.
    pub chunks: Vec<(u16, Vec<u8>)>,
}

/// Lee un fichero de un fotograma: cabecera, cabecera del fotograma y chunks.
pub fn parsea(bytes: &[u8]) -> FicheroLeido {
    let mut l = Lector::nuevo(bytes);
    let tam_fichero = l.u32();
    let magia = l.u16();
    let fotogramas = l.u16();
    let ancho = l.u16();
    let alto = l.u16();
    let profundidad = l.u16();
    let flags = l.u32();
    let velocidad = l.u16();
    let dwords_cero = [l.u32(), l.u32()];
    let transparente = l.u8();
    let ignorados = [l.u8(), l.u8(), l.u8()];
    let colores = l.u16();
    let pixel = (l.u8(), l.u8());
    let rejilla = (l.i16(), l.i16(), l.u16(), l.u16());
    let futuro_a_cero = l.bytes(84).iter().all(|&b| b == 0);
    assert_eq!(l.pos, TAM_CABECERA, "la cabecera mide 128 bytes");
    let cabecera = CabeceraLeida {
        tam_fichero,
        magia,
        fotogramas,
        ancho,
        alto,
        profundidad,
        flags,
        velocidad,
        dwords_cero,
        transparente,
        ignorados,
        colores,
        pixel,
        rejilla,
        futuro_a_cero,
    };
    let fotograma = FotogramaLeido {
        tam: l.u32(),
        magia: l.u16(),
        chunks_antiguo: l.u16(),
        duracion: l.u16(),
        futuro: [l.u8(), l.u8()],
        chunks_nuevo: l.u32(),
    };
    let mut chunks = Vec::new();
    for _ in 0..fotograma.chunks_nuevo {
        let tam = l.u32() as usize;
        let tipo = l.u16();
        chunks.push((tipo, l.bytes(tam - 6).to_vec()));
    }
    assert_eq!(l.resto(), 0, "no sobran bytes tras el primer fotograma");
    FicheroLeido {
        cabecera,
        fotograma,
        chunks,
    }
}

/// Spec: Layer Chunk, campo a campo.
#[derive(Debug)]
pub struct CapaLeida {
    pub flags: u16,
    pub tipo: u16,
    pub nivel: u16,
    pub ancho: u16,
    pub alto: u16,
    pub mezcla: u16,
    pub opacidad: u8,
    pub futuro: [u8; 3],
    pub nombre: String,
}

pub fn decodifica_capa(datos: &[u8]) -> CapaLeida {
    let mut l = Lector::nuevo(datos);
    let flags = l.u16();
    let tipo = l.u16();
    let nivel = l.u16();
    let ancho = l.u16();
    let alto = l.u16();
    let mezcla = l.u16();
    let opacidad = l.u8();
    let futuro = [l.u8(), l.u8(), l.u8()];
    let n = usize::from(l.u16());
    let nombre = String::from_utf8(l.bytes(n).to_vec()).expect("nombre UTF-8");
    assert_eq!(l.resto(), 0, "la capa no lleva más datos");
    CapaLeida {
        flags,
        tipo,
        nivel,
        ancho,
        alto,
        mezcla,
        opacidad,
        futuro,
        nombre,
    }
}

/// Spec: Cel Chunk de tipo 2, con los píxeles ya descomprimidos.
#[derive(Debug)]
pub struct CeldaLeida {
    pub capa: u16,
    pub x: i16,
    pub y: i16,
    pub opacidad: u8,
    pub tipo: u16,
    pub z: i16,
    pub futuro: [u8; 5],
    pub ancho: u16,
    pub alto: u16,
    pub pixeles: Vec<u8>,
}

pub fn decodifica_celda(datos: &[u8]) -> CeldaLeida {
    let mut l = Lector::nuevo(datos);
    let capa = l.u16();
    let x = l.i16();
    let y = l.i16();
    let opacidad = l.u8();
    let tipo = l.u16();
    let z = l.i16();
    let futuro = [l.u8(), l.u8(), l.u8(), l.u8(), l.u8()];
    let ancho = l.u16();
    let alto = l.u16();
    let resto = l.bytes(l.resto());
    let pixeles = if tipo == CELDA_COMPRIMIDA {
        inflar(resto)
    } else {
        resto.to_vec()
    };
    CeldaLeida {
        capa,
        x,
        y,
        opacidad,
        tipo,
        z,
        futuro,
        ancho,
        alto,
        pixeles,
    }
}
