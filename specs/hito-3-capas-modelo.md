# Hito 3 · Unidad 3.1: modelo de capas en `px-core` (no aditiva)

Estado: **aprobada** (2026-10-21; condiciones 1 a 7 en «Visto bueno» al final, que **prevalecen sobre cualquier otra frase de la página**). El visto bueno vale como aprobación expresa de `px-core` **solo para los cambios numerados de abajo** (D02): `C-01` a `C-31` (aditivos) y `N-01` a `N-08` (los que no lo son). Si durante la unidad hace falta algo que no esté en esa lista, me paro.

Referencias: filas 3.1 y 3.2 y decisiones D02 y D04 a D08 de [`mapa-funciones.md`](mapa-funciones.md); [`hito-1-document.md`](hito-1-document.md) (comandos como datos, atomicidad, historia por cambios) y [`hito-2-historia-regiones.md`](hito-2-historia-regiones.md) (pasos por partes, coste real). Sin dependencias nuevas, sin `unsafe`. No toca `px-raster`, `px-formats`, `px-ui` ni `px-app` (ver «Llamadores»). No usa el corpus `golden/aseprite/` (lo usan 3.5 y 3.8).

## Objetivo

Que `Document` tenga **capas**: una lista ordenada de capas con nombre, visibilidad y opacidad, cada una con su celda (hoy un fotograma) del tamaño del lienzo, y que las celdas vacías no reserven memoria. Un modo y una paleta para todo el documento. Seis comandos nuevos con deshacer, y los límites y estimaciones que pide D04. La composición (3.2), pintar sobre la capa activa (3.3) y el panel (3.4) vienen después; esta unidad no cambia nada de lo que ve el usuario.

## Lo comprobado (en el código, no en el plan)

1. **La capa única casi existe ya.** `Document` guarda `images: Vec<(ImageId, Image)>` y todo comando lleva el `ImageId` de lo que edita. Una celda puede ser un `ImageId`: `SetPixels`, `FillSpans`, `Batch`, `state_id` y la historia no cambian de forma. Por eso **ninguna firma pública existente cambia**: lo que no es aditivo es el comportamiento y lo interno (N-01 a N-08).
2. **Lo que de verdad no es aditivo** (los motivos, uno a uno, en «Lo que NO es aditivo»): el almacenamiento y la igualdad de `Document`; qué significa `image(id) == None`; los comandos de paleta, que hoy editan una imagen y con capas editan todas a la vez en un paso; escribir en una celda vacía (hoy `UnknownImage`); y `ConvertMode`.
3. **Llamadores.** `main_image()` aparece 489 veces en 59 ficheros fuera de `px-core/src` (la mayoría en pruebas) y `Document::new` 264 veces en 57. Una API de compatibilidad no es opcional: sin ella, esta unidad cambiaría cientos de líneas de pruebas.
4. **Medido en una copia desechable** (fuera de `/tmp` y del repositorio, borrada al terminar), con un prototipo de solo la parte no aditiva (almacenamiento por capas con celda `Option<Image>`, igualdad, paso de paleta de todo el documento, lienzo que sigue a `ConvertMode`; sin las capas nuevas): base 3825 pruebas pasan, 0 fallan, 12 ignoradas; con el prototipo, **3825 pasan, 0 fallan, 12 ignoradas**. Detalle en «Pruebas anteriores que cambian».
5. **Una alternativa medida y descartada.** La primera forma de editar la paleta de todo el documento fue crear un `PaletteStep` por celda más otro para la copia de la paleta del lienzo. Falla **una** prueba anterior, `propiedad_6_coste_del_paso` (`px-core/tests/palette_edit.rs`): un `PermutePalette` de 256 entradas cuesta el doble y se pasa de la cota de 512 bytes. La spec elige por eso **un solo paso de paleta para todo el documento** (N-04).
6. **`px-ui` ya prepara el terreno sin saberlo.** `dirty_of` (en `px-ui/src/dirty.rs`) trata cualquier comando que no conoce como «todo el lienzo sucio» (`_ => BlockSet::full`): los comandos de capa se redibujan enteros, que es correcto aunque grueso. Lo afina 3.3.
7. **Dos funciones de `px-raster` miran `Document::image`.** `draw_line` y `flood_fill` hacen `doc.image(id).ok_or(UnknownImage)`: con una celda vacía darían `UnknownImage`. Hoy no se puede crear una celda vacía; lo resuelve 3.3 (anotado en «Obligaciones para las unidades siguientes»).

## Modelo (las decisiones de fondo)

- **Capa = identidad estable + propiedades + una celda.** `LayerId` (nuevo) identifica la capa; `Layer::cel()` es el `ImageId` de su celda, que existe desde que se crea la capa. La **imagen** de la celda es opcional: sin imagen (`None`) la celda está vacía y no reserva píxeles. Así los comandos siguen direccionando por `ImageId`, y los pasos antiguos de la historia nunca apuntan a otra cosa (los ids no se reutilizan, regla del hito 1).
- **Orden:** `layers()[0]` es el fondo y la última, la de arriba (el orden de pintado y el de `.aseprite`). «Fondo» es **solo una posición**: la capa de la posición 0 no es especial, ni bloqueada, ni opaca por naturaleza, y se puede mover, quitar, ocultar y renombrar como cualquier otra (si es la única, `LastLayer`).
- **Lienzo:** el documento guarda su ancho, alto, modo y paleta (`width()`, `height()`, `mode()`, `palette()`). Toda celda con imagen tiene ese tamaño y ese modo y **la misma paleta que el lienzo**; esa es la invariante que D05 pide probar. Hace falta guardar el lienzo aparte porque, con las celdas vacías, puede no haber ninguna imagen de la que leerlo (por ejemplo, tras quitar la única capa con contenido).
- **Valor de una celda vacía = transparente.** Para poder escribir en ella, el modo tiene que tener un valor transparente: RGBA, grises con alfa e indexado con índice transparente. Grises e indexado sin índice transparente no lo tienen: ahí `AddLayer` da `NoTransparency` (el documento de una capa funciona igual que hoy).
- **Indexado sin opacidad (D06):** en indexado toda capa tiene opacidad 255 y el núcleo lo impone (`IndexedOpacity`). La visibilidad sí vale.
- **Escribir en una celda vacía la materializa.** `SetPixels` y `FillSpans` sobre una celda vacía reservan la imagen (transparente) antes de modificar nada y el paso anota qué celdas materializó; **deshacer las vacía otra vez** (libera la memoria y deja el documento igual que antes) y **rehacer reserva antes de tocar nada** (si no hay memoria, `OutOfMemory` y todo queda como estaba). Si el comando no cambia nada (`Ok(false)`) o falla, las celdas materializadas para él se sueltan. Los errores de materialización (`NoTransparency`, `CelsTooLarge`, `OutOfMemory`) se comprueban antes de ejecutar ningún comando del `Batch`, sobre todas las celdas vacías que nombre, y **`CelsTooLarge` sale antes de reservar nada y de modificar nada** (la cuenta es aritmética: `materialized_bytes() + cel_bytes() × celdas vacías nombradas`; documento, historia y `state_id` intactos); un id que no es de ninguna celda sigue dando `UnknownImage` en su turno, como hoy.
- **La paleta es del documento.** Los seis comandos de paleta de hoy editan el lienzo y **todas** las celdas con imagen en **un solo paso**. Su campo `image` sigue ahí (no cambia ninguna firma): basta que sea una celda cualquiera, vacía o no, del documento; solo se valida que exista (`UnknownImage` si no). Una celda vacía no tiene paleta que editar: al materializarse toma la del lienzo en ese momento.
- **Comandos de capa: exclusivos y de un paso.** Como `ConvertMode` y los de paleta, no van dentro de un `Batch` (`NotBatchable`): un comando de capa es un paso. Sin cambios reales dan `Ok(false)`.
- **Un comando por propiedad** (renombrar, opacidad, visibilidad): datos simples y fáciles de escribir desde texto en la CLI, y un paso de historia pequeño cada uno.
- **`AddLayer` direcciona por posición**, no devuelve el id: el llamador lo lee de `layers()[at]` tras el `Ok(true)`. Los ids nuevos los reparte siempre `Document`.
- **Igualdad (`PartialEq`)** compara las capas en orden (id de la capa, nombre, visibilidad, opacidad, id de la celda y su imagen o «vacía») y no la historia. Una celda vacía **no** es igual a una celda con una imagen toda transparente: son estados distintos (y deshacer devuelve exactamente la vacía).

## Lista exacta de cambios en `px-core`

### Aditivos (se compilan junto a todo lo existente)

| N.º | Dónde | Qué |
| --- | --- | --- |
| C-01 | `document.rs`, `lib.rs` | `pub struct LayerId(u32)`: `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`; `LayerId::from_raw(u32)` y `raw() -> u32`. Solo `Document` reparte ids válidos. Exportado. |
| C-02 | ídem | `pub struct Layer` (campos privados; `Clone`, `Debug`, `PartialEq`, `Eq`) con `id() -> LayerId`, `name() -> &str`, `visible() -> bool`, `opacity() -> u8` (0 a 255) y `cel() -> ImageId`. No contiene la imagen: se pide con `Document::image(layer.cel())`. Exportado. |
| C-03 | `Document` | `pub fn layers(&self) -> &[Layer]` (fondo primero). |
| C-04 | `Document` | `pub fn layer(&self, id: LayerId) -> Option<&Layer>`. |
| C-05 | `Document` | `pub fn width(&self) -> u32`, `height(&self) -> u32`, `mode(&self) -> Mode` y `palette(&self) -> Option<&Palette>` (`Some` solo en indexado): el lienzo. |
| C-06 | `Document` | `pub fn cel_bytes(&self) -> usize`: bytes de píxeles de una celda (`Image::byte_len` del lienzo; sin la paleta). |
| C-07 | `Document` | `pub fn materialized_bytes(&self) -> usize`: suma del contenido de las celdas con imagen (no cuenta la historia). |
| C-08 | `Document` | `pub fn worst_case_bytes(&self) -> usize`: `layers().len() × cel_bytes()`, saturando: lo que ocuparía el documento con todas las celdas materializadas. |
| C-09 | `Document` | `pub fn cells_limit(&self) -> usize` y `pub fn set_cells_limit(&mut self, bytes: usize)`: tope de `materialized_bytes()`. Bajarlo no descarta nada; solo impide materializar más. |
| C-10 | `Document` | `pub const MAX_LAYERS: usize = 256`. |
| C-11 | `Document` | `pub const MAX_LAYER_NAME_BYTES: usize = 255` (bytes UTF-8; cabe en la cadena de `.aseprite`). |
| C-12 | `Document` | `pub const DEFAULT_CELLS_LIMIT: usize = 2 << 30` (2 GiB; cabe en `usize` de 32 bits). |
| C-13 | `Document` | `pub const MEMORY_WARN_BYTES: usize = 1 << 30` (1 GiB): umbral del **aviso** de D04. El núcleo no lo aplica; la interfaz (3.4) avisa si `worst_case_bytes() + cel_bytes() > MEMORY_WARN_BYTES` antes de añadir una capa. |
| C-14 | `Command` | `AddLayer { name: String, at: usize }`: capa nueva, visible, opacidad 255, celda vacía con `ImageId` nuevo, en la posición `at` (`0..=layers().len()`; `len` = encima de todas). |
| C-15 | `Command` | `RemoveLayer { layer: LayerId }`. El paso guarda la capa con su celda (se mueve, no se copia). |
| C-16 | `Command` | `MoveLayer { layer: LayerId, to: usize }`: `to` es la posición final (`0..len`). |
| C-17 | `Command` | `RenameLayer { layer: LayerId, name: String }`. |
| C-18 | `Command` | `SetLayerOpacity { layer: LayerId, opacity: u8 }`. |
| C-19 | `Command` | `SetLayerVisible { layer: LayerId, visible: bool }`. |
| C-20 | `DocumentError` | `UnknownLayer(LayerId)`: el documento no tiene esa capa. |
| C-21 | ídem | `LayerIndex { index: usize, max: usize }`: posición fuera de rango; `max` es la mayor válida. |
| C-22 | ídem | `LastLayer`: `RemoveLayer` de la única capa. |
| C-23 | ídem | `TooManyLayers { limit: usize }`: ya hay `MAX_LAYERS` (o se agotaron los ids, que se reparten con `checked_add`). |
| C-24 | ídem | `LayerName { len: usize, limit: usize }`: nombre de más de `MAX_LAYER_NAME_BYTES` bytes. |
| C-25 | ídem | `NoTransparency { mode: Mode }`: el modo (o el indexado sin índice transparente) no tiene valor transparente; sale de `AddLayer` y de materializar una celda. |
| C-26 | ídem | `IndexedOpacity`: opacidad distinta de 255 en indexado (en `SetLayerOpacity` y en `ConvertMode` a indexado). |
| C-27 | ídem | `MultiLayerConvert { layers: usize }`: `ConvertMode` con más de una capa (hasta 3.5). |
| C-28 | ídem | `CelsTooLarge { needed: usize, limit: usize }`: materializar la celda superaría `cells_limit()`; `needed = materialized_bytes() + cel_bytes()`. |
| C-29 | `Display` / `Error` de `DocumentError` | Un mensaje en español por variante nueva (`source` vacío). |
| C-30 | `Step` (privado) | Variantes de paso para los seis comandos y la lista de celdas materializadas; el nombre anterior se intercambia con el nuevo (sin reservar al deshacer ni rehacer). |
| C-31 | `Step::cost` (privado) | Coste de los pasos nuevos: `size_of` más la capacidad del nombre; `RemoveLayer`, además `size_of::<Image>() + content_bytes()` de la celda guardada. La materialización **no** cuenta en `history_bytes()` (esa memoria es del documento: `materialized_bytes()`). |

### Lo que NO es aditivo

| N.º | Qué cambia de lo que ya existe | Quién lo nota | Por qué no puede ser aditivo |
| --- | --- | --- | --- |
| N-01 | `Document` pasa de `images: Vec<(ImageId, Image)>` a capas con celda `Option<Image>` y lienzo (ancho, alto, modo, paleta). Es interno: ninguna firma pública cambia. | Nadie fuera de `document.rs` y `palette_edit.rs`. | Es la propia estructura. |
| N-02 | `Document: PartialEq` compara ahora las capas (ver «Modelo»), no solo las imágenes por id. | Toda prueba que compare documentos. Con una capa no hay diferencia observable. | Una igualdad que ignorase nombre, orden u opacidad haría que deshacer un `RenameLayer` «devolviera el mismo documento» sin hacerlo. |
| N-03 | `Document::image(id)` devuelve `None` también para una **celda vacía**; hoy `None` significa «id desconocido». `UnknownImage` sigue para ids que no son de ninguna celda. | Solo quien distinga los dos casos; con una capa nunca hay celda vacía. | Una celda vacía no tiene imagen que devolver, y reservarla al leer rompería «las vacías no reservan memoria». |
| N-04 | Los comandos de paleta (`SetPaletteColor`, `MovePaletteEntry`, `PermutePalette`, `AddPaletteEntry`, `RemovePaletteEntry`, `SetTransparentIndex`, `RotateColors`) editan **el lienzo y todas las celdas con imagen** en un solo paso. Interno: `PaletteStep` pasa a ser del documento (`Color` y `Transparent` guardan `antes` y `después`, en vez de intercambiar un `other` con una imagen; `Remove` guarda una máscara por celda). Con una celda, el coste de un paso es el de hoy (más el `Vec` de máscaras, ~56 B, solo en `Remove`). | Con una capa, nadie (medido). Con varias, el coste de `Remove` crece con las celdas con píxeles que usan la entrada. | D05: la paleta es idéntica en todas las celdas tras cualquier operación; una paleta por celda editada por separado dejaría de serlo en el primer olvido. |
| N-05 | `SetPixels` y `FillSpans` sobre una celda vacía la **materializan** (hoy sería `UnknownImage`); el paso anota qué celdas materializó, deshacer las vacía y rehacer las reserva antes de tocar. | Nadie hoy (no hay celdas vacías); desde 3.3, las herramientas. | Es lo que hace que «vacía» no cueste memoria y que deshacer devuelva un documento **igual**. |
| N-06 | `ConvertMode`: con más de una capa da `MultiLayerConvert`; a indexado con opacidad ≠ 255, `IndexedOpacity`; el lienzo (modo y paleta) sigue a la imagen convertida al aplicar, deshacer y rehacer. | Con una capa y opacidad 255, nadie. | Las capas tienen que convertirse **todas** o ninguna (`ConvertAll`, unidad 3.5); hasta entonces solo se permite lo que ya se podía. |
| N-07 | `Document::new(image)` crea una capa `Capa 1` (id 0, visible, opacidad 255, celda `ImageId(0)` con `image`). `main_image()` sigue devolviendo `ImageId(0)` y `image(main_image())` sigue dando la imagen. | Quien lea `layers()` (nadie todavía). | Es el documento mínimo con el nuevo modelo. |
| N-08 | `Document` gana la regla «`NotBatchable`» para los seis comandos nuevos en `execute_leaf`, y `apply` los despacha antes de contar escrituras, como hoy `ConvertMode` y los de paleta. | Nadie. | Mantener `apply` no recursivo y los pasos de capa de un solo tipo. |

**Resumen.** Ninguna firma pública existente cambia. Lo no aditivo es el comportamiento de `==`, de `image(id)`, de los comandos de paleta, de la escritura sobre celdas vacías y de `ConvertMode`, más el almacenamiento interno. N-02, N-03, N-04, N-05, N-06 y N-07 son los que cambian lo que ya existe y por eso piden tu aprobación explícita; N-01 y N-08 son internos.

## Límites (decisión 11)

| Qué | Valor | Se aplica |
| --- | --- | --- |
| Capas | `MAX_LAYERS` = 256 | Núcleo (`TooManyLayers`). |
| Nombre | 255 bytes | Núcleo (`LayerName`), en `AddLayer` y `RenameLayer`. |
| Celdas materializadas | `cells_limit()`, por defecto 2 GiB | Núcleo (`CelsTooLarge`), al materializar. El `Vec` se llena a ceros: un límite evita que Linux responda al exceso con el OOM-killer en vez de con un error. |
| Aviso de presupuesto | `MEMORY_WARN_BYTES` = 1 GiB sobre el peor caso tras añadir | **Interfaz** (3.4); el núcleo da `worst_case_bytes()` y `cel_bytes()`. |
| Por imagen | `Image::MAX_BYTES` = 1 GiB (sin cambios) | `Image`. |

Ejemplos del aviso: 8192² RGBA, celda de 256 MiB: avisa al añadir la quinta capa (peor caso 1,25 GiB); 4096² RGBA (64 MiB), a la decimoséptima; 1024² RGBA (4 MiB), nunca (antes llega a las 256). Con 8192² RGBA (256 MiB por celda) **caben 8 celdas materializadas** bajo el tope de 2 GiB; la novena escritura sobre una celda vacía da `CelsTooLarge { needed: 2304 MiB, limit: 2048 MiB }` sin reservar nada.

**Historia y capa borrada.** `RemoveLayer` mueve la celda al paso: no copia ni reserva píxeles, y su coste es `content_bytes()` de la celda (D04, «coste de historia de la capa borrada»). Una celda de 256 MiB supera el límite de 64 MiB de la historia: por la regla de siempre («nunca se descarta el último paso») se conserva hasta el siguiente cambio y entonces se descarta (y se libera). El pico de memoria es el documento más ese paso, no el doble.

## Estado, «modificado» y guardado

- **`state_id`.** Los seis comandos nuevos pasan por el mismo `commit` que los demás: un `Ok(true)` da un identificador nuevo y nunca reutilizado; `Ok(false)` y los errores no lo tocan; `undo` y `redo` los recorren. Materializar al pintar es parte del paso de ese `SetPixels` o `FillSpans`, no un paso aparte.
- **«Modificado».** `Session` compara `state_id() != saved_id`: renombrar, mover, cambiar la opacidad o la visibilidad de una capa y añadir o quitar capas marcan el documento como modificado, y deshacerlos hasta el estado guardado lo desmarca. No cambia ninguna línea de `px-app`.
- **Guardado PNG.** **Esta unidad no lo cambia**: sin interfaz de capas, la aplicación sigue con una sola capa y guarda `image(main_image())`, exactamente como hoy. Obligación para quien lo cambie (ver más abajo): el PNG será la **composición visible** (con la composición de 3.2), antes de que 3.4 deje a nadie crear una segunda capa; combinar, aplanar y exportar el PNG aplanado como operaciones del documento llegan en 3.5. Con más de una capa (solo alcanzable desde la API del núcleo y las pruebas), `image(main_image())` es la celda de la capa 0 mientras exista.

## Migración por etapas (el workspace sigue verde en cada commit, salvo el de pruebas en rojo)

La compatibilidad es la propia API de hoy: `Document::new`, `main_image()`, `image(id)`, `apply`, `undo`, `redo`, `state_id`, `history_bytes` y los comandos existentes **no cambian de firma**. Con una sola capa implícita desde el primer commit de lógica, no hace falta ninguna API temporal ni ningún `#[deprecated]` (que además rompería `clippy -D warnings` en los llamadores). Lo único transitorio es `main_image()`: queda con un `TODO(3.3)` en su comentario de documentación y se retira cuando 3.3 migre a los llamadores.

| Commit | Contenido | Estado del workspace |
| --- | --- | --- |
| 1. spec | Esta página y su fila en `INDEX.md`. | Verde. |
| 2. adaptación de pruebas anteriores | **No habrá**: ninguna prueba anterior cambia (medido; «Pruebas anteriores que cambian»). Si al implementar una fallara, me paro. | — |
| 3. pruebas en rojo (`test-writer`) | `crates/px-core/tests/layers.rs`: una prueba por fila y por propiedad, escritas solo desde esta spec. | **Rojo a propósito** solo en `cargo test -p px-core --test layers` (no compila: usa la API nueva). El resto del workspace sigue verde. CLAUDE.md permite este commit rojo. |
| 4. lógica A: «celdas y lienzo» | N-01, N-02, N-04, N-07 y la parte de N-06 que hace seguir al lienzo a `ConvertMode`. Sin API nueva: una sola capa implícita. | Verde, con las 3825 pruebas anteriores sin tocar. |
| 5. lógica B: «capas y límites» | C-01 a C-13, C-20 a C-29 en lo que usan; N-03, N-05; `AddLayer` y `RemoveLayer` con sus pasos (C-14, C-15, C-30, C-31 parciales); `ConvertMode` con `MultiLayerConvert` e `IndexedOpacity`. | Verde. |
| 6. lógica C: «propiedades y orden» | `MoveLayer`, `RenameLayer`, `SetLayerOpacity`, `SetLayerVisible` (C-16 a C-19), N-08, y lo que falte de C-30 y C-31. | Verde. `layers.rs` pasa entero. |
| 7. `px-app` | **No hay.** `px-ui` y `px-app` no necesitan cambios (los compila y los prueba el cierre). | — |
| Cierre | `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check` y el `license-auditor` una sola vez sobre el rango (cambia `px-core`). | — |

Si prefieres **verde estricto también en el commit 3**, la alternativa es repartir `layers.rs` en tres ejecutables y commitear cada uno junto con su etapa de lógica (se pierde «pruebas antes que lógica» en commits separados). Recomiendo lo de arriba.

## Llamadores en `px-ui`, `px-app` y `px-raster` (lista por lectura; no se tocan en 3.1)

Código de producción (el resto son pruebas, 489 usos de `main_image()` en 59 ficheros, que siguen compilando y pasando):

| Fichero | Uso | Quién lo migra |
| --- | --- | --- |
| `px-app/src/session.rs` | 15 `main_image()` y 14 `.image(` (líneas 122 a 805: color activo, cuentagotas, guardado, conversión, alto y ancho para el lienzo). | 3.3 (capa activa) y, el guardado, antes de 3.4. |
| `px-app/src/lib.rs` | 9 `main_image()` y 10 `.image(` (líneas 173 a 1198: lienzo, textura, herramientas). | 3.3. |
| `px-ui/src/tool.rs` | `doc.image(image)` con `image: ImageId` por parámetro (líneas 174 a 319): el cuentagotas y el borrador leen la celda. | 3.3. |
| `px-ui/src/stroke.rs` | `Stroke::commit(doc, image)`. | 3.3. |
| `px-ui/src/dirty.rs` | `dirty_of`: `_ => BlockSet::full` para los comandos de capa. | 3.3 (afinar por celda). |
| `px-raster/src/draw.rs:19`, `fill.rs:39` | `doc.image(id).ok_or(UnknownImage)`: una celda vacía daría `UnknownImage`. | 3.3 (materializar o saltar la herramienta). |
| `px-formats`, `px-golden` (producción) | Ninguno (`write_png` y `read_png` trabajan con `Image`). | — |

## Pruebas anteriores que cambian (lista exacta, medida)

**Ninguna.** Medido el 2026-10-21 en una copia de `HEAD` fuera de `/tmp` y del repositorio (`~/.cache/pixoko-scratch-3-1`, con su propio `CARGO_TARGET_DIR`; **borrada al terminar**):

| Paso | Resultado |
| --- | --- |
| Copia sin cambios, `cargo test -q --workspace --locked --no-fail-fast` | 3825 pasan, 12 ignoradas, 0 fallan. (A la primera salieron 11 fallos de `px-golden` y `px-ui/tests/text.rs` porque la copia no tenía `.github` ni `.gitattributes`; con ellos, 0.) |
| Prototipo: N-01, N-02, N-04 (un paso de paleta de todo el documento), N-07 y el lienzo que sigue a `ConvertMode`; sin las capas nuevas | **3825 pasan, 12 ignoradas, 0 fallan.** Ninguna prueba cambia, ni siquiera en firmas, literales o cifras. |
| Alternativa descartada: un `PaletteStep` por celda más otro para el lienzo | Falla `propiedad_6_coste_del_paso` (`px-core/tests/palette_edit.rs`): el coste del paso se duplica y pasa de 512 bytes. Esa prueba tendría que cambiar de **cota**, no de cifra; por eso no se elige. |

Lo que el prototipo no cubre, y por qué no puede afectar a pruebas anteriores: las variantes nuevas de `Command` y `DocumentError` (los `match` de otros crates tienen rama comodín porque son `#[non_exhaustive]`; lo comprueba el compilador) y N-03, N-05 y N-06 (solo se activan con celdas vacías o con más de una capa, que ninguna API anterior puede crear). Cada commit de lógica corre `cargo test --workspace --locked`; **si alguna prueba anterior falla, me paro y te lo cuento** (regla 4).

## Casos (12 filas; una prueba por fila en `crates/px-core/tests/layers.rs`)

Notación: `D` es un documento RGBA 4 × 3 recién creado; `A` su capa 0; `cel(X)` es el `ImageId` de la celda de la capa `X`; `S(c, px)` es `SetPixels` en la celda `c`; `F(c, spans, px)` es `FillSpans`; `t` es el píxel transparente `Rgba(0,0,0,0)` y `c1`, `c2` colores opacos distintos.

| # | Operación | Resultado |
| --- | --- | --- |
| 1 | `Document::new(img)` | Una capa `Capa 1`, `LayerId(0)`, visible, opacidad 255, `cel == main_image()` con `image(cel) == Some(img)`; `width`, `height`, `mode` y `palette` los de `img`; `cel_bytes() == 48` y `materialized_bytes() == 48`; `worst_case_bytes() == 48`; `state_id() == 0`, sin deshacer; `cells_limit() == DEFAULT_CELLS_LIMIT` |
| 2 | `AddLayer { name: "B", at: 1 }`; `AddLayer { at: 0 }`; `AddLayer { at: 2 }` (ahora hay 3: cabe hasta `at == 3`); deshacer y rehacer | `Ok(true)`; la capa nueva está en esa posición, visible, opacidad 255, celda vacía (`image(cel) == None`) con `ImageId` distinto de todos; `materialized_bytes()` no cambia; un `undo` quita la última añadida y `layers()` es igual al de antes; `redo` la repone con los mismos ids |
| 3 | En `D` con `B` vacía (fila 2): `S(cel(B), [(0,0,c1)])`; `undo`; `redo`. Además `F(cel(B), fila 0, c1)`, y `Batch([S(cel(A), …), F(cel(B), …)])` con dos capas vacías y una con imagen | Materializa: `image(cel(B))` pasa a `Some` (a `c1` en `(0,0)`, `t` en el resto) y `materialized_bytes()` sube `cel_bytes()` por celda vacía tocada; **un** `undo` deja `D` **igual** (celda otra vez vacía y los bytes de antes) y un `redo` repone la imagen; `S(cel(B), [(0,0,t)])` sobre una celda vacía es `Ok(false)` y sigue vacía; `S(cel(B), [(9,9,c1)])` es `Err(Pixel { .. })` y sigue vacía; el `Batch` materializa las dos y es un solo paso |
| 4 | `RemoveLayer` de una capa con imagen; `undo`; `redo`; `RemoveLayer` de la única capa; de un `LayerId` que no existe; `AddLayer` tras quitar | `Ok(true)`; `undo` devuelve **el mismo** `LayerId`, el mismo `ImageId`, la misma imagen y la misma posición (documento igual); `history_bytes()` tras el borrado es al menos el `content_bytes` de la celda; la única capa da `Err(LastLayer)`; el id desconocido, `Err(UnknownLayer(..))`; la capa añadida después tiene un `LayerId` y un `ImageId` **mayores que cualquiera visto** (los ids no se reutilizan) |
| 5 | Con 3 capas `[A, B, C]`: `MoveLayer { A, to: 2 }`; `MoveLayer { A, to: 2 }` otra vez; `MoveLayer { A, to: 3 }`; deshacer y rehacer | Orden `[B, C, A]` y `Ok(true)`; la segunda vez `Ok(false)` sin paso; `to: 3` es `Err(LayerIndex { index: 3, max: 2 })`; `undo` devuelve `[A, B, C]` y `redo` `[B, C, A]`; las celdas no cambian de id |
| 6 | `RenameLayer`, `SetLayerOpacity { 128 }`, `SetLayerVisible { false }`, cada uno con el mismo valor otra vez; nombre de 255 y de 256 bytes; `LayerId` desconocido; en un documento indexado, opacidad 128 y 255, y visibilidad | Cada cambio es `Ok(true)` y un paso; repetir el mismo valor, `Ok(false)` y sin paso; `undo` y `redo` los recorren; 255 bytes vale y 256 es `Err(LayerName { len: 256, limit: 255 })`; `Err(UnknownLayer(..))`; en indexado 128 da `Err(IndexedOpacity)`, 255 es `Ok(false)` y la visibilidad vale |
| 7 | `AddLayer` en: grises; indexado sin índice transparente; indexado con él; con 256 capas (lienzo 1 × 1); con `at > len`; con un nombre de 256 bytes; y con varios fallos a la vez | `Err(NoTransparency { mode: Gray })` en grises (la 3.4 mostrará «convierte a gris con alfa») y `Err(NoTransparency { mode: Indexed })` en indexado sin índice transparente; `Ok(true)` con índice transparente; `Err(TooManyLayers { limit: 256 })`; `Err(LayerIndex { index, max })`; `Err(LayerName { .. })`; el orden de comprobación es nombre, transparencia, número de capas y posición; documento, `state_id` e historia intactos en todos los fallos |
| 8 | Documento indexado de paleta de 4 entradas con tres capas: dos con imagen (con píxeles de la entrada 2 en las dos) y una vacía; cada comando de paleta (`SetPaletteColor`, `SetTransparentIndex`, `AddPaletteEntry`, `MovePaletteEntry`, `PermutePalette`, `RemovePaletteEntry { index: 2, into: 0 }`, `RotateColors`); después materializar la vacía | En cada caso, `Ok(true)`, **un** paso y **todas** las paletas de las celdas con imagen son iguales entre sí y a `palette()`; los píxeles de las dos celdas se remapean igual; `undo` y `redo` son exactos; la celda vacía sigue vacía y, al materializarla, toma la paleta que hay entonces; en un documento de **una** capa el coste de cada paso de paleta es el de hoy (`≤ 512` B salvo `Remove`) |
| 9 | `ConvertMode` en: un documento de una capa (RGBA → indexado, y de vuelta); con dos capas; a indexado con la capa a opacidad 128; deshacer y rehacer | Una capa y opacidad 255: igual que hoy, y `mode()` y `palette()` siguen a la imagen en `apply`, `undo` y `redo`; dos capas: `Err(MultiLayerConvert { layers: 2 })`; opacidad 128: `Err(IndexedOpacity)`; en los dos errores el documento queda intacto |
| 10 | `Batch([S(cel(A),..), F(cel(B vacía),..)])`; un `Batch` con una orden de capa dentro; `Batch([F(cel(B vacía), ok), S(cel(A), fuera de rango)])` | El primero es un solo paso (un `undo` los deshace los dos y `B` vuelve a estar vacía); el segundo, `Err(InBatch { index: 0, source: NotBatchable })`; el tercero, `Err(InBatch { index: 1, source: Pixel { .. } })` y **`B` no queda materializada** (documento igual al anterior) |
| 11 | `set_cells_limit(cel_bytes())` y materializar una segunda celda; `redo` tras bajar el límite; `worst_case_bytes`; constantes | `Err(CelsTooLarge { needed: 96, limit: 48 })`, documento e historia intactos y ninguna celda a medias; un `redo` que necesitaría reservar sobre el nuevo límite da `Err(CelsTooLarge { .. })` y el paso sigue en la pila de rehacer (se puede rehacer tras subir el límite); `worst_case_bytes() == layers().len() × cel_bytes()`; `MAX_LAYERS`, `MAX_LAYER_NAME_BYTES`, `DEFAULT_CELLS_LIMIT` y `MEMORY_WARN_BYTES` valen 256, 255, 2 GiB y 1 GiB |
| 12 | Cada comando de capa con `Ok(true)`, `Ok(false)` y error; deshacer y rehacer; comparar documentos | `state_id` cambia en cada `Ok(true)` a un valor **nuevo** y no en `Ok(false)` ni en errores; `undo` y `redo` lo recorren como con los demás pasos; `==` distingue nombre, orden, opacidad, visibilidad, ids y «vacía» frente a «imagen transparente»; no mira la historia (un documento con el mismo contenido e ids y otra historia es igual) |

## Propiedades (5 con la añadida en el visto bueno; `proptest`, 256 casos cada una, imágenes hasta 4 × 4 en cualquier modo)

Generadores: secuencias de 0 a 30 comandos entre `SetPixels`, `FillSpans`, `Batch`, los seis de capa y los de paleta, sobre documentos de 1 a 4 capas (algunas celdas vacías), con fallos inyectados (capa o celda desconocida, posición fuera, nombre largo, opacidad en indexado, modo sin transparencia). Los comandos que fallan se ignoran donde se dice.

1. **Central con capas.** Tras aplicar la secuencia, `undo()` hasta `Err` se ejecuta tantas veces como `Ok(true)` y deja un documento **igual** al inicial, con el mismo `materialized_bytes()`, el mismo `state_id()` y `history_bytes() == 0`; después `redo()` hasta `Err` deja uno igual al final, con el mismo `state_id()` y los mismos `materialized_bytes()`.
2. **Invariantes tras cualquier operación** (aquí vive la prueba de D05). Después de **cada** `apply` (válido, inválido o sin cambios), `undo`, `redo` y `set_history_limit`/`set_cells_limit`: (a) toda celda con imagen tiene el ancho, alto y modo del lienzo y **todas las paletas son idénticas** entre sí y a `palette()`; (b) en indexado, toda opacidad es 255; (c) los `LayerId` y los `ImageId` de las celdas son únicos, y un id que ya salió en algún `layers()` anterior sigue siendo de la misma capa o ya no existe, nunca de otra; (d) hay entre 1 y 256 capas y ningún nombre pasa de 255 bytes; (e) `materialized_bytes()` es la suma de las celdas con imagen y no pasa de `cells_limit()` salvo que se haya bajado el límite después.
3. **Atomicidad y ausencia de pánicos.** Con comandos arbitrarios (cualquier `LayerId` y `ImageId`, cualquier `usize` en `at` y `to`, nombres de cualquier longitud y contenido, `Batch` anidados o con comandos de capa, listas vacías, `set_cells_limit` con cualquier `usize`): `apply`, `undo` y `redo` nunca hacen pánico y, si devuelven `Err` o `Ok(false)`, el documento es igual a una copia previa y `can_undo`, `can_redo`, `history_bytes`, `state_id` y `materialized_bytes` no cambian.
4. **La tabla de capas contra un modelo.** Un modelo independiente (un `Vec` de capas con nombre, visibilidad, opacidad y ids, y un contador de ids) ejecuta las mismas órdenes de capa con las reglas de esta spec: tras cada orden, `layers()` coincide con el modelo en orden, ids, nombre, visibilidad y opacidad; el `bool` de `apply` y el error son los esperados (con el orden de comprobación de la fila 7); y `undo` y `redo` siguen al cursor de una lista de instantáneas del modelo.

5. **Quitar una capa y deshacer devuelve el mismo documento** (añadida en el visto bueno). Para cualquier documento de 1 a 4 capas (algunas con imagen, otras vacías, en cualquier modo) y cualquier capa que no sea la última: `RemoveLayer` y después `undo()` dan un documento **igual** al de antes, con los mismos `LayerId` e `ImageId` de **todas** las capas en el mismo orden, el mismo `materialized_bytes()` y el mismo `state_id()`; `redo()` vuelve a quitarla y el documento es igual al de justo después del borrado.

Sin dorados (el corpus de Aseprite lo usan 3.5 y 3.8, no esta unidad). `OutOfMemory` no tiene prueba (no se puede provocar sin reservar memoria real); la reversión sin fallos y la reserva previa en `redo` se comprueban por las propiedades 1 a 3 y por revisión del código.

## Obligaciones para las unidades siguientes (no se hacen aquí)

- **3.2:** la composición lee `layers()`, `image(layer.cel())`, `visible()` y `opacity()`; en indexado solo visibilidad e índice transparente. Añadirá a `Layer` el modo de mezcla (campo y comando nuevos: aditivo, no se cambia nada de esta unidad).
- **3.3:** migrar los llamadores de `main_image()` (tabla de arriba) a la capa activa y retirar `main_image()`; escribir el bloqueo de capa; materializar o saltar en `draw_line` y `flood_fill`; que `dirty_of` conozca los comandos de capa; **el guardado PNG pasa a ser la composición visible**, antes de que 3.4 deje crear capas.
- **3.4 (no puede empezar hasta cerrar la 3.3; mientras la interfaz no exponga comandos de capa, `px-app` no lleva guardia de guardado):** el aviso de presupuesto (`worst_case_bytes() + cel_bytes() > MEMORY_WARN_BYTES`) y atenuar los controles de opacidad en indexado con el motivo «convierte a RGBA»; el comando `DuplicateLayer` (necesita copiar una celda; es aditivo).
- **3.5:** `ConvertAll` (levanta `MultiLayerConvert`), combinar, aplanar y exportar el PNG aplanado.

## Fuera de alcance

Composición y modos de mezcla (3.2); capa activa, herramientas y bloqueo (3.3); panel y menú `Capa` (3.4); duplicar, combinar y aplanar (3.4, 3.5); fotogramas (hito 5: una columna más de la misma tabla); `.aseprite` (3.8); serialización de comandos (hito 8); un `Layer` con grupos o capas de referencia.

## Decisiones que necesito que confirmes

Cada número se refiere a los cambios `C-` y `N-` de arriba; confirma o corrige uno a uno.

1. **`LayerId` aparte de `ImageId`**, con la celda de cada capa direccionada por un `ImageId` que existe desde que se crea la capa y una imagen opcional (C-01, C-02, N-01). Alternativa descartada: usar el `ImageId` como id de capa; obligaría a que una capa vacía no tuviera identidad.
2. **Orden: índice 0 = fondo** (C-03).
3. **Los comandos direccionan la capa por `LayerId`; `AddLayer` y `MoveLayer` por posición y sin devolver el id** (C-14 a C-19).
4. **Seis comandos, uno por propiedad**, y no `SetLayerProps` ni `DuplicateLayer` como dice el mapa: el mapa listaba también `DuplicateLayer` (pasa a 3.4, necesita copiar), mezcla (3.2) y bloqueo (3.3); los tres se añaden después sin cambiar nada de esto. **Esto se desvía de la fila 3.1 del mapa.**
5. **Los comandos de capa no van en un `Batch`** (`NotBatchable`, N-08). Si 3.5 necesita combinar varios en un paso, se levanta la regla allí.
6. **Escribir en una celda vacía la materializa, deshacer la vacía otra vez y la igualdad distingue «vacía» de «transparente»** (N-05, N-02). Cuesta que `redo` pueda necesitar reservar memoria (con reserva previa). Alternativa: no vaciar al deshacer, y entonces `undo` no devuelve un documento **igual**.
7. **Más de una capa exige un valor transparente** (`NoTransparency` en grises e indexado sin índice transparente, C-25). Alternativa: capas sin transparencia en esos modos, que no tienen sentido.
8. **Indexado ⇒ opacidad 255, impuesto en el núcleo** (`IndexedOpacity` en `SetLayerOpacity` y en `ConvertMode` a indexado, C-26, N-06). Alternativa: guardarla y que la composición la ignore; al volver a RGBA reaparecería sola.
9. **La paleta es del documento: sus comandos editan todas las celdas en un solo paso y su campo `image` solo se valida** (N-04). Un paso por celda está descartado con medida (rompe `propiedad_6_coste_del_paso`).
10. **`ConvertMode` con más de una capa se rechaza hasta 3.5** (`MultiLayerConvert`, C-27, N-06). Con una capa y una celda vacía (solo alcanzable tras quitar la capa con contenido de un documento de dos) da `UnknownImage`, como hoy; 3.5 lo trata con `ConvertAll`.
11. **Límites:** 256 capas, nombre de 255 bytes, 2 GiB de celdas materializadas (ajustable con `set_cells_limit`), aviso a 1 GiB de peor caso decidido por la interfaz (C-09 a C-13, C-28). Son cifras mías: dime si quieres otras.
12. **La igualdad de documentos incluye los ids** de capa y de celda (N-02): dos historias distintas con el mismo contenido pueden dar documentos distintos si repartieron ids distintos, porque los comandos no son intercambiables entre ellos. Alternativa: ignorar los ids; entonces la propiedad 2(c) es la única que los vigila.
13. **`Document::new` crea una capa llamada `Capa 1`** (texto en español en el núcleo, N-07). Las traducciones (hito 8) cambian la interfaz, no los nombres ya guardados en un fichero.
14. **`main_image()` se queda hasta 3.3** (constante `ImageId(0)`, con `TODO(3.3)` en la documentación) en vez de una API de compatibilidad nueva (N-07). Con dos capas, y si se quitó la capa 0, `image(main_image())` es `None`: no es alcanzable desde la interfaz hasta 3.4.
15. **El guardado PNG no cambia en 3.1** y pasa a ser la composición visible en 3.3, antes de 3.4. Si prefieres un guardia en 3.1 (`Session::save` rechaza con aviso si hay más de una capa), cuesta un cambio en `px-app` que esta spec no hace.
16. **Un commit de pruebas en rojo** (`layers.rs`) y verde en todos los demás; alternativa de verde estricto en el commit 3 descrita arriba.
17. **Un ejecutable de pruebas nuevo, `layers.rs`**, en vez de un módulo de `document.rs` (que ya pasa de mil líneas y tiene generadores incompatibles: sin capas ni celdas vacías). Es un solo enlazado más.
18. **`test-writer` sí** en esta unidad (es `px-core`); `license-auditor` una sola vez al cerrar (cambia `px-core`; sin dependencias nuevas).

**Qué cambios de `px-core` no pueden ser aditivos, en una línea:** N-02 (igualdad), N-03 (`image(id) == None` para celdas vacías), N-04 (paleta de todo el documento en un paso), N-05 (escribir en una celda vacía la materializa), N-06 (`ConvertMode`) y N-07 (`Document::new` trae una capa), más los internos N-01 y N-08. Ninguna firma pública existente cambia y ninguna prueba anterior cambia (medido).

## Visto bueno (2026-10-21)

Aprobados **C-01 a C-31 y N-01 a N-08 tal como están**; si hace falta cualquier cambio fuera de esa lista, parar y avisar. Las 18 decisiones, aprobadas. Condiciones (modo ahorro de CLAUDE.md):

1. **Gris:** añadir una capa a un documento gris da un error explícito (`NoTransparency { mode: Gray }`); la fila 7 lo fija y la 3.4 mostrará «convierte a gris con alfa».
2. **Memoria:** una escritura que materializaría una celda por encima de 2 GiB da `CelsTooLarge` **antes de reservar y de modificar nada** (documento, historia y `state_id` intactos). Con 8192² RGBA caben 8 celdas materializadas (arriba, «Límites»).
3. **La 3.4 no puede empezar hasta cerrar la 3.3**, y `px-app` no lleva guardia de guardado mientras la interfaz no exponga los comandos de capa. Escrito aquí y en `mapa-funciones.md`.
4. **«El fondo es el índice 0» es solo una posición**: sin capa especial ni bloqueada (arriba, «Modelo»).
5. **Propiedad 5 añadida:** quitar una capa y deshacer devuelve el documento igual, con los mismos `LayerId` e `ImageId`. Con ella, la unidad tiene 5 propiedades (el tope de 4 de CLAUDE.md se amplía a petición expresa).
6. **`tests/layers.rs`, escrito por el `test-writer`, con un máximo de unas 60 pruebas** (hoy: 12 filas, 5 propiedades; el resto, si hiciera falta, son variantes de las filas).
7. **El desvío del mapa** (`DuplicateLayer` a 3.4, mezcla a 3.2, bloqueo a 3.3, sin `SetLayerProps`) queda reflejado en `mapa-funciones.md` y `docs/PLAN-RESUMEN.md`.

Orden de trabajo: commit de spec; commit de pruebas en rojo (solo `tests/layers.rs` deja de compilar); lógica A, B y C con el workspace verde en cada uno y sin API temporal; cierre completo; `license-auditor` una vez al final. Sin `golden/aseprite/`. Informe final de 20 líneas como máximo.

## Corrección posterior (al cerrar la unidad; la única)

Estado: **implementada** (commits `fd33133` spec, `1b5907c` pruebas, `e8a604e`, `7f4edea` y `88d6426` lógica A, B y C, `aa14c1c` ajuste de estilo de las pruebas). Lo que la implementación precisó, sin salirse de `C-01..C-31` ni `N-01..N-08`:

1. **Orden de comprobación en los comandos de capa:** `UnknownLayer` gana siempre a los demás errores (también en `RemoveLayer` de la única capa con un id desconocido); después, `MoveLayer`: `LayerIndex`; `RenameLayer`: `LayerName`; `SetLayerOpacity`: `IndexedOpacity`; y por último el «sin cambios» (`Ok(false)`).
2. **`CelsTooLarge` dentro de un `Batch`** sale sin envolver en `InBatch` (se comprueba antes de ejecutar ningún comando) y `needed = materialized_bytes() + cel_bytes() × celdas vacías nombradas` (con una sola celda es lo que dice C-28).
3. **`materialized_bytes()` (C-07) cuenta solo los bytes de píxeles**, como `cel_bytes()`, sin las paletas; el coste en la historia del `RemoveLayer` sí cuenta el contenido de la celda entero.
4. **El lienzo guarda su paleta en una plantilla de 1 × 1** con el modo de las celdas, que recibe los mismos pasos de paleta (un solo paso, sin copia de coste: la cota de 512 B de `propiedad_6_coste_del_paso` sigue valiendo); el paso de `ConvertMode` guarda también la plantilla del otro lado (no se cuenta en su coste: ≤ 1 KiB). `ImageId(u32::MAX)` queda reservado para ella.
5. **Deshacer no cuesta más con más capas** (medido, release, `SetPixels` de 10⁶ píxeles en una celda de 1024² RGBA, tiempo de `undo`/`redo`): 1 capa 5,1/4,8 ms, 64 capas 4,3/4,4 ms, 256 capas 4,7/4,4 ms.
6. **Cifras de memoria:** 8192² RGBA, 256 MiB por celda: caben 8 celdas materializadas bajo el tope de 2 GiB; la novena escritura sobre una celda vacía da `CelsTooLarge { needed: 2304 MiB, limit: 2048 MiB }` antes de reservar; el aviso de 1 GiB de peor caso salta al añadir la quinta capa.
7. **Medido en el cierre:** 3873 pruebas pasan (3825 anteriores sin tocar + 48 nuevas), 12 ignoradas; `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings` y `cargo deny check` limpios. `px-ui`, `px-app` y `px-raster` no cambian.
