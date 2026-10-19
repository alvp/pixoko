//! Chunks auxiliares que la lectura de PNG no usa (specs/hito-2-chunks-auxiliares.md).
//!
//! Se clasifican por el tipo y, en los cuatro cuyo valor decide la apariencia, por unos pocos
//! bytes de datos. No se reserva nada por la longitud declarada y no se verifica el CRC.

/// Chunk auxiliar ignorado. El orden de declaración es el de `AuxReport::found`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AuxChunk {
    /// `iCCP`: perfil de color ICC.
    Icc,
    /// `cICP`: primarios y transferencia.
    CodePoints,
    /// `sRGB`.
    Srgb,
    /// `gAMA`.
    Gamma,
    /// `cHRM`.
    Chromaticities,
    /// `pHYs`: resolución.
    Physical,
    /// `tEXt`, `zTXt` o `iTXt`.
    Text,
    /// `tIME`.
    Time,
    /// `eXIf`.
    Exif,
    /// `acTL`, `fcTL` o `fdAT` (APNG).
    Animation,
    /// Cualquier otro auxiliar, o uno de los cuatro con valor y longitud imposible.
    Other,
}

const KINDS: [AuxChunk; 11] = [
    AuxChunk::Icc,
    AuxChunk::CodePoints,
    AuxChunk::Srgb,
    AuxChunk::Gamma,
    AuxChunk::Chromaticities,
    AuxChunk::Physical,
    AuxChunk::Text,
    AuxChunk::Time,
    AuxChunk::Exif,
    AuxChunk::Animation,
    AuxChunk::Other,
];

/// Lo que el lector vio y no usa.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuxReport {
    /// Los tipos presentes, sin repetidos y en el orden de declaración de `AuxChunk`.
    pub found: Vec<AuxChunk>,
    /// Por qué guardar sobre el original perdería algo que se ve distinto: `Animation`, `Icc`,
    /// `CodePoints`, `Gamma` o `Chromaticities`, siempre presente en `found`.
    pub appearance: Option<AuxChunk>,
}

/// Gamma de sRGB (1/2,2) por 100 000 y tolerancia: 45455 ± 1000 es una gamma de 2,15 a 2,25.
const SRGB_GAMMA: u32 = 45455;
/// Blanco D65 y primarios de sRGB (x, y de blanco, rojo, verde y azul) por 100 000.
const SRGB_CHRM: [u32; 8] = [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000];
const TOLERANCE: u32 = 1000;

/// Acumula los chunks de un fichero, uno a uno.
#[derive(Default)]
pub(crate) struct AuxScan {
    seen: u16,
    animation: bool,
    icc: bool,
    srgb: bool,
    /// Primarios y transferencia del primer `cICP` de longitud válida.
    code_points: Option<[u8; 2]>,
    gamma: Option<u32>,
    chromaticities: Option<[u32; 8]>,
}

impl AuxScan {
    fn mark(&mut self, kind: AuxChunk) {
        self.seen |= 1 << (kind as u16);
    }

    /// `kind` es el tipo de 4 bytes del chunk y `data` sus datos (sin longitud ni CRC).
    pub(crate) fn chunk(&mut self, kind: &[u8], data: &[u8]) {
        match kind {
            // Estructurales o usados: no son auxiliares ignorados.
            b"IHDR" | b"PLTE" | b"IDAT" | b"IEND" | b"tRNS" => {}
            b"iCCP" => {
                self.icc = true;
                self.mark(AuxChunk::Icc);
            }
            b"cICP" if data.len() == 4 => {
                self.code_points.get_or_insert([data[0], data[1]]);
                self.mark(AuxChunk::CodePoints);
            }
            b"sRGB" if data.len() == 1 => {
                self.srgb = true;
                self.mark(AuxChunk::Srgb);
            }
            b"gAMA" if data.len() == 4 => {
                self.gamma
                    .get_or_insert(u32::from_be_bytes([data[0], data[1], data[2], data[3]]));
                self.mark(AuxChunk::Gamma);
            }
            b"cHRM" if data.len() == 32 => {
                let mut values = [0u32; 8];
                for (value, bytes) in values.iter_mut().zip(data.as_chunks::<4>().0) {
                    *value = u32::from_be_bytes(*bytes);
                }
                self.chromaticities.get_or_insert(values);
                self.mark(AuxChunk::Chromaticities);
            }
            // Longitud imposible en uno de los cuatro cuyo valor se lee.
            b"cICP" | b"sRGB" | b"gAMA" | b"cHRM" => self.mark(AuxChunk::Other),
            b"pHYs" => self.mark(AuxChunk::Physical),
            b"tEXt" | b"zTXt" | b"iTXt" => self.mark(AuxChunk::Text),
            b"tIME" => self.mark(AuxChunk::Time),
            b"eXIf" => self.mark(AuxChunk::Exif),
            b"acTL" => {
                self.animation = true;
                self.mark(AuxChunk::Animation);
            }
            b"fcTL" | b"fdAT" => self.mark(AuxChunk::Animation),
            // Auxiliar (minúscula inicial) desconocido. Un crítico desconocido ya lo rechazó el crate.
            other if other.first().is_some_and(|b| b & 0x20 != 0) => self.mark(AuxChunk::Other),
            _ => {}
        }
    }

    pub(crate) fn finish(self) -> AuxReport {
        let found = KINDS
            .iter()
            .copied()
            .filter(|&kind| self.seen & (1 << (kind as u16)) != 0)
            .collect();
        let appearance = self.appearance();
        AuxReport { found, appearance }
    }

    /// Spec D2: `acTL`; `iCCP`; `cICP`; `sRGB`; `gAMA`; `cHRM`.
    fn appearance(&self) -> Option<AuxChunk> {
        if self.animation {
            return Some(AuxChunk::Animation);
        }
        if self.icc {
            return Some(AuxChunk::Icc);
        }
        if let Some([primaries, transfer]) = self.code_points {
            return (primaries != 1 || transfer != 13).then_some(AuxChunk::CodePoints);
        }
        if self.srgb {
            return None;
        }
        if self
            .gamma
            .is_some_and(|g| g.abs_diff(SRGB_GAMMA) > TOLERANCE)
        {
            return Some(AuxChunk::Gamma);
        }
        let differs = |values: &[u32; 8]| {
            values
                .iter()
                .zip(SRGB_CHRM)
                .any(|(&v, srgb)| v.abs_diff(srgb) > TOLERANCE)
        };
        self.chromaticities
            .as_ref()
            .is_some_and(differs)
            .then_some(AuxChunk::Chromaticities)
    }
}
