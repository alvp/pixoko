# Hito 2 · Unidad 12: acceso por filas y bytes a `Image`

Estado: **aprobada** (2026-10-19; D1 a D8 como se proponían, con los matices del visto bueno, que **prevalecen sobre cualquier otra frase de la página**: ver «Matices del visto bueno»). Cambia `px-core` (**solo aditivo**: apruebas tocarlo para esta unidad, con las condiciones de tu encargo) y migra, cada una en su commit, seis funciones de `px-formats`, `px-ui` y `px-app`. No cambia ninguna prueba de las unidades anteriores. Sin `unsafe` ni dependencias nuevas; `docs/DEPENDENCIES.md` no cambia. Parte de [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md) («API», «PNG»), [`hito-1-image.md`](hito-1-image.md) (comportamientos 2, 3, 8 y 10) y [`hito-2-nueva-imagen.md`](hito-2-nueva-imagen.md) (D10); reparto en [`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md) (orden 9).

## Objetivo

Los recorridos píxel a píxel (`write_png`, `read_png`, `to_rgba8`, relleno de lienzos nuevos, `uses_entry`) pasan por `get_pixel` y `set_pixel`, que validan y construyen un `Pixel` por cada píxel. Esta unidad da a `Image` acceso de **solo lectura y cero copias** a sus bytes y filas, y constructores que validan una sola vez y devuelven error sin pánico. **Ninguna API devuelve `&mut` a los bytes ni deja construir una imagen sin revalidar**: los invariantes de `hito-1-image.md` (índice de píxel `< paleta.len()`, `bytes().len() == byte_len`) se conservan por construcción.

## Formato de los bytes (descripción exacta)

Es el almacenamiento de `hito-1-image.md`, comportamiento 10, ahora público y de solo lectura:

- **Orden.** Filas de arriba abajo; dentro de una fila, de izquierda a derecha. **Sin relleno entre filas**: `row_len() = width × bytes_per_pixel` es también el paso entre filas. El píxel `(x, y)` empieza en `(y × width + x) × bytes_per_pixel`.
- **Canales.** 8 bits cada uno; no hay orden de bytes que elegir.

| Modo | Bytes por píxel | Orden |
| --- | --- | --- |
| `Rgba` | 4 | `r, g, b, a`; alfa directo, no premultiplicado; con `a = 0` el RGB se conserva tal cual |
| `Indexed` | 1 | el índice; siempre `< palette().len()`; el índice transparente no se resuelve |
| `Gray` | 1 | `gray` (opaco) |
| `GrayAlpha` | 2 | `gray, alpha` |

## API (todo nuevo, `pub`)

Lectura (sin copias):

- `Mode::bytes_per_pixel(self) -> usize`: hoy privada; pasa a pública (4, 1, 1, 2).
- `Image::row_len(&self) -> usize`.
- `Image::bytes(&self) -> &[u8]`: la vista completa; su longitud es exactamente `Image::byte_len(width, height, mode)`. Sustituye a la `raw()` de crate, que desaparece (sus usos en `convert.rs` pasan a `bytes()`).
- `Image::row(&self, y: u32) -> Option<&[u8]>`: `None` si `y >= height`; si no, `row_len()` bytes. Nunca hace pánico, tampoco con `u32::MAX`.
- `Image::rows(&self) -> impl ExactSizeIterator<Item = &[u8]> + DoubleEndedIterator + Clone`: `height` filas, de arriba abajo.

Constructores (validan y devuelven `Result`, sin pánico):

- `Image::filled(width, height, pixel: Pixel) -> Result<Image, ImageError>`: para `Rgba`, `Gray` y `GrayAlpha`. Con `Pixel::Indexed(_)` da `PaletteRequired` (como `new`). El píxel se guarda tal cual, sin normalizar.
- `Image::filled_indexed(width, height, palette: Palette, index: u8) -> Result<Image, ImageError>`: `EmptyPalette` si la paleta está vacía; `IndexNotInPalette { index, len }` si `index >= len`.
- `Image::try_clone(&self) -> Result<Image, ImageError>`: `TooLarge { width, height }` si no se pudo reservar (`try_reserve_exact`); es `clone` sin abortar. La paleta (≤ 1 KiB) se clona con `Clone`.
- `Image::from_bytes(width, height, mode, data: Vec<u8>) -> Result<Image, ImageError>` e `Image::from_bytes_indexed(width, height, palette, data)`: toman el `Vec` **sin copiarlo**. Con `Mode::Indexed`, `from_bytes` da `PaletteRequired`.
- `ImageError::WrongLength { expected: usize, found: usize }`: variante nueva, `data.len() != byte_len`. Con un `Err` el `Vec` se descarta (se consumió).
- `Image::uses_index(&self, index: u8) -> bool`: algún píxel vale `index`; `false` si la imagen no es indexada.

**Orden de comprobación, fijo** (visto bueno de D5):

- `from_bytes` y `from_bytes_indexed`: 1) tamaño cero o excesivo (`ZeroSize`, luego `TooLarge`; **antes** de mirar `data`); 2) `PaletteRequired`, solo en `from_bytes` con `Mode::Indexed` (es el modo, no los bytes, como en `new`); 3) longitud (`WrongLength { expected, found }`); 4) paleta vacía (`EmptyPalette`, solo en `from_bytes_indexed`); 5) índices (`IndexNotInPalette { index, len }`, con el **mayor** índice encontrado: determinista, sin posición y una pasada que el compilador vectoriza).
- `filled` y `filled_indexed`: tamaño cero o excesivo; `PaletteRequired` (solo `filled` con `Pixel::Indexed`); `EmptyPalette` (solo `filled_indexed`); `IndexNotInPalette`. No hay longitud.

**Documentación de los constructores con `Vec`** (`from_bytes`, `from_bytes_indexed`): dice que el `Vec` lo recibe la imagen tal cual y que **quien llama debe haberlo reservado con `try_reserve`** (o `try_reserve_exact`), no con `vec![..]` ni `resize` a ciegas: una reserva que aborta el proceso no se puede convertir en `Err`. La documentación pública de `bytes`, `row`, `rows` y `row_len` lleva la **tabla del formato por modo** de arriba.

## Comportamiento

1. **Solo lectura.** `bytes`, `row` y `rows` prestan `&[u8]` de `&self`: mientras existen, la imagen no se puede modificar ni se puede mover fuera de ellos. No hay `bytes_mut`, `row_mut` ni `into_bytes`.
2. **Sin escritura por filas** (D2). Para escribir se sigue usando `set_pixel` (y los comandos del documento) o se construye con `from_bytes`.
3. **Equivalencia con `get_pixel`.** Para todo `(x, y)` dentro, los `bytes_per_pixel` bytes de `bytes()` desde `(y × width + x) × bytes_per_pixel` son la codificación del `Pixel` de `get_pixel(x, y)` según la tabla; `row(y)` es el trozo de `bytes()` de esa fila y `rows()` es la secuencia de `row(0)`, `row(1)`…
4. **`filled`, `filled_indexed` y `try_clone` son idénticos a la construcción lenta**: `filled` es `==` a `new` (o `new_indexed`) más `set_pixel` en cada píxel; `try_clone` es `==` a `clone()` y no comparte memoria con el original.
5. **`from_bytes` no relaja ningún invariante.** Si devuelve `Ok`, la longitud es `byte_len` y, en indexado, todo píxel es `< paleta.len()`. Un índice fuera de la paleta nunca produce una imagen.
6. **Aditivo.** Ninguna firma ni comportamiento existente cambia. `ImageError` no es `#[non_exhaustive]`: una variante nueva rompería un `match` exhaustivo ajeno; en este repositorio no hay ninguno (comprobado: todos los usos construyen o comparan, ninguno hace `match`).

## Medidas (release, Linux, 12 hilos; mejor de 3; milisegundos, antes → después)

Banco desechable en una copia del repositorio fuera de él (`~/.cache/pixoko-scratch`, borrada al terminar). **«Después» es de un prototipo** de esta spec en esa copia, con las seis migraciones aplicadas, no del código final: se remide al cerrar la unidad con el código real. Imagen de prueba: bloques de 32 colores (PNG de ~30 a ~500 KiB, así que el coste del deflate es **bajo**; con una foto el `write_png` cambia menos en proporción). `get_pixel` y `rows()` no hacen lo mismo: el segundo solo suma el primer byte de cada píxel. `write_png` y `read_png` se midieron en dos pasadas a 4096² y en una a 8192² (el resto, mejor de tres).

**8192²** (el lienzo mayor de «Nueva imagen» y de la puerta):

| Operación | RGBA | Indexado | Gris | Gris+alfa |
| --- | --- | --- | --- | --- |
| Recorrer todo: `get_pixel` → `rows()` | 152 → 14 | 138 → 12 | 139 → 12 | 140 → 12 |
| Rellenar con blanco: `new` + `set_pixel` → `filled` | 436 → 108 | 206 → 21 | 198 → 23 | 307 → 55 |
| Clonar: `clone` → `try_clone` | 108 → 110 | 17 → 23 | 19 → 23 | 55 → 55 |
| `to_rgba8` | 584 → 107 | 548 → 116 | 535 → 122 | 567 → 133 |
| `to_rgba8_with_entry` | — | 238 → 116 | — | — |
| `uses_entry` con la entrada sin usar → `uses_index` | — | 138 → 2,7 | — | — |
| `write_png` | 904 → 337 | 601 → 89 | 596 → 83 | 731 → 172 |
| `read_png` | 910 → 267 | 366 → 99 | 354 → 77 | 619 → 156 |
| (suelo) `new` a ceros | 103 | 16 | 19 | 52 |

**4096²:**

| Operación | RGBA | Indexado | Gris | Gris+alfa |
| --- | --- | --- | --- | --- |
| Recorrer todo: `get_pixel` → `rows()` | 38 → 3,2 | 35 → 3,1 | 35 → 3,1 | 35 → 3,1 |
| Rellenar con blanco: `new` + `set_pixel` → `filled` | 95 → 19 | 48 → 0,1 | 44 → 0,1 | 65 → 2,5 |
| Clonar: `clone` → `try_clone` | 20 → 20 | 0,4 → 0,4 | 0,4 → 0,4 | 2,8 → 3,0 |
| `to_rgba8` | 140 → 22 | 120 → 10 | 119 → 14 | 127 → 24 |
| `to_rgba8_with_entry` | — | 40 → 10 | — | — |
| `uses_entry` con la entrada sin usar → `uses_index` | — | 35 → 0,7 | — | — |
| `write_png` | 222 → 85 | 147 → 24 | 146 → 22 | 173 → 45 |
| `read_png` | 220 → 67 | 78 → 14 | 76 → 9,0 | 126 → 28 |
| (suelo) `new` a ceros | 18 | 0,1 | 0,1 | 2,0 |

`index_counts` (histograma de 256 contadores, con cuatro subhistogramas para que las tiradas de un mismo índice no se serialicen): 4,3 ms a 4096² y 17 ms a 8192², indexado.

**`convert_image`** (8192², antes → prototipo con la pasada 1 omitida si el destino es RGBA y con el histograma si el origen es indexado; la función ya trabajaba sobre bytes por dentro y **no usa `get_pixel`**, así que la API pública no la cambia):

| Conversión | Antes | Prototipo |
| --- | --- | --- |
| RGBA → Gris / Gris+alfa / Indexado | 339 / 327 / 604 | 331 / 314 / 596 |
| Indexado → RGBA / Gris / Gris+alfa | 326 / 417 / 356 | 229 / 274 / 225 |
| Gris → RGBA; Gris+alfa → RGBA | 287; 320 | 212; 224 |

### Lo que dicen las cifras

- **Lo que se gana de verdad** es el coste por píxel de `get_pixel`: `to_rgba8` (~5×), `write_png` y `read_png` (2,7 a 7×) y `uses_entry` (~50×). `write_png` además **deja de copiar la imagen entera** (a 8192² RGBA, 256 MiB menos de pico; hasta 1 GiB en el límite): `bytes()` es ya lo que pide el codificador.
- **`filled` queda en el suelo de `new`**: a 8192² RGBA son ~108 ms frente a ~103 ms de `new` a ceros, porque lo que cuesta es tocar la memoria por primera vez. «Nueva imagen» a 8192² RGBA con fondo blanco pasa de ≈ 0,9 s (dos imágenes, notas de la 14) a ≈ 0,22 s (2 × 108 ms): no baja más sin cambiar el reparto de memoria.
- **`try_clone` no es más rápido que `clone`**: se añade por **robustez** (`clone` aborta el proceso si falta memoria; a 16384² RGBA son 1 GiB), no por velocidad.
- **`convert_image` mejora a lo sumo un 35 % (≤ 0,14 s a 8192²)** y no por esta API. No se migra (D7).
- **El editor de color**: `to_rgba8_with_entry` baja ~2× en CPU, pero no toca la subida a la GPU de la textura entera; la lentitud que mediste a 6000² puede quedarse en parte (es de la 14a).

## Casos (12 filas)

Notación: `M` es cualquier modo no indexado; `P[…]` es una `Palette`; «los cuatro modos» incluye indexado con una paleta de 3 colores.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | Imagen 3 × 2 en los cuatro modos | `bytes().len() == byte_len(3, 2, modo)`; `row_len() == 3 × bytes_per_pixel` (4, 1, 1, 2); tras `set_pixel(1, 0, ·)` con `Rgba(1,2,3,4)`, `Indexed(1)`, `Gray(7)` y `GrayAlpha { gray: 7, alpha: 9 }`, los bytes del píxel son `[1,2,3,4]`, `[1]`, `[7]` y `[7,9]` en el offset `(0 × 3 + 1) × bpp`; `Rgba(1,2,3,0)` conserva `[1,2,3,0]` |
| 2 | `row(y)` en una imagen 3 × 2 | `Some` con `row_len` bytes en `y = 0` y `y = 1` (el contenido de esa fila); `None` en `y = 2` y `y = u32::MAX` |
| 3 | `rows()` en una imagen 3 × 2 | `len() == 2`; cada fila `== row(y)`; `.rev()` empieza por la última; la concatenación `== bytes()` |
| 4 | Formas límite 1×1, 1×5 y 5×1 en los cuatro modos | `rows().len() == height`; `row_len() == width × bpp`; `bytes().len() == byte_len` |
| 5 | `filled(3, 2, p)` con `Rgba(c)`, `Rgba(1,2,3,0)`, `Gray(255)`, `GrayAlpha { gray: 255, alpha: 0 }` | `Ok`; todos los `get_pixel` son `p` (el RGB con `a = 0` intacto); `==` a `new` más `set_pixel` en cada píxel |
| 6 | `filled(0, 5, p)`; `filled(u32::MAX, u32::MAX, p)`; `filled(16385, 16384, Rgba)`; `filled(2, 2, Indexed(0))`; `filled(0, 2, Indexed(0))`; `filled(u32::MAX, u32::MAX, Indexed(0))` | `ZeroSize`; `TooLarge`; `TooLarge` (sin reservar); `PaletteRequired`; `ZeroSize`; `TooLarge` (las dimensiones ganan al modo) |
| 7 | `filled_indexed(3, 2, P[c0,c1,c2], 2)`; con paleta de 256 y `255`; `P[c0,c1,c2]` con `3`; `Palette::new()` con `0`; `(0, 2, P[c0], 5)` | `Ok`, todos `Indexed(2)`, paleta igual, `==` a la lenta; `Ok`; `IndexNotInPalette { index: 3, len: 3 }`; `EmptyPalette`; `ZeroSize` (dimensiones, luego paleta, luego índice) |
| 8 | `try_clone` en los cuatro modos, también 1×1 | `Ok(c)` con `c == original` y `c == clone()`; `set_pixel` en `c` no cambia el original; la paleta es igual |
| 9 | `from_bytes(w, h, M, original.bytes().to_vec())` y `from_bytes_indexed(w, h, paleta, …)` en los cuatro modos | `Ok(i)` con `i == original` (incluido el alfa 0 con su RGB) |
| 10 | `from_bytes` con `data` de `len − 1`, `len + 1` y vacío; `(0, 5, M, …)`; `(16385, 16384, Rgba, vec![])`; `(16384, 16384, Rgba, vec![])`; `(2, 2, Indexed, …)` | `WrongLength { expected, found }` (los tres); `ZeroSize`; `TooLarge` (antes que `WrongLength`, sin mirar `data`); `WrongLength { expected: MAX_BYTES, found: 0 }` (el límite exacto cabe en `byte_len` y se rechaza por longitud, sin reservar 1 GiB); `PaletteRequired` |
| 11 | `from_bytes_indexed` con `P[c0,c1]` y datos `[0,1,0,1]` (2 × 2); `[0,1,2,1]`; `[255,3,0,1]`; paleta de 256 con 256 valores (16 × 16); `Palette::new()` con la longitud correcta; `Palette::new()` con la longitud incorrecta | `Ok`; `IndexNotInPalette { index: 2, len: 2 }`; `IndexNotInPalette { index: 255, len: 2 }` (el mayor); `Ok`; `EmptyPalette`; `WrongLength` (la longitud gana a la paleta vacía) |
| 12 | `uses_index` en indexado 3 × 2 con índices `{0, 1}` | `true` para 0 y 1; `false` para 2 y para `255` (fuera de paleta); `false` para cualquier índice en una imagen de cada `M` |

## Propiedades (4; `proptest`, 256 casos cada una; se puede bajar con `PROPTEST_CASES` al desarrollar, no al cerrar)

1. **Bytes y filas frente a la construcción por `set_pixel`, en los cuatro modos.** Para cualquier imagen de 1 a 16 de lado en los cuatro modos (indexado con paleta de 1 a 256 entradas) construida con `new` más `set_pixel` desde un modelo `Vec<Pixel>` (con repeticiones): `bytes().len() == byte_len`; los bytes esperados se calculan **del modelo** con la tabla del formato (no con `get_pixel`) y son iguales a `bytes()`; `row(y)` es su trozo de fila y `rows()` concatenado es `bytes()`; además `get_pixel` devuelve el `Pixel` del modelo.
2. **Construcción rápida igual a la lenta.** Para dimensiones de 1 a 16, un modo y un `Pixel` válido: `filled` (o `filled_indexed`) `==` `new` más `set_pixel` en cada píxel; y tras cualquier serie de `set_pixel` sobre una imagen: `try_clone() == clone()` y `from_bytes(.., bytes().to_vec()) == original`.
3. **`from_bytes` con datos arbitrarios.** Para dimensiones de 1 a 8, modo y un `Vec<u8>` arbitrario de longitud `len + d` con `d` en `−2..=2`: es `Ok` si y solo si `d == 0` y, en indexado, el máximo `< paleta.len()`; si no, el error es el que da el orden de comprobación (`WrongLength` antes que `IndexNotInPalette`, con el mayor índice). Si es `Ok`: `bytes() == data`, todo `get_pixel` es `Ok` y, en indexado, todo `Indexed(i)` cumple `i < paleta.len()`. Nunca hay pánico.
4. **`uses_index`.** Para cualquier imagen del caso 1 y cualquier `index: u8`: `uses_index(index)` es verdadero si y solo si algún `get_pixel(x, y)` es `Indexed(index)`; en una imagen no indexada es siempre falso.

Todas las pruebas son sin ventana. Van en un **archivo nuevo**, `crates/px-core/tests/image_bytes.rs`, para no modificar ninguna prueba existente (un enlazado más, y es poco). Los tests de los cuatro modos usan imágenes pequeñas; el límite exacto de `MAX_BYTES` se prueba por el lado de `byte_len` (fila 10), sin reservar.

## Migraciones (cada una en su propio commit, tras el de la lógica)

Cada una pasa con **las pruebas existentes sin cambiarlas**, y **en cada commit de migración pasa el workspace entero** (`cargo test --workspace --locked`; el prototipo no ejecutó `px-app`). Si alguna prueba existente necesitara más que no cambiar, me paro y aviso. Las pruebas nuevas de una migración (oráculos) van en su commit, escritas y ejecutadas antes con la implementación antigua para comprobar que el oráculo vale.

| # | Dónde | Qué cambia | Gana (8192², RGBA o indexado) |
| --- | --- | --- | --- |
| M1 | `write_png` (`px-formats`) | `write_image_data(image.bytes())`: sin copiar la imagen; **salida idéntica byte a byte**: se comprueba contra el corpus (`golden/png`) y con una propiedad que usa, **dentro de la prueba**, una copia de la implementación antigua como oráculo (nada de eso en `src/`) | 904 → 337 y 601 → 89 ms; −256 MiB de pico |
| M2 | `to_rgba8` (`px-ui`) | RGBA: copia; gris y gris+alfa: expansión por bytes; indexado: tabla de 256 entradas; **salida idéntica**, con la implementación antigua copiada en la prueba como oráculo | 584 → 107 y 548 → 116 ms |
| M3 | `to_rgba8_with_entry` (`px-ui`) | la misma tabla con la entrada sustituida, sobre `bytes()`; **salida idéntica**, con la antigua como oráculo en la prueba | 238 → 116 ms |
| M4 | `build_image` de `read_png` (`px-formats`) | **sigue reservando `raw` con `try_reserve_exact` antes de decodificar**. Con 8 bits y el diseño de `Image` (RGBA, gris, gris+alfa e indexado de 8 bits), `raw` **es ya** el vector de la imagen: se pasa a `from_bytes` o `from_bytes_indexed` **sin copias** (el pico baja de `raw` + imagen a `raw`). Si no (16 bits, menos de 8 bits, RGB, clave de color), se construye un `Vec` reservado con `try_reserve_exact` y se pasa igual. `PaletteIndexOutOfRange` conserva su valor (el primer índice fuera, en orden de lectura). **El pico de memoria no sube; se mide antes y después** | 910 → 267 y 366 → 99 ms |
| M5 | `uses_entry` (`px-ui`) | `image.uses_index(index)` | 138 → 2,7 ms |
| M6 | `SystemAlloc::image` (`px-app`) | `filled` o `filled_indexed`; el `trait ImageAlloc` y `build_new_images` **no cambian** | blanco en 8192² RGBA: ≈ 0,9 → ≈ 0,22 s |

**No se migran:** `convert_image` (no usa `get_pixel`; ≤ 35 %); `Canvas::set_image`, que clona sin canal de error (convertirlo en `try_clone` cambia su firma y sus llamadas; es de la 14a); `px-raster` (no autorizado y sin medida que lo pida); el instantáneo de tramos de `Document::apply` (sin medida que lo pida); `demo.rs`.

## Matices del visto bueno (2026-10-19)

- **D1.** La documentación pública lleva la tabla del formato por modo; una propiedad (la 1) compara `bytes()` y `row()` con la construcción por `set_pixel` en los cuatro modos; `raw()` (de crate) desaparece.
- **D5.** `WrongLength` lleva la longitud esperada y la recibida. El orden de errores es el fijo de «API». La documentación de los constructores con `Vec` dice que quien llama debe haberlo reservado con `try_reserve`.
- **Migraciones.** Una por commit. `write_png`: salida idéntica byte a byte (corpus y oráculo de propiedades dentro de la prueba). `read_png`: ver M4. `to_rgba8` y `to_rgba8_with_entry`: salida idéntica con la antigua como oráculo. Workspace entero en cada commit.
- **Notas.** La nota «Para el hito 2» de [`hito-1-png.md`](hito-1-png.md) (pico al escribir) se actualiza con las cifras medidas tras la migración. `screen_to_image_i32` y `TitleInfo.tool` quedan asignadas a una unidad corta de limpieza junto a la 14a ([`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md)).
- **Cierre.** Se remiden las cifras de antes y después con el código final y se ponen en esta página.

## Fuera de alcance

Escritura por filas (D2); `index_counts` (D6); `convert_report` (candidato de la 20 en las notas: sigue anotado; con un origen indexado se apoyaría en `index_counts`, D6); el escritor de PNG por flujo y la lectura sin reserva anticipada (unidad 13); las dos limpiezas de API de las notas (`screen_to_image_i32` y `TitleInfo.tool`), que **cambian pruebas anteriores**: unidad corta de limpieza junto a la 14a (ver el reparto); la subida a la GPU y la textura por regiones (14a).

## Criterios de aceptación

`cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --locked` y `cargo deny check`; el `license-auditor` una vez sobre el rango de la unidad; y la tabla de medidas rehecha con el código final (si una cifra de «después» sale peor que la del prototipo en más de un 25 %, lo digo en el informe).

## Decisiones que necesito que confirmes

Las recomendaciones van marcadas; el resto son consecuencia directa.

1. **? D1. Nombres y forma de la lectura.** `bytes()`, `row(y)`, `rows()`, `row_len()` y `Mode::bytes_per_pixel` pública. `rows()` devuelve `impl ExactSizeIterator + DoubleEndedIterator + Clone` (no un tipo con nombre, para no fijar la implementación). `raw()` de crate se elimina en favor de `bytes()`. *Recomendado.*
2. **? D2. No se añade escritura por filas.** Si se quisiera, sería `Image::set_row(y, &[u8]) -> Result<(), ImageError>`: comprueba `y` (`OutOfBounds`), la longitud (`WrongLength`) y, en indexado, cada byte contra la paleta (`IndexNotInPalette`, el mayor) **antes de escribir** (atómico, como `set_pixel`). Frente a `set_pixel` ahorraría la validación y el `Pixel` por píxel, pero **ningún consumidor la necesita**: `read_png` construye una imagen entera (`from_bytes`) y los comandos del documento ya escriben por tramos dentro de `px-core`. Con una API sin uso habría que mantenerla y probarla. *Recomendado: no añadirla; se pide cuando haya un consumidor.*
3. **? D3. `filled` y `filled_indexed`, dos funciones**, como `new` y `new_indexed` (el indexado necesita una paleta; con una sola función habría `Option<Palette>` y un pánico o error confuso con un `Pixel` que no corresponde). *Recomendado.*
4. **? D4. `try_clone` se añade sin migrar a nadie** (la ganancia es robustez, no velocidad, y `Canvas::set_image` no tiene dónde devolver el error). Alternativa: cambiar `Canvas::set_image` ahora para que devuelva `Result`: toca la 14a y pruebas anteriores. *Recomendado: dejarlo para la 14a.*
5. **? D5. `from_bytes` y `from_bytes_indexed` con `Vec<u8>` por valor, una variante nueva `ImageError::WrongLength` y el **mayor** índice en `IndexNotInPalette`.** Alternativas: `&[u8]` (copia, más lento y con más memoria), o reutilizar `TooLarge` para la longitud (engaña: el tamaño cabe). `ImageError` no es `non_exhaustive` (ver comportamiento 6). *Recomendado.*
6. **? D6. `uses_index` sí; `index_counts` no.** `uses_index` tiene consumidor (`uses_entry`, 138 → 2,7 ms). El histograma (17 ms a 8192²) solo lo usaría el informe de `convert_image` (mejora ≤ 35 % y de dentro de `px-core`, que ya ve los bytes) o un futuro `convert_report`. Si prefieres tenerlo ya, es `Image::index_counts(&self) -> Option<[usize; 256]>` (`Some` solo en indexado; la suma es `width × height`) y añade una fila y una propiedad a esta spec (hoy 12 y 4: el máximo). *Recomendado: dejarlo para `convert_report`.*
7. **? D7. Las migraciones son M1 a M6 y no otras.** En particular **`convert_image` no se toca**, aunque lo nombraste: las cifras dicen que ya iba por bytes y que lo que mejora (omitir la pasada 1 con destino RGBA; histograma con origen indexado) es una optimización interna de ≤ 0,14 s, no un uso de esta API. *Recomendado.* Si la quieres, es un M7 con `index_counts` (D6).
8. **? D8. Pruebas en un archivo nuevo** (`image_bytes.rs`) en vez de un módulo de `image.rs`: así ninguna prueba existente cambia ni en una línea. Coste: un ejecutable de pruebas más.
9. **D9. Medidas.** «Después» es de un prototipo en una copia; se remiden con el código final. Una incertidumbre que no he resuelto: el ruido de la máquina (una sola, con 12 hilos) y que la imagen de prueba comprime muy bien.
