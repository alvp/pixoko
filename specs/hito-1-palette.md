# Hito 1 · Unidad 2: `Palette`, paleta de hasta 256 colores

Estado: **aprobada** (2026-10-17)

## Objetivo

Definir en `px-core` la paleta del modo indexado: una lista ordenada de hasta 256 colores `Rgba`, cada uno con su propio alfa (como la tabla PLTE + tRNS de un PNG indexado), y un índice transparente opcional. La unidad 3 (`Image` indexada) guardará píxeles `u8` que apuntan a esta paleta, y la unidad 5 (PNG) la leerá y escribirá.

## Entradas y salidas

- Tipo: `px_core::Palette`. Deriva `Clone`, `Debug`, `Default`, `PartialEq`, `Eq`. `Default` es la paleta vacía, sin índice transparente. La igualdad compara los colores y el índice transparente.
- `Palette::MAX_COLORS: usize = 256`.
- `Palette::new() -> Palette` (igual que `Default`).
- `Palette::from_colors(impl IntoIterator<Item = Rgba>) -> Result<Palette, PaletteError>`.
- `len() -> usize`, `is_empty() -> bool`, `colors() -> &[Rgba]`.
- `get(index: u8) -> Result<Rgba, PaletteError>`: el color tal como está guardado.
- `resolved(index: u8) -> Result<Rgba, PaletteError>`: el color efectivo (ver comportamiento 5).
- `push(color: Rgba) -> Result<u8, PaletteError>`: añade al final y devuelve el índice de la entrada nueva.
- `set(index: u8, color: Rgba) -> Result<(), PaletteError>`: sustituye una entrada.
- `transparent_index() -> Option<u8>`.
- `set_transparent_index(index: Option<u8>) -> Result<(), PaletteError>`.
- `PaletteError`: enum con
  - `TooManyColors`: se intentó superar `MAX_COLORS` entradas.
  - `IndexOutOfRange { index: u8, len: usize }`: el índice es `>= len`.

  Implementa `Display`, `Error`, `Debug`, `Clone`, `PartialEq`, `Eq`.

Los índices son `u8`: con un máximo de 256 entradas cubren toda la paleta y coinciden con el píxel de la imagen indexada. La longitud sigue siendo `usize`.

## Comportamiento

1. **Capacidad.** De 0 a 256 entradas, ambas incluidas. Una paleta vacía es válida. Con 257 o más, `from_colors` devuelve `Err(TooManyColors)`; con 256 entradas, `push` devuelve `Err(TooManyColors)`.
2. **Alfa por entrada.** Cada entrada guarda su `Rgba` completo, con su alfa, sin normalizar: una entrada con `a = 0` conserva su RGB (alfa directo, como en `Rgba`), y una con `a = 128` sigue valiendo 128.
3. **Duplicados permitidos.** Nada se deduplica: `from_colors([c, c, c])` da tres entradas, y los índices 0, 1 y 2 son independientes (cambiar uno con `set` no afecta a los otros). El orden de entrada se conserva.
4. **Índice transparente.** Opcional, `None` por defecto. `set_transparent_index(Some(i))` exige `i < len`; si no, `Err(IndexOutOfRange)`. `set_transparent_index(None)` siempre funciona, incluso en una paleta vacía. Invariante: `transparent_index()` es `None` o `Some(i)` con `i < len`. Como no hay operación que quite entradas ni las reordene (ver «Fuera de alcance»), el invariante no puede romperse después.
5. **Color efectivo.** `get(i)` devuelve siempre lo guardado. `resolved(i)` devuelve lo guardado, salvo que `i` sea el índice transparente: entonces devuelve el mismo RGB con `a = 0`. Marcar un índice como transparente no modifica la entrada. Una entrada con `a = 0` es transparente aunque no sea el índice transparente: `resolved` no la cambia.
6. **Errores sin pánico.** Ninguna operación pública hace pánico, sea cual sea la entrada. `get`, `resolved` y `set` con `index >= len` devuelven `IndexOutOfRange { index, len }`, también en una paleta vacía.
7. **Atomicidad.** Si una operación devuelve error, la paleta queda exactamente igual que antes (incluido el índice transparente).
8. **Entrada infinita o enorme.** `from_colors` toma como mucho 257 elementos del iterador: con el 257.º ya sabe que devuelve error y no sigue leyendo. Un iterador infinito termina con `Err(TooManyColors)`.
9. **`push` y el índice.** El índice devuelto por `push` es la longitud anterior (el de la entrada nueva). `push` no cambia el índice transparente, y `set` tampoco.

## Fuera de alcance

- Quitar, insertar y reordenar entradas, y cualquier cambio que desplace índices: van con «reordenar la paleta sin alterar la imagen» en el hito 2, porque necesitan la imagen para remapear los píxeles.
- Conversiones RGB ↔ HSL y búsqueda del color más cercano (hito 2).
- Leer o escribir PNG (ver las notas siguientes).
- Formatos de archivo de paleta.

## Notas para la unidad 5 (PNG)

Acuerdos que la unidad 5 debe respetar; los detalles de PLTE y tRNS se documentan en su página.

- **Al leer** un PNG indexado no se fija `transparent_index`: se conserva el alfa de cada entrada tal como lo da tRNS (las entradas que tRNS no cubre son opacas, `a = 255`, según la especificación de PNG).
- **Al escribir** se usa `resolved` para cada entrada, de modo que el índice transparente sale como alfa 0 en tRNS.
- **Una paleta vacía no se puede escribir**: la escritura devuelve un error (PNG exige al menos una entrada en PLTE).

## Casos límite

Notación: `P[...]` es una paleta con esas entradas; `c0`, `c1`… son colores cualesquiera. Las filas agrupan casos parecidos.

| Operación | Resultado |
| --- | --- |
| `Palette::new()` y `Palette::default()` | `len() == 0`, `is_empty()`, `transparent_index() == None` |
| `from_colors` con 0, 1 y 256 colores | `Ok`, `len()` igual a la cantidad, `colors()` igual a la entrada |
| `from_colors` con 257 colores, y con `repeat(c)` (infinito) | `Err(TooManyColors)`; el infinito termina |
| `from_colors([c, c, c])`, y luego `set(0, c2)` | `colors() == [c, c, c]`; tras `set`, `[c2, c, c]` (el duplicado no se mueve) |
| `from_colors([Rgba(1,2,3,0), Rgba(1,2,3,128)])` | `get(0) == Rgba(1,2,3,0)`, `get(1) == Rgba(1,2,3,128)` (alfa y RGB intactos) |
| `P[c0, c1].get(1)`; paleta de 256, `get(255)` | `Ok(c1)`; `Ok` (último índice válido) |
| `P[c0, c1].get(2)` y `get(255)`; `Palette::new().get(0)` | `Err(IndexOutOfRange { index: 2, len: 2 })`, `{ index: 255, len: 2 }` y `{ index: 0, len: 0 }` |
| `Palette::new().push(c)`; `P[c0].push(c1)` | `Ok(0)`; `Ok(1)` con `colors() == [c0, c1]` |
| paleta de 255, `push(c)`; paleta de 256, `push(c)` | `Ok(255)` y `len() == 256`; `Err(TooManyColors)` con la paleta sin cambios |
| `P[c0, c1].set(1, c2)`; `set(2, c2)` | `Ok(())` y `colors() == [c0, c2]`; `Err(IndexOutOfRange { index: 2, len: 2 })` sin cambios |
| `P[c0, c1].set_transparent_index(Some(1))` | `Ok(())`, `transparent_index() == Some(1)` |
| `P[c0, c1].set_transparent_index(Some(2))`; `Palette::new().set_transparent_index(Some(0))` | `Err(IndexOutOfRange { index: 2, len: 2 })` y `{ index: 0, len: 0 }`; el índice transparente anterior no cambia |
| `Palette::new().set_transparent_index(None)`; con `Some(1)` fijado, `set_transparent_index(None)` | `Ok(())` en ambos; después `transparent_index() == None` y `resolved` coincide con `get` |
| `P[Rgba(9,8,7,255), c1]` con transparente `Some(0)` | `resolved(0) == Ok(Rgba(9,8,7,0))`, `get(0) == Ok(Rgba(9,8,7,255))`, `resolved(1) == Ok(c1)` |
| `P[Rgba(9,8,7,0)]` sin índice transparente; `resolved(i)` con `i >= len` | `resolved(0) == Ok(Rgba(9,8,7,0))`; `Err(IndexOutOfRange { index: i, len })` |
| con transparente `Some(0)`: `push(c)` y `set(0, c)` | el índice sigue siendo `Some(0)`; tras `set`, `resolved(0)` es `c` con `a = 0` |

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (no se añade ninguna dependencia: `proptest` ya está como dev-dependency de `px-core`)

## Tests

En `crates/px-core/tests/palette.rs` (tests de integración sobre la API pública, como en `Rgba`):

**Unitarios**: una prueba por fila de la tabla de casos límite (cada fila puede ser una o varias aserciones). Los casos de 256 y 257 colores se construyen con `(0..=255).map(...)` y `(0..=256).map(...)`; el del iterador infinito, con `std::iter::repeat`.

**Propiedades** (`proptest`):

1. **Construcción.** Para cualquier `Vec<Rgba>` de 0 a 256 elementos (generando también colores de un conjunto pequeño para forzar duplicados), `from_colors` da `Ok`, `colors()` es igual al vector y `get(i)` devuelve `v[i]` para todo `i`. Para 257 a 400 elementos, `Err(TooManyColors)`.
2. **`get` y `resolved` acotados.** Para cualquier paleta y cualquier `index: u8`: `get(index)` y `resolved(index)` son `Ok` si y solo si `index < len`; si no, `IndexOutOfRange { index, len }`. Nunca hay pánico.
3. **`push` hasta llenar.** Empujar `n` colores aleatorios (0 a 300) a una paleta vacía: los primeros `min(n, 256)` devuelven `Ok(k)` con `k` igual a la longitud previa; el resto, `Err(TooManyColors)`. Al final `len() == min(n, 256)`, y cada `push` fallido deja la paleta igual a una copia hecha antes.
4. **`set` local.** Para `index < len`: `get(index)` devuelve el color nuevo, el resto de entradas, `len()` y el índice transparente no cambian. Para `index >= len`: error y paleta igual a la copia previa.
5. **Índice transparente y `resolved`.** Para cualquier paleta e `index: u8`: `set_transparent_index(Some(index))` es `Ok` si y solo si `index < len`; si falla, el índice anterior se conserva; `None` siempre es `Ok`. Con un índice transparente `t`, para todo `i < len`: si `Some(i) == t`, `resolved(i)` es `get(i)` con `a = 0` y mismo RGB; en otro caso `resolved(i) == get(i)`.
6. **Secuencias aleatorias.** Dada una secuencia aleatoria de 0 a 60 operaciones (`push`, `set`, `set_transparent_index`) con índices `u8` cualesquiera, tras cada una: `len() <= 256`; `transparent_index()` es `None` o `Some(i)` con `i < len`; si la operación devolvió `Err`, la paleta es igual a la anterior; y las entradas que la operación no tocaba conservan su RGBA exacto. Ninguna hace pánico.

No hay dorado: no hay un comportamiento de programa de referencia que comparar; el límite de 256 entradas y el alfa por entrada vienen de la especificación pública de PNG (tablas PLTE y tRNS), que se citará en la página de la unidad 5.

## Decisiones

1. **Índices `u8`** (confirmado). Coinciden con el píxel de la imagen indexada (unidad 3); `len` sigue siendo `usize`.
2. **`get` devuelve lo guardado y `resolved` aplica el índice transparente** (confirmado). La paleta no pierde información y el compositor y el exportador PNG tienen una sola función que consultar.
3. **El índice transparente vive dentro de la paleta**, no en la imagen. Un cambio de paleta lleva consigo su transparencia y la imagen indexada solo guarda píxeles.
4. **Sin `remove`, `insert` ni reordenar** (confirmado; van al hito 2). Mientras tanto, el invariante del índice transparente es trivial.
5. **Paleta vacía válida** (confirmado). La imagen indexada (unidad 3) decidirá si exige al menos una entrada; el PNG no la puede escribir (notas para la unidad 5).
6. **Un solo error de capacidad** (`TooManyColors`, sin cifras): la cota es la constante `MAX_COLORS`, y con un iterador no se conoce el tamaño real, solo que excede 256.
7. **Sin dependencias nuevas.**
