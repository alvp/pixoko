# Hito 1 · Unidad 4: `Document` y comandos con deshacer

Estado: **aprobada** (2026-10-17)

## Objetivo

Definir en `px-core` el documento y la única vía para editarlo: comandos que son datos, con deshacer y rehacer. La interfaz (hito 2) y la CLI (hito 8) usarán esta misma API. En este hito el documento contiene una sola `Image` y hay dos comandos: `SetPixels` y `Batch`.

## Entradas y salidas

- `px_core::ImageId`: identificador opaco de una imagen dentro del documento. `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`. `ImageId::from_raw(u32)` y `raw() -> u32` (la CLI y un futuro serde necesitan escribirlo y leerlo). Solo `Document` reparte identificadores válidos.
- `px_core::PixelEdit { x: u32, y: u32, pixel: Pixel }`: campos públicos. `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`.
- `px_core::Command`: `#[non_exhaustive]`; `Clone`, `Debug`, `PartialEq`, `Eq`. Sin closures ni referencias: es un dato completo.
  - `SetPixels { image: ImageId, pixels: Vec<PixelEdit> }`: escribe los píxeles en orden.
  - `Batch(Vec<Command>)`: ejecuta los comandos en orden como **un solo paso** de deshacer.
- `px_core::Document`: `Clone`, `Debug`, `PartialEq`, `Eq`. **La igualdad compara el contenido (las imágenes por id) y no la historia**: dos documentos con el mismo contenido son iguales aunque lleguen por caminos distintos.
  - `pub const DEFAULT_HISTORY_LIMIT: usize = 64 << 20` (64 MiB).
  - `Document::new(image: Image) -> Document`: historia vacía y límite por defecto.
  - `main_image(&self) -> ImageId`: el id de la imagen con la que se creó el documento.
  - `image(&self, id: ImageId) -> Option<&Image>`: `None` si el documento no tiene esa imagen. No hay `&mut Image`: toda escritura pasa por `apply`.
  - `apply(&mut self, command: Command) -> Result<bool, DocumentError>`: `Ok(true)` si el documento cambió y `Ok(false)` si no (comportamiento 9).
  - `undo(&mut self) -> Result<(), DocumentError>` y `redo(&mut self) -> Result<(), DocumentError>`.
  - `can_undo() -> bool`, `can_redo() -> bool`.
  - `history_bytes() -> usize`: memoria de los pasos de deshacer (ver comportamiento 6).
  - `history_limit() -> usize` y `set_history_limit(&mut self, bytes: usize)`.
- `DocumentError`: `#[non_exhaustive]`; `Display`, `Error` (con `source`), `Debug`, `Clone`, `PartialEq`, `Eq`.
  - `UnknownImage(ImageId)`: el documento no tiene esa imagen.
  - `Pixel { pixel: usize, source: ImageError }`: falló la escritura del píxel número `pixel` de la lista (`OutOfBounds`, `ModeMismatch`, `IndexNotInPalette`).
  - `EmptyCommand`: `SetPixels` sin píxeles o `Batch` sin comandos.
  - `NestedBatch`: un `Batch` dentro de otro `Batch`.
  - `InBatch { index: usize, source: Box<DocumentError> }`: el comando número `index` de un `Batch` falló con `source`.
  - `NothingToUndo`, `NothingToRedo`.
  - `OutOfMemory`: no se pudo reservar memoria para la historia.

## Comportamiento

1. **Contenido y futuro.** Hoy `Document` guarda una imagen con el id `main_image()` (el valor crudo 0). Para no reescribir la API pública cuando lleguen las capas (hito 3) y los fotogramas (hito 5):
   - Los comandos nunca asumen «la» imagen: llevan el `ImageId` de la imagen que editan. Con capas y fotogramas, cada celda es una imagen con su id; `SetPixels` no cambia.
   - `Document::new(image)` seguirá creando un documento con una capa y un fotograma; `image(id)` seguirá devolviendo la imagen de ese id.
   - `Command` y `DocumentError` son `#[non_exhaustive]`: añadir variantes (`AddLayer`, `ResizeCanvas`…) no rompe a los demás crates.
   - Los ids nunca se reutilizan en un documento (regla para el hito 3: al quitar una imagen su id queda retirado), de modo que un paso antiguo de la historia no puede apuntar a otra imagen.
   - Lo que sí cambiará: los pasos de la historia hoy solo contienen cambios de píxel; los comandos estructurales añadirán otro tipo de cambio. El tipo del paso es privado, así que no afecta a la API.
2. **Comandos como datos.** `Command` no lleva `Fn`, `Box<dyn …>` ni referencias, y todos sus campos son públicos y de tipos simples, para poder construirlos desde texto en la CLI y añadir `serde` más adelante (no se añade ahora).
3. **`SetPixels`.** Orden de comprobación: `EmptyCommand`, `UnknownImage`, y después cada píxel en orden con `Image::set_pixel` (el primer fallo es `Pixel { pixel, source }`). Un mismo píxel puede aparecer varias veces: las escrituras se aplican en orden y gana la última; deshacer restaura el valor que había antes del comando. Escribir el valor que ya hay no es un error.
4. **`Batch`.** Ejecuta los comandos en orden. Un `Batch` vacío es `EmptyCommand`; un `Batch` anidado es `NestedBatch`. Los errores de un comando interno se devuelven como `InBatch { index, source }`, también `EmptyCommand` y `NestedBatch`. Se ejecuta en orden y el primer error gana (no se valida todo antes): en `Batch([S(fuera de rango), Batch(..)])` el error es el `Pixel` del índice 0. Cualquier `Batch` interno, vacío o no, es `NestedBatch`. No se anida: así `apply` no es recursivo y no hay límite de profundidad que vigilar.
5. **Atomicidad y memoria.** Antes de modificar nada, `apply` reserva todo lo que pide memoria: el `Vec` de cambios, con capacidad para todas las escrituras del comando (la suma se hace con `checked_add`), y el hueco en la pila de deshacer. Si una reserva falla, `OutOfMemory` y el documento no se ha tocado. Después ejecuta los comandos uno a uno anotando cada escritura que cambia el valor (`imagen, x, y, antes, después`). Si algo falla, restaura los valores anteriores en orden inverso y devuelve el error: el documento queda igual que antes y la historia no se toca (la pila de rehacer tampoco se borra). La reversión no puede fallar: no reserva memoria (todo se reservó antes) y solo escribe valores que salieron de la propia imagen, cuyo tamaño, modo y paleta no cambian. La implementación no usa `unwrap`, `expect` ni índices que puedan hacer pánico en ese camino. `undo` y `redo` siguen la misma regla: reservan el hueco en la pila de destino antes de tocar un píxel. Se elige revertir en vez de validar antes porque los comandos de hitos posteriores (por ejemplo, redimensionar y luego dibujar en un `Batch`) dependerán del resultado de los anteriores.
6. **Historia por cambios.** Un paso guarda **solo las escrituras que cambiaron un valor**, en orden de aplicación; deshacer los escribe en orden inverso con el valor «antes» y rehacer en orden directo con el valor «después». Coste de un paso: `size_of::<Step>() + capacidad * size_of::<Change>()`, unos 24 bytes por cambio (el tamaño exacto no es API). Como la reserva previa es una cota superior, tras un `apply` correcto se intenta ajustar la capacidad al número de cambios copiándolos a un `Vec` exacto; si esa reserva falla, se conserva el original (solo ocupa más) y no es un error. Un trazo de 1000 píxeles ocupa unos 24 KB; una copia de la imagen costaría hasta 1 GiB por paso.
7. **Límite.** `history_bytes()` suma el coste de los pasos de la pila de deshacer. Tras cada operación que añade un paso a esa pila (`apply` con cambio o `redo`) y en `set_history_limit`, se descartan los pasos más antiguos mientras `history_bytes() > history_limit()`, **pero nunca el más reciente**: un solo paso mayor que el límite se conserva y la última edición siempre se puede deshacer. Con límite 0 queda un solo paso. Los pasos descartados no se pueden recuperar. La pila de rehacer no cuenta: solo contiene pasos que estuvieron en la de deshacer, así que no crece por encima de ella.
8. **`undo` y `redo`.** `undo` saca el último paso, lo revierte y lo pasa a la pila de rehacer; `redo` hace lo contrario. Sin paso disponible, `NothingToUndo` o `NothingToRedo`, sin cambios.
9. **Resultado de `apply`.** Devuelve `Ok(true)` si alguna escritura cambió un valor y `Ok(false)` si ninguna lo hizo (todas escribían el valor que ya había). Cada escritura se compara con el valor que hay en ese momento: `[(0,0,c1), (0,0,c0)]` sobre un píxel `c0` cambia el valor y lo restaura, y cuenta como cambio (`Ok(true)`, un paso con dos cambios; deshacer lo deja igual). Con `Ok(true)` se añade un paso a la pila de deshacer y se **borra la de rehacer**. Con `Ok(false)` no ocurre nada: ni paso, ni borrado de rehacer, ni cambio en la historia. Un `Batch` crea un paso si cambió al menos un píxel entre todos sus comandos, y ninguno si no cambió ninguno.
10. **Sin pánicos.** Ninguna operación hace pánico, con cualquier coordenada `u32`, cualquier `ImageId`, cualquier `usize` como límite y cualquier combinación de comandos. No hay `unsafe`.

## Fuera de alcance

- Capas (hito 3), fotogramas (hito 5) y comandos de herramientas como línea o relleno (hito 2).
- Serialización de comandos (`serde`) y su lectura desde la CLI (hito 8).
- Fusionar pasos consecutivos, marcar el documento como modificado, panel de historial y guardar la historia en disco.
- Cambiar la paleta o el tamaño de la imagen: no hay comando para ello todavía.

## Notas para el hito 2

- Un solo paso mayor que el límite se conserva (comportamiento 7). Esto importa con los comandos masivos: un relleno del lienzo entero con 24 bytes por píxel costaría unas 6 veces los 4 bytes del píxel RGBA, y a 16384 × 16384 son unos 6 GiB.
- Por eso los comandos masivos del hito 2 (el relleno) **guardarán regiones o tramos**, no píxeles sueltos: el paso podrá guardar, por ejemplo, un rectángulo con los valores de antes y de después. El tipo `Step` es privado y los comandos no cambian, así que la API pública de esta unidad sigue valiendo.
- `apply` devuelve si hubo cambio para que la interfaz no marque el documento como modificado ni refresque el lienzo con un trazo que no tocó nada.

## Casos límite

Notación: `D` es un documento de una imagen RGBA 4 × 3 recién creado; `id = D.main_image()`; `S(...)` es `SetPixels { image: id, pixels: [...] }`; `px(x, y, c)` es `PixelEdit { x, y, pixel: Rgba(c) }`; `c0` es el valor inicial (transparente) y `c1`, `c2` colores distintos.

| Operación | Resultado |
| --- | --- |
| `Document::new(img)` | `image(main_image())` es `Some` e igual a `img`; `can_undo()` y `can_redo()` falsos; `history_bytes() == 0`; `history_limit() == DEFAULT_HISTORY_LIMIT` |
| `apply(S(px(0,0,c1)))`; luego `undo()`; luego `redo()` | `Ok(true)`; tras `apply` el píxel es `c1` y `can_undo()`, no `can_redo()`; tras `undo` el documento es igual a `D` y `can_redo()`; tras `redo` es igual al de tras `apply` |
| `apply(S(a))`, `undo()`, `apply(S(b))`, `redo()` | `redo()` da `Err(NothingToRedo)`; `can_redo()` falso desde el segundo `apply`; el documento tiene `b` |
| `undo()` y `redo()` en `D` | `Err(NothingToUndo)` y `Err(NothingToRedo)`; sin cambios |
| `apply(S(px(0,0,c1), px(0,0,c2)))`; `undo()`; y `apply(S(px(0,0,c1), px(0,0,c0)))`; `undo()` | `Ok(true)` ambos; el píxel queda en `c2` tras el primer `apply` y en `c0` tras su `undo`; en el segundo queda en `c0` y `undo()` es `Ok` y deja el documento igual (un paso con dos cambios) |
| `apply(S([]))`; `apply(Batch([]))` | `Err(EmptyCommand)`; documento e historia intactos |
| `apply(S(px(4,0,c)))`; `apply(S(px(0,0,c1), px(9,9,c)))` | `Err(Pixel { pixel: 0, source: OutOfBounds { x: 4, y: 0, width: 4, height: 3 } })`; `Err(Pixel { pixel: 1, .. })` y el píxel `(0,0)` sigue en `c0`; `can_undo()` falso |
| `apply(S(Gray(1) en (0,0)))`; en una imagen indexada de paleta 2, `Indexed(9)` | `Err(Pixel { source: ModeMismatch { .. } })`; `Err(Pixel { source: IndexNotInPalette { index: 9, len: 2 } })`; intactos |
| `apply(Command::SetPixels { image: ImageId::from_raw(99), .. })`; `image(ImageId::from_raw(99))` | `Err(UnknownImage(..))`; `None`; documento intacto |
| `apply(S(a))`, `undo()`, `apply(comando inválido)` | `Err`; `can_redo()` sigue siendo cierto y `redo()` devuelve el documento con `a` |
| `apply(S(px(0,0,c0)))` en `D` (el valor que ya hay); y `apply(S(a))`, `undo()`, `apply(S(px(0,0,c0)))` | `Ok(false)`; `can_undo()` y `history_bytes()` no cambian (falso y 0 en `D`); en el segundo caso `can_redo()` sigue siendo cierto y `redo()` devuelve el documento con `a` |
| `apply(Batch([S(a), S(b), S(c)]))` en `D`; `undo()`; `redo()` | `Ok(true)`; un solo `undo()` deja el documento igual a `D` y `can_undo()` falso; un solo `redo()` deja los tres cambios |
| `apply(Batch([S(no-op), S(a)]))`; `apply(Batch([S(no-op), S(no-op)]))` | `Ok(true)` con un paso que guarda solo el cambio de `a`; `Ok(false)` sin paso |
| `apply(Batch([S(ok), S(fuera de rango), S(ok)]))`; `apply(Batch([S(ok), Batch([S(ok)])]))` | `Err(InBatch { index: 1, source: Pixel { .. } })` y `Err(InBatch { index: 1, source: NestedBatch })`; ningún píxel cambia y `can_undo()` sigue como antes |
| `set_history_limit(0)`; tres `apply` de un píxel cada uno | solo el último paso es deshacible: `undo()` es `Ok` y el segundo da `Err(NothingToUndo)`; `history_bytes()` es el coste de un paso |
| dos `apply` de igual coste; `set_history_limit` con el `history_bytes()` de esos dos pasos; un tercer `apply`; después `set_history_limit(0)` | tras el tercero se descarta el más antiguo: quedan dos pasos y `history_bytes() <= límite`; tras `set_history_limit(0)` queda solo el último |
| un `apply` cuyo paso solo ya supera el límite | `Ok(true)`; el paso se conserva y `undo()` lo revierte |

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (no se añade ninguna dependencia)

## Tests

En `crates/px-core/tests/document.rs` (integración sobre la API pública). Los tests usan imágenes de hasta 4 × 4 en cualquier modo (el indexado con paleta de 1 a 8 entradas), para que las coordenadas colisionen a menudo; los comandos se generan con valores válidos y con fallos inyectados (coordenada fuera, otro modo, índice fuera de paleta, id desconocido, lista vacía, `Batch` anidado) y con **coordenadas repetidas dentro de un mismo `SetPixels`** y escrituras de valores que ya estaban (no-ops).

**Unitarios**: una prueba por fila de la tabla de casos límite.

**Propiedades** (`proptest`), con el límite por defecto salvo en la 5:

1. **Central: deshacer todo y rehacer todo.** Para cualquier documento inicial y una secuencia de 0 a 30 comandos (los que fallan se ignoran): `undo()` hasta `Err` se ejecuta tantas veces como `apply` que devolvieron `Ok(true)` y deja un documento igual al inicial (aunque un mismo `SetPixels` tocara varias veces la misma coordenada); después, `redo()` hasta `Err` se ejecuta las mismas veces y deja uno igual al final (el de justo después del último `apply`).
2. **Historia contra un modelo.** Para una secuencia de 0 a 40 operaciones mezcladas (`apply` válido, inválido o sin cambios, `undo`, `redo`), comparada con un modelo simple (un `Vec` de documentos instantáneos, un cursor, y los píxeles escritos con `Image::set_pixel` sobre una copia, de modo que también da el `bool` esperado): tras cada operación el documento es igual al instante del cursor; `can_undo` y `can_redo` coinciden con el modelo; los errores son los esperados; un `apply` con `Ok(true)` borra lo rehacible y uno fallido o con `Ok(false)` no toca ni el documento ni la historia.
3. **Atomicidad y ausencia de pánicos.** Para comandos totalmente arbitrarios (coordenadas `u32` completas, cualquier `Pixel`, ids cualesquiera, `Batch` anidados, listas vacías): `apply` nunca hace pánico y, si devuelve `Err` o `Ok(false)`, el documento es igual a una copia previa y `can_undo`, `can_redo` e `history_bytes` no cambian. Lo mismo con `undo`, `redo` y `set_history_limit` con cualquier `usize`.
4. **`Batch` es un paso.** Para una lista de 1 a 6 `SetPixels` válidos: `Batch(lista)` deja el mismo documento que aplicarlos uno a uno; devuelve `Ok(true)` si y solo si alguno cambió algo; en ese caso un solo `undo()` devuelve el inicial (con la historia vacía de partida, después `can_undo()` es falso) y un solo `redo()` devuelve el final, y en el otro no hay paso.
5. **Límite de la historia.** Con un límite aleatorio de 0 a 2000 bytes y una secuencia de `apply` válidos: tras cada uno, `history_bytes() <= límite` o solo queda un paso deshacible (se comprueba contando los `undo()` en una copia); deshacer todo deja el documento igual al instante en que empezó el primer paso conservado (el instante número `pasos_creados − pasos_conservados`, contando solo los `apply` con `Ok(true)`); y rehacer todo devuelve el final. Bajar el límite con `set_history_limit` cumple lo mismo.

`OutOfMemory` no tiene prueba: no se puede provocar sin reservar memoria real. La reversión sin fallos se comprueba por las propiedades 2 y 3 (con fallos inyectados a mitad de comando) y por revisión del código.

No hay dorado: el deshacer no tiene un comportamiento de programa de referencia que comparar.

## Decisiones

Aprobadas el 2026-10-17; la 5 se cambió respecto a la propuesta.

1. **Los comandos direccionan la imagen con `ImageId`** y `Document::new(image)` + `main_image()` en vez de un documento «de una sola imagen» sin ids. Cuesta un tipo y un parámetro más hoy y evita cambiar `SetPixels` en los hitos 3 y 5.
2. **`Batch` no se anida y los vacíos son errores.** Sin anidar no hay recursión ni límite de profundidad; si un comando compuesto lo necesita más adelante se levanta la regla. Un comando vacío no expresa ninguna edición, así que es un error.
3. **La historia guarda cambios por píxel (~24 bytes), con límite en bytes (64 MiB por defecto) que descarta lo más antiguo y nunca el último paso.** Los comandos masivos del hito 2 guardarán regiones (ver «Notas para el hito 2»).
4. **Atomicidad por reversión, no por validación previa.** Valida y aplica a la vez y, si falla, deshace lo anotado; toda la memoria se reserva antes de modificar nada, así que la reversión no puede fallar.
5. **Un `apply` que no modifica ningún píxel no es un paso, no borra rehacer y devuelve `Ok(false)`.** `apply` devuelve `Result<bool, _>`. «Cambió» se mide escritura a escritura: una escritura que cambia un valor y otra posterior que lo restaura cuentan como cambio. Detectar el efecto neto exigiría reservar memoria después de modificar, y eso rompería la regla de la decisión 4.
6. **`Document: PartialEq` compara el contenido, no la historia.** Es lo que pide la propiedad central («documento idéntico»); comparar la historia haría falso que deshacer devuelve el mismo documento.
7. **`undo` y `redo` devuelven `Result` con `NothingToUndo` y `NothingToRedo`**, no `bool`; `can_undo` y `can_redo` sirven para consultar sin intentar.
8. **El límite cuenta solo la pila de deshacer.** La de rehacer es un subconjunto de lo que estuvo ahí, así que no puede superarla; contar las dos haría que bajar el límite pudiera borrar pasos rehacibles.
9. **`Command` y `DocumentError` son `#[non_exhaustive]`** para poder añadir variantes en los hitos 2 a 5 sin romper `px-ui` ni `px-cli`.
10. **`image(id)` devuelve `Option<&Image>`**, no `Result`: «no existe» es el único fallo posible. `UnknownImage` queda para `apply`, donde sí hay que decir qué id falló.
