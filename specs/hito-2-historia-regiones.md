# Hito 2 · Unidad 4: historia por regiones y `FillSpans`

Estado: **aprobada** (2026-10-17; decisiones 1 a 5 tal cual y la 6 corregida: el paso es una secuencia ordenada de partes, con undo al revés y redo hacia delante; filas 16 y 17 añadidas; notas para el hito 4 al final)

## Objetivo

Que un comando masivo (el relleno de la unidad 5) cueste en la historia lo que ocupan sus **tramos**, no sus píxeles. Se añade a `px-core` el comando `FillSpans` (una lista de tramos y un `Pixel`) y un formato de paso nuevo para él. La unidad 5 calculará la región por inundación y la aplicará con este comando. `apply`, `undo` y `redo` no cambian de firma ni de semántica.

## Entradas y salidas

- `px_core::Span { y: u32, x_start: u32, x_end: u32 }`: los píxeles `x_start..x_end` (fin **excluido**) de la fila `y`. Campos públicos; `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`.
- `Command::FillSpans { image: ImageId, spans: Vec<Span>, pixel: Pixel }`: pone `pixel` en todos los píxeles de los tramos. Variante nueva (`Command` ya es `#[non_exhaustive]`).
- `DocumentError` (también `#[non_exhaustive]`) gana dos variantes:
  - `Span { span: usize, source: ImageError }`: el tramo número `span` se sale del lienzo; `source` es `OutOfBounds` y da el **primer píxel fuera**: `(x_start, y)` si `y >= height`, y si no `(max(x_start, width), y)`.
  - `BadSpans { span: usize }`: el tramo número `span` está vacío (`x_start >= x_end`) o no va estrictamente después del anterior (desordenado o solapado).
- Un `Pixel` de otro modo o con índice fuera de la paleta da `Pixel { pixel: 0, source }` (el relleno tiene un solo píxel, el 0), como en la línea.
- Sin cambios en `Document`, `Image`, `ImageId`, `PixelEdit` ni `SetPixels`. Solo se añaden funciones `pub(crate)` a `Image` si la implementación las necesita (leer o escribir un tramo sin comprobar cada píxel).

## Comportamiento

1. **Qué guarda el paso: tramos con el valor anterior (elegido), no rectángulos.** Un paso de relleno guarda, para cada `Span`, los **trozos** de píxeles que cambian: `Run { y, x, len, before }`, con `before` un solo valor. Un trozo es una serie máxima de píxeles contiguos de un tramo, todos con el mismo valor anterior y todos distintos de `pixel`; los píxeles que ya valían `pixel` no se guardan, igual que en el hito 1 (solo cambios). Un trozo nunca cruza de un tramo a otro. El valor «después» es el mismo para todo el paso, así que se guarda **una vez** (`after`). El coste de un trozo es `size_of::<Run>()`, que se calcula con `size_of` y no con una cifra escrita a mano (hoy son 20 bytes: `y`, `x`, `len` en `u32` y un `Pixel` de 5, con alineación a 4; las cifras de la tabla son con ese tamaño y no son API).

   | | `SetPixels` (hoy) | `FillSpans` con trozos | Rectángulos con los valores anteriores |
   | --- | --- | --- | --- |
   | Coste | 24 B por píxel (`size_of::<Change>()`) | 20 B por **trozo** (`size_of::<Run>()`) | 1 a 4 B por píxel del rectángulo (según el modo) |
   | **Mejor caso**: relleno compacto sobre fondo uniforme (el relleno por inundación normal) | 100 × 100: 240 KB; lienzo de 16384 × 16384: 6 GiB | 100 × 100: 100 trozos = 2 KB; 16384 × 16384: 16 384 trozos = 320 KB. Independiente del ancho | 100 × 100 RGBA: 40 KB; 16384 × 16384 RGBA: 1 GiB por paso |
   | **Peor caso**: regiones dispersas, o fondo que cambia a cada píxel (un trozo por píxel) | 24 B por píxel | 20 B por píxel (nunca peor que `SetPixels`; solo un 17 % mejor) | El rectángulo envolvente, no la región: una diagonal de N píxeles cuesta N² · bpp |
   | Peor caso para este formato | | Relleno compacto sobre contenido ruidoso: 20 B frente a 4 B por píxel RGBA (5 veces más que un rectángulo, pero nunca más que `SetPixels`) | |

   Se elige por **trozos** porque el caso real (rellenar una zona del mismo color) es el mejor caso suyo y el peor de los rectángulos (que además crecen con el área, y un solo paso mayor que el límite se conserva: un relleno de lienzo entero se comería 1 GiB). Su peor caso es acotado y mejor que el actual. La contrapartida (contenido ruidoso) se acepta; si el relleno con tolerancia del hito 4 la hiciera frecuente, el tipo `Step` es privado y se puede añadir un paso por rectángulos sin tocar la API.
2. **Validación: error, sin normalizar (elegido).** Los tramos deben venir **estrictamente ordenados por `(y, x_start)`, sin solapes y sin tramos vacíos**; dos tramos pegados (`x_end == x_start` siguiente en la misma fila) son válidos. Orden de comprobación: `EmptyCommand` (lista vacía), `UnknownImage`, el `Pixel` (`Pixel { pixel: 0, .. }`) y después cada tramo en orden: primero `BadSpans`, luego `Span` (fuera del lienzo); gana el primer error. Todo esto es una pasada de **solo lectura** antes de escribir. Normalizar (ordenar, fusionar y recortar) exigiría reservar memoria y escondería errores del productor; el relleno por inundación ya produce los tramos en orden de líneas de barrido, así que la comprobación cuesta O(n) sin memoria.
3. **Sin cambios: `Ok(false)`.** Si todos los píxeles de los tramos ya valen `pixel`, no hay trozos: `apply` devuelve `Ok(false)` y no hay paso, ni se borra la pila de rehacer, ni cambia `history_bytes` (regla 9 del hito 1).
4. **Dentro de un `Batch`.** `FillSpans` es un comando hoja más: se mezcla con `SetPixels` y se ejecuta en orden como un solo paso. Los errores salen como `InBatch { index, source }` (el `Span`, `BadSpans` y los demás); `FillSpans` vacío de tramos es `EmptyCommand`; un `Batch` dentro de otro sigue siendo `NestedBatch`. Un `Batch` crea paso si cualquiera de sus comandos cambió algo. Un relleno que no cambia nada dentro de un `Batch` no deja trozos. El resultado de un relleno depende de los comandos anteriores del `Batch` (se lee el estado actual): `Batch([Fill(c1), Fill(c1)])` cambia solo con el primero; `Batch([Fill(c1), Fill(c0)])` sobre `c0` cambia y restaura, y cuenta como cambio (`Ok(true)`).
5. **El paso es una secuencia ordenada de partes.** Un paso guarda, en el orden en que se ejecutaron los comandos, partes de dos clases: **píxeles sueltos** (los `Change` de un `SetPixels`, como hoy) y **tramos** (los trozos y el `after` de un `FillSpans`). `undo` recorre la secuencia **al revés** (cada parte, y dentro de ella sus elementos, en orden inverso) escribiendo `before`; `redo` la recorre **hacia delante** escribiendo `after`. Así `Batch([S, Fill, S])` o `Batch([Fill, S])` con píxeles solapados se deshace y se rehace exactamente, sea cual sea el orden. Una lista de rellenos independiente de los píxeles sueltos perdería ese orden: por eso cada relleno guarda su **posición en la secuencia**.
   - Representación (privada, para que el coste de un paso solo de `SetPixels` siga siendo idéntico): el `Vec<Change>` actual, con todos los píxeles sueltos en orden, más un `Vec<FillPart>` de rellenos; cada `FillPart { at, image, after, runs }` dice que va **tras** los primeros `at` píxeles sueltos. La secuencia es `changes[..at₀]`, `fill₀`, `changes[at₀..at₁]`, `fill₁`, … , resto. Sin rellenos, el `Vec<FillPart>` está vacío y no reserva.
   - Reversión: `undo` recorre al revés: resto de `changes`, `fill_n`, `changes[at_{n-1}..at_n]`, … Los `at` son no decrecientes (dos rellenos seguidos tienen el mismo `at`).
6. **Coste real en el límite.** `Step::cost` pasa a ser `STEP_BASE + changes.capacity() · size_of::<Change>() + fills.capacity() · size_of::<FillPart>() + Σ runs.capacity() · size_of::<Run>()`, con la misma regla de saturación, donde `STEP_BASE` es `size_of::<Vec<Change>>()` (lo que hoy es `size_of::<Step>()`). Así **un paso solo de `SetPixels` cuesta exactamente lo mismo que antes**, aunque `Step` gane un campo. El límite, `trim` y `history_bytes` no cambian: se descarta lo más antiguo mientras `history_bytes() > límite`, **nunca el último paso**, y un paso mayor que el límite se conserva. **`history_bytes()` es el coste de la pila de deshacer, y solo de ella** (comportamiento 7 y decisión 8 del hito 1; la de rehacer no cuenta): sube con cada `apply` que cambia algo por el coste de su paso, **baja tras `undo`** en el coste del paso que sale de la pila y **sube tras `redo`** en el mismo coste, porque el paso se mueve entre pilas sin cambiar de coste. Tras `apply`, `undo` y `redo` vale lo mismo que tras `apply`; tras el `undo` vale lo que valía antes del `apply` (0 si era el único paso). Un `apply` sin cambios, un `apply` con error y un `undo` o `redo` fallidos no lo modifican.
7. **Memoria: reservar antes de escribir, y la reversión no reserva.** `apply` sigue reservando antes de tocar nada el hueco en la pila de deshacer, el `Vec<Change>` (con la cota de las escrituras de los `SetPixels`, como hoy) y la lista de rellenos (cota: número de `FillSpans` del comando). La reserva se hace **por comando, justo antes de modificarlo**, y la reversión usa **solo registros ya reservados**. Los trozos de un relleno **no tienen cota barata** (un lienzo de 2²⁸ píxeles con un trozo por píxel pediría 5 GiB, justo lo que se quiere evitar) y su número depende del estado en ese momento. Por eso cada `FillSpans` hace, **antes de escribir su primer píxel**: (a) la pasada de validación de solo lectura (comportamiento 2), que además **cuenta los trozos exactos**; (b) `try_reserve_exact(trozos)`; (c) la pasada de escritura, que usa la misma partición y solo hace `push` dentro de la capacidad (sin crecer). Si (b) falla: `OutOfMemory` y se revierten los comandos anteriores del `Batch`, si los hay. Para un `FillSpans` suelto, o el primero de un `Batch`, esto es literalmente «reservar antes de modificar nada». La reversión no puede fallar: solo escribe valores guardados (`Change` y trozos, que están ya en memoria porque cada trozo se registra antes de escribir sus píxeles) y no reserva. Sin `unwrap`, `expect` ni índices que puedan hacer pánico. `undo` y `redo` reservan el hueco de la pila de destino antes de tocar un píxel, como hoy.
8. **Sin pánicos.** Con cualquier `u32` en `y`, `x_start`, `x_end`, tramos desordenados o gigantes, cualquier `ImageId` y cualquier `Pixel`. No hay `unsafe`. La aritmética de longitudes usa `u32` restando solo tras comprobar `x_start < x_end`, y los contadores `usize` con `checked_add` o saturando.

## Compatibilidad con el hito 1

- Firmas y semántica de `apply`, `undo`, `redo`, `can_undo`, `can_redo`, `history_bytes`, `history_limit` y `set_history_limit`: sin cambios, incluidas atomicidad, límite en bytes que nunca descarta el último paso y paso mayor que el límite conservado.
- `crates/px-core/tests/document.rs` **debe pasar sin modificarse**. Lo he comprobado leyéndolo: ningún test fija el coste en bytes (solo lo mide con `coste_de_un_paso()` y compara entre `apply`), y sus `match` sobre `Command` tienen rama `otro` (el modelo nunca genera `FillSpans`, así que no se alcanza). **No hay ninguna prueba del hito 1 que deba cambiar.** Si al implementar alguna fallara, me paro y te lo digo antes de tocarla.
- `px-raster` (`draw.rs`) y `px-app` no hacen `match` sobre `Command` ni `DocumentError`; no necesitan cambios.
- Sin dependencias nuevas.

## Fuera de alcance

- El relleno por inundación (calcular la región): unidad 5. El comando `FillSpans` aquí solo escribe los tramos que le den.
- Tolerancia de color y relleno degradado (hito 4); fusionar pasos; comprimir pasos con fondo ruidoso con rectángulos.
- Otros comandos masivos (rectángulo y elipse rellenos) que podrían usar `FillSpans`: salen de las unidades siguientes.
- Serialización del comando (hito 8).

## Casos límite

`D`: documento de una imagen RGBA 4 × 3 recién creado (todo `c0`, transparente); `id = D.main_image()`; `S(...)` como en el hito 1; `F(spans, p)` es `FillSpans { image: id, spans, pixel: Rgba(p) }`; `sp(y, a, b)` es `Span { y, x_start: a, x_end: b }`; `c1`, `c2` colores distintos de `c0`.

| # | Operación | Resultado |
| --- | --- | --- |
| 1 | `apply(F([sp(0,0,4)], c1))`; `undo()`; `redo()` | `Ok(true)`; la fila 0 es `c1` y el resto `c0`; `undo` deja el documento igual a `D` con `can_redo()`; `redo` repite el estado de tras `apply` |
| 2 | `apply(F([sp(0,1,3), sp(1,0,2), sp(1,2,4), sp(2,3,4)], c1))`; `undo()` | `Ok(true)`; justo esos 7 píxeles son `c1` (tramos pegados válidos); un `undo` deja `D` |
| 3 | `apply(S(px(1,0,c2)))`; `apply(F([sp(0,0,4)], c1))`; `undo()` | `Ok(true)` ambos; tras `undo` el píxel `(1,0)` vuelve a `c2` (no a `c0`) y el resto de la fila a `c0`; se guarda un trozo por valor anterior distinto |
| 4 | `apply(S(px(1,0,c1)))`; `apply(F([sp(0,0,4)], c1))`; `undo()` | `Ok(true)`; los píxeles que ya valían `c1` no se tocan ni se guardan; tras `undo` queda solo `(1,0)` en `c1` |
| 5 | `apply(F([sp(0,0,4)], c0))` en `D`; y `apply(S(a))`, `undo()`, `apply(F(.., c0))` | `Ok(false)`; `can_undo()` y `history_bytes()` no cambian (falso y 0 en `D`); en el segundo caso `can_redo()` sigue siendo cierto |
| 6 | `apply(F([], c1))`; `apply(Command::FillSpans` con `ImageId::from_raw(99)`); `F(.., Gray(1))`; en una imagen indexada de paleta 2, `F(.., Indexed(9))` | `Err(EmptyCommand)`; `Err(UnknownImage(..))`; `Err(Pixel { pixel: 0, source: ModeMismatch { .. } })`; `Err(Pixel { pixel: 0, source: IndexNotInPalette { index: 9, len: 2 } })`; intactos |
| 7 | `F([sp(3,0,1)])`; `F([sp(0,2,5)])`; `F([sp(1,5,7)])`; `F([sp(0,0,2), sp(1,3,9)])` | `Err(Span { span: 0, source: OutOfBounds { x: 0, y: 3, width: 4, height: 3 } })`; `x: 4, y: 0`; `x: 5, y: 1`; `Err(Span { span: 1, source: OutOfBounds { x: 4, y: 1, .. } })` y `(0,0)`, `(1,0)` siguen en `c0` (atomicidad); `can_undo()` falso |
| 8 | `F([sp(0,2,2)])`; `F([sp(0,3,1)])` | `Err(BadSpans { span: 0 })` ambos (vacío o invertido); intactos |
| 9 | `F([sp(1,0,2), sp(0,0,2)])`; `F([sp(0,0,3), sp(0,2,4)])`; `F([sp(0,0,2), sp(0,0,2)])` | `Err(BadSpans { span: 1 })` los tres (desordenado, solapado, repetido); intactos |
| 10 | `F([sp(0,0,2), sp(0,2,4)], c1)`; `F([sp(1,0,9), sp(0,0,1)])`; `F([sp(0,0,2), sp(0,1,9)])` | `Ok(true)` con los 4 píxeles de la fila 0; `Err(Span { span: 0, source: OutOfBounds { x: 4, y: 1, .. } })` (gana el primer tramo defectuoso, aunque el 1 también lo sea); `Err(BadSpans { span: 1 })` (el tramo 1 solapa **y** se sale del lienzo: `BadSpans` antes que `Span`) |
| 11 | `apply(Batch([S(a), F([sp(0,0,4)], c1), S(b)]))` en `D`; `undo()`; `redo()` | `Ok(true)`; un `undo` deja `D` y `can_undo()` falso (con `a` o `b` repetidos sobre la fila); un `redo` deja el estado final |
| 12 | `Batch([F(fila 0, c1), F(fila 0, c1)])`; `Batch([F(fila 0, c1), F(fila 0, c0)])` | `Ok(true)` con un paso del primero solo; `Ok(true)` y el documento es igual a `D` (cambió y restauró), un `undo` lo deja igual a `D` |
| 13 | `Batch([F(ok), F([sp(3,0,1)])])`; `Batch([S(ok), F([sp(1,0,2), sp(0,0,1)])])`; `Batch([F(ok), Batch([..])])` | `Err(InBatch { index: 1, source: Span { .. } })`; `Err(InBatch { index: 1, source: BadSpans { span: 1 } })`; `Err(InBatch { index: 1, source: NestedBatch })`; ningún píxel cambia y la historia queda como antes |
| 14 | relleno de toda la imagen en un lienzo de 4 × 3 y en otro de 4000 × 3 (fondo uniforme) | los dos `Ok(true)` con el **mismo** `history_bytes()` (tres trozos): el coste no depende del ancho |
| 15 | `set_history_limit(0)`; dos `F(..)` de efecto distinto | solo el último es deshacible; `history_bytes()` es el coste de un paso. Un solo relleno mayor que el límite se conserva y `undo()` lo revierte. Con límite igual a la suma de dos pasos de relleno y un tercero: se descarta el más antiguo y `history_bytes() <= límite` |
| 16 | `F([sp(0,0,2)], Indexed(2))` y `F(.., Indexed(9))` en una imagen indexada de paleta 2 (`Indexed(1)` sí es válido); lo mismo como segundo comando de un `Batch([S(ok), F(..)])` | `Err(Pixel { pixel: 0, source: IndexNotInPalette { index: 2, len: 2 } })` y `index: 9`; en el `Batch`, `Err(InBatch { index: 1, source: Pixel { pixel: 0, source: IndexNotInPalette { .. } } })` con el `S` revertido; igual que `SetPixels`; la imagen y la historia intactas; `F([sp(0,0,2)], Indexed(1))` da `Ok(true)` |
| 17 | `D'` = `D` tras `S(px(1,0,c1))` y `S(px(2,1,c2))`. (a) `apply(Batch([S(px(1,0,c2)), F([sp(0,0,4)], c1)]))`; (b) `apply(Batch([F([sp(0,0,4)], c1), S(px(1,0,c2))]))`; en ambos `undo()` y después `redo()` | `Ok(true)` y un solo paso. (a) Tras `apply` la fila 0 es toda `c1` (el relleno pisa el píxel suelto); (b) tras `apply` la fila 0 es `c1` salvo `(1,0)` que es `c2` (el píxel suelto pisa el relleno). En ambos `undo()` deja una imagen **idéntica a `D'`** (el `(1,0)` vuelve a `c1`, no a `c0`) y `redo()` deja **exactamente** la imagen de tras `apply`. Se repite con `Batch([S(p), F(cubre p), S(p')])` y con dos rellenos seguidos que se solapan |
| 18 | `apply` de `S`, de `F` y de un `Batch([S, F, S])`, uno tras otro sobre `D`, con `bytes_k` = `history_bytes()` tras el `apply` número `k` (y `bytes_0 = 0`); luego todos los `undo()` y todos los `redo()`; además un solo paso de cada clase; además `apply`, `apply`, `undo`, `apply` | `bytes_k` crece estrictamente; tras el `undo` número `j` vale `bytes_{n-j}` (baja hasta 0) y tras el `redo` número `j` vale `bytes_j` (sube hasta `bytes_n`); un solo paso: 0 tras `undo()` y su coste tras `redo()`; tras `apply`, `apply`, `undo`, `apply` la pila de deshacer vuelve a tener dos pasos (el segundo es otro) y `history_bytes()` es el de dos pasos de igual forma; un `apply` sin cambios y un `undo()` sin paso no lo tocan |

Las filas 6, 7 y 10 fijan el orden de errores: `EmptyCommand`, `UnknownImage`, `Pixel`, y después, tramo a tramo, `BadSpans` antes que `Span`; gana el primer error.

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`, con `crates/px-core/tests/document.rs` sin modificar
- `cargo deny check` (no se añade ninguna dependencia)

## Tests

En `crates/px-core/tests/fill_spans.rs` (nuevo; integración sobre la API pública). Imágenes de hasta 6 × 6 en cualquier modo (indexado con paleta de 1 a 8 entradas), para que los tramos y los valores anteriores choquen a menudo; tramos válidos generados ordenados (con y sin tramos pegados) y fallos inyectados (fuera del lienzo, vacío, desordenado, solapado, otro modo, índice fuera de paleta, id desconocido, lista vacía). **Unitarios**: una prueba por fila de la tabla.

**Propiedades** (`proptest`), con el límite por defecto salvo en la 5:

1. **Central ampliada.** Para cualquier documento inicial y una secuencia de 0 a 30 comandos entre `SetPixels`, `FillSpans` y `Batch`. Los `Batch` mezclan ambos comandos, en cualquier orden y con **coordenadas solapadas a propósito**: los píxeles de un `SetPixels` se eligen dentro de tramos de un `FillSpans` del mismo lote (y al revés), y hay lotes con dos rellenos que se pisan (los que fallan se ignoran): `undo()` hasta `Err` se ejecuta tantas veces como `apply` con `Ok(true)` y deja un documento igual al inicial; después `redo()` hasta `Err` se ejecuta las mismas veces y deja uno igual al final.
2. **Historia contra un modelo.** Para 0 a 40 operaciones mezcladas (`apply` de `SetPixels`, `FillSpans` y `Batch`, válido, inválido o sin cambios, `undo`, `redo`), comparadas con un modelo que escribe píxel a píxel sobre instantáneas de `Image`: tras cada operación el documento es igual al instante del cursor, `can_undo` y `can_redo` coinciden, el `bool` de `apply` es «algún píxel cambió» y el error es el esperado, con el orden de comprobación del comportamiento 2.
3. **Atomicidad y ausencia de pánicos.** Con `FillSpans` totalmente arbitrarios (`y`, `x_start`, `x_end` cualquier `u32`, listas sin orden, vacías, `Pixel`s e ids cualesquiera, dentro y fuera de `Batch`): `apply` nunca hace pánico y, si devuelve `Err` o `Ok(false)`, el documento y `can_undo`, `can_redo` e `history_bytes` no cambian.
4. **Coste por trozos.** Para un relleno válido sobre una imagen cualquiera: `history_bytes()` tras el relleno **no supera** el de un `SetPixels` equivalente (mismos píxeles, mismo valor) más un margen fijo de 128 bytes; y sobre un fondo uniforme es igual al de otro relleno con los mismos tramos en las mismas filas pero con la mitad de ancho (el coste depende de los trozos, no de los píxeles).
5. **Límite con coste real.** Con límite aleatorio de 0 a 2000 bytes y una secuencia de `apply` válidos de `SetPixels`, `FillSpans` y `Batch`: tras cada uno, `history_bytes() <= límite` o solo queda un paso; deshacer todo deja el documento en el instante del primer paso conservado; rehacer todo da el final; `history_bytes()` tras `apply`, `undo` y `redo` vale lo mismo que tras `apply` (ver propiedad 7).
6. **`Batch` mixto es un paso.** Para 1 a 6 comandos hoja (`SetPixels` y `FillSpans` válidos) con solapes: `Batch(lista)` deja el mismo documento que aplicarlos uno a uno, devuelve `Ok(true)` si y solo si alguno cambió algo y un solo `undo()` devuelve el inicial y un solo `redo()` el final.
7. **`history_bytes()` sigue la pila de deshacer.** Con secuencias de 0 a 12 `apply` válidos de `SetPixels`, `FillSpans` y `Batch` y el límite por defecto: `history_bytes()` crece estrictamente con cada `apply` con `Ok(true)`; tras cada `undo` vale lo que valía antes del `apply` correspondiente (y 0 al deshacerlo todo); tras cada `redo` vuelve a esos valores en orden.

`OutOfMemory` no tiene prueba: no se puede provocar sin reservar memoria real. La reversión sin fallos se comprueba por las propiedades 2 y 3 y por revisión del código. Sin dorados: no hay un comportamiento de referencia que comparar.

## Decisiones (aprobadas el 2026-10-17; la 6 corregida)

1. **Trozos con valor anterior (20 B por trozo) y no rectángulos.** Ver tabla del comportamiento 1: peor caso 20 B/píxel (< 24), mejor caso 20 B por fila; los rectángulos cuestan por el área, no por la región.
2. **Tramos con fin excluido (`x_start..x_end`).** Se descarta fin incluido para que el tramo vacío no sea inexpresable y para no restar 1. Los tramos vacíos son error.
3. **Validación estricta, sin normalizar.** Tramos ordenados por `(y, x_start)`, sin solapes ni vacíos, y los pegados valen. Desorden o solape: `BadSpans`, no se arregla.
4. **Dos variantes de error nuevas** (`Span`, `BadSpans`) y reutilizar `Pixel { pixel: 0, .. }` para un píxel de modo erróneo, en lugar de una tercera variante.
5. **Reserva por comando en el relleno.** Los trozos se reservan antes de escribir cada relleno, tras contarlos con una pasada de solo lectura, y no una vez al principio del `Batch`. Es una **desviación respecto a la regla 5 del hito 1** («todo antes de modificar nada») solo cuando un relleno no es el primero de un `Batch`; ahí un `OutOfMemory` revierte lo anterior sin reservar. La alternativa (cota superior en el `apply`) pediría 20 B por píxel de la región y haría fallar justo los rellenos grandes.
6. **El paso es una secuencia ordenada de partes y el coste de uno solo de `SetPixels` no cambia.** Una lista de rellenos aparte, sin posición, perdería el orden en un `Batch` con `SetPixels` y `FillSpans` solapados (corrección del visto bueno). Cada relleno guarda su posición en la secuencia de píxeles sueltos y `STEP_BASE = size_of::<Vec<Change>>()` mantiene el coste de los pasos de `SetPixels` idéntico al actual, así que los tests del hito 1 (también el de `Batch([no-op, S(a)])` contra `S(a)`) no cambian.

## Notas para el hito 4

- Las operaciones sobre contenido ruidoso (borrar una selección, rellenar un rectángulo sobre un dibujo, pegar) tienen el peor caso de este formato: un trozo por cada cambio de valor anterior, 20 B por píxel frente a 1 a 4 B de un rectángulo con los valores anteriores. Podrían preferir **rectángulos**.
- `Step` es privado y la API de `apply`, `undo` y `redo` no depende de su formato, así que añadir una parte «rectángulo» (o elegir por el coste, comparando trozos y rectángulos al registrar) no cambia nada público ni los comandos. La secuencia ordenada de partes del comportamiento 5 ya admite una tercera clase de parte.

## Notas para la unidad 5 (relleno por inundación)

- Debe producir los tramos **ya ordenados por `(y, x_start)`**, sin solapes y sin vacíos: `FillSpans` no los ordena ni los fusiona (decisión 3); un tramo desordenado es `BadSpans`.
- Debe leer el estado del documento (`&Document`) y llamar a `apply` con un único `FillSpans`; un relleno sin cambios no debe llegar a `apply` con una lista vacía (sería `EmptyCommand`): usar `Ok(false)` como en `draw_line`.
