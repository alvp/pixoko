# Hito 1 · Unidad 5: PNG en `px-formats`

Estado: **aprobada** (2026-10-17)

## Objetivo

Leer y escribir PNG en `px-formats`, sobre bytes en memoria, con ida y vuelta sin pérdida de **valores de píxel** (no de bytes del fichero) en los cuatro modos de `Image`. Lo que el modo de `Image` no puede representar (16 bits, gris de menos de 8 bits, RGB sin alfa, color clave) se convierte de forma documentada y el lector lo declara en un informe. El acceso a ficheros lo harán la CLI y la app.

Especificación de referencia: *Portable Network Graphics (PNG) Specification, Third Edition* (W3C). Los valores esperados de los tests salen de ella, no de ningún programa de referencia.

## Entradas y salidas

En `px_formats` (la crate pasa a depender de `px-core`; nada del crate `png` aparece en la API pública):

- `read_png(bytes: &[u8]) -> Result<PngRead, PngError>`
- `write_png(image: &Image) -> Result<Vec<u8>, PngError>`
- `PngRead { pub image: Image, pub report: Vec<Conversion> }`: `Clone`, `Debug`, `PartialEq`, `Eq`. `is_exact(&self) -> bool` es `report.is_empty()`.
- `Conversion`: `#[non_exhaustive]`; `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`. Cada variante es un hecho sobre el fichero:
  - `DepthExpanded { from_bits: u8 }`: gris de 1, 2 o 4 bits llevado a 8.
  - `Depth16Reduced`: muestras de 16 bits reducidas a 8.
  - `RgbToRgba`: el fichero era RGB (tipo 2); la imagen es `Rgba`.
  - `ColorKeyToAlpha`: había tRNS de gris o RGB (color clave) y se convirtió en alfa por píxel.

  Una variante aparece como mucho una vez, en este orden: `DepthExpanded`, `Depth16Reduced`, `RgbToRgba`, `ColorKeyToAlpha`.
- `PngError`: `#[non_exhaustive]`; `Display`, `Error`, `Debug`, `Clone`, `PartialEq`, `Eq`.
  - `Decode(String)`: el crate `png` rechaza el fichero (firma, CRC, datos truncados, chunk inválido, combinación de color y profundidad inválida, dimensión cero, chunk crítico desconocido, límites del crate…). El texto es el del crate y no es estable.
  - `TooLarge { width: u32, height: u32 }`: ver comportamiento 7.
  - `InvalidPalette`: indexado sin PLTE o con más de `2^profundidad` entradas (un PLTE vacío o de longitud no múltiplo de 3 ya lo rechaza el crate: `Decode`).
  - `PaletteIndexOutOfRange { index: u8, len: usize }`: un píxel apunta fuera de PLTE.
  - `InvalidTrns`: tRNS con longitud incorrecta, clave fuera del rango de la profundidad, o más entradas que PLTE.
  - `TrailingData`: hay bytes después del chunk IEND.
  - `EmptyPalette`: (solo escritura) paleta sin entradas; defensivo, ver comportamiento 12.
  - `Encode(String)`: el crate `png` falló al escribir.

## Comportamiento

### Lectura

1. **Sin las conversiones del crate.** El crate se usa en modo crudo (`Transformations::IDENTITY`): entrega las muestras tal cual están en el fichero (sub-8 bits empaquetados, 16 bits en big-endian, sin expandir tRNS ni paleta) y desentrelaza Adam7. Las conversiones de la tabla son nuestras, para que sean las documentadas. Se ignoran los chunks de texto e iCCP del crate (no se reserva memoria para ellos) y se activa expresamente la verificación de CRC y de Adler-32 (`ignore_checksums(false)`): por defecto el crate **no** comprueba el Adler-32.
2. **Correspondencia.** Para cada tipo de color y profundidad del fichero:

   | Fichero | tRNS | `Image` | Informe |
   | --- | --- | --- | --- |
   | Indexado 1, 2, 4 u 8 bits | alfa por entrada | `Indexed`, índices intactos; paleta = PLTE con el alfa de tRNS (255 en las entradas que tRNS no cubre); `transparent_index = None` | (vacío) |
   | Gris 8 | no | `Gray` | (vacío) |
   | Gris 1, 2 o 4 | no | `Gray` escalado (comportamiento 3) | `DepthExpanded` |
   | Gris 16 | no | `Gray` reducido (comportamiento 3) | `Depth16Reduced` |
   | Gris, cualquier profundidad | clave | `GrayAlpha`: alfa 0 si la muestra es la clave, 255 si no | las de profundidad que correspondan, y `ColorKeyToAlpha` |
   | Gris con alfa 8 | (ignorada) | `GrayAlpha` | (vacío) |
   | Gris con alfa 16 | (ignorada) | `GrayAlpha` reducido | `Depth16Reduced` |
   | RGB 8 | no | `Rgba` con `a = 255` | `RgbToRgba` |
   | RGB 16 | no | `Rgba` reducido, `a = 255` | `Depth16Reduced`, `RgbToRgba` |
   | RGB, 8 o 16 | clave | `Rgba`: `a = 0` si (r, g, b) es la clave, 255 si no; el RGB se conserva | las anteriores y `ColorKeyToAlpha` |
   | RGBA 8 | (ignorada) | `Rgba` | (vacío) |
   | RGBA 16 | (ignorada) | `Rgba` reducido | `Depth16Reduced` |

   El modo de salida lo decide el fichero, no los datos: un gris con tRNS da `GrayAlpha` aunque ningún píxel coincida con la clave. El entrelazado Adam7 no es una conversión (no cambia ningún valor) y no figura en el informe.
3. **Escalado.** Gris de `n` bits (1, 2, 4): `v * 255 / (2^n − 1)` (0 y 255; 0, 85, 170, 255; `v * 17`). De 16 a 8 bits, en cada muestra (gris, RGB y alfa): `(v + 128) / 257` en enteros, que es el redondeo al más cercano de `v * 255 / 65535`: `0 → 0`, `65535 → 255` y `b * 257 → b`. Una muestra de 16 bits `0x01FF` da 2 (el byte alto daría 1).
4. **Color clave.** Se compara con la muestra **original** (a la profundidad del fichero, antes de escalar), con las tres muestras para RGB: en gris de 16 bits, `0x0100` y `0x0101` se reducen a 1 pero solo la primera es la clave. tRNS de gris debe medir 2 bytes y el de RGB 6; cada valor debe caber en la profundidad (`< 2^profundidad`). Si no, `InvalidTrns`. En los tipos 4 y 6 PNG prohíbe tRNS: se ignora. El tRNS se lee del fichero crudo (el mismo recorrido de chunks del comportamiento 6, que toma el primero previo al IDAT) porque el crate descarta en silencio el demasiado corto y, por debajo de 16 bits, recorta el de gris y RGB a un byte por muestra: desde `info()` no se pueden ver esos errores.
5. **Indexado estricto.** PLTE debe existir y tener como mucho `2^profundidad` entradas (que no sea vacío lo exige ya el crate), y todo píxel debe apuntar a una entrada existente (`Image` lo exige). tRNS no puede tener más entradas que PLTE; puede tener menos. El crate no comprueba nada de esto.
6. **Basura final.** Tras la imagen se recorre la estructura de chunks (longitud, tipo, datos, CRC; aritmética comprobada) hasta IEND: el IEND debe estar completo y terminar exactamente en el último byte. Si sobran bytes (basura o más chunks), `TrailingData`; si falta parte del IEND, `Decode`. El crate no detecta ninguno de los dos casos.
7. **Tamaño y memoria.** Tras leer la cabecera y los chunks previos a IDAT, y antes de reservar nada, se calcula `Image::byte_len(width, height, modo_de_salida)` y se compara con el búfer de muestras del fichero (`output_buffer_size`, que con 16 bits es el doble por canal): si cualquiera de los dos desborda o supera `Image::MAX_BYTES`, `TooLarge { width, height }`. Las reservas usan `try_reserve_exact` (un fallo es `TooLarge`, nunca aborta). El crate se configura con `Limits { bytes: 64 MiB }` explícito: protege su propia memoria (una fila gigante o un chunk que declara tamaños enormes da `Decode`). El pico de memoria es como mucho `2 × MAX_BYTES` más lo del crate.
8. **Nunca pánico.** Ninguna entrada hace pánico: truncada, CRC incorrecto, chunk inválido, dimensión cero, basura final o bytes arbitrarios dan `Err`. Un error no deja nada reservado.
9. **Qué se ignora.** Ver «Limitaciones». Los chunks auxiliares que no se leen no influyen en el resultado ni en el informe. Un chunk crítico desconocido es `Decode`. APNG: solo se lee la imagen por defecto (el IDAT); `acTL`, `fcTL` y `fdAT` se ignoran.

### Escritura

10. **Tal cual.** `Indexed`, `Gray`, `GrayAlpha` y `Rgba` salen con el tipo de color PNG correspondiente (3, 0, 4 y 6), siempre a 8 bits, sin entrelazado, sin tocar ningún valor (un `Rgba` con `a = 0` conserva su RGB). Solo se escriben IHDR, IDAT e IEND, más PLTE y tRNS en el indexado. Compresión y filtros, los del crate por defecto.
11. **Indexado.** PLTE con el RGB de cada entrada. tRNS con el alfa de `palette.resolved(i)` (el índice transparente sale con alfa 0): se omite si todos son 255 y, si no, llega hasta la última entrada con alfa distinto de 255 (la especificación permite una tRNS más corta). Consecuencia: una paleta con `transparent_index = Some(t)` se lee de vuelta con esa entrada ya resuelta (`a = 0`) y `transparent_index = None`. Es la única diferencia posible en la ida y vuelta, y es coherente con la nota de la spec de `Palette`.
12. **Paleta vacía.** `write_png` la rechaza con `EmptyPalette` (el crate escribiría un PLTE vacío o ninguno sin quejarse). Hoy es inalcanzable: `Image::new_indexed` ya exige paleta no vacía y la imagen no expone `&mut Palette`. Es una comprobación defensiva sin test posible desde la API pública.
13. **Determinista.** La misma `Image` da los mismos bytes en la misma versión del crate (no se promete entre versiones).

## Limitaciones

Se ignoran al leer y no se escriben, así que **se pierden al abrir y guardar**. El informe de conversiones no las lista. Quedan para un hito posterior, cuando el documento tenga dónde guardar metadatos:

- **gAMA, cHRM, sRGB, iCCP, sBIT:** gestión de color. Aplicarlos cambiaría los valores de píxel, y el pixel art trabaja con los valores del fichero.
- **pHYs, tEXt, zTXt, iTXt, tIME, eXIf, bKGD, hIST, sPLT** y chunks auxiliares desconocidos: sin dónde guardarlos en `Document`.
- **PLTE en RGB y RGBA** (paleta sugerida): la imagen no es indexada.
- **APNG:** un APNG se lee como su imagen estática (la imagen por defecto, el IDAT, comprobado con el crate) y el resto (`acTL`, `fcTL`, `fdAT`) se ignora sin aviso. La animación llegará con las capas y el tiempo.
- **Datos tras IEND:** rechazarlos (`TrailingData`) es **deliberado, por seguridad** (un PNG con un segundo fichero o una carga útil pegada al final no se abre en silencio). Se reconsiderará si los ficheros reales del hito 6 lo exigen.
- **Escritura:** indexado siempre a 8 bits (los ficheros de 1, 2 y 4 bits crecen) y sin optimizar la compresión.
- **Datos de sobra dentro del IDAT** (después de la última fila): el crate los acepta y no se detectan.
- **16 bits:** la lectura pierde precisión por diseño, y un PNG de 16 bits se rechaza por tamaño antes que su equivalente de 8 bits (comportamiento 7).

## Dependencias

Comprobado con `cargo deny check` (0.20.2, el `deny.toml` del repo) sobre una copia del workspace con las dependencias añadidas: `advisories ok, bans ok, licenses ok, sources ok`. Ninguna crate cae fuera de la lista de licencias; solo hay un aviso (no falla) por `miniz_oxide` duplicado.

En `crates/px-formats/Cargo.toml`:

| Entrada | Versión | Licencia | Notas |
| --- | --- | --- | --- |
| `px-core` (ruta) | — | MIT | Arista entre crates; no cuenta para `DEPENDENCIES.md` |
| `png` | 0.18.1 | MIT OR Apache-2.0 | `rust-version` 1.73 |
| `proptest` (dev) | 1.12.0 | MIT OR Apache-2.0 | Ya está en el lock por `px-core` |

**Features de `png`: ninguna.** El crate no tiene features por defecto; no se activan `zlib-rs` (inflado más rápido, añade `zlib-rs`), `unstable` ni `benchmarks`.

Transitivas que se compilan (`cargo tree -p px-formats -e normal`), todas con una alternativa MIT o Apache-2.0:

| Crate | Versión | Licencia |
| --- | --- | --- |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 |
| `crc32fast` | 1.5.2 | MIT OR Apache-2.0 |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 |
| `miniz_oxide` | 0.8.9 y 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| `simd-adler32` | 0.3.10 | MIT |

`zlib-rs` 0.6.8 (Zlib, en la lista) aparece solo en `Cargo.lock`, por la feature opcional de `flate2`; no se compila. Al implementar: anotar `png` y sus transitivas en `docs/DEPENDENCIES.md` (el test de estructura de `px-golden` lo exige), commitear `Cargo.lock` (el CI usa `--locked`) y pasar `license-auditor`.

## Fuera de alcance

- Rutas y ficheros (CLI y app); lectura incremental o en flujo.
- Escribir 16 bits, gris de menos de 8 bits, RGB sin alfa o entrelazado.
- Metadatos, gestión de color y APNG (ver «Limitaciones»).
- Acceso a bytes o filas de `Image`: esta unidad usa `get_pixel` y `set_pixel` (ver decisión 2).

## Casos límite

Notación: `png(tipo, prof, filas…)` es un fichero construido byte a byte en el test (con los chunks que se citen); `P[...]` es una `Palette`. Cada fila agrupa casos parecidos y es una o varias aserciones.

| Entrada | Resultado |
| --- | --- |
| `png(indexado, 8, PLTE de 3, filas [0,1,2])`; igual a 2 y 4 bits, y a 1 bit con PLTE de 2 e índices `[0,1,1]` | `Indexed` con esos índices y paleta de 3 con `a = 255`, `transparent_index() == None`; informe vacío |
| indexado, PLTE de 3, tRNS `[0, 128]` | alfa de las entradas `[0, 128, 255]`; `transparent_index() == None`; informe vacío |
| indexado con píxel 5 y PLTE de 2; sin PLTE; PLTE de 3 entradas a 1 bit; PLTE vacío; tRNS de 3 con PLTE de 2 | `Err(PaletteIndexOutOfRange { index: 5, len: 2 })`; `Err(InvalidPalette)` (los dos siguientes); `Err(Decode)` (lo rechaza el crate); `Err(InvalidTrns)` |
| gris 8, gris con alfa 8 y RGBA 8, con un píxel `a = 0` y RGB distinto de 0 | `Gray`, `GrayAlpha` y `Rgba` con los mismos valores (RGB intacto con `a = 0`); informe vacío |
| gris 1, 2 y 4 bits con todos los valores; gris 2 bits `[0,1,2,3]` | `Gray` escalado (`[0,255]`; `[0,85,170,255]`; `v * 17`); informe `[DepthExpanded { from_bits }]` |
| gris 16 `[0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF]`; RGBA 16 y gris con alfa 16 con las mismas muestras | `Gray([0, 0, 1, 2, 255])`; cada canal con la misma regla; informe `[Depth16Reduced]` |
| RGB 8 `(1,2,3)`; RGB 16 `(0x0102, 0x0304, 0xFFFF)` | `Rgba(1,2,3,255)` con `[RgbToRgba]`; `Rgba(1,3,255,255)` con `[Depth16Reduced, RgbToRgba]` |
| gris 8 con tRNS 7 y píxeles `[7, 8]`; gris 2 bits con tRNS 1 y `[1, 2]`; gris 16 con tRNS `0x0100` y `[0x0100, 0x0101]` | `[GrayAlpha{7,0}, GrayAlpha{8,255}]` con `[ColorKeyToAlpha]`; `[{85,0}, {170,255}]` con `[DepthExpanded{2}, ColorKeyToAlpha]`; `[{1,0}, {1,255}]` con `[Depth16Reduced, ColorKeyToAlpha]` (se compara antes de escalar) |
| RGB 8 con tRNS `(1,2,3)` y píxeles `(1,2,3)`, `(1,2,4)`; gris sin ningún píxel igual a la clave | `[Rgba(1,2,3,0), Rgba(1,2,4,255)]` (RGB conservado) con `[RgbToRgba, ColorKeyToAlpha]`; `GrayAlpha` con todo `alpha = 255` |
| gris con tRNS de 1 byte; RGB con tRNS de 4 bytes; gris 2 bits con clave 9; GrayAlpha y RGBA con tRNS | `Err(InvalidTrns)` (los tres primeros); `Ok`, tRNS ignorado, informe vacío |
| imagen de 8 × 8 (usa las 7 pasadas) entrelazada Adam7 y la misma sin entrelazar, en gris 8, indexado 4 bits y RGBA 8 | la misma `Image` en ambos; informe vacío |
| con `gAMA`, `sRGB`, `iCCP`, `pHYs`, `tEXt`, `tIME` y un auxiliar desconocido; un APNG de 2 cuadros; chunk crítico desconocido | `Ok` con la `Image` y el informe del fichero sin ellos; `Ok` con la imagen del IDAT; `Err(Decode)` |
| vacío; firma incorrecta; truncado a mitad de IDAT; sin IEND; CRC erróneo en IHDR y en IDAT; Adler-32 erróneo con el CRC del chunk recalculado; filtro de fila 9; RGB a 4 bits; ancho 0 | `Err(Decode(_))` en cada uno, sin pánico |
| fichero válido más `b"x"`; más un chunk `tEXt` completo; sin los últimos 1 a 11 bytes (IEND incompleto) | `Err(TrailingData)` (los dos primeros); `Err(Decode)` |
| gris 8 de 32769 × 32768 y de 100000 × 100000 con IDAT mínimo; RGB 16 de 14000 × 14000 (la `Rgba` son 784 000 000 bytes y cabe; el búfer del fichero, 1 176 000 000, no); RGBA 16 de 2 000 000 000 × 1 | `Err(TooLarge { width, height })` (los tres primeros, sin reservar); `Err(Decode)` (límites del crate) |
| `write_png` de `Indexed` con `P[(1,2,3,255), (4,5,6,255)]`; con alfa `[0, 255]`; con `[255, 128]`; con `transparent_index = Some(1)` | IHDR tipo 3, 8 bits, sin entrelazar, y PLTE de 6 bytes; sin tRNS; tRNS `[0]`; tRNS `[255, 128]`; tRNS `[255, 0]` y, al leerlo, la entrada 1 con `a = 0` y `None` |
| `write_png` de `Gray`, `GrayAlpha` y `Rgba` 1 × 1 | tipo 0, 4 y 6, 8 bits, sin PLTE ni tRNS; al leerlo, la misma `Image` con informe vacío |

## Notas para la unidad 6

- El corpus de ida y vuelta incluirá, además de los PNG generados en los tests, unos pocos PNG pequeños guardados con Grafx2 y LibreSprite a partir de imágenes propias del usuario, para probar la interoperabilidad. **Los añade el usuario**; no se generan con esos programas desde el código ni desde Claude. Irán en `golden/` con una nota de qué programa y versión (`pacman -Q`) los guardó.

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (con `png` añadido, ver «Dependencias»)
- Sin `unsafe` en `px-formats` (el lint del workspace lo prohíbe).

## Tests

En `crates/px-formats/tests/png.rs` (integración sobre la API pública). **El corpus se construye en los tests**, sin ficheros `.png` en el repo: un ayudante escribe chunks, CRC-32, Adler-32 y deflate «stored» a mano (sin `flate2` ni `crc32fast`, que no son dependencias directas) y, para Adam7, extrae las pasadas. Los tests no reservan más de unos pocos KiB: el límite se prueba por el lado del rechazo, con ficheros de cabecera enorme y datos mínimos.

**Unitarios**: una prueba por fila de la tabla de casos límite.

**Propiedades** (`proptest`):

1. **Ida y vuelta en los cuatro modos.** Para cualquier `Image` de 1 a 8 de lado en cualquier modo (el indexado con paleta de 1 a 256 entradas, alfas cualesquiera y `transparent_index` o no; `Rgba` con `a = 0` y RGB distinto de 0): `read_png(write_png(img))` es `Ok`, con informe vacío y una imagen igual a `normalizada(img)`, que es `img` salvo en un indexado con `transparent_index = Some(t)`, cuya paleta sale ya resuelta (`resolved(i)`) y sin índice transparente.
2. **Bytes arbitrarios, nunca pánico.** `read_png` con bytes aleatorios (0 a 512), con «casi PNG» (firma e IHDR con CRC válido y ancho, alto, tipo y profundidad `u32` y `u8` cualesquiera, más IDAT mínimo e IEND) y con un PNG válido de la propiedad 1 al que se cambian 1 a 4 bytes: siempre devuelve `Ok` o `Err`, sin pánico. Si es `Ok`, la imagen cumple los invariantes de `Image` (indexado con índices menores que la paleta).
3. **Truncado y basura final.** Para un PNG escrito por `write_png`: cada prefijo estricto (de 0 a `len − 1` bytes) es `Err`, y el fichero con 1 a 16 bytes cualesquiera añadidos es `Err(TrailingData)`.
4. **Indexado intacto.** Para profundidad 1, 2, 4 u 8, paleta de 1 a `2^profundidad` entradas, índices menores que la paleta, tRNS de 0 a `len` bytes, con o sin Adam7: la imagen es `Indexed` con exactamente esos índices, la paleta con el RGB de PLTE y el alfa de tRNS (255 más allá), `transparent_index() == None` e informe vacío.
5. **Conversiones contra un modelo.** Para gris (1, 2, 4, 8 y 16 bits), gris con alfa (8 y 16), RGB y RGBA (8 y 16), con y sin tRNS cuando el tipo lo permite y con o sin Adam7: modo, píxeles e informe coinciden con la tabla del comportamiento 2, con un modelo independiente del código (las fórmulas del comportamiento 3 y la comparación del 4 sobre la muestra original, con claves que a veces coinciden con una muestra existente).
6. **Forma del fichero escrito.** Para cualquier imagen de la propiedad 1: la secuencia de chunks (recorrida con el ayudante del test) es IHDR, [PLTE, [tRNS]], IDAT+, IEND; IHDR dice 8 bits, sin entrelazar y el tipo de color del modo (3, 0, 4, 6); PLTE y tRNS solo en el indexado, PLTE con `3 * len` bytes y tRNS con la longitud del comportamiento 11; y dos llamadas a `write_png` dan los mismos bytes.

No hay dorado: el PNG tiene especificación pública y los valores esperados salen de ella, no de la salida de un programa de referencia.

## Para el hito 2

- **Pico de memoria al escribir: unos 2 GiB.** `write_png` conserva la `Image` (hasta `Image::MAX_BYTES`, 1 GiB), copia todas las muestras a un búfer contiguo (otro tanto, hasta 1 GiB) y codifica sobre un `Vec` de salida que crece con el fichero comprimido (hasta ~1 GiB si los datos no comprimen). Con un lienzo en el límite, el pico supera los 2 GiB. La lectura tiene un pico parecido (comportamiento 7: hasta `2 × MAX_BYTES` más lo del crate).
- **Escritor por flujo para lienzos grandes.** Una función que escriba sobre un `impl Write` y codifique fila a fila (el crate `png` ofrece escritura por flujo), con un búfer de una sola fila en lugar de la copia completa: el pico baja a la `Image` más una fila más el codificador, y se puede escribir directo a fichero sin acumular la salida. Necesita acceso por filas a `Image`, que enlaza con el acceso por bytes de la decisión 2 (spec propia en `px-core`).

## Decisiones

Las marcadas con **?** se aprobaron el 2026-10-17 (decisiones 1 a 6, sin cambios); el resto son consecuencia directa de ellas.

1. **? API sobre bytes y un informe de conversiones.** `PngRead { image, report }`: el corpus distingue «exacto» (`is_exact()`, se compara píxel a píxel) de «convertido» (se compara con la conversión documentada). Errores propios sin tipos del crate (`#[non_exhaustive]`, como `DocumentError`). El informe cuenta solo conversiones de valores, no los chunks ignorados.
2. **? `px-core` no cambia: el PNG recorre la imagen con `get_pixel` y `set_pixel`.** La spec de `Image` dejaba abierto el acceso a bytes «para esta unidad». Opción elegida: no congelar todavía el diseño interno como API pública; el coste es una llamada con `Result` por píxel, aceptable hasta que haya lienzos grandes que medir. Alternativa: `Image::from_bytes` y `as_bytes` (spec propia en `px-core`), más rápida y con el diseño interno ya fijado.

   **Para el hito 2:** el acceso píxel a píxel es lento en lienzos muy grandes; entonces se valorará un acceso por bytes en `px-core`, con su propia spec.
3. **? Escalado de 16 a 8 bits con redondeo, `(v + 128) / 257`.** Alternativa: el byte alto (`v >> 8`), que es lo que hace `STRIP_16` del crate y es más simple. El redondeo es el valor más cercano y devuelve exactamente el original en cualquier PNG de 16 bits obtenido replicando un byte (`b * 257`); el byte alto también, pero se equivoca hasta en 1 en el resto. El gris de menos de 8 bits se escala a `0..=255` (el método que recomienda la especificación).
4. **? tRNS de gris y RGB da `GrayAlpha` y `Rgba`** (el modo lo fija el fichero, no los píxeles) y la clave se compara con la muestra original. Alternativa: devolver un modo con clave de color, que `Image` no tiene.
5. **? Estricto donde el crate no lo es:** basura o chunks tras IEND, índice fuera de PLTE, tRNS más largo que PLTE, PLTE de más de `2^profundidad` entradas, PLTE ausente, y tRNS de gris o RGB mal formado son errores (todos comprobados contra el crate: los acepta). El Adler-32 también se verifica, aunque el crate por defecto no lo haga. Es lo que pides para «basura final» y «chunks inválidos»; el coste es rechazar algún PNG que otros lectores toleran. Lo contrario, tRNS en los tipos 4 y 6, se ignora porque no cambia ningún valor.
6. **? Ida y vuelta e índice transparente.** `write_png` usa `resolved`, así que una `Image` indexada con `transparent_index = Some(t)` vuelve distinta (entrada resuelta, sin índice). La propiedad 1 lo compara con `normalizada(img)` en lugar de exigir igualdad; sin índice transparente la igualdad es exacta.
7. **? Memoria:** el búfer de muestras del fichero también se limita a `MAX_BYTES` (un RGBA de 16 bits de más de ~134 millones de píxeles, o un RGB de 16 bits de más de ~178 millones, se rechaza aunque su `Image` cupiera) y `Limits` del crate a 64 MiB explícitos. Es lo que permite no leer por filas y aceptar Adam7 con `next_frame`.
8. **El crate se usa sin features** y no se activa `zlib-rs`: el inflado es más lento, pero evita una dependencia más y no hay lienzos grandes todavía.
9. **Escritura a 8 bits sin empaquetar el indexado.** Es lo que se pide («tal cual») y lo más simple; ficheros más grandes, no distintos en valores.
10. **`EmptyPalette` en la escritura es inalcanzable** y no se prueba (comportamiento 12); se deja porque el crate no lo comprueba.
11. **El corpus se genera en los tests**, sin PNG de terceros en el repo: no hay problema de licencias y cada caso se lee en el propio código. Un dorado con PNG hechos por LibreSprite o Grafx2 (comprobar que abren lo que escribimos) queda como mejora posterior.
