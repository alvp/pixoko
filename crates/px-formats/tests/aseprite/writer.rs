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
pub const TAM_HEADER: usize = 128;
/// Spec: Frames (tamaño de la cabecera de un fotograma).
pub const TAM_HEADER_FRAME: usize = 16;
/// Spec: Header («Magic number»).
pub const TEST_MAGIC_FILE: u16 = 0xA5E0;
/// Spec: Frames («Magic number»).
pub const TEST_MAGIC_FRAME: u16 = 0xF1FA;

/// Spec: Header, desplazamientos de los campos (cabecera de 128 bytes).
pub const OFF_TAM_FILE: usize = 0;
pub const OFF_MAGIC: usize = 4;
pub const OFF_FRAMES: usize = 6;
pub const OFF_WIDTH: usize = 8;
pub const OFF_HEIGHT: usize = 10;
pub const OFF_DEPTH: usize = 12;
pub const OFF_SPEED: usize = 18;
pub const OFF_TRANSPARENT: usize = 28;
pub const OFF_REASON_PIXEL: usize = 34;
/// Spec: Header: `SHORT` x, `SHORT` y, `WORD` ancho y `WORD` alto de la cuadrícula, seguidos.
pub const OFF_GRID: usize = 36;
/// Spec: Frames, desplazamientos dentro del primer fotograma (que sigue a la cabecera).
pub const OFF_FRAME_TAM: usize = 128;
pub const OFF_FRAME_MAGIC: usize = 128 + 4;
pub const OFF_FRAME_CHUNKS_OLD: usize = 128 + 6;
pub const OFF_FRAME_DURATION: usize = 128 + 8;
pub const OFF_FRAME_CHUNKS_NEW: usize = 128 + 12;

/// Spec: Header («Color depth»): 32 = RGBA, 16 = escala de grises, 8 = indexado.
pub const TEST_DEPTH_RGBA: u16 = 32;
pub const DEPTH_GRAYS: u16 = 16;
pub const TEST_DEPTH_INDEXED: u16 = 8;
/// Spec: Header («Flags»): 1 = la opacidad de capa es válida; 4 = las capas llevan UUID.
pub const HEADER_OPACITY_VALID: u32 = 1;
pub const HEADER_UUID: u32 = 4;

/// Spec: Chunk Types (tipos de chunk).
pub const CHUNK_PALETTE_OLD: u16 = 0x0004;
pub const CHUNK_OLD_PALETTE_6: u16 = 0x0011;
pub const TEST_CHUNK_LAYER: u16 = 0x2004;
pub const TEST_CHUNK_CEL: u16 = 0x2005;
pub const TEST_CHUNK_CEL_EXTRA: u16 = 0x2006;
pub const CHUNK_PROFILE: u16 = 0x2007;
pub const CHUNK_FILES: u16 = 0x2008;
pub const CHUNK_MASCARA: u16 = 0x2016;
pub const CHUNK_STROKE: u16 = 0x2017;
pub const CHUNK_LABELS: u16 = 0x2018;
pub const TEST_CHUNK_PALETTE: u16 = 0x2019;
pub const CHUNK_DATA_USER: u16 = 0x2020;
pub const CHUNK_CUTS: u16 = 0x2022;
pub const CHUNK_TILESET: u16 = 0x2023;

/// Spec: Layer Chunk («Flags»).
pub const TEST_LAYER_VISIBLE: u16 = 1;
pub const TEST_LAYER_EDITABLE: u16 = 2;
pub const LAYER_LOCK_MOVEMENT: u16 = 4;
pub const TEST_LAYER_BACKGROUND: u16 = 8;
pub const LAYER_LINKED: u16 = 16;
pub const LAYER_COLLAPSED: u16 = 32;
pub const TEST_LAYER_REFERENCE: u16 = 64;
/// Spec: Layer Chunk («Layer type»).
pub const TEST_LAYER_TYPE_NORMAL: u16 = 0;
pub const TEST_LAYER_TYPE_GROUP: u16 = 1;
pub const TEST_LAYER_TYPE_TILEMAP: u16 = 2;

/// Spec: Layer Chunk («Blend mode»). Los códigos 6 a 9, 11 a 15, 17 y 18 existen en la tabla
/// pero Pixoko no los admite todavía.
pub const BLEND_NORMAL: u16 = 0;
pub const BLEND_MULTIPLY: u16 = 1;
pub const BLEND_PATTERN: u16 = 2;
pub const BLEND_OVERLAY: u16 = 3;
pub const BLEND_DARKEN: u16 = 4;
pub const BLEND_LIGHTEN: u16 = 5;
pub const BLEND_DIFFERENCE: u16 = 10;
pub const BLEND_SUM: u16 = 16;

/// Spec: Cel Chunk («Cel Type»).
pub const TEST_CEL_RAW: u16 = 0;
pub const TEST_CEL_LINKED: u16 = 1;
pub const TEST_CEL_COMPRESSED: u16 = 2;
pub const CEL_MAP_TILES: u16 = 3;

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
pub fn string(v: &mut Vec<u8>, text: &[u8]) {
    w16(v, text.len() as u16);
    v.extend(text);
}

/// Spec: Frames, formato de un chunk: `DWORD` tamaño (incluye los 6 bytes de su cabecera),
/// `WORD` tipo y los datos.
pub fn chunk(kind: u16, data: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    w32(&mut v, (data.len() + 6) as u32);
    w16(&mut v, kind);
    v.extend(data);
    v
}

/// zlib como lo pide Spec: NOTE.3.
pub fn zlib(data: &[u8]) -> Vec<u8> {
    compress_to_vec_zlib(data, 6)
}

/// Descomprime un flujo zlib (Spec: NOTE.3) en el lector de pruebas.
pub fn inflate(data: &[u8]) -> Vec<u8> {
    decompress_to_vec_zlib(data).expect("el flujo zlib de la prueba debe descomprimirse")
}

/// Cambia un `DWORD` de un fichero ya construido (para fabricar defectos).
pub fn put_u_32(bytes: &mut [u8], pos: usize, valor: u32) {
    bytes[pos..pos + 4].copy_from_slice(&valor.to_le_bytes());
}

/// Cambia un `WORD` de un fichero ya construido (para fabricar defectos).
pub fn put_u_16(bytes: &mut [u8], pos: usize, valor: u16) {
    bytes[pos..pos + 2].copy_from_slice(&valor.to_le_bytes());
}

// ---------------------------------------------------------------------------------------
// Chunks de capa y de celda.
// ---------------------------------------------------------------------------------------

/// Spec: Layer Chunk.
#[derive(Clone)]
pub struct TestLayer {
    pub flags: u16,
    pub kind: u16,
    pub level: u16,
    pub blend: u16,
    pub opacity: u8,
    pub name_2: Vec<u8>,
    /// Solo con `kind == TEST_LAYER_TYPE_TILEMAP` (Spec: Layer Chunk, «Tileset index»).
    pub tileset: Option<u32>,
}

impl TestLayer {
    /// Capa normal, visible y editable, Normal, opacidad 255.
    pub fn new(name_2: &str) -> TestLayer {
        TestLayer {
            flags: TEST_LAYER_VISIBLE | TEST_LAYER_EDITABLE,
            kind: TEST_LAYER_TYPE_NORMAL,
            level: 0,
            blend: BLEND_NORMAL,
            opacity: 255,
            name_2: name_2.as_bytes().to_vec(),
            tileset: None,
        }
    }

    pub fn chunk(&self) -> Vec<u8> {
        self.chunk_with_uuid(false)
    }

    /// Con `uuid`, añade el `UUID` de 16 bytes del final (Spec: Layer Chunk, bandera de
    /// cabecera 4).
    pub fn chunk_with_uuid(&self, uuid: bool) -> Vec<u8> {
        let mut d = Vec::new();
        w16(&mut d, self.flags);
        w16(&mut d, self.kind);
        w16(&mut d, self.level);
        w16(&mut d, 0); // ancho por defecto (ignorado)
        w16(&mut d, 0); // alto por defecto (ignorado)
        w16(&mut d, self.blend);
        d.push(self.opacity);
        d.extend([0u8; 3]);
        string(&mut d, &self.name_2);
        if let Some(tileset) = self.tileset {
            w32(&mut d, tileset);
        }
        if uuid {
            d.extend([0x5A; 16]);
        }
        chunk(TEST_CHUNK_LAYER, &d)
    }
}

/// Spec: Cel Chunk.
#[derive(Clone)]
pub struct Cel {
    pub layer_2: u16,
    pub x: i16,
    pub y: i16,
    pub opacity: u8,
    pub kind: u16,
    pub z: i16,
    pub width: u16,
    pub height: u16,
    /// Píxeles sin comprimir (`PIXEL[]`, fila a fila).
    pub pixels: Vec<u8>,
    /// Si es `Some`, sustituye al flujo zlib que saldría de `pixels` (tipo 2).
    pub compressed: Option<Vec<u8>>,
}

impl Cel {
    /// Celda comprimida (tipo 2), opacidad 255 y z-index 0.
    pub fn new(layer_2: u16, x: i16, y: i16, width: u16, height: u16, pixels: Vec<u8>) -> Cel {
        Cel {
            layer_2,
            x,
            y,
            opacity: 255,
            kind: TEST_CEL_COMPRESSED,
            z: 0,
            width,
            height,
            pixels,
            compressed: None,
        }
    }

    pub fn chunk(&self) -> Vec<u8> {
        let mut d = Vec::new();
        w16(&mut d, self.layer_2);
        wi16(&mut d, self.x);
        wi16(&mut d, self.y);
        d.push(self.opacity);
        w16(&mut d, self.kind);
        wi16(&mut d, self.z);
        d.extend([0u8; 5]);
        match self.kind {
            TEST_CEL_RAW => {
                w16(&mut d, self.width);
                w16(&mut d, self.height);
                d.extend(&self.pixels);
            }
            TEST_CEL_LINKED => w16(&mut d, 0), // fotograma con el que se enlaza
            CEL_MAP_TILES => {
                w16(&mut d, self.width);
                w16(&mut d, self.height);
                w16(&mut d, 32); // bits por tile
                for mascara in [0x1fff_ffffu32, 0x2000_0000, 0x4000_0000, 0x8000_0000] {
                    w32(&mut d, mascara);
                }
                d.extend([0u8; 10]);
                d.extend(zlib(&self.pixels));
            }
            _ => {
                w16(&mut d, self.width);
                w16(&mut d, self.height);
                match &self.compressed {
                    Some(flow) => d.extend(flow),
                    None => d.extend(zlib(&self.pixels)),
                }
            }
        }
        chunk(TEST_CHUNK_CEL, &d)
    }
}

// ---------------------------------------------------------------------------------------
// Paleta, perfil de color y chunks que se ignoran.
// ---------------------------------------------------------------------------------------

/// Spec: Palette Chunk (0x2019). `entries`: (R, G, B, A, nombre).
pub fn palette_new(
    tam: u32,
    first: u32,
    ultimo: u32,
    entries: &[([u8; 4], Option<&str>)],
) -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, tam);
    w32(&mut d, first);
    w32(&mut d, ultimo);
    d.extend([0u8; 8]);
    for (rgba, name_2) in entries {
        w16(&mut d, u16::from(name_2.is_some())); // bandera 1 = lleva nombre
        d.extend(rgba);
        if let Some(name_2) = name_2 {
            string(&mut d, name_2.as_bytes());
        }
    }
    chunk(TEST_CHUNK_PALETTE, &d)
}

/// Paleta completa de `colors` en un solo chunk 0x2019, sin nombres.
pub fn palette_complete(colors: &[[u8; 4]]) -> Vec<u8> {
    let entries: Vec<([u8; 4], Option<&str>)> = colors.iter().map(|&c| (c, None)).collect();
    palette_new(colors.len() as u32, 0, colors.len() as u32 - 1, &entries)
}

/// Spec: Old palette chunk (0x0004 o 0x0011). `packets`: (saltar, cantidad escrita, colores);
/// la cantidad 0 significa 256.
pub fn palette_old(kind: u16, packets: &[(u8, u8, Vec<[u8; 3]>)]) -> Vec<u8> {
    let mut d = Vec::new();
    w16(&mut d, packets.len() as u16);
    for (skip, amount, colors) in packets {
        d.push(*skip);
        d.push(*amount);
        for c in colors {
            d.extend(c);
        }
    }
    chunk(kind, &d)
}

/// Spec: Color Profile Chunk (0x2007). `kind` 0 = ninguno, 1 = sRGB, 2 = ICC.
pub fn profile(kind: u16, flags: u16, gamma: u32, icc: &[u8]) -> Vec<u8> {
    let mut d = Vec::new();
    w16(&mut d, kind);
    w16(&mut d, flags);
    w32(&mut d, gamma);
    d.extend([0u8; 8]);
    if kind == 2 {
        w32(&mut d, icc.len() as u32);
        d.extend(icc);
    }
    chunk(CHUNK_PROFILE, &d)
}

pub fn profile_srgb() -> Vec<u8> {
    profile(1, 0, 0, &[])
}

/// Spec: Tags Chunk (0x2018), con una etiqueta.
pub fn labels() -> Vec<u8> {
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
    string(&mut d, b"anda");
    chunk(CHUNK_LABELS, &d)
}

/// Spec: User Data Chunk (0x2020), solo con texto.
pub fn data_user() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 1);
    string(&mut d, b"nota");
    chunk(CHUNK_DATA_USER, &d)
}

/// Spec: Slice Chunk (0x2022), con una clave.
pub fn cuts() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 1);
    w32(&mut d, 0);
    w32(&mut d, 0);
    string(&mut d, b"corte");
    w32(&mut d, 0); // fotograma
    w32(&mut d, 0); // x
    w32(&mut d, 0); // y
    w32(&mut d, 2); // ancho
    w32(&mut d, 2); // alto
    chunk(CHUNK_CUTS, &d)
}

/// Spec: External Files Chunk (0x2008), con una entrada.
pub fn files_external() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 1);
    d.extend([0u8; 8]);
    w32(&mut d, 1); // id de la entrada
    d.push(0); // tipo: paleta externa
    d.extend([0u8; 7]);
    string(&mut d, b"paleta.gpl");
    chunk(CHUNK_FILES, &d)
}

/// Spec: Cel Extra Chunk (0x2006).
pub fn cel_extra() -> Vec<u8> {
    let mut d = Vec::new();
    w32(&mut d, 0);
    d.extend([0u8; 16]); // cuatro FIXED
    d.extend([0u8; 16]);
    chunk(TEST_CHUNK_CEL_EXTRA, &d)
}

/// Spec: Mask Chunk (0x2016), de 8 × 2 píxeles.
pub fn mascara() -> Vec<u8> {
    let mut d = Vec::new();
    wi16(&mut d, 0);
    wi16(&mut d, 0);
    w16(&mut d, 8);
    w16(&mut d, 2);
    d.extend([0u8; 8]);
    string(&mut d, b"mascara");
    d.extend([0xFF, 0x0F]); // alto * ((ancho + 7) / 8) bytes
    chunk(CHUNK_MASCARA, &d)
}

/// Spec: Path Chunk (0x2017): «Never used», sin estructura.
pub fn stroke() -> Vec<u8> {
    chunk(CHUNK_STROKE, &[1, 2, 3])
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
    string(&mut d, b"tiles");
    chunk(CHUNK_TILESET, &d)
}

/// Un chunk de un tipo que la especificación no lista.
pub fn unknown(kind: u16) -> Vec<u8> {
    chunk(kind, &[9, 8, 7])
}

// ---------------------------------------------------------------------------------------
// El fichero entero.
// ---------------------------------------------------------------------------------------

/// Un fichero con cabecera y UN fotograma (Spec: Header y Frames). `frames` es lo que
/// declara la cabecera; `rest_2` son los bytes que siguen al primer fotograma.
#[derive(Clone)]
pub struct File {
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub flags: u32,
    pub transparent: u8,
    pub colors: u16,
    pub frames: u16,
    pub chunks: Vec<Vec<u8>>,
    pub rest_2: Vec<u8>,
}

impl File {
    pub fn rgba(width: u16, height: u16, chunks: Vec<Vec<u8>>) -> File {
        File {
            width,
            height,
            depth: TEST_DEPTH_RGBA,
            flags: HEADER_OPACITY_VALID,
            transparent: 0,
            colors: 0,
            frames: 1,
            chunks,
            rest_2: Vec::new(),
        }
    }

    pub fn indexed(
        width: u16,
        height: u16,
        transparent: u8,
        colors: u16,
        chunks: Vec<Vec<u8>>,
    ) -> File {
        File {
            depth: TEST_DEPTH_INDEXED,
            transparent,
            colors,
            ..File::rgba(width, height, chunks)
        }
    }

    /// Los bytes del fichero, con los tamaños y contadores coherentes.
    pub fn bytes(&self) -> Vec<u8> {
        let body: Vec<u8> = self.chunks.iter().flatten().copied().collect();
        let tam_frame = TAM_HEADER_FRAME + body.len();
        let total = TAM_HEADER + tam_frame + self.rest_2.len();
        let mut v = Vec::with_capacity(total);
        // Spec: Header.
        w32(&mut v, total as u32);
        w16(&mut v, TEST_MAGIC_FILE);
        w16(&mut v, self.frames);
        w16(&mut v, self.width);
        w16(&mut v, self.height);
        w16(&mut v, self.depth);
        w32(&mut v, self.flags);
        w16(&mut v, 100); // velocidad
        w32(&mut v, 0);
        w32(&mut v, 0);
        v.push(self.transparent);
        v.extend([0u8; 3]);
        w16(&mut v, self.colors);
        v.push(1); // ancho de píxel
        v.push(1); // alto de píxel
        wi16(&mut v, 0); // cuadrícula x
        wi16(&mut v, 0); // cuadrícula y
        w16(&mut v, 16); // ancho de la cuadrícula
        w16(&mut v, 16); // alto de la cuadrícula
        v.extend([0u8; 84]);
        assert_eq!(v.len(), TAM_HEADER);
        // Spec: Frames.
        w32(&mut v, tam_frame as u32);
        w16(&mut v, TEST_MAGIC_FRAME);
        w16(&mut v, self.chunks.len().min(0xFFFF) as u16);
        w16(&mut v, 100); // duración
        v.extend([0u8; 2]);
        w32(&mut v, self.chunks.len() as u32);
        v.extend(body);
        v.extend(&self.rest_2);
        v
    }
}

/// Píxeles RGBA fila a fila: `f(x, y)` da `[r, g, b, a]`.
pub fn pixels_rgba(width: usize, height: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> Vec<u8> {
    let mut v = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            v.extend(f(x, y));
        }
    }
    v
}

// ---------------------------------------------------------------------------------------
// Lector mínimo (solo para comprobar lo que escribe `write_aseprite`).
// ---------------------------------------------------------------------------------------

/// Cursor sobre bytes; en la prueba un fin de datos inesperado es un fallo de la prueba.
pub struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new_2(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let piece = self
            .data
            .get(self.pos..self.pos + n)
            .expect("fin de datos inesperado en el lector de la prueba");
        self.pos += n;
        piece
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

    pub fn rest_2(&self) -> usize {
        self.data.len() - self.pos
    }
}

/// Spec: Header, campo a campo.
#[derive(Debug)]
pub struct HeaderRead {
    pub tam_file: u32,
    pub magic: u16,
    pub frames: u16,
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub flags: u32,
    pub speed: u16,
    pub dwords_zero: [u32; 2],
    pub transparent: u8,
    pub ignored: [u8; 3],
    pub colors: u16,
    pub pixel: (u8, u8),
    pub grid: (i16, i16, u16, u16),
    pub future_to_zero: bool,
}

/// Spec: Frames, cabecera del primer fotograma.
#[derive(Debug)]
pub struct FrameRead {
    pub tam: u32,
    pub magic: u16,
    pub chunks_old: u16,
    pub duration: u16,
    pub future: [u8; 2],
    pub chunks_new: u32,
}

pub struct FileRead {
    pub header: HeaderRead,
    pub frame_2: FrameRead,
    /// (tipo, datos) de cada chunk del primer fotograma, en orden.
    pub chunks: Vec<(u16, Vec<u8>)>,
}

/// Lee un fichero de un fotograma: cabecera, cabecera del fotograma y chunks.
pub fn parsea(bytes: &[u8]) -> FileRead {
    let mut l = Reader::new_2(bytes);
    let tam_file = l.u32();
    let magic = l.u16();
    let frames = l.u16();
    let width = l.u16();
    let height = l.u16();
    let depth = l.u16();
    let flags = l.u32();
    let speed = l.u16();
    let dwords_zero = [l.u32(), l.u32()];
    let transparent = l.u8();
    let ignored = [l.u8(), l.u8(), l.u8()];
    let colors = l.u16();
    let pixel = (l.u8(), l.u8());
    let grid = (l.i16(), l.i16(), l.u16(), l.u16());
    let future_to_zero = l.bytes(84).iter().all(|&b| b == 0);
    assert_eq!(l.pos, TAM_HEADER, "la cabecera mide 128 bytes");
    let header = HeaderRead {
        tam_file,
        magic,
        frames,
        width,
        height,
        depth,
        flags,
        speed,
        dwords_zero,
        transparent,
        ignored,
        colors,
        pixel,
        grid,
        future_to_zero,
    };
    let frame_2 = FrameRead {
        tam: l.u32(),
        magic: l.u16(),
        chunks_old: l.u16(),
        duration: l.u16(),
        future: [l.u8(), l.u8()],
        chunks_new: l.u32(),
    };
    let mut chunks = Vec::new();
    for _ in 0..frame_2.chunks_new {
        let tam = l.u32() as usize;
        let kind = l.u16();
        chunks.push((kind, l.bytes(tam - 6).to_vec()));
    }
    assert_eq!(l.rest_2(), 0, "no sobran bytes tras el primer fotograma");
    FileRead {
        header,
        frame_2,
        chunks,
    }
}

/// Spec: Layer Chunk, campo a campo.
#[derive(Debug)]
pub struct LayerRead {
    pub flags: u16,
    pub kind: u16,
    pub level: u16,
    pub width: u16,
    pub height: u16,
    pub blend: u16,
    pub opacity: u8,
    pub future: [u8; 3],
    pub name_2: String,
}

pub fn decodes_layer(data: &[u8]) -> LayerRead {
    let mut l = Reader::new_2(data);
    let flags = l.u16();
    let kind = l.u16();
    let level = l.u16();
    let width = l.u16();
    let height = l.u16();
    let blend = l.u16();
    let opacity = l.u8();
    let future = [l.u8(), l.u8(), l.u8()];
    let n = usize::from(l.u16());
    let name_2 = String::from_utf8(l.bytes(n).to_vec()).expect("nombre UTF-8");
    assert_eq!(l.rest_2(), 0, "la capa no lleva más datos");
    LayerRead {
        flags,
        kind,
        level,
        width,
        height,
        blend,
        opacity,
        future,
        name_2,
    }
}

/// Spec: Cel Chunk de tipo 2, con los píxeles ya descomprimidos.
#[derive(Debug)]
pub struct CelRead {
    pub layer_2: u16,
    pub x: i16,
    pub y: i16,
    pub opacity: u8,
    pub kind: u16,
    pub z: i16,
    pub future: [u8; 5],
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,
}

pub fn decodes_cel(data: &[u8]) -> CelRead {
    let mut l = Reader::new_2(data);
    let layer_2 = l.u16();
    let x = l.i16();
    let y = l.i16();
    let opacity = l.u8();
    let kind = l.u16();
    let z = l.i16();
    let future = [l.u8(), l.u8(), l.u8(), l.u8(), l.u8()];
    let width = l.u16();
    let height = l.u16();
    let rest_2 = l.bytes(l.rest_2());
    let pixels = if kind == TEST_CEL_COMPRESSED {
        inflate(rest_2)
    } else {
        rest_2.to_vec()
    };
    CelRead {
        layer_2,
        x,
        y,
        opacity,
        kind,
        z,
        future,
        width,
        height,
        pixels,
    }
}
