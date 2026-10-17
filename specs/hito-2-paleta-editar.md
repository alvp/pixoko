# Hito 2 · Unidad 10: editar, reordenar y rotar la paleta de una imagen indexada

Estado: **aprobada** (2026-10-17; decisiones 1 a 7 con los cambios del visto bueno incorporados al texto: `Move` ≡ `Permute` con propiedad propia, coste de la máscara de `Remove` dentro del límite de la historia y reserva antes de modificar, desplazamiento del transparente y aviso para la unidad 11, orden completo de errores de `Remove`, `undo`/`redo` atómicos ante falta de memoria con la alternativa de reservar valorada y descartada, definiciones con ejemplos a mano, oráculo independiente, propiedad central ampliada con `SetPixels`, `FillSpans` y `ConvertMode`; y un séptimo comando, `RotateColors`, con su sección para el hito 5 o 7). Las cifras de los casos están calculadas a mano (la derivación va en cada fila).

## Objetivo

Operaciones de edición de la paleta de una imagen `Indexed` del documento, todas como `Command` con deshacer, en `px-core`. Dos familias:

- **Reordenar sin cambiar la imagen** (`MovePaletteEntry`, `PermutePalette`): ningún color visible cambia; píxeles, paleta e índice transparente se remapean a la vez.
- **Editar** (`SetPaletteColor`, `AddPaletteEntry`, `RemovePaletteEntry`, `SetTransparentIndex`, `RotateColors`): cambian el aspecto de la imagen (salvo `Add`), cada una con su regla explícita. `RotateColors` es lo contrario de `Move` y `Permute`: los colores se desplazan entre índices y **los píxeles conservan su índice**.

Regla central: **ningún índice de píxel es `>= len` de la paleta ni el transparente apunta fuera, tras ninguna operación.** Sin dependencias nuevas, sin `unsafe`, sin dorado (la semántica es definición propia).

## Entradas y salidas

Siete variantes nuevas de `Command` (ya es `#[non_exhaustive]`), todas con `image: ImageId`:

| Comando | Qué hace |
| --- | --- |
| `SetPaletteColor { index: u8, color: Rgba }` | Cambia el color guardado de una entrada. Los píxeles no se tocan. |
| `MovePaletteEntry { from: u8, to: u8 }` | La entrada `from` pasa a estar en `to`; las de en medio se desplazan una posición. |
| `PermutePalette { order: Vec<u8> }` | `order[nuevo] = antiguo`. Debe ser una permutación de `0..len`. |
| `AddPaletteEntry { color: Rgba }` | Añade una entrada al final (índice `len`). |
| `RemovePaletteEntry { index: u8, into: u8 }` | Quita `index`; los píxeles que la usan pasan a `into`. Ambos son índices **antes** de quitar. |
| `SetTransparentIndex { index: Option<u8> }` | Fija o quita el índice transparente. |
| `RotateColors { start: u8, len: u16, steps: i32 }` | Rota los colores del rango `start..start + len` de la paleta `steps` posiciones (con signo). Los píxeles no cambian de índice. |

`MovePaletteEntry` equivale a la `PermutePalette` correspondiente y comparten mecanismo (propiedad 2).

- `DocumentError` (también `non_exhaustive`) gana `PaletteEdit(PaletteEditError)` (con `source`). `PaletteEditError` (`Clone`, `Debug`, `PartialEq`, `Eq`, `Display`, `Error`, `non_exhaustive`, exportado desde `px_core`):
  - `NotIndexed { mode: Mode }`: la imagen no es indexada.
  - `IndexOutOfRange { index: u8, len: usize }`: cualquier índice (`index`, `from`, `to`, `into`, transparente) `>= len`.
  - `BadPermutation`: `order` no tiene `len` elementos, repite alguno o se sale de `0..len`.
  - `PaletteFull`: `AddPaletteEntry` con 256 entradas.
  - `RemoveIntoSelf { index: u8 }`: `into == index`.
  - `LastEntry`: quitar la única entrada. Toda imagen tiene al menos un píxel, así que con una sola entrada quitar siempre es inválido.
  - `RangeOutOfPalette { start: u8, len: u16, palette_len: usize }`: `start + len > palette_len` en `RotateColors`.
- Resto de errores, los existentes: `UnknownImage`, `NotBatchable`, `InBatch`, `OutOfMemory`.

## Definiciones, con ejemplos a mano

Sea la paleta `[A,B,C,D]` (índices 0 a 3). Todas las tablas son `adelante[antiguo] = nuevo` (qué índice pasa a tener cada píxel).

1. **`Permute`.** `order[nuevo] = antiguo`: la paleta nueva es `order.map(|o| vieja[o])`. Con `order = [2,0,3,1]`: `nueva[0] = vieja[2] = C`, `nueva[1] = vieja[0] = A`, `nueva[2] = vieja[3] = D`, `nueva[3] = vieja[1] = B`; paleta `[C,A,D,B]`; `adelante = {0→1, 1→3, 2→0, 3→2}` (la inversa de `order`); un píxel que valía 2 (C) pasa a valer 0. La identidad `[0,1,2,3]` no cambia nada.
2. **`Move`.** `from` sale y se reinserta en `to`: `Vec::remove(from)` y `Vec::insert(to, x)`. `Move 0→2` sobre `[A,B,C,D]` da `[B,C,A,D]` (A sale y entra tras C); equivale a `Permute [1,2,0,3]`; `adelante = {0→2, 1→0, 2→1, 3→3}`. `Move 3→1` da `[A,D,B,C]` y equivale a `Permute [0,3,1,2]`. En general `order` es la lista `[0..len)` con `remove(from)` e `insert(to, x)`.
3. **`Remove(index, into)`** con índices antiguos, `into' = into` si `into < index` y `into − 1` si no. Tabla: `i < index → i`, `index → into'`, `i > index → i − 1`. Con `Remove(1, 0)` sobre una imagen de píxeles `[0,1,2,3]` la tabla es `{0→0, 1→0, 2→1, 3→2}` y los píxeles quedan **`[0,0,1,2]`**. La tabla no es inyectiva (0 y 1 acaban en 0): por eso deshacer necesita la máscara (punto 8).
4. **`RotateColors(start, len, steps)`.** Con `k = steps.rem_euclid(len)` (`0 ≤ k < len`), el rango se rota **hacia índices mayores** `k` posiciones: `nueva[i] = vieja[(i − k) mod len]` dentro del rango; es `rotate_right(k)` del trozo de paleta. Rotar toda `[A,B,C,D]` con 1 paso da **`[D,A,B,C]`** (D, el último, pasa a la primera posición); con `start 1, len 3, steps 1`: el trozo `[B,C,D]` pasa a `[D,B,C]` y la paleta es **`[A,D,B,C]`**. Con `steps −1`: `[B,C,D,A]` (toda) y `[A,C,D,B]` (el rango 1..4). Los píxeles conservan su índice: el píxel de valor 0 sigue valiendo 0, pero ahora resuelve al color nuevo de la entrada 0. «Todos los colores» es `start 0, len = palette.len()` (hasta 256, por eso `len` es `u16`).

## Comportamiento

1. **Invariante central de reordenar.** Para todo píxel, el color resuelto antes y después de `MovePaletteEntry` o `PermutePalette` es **idéntico**; se remapean píxeles, paleta e índice transparente (`transparente' = adelante[transparente]`) con la misma tabla. Las entradas sin usar viajan con las demás. `SetPaletteColor`, `SetTransparentIndex`, `Remove` y `RotateColors` **sí** cambian el aspecto; `Add` no.
2. **`RemovePaletteEntry`: lo que cambia es explícito.** Los píxeles con valor `index` pasan a resolver al color **resuelto** que tenía `into`; los demás, no cambian. **Índice transparente:** `None → None`; `Some(index) → None`; `Some(t)` con `t > index → Some(t − 1)` (todo lo que está tras la quitada baja un lugar, y la transparente con ellos); `Some(t)` con `t < index → Some(t)`. Si la quitada era la transparente, desaparece y sus píxeles toman el color de `into`: **dejan de ser transparentes** (opacos si `into` lo es). *Anotación para la unidad 11: la interfaz debe avisar antes de quitar la entrada transparente de que los píxeles que la usan pasan a ser visibles.* Si `into` era la transparente, los píxeles quedan transparentes: lo manda el llamante.
3. **Ningún índice fuera de rango.** Reordenar y mover usan una permutación de `0..n`; quitar manda `0..n` a `0..n − 1` (se comprueba para `i < index`, `index` y `i > index`); añadir solo agranda; `RotateColors` y `SetPaletteColor` no tocan índices; `SetTransparentIndex` valida contra `len`. Lo que no es válido se rechaza antes de tocar nada.
4. **`Ok(false)` sin paso** (sin tocar historia, rehacer, `state_id` ni `history_bytes`): `Move` con `from == to`; `Permute` con la identidad; `SetPaletteColor` con el mismo color; `SetTransparentIndex` con el mismo valor; `RotateColors` con `len < 2`, con `steps` múltiplo de `len` (también negativo) o con una rotación que no cambia la paleta (colores iguales, se comprueba sin reservar). Se decide **después** de validar: un índice o un rango fuera de la paleta sigue siendo error. `Add` y `Remove` siempre cambian algo: `Ok(true)`. `Ok(true)` en `SetPaletteColor`, `SetTransparentIndex`, `Add`, `Remove` y `RotateColors` implica que la imagen cambió; en `Move` y `Permute` (con orden distinto de la identidad) es `Ok(true)` aunque la imagen quede igual por colores repetidos y entradas sin usar (no se calcula el resultado antes de decidir). Cambiar el color de la entrada transparente es `Ok(true)`: el RGB guardado es contenido (se escribe en la paleta del PNG) aunque no se vea.
5. **Orden de errores** (el primero que aplica gana): `UnknownImage`, `NotIndexed`, argumentos, y después `Ok(false)`, reservas y modificación.
   - `SetPaletteColor`, `SetTransparentIndex` (con `Some`), `Move`: `IndexOutOfRange` por orden de campo (`from`, `to`).
   - `Permute`: `BadPermutation`. `Add`: `PaletteFull`.
   - **`Remove`: `UnknownImage`, `NotIndexed`, `IndexOutOfRange` de `index`, `IndexOutOfRange` de `into`, `LastEntry`, `RemoveIntoSelf`.** Con una entrada y `Remove(0, 1)` gana `IndexOutOfRange` (de `into`); con `Remove(0, 0)` gana `LastEntry`.
   - `RotateColors`: `RangeOutOfPalette` (`start + len > palette_len`; un rango vacío al final, `start = palette_len`, `len = 0`, es válido y da `Ok(false)`).
6. **Atomicidad y reserva.** Todo lo que puede fallar o reservar ocurre antes de la primera escritura, en este orden: validar; `Ok(false)`; hueco de la pila de deshacer; **tabla del paso (`Move`/`Permute`) o, en `Remove`, el recuento de píxeles que usan la entrada y la reserva exacta de la máscara**; modificar. La modificación no reserva ni puede fallar (sin `unwrap`, `expect` ni índices que puedan dar pánico). Con error, documento e historia intactos.
7. **Dentro de un `Batch`: no, para los siete.** Dan `Err(InBatch { index, source: NotBatchable })` con el lote revertido, y `NotBatchable` gana a cualquier otro error del comando (la misma regla que `ConvertMode`). Motivo: un `Step` guarda cambios de píxel sueltos y rellenos en orden, y un remapeo o una paleta distinta en medio exigen intercalar tipos de parte. Relajarlo después solo acepta más y no rompe a nadie. **Anotado para el hito 3: el rediseño de `Step` (ya previsto para `ConvertAll`) debe admitir también estos comandos en lotes** (por ejemplo, «añadir color y dibujar» como un paso).
8. **Deshacer y rehacer.** Un paso de paleta es el único contenido de su paso (por 7). Va en un campo `Option<PaletteStep>` de `Step` cuyo coste solo cuenta si es `Some`.

   | Comando | Qué guarda | Deshacer | Rehacer |
   | --- | --- | --- | --- |
   | `SetPaletteColor` | el color del otro lado | intercambia con la entrada | igual |
   | `SetTransparentIndex` | el `Option<u8>` del otro lado | intercambia | igual |
   | `AddPaletteEntry` | el color | quita la última | la vuelve a añadir |
   | `Move`, `Permute` | **solo `adelante`** (`len` bytes, ≤ 256) | aplica la inversa (tabla de pila de 256 bytes) | aplica `adelante` |
   | `Remove` | `index`, `into'`, el color quitado, el transparente anterior y la **máscara** | ver abajo | aplica la tabla del punto 3 y quita la entrada |
   | `RotateColors` | `start`, `len` y `k` (`steps` módulo `len`) | `rotate_left(k)` del rango, en sitio | `rotate_right(k)` |

   - **La paleta anterior no hace falta en `Move`/`Permute`**: la permutación es invertible y la paleta se reordena en sitio (copia a un array de pila de 256 `Rgba`, 1 KiB, y reescribe).
   - **La tabla no basta para `Remove`** (no es inyectiva, definición 3): el paso guarda una **máscara de 1 bit por píxel** (`⌈w·h / 64⌉` palabras `u64`, los píxeles que valían `index`), **solo si alguno la usaba**. Deshacer recorre los píxeles una vez: bit puesto → `index`; si no, `v < index → v` y `v ≥ index → v + 1`; reinserta el color en `index` y restaura el transparente guardado. Rehacer no usa la máscara. Descartado: copiar la imagen (`w·h` bytes, 8 veces más) y listas de tramos (hasta 16 bytes por tramo).
   - **`RotateColors` no reserva nada nunca**: `rotate_left` y `rotate_right` de la biblioteca estándar son en sitio. Deshacer y rehacer no pueden fallar por memoria.
   - **Memoria al deshacer/rehacer de `Remove` (deshacer) y `Add` (rehacer).** Hace falta un hueco en el `Vec` de la paleta. Se hace `try_reserve(1)` en la paleta **antes de sacar el paso de su pila**; si falla (o falla la reserva de la pila contraria, que ya existía), el resultado es `OutOfMemory` y **documento, paso y pilas quedan intactos y se puede reintentar** (una reserva ya hecha no estorba). **Alternativa valorada y descartada:** que `Add` reserve hasta 256 entradas en `apply` y `Remove` el hueco en la pila contraria. No es más simple y no es sólida: `Palette: Clone` conserva solo `len` en la copia (un `Document` clonado, o una imagen leída de un PNG, no tiene esa reserva y el `redo` de un `Add` reasignaría); y `undo`/`redo` ya reservan hueco en la pila contraria para **todos** los pasos, así que hacerlos infalibles exigiría tocar los pasos de píxel y de conversión que ya existen. Se queda `try_reserve(1)`.
9. **Coste y límite de la historia.** `Step::cost` suma el del `PaletteStep` solo en estos pasos (los de `SetPixels`, `FillSpans` y `ConvertMode` no cambian):
   - `SetPaletteColor`, `SetTransparentIndex`, `Add`, `RotateColors`: `size_of::<PaletteStep>()` (unas decenas de bytes, independiente de la imagen).
   - `Move`, `Permute`: lo mismo más `len` bytes (≤ 256), **independiente del tamaño de la imagen**.
   - `Remove`: lo mismo más 4 bytes de color y `⌈w·h/64⌉·8` bytes de máscara solo si hay píxeles que usan la entrada (4096×4096: 2 MiB; el máximo de 1 GiB: 128 MiB; copiar la imagen costaría 16 MiB y 1 GiB).
   - **La máscara cuenta en el límite**: `undo_bytes` y el recorte (`trim`) la incluyen como cualquier otro paso; se descarta lo más antiguo mientras se supere el límite, **nunca el último paso** (un `Remove` con máscara mayor que el límite se conserva). `history_bytes()` es igual tras `apply`, `undo` y `redo`.
10. **Por qué no hay `image.palette_mut()`.** La invariante de `Image` es conjunta (todo píxel `< palette.len()`, transparente existente) y mezcla `data` y paleta: un `&mut Palette` permitiría encoger, reordenar o rotar la paleta sin remapear los píxeles. `Image::palette()` seguirá devolviendo solo `&Palette` y no se añade ningún setter. Además, una mutación genérica no se puede deshacer sin guardar una copia. La API pública de `Image` y `Palette` **no cambia**.
11. **Métodos nuevos, todos `pub(crate)`** (nunca pánico; con argumentos inválidos devuelven `false` sin tocar nada):
    - `Palette`: `pop`, `permute(adelante)`, `remove_entry(index)`, `insert_entry(index, color)`, `rotate_range(start, len, k, hacia_la_derecha)` y `reserve_one`; son los únicos que tocan `colors` directamente (el campo es privado).
    - `Image`: `set_palette_entry`, `set_transparent`, `push_palette_entry`, `pop_palette_entry`, `reorder_palette(adelante)` (remapea píxeles, permuta paleta y transparente a la vez), `remove_palette_entry(index, into)`, `restore_palette_entry(index, color, transparente, máscara)`, `rotate_palette_range`, `count_pixels_using(index)` y `fill_mask(index, &mut [u64])` (lectura) y `reserve_palette_slot()`. Cada uno deja cumplida la invariante de `Image` al terminar.
12. **`RotateColors` y el índice transparente.** El índice transparente **se queda en su ranura** aunque caiga dentro del rango: no rota. El **RGB guardado** de esa entrada sí rota con los demás colores. Consecuencias, anotadas: el color que llega a la ranura transparente queda oculto (alfa 0 al resolver), y el color guardado que estaba en ella —que resolvía transparente— pasa a una ranura visible con su alfa guardado (puede verse opaco).
13. **Sin pánicos** con cualquier imagen, modo, índice, permutación, rango, `steps` (incluido `i32::MIN`), `ImageId` y límite.

## Compatibilidad con unidades anteriores

- **Nada no aditivo en la API pública de `Document` ni de `Image`.** `Command` y `DocumentError` son `non_exhaustive`; `Step` es privado y gana un campo. En `apply` se atienden los comandos de paleta antes de `execute` (como `ConvertMode`); en `execute_leaf` dan `NotBatchable`.
- **Un cambio interno de `undo` y `redo`**: hoy sacan el paso de la pila y después actúan; para el punto 8 miran antes de qué clase es y hacen la reserva de la paleta antes de sacarlo. Es privado y no cambia firmas ni el comportamiento de los pasos de píxel.
- **Todos los tests de `crates/px-core/tests/*` y los de `document.rs` deben pasar sin modificarse**; si no, me paro y aviso.
- El código nuevo va en `crates/px-core/src/palette_edit.rs` (error, `PaletteStep`, validación y aplicación); `document.rs` recibe el despacho y las ramas de `undo`, `redo` y `Step::cost`.

## Fuera de alcance

El panel de paletas (arrastrar, selector de color; unidad 11), ordenar por tono o luminancia (una llamada a `PermutePalette` con el `order` calculado fuera), fusionar duplicados, importar paletas, cuantización, y el ciclo de color como animación (sección siguiente).

## Para el hito 5 o 7: ciclo de color como animación

`RotateColors` es una edición **de una sola vez**: cambia el documento y se deshace como cualquier paso. El ciclo de color animado es otra cosa y no se hace aquí:

- **Datos en el documento.** Una lista de rangos de ciclo (`start`, `len`, velocidad en pasos por segundo, sentido, activo o no), almacenada en el documento y no en la historia de píxeles.
- **Aplicación al dibujar, sin modificar el documento por fotograma.** El visor calcula la fase (`pasos = tiempo × velocidad`, módulo `len`) y resuelve cada píxel con la paleta rotada por esa fase, sin tocar el documento. Para pasar una fase al documento («aplicar el ciclo actual») se emite un `RotateColors` con esos pasos: ese es el uso de este comando.
- **IFF ILBM.** El chunk `CRNG` de IFF ILBM guarda este tipo de rangos (a confirmar con la especificación pública de ILBM antes de implementarlo; de memoria: una velocidad en unidades de 1/16384, un campo de flags con «activo» e «invertido», y los índices inicial y final del rango). Se implementa solo a partir de la especificación, sin leer código de ningún editor, y se comprueba ejecutando el programa y guardando su salida en `golden/`.
- **GIF animado.** Se genera componiendo las fases del ciclo (una imagen resuelta por fase); la duración de cada fotograma sale de la velocidad.

## Casos límite

Imagen base `F`: Indexed 4×1; `R=(255,0,0,255)`, `G=(0,255,0,255)`, `B=(0,0,255,255)`, `K=(0,0,0,255)`; paleta `[R,G,B,K]`; píxeles `[0,1,2,3]`; sin transparente salvo que se diga.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `SetPaletteColor(1, (255,255,0,255))` | `Ok(true)`; paleta `[R,Y,B,K]`, píxeles `[0,1,2,3]` iguales; `undo` deja la imagen `==` a la original; `redo` otra vez `[R,Y,B,K]` |
| 2 | `SetPaletteColor(1, G)`; `SetPaletteColor(4, R)`; con transparente `Some(2)`, `SetPaletteColor(2, (9,9,9,255))` | `Ok(false)` sin paso; `Err(IndexOutOfRange { 4, 4 })`; `Ok(true)`: guardado `(9,9,9,255)`, resuelto `(9,9,9,0)` |
| 3 | Transparente `Some(0)`; `PermutePalette([3,2,1,0])` | Paleta `[K,B,G,R]`, píxeles `[3,2,1,0]`, transparente `Some(3)`. Resueltos antes y después: `[(255,0,0,0), (0,255,0,255), (0,0,255,255), (0,0,0,255)]` |
| 4 | Píxeles `[0,0,2,2]`; `PermutePalette([2,0,3,1])` | `adelante = {0→1, 1→3, 2→0, 3→2}`; paleta `[B,R,K,G]`, píxeles `[1,1,0,0]`; las entradas sin usar (G, K) viajan a 3 y 2; resueltos `R,R,B,B` antes y después. Paleta `[C,A,D,B]` en la definición 1 es la misma regla |
| 5 | `PermutePalette([0,1,2,3])`; `[0,1,2]`; `[0,1,1,3]`; `[0,1,2,4]` | `Ok(false)`; los otros tres `Err(BadPermutation)`; todo intacto |
| 6 | Transparente `Some(1)`; `MovePaletteEntry(0, 2)` | Paleta `[G,B,R,K]` (la definición 2), píxeles `[2,0,1,3]`, transparente `Some(0)` (G sigue siendo la transparente). Mismo documento final que `PermutePalette([1,2,0,3])` |
| 7 | `MovePaletteEntry(3, 1)`; `MovePaletteEntry(2, 2)`; `MovePaletteEntry(0, 4)` | Paleta `[R,K,G,B]`, píxeles `[0,2,3,1]`; `Ok(false)`; `Err(IndexOutOfRange { 4, 4 })` |
| 8 | `AddPaletteEntry(W=(255,255,255,255))`; `undo`; `redo`; después sobre una imagen de 256 entradas | `Ok(true)`, 5 entradas, `W` en el índice 4, píxeles y transparente iguales; `undo` deja 4; `redo` 5; con 256 entradas `Err(PaletteFull)`. Añadir `(0,0,0,0)` no lo vuelve transparente |
| 9 | `RemovePaletteEntry(1, 0)` | Paleta `[R,B,K]`, píxeles `[0,0,1,2]` (la definición 3); resueltos antes `R,G,B,K`, después `R,R,B,K`. `undo` restaura **exactamente** (el píxel 0 sigue en 0 y el 1 vuelve a 1: lo decide la máscara) |
| 10 | Transparente `Some(2)`; `RemovePaletteEntry(0, 3)` | `into' = 2`; paleta `[G,B,K]`; píxeles `[2,0,1,2]`; transparente `Some(1)` (B: `t = 2 > index = 0`, baja un lugar); resueltos `K, G, (0,0,255,0), K` |
| 11 | Transparente `Some(2)`: `RemovePaletteEntry(2, 0)`; en otra copia `RemovePaletteEntry(0, 2)` | Primera: paleta `[R,G,K]`, píxeles `[0,1,0,2]`, transparente `None`; el píxel 2 resuelve `R` opaco. Segunda: `into' = 1`, paleta `[G,B,K]`, píxeles `[1,0,1,2]`, transparente `Some(1)`; los píxeles 0 y 2 resuelven `(0,0,255,0)` (el 0 era R opaco: ahora transparente, porque el destino lo es) |
| 12 | `Remove(4, 0)`; `Remove(0, 4)`; `Remove(5, 7)`; `Remove(2, 2)`; imagen 2×1 de una entrada: `Remove(0, 0)` y `Remove(0, 1)` | `IndexOutOfRange { 4, 4 }`; `IndexOutOfRange { 4, 4 }`; `IndexOutOfRange { 5, 4 }` (`index` antes que `into`); `RemoveIntoSelf { index: 2 }`; `LastEntry` (gana a `RemoveIntoSelf`); `IndexOutOfRange { 1, 1 }` (gana a `LastEntry`). Todo intacto |
| 13 | `SetTransparentIndex`: `Some(2)`, `Some(2)`, `Some(1)`, `None`, `None`, `Some(4)` | `Ok(true)` (píxeles iguales, el 2 resuelve `(0,0,255,0)`), `Ok(false)`, `Ok(true)`, `Ok(true)`, `Ok(false)`, `Err(IndexOutOfRange { 4, 4 })` |
| 14 | Imagen `Rgba` y otra `Gray`, cada uno de los siete comandos; id 99; `Batch([SetPixels ok, SetPaletteColor ok])`; `Batch([PermutePalette sobre 99])`; `Batch([Remove(2, 2)])`; `Batch([RotateColors(0, 4, 1)])` | `Err(PaletteEdit(NotIndexed { mode }))`; `Err(UnknownImage(99))`; `Err(InBatch { index: 1, source: NotBatchable })` con el `SetPixels` revertido; `Err(InBatch { index: 0, source: NotBatchable })` (gana a `UnknownImage`); `InBatch { 0, NotBatchable }` (gana a `RemoveIntoSelf`); `InBatch { 0, NotBatchable }` (el lote con `RotateColors` válido también se rechaza). Todo intacto |
| 15 | Coste: `Permute` de 16 entradas en 8×8 y en 512×512; `Remove` de entrada sin usar en 512×512; `Remove` de entrada usada en 64×64; `RotateColors`; `apply`, `undo`, `redo`. Límite: `set_history_limit(300)`, un `Permute` y un `Remove` de entrada usada en 64×64 | Los dos `Permute` suben `history_bytes` **lo mismo**, entre 16 y 512; `Remove` sin usar ≤ 512; el usado en 64×64 entre 512 y 1024 (máscara `⌈4096/64⌉·8 = 512`); `RotateColors` ≤ 512 y igual en cualquier tamaño de imagen; `history_bytes()` igual tras `apply`, `undo`, `redo`. Con límite 300: el `Permute` (cuesta menos de 300) cabe, el `Remove` con su máscara de 512 B lo hace superar el límite, se descarta el `Permute` y **queda solo el `Remove`** (un único `undo`); la máscara cuenta en el límite |
| 16 | `RotateColors(0, 4, 1)`; `RotateColors(1, 3, 1)`; `RotateColors(0, 4, −1)`; `RotateColors(1, 3, −1)`; `RotateColors(0, 4, 5)` | Paleta `[K,R,G,B]` (toda, 1 paso: `[A,B,C,D] → [D,A,B,C]`); `[R,K,G,B]` (el trozo `[G,B,K]` pasa a `[K,G,B]`); `[G,B,K,R]`; `[R,B,K,G]`; `[K,R,G,B]` (5 mod 4 = 1). En todas los píxeles siguen siendo `[0,1,2,3]` y resuelven a la paleta nueva en ese orden. `undo` deja `[R,G,B,K]` exacta; `redo`, la rotada |
| 17 | Sobre `F`: `RotateColors(0, 4, 4)`; `(0, 4, −8)`; `(0, 4, 0)`; `(1, 1, 3)`; `(2, 0, 1)`; `(4, 0, 1)`; `(0, 3, i32::MIN)`. Con paleta `[R,R,R,R]`: `(0, 4, 1)`. Sobre una paleta de 256 entradas distintas: `(0, 256, 1)` y `(0, 256, i32::MIN)` | `Ok(false)` los seis primeros (múltiplos de `len`; `len < 2`; `start = 4 = palette_len` con `len = 0` es un rango vacío válido) y el de `[R,R,R,R]` (la rotación deja la paleta igual); `(0, 3, i32::MIN)` es `Ok(true)` sin pánico: `i32::MIN = −2³¹`, `2³¹ mod 3 = 2`, luego `k = 1` y `[R,G,B]` pasa a `[B,R,G]` (paleta `[B,R,G,K]`); con 256 entradas, `(0, 256, 1)` es `Ok(true)` (`len = 256` cabe en `u16`) y `(0, 256, i32::MIN)` es `Ok(false)` (`−2³¹` es múltiplo de 256) |
| 18 | `RotateColors(2, 3, 1)` sobre `F`; `RotateColors(5, 1, 1)`; `RotateColors(0, 5, 1)`; `RotateColors(4, 1, 1)` | `Err(RangeOutOfPalette { start: 2, len: 3, palette_len: 4 })` (2 + 3 = 5 > 4); `Err(RangeOutOfPalette { 5, 1, 4 })`; `Err(RangeOutOfPalette { 0, 5, 4 })`; `Err(RangeOutOfPalette { 4, 1, 4 })` (4 + 1 > 4). Todo intacto |
| 19 | Transparente `Some(3)` (K); `RotateColors(0, 4, 1)` | Paleta `[K,R,G,B]`; el transparente **sigue en `Some(3)`**; píxeles `[0,1,2,3]` iguales; resueltos antes `[R,G,B,(0,0,0,0)]`, después `[K,R,G,(0,0,255,0)]` = `[(0,0,0,255), (255,0,0,255), (0,255,0,255), (0,0,255,0)]`: K, que resolvía transparente, ahora se ve opaco en la ranura 0 y B queda oculta en la 3 |

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas): `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` con todos los tests anteriores (y los de `document.rs`) sin modificar, y `cargo deny check` (sin dependencias nuevas; `proptest` ya es dev-dependency). Los tests corren en debug (`overflow-checks`).

## Tests

`crates/px-core/tests/palette_edit.rs`, integración sobre la API pública. **Unitarios**: uno por fila. **Propiedades** (`proptest`; imágenes de hasta 8×8, paletas de 1 a 16 entradas y, en uno de cada tres casos, de 255 o 256 para cubrir el índice 255, la tabla de 256 y la longitud que no cabe en `u8`; colores de un conjunto pequeño para provocar duplicados y alfa 0):

1. **Oráculo independiente de reordenar.** Para `PermutePalette` (permutación aleatoria) y `MovePaletteEntry`: un oráculo escrito en el propio test, **sin compartir código con la implementación y sin llamar a `Palette::resolved` ni a `Image::set_pixel`**, calcula el color resuelto de cada píxel **con la paleta original** (`visible(color, es_transparente)` propio, leyendo `palette().colors()`, `transparent_index()` y `get_pixel`), construye la imagen esperada (`order.map`, búsqueda lineal del nuevo índice de cada píxel) y comprueba que (a) la imagen coincide exactamente (píxeles, paleta y transparente) y (b) el color resuelto de **cada** píxel es idéntico antes y después: ningún color visible cambia. Ida y vuelta: `Permute(inversa)` y `Move(to, from)` devuelven la imagen original.
2. **`Move` equivale a `Permute`.** Para `from`, `to` aleatorios: el `order` se construye en el test con `Vec::remove` e `insert` sobre `[0..len)`; aplicar `Move(from, to)` a un documento y `Permute(order)` a otro igual da documentos iguales, el mismo `bool` (`Ok(false)` si y solo si `from == to`), y el mismo `history_bytes()`; y `undo` en los dos da la imagen original.
3. **Oráculo de quitar.** Para `Remove(index, into)` aleatorio: los píxeles que no valían `index` resuelven igual; los que sí, al resuelto que tenía `into` en la paleta original (calculado con el mismo `visible` propio); la paleta es la antigua sin `index`; el transparente sigue el punto 2 (con `None`, `< index`, `== index` y `> index`); ningún píxel `>= len`; el número de píxeles con el valor de `into'` es la suma de los que valían `into` e `index`; `undo` restaura la imagen `==` y `redo` la quitada.
4. **Propiedad central, secuencias mezcladas.** Para un documento de una imagen de hasta 6×6 (indexada casi siempre, a veces en otro modo), una secuencia de 0 a 24 operaciones entre los siete comandos nuevos, `SetPixels`, `FillSpans` y `ConvertMode` (válidos o no, con ids desconocidos y `Batch` con comandos de paleta), con un límite de historia grande (nunca se descarta un paso), `undo` y `redo` intercalados. Tras cada operación: ningún índice de píxel es `>= len` de la paleta y el transparente es `None` o `< len` (comprobado por la API pública, en cualquier modo indexado que haya en ese momento); con `Err` u `Ok(false)` quedan intactos documento, `can_undo`, `can_redo`, `history_bytes` y `state_id`. Se guarda una instantánea (`Image` clonada) tras cada `Ok(true)`; al final, deshacer todo pasa por cada instantánea anterior y llega a la **inicial exacta** (imagen, paleta e índice transparente) y rehacer todo pasa por cada una y llega a la **final exacta**.
5. **Validación y atomicidad.** Para comandos de paleta aleatorios (válidos o no), `apply` da error si y solo si lo dice un predicado independiente escrito en el test, con el orden de errores del punto 5 (incluido el de `Remove` completo y `RangeOutOfPalette`); con `Err`, `Ok(false)` o dentro de un `Batch` (`NotBatchable`), documento, `can_undo`, `can_redo`, `history_bytes` y `state_id` intactos. `Ok(false)` en exactamente los casos del punto 4.
6. **Coste y límite.** Con la misma paleta y dos tamaños de imagen, `Permute`, `Move`, `SetPaletteColor`, `SetTransparentIndex` y `RotateColors` suben `history_bytes` lo mismo y ≤ 512; `Remove` sube entre la máscara (`⌈w·h/64⌉·8` si hay píxeles que usan la entrada, 0 si no) y esa cifra más 512; `history_bytes` es igual tras `apply`, `undo`, `redo`; y con límite aleatorio de 0 a 2000 bytes, tras cada `apply` `history_bytes() <= límite` o solo queda un paso, y el último paso (incluido un `Remove` con máscara mayor que el límite) nunca se descarta.
7. **Cada edición toca solo lo suyo.** `SetPaletteColor(i, c)`: los píxeles son los mismos bytes y el RGBA resuelto cambia exactamente en los píxeles de valor `i`. `SetTransparentIndex`: los píxeles son los mismos bytes y solo cambia el alfa de los que usan la entrada afectada. `Add` no cambia ni un píxel ni el transparente.
8. **`RotateColors`.** Para `start`, `len`, `steps` aleatorios (incluidos negativos, múltiplos de `len`, `len` de 0 a 256 y `i32::MIN`/`MAX`): un oráculo que rota la paleta **a mano** (`nueva[start + i] = vieja[start + (i + len − k) mod len]`, sin `rotate_right`) y calcula el color resuelto de cada píxel (el transparente se queda en su ranura); coincide la paleta, los **índices de los píxeles no cambian** (bytes iguales) y el color resuelto de cada píxel es el del oráculo; los píxeles cuyo índice cae **fuera** del rango resuelven igual que antes; se conserva el multiconjunto de colores de la paleta; rotar `k` y luego `−k` devuelve la paleta exacta; rotar `len` (y cualquier múltiplo, también negativo) da `Ok(false)` sin paso; y `undo` y `redo` son exactos (la imagen `==` a la anterior y a la rotada).

`OutOfMemory` no tiene prueba (no se puede provocar sin reservar memoria real); su garantía (documento, paso y pilas intactos) es de diseño: las reservas van antes de sacar el paso de su pila.

## Decisiones (aprobadas el 2026-10-17, con los cambios indicados)

1. **Siete comandos**, con `Move` y `Permute` los dos (mismo mecanismo y propiedad 2 que los iguala).
2. **Quitar se deshace con una máscara de 1 bit por píxel**, solo si algún píxel usa la entrada, y la máscara cuenta en el límite de la historia con reserva antes de modificar; `Move` y `Permute` guardan solo `adelante`.
3. **`into` en `Remove` en índices antiguos; quitar la transparente deja el transparente en `None`** y los píxeles toman el color de `into`; el transparente mayor que el quitado baja un lugar. La interfaz (unidad 11) debe avisar de que esos píxeles dejan de ser transparentes.
4. **Ningún comando de paleta entra en un `Batch`** (`NotBatchable`); anotado para el rediseño de `Step` del hito 3.
5. **`Ok(false)`**: mismo color, identidad, `from == to`, mismo transparente y las tres condiciones de `RotateColors`; `SetPaletteColor` sobre la transparente es `Ok(true)`.
6. **Orden de errores de `Remove`**: imagen desconocida, modo no indexado, `index`, `into`, `LastEntry`, `RemoveIntoSelf`.
7. **`undo`/`redo` hacen `try_reserve(1)` en la paleta antes de sacar el paso**; si falla, todo intacto y reintentable. Reservar 256 entradas en `Add` se valoró y se descartó (no es más simple y no es sólido con `Clone`).
8. **`RotateColors { start: u8, len: u16, steps: i32 }`**: rotación hacia índices mayores (`rotate_right`), `steps` módulo `len`; `RangeOutOfPalette` como error propio; el transparente se queda en su ranura y su RGB guardado rota; deshacer es la rotación inversa en sitio sin reservar; `NotBatchable`; el ciclo de color como animación queda para el hito 5 o 7.
