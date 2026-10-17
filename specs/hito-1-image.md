# Hito 1 · Unidad 3: `Image`, imagen en modos RGBA, indexado y grises

Estado: **aprobada** (2026-10-17)

## Objetivo

Definir en `px-core` la imagen de un solo plano: un rectángulo de píxeles en uno de cuatro modos (RGBA, indexado, grises y grises con alfa). Es la base del documento y los comandos (unidad 4) y del PNG (unidad 5), que debe poder hacer la ida y vuelta sin perder ningún valor de píxel. Esta unidad solo crea, lee y escribe píxeles de uno en uno.

## Entradas y salidas

- `px_core::Mode`: enum `Rgba`, `Indexed`, `Gray`, `GrayAlpha`. Deriva `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`.
- `px_core::Pixel`: un píxel con el tipo de su modo. Deriva `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`.
  - `Rgba(Rgba)`: alfa directo, como en `Rgba`.
  - `Indexed(u8)`: índice en la paleta de la imagen.
  - `Gray(u8)`: gris opaco.
  - `GrayAlpha { gray: u8, alpha: u8 }`: gris con alfa directo.
- `px_core::Image`: deriva `Clone`, `Debug`, `PartialEq`, `Eq`. La igualdad compara modo, dimensiones, píxeles y paleta.
- `pub const Image::MAX_BYTES: usize = 1 << 30`: constante pública, 1 GiB de píxeles (un RGBA de 16384 × 16384 cabe justo).
- `Image::byte_len(width: u32, height: u32, mode: Mode) -> Result<usize, ImageError>`: función pura y pública con el tamaño en bytes de los píxeles; no reserva memoria (ver comportamiento 2).
- `Image::new(width: u32, height: u32, mode: Mode) -> Result<Image, ImageError>`: para `Rgba`, `Gray` y `GrayAlpha`. Con `Mode::Indexed` devuelve `PaletteRequired`.
- `Image::new_indexed(width: u32, height: u32, palette: Palette) -> Result<Image, ImageError>`.
- `width() -> u32`, `height() -> u32`, `mode() -> Mode`.
- `palette() -> Option<&Palette>`: `Some` solo en modo indexado.
- `get_pixel(x: u32, y: u32) -> Result<Pixel, ImageError>`.
- `set_pixel(x: u32, y: u32, pixel: Pixel) -> Result<(), ImageError>`.
- `ImageError`: enum con
  - `ZeroSize { width: u32, height: u32 }`: ancho o alto cero.
  - `TooLarge { width: u32, height: u32 }`: (los valores pedidos) el tamaño en bytes desborda, supera `MAX_BYTES` o no se pudo reservar.
  - `PaletteRequired`: `new` con `Mode::Indexed`.
  - `EmptyPalette`: `new_indexed` con una paleta sin entradas.
  - `OutOfBounds { x: u32, y: u32, width: u32, height: u32 }`: la coordenada cae fuera de la imagen.
  - `ModeMismatch { expected: Mode, found: Mode }`: el `Pixel` no es del modo de la imagen (`expected` es el modo de la imagen y `found` el del píxel).
  - `IndexNotInPalette { index: u8, len: usize }`: `Indexed(index)` con `index >= len` de la paleta.

  Implementa `Display`, `Error`, `Debug`, `Clone`, `PartialEq`, `Eq`.

## Comportamiento

1. **Modos.** Cuatro modos, porque PNG distingue gris (tipo 0) de gris con alfa (tipo 4) y la ida y vuelta no puede inventar un alfa ni quitarlo. `Gray` no tiene alfa (opaco). Un `Pixel` guarda exactamente lo que se escribió: no se normaliza ni se convierte (`Rgba(1,2,3,0)` conserva su RGB). La conversión entre modos queda para el hito 2.
2. **Dimensiones.** `width >= 1` y `height >= 1`; con cero, `Err(ZeroSize)`. Así una imagen siempre tiene al menos un píxel. `Image::byte_len` calcula el tamaño en bytes (válido para los cuatro modos, `Indexed` incluido, y sin comprobar la paleta) y `new` y `new_indexed` lo usan. El tamaño es `width * height * bytes_por_píxel` (RGBA 4, grises con alfa 2, indexado y grises 1), calculado con `usize::try_from` y `checked_mul` en cada paso: si desborda o supera `MAX_BYTES`, `Err(TooLarge)`, sin reservar memoria antes de comprobarlo. La reserva usa `try_reserve_exact`: si falla, también `TooLarge` (nunca aborta). El resultado es el mismo en 32 y 64 bits.
3. **Orden de comprobación al crear.** Primero las dimensiones (`ZeroSize`, luego `TooLarge`), después el modo y la paleta (`PaletteRequired` o `EmptyPalette`). Un `Err` no deja nada reservado.
4. **Contenido inicial.** Todo a ceros: `Rgba(Rgba::TRANSPARENT)`, `Gray(0)` (negro opaco), `GrayAlpha { gray: 0, alpha: 0 }` (transparente) e `Indexed(0)`.
5. **Coordenadas.** `x` crece hacia la derecha y `y` hacia abajo, desde `(0, 0)` arriba a la izquierda. Son `u32`: no hay coordenadas negativas y quien parta de `i32` convierte antes. `get_pixel` y `set_pixel` con `x >= width` o `y >= height` devuelven `Err(OutOfBounds)` con la coordenada y el tamaño; nunca hacen pánico, tampoco con `u32::MAX`.
6. **`set_pixel` estricto.** El `Pixel` debe ser del modo de la imagen: si no, `Err(ModeMismatch { expected, found })` (`Gray` y `GrayAlpha` son modos distintos). Orden de comprobación: `OutOfBounds`, `ModeMismatch`, `IndexNotInPalette`.
7. **Atomicidad.** Si `set_pixel` devuelve error, la imagen queda exactamente igual que antes. Un `set_pixel` correcto solo cambia ese píxel.
8. **Indexado: invariante.** `palette()` es `Some` si y solo si el modo es `Indexed`; en ese caso la paleta tiene al menos una entrada y todo píxel cumple `índice < paleta.len()`. `new_indexed` exige paleta no vacía (`EmptyPalette`; como toda imagen tiene píxeles, la regla no tiene excepciones) y `set_pixel(Indexed(i))` exige `i < len` (`IndexNotInPalette`). La imagen es dueña de la paleta y **no expone `&mut Palette`**, pero eso no basta: el invariante lo sostienen a la vez la comprobación de `set_pixel` (un `Indexed(i)` con `i >= paleta.len()` es siempre un error y no escribe nada) y la ausencia de una vía para cambiar la paleta. Si alguna de las dos faltara, un píxel podría apuntar fuera. Editar la paleta de una imagen (cambiar un color, añadir, quitar o reordenar remapeando los píxeles) va con el editor de paletas del hito 2, como métodos de `Image` que mantengan el invariante.
9. **Indexado y transparencia.** `get_pixel` devuelve el índice guardado (`Indexed(i)`), sin resolverlo, también si `i` es el índice transparente. El color efectivo se pide a `palette().resolved(i)`.
10. **Almacenamiento (privado).** Un solo `Vec<u8>` fila a fila, de arriba abajo y de izquierda a derecha, sin relleno entre filas (`stride = width * bytes_por_píxel`). El píxel `(x, y)` empieza en `(y * width + x) * bytes_por_píxel` (el producto cabe: ya se comprobó en el comportamiento 2). Orden de los bytes: RGBA `r, g, b, a`; grises con alfa `gray, alpha`; indexado y grises, un byte. Los campos son privados; el diseño no se observa desde fuera salvo por `get_pixel` y `set_pixel`.

## Fuera de alcance

- Conversión entre modos, relleno, recorte, cambio de tamaño y copia de regiones (hito 2 en adelante).
- Cambiar la paleta de una imagen indexada y cualquier operación que desplace índices (hito 2; ver comportamiento 8).
- Acceso a los bytes o a filas enteras: lo añade la unidad que lo necesite (probablemente la 5, para el PNG), con su propia spec.
- Capas, mezcla y composición (hito 3).
- Profundidades de 16 bits y menores de 8 bits, RGB sin alfa y clave de color de PNG: son asuntos de la unidad 5 (ver notas).

## Notas para la unidad 5 (PNG)

- Correspondencia con los tipos de color de PNG: gris 8 bits → `Gray`; gris con alfa → `GrayAlpha`; indexado → `Indexed` con su paleta (alfa por entrada con tRNS, ver la spec de `Palette`); RGBA 8 bits → `Rgba`. Al escribir se usa el mismo tipo de color con el que se leyó, así que la ida y vuelta conserva los valores de todos los píxeles.
- Los PNG indexados de menos de 8 bits se expanden a un índice por byte al leer; la clave de color tRNS en gris o RGB se resuelve en la página de la unidad 5. La ida y vuelta es «sin pérdida» sobre los valores de píxel, no sobre los bytes del fichero.
- **Los PNG de 16 bits y los RGB sin alfa se convierten de forma explícita y documentada, nunca en silencio**: la página de la unidad 5 fija la conversión de cada caso (por ejemplo, 16 → 8 bits y RGB → `Rgba` con `a = 255`) y el lector la señala al llamador (aviso o campo en el resultado). El corpus de ida y vuelta distingue esos casos de los exactos: los exactos se comparan píxel a píxel y los convertidos se prueban contra la conversión documentada.
- Una imagen indexada siempre tiene paleta no vacía, así que el error de «paleta vacía» de la unidad 2 no puede darse al escribir una `Image`.

## Casos límite

Notación: `P[...]` es una `Palette` con esas entradas; `c0`, `c1`… son colores cualesquiera; `M` es cualquier modo no indexado.

| Operación | Resultado |
| --- | --- |
| `new(4, 3, Rgba)` | `Ok`; `width() == 4`, `height() == 3`, `mode() == Rgba`, `palette() == None`; los 12 píxeles son `Rgba(Rgba::TRANSPARENT)` |
| `new(2, 2, Gray)`; `new(2, 2, GrayAlpha)` | todos `Gray(0)`; todos `GrayAlpha { gray: 0, alpha: 0 }` |
| `new(1, 1, M)` para cada `M` | `Ok` (tamaño mínimo); `get_pixel(0, 0)` es el valor inicial |
| `new(0, 5, M)`, `new(5, 0, M)`, `new(0, 0, M)` | `Err(ZeroSize { width, height })` con los valores pedidos |
| `new(u32::MAX, u32::MAX, Rgba)` y con `Gray` | `Err(TooLarge { .. })` (el producto desborda) |
| `new(u32::MAX, 1, Gray)`; `new(16385, 16384, Rgba)` | `Err(TooLarge { .. })` (superan `MAX_BYTES`); no reserva memoria |
| `byte_len(16384, 16384, Rgba)`; `byte_len(MAX_BYTES as u32, 1, Gray)`; `byte_len(16384, 16384, Indexed)` | `Ok(MAX_BYTES)`, `Ok(MAX_BYTES)` y `Ok(16384 * 16384)` (el límite exacto cabe) |
| `byte_len(MAX_BYTES as u32 + 1, 1, Gray)`; `byte_len(1, MAX_BYTES as u32 + 1, Gray)`; `byte_len(16385, 16384, Rgba)`; `byte_len(u32::MAX, u32::MAX, Rgba)` | `Err(TooLarge { width, height })` con los valores pedidos (un byte de más; desbordamiento en el último) |
| `byte_len(0, 5, M)`; `byte_len(5, 0, M)` | `Err(ZeroSize { width, height })` |
| `new(2, 2, Indexed)`; `new(0, 2, Indexed)` | `Err(PaletteRequired)`; `Err(ZeroSize)` (las dimensiones se comprueban antes) |
| `new_indexed(2, 2, P[c0, c1])` | `Ok`; `mode() == Indexed`; `palette() == Some(&P[c0, c1])`; todos los píxeles `Indexed(0)` |
| `new_indexed(2, 2, Palette::new())`; `new_indexed(0, 2, Palette::new())`; `new_indexed(u32::MAX, u32::MAX, P[c0])` | `Err(EmptyPalette)`; `Err(ZeroSize)`; `Err(TooLarge)` |
| imagen 3 × 2: `get_pixel` en `(0,0)` y `(2,1)`; en `(3,0)`, `(0,2)`, `(3,2)` y `(u32::MAX, u32::MAX)` | `Ok` en las dos esquinas; `Err(OutOfBounds { x, y, width: 3, height: 2 })` en el resto |
| `set_pixel` + `get_pixel` en cada modo: `Rgba(c)`, `Rgba(Rgba::new(1,2,3,0))`, `Indexed(1)`, `Gray(7)`, `GrayAlpha { gray: 7, alpha: 9 }` | `Ok(())` y `get_pixel` devuelve el mismo valor (RGB intacto con `a = 0`); los demás píxeles no cambian |
| `set_pixel(3, 0, p)` y `set_pixel(u32::MAX, 0, p)` en una imagen 3 × 2 | `Err(OutOfBounds { .. })`; imagen igual a una copia previa |
| `set_pixel(0, 0, Gray(1))` en una imagen `Rgba`; `GrayAlpha {..}` en una `Gray`; `Indexed(0)` en una `Rgba` | `Err(ModeMismatch { expected, found })`; imagen sin cambios |
| paleta de 2: `set_pixel(0, 0, Indexed(1))`; `Indexed(2)` y `Indexed(255)`; paleta de 1: `Indexed(1)`; paleta de 256: `Indexed(255)` | `Ok`; `Err(IndexNotInPalette { index: 2, len: 2 })` y `{ index: 255, len: 2 }`, imagen sin cambios; `Err(IndexNotInPalette { index: 1, len: 1 })`; `Ok` |
| `P[c0, c1]` con transparente `Some(1)`: `set_pixel(0, 0, Indexed(1))` y `get_pixel(0, 0)` | `Indexed(1)` (no se resuelve); `palette().unwrap().resolved(1)` da `c1` con `a = 0` |
| coordenada fuera y `Pixel` de otro modo a la vez; `Pixel` de otro modo e `Indexed` fuera de paleta | `OutOfBounds` (gana a `ModeMismatch`); `ModeMismatch` en una imagen `Rgba` con `Indexed(9)` |

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (no se añade ninguna dependencia: `proptest` ya está en `px-core`)

## Tests

En `crates/px-core/tests/image.rs` (integración sobre la API pública). Los tests no reservan más de unos pocos KiB: el límite exacto de `MAX_BYTES` se prueba con `byte_len`, que no reserva; `new` y `new_indexed` solo se prueban con tamaños pequeños y por el lado del rechazo.

**Unitarios**: una prueba por fila de la tabla de casos límite.

**Propiedades** (`proptest`):

1. **Dimensiones y tamaño.** Para cualquier `width, height: u32` y modo no indexado: `Err(ZeroSize)` si alguno es cero; si no, `Err(TooLarge)` si `width * height * bytes_por_píxel` (calculado en `u128`) supera `MAX_BYTES`; si no, `Ok` con las dimensiones y el modo pedidos y todos los píxeles al valor inicial. Los casos que darían `Ok` se limitan a un tamaño pequeño para no reservar memoria. `byte_len` devuelve el mismo `ZeroSize` o `TooLarge`, y en el resto `Ok(width * height * bytes_por_píxel)`, sin límite de tamaño de prueba.
2. **Coordenadas.** Para una imagen de cualquier modo y cualquier `(x, y)` de `u32` completo: `get_pixel` es `Ok` si y solo si `x < width && y < height`; si no, `OutOfBounds` con esos mismos valores. Nunca hay pánico.
3. **Ida y vuelta y localidad.** Para cualquier imagen pequeña (1 a 16 de lado, cualquier modo; el indexado con paleta de 1 a 256 entradas) y cualquier `(x, y)` válidos y `Pixel` válido para ese modo: tras `set_pixel`, `get_pixel(x, y)` devuelve ese `Pixel` exacto y todos los demás píxeles son iguales que antes.
4. **Secuencias contra un modelo.** Para una secuencia de 0 a 80 `set_pixel` con `(x, y)` en un rango algo mayor que la imagen y `Pixel` de cualquier variante (incluidos otros modos e índices fuera de paleta), comparada con un `Vec<Pixel>` simple como modelo: cada operación devuelve el error esperado según el orden del comportamiento 6 o `Ok`; si es error la imagen no cambia; y tras cada operación todos los píxeles coinciden con el modelo. Ninguna hace pánico.
5. **`set_pixel` con índice fuera de la paleta.** Para una paleta de `n` entradas (1 a 256) y cualquier `index: u8`: `set_pixel(x, y, Indexed(index))` en un `(x, y)` válido es `Ok` si y solo si `index < n`; si no, `Err(IndexNotInPalette { index, len: n })` y la imagen queda igual que una copia previa.
6. **Invariante del indexado.** Tras cualquier secuencia de la propiedad 4, con índices `u8` cualesquiera, sobre una imagen indexada: `palette()` es igual que la inicial y todo `get_pixel` válido es `Indexed(i)` con `i < palette().len()`.

No hay dorado: la imagen no tiene un comportamiento de programa de referencia que comparar; los tipos de color de PNG que justifican los cuatro modos se citarán en la página de la unidad 5.

## Decisiones

Las marcadas con **?** necesitan visto bueno; el resto son consecuencia directa de ellas.

1. **? Cuatro modos: `Gray` y `GrayAlpha` separados.** Un solo modo «grises con alfa siempre» haría que un PNG de gris sin alfa saliera como gris con alfa (cambia el fichero y obliga a decidir qué alfa inventar); uno «sin alfa» perdería el alfa de los PNG tipo 4. Coste: una variante más.
2. **? `Pixel` es un enum con una variante por modo y `set_pixel` es estricto** (`ModeMismatch`, sin conversión implícita). Alternativa descartada: `get_pixel` devolviendo siempre `Rgba` resuelto; pierde el índice y rompe la ida y vuelta del indexado.
3. **? Ancho o alto cero es error (`ZeroSize`)** y hay un tope `MAX_BYTES = 1 GiB` (`TooLarge`). Sin tope, un tamaño como 100 000 × 100 000 podría aceptarse o fallar según la máquina y el sistema; con él, el resultado es el mismo en todas. 1 GiB admite lienzos de 16384 × 16384 RGBA; subirlo es cambiar una constante. `byte_len` es pública para poder probar el límite exacto sin reservar 1 GiB (y como asociada de `Image`, no como función suelta de la crate, para no ocupar un nombre genérico).
4. **? La imagen indexada es dueña de la paleta y solo la expone como `&Palette`.** Con esto el invariante «todo índice < `paleta.len()`» se sostiene sin comprobaciones extra, pero en esta unidad la paleta de una imagen no se puede editar; el editor del hito 2 añade métodos que la cambian y remapean los píxeles a la vez. Alternativa: exponer `push` y `set` ahora (también seguros hoy), a costa de más API sin un uso inmediato.
5. **`new_indexed` aparte de `new`**: el indexado necesita paleta y `new` solo toma el modo. `new(.., Mode::Indexed)` devuelve `PaletteRequired` en lugar de inventar una paleta.
6. **Contenido inicial a ceros** en todos los modos (transparente en RGBA y grises con alfa, negro opaco en grises, índice 0 en indexado).
7. **`get_pixel` devuelve `Result`, no `Option`**, igual que `Palette::get`: el error dice qué coordenada falló y contra qué tamaño. Coordenadas `u32`.
8. **Almacenamiento en un `Vec<u8>` plano**, sin relleno entre filas y con campos privados: es el formato que querrá el PNG y evita cuatro vectores distintos. No hay `unsafe` ni dependencias nuevas.
