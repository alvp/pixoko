# Hito 3 · Unidad 3.8: `.aseprite`, etapa 1 (capas, celdas y paleta, un fotograma)

Estado: **aprobada** (2026-10-22), con las condiciones 1 a 10 de «Visto bueno» al final, que **prevalecen sobre cualquier otra frase de la página**, y con un **paso previo**: el caso 29 del corpus, `capa-bloqueada`, que confirma la regla del bloqueo antes de escribir las pruebas. El visto bueno vale como aprobación expresa de `px-core` **solo para `C-01` a `C-04`** y de `px-formats` **solo para `F-01` a `F-10`** (D02): todos aditivos, ninguna firma pública existente cambia. Si durante la unidad hace falta algo fuera de esas listas, me paro y lo digo.

Referencias: fila 3.8 y decisiones D02 y D10 de [`mapa-funciones.md`](mapa-funciones.md); [`corpus-hito-3.md`](corpus-hito-3.md) (los 28 casos, ya generados); [`hito-3-capas-modelo.md`](hito-3-capas-modelo.md) (3.1), [`hito-3-mezcla.md`](hito-3-mezcla.md) (3.2, incluida la nota para 3.8 y «Datos conocidos») y [`hito-3-combinar.md`](hito-3-combinar.md) (3.5); base de límites y copia: [`hito-2-png-flujo.md`](hito-2-png-flujo.md) y [`hito-2-chunks-auxiliares.md`](hito-2-chunks-auxiliares.md). **No toca `px-raster`.** Dependencia directa nueva: `miniz_oxide` (ya está en el árbol; no entra ninguna crate nueva). Sin `unsafe`.

## Fuente de la especificación (regla de procedencia a)

Única fuente del formato: **`ase-file-specs.md`**, la especificación pública del formato, <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md> (descargada de `raw.githubusercontent.com/.../docs/ase-file-specs.md`; el hook la permite por su nombre). Versión usada: la del 22 de octubre de 2026, **SHA-256 `31a6a80526c24209361fc35f9431bd8909603bb106bb6c3ed866f9af28976ae3`** (24 392 bytes). **No entra en el repositorio**: se guarda fuera, en `~/refs/ase-file-specs.md`. `docs/REFERENCIAS.md` anota la URL, la fecha, el hash completo, la frase «se consultó; no se copia» y la tabla campo → apartado en palabras propias (la tabla de esta spec, resumida). De ese documento solo se lee texto: **no se sigue ningún enlace a código** (lo hay, a un fichero de composición de capas, en NOTE.5) y no se lee, abre ni ejecuta nada del programa. En el código y las pruebas cada constante lleva un comentario con **su apartado de la especificación** («§Layer Chunk», «NOTE.2») y **nunca** una descripción de cómo lo hace un programa (regla c).

**Datos del corpus que sí se miraron** (los `.aseprite` de `golden/aseprite/` son datos, no código; una script mía de 40 líneas, escrita solo con las tablas de la especificación, lista sus chunks): los 28 ficheros traen **solo** cuatro tipos de chunk: color profile `0x2007` (sRGB), paleta antigua `0x0004` (**no** el `0x2019`), capa `0x2004` y celda `0x2005`; todas las celdas son de tipo 2 (comprimidas); banderas de cabecera = 1; banderas de capa = 3 (visible y editable) o 2 (oculta y editable); ninguna capa de grupo ni de mapa; 24 de 32 bits y 4 de 8; cuadrícula `(0, 0, 16, 16)`, razón de píxel 1:1, duración 100 ms. Los códigos de mezcla que aparecen son exactamente los de la tabla de la especificación para los ocho modos (0, 1, 2, 3, 4, 5, 10 y 16); **de ahí no se saca ninguna tabla**: el `match` sale del apartado «Layer Chunk» (regla b). Dos cosas **no** las cubrían: una capa **bloqueada** (todas estaban editables) y una capa de fondo (bandera 8). Para la primera, el paso previo añade el caso 29, `capa-bloqueada` (visible y bloqueada, y oculta y bloqueada); cuando el usuario lo genere, sus chunks se listan con la misma script y se **confirma o corrige** la regla `locked = !Editable` (sección «Confirmación del bloqueo», abajo). La capa de fondo sigue sin caso: ver «Riesgos» y la prueba manual.

## Objetivo

Que Pixoko abra y guarde `.aseprite` con **capas, celdas y paleta de un fotograma**, sin pérdida silenciosa: lo que entiende lo lee y lo escribe idéntico (ida y vuelta), lo que no entiende o lo rechaza con un motivo claro o lo avisa y nunca pisa el original (copia, como el PNG con `iCCP`). El lector no se fía del fichero.

## Lo comprobado (en el código)

1. **No hay constructor de un documento con capas.** Hoy se llega con `Document::new` más `AddLayer` y `SetPixels`: dejaría una historia de deshacer llena y, con celdas grandes, pasos que cuestan memoria. Hace falta un constructor sin historia (`C-02`).
2. **`Document::==` compara los `LayerId`** (3.1, «Igualdad»). El fichero no guarda ids (la capa es su posición, NOTE.2): al leer se numeran `0..n`. «Escribir y leer devuelve el mismo documento» se define por tanto **sin ids y por contenido de píxeles** (condición 1): mismas capas en el mismo orden, con nombre, visibilidad, opacidad, mezcla y bloqueo iguales, **el mismo contenido de píxeles en cada capa**, mismo lienzo, misma paleta (colores y alfa) y mismo índice transparente. **Una celda materializada y totalmente transparente equivale a una vacía** (y no se escribe). «Transparente» es alfa 0 en RGBA e índice transparente en indexado; el contenido se compara píxel a píxel con esa equivalencia (dos píxeles transparentes son iguales aunque su RGB difiera, y los demás, byte a byte). La prueba usa una función `mismo_contenido`; no se compara con `Document::==`.
3. **`Session::load` entrega un `Loaded` con una sola `Image`** y `lib.rs` la sube al lienzo con `Canvas::load_with(&loaded.image)`; el lienzo ya sabe leer de un `PixelSource` (`DocSource`), así que basta que `Loaded` lleve el `Document` (ver «Interfaz»).
4. **`Ctrl+S` con ruta guarda sin diálogo; sin ruta abre el de guardar.** Con varias capas y ruta `.png` hoy guarda aplanado con un aviso (`with_flatten_notice`) y nada avisa antes.
5. **Zlib.** `png` 0.18.1 usa `miniz_oxide` 0.8.9 directamente y `flate2` la 0.9.1: **se elige la 0.8.9** (la de `png`; `miniz_oxide = "0.8.9"` no añade entradas a `Cargo.lock`). Tiene `decompress_to_vec_zlib_with_limit` y `compress_to_vec_zlib`. Las dos versiones ya están en el árbol y `cargo deny` pasa con ellas; la comprobación y la nota en `docs/DEPENDENCIES.md` van en el commit de lógica.

## Alcance de la etapa 1 (decisión 1)

| Cosa del fichero | Lee | Escribe | Si no lo entiende |
| --- | --- | --- | --- |
| Cabecera de 128 bytes | sí | sí | magia, tamaño, fotogramas, anchura, altura, profundidad: error |
| Capas normales (tipo 0) | sí | sí | — |
| Capas de grupo (1) y de mapa de tiles (2) | no | no | **error** («capas de grupo: llegan en el hito 6») |
| Capa de referencia (bandera 64) | no | no | **error** |
| Celdas tipo 2 (comprimida) y tipo 0 (sin comprimir) | sí | tipo 2 | — |
| Celda enlazada (1), mapa de tiles (3) | no | no | **error** |
| Paleta nueva `0x2019` y antigua `0x0004` | sí | solo `0x2019` | — |
| Paleta antigua `0x0011` (componentes 0–63) | solo si no hay otra | no | **error** si es la única (la especificación no dice cómo escalar 0–63 a 0–255: no se inventa) |
| Color profile `0x2007` | tipo 0 y 1 sin gamma: nada | escribe tipo 1 (sRGB) | tipo 2 (ICC) o gamma: **ignorado con aviso** (`lossy`) |
| Etiquetas `0x2018`, datos de usuario `0x2020`, cortes `0x2022`, ficheros externos `0x2008`, extra de celda `0x2006`, máscara `0x2016`, trazo `0x2017`, conjunto de tiles `0x2023` y tipos desconocidos | **se saltan por su longitud** | no | **descartados con aviso** (`lossy`); nada se conserva en memoria hasta el hito 5 |
| Más de un fotograma | solo el primero | uno | **aviso** (`lossy`) |

**Descartar con aviso, no conservar** (decisión 1): el modelo no tiene dónde guardar etiquetas ni datos de usuario hasta el hito 5, y conservar los bytes sin interpretarlos obligaría a un campo nuevo en `Document` (`px-core` no aditivo). Para que descartar no pierda datos del usuario, **`Ctrl+S` sobre un fichero abierto con `lossy` guarda una copia** (`nombre-copia.aseprite`), igual que el PNG con `iCCP`: nunca se pisa lo que Pixoko no entendió entero.

**Qué es `lossy`**: algo se descartó que cambia lo que el usuario tiene o ve: cualquier chunk de la lista de arriba, un perfil ICC o gamma, más de un fotograma, y una capa de fondo (bandera 8) **indexada** con píxeles del índice transparente (en el fichero se verían opacos; en Pixoko serían transparentes). **No es `lossy`** (se ignora sin aviso): el chunk de paleta en un fichero RGBA (el modelo RGBA no tiene paleta; se anota en el informe, sin copia), la cuadrícula y la razón de píxel de la cabecera, la duración y la «velocidad», y las banderas de capa 4 (bloqueo de movimiento), 16 y 32.

## El mapa modelo ↔ fichero (decisión 2) y tabla «campo → apartado» (regla a)

### Cabecera (§Header; 128 bytes)

| Campo | Desplazamiento | Lee | Escribe |
| --- | --- | --- | --- |
| `DWORD` tamaño del fichero | 0 | debe ser `bytes.len()` | `len` |
| `WORD` magia `0xA5E0` | 4 | exige | `0xA5E0` |
| `WORD` fotogramas | 6 | ≥ 1; > 1 ⇒ aviso | 1 |
| `WORD` ancho, `WORD` alto | 8, 10 | ≥ 1; el lienzo | del lienzo (≤ 65 535: si no, error) |
| `WORD` profundidad | 12 | 32 ⇒ RGBA, 8 ⇒ indexado, 16 ⇒ error («modo de grises: hito 6») | 32 u 8 |
| `DWORD` banderas | 14 | bit 1 ⇒ la opacidad de capa es válida (si no, 255, NOTE.6); bit 2: se ignora (no hay grupos); bit 4 ⇒ las capas llevan UUID (se salta) | 1 |
| `WORD` velocidad | 18 | ignorada | 100 |
| `BYTE` índice transparente | 28 | indexado: ver paleta | indexado: el de la paleta |
| `WORD` número de colores | 32 | 0 ⇒ 256 (§Header); longitud de la paleta antigua | longitud de la paleta (256 ⇒ 256) |
| `BYTE` ancho y alto de píxel | 34, 35 | ignorados | 1, 1 |
| `SHORT` x, y y `WORD` ancho, alto de la cuadrícula | 36 a 43 | ignorados | 0, 0, 16, 16 |
| `BYTE[84]` | 44 | ignorado | ceros |

### Fotograma y chunks (§Frames)

| Campo | Lee | Escribe |
| --- | --- | --- |
| `DWORD` bytes del fotograma | ≥ 16 y ≤ lo que queda del fichero; con 1 fotograma, igual a lo que queda | el real |
| `WORD` magia `0xF1FA` | exige | `0xF1FA` |
| `WORD` chunks (antiguo) y `DWORD` chunks (nuevo) | cuenta = nuevo si no es 0, y si no el antiguo; ≤ `MAX_CHUNKS` (65 536) | los dos iguales |
| `WORD` duración | ignorada | 100 |
| Chunk: `DWORD` tamaño (≥ 6, incluye los 6 bytes de su cabecera), `WORD` tipo | el tamaño cabe en lo que queda del fotograma; se salta por su tamaño | — |

### Capa (§Layer Chunk 0x2004, NOTE.1, NOTE.2, NOTE.6)

| Campo | Modelo | Lee | Escribe |
| --- | --- | --- | --- |
| Posición del chunk (NOTE.2) | `layers()[i]`; `LayerId` | índice i; ids `0..n` | i |
| `WORD` banderas, bit 1 «Visible» | `visible()` | sí | sí |
| `WORD` banderas, bit 2 «Editable» | `locked() = !editable` | sí | sí |
| `WORD` banderas, bits 4, 16, 32 | — | ignorados | 0 |
| `WORD` banderas, bit 8 «Background» | — | ignorado (aviso `lossy` solo indexado, arriba) | 0 |
| `WORD` banderas, bit 64 «referencia» | — | **error** | 0 |
| `WORD` tipo | — | 0; 1 y 2 **error** | 0 |
| `WORD` nivel hijo (NOTE.1) | — | debe ser 0 (sin grupos no hay hijos); si no, error | 0 |
| `WORD` ancho y alto por defecto | — | ignorados («ignored») | 0 |
| `WORD` modo de mezcla | `blend()` | tabla de abajo | tabla de abajo |
| `BYTE` opacidad | `opacity()` | si la bandera de cabecera 1 | sí |
| `BYTE[3]` | — | ignorados | ceros |
| `STRING` nombre (`WORD` longitud + UTF-8) | `name()` | ≤ 255 bytes (tope del modelo) y UTF-8 válido; si no, error | sí |
| `DWORD` tileset (solo tipo 2), `UUID` (bandera de cabecera 4) | — | el UUID se salta (16 bytes); el tileset no se alcanza (tipo 2 ya dio error) | no |

**Modos de mezcla** (§Layer Chunk, «Blend mode»), de apartado y no de Lua:

| Código | Nombre (especificación) | `BlendMode` |
| --- | --- | --- |
| 0 | Normal | `Normal` |
| 1 | Multiply | `Multiply` |
| 2 | Screen | `Screen` |
| 3 | Overlay | `Overlay` |
| 4 | Darken | `Darken` |
| 5 | Lighten | `Lighten` |
| 10 | Difference | `Difference` |
| 16 | Addition | `Add` |
| 6–9, 11–15, 17, 18 y mayores | Color Dodge, Color Burn, Hard Light, Soft Light, Exclusion, Hue, Saturation, Color, Luminosity, Subtract, Divide | **error** `UnsupportedBlend { layer, code }` («modo de mezcla N: llega en el hito 6») |

Indexado: una capa con opacidad ≠ 255 o mezcla ≠ Normal da **error** (`IndexedLayer`; el modelo la prohíbe, D06). Ningún caso del corpus lo tiene.

### Celda (§Cel Chunk 0x2005, NOTE.2, NOTE.3, NOTE.5)

| Campo | Lee | Escribe |
| --- | --- | --- |
| `WORD` índice de capa | debe ser < capas leídas hasta ahí (las capas van antes, §Layer Chunk); una celda por capa (repetida ⇒ error) | el de la capa |
| `SHORT` x, y | origen de la celda en el lienzo (puede ser negativo o salirse) | la esquina del rectángulo escrito (abajo) |
| `BYTE` opacidad de celda | debe ser 255 (la de celda es del hito 6): si no, error | 255 |
| `WORD` tipo | 0 (sin comprimir) y 2 (comprimida); 1 y 3 error | 2 |
| `SHORT` z-index (NOTE.5) | debe ser 0; si no, error | 0 |
| `BYTE[5]` | ignorados | ceros |
| Tipos 0 y 2: `WORD` ancho, alto | celda de `ancho × alto` píxeles | del rectángulo escrito |
| Tipo 0: `PIXEL[]` | exactamente `ancho·alto·bpp` bytes | no |
| Tipo 2: `PIXEL[]` comprimido con zlib (NOTE.3) | descompresión con tope = `ancho·alto·bpp`; la salida debe ser exactamente esa | zlib de `miniz_oxide`, nivel 6 |

`PIXEL` (§References): RGBA = 4 bytes R, G, B, A; indexado = 1 byte. **Celda → modelo**: se copia a una celda del tamaño del lienzo, rellena con el valor de una celda nueva (RGBA `(0,0,0,0)`; indexado, el índice transparente), colocando la celda en `(x, y)` y **recortando lo que sale**. Si la intersección con el lienzo está vacía (incluida una celda de 0 píxeles) **o todo lo que cae dentro es transparente**, la celda del modelo queda **vacía** (sin memoria; es lo que el corpus dice de las celdas fuera del lienzo y lo que la condición 1 hace equivalente para `celda-vacia.transparente`). Una capa sin chunk de celda queda vacía (`celda-vacia.sin-celda`). El resto se materializa.

**Escritura de la celda (decisión 9)**: se escribe **el rectángulo mínimo** que contiene todos los píxeles no transparentes (el RGB bajo alfa 0 dentro del rectángulo se conserva tal cual). Una celda vacía, o materializada y totalmente transparente, **no escribe chunk**. Esto da ficheros pequeños y la lectura devuelve el mismo contenido.

### Paleta (§Old palette chunk 0x0004, §Palette Chunk 0x2019, §Header «Palette entry» y «Number of colors»)

- **Elección:** si hay algún `0x2019`, solo se usan los `0x2019` (§Old palette chunk: «ignore this chunk if you find the new palette chunk»); si no, los `0x0004`; si no, los `0x0011` (error, tabla de arriba); indexado sin ninguno: error (`NoPalette`).
- **Longitud:** `0x2019`: «New palette size» (1 a 256; si no, error; el último valor leído manda); `0x0004`: el «Number of colors» de la cabecera (0 ⇒ 256).
- **Entradas:** `0x2019`: de «first» a «last» (`first ≤ last < tamaño`), cada una con banderas (bit 1 = lleva nombre, que se salta), R, G, B y **A**; `0x0004`: paquetes de «saltar» (BYTE) y «cantidad» (BYTE, 0 ⇒ 256) colores R, G, B con alfa 255. Lo que ningún paquete cubra queda `(0, 0, 0, 255)` (decisión propia, no está en la especificación); cubrir más allá de la longitud es error.
- **Índice transparente** (cabecera, byte 28): `Some(byte)` si `byte < longitud`; si no, `None`. **Al escribir**, un indexado con `transparent_index() == None` da **error** `NoTransparentIndex` con el mensaje exacto «esta imagen indexada no tiene entrada transparente: fíjala con Fijar transparente o convierte a RGBA»; no se inventa uno. Todo documento con ≥ 2 capas o con alguna celda vacía ya lo tiene (3.1: `NoTransparency`).
- **RGBA:** el chunk de paleta se ignora y no se escribe ninguno.
- Todo índice de píxel debe ser menor que la longitud: si no, error (`IndexNotInPalette`, de `Image`).

## Un solo fotograma (decisión 3)

Se lee **solo el primero**: sus capas y sus celdas. Si el fichero declara más, se devuelve `Ok` con `frames_in_file = n` y `lossy`, y no se mira nada del resto (ni se recorren sus cabeceras: basta con que el primero quepa). Con un solo fotograma declarado, ese fotograma debe terminar exactamente donde acaba el fichero. Motivo: perder fotogramas es perder trabajo, así que se avisa y `Ctrl+S` copia; un error impediría abrir cualquier animación para ver sus capas. `0` fotogramas: error.

## Límites del lector (regla d)

Ninguna longitud declarada se usa para reservar. Todo desplazamiento se calcula con aritmética comprobada sobre trozos del búfer, y todo tope se aplica **antes** de reservar:

| Qué | Tope | Error |
| --- | --- | --- |
| Fichero | el que pasa el llamador (`MAX_FILE_BYTES`, 256 MiB, ya existe) | — |
| Lienzo | `AsepriteLimits::max_pixels` (por defecto 2²⁶, como `MAX_OPEN_PIXELS` del PNG; la app pasa `MAX_OPEN_PIXELS`), mirado en la cabecera y **antes de reservar**, y `Image::MAX_BYTES` | `TooLarge`, y el mensaje dice el límite («el lienzo de {w}×{h} supera el máximo de {max} píxeles») |
| Chunks del primer fotograma | `MAX_CHUNKS` = 65 536 (un chunk mide ≥ 6 bytes, y recorrerlos no reserva) | `TooManyChunks` |
| Capas | `Document::MAX_LAYERS` = 256, contadas **al leer cada chunk** | `TooManyLayers` |
| Nombre | 255 bytes | `LayerName` |
| Celda | `ancho·alto·bpp` ≤ `Image::MAX_BYTES` y **≤ 1032 × (bytes de zlib)** (el máximo de DEFLATE: una celda que declara más de lo que sus datos pueden dar se rechaza antes de descomprimir, como `read_png`, 2.13) | `CelData` |
| Descompresión | `decompress_to_vec_zlib_with_limit(datos, ancho·alto·bpp)`; la salida debe medir **exactamente** eso | `CelData` |
| Celdas materializadas | **un solo tope, el del núcleo**: suma ≤ `AsepriteLimits::max_cells_bytes` (por defecto `Document::DEFAULT_CELLS_LIMIT`, 2 GiB; la app pasa el valor por defecto, no uno propio), comprobada **antes** de reservar cada una, con `try_reserve_exact`; lo que Pixoko puede crear con 2 GiB de celdas lo puede reabrir | `TooLarge` |
| Memoria viva | una celda descomprimida y una celda del lienzo a la vez, además de las ya materializadas | — |

**Riesgo declarado (R1):** el modelo materializa celdas **del tamaño del lienzo** (D04), así que un fichero de pocos KB con un lienzo de 8192² y celdas pequeñas puede pedir hasta `max_cells_bytes` (2 GiB). No hay forma de evitarlo sin perder ficheros legítimos (lienzos grandes con dibujos pequeños) ni sin cambiar D04; lo acotan el lienzo máximo de 2²⁶ píxeles y el tope de celdas **del núcleo** (condición 4: un solo tope, el de 2 GiB). El pico incluye además una celda descomprimida (hasta 256 MiB en 8192²).

**Mensajes de lo no soportado** (condición 6): `Unsupported` nombra qué es y dice que el fichero no se ha modificado, p. ej. «capa de grupo «{nombre}»: Pixoko aún no abre grupos (hito 6); el fichero no se ha modificado» y «modo de mezcla 6 (Color Dodge) en la capa «{nombre}»: aún no se admite; el fichero no se ha modificado». Lo mismo vale para cualquier error de lectura: abrir nunca escribe.

**Truncados, repetidos, CRC.** Truncado en cualquier punto ⇒ `Err` (cada prefijo de un fichero válido, probado). Repetidos: una celda repetida para una capa ⇒ error; paletas repetidas ⇒ las posteriores sobrescriben su rango; perfiles repetidos ⇒ vale el peor (cualquier ICC avisa). «CRC erróneo»: el formato no tiene CRC por chunk; lo más parecido es el **Adler-32 del final de cada flujo zlib**: se exige (si `miniz_oxide` no lo verifica con el tope, el lector lo calcula, unas 15 líneas) y un flujo corrupto es `CelData`.

## API (`px-formats`, aditivo)

| N.º | Qué |
| --- | --- |
| F-01 | `pub fn read_aseprite(bytes: &[u8], limits: &AsepriteLimits) -> Result<AsepriteRead, AsepriteError>` |
| F-02 | `pub fn write_aseprite(doc: &Document) -> Result<Vec<u8>, AsepriteError>` |
| F-03 | `pub fn aseprite_dimensions(bytes: &[u8]) -> Result<(u32, u32), AsepriteError>`: solo la cabecera (como `png_dimensions`, para `check_fits` antes de leer) |
| F-04 | `pub struct AsepriteRead { pub document: Document, pub report: AsepriteReport }` |
| F-05 | `pub struct AsepriteReport { pub frames_in_file: u16, pub ignored: Vec<(Ignored, u32)>, pub lossy: bool }` (`ignored`: tipo y cuántos hubo, ordenado por tipo) |
| F-06 | `#[non_exhaustive] pub enum Ignored { Tags, UserData, Slices, ExternalFiles, CelExtra, Mask, Path, Tileset, ColorProfile, RgbaPalette, BackgroundTransparent, Unknown }` (`Unknown` agrupa los tipos no listados, con su cuenta) |
| F-07 | `pub struct AsepriteLimits { pub max_pixels: u64, pub max_cells_bytes: usize }` con `Default` (2²⁶ y 2 GiB) |
| F-08 | `#[non_exhaustive] pub enum AsepriteError`, `Display` en español y `Error`: `NotAseprite`, `Truncated`, `Malformed(&'static str)` (campo y apartado), `Unsupported(Unsupported)` (grupo, mapa de tiles, referencia, celda enlazada, mapa de tiles de celda, grises, mezcla N, z-index, opacidad de celda, paleta de 6 bits), `IndexedLayer { layer }`, `NoPalette`, `NoTransparentIndex` (mensaje accionable de arriba), `TooLarge`, `TooManyChunks`, `TooManyLayers`, `LayerName`, `CelData { layer }`, `Document(DocumentError)`, `Image(ImageError)` |
| F-09 | `pub enum FileFormat { Png, Aseprite }` y `pub fn detect_format(bytes: &[u8]) -> Option<FileFormat>`: por contenido (firma PNG de 8 bytes; magia `0xA5E0` en el desplazamiento 4), no por extensión |
| F-10 | `Cargo.toml`: `miniz_oxide = "0.8.9"`; `docs/DEPENDENCIES.md` y `fuzz/Cargo.toml` (ya depende de `px-formats`) |

Módulo `aseprite` (ficheros `read.rs`, `write.rs`, `tables.rs` con todas las constantes y su apartado). **Procedencia en el código** (condición b): cada estructura y cada función de lectura o escritura lleva un comentario con el apartado que implementa («Spec: Layer Chunk», «Spec: Cel Chunk»…); ningún comentario describe cómo lo hace ningún programa. **`Document::from_layers` es el único camino por el que el lector construye un documento** (nada de `Document::new` más comandos): valida una sola paleta, el modo único, los límites de capas y celdas y la opacidad 255 en indexado, y devuelve error en vez de un documento inválido (P2 lo fija). `read_png`, `write_png` y el resto no cambian. El lector no usa `unsafe`, ni `unwrap`/`expect`/indexado directo sobre datos del fichero (un lector de trozos `Cursor` con `take(n) -> Result`, como el escáner de chunks del PNG).

## Cambios en `px-core` (aditivos; piden aprobación expresa)

| N.º | Qué |
| --- | --- |
| C-01 | `pub struct LayerSpec { pub name: String, pub visible: bool, pub opacity: u8, pub blend: BlendMode, pub locked: bool, pub image: Option<Image> }` (`Clone`, `Debug`, `PartialEq`) y su export |
| C-02 | `Document::from_layers(width: u32, height: u32, mode: Mode, palette: Option<Palette>, layers: Vec<LayerSpec>) -> Result<Document, DocumentError>`: capas de abajo arriba con `LayerId(i)` e `ImageId(i)`; historia vacía y `state_id` 0; valida con las **reglas de siempre**: `TooManyLayers`, `LayerName`, `IndexedOpacity`, `IndexedBlend`, `CelsTooLarge` (por `cells_limit`), `NoTransparency` (celda vacía sin valor transparente) y, nuevas, `NoLayers` y `CelMismatch` |
| C-03 | `DocumentError::NoLayers` (lista vacía) y `DocumentError::CelMismatch { layer: usize }` (la imagen no tiene el ancho, alto, modo o paleta del lienzo) |
| C-04 | Un mensaje en español por variante nueva en `Display` |

`Document::new` y todo lo demás no cambian. `main_image()` sigue devolviendo `ImageId(0)`: con capas leídas del fichero es la celda de la capa de abajo, como ya documenta su comentario (retirada: 3.9).

## Interfaz (`px-app`; sin tocar `px-ui`) (decisión 4)

- **Abrir.** `Session::load` mira el contenido (`detect_format`), no la extensión. `Loaded` pasa a llevar `document: Document` en lugar de `image` (un PNG se envuelve con `Document::new`); `lib.rs` sube `DocSource` al lienzo. Argumento de línea de comandos, `Ctrl+O` y soltar: lo mismo. El diálogo de abrir: título «Abrir imagen» y filtros «Imágenes (png, aseprite)», «PNG», «Aseprite», «Todos». La capa activa al abrir un `.aseprite` es **la de arriba**.
- **Aviso al abrir** `.aseprite`: `{nombre} · {w}×{h} · {n} capas · {modo}`; con `lossy`: `· se ignoran: etiquetas (2), datos de usuario…; Ctrl+S guardará una copia`; con más de un fotograma: `· solo se abre el fotograma 1 de {n}`. Errores: el texto de `AsepriteError`, con el motivo de lo no soportado.
- **Guardar** (`Session::save_to` y `save`): el formato lo decide la **extensión** del destino: `.aseprite` (sin mirar mayúsculas) escribe `.aseprite`; cualquier otra cosa, PNG como hoy (se añade `.png` si falta). Atómico (`write_atomic`) y con el mismo aviso: `guardado nombre.aseprite · {n} capas`. **Tras guardar como `.aseprite`, el documento pasa a ese fichero (ruta nueva) y deja de estar modificado** (`saved_id`), como con un PNG.
- **`Ctrl+S`** (reglas, en este orden): (1) ruta `.aseprite` ⇒ guarda `.aseprite` sin diálogo (con `lossy`, una copia `nombre-copia.aseprite`); (2) documento con **≥ 2 capas** y ruta que no es `.aseprite` (un PNG abierto al que se añadieron capas) ⇒ **abre el diálogo de guardar** proponiendo `{nombre}.aseprite`, con el aviso «este PNG tiene {n} capas: elige .aseprite para conservarlas» (si el usuario elige un `.png`, se guarda aplanado como hoy y sigue el aviso «se guarda aplanado»); (3) sin ruta ⇒ diálogo como hoy, proponiendo `sin-titulo.aseprite` si hay ≥ 2 capas y `sin-titulo.png` si no; (4) el resto, como hoy. `Ctrl+Mayús+S` y «Guardar como…»: el diálogo propone el formato del documento (`.aseprite` si la ruta lo es o hay ≥ 2 capas; si no, PNG). `Archivo → Exportar PNG aplanado…` no cambia.
- **«El título dice el formato»**: lo entiendo como (a) el nombre del fichero con su extensión en el título de la ventana (ya lo hace `title_with_file`: `nombre.aseprite*`) y (b) los **títulos de los diálogos**, que dicen el formato: «Abrir imagen (PNG o Aseprite)» al abrir y «Guardar como .aseprite (con capas) o PNG (aplanado)» al guardar un documento con capas (y «Guardar PNG» / «Guardar .aseprite» cuando el formato ya está decidido por la ruta del documento).
- **Textos que cambian**: `with_flatten_notice` («se guarda aplanado; el formato con capas llega con .aseprite» ⇒ «se guarda aplanado; con Guardar como… .aseprite conservas las capas») y el uso («uso: px-app [archivo.png]» ⇒ «uso: px-app [archivo]»).
- **Gris:** guardar un documento de Gris o Gris con alfa como `.aseprite`, y abrir uno de 16 bits, dan el error `Unsupported` («el modo gris aún no se admite en .aseprite: convierte a RGBA o indexado; el fichero no se ha modificado»); el PNG sigue guardando gris. **Candidato fácil para el hito 6** (anotado, con su caso de corpus): la profundidad de 16 bits del formato es valor + alfa (§References, `PIXEL`), es decir, el `GrayAlpha` de Pixoko; y `Gray` sin alfa se escribiría como `GrayAlpha` con alfa 255 (lo que no vuelve igual, de ahí que se decida entonces). Un caso `gris-alfa-capas` (RGBA→Gris con alfa, 2 capas) se añade al corpus del hito 6.
- **Acciones y ayuda (condición 9):** no hay acciones nuevas (ni entradas de menú ni teclas): `Abrir`, `Guardar` y `Guardar como` ya existen. No hay textos de ayuda nuevos; si durante la unidad aparece una acción, sus textos van a un fichero aparte (`help_texts_aseprite_es.rs`) para tu revisión.
- **Copia:** `copy_path` conserva la extensión `.aseprite` (`nombre-copia.aseprite`); para todo lo demás, como hoy.

## Pruebas anteriores que cambian (lista por lectura, sin copia desechable)

Si alguna necesita más que firmas, literales o cifras, **me paro** (CLAUDE.md, modo ahorro). Commit propio, antes de las pruebas nuevas, con el mensaje explicando qué comprobaba cada una.

| Prueba | Qué comprobaba | Cambio |
| --- | --- | --- |
| `px-app/src/session/tests.rs:1031` (propiedad de `load`) | que `Loaded.image` es la imagen leída | lee `loaded.document.canvas_image()`/la celda en vez de `loaded.image` (firma) |
| `px-app/src/session/layers_tests.rs:629` | el texto del aviso de aplanado | literal nuevo |
| `px-app/src/files.rs` (uso), `licenses.rs:99`, `px-app/tests/arranque.rs:83` | el mensaje de uso | literal nuevo (`[archivo]`) |
| Las que construyen un `Loaded` a mano o leen su campo `image` (`tiles_tests.rs:692, 726`, `puerta/grande.rs:165` llaman a `install(cargado)` y se ven afectadas solo si lo construyen) | cargar un PNG en la sesión | firma |
| Pruebas de `Session::save_to`/`with_png_extension`/`copy_path` con extensiones | que el destino acaba en `.png` | **no cambian** (otras extensiones siguen siendo PNG; `.aseprite` es nuevo) |

Hasta donde he leído, **ninguna necesita más que firmas y literales**; la medida real (compilar y ver qué falla) la hace el commit de adaptación.

## Casos (12 filas; una prueba por fila salvo donde se indica; 40 pruebas con las 4 propiedades, el tope de la spec)

Dónde: `px-core/tests/layers.rs` (fila 1), `px-formats/tests/aseprite.rs` (nuevo; filas 2 a 11, con datos de `golden/aseprite/` y ficheros construidos a mano **desde las tablas de arriba**, con un escritor mínimo propio en la prueba) y `px-golden/tests/png_interop/aseprite_read.rs` (filas 3 y 8, módulo del ejecutable existente), `px-app/src/session/aseprite_tests.rs` (fila 12).

| Fila | Qué | Resultado |
| --- | --- | --- |
| 1 | `Document::from_layers`: un documento correcto; 0 capas, 257, nombre de 256 bytes, celda de otro tamaño, modo o paleta, indexado con opacidad o mezcla, celda vacía sin valor transparente, celdas que pasan de `cells_limit` | ids `0..n`, historia vacía, `state_id` 0, `==` con el construido a mano (2 pruebas); cada defecto, su error y ningún documento construido (1 prueba de tabla) |
| 2 | Lectura de la estructura de casos del corpus: `capas-opacidad` (6 capas, opacidades 255, 0, 64, 128, 200, 255), los 8 `mezcla-*` (la capa `fuente` tiene el modo de su nombre), `capas-ocultas` y **`capa-bloqueada`** (visibilidad y bloqueo independientes: `bloqueada` visible y bloqueada, `oculta-bloqueada` oculta y bloqueada, las otras ni una cosa ni otra), `celda-vacia` (`sin-celda` vacía con Multiplicar y 100, `transparente` equivalente a una vacía: sin imagen), `indexado-transparente-7` (índice 7) | las propiedades de la tabla del corpus (4 pruebas) |
| 3 | Composición de los 29 `.aseprite` con `px_raster::compose` contra su PNG aplanado (RGBA resuelto, indexado por su paleta) | **tolerancia 0** en todos los píxeles, el RGB bajo alfa 0 no se exige si los dos alfa son 0; dentro del dominio de 3.2 (fondo translúcido y modo ≠ Normal) alfa exacto y error de canal ≤ 2 con el suelo de 1 por caso agregado, **sin ensancharlo**; un fallo da el nombre y el primer píxel (1 prueba) |
| 4 | Paleta: `indexado-paleta-256` contra la paleta de su PNG (256 colores, alfa y índice); `0x2019` con alfa, nombres y rangos; `0x0004` con saltos y 0 = 256; los dos a la vez (gana `0x2019`); indexado sin paleta; solo `0x0011`; paleta en RGBA | colores y alfa exactos; los dos últimos casos, `NoPalette` y `Unsupported`; RGBA ignora (`RgbaPalette`, sin `lossy`) (3 pruebas) |
| 5 | Lo que se ignora: etiquetas, datos de usuario, cortes, ficheros externos, extra de celda, tipo desconocido, ICC, gamma; perfil sRGB; fondo indexado con índice transparente | `ignored` con tipos y cuentas; `lossy` solo cuando toca (2 pruebas) |
| 6 | Lo que no se entiende: grupo, mapa de tiles (capa y celda), referencia, celda enlazada, grises (16 bpp), opacidad de celda ≠ 255, z-index ≠ 0, capa indexada con opacidad o mezcla, **los códigos de mezcla 0 a 40 de la tabla** | los 8 válidos, `Ok` con el `BlendMode` de la tabla; los demás, `Unsupported` con el código (3 pruebas) |
| 7 | Fotogramas: un fichero de 3 fotogramas (el 2.º y el 3.º con basura detrás del primero), uno de 0 y uno de 1 fotograma con bytes de más | primero leído, `frames_in_file = 3`, `lossy`; 0 ⇒ `Malformed`; bytes de más ⇒ `Malformed` (2 pruebas) |
| 8 | Celdas: tipos 0 y 2 dan el mismo resultado; desplazada, recortada por los cuatro lados, de 0 píxeles, del todo fuera (`celda-recortada` tiene dos) y toda transparente | la celda del modelo es el recorte exacto sobre el relleno; fuera o toda transparente ⇒ vacía (2 pruebas, una con el corpus) |
| 9 | El lector no se fía: cada prefijo de un fichero válido; tamaño de cabecera distinto; magias; chunk de < 6 bytes o que sale del fotograma; fotograma que sale del fichero; 65 537 chunks declarados con el fichero corto; 257 capas; celda con 65 535 × 65 535 y 20 bytes de zlib (**sin reservar**, mismo método que `png_reserve.rs`); zlib con Adler-32 erróneo, corto o largo; celda repetida o con índice de capa ≥ n; índice de píxel fuera de la paleta; nombre no UTF-8 o de 256 bytes; ancho 0; lienzo de más de 2²⁶ píxeles (el error dice el límite y no se reserva) | cada uno, su error y **nunca pánico**; el de la bomba sube el pico de RSS menos de 32 MiB (4 pruebas). **Más una prueba `#[ignore]`** (memoria; se corre una vez al cerrar, no es del CI): un fichero de 8 capas con un píxel cada una en un lienzo de 8192² (8 celdas de 256 MiB = 2 GiB, lo que el núcleo deja crear) se reabre entero; con una novena capa da `TooLarge` y el mensaje dice el límite |
| 10 | Escritura (el mensaje de `NoTransparentIndex` es el accionable de arriba, comprobado literalmente): el fichero que sale se lee con un lector mínimo de la prueba escrito **solo con las tablas de arriba**: cabecera, fotograma, orden de chunks (perfil, paleta si indexado, capas, celdas), banderas de capa, los 8 códigos de mezcla, rectángulo mínimo, celda vacía sin chunk, paleta `0x2019`; errores de escritura: gris, indexado sin índice transparente, lienzo > 65 535 | coinciden campo a campo con la tabla; los errores, `Unsupported`/`NoTransparentIndex`/`TooLarge` (3 pruebas) |
| 11 | Ida y vuelta: RGBA de 3 capas con propiedades variadas, celda vacía, celda materializada y toda transparente (vuelve vacía: equivalentes) y RGB bajo alfa 0 dentro de lo dibujado; indexado de 256 colores con alfa e índice 255; nombre de 255 bytes con multibyte; 256 capas; 1×1; 65 535 × 1; y el fichero que se escribió dos veces | `mismo_contenido` (la igualdad de la condición 1, sin ids); mismos bytes en la segunda escritura (3 pruebas) |
| 12 | App: abrir un `.aseprite` (instala el documento con capas, capa activa la de arriba, aviso); un `.aseprite` con `lossy` y `Ctrl+S` ⇒ `-copia.aseprite`; `Ctrl+S` con ruta `.aseprite` escribe `.aseprite`; `Guardar como` por extensión; abrir por contenido (un PNG llamado `.aseprite` y al revés); gris a `.aseprite` ⇒ error con motivo; PNG con 2 capas pide diálogo (regla 2); tras guardar como `.aseprite` el documento está en ese fichero y no modificado; un `.png` elegido en ese diálogo sigue con el aviso de aplanado | los resultados de «Interfaz» (5 pruebas) |

## Propiedades (4; `proptest`, 256 casos, tamaño hasta 12 × 12, 1 a 6 capas, RGBA e indexado)

- **P1. Ida y vuelta.** Para todo documento generado (capas con nombre, visibilidad, opacidad, mezcla, bloqueo y celda vacía o con píxeles aleatorios, RGB bajo alfa 0 incluido; indexado con paleta de hasta 256 entradas con alfa): `read_aseprite(write_aseprite(d))` tiene `mismo_contenido` que `d`, y además `write(read(write(d))) == write(d)` (bytes estables).
- **P2. `from_layers` no construye documentos inválidos** (condición 5). Para listas arbitrarias de `LayerSpec` (tamaños, modos y paletas incoherentes, opacidades, nombres largos, 0 a 300 capas, celdas que pasan del tope) y cualquier lienzo: o `Err`, o un documento que cumple **todas** las invariantes: de 1 a 256 capas, nombres ≤ 255 bytes, toda celda con imagen del ancho, alto, modo y **paleta idéntica** a la del lienzo, indexado con opacidad 255 y mezcla Normal, `materialized_bytes() ≤ cells_limit()`, ids `0..n`, historia vacía.
- **P3. Robustez.** Para bytes arbitrarios y para ficheros válidos truncados o con bytes cambiados: `read_aseprite` no entra en pánico, y si da `Ok`, el documento cumple sus invariantes (≤ 256 capas, celdas del tamaño del lienzo, índices < paleta, celdas materializadas ≤ el tope).
- **P4. Colocación de la celda.** Un fichero construido a mano con una celda de tamaño y posición arbitrarios (negativos, fuera, parcial): el píxel `(px, py)` del documento es el de la celda en `(px − x, py − y)` si cae dentro y el relleno si no; vacía si la intersección es vacía.

## Fuzz

Un objetivo nuevo, `fuzz/fuzz_targets/read_aseprite.rs`, con su `[[bin]]` en `fuzz/Cargo.toml` (sin dependencias nuevas fuera de `miniz_oxide` 0.8.9, que ya trae `px-formats`): llama a `read_aseprite` con límites pequeños (2²⁴ píxeles, 256 MiB), y si da `Ok` escribe el documento, lo lee otra vez y exige `mismo_contenido`. `docs/FUZZING.md` gana su sección (semillas: los **29** `.aseprite` de `golden/aseprite`; `-rss_limit_mb=4096 -timeout=10`). Se corre una pasada de 60 s al cerrar; no es puerta del CI. `fuzz/deny.toml` se deja al día (misma lista de licencias que el de la raíz) y `cargo deny --manifest-path fuzz/Cargo.toml --locked --config fuzz/deny.toml check` se repite; `cargo deny check` de la raíz verifica `miniz_oxide` 0.8.9 como dependencia directa y se anota en `docs/DEPENDENCIES.md`.

## Commits (orden de CLAUDE.md)

1. spec (con `docs/REFERENCIAS.md`). 2. **paso previo**: el caso 29 del corpus (scripts, lista de `px-golden` y `corpus-hito-3.md`; solo scripts y docs más la lista fija de la prueba); el usuario genera `capa-bloqueada` y pasa los ficheros; se listan sus chunks y se anota la regla del bloqueo en esta spec («Confirmación del bloqueo»). 3. adaptación de pruebas anteriores (tabla de arriba; mensaje explicativo; si alguna necesita más que firmas, literales o cifras, me paro). 4. pruebas en rojo por el **`test-writer`** (esta unidad sí lo usa: `px-formats` y lógica delicada), solo desde esta spec; no compilan hasta `C-02`/`F-01`, y solo en sus ejecutables. 5. lógica `px-core` (`C-01` a `C-04`). 6. lógica `px-formats` (lector, escritor, `detect_format`, dependencia, `DEPENDENCIES.md`, fuzz). 7. `px-app`. 8. documentos (`REFERENCIAS.md`, `PLAN-RESUMEN.md`, `INDEX.md`, mapa, una única corrección posterior de la spec). Cierre: `fmt`, `clippy -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y el `license-auditor` una vez (hay dependencia directa nueva y cambia `px-core`/`px-formats`).

## Riesgos

- **R1 (arriba):** la memoria que un fichero pequeño puede pedir está acotada por tope de celdas, no por el tamaño del fichero.
- **R2. El bloqueo.** `locked = !Editable` sale de la tabla («2 = Editable»). El caso 29 del corpus lo comprueba con datos antes de escribir las pruebas (ver «Confirmación del bloqueo»); lo que no cubre es lo que Aseprite hace al leer nuestro fichero, que queda en la prueba manual (paso 5).
- **R3. Lo que Aseprite acepta de nuestro fichero** no se comprueba aquí (no se ejecuta): lo hace la prueba manual. Dudas concretas: que abra un RGBA sin paleta, que acepte la cuadrícula `0, 0, 16, 16` y que lea la celda recortada.
- **R4. Los bytes de la compresión dependen de la versión de `miniz_oxide`.** P2 y la segunda escritura de la fila 11 hablan de la misma compilación; actualizar la crate puede cambiar los bytes (no el contenido).
- **R5. Techo 2 de 3.2.** Si algún caso del corpus no cumple con otro par de alfa, se re-evalúa con datos (nota de 3.2), no se ensancha.
- **R6. Fondo indexado.** Un fondo (bandera 8) indexado cuyo índice transparente se dibuja opaco en el programa de referencia queda transparente en Pixoko; por eso es `lossy` y copia. No hay caso de corpus.
- **R7. Compromiso a largo plazo.** Una vez publicado, el formato escrito es una promesa de compatibilidad; las etapas del hito 5 y 6 solo pueden añadir.

## Fuera de alcance

Hito 5: fotogramas, duraciones, etiquetas y celdas enlazadas (5.5). Hito 6: grises (**candidato fácil**: la profundidad de 16 bits del formato es valor + alfa, es decir, `GrayAlpha`; con su caso de corpus `gris-alfa-capas`), grupos, mapas de tiles, referencia, opacidad de celda, z-index, los diez modos de mezcla restantes, paleta de 6 bits, perfil ICC, propiedades de usuario y conservar los chunks no entendidos.

## Confirmación del bloqueo (paso previo; hecho el 22 de octubre)

`golden/aseprite/capa-bloqueada.aseprite` (624 bytes, 16×16, 32 bpp, un fotograma) tiene los chunks, en orden: color profile `0x2007`, paleta antigua `0x0004`, cuatro capas `0x2004` y cuatro celdas `0x2005` (tipo 2, z-index 0, opacidad 255). Banderas de cada capa (§Layer Chunk: 1 = Visible, 2 = Editable), leídas con la script de la especificación (el desplazamiento del nombre es el 16, tras la mezcla y la opacidad):

| Capa | Banderas | Visible (1) | Editable (2) | Lo que se esperaba |
| --- | --- | --- | --- | --- |
| `fondo` | 3 | sí | sí | visible y desbloqueada |
| `bloqueada` | **1** | sí | **no** | visible y bloqueada |
| `arriba` | 3 | sí | sí | visible y desbloqueada |
| `oculta-bloqueada` | **0** | no | **no** | oculta y bloqueada |

**La regla `locked = !Editable` (bit 2) se confirma**, y la visibilidad (bit 1) es independiente: la capa `bloqueada` sigue visible y la `oculta-bloqueada` combina las dos. Ninguna otra bandera aparece (4, 8, 16, 32 y 64 a cero) y las cuatro son de tipo 0, nivel 0, Normal y opacidad 255. El PNG aplanado (16×16) no contiene el magenta de la capa oculta y sí el verde y el marrón de la bloqueada, el gris del fondo y el azul y el amarillo de `arriba`: se ve lo visible, bloqueado o no. Las filas 2, 10 y 11 y el mapa de la capa quedan como están escritos.

## Visto bueno (2026-10-22; condiciones que prevalecen)

Aprobados expresamente `C-01` a `C-04` (`px-core`) y `F-01` a `F-10` (`px-formats`); cualquier otro cambio, me paro. Las diez decisiones de la propuesta se aceptan con estas condiciones:

1. **Ida y vuelta por contenido de píxeles**, sin ids; una celda materializada y totalmente transparente equivale a una vacía y no se escribe (ver «Lo comprobado» 2, «Celda → modelo», «Escritura de la celda» y las filas 2, 8 y 11).
2. **Indexado sin índice transparente**: error al escribir con el mensaje «esta imagen indexada no tiene entrada transparente: fíjala con Fijar transparente o convierte a RGBA».
3. **`Ctrl+S` de un PNG con 2 o más capas** abre el diálogo de guardar proponiendo `nombre.aseprite`; si se elige `.png`, sigue el aviso «se guarda aplanado»; tras guardar como `.aseprite` el documento pasa a ese fichero y deja de estar modificado; los títulos de los diálogos dicen el formato.
4. **Un solo tope de celdas al abrir**: el del núcleo (2 GiB materializados), no 1 GiB; lienzo máximo de 2²⁶ píxeles, rechazado antes de reservar, con el límite en el error; una prueba de que lo que Pixoko puede crear con 2 GiB lo puede reabrir.
5. **`Document::from_layers` es el único camino del lector** y valida todas las invariantes; la propiedad P2 lo fija.
6. **Lo no soportado** nombra qué es y dice que el fichero no se ha modificado.
7. **Máximo unas 40 pruebas nuevas**; fuzz `read_aseprite` con los 29 `.aseprite` como semillas; `miniz_oxide` 0.8.9 verificada con `cargo deny` y anotada en `docs/DEPENDENCIES.md`; `fuzz/deny.toml` al día.
8. **Gris**: error por ahora; «gris con alfa del formato = `GrayAlpha` de Pixoko» queda anotado como candidato fácil con su caso de corpus.
9. **Textos de ayuda** de acciones nuevas, si las hay, en un fichero aparte para su revisión (hoy no hay acciones nuevas).
10. **Pruebas anteriores que cambian** (`Loaded.image`, el aviso de aplanado, el mensaje de uso) en un commit propio con mensaje explicativo; si alguna necesita más que firmas, literales o cifras, me paro.

Las decisiones 1 a 3 y 5 a 10 de la propuesta (descartar con aviso y copiar al guardar; mapa del modelo; primer fotograma con aviso; error y no degradación; topes de chunks, capas y 1032×; `miniz_oxide` 0.8.9 nivel 6; rectángulo mínimo; sRGB y paleta `0x2019`) valen tal como están escritas arriba.

## Prueba manual (la haces tú; Aseprite no se ejecuta en la unidad)

1. Abre en Pixoko cada `.aseprite` de `golden/aseprite/` (los 29; o al menos `mixto-rgba`, `celda-recortada`, `indexado-celdas`, `mezcla-ocho-fondo-translucido`) y compáralos con su PNG: capas, nombres, opacidad y ocultas en el panel de capas.
2. Cierra, abre `indexado-transparente-7` y comprueba en el editor de paleta que el índice transparente es el 7.
3. Dibuja en una capa, añade una capa y pulsa `Ctrl+S` sobre un PNG de una capa abierto antes: debe abrir el diálogo con `.aseprite` propuesto; guarda `prueba.aseprite`.
4. **Abre `prueba.aseprite` con Aseprite**: capas en su orden, nombres, opacidades, modos de mezcla, capa oculta, paleta e índice transparente; que no pida arreglar nada ni avise de formato.
5. En Pixoko bloquea una capa y guarda; ábrelo en Aseprite y mira si la capa sale bloqueada (R2). Haz lo contrario: bloquea una en Aseprite, guarda y ábrela en Pixoko.
6. Con Aseprite crea un `.aseprite` con una **etiqueta** y dos fotogramas, ábrelo en Pixoko: aviso de etiquetas y «solo se abre el fotograma 1 de 2»; `Ctrl+S` debe crear `…-copia.aseprite` sin tocar el original.
7. Crea en Aseprite una capa con modo «Color Dodge» y ábrela: error claro, sin abrir nada.
8. Un `.aseprite` indexado con capa de fondo y píxeles del índice transparente: aviso de `lossy` (R6).
9. Gris: convierte a Gris y prueba `Guardar como… .aseprite`: error con motivo; el PNG se guarda.
10. Arrastra un `.aseprite` y un PNG sobre la ventana y abre uno por la línea de comandos (`px-app x.aseprite`).
