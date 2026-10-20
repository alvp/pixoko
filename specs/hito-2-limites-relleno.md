# Hito 2 · Unidad 14c: límites del relleno y de la historia

Estado: **pendiente de visto bueno**. Toca `px-raster` y `px-core` (**solo aditivo**; nada se escribe en ninguno de los dos hasta que apruebes las decisiones D2 a D4) y no necesita cambios en `px-app`. Parte de [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md) («Rellenos»), [`hito-2-relleno.md`](hito-2-relleno.md) (comportamientos 7 y 8, «Para la unidad 14»), [`hito-2-historia-regiones.md`](hito-2-historia-regiones.md) (comportamientos 6 y 7) y [`hito-2-image-filas-bytes.md`](hito-2-image-filas-bytes.md) (de la que solo toma el método de medida).

## Objetivo

Que un relleno, o el paso de historia que deja, en un lienzo de hasta 8192×8192 (`MAX_OPEN_PIXELS = 2²⁶`) no pueda agotar la memoria ni bloquear la interfaz de forma peligrosa: si una región es demasiado fragmentada, error claro **antes de reservar**, aviso de 5 s y documento e historia intactos.

## Lo medido (antes de decidir)

Banco desechable fuera del repositorio (`~/.cache/pixoko-scratch`, borrado al terminar): `px-core` real, `fill_command` real salvo la columna «pila», que usa una **copia** de `fill.rs` con un contador de la pila (y, en la última tabla, el corte propuesto). Release, Linux, 12 hilos, un proceso por caso, una pasada (ruido ±15 %). Pico = máximo de heap vivo medido con un asignador contador (solo en el banco; el repositorio prohíbe `unsafe`), contrastado con el RSS máximo del proceso. `A` es la región, `B` la pared. Semilla en un píxel de la región.

| Patrón (8192²) | Tramos `S` | Píxeles de la región |
| --- | --- | --- |
| **tablero** de 1 píxel (`A` si `x+y` par) | 1 (la región es un píxel) | 1 |
| **uniforme** | 8192 | 67 M |
| **peine** (fila 0 llena; columnas pares `A`, impares `B`) | 33 550 337 (≈ N/2) | 33,6 M |
| **peine doble** (filas pares llenas; en las impares, columnas pares `A`) | 16 781 312 (≈ N/4) | 50 M |
| **disperso** (un `B` cada 5 en x e y) | 2 692 874 | 64 M |

**Tiempo (ms), RGBA / indexado:**

| Patrón | `fill_command` | `apply` | `undo` | `redo` | Total relleno | `dirty_of` (px-ui) |
| --- | --- | --- | --- | --- | --- | --- |
| tablero | 2,8 / 1,0 | 0 | 0 | 0 | 3 / 1 | 0 |
| uniforme | 311 / 254 | 1354 / 1120 | 380 / 249 | 389 / 257 | **1665 / 1374** | — |
| peine | 1464 / 1034 | 1054 / 899 | 209 / 165 | 201 / 167 | **2518 / 1933** | 135 |
| peine doble | 795 / 684 | 1173 / 947 | 291 / 202 | 299 / 202 | 1968 / 1631 | 68 |
| disperso | 366 / — | 1262 / — | — | — | 1628 / — | 11 |

**Memoria (MiB sobre la imagen; la imagen son 256 RGBA y 64 indexado):**

| Patrón | Pico de `fill_command` | Pico de `apply` (comando vivo + trozos) | Historia que queda | RSS máx. sobre la base |
| --- | --- | --- | --- | --- |
| tablero | +8 (el mapa de visitados, `N/8`) | 0 | 0 | +8 |
| uniforme | +8 | +0,2 | 0,2 | +8 |
| peine | +392 (salida 384; la pila solo llega a 4096 tramos) | +384 (comando) **+640** (trozos) = **1024** | **640** | **+1023** |
| peine doble | +200 (**la pila llega a 16 773 121 tramos = 192 MiB**, igual que la salida) | 192 + 320 = 512 | 320 | +512 |
| disperso | +56 (la capacidad de la pila, 48, domina) | 31 + 51 = 82 | 51 | — |

**Con el corte propuesto** (3 000 000 tramos, copia del banco): peine 8192² RGBA y indexado, error a los **110 y 80 ms**, pico +8 MiB; peine doble, error a los 113 ms, pico +56 MiB (pila de 2 998 535 tramos, capacidad 4 194 304 = 48 MiB), RSS +44 MiB. El peine más grande que cabe (2448², `S = 2 995 129`): `fill_command` 111 ms y +35 MiB, `apply` 70 ms con paso de 57,1 MiB (la historia entera ocupa 57,1 MiB; el comando, 34 MiB más mientras vive).

### Lo que dicen las cifras

- **El tablero de 1 píxel no es el peor caso:** con 4 vecinos cada casilla es una región de 1 píxel (1 tramo, 3 ms). Un tramo por píxel solo ocurre en el **peine** (decisión 5 de `hito-2-relleno.md`); con 8 vecinos (hito 4) sí lo sería el tablero.
- **El peor caso de memoria es el peine: ≈ 1,25 GiB de heap en 8192² RGBA** (256 de imagen + 384 de tramos + 640 de trozos; RSS medido 1282 MiB) y la historia **retiene 640 MiB** tras el relleno, diez veces el límite por defecto (64 MiB), porque un paso mayor que el límite se conserva (hito 1). Lo que decía la estimación de la spec del relleno se confirma (a 16384² serían unos 5 GiB: ×4 los tramos).
- **Corrección a `hito-2-relleno.md` (comportamiento 7):** «en la práctica el orden LIFO deja la pila muy por debajo de la cota» es cierto para el peine, **falso para el peine doble**: la pila llega a `S`. La cota demostrada (`S` entradas, hasta 2× por el crecimiento) era la buena; un tope de `S` acota los tres vectores (pila, salida, trozos).
- **El tiempo no depende solo de `S`:** cuesta O(píxeles de la región). El relleno de toda la imagen (8192 tramos) tarda 1,7 s en RGBA porque `apply` recorre cada píxel tres veces (`get_pixel` para contar los trozos, otra vez para anotarlos y `set_pixel` para escribir: ≈ 1,1 a 1,35 s) y `fill_command` lo lee unas cinco veces con `get_pixel` (≈ 0,25 a 0,3 s). Un tope de tramos **no lo reduce** (8192 tramos); sí quita lo que pasa de 2 s: el peine (2,5 s) y el peine doble pasan a un error en ~0,1 s. El peor caso que sigue aceptando el tope es el disperso, **≈ 1,7 s** en esta máquina.
- **`undo` y `redo` no reservan** (0 MiB) y tardan 0,2 a 0,4 s en el peor caso medido; no necesitan tope propio: su coste lo acota el del paso.

## Comportamiento propuesto

1. **Tope de tramos por relleno.** `px_raster::MAX_FILL_SPANS: usize = Document::MAX_FILL_RUNS = 3 000 000` (D1). `fill_command` cuenta los tramos al descubrirlos; al descubrir el **número `MAX + 1`**, aborta con `Err(DocumentError::FillTooFragmented { limit: MAX })`. Se comprueba **antes** de apilar el tramo y por tanto antes de reservar la salida: la pila nunca pasa de `MAX` entradas, la salida de `MAX` tramos, y el mapa de visitados es el mismo de siempre (`N/8`). Una región con exactamente `MAX` tramos se rellena.
2. **Sin hilo aparte y sin comprobación previa por píxeles** (D5): el relleno sigue síncrono. Con el tope, el coste del peor caso que se acepta es ≈ 1,7 s (8192² RGBA, esta máquina) y el de uno que se rechaza, ≤ 0,15 s.
3. **El paso de historia nunca pasa de `MAX` trozos** (D3). `Document::apply` de un `FillSpans` cuenta los trozos en su pasada de solo lectura (comportamiento 7 de la spec de la historia) y, si pasan de `Document::MAX_FILL_RUNS`, devuelve `FillTooFragmented { limit }` **antes de `try_reserve_exact`**. Es lo que acota el peine de la historia: un paso cuesta como mucho `20 · 3 000 000 = 57,2 MiB` (por debajo del límite por defecto de 64 MiB) y el pico de un relleno es **imagen + comando (36 MB) + paso (57 MiB) ≈ +91 MiB**, frente a +1024 MiB. Para un relleno por inundación, trozos = tramos, así que lo que pasa `fill_command` pasa `apply`; el tope de `apply` cubre los `FillSpans` hechos a mano o por futuras herramientas (rectángulo, elipse, hito 4) sobre contenido ruidoso, donde trozos ≫ tramos. Dentro de un `Batch` es `InBatch { index, source }` y se revierte lo anterior, como cualquier error.
4. **Qué se hace al superar el tope de memoria: error, no descartar ni cambiar el límite de la historia.** Descartar pasos viejos no baja el pico (el paso nuevo ya existe) y bajar el límite no impide reservarlo (el último paso nunca se descarta). `set_history_limit` y `trim` no cambian.
5. **El error.** Variante nueva y aditiva `DocumentError::FillTooFragmented { limit: usize }` (`DocumentError` ya es `#[non_exhaustive]`). Texto (`Display`): `el relleno se cancela: la región tiene más de {limit} tramos (demasiado fragmentada)`. Sin `source`.
6. **Cómo lo ve la app: sin cambios en `px-app`.** `canvas_input_with` devuelve el `Err`, `App::finish` lo pasa a `notify(error.to_string())` y el `Notice` dura `NOTICE_SECS = 5` s. El documento, el historial, `can_undo`, el registro de cambios (`take_changes`) y el gesto (`Waiting`) quedan como estaban; el siguiente clic funciona.
7. **Con tope de prueba.** `px_raster::fill_command_with_limit(doc, image, x, y, pixel, max_spans)` (D4) es `fill_command` con el tope como parámetro; `fill_command` la llama con `MAX_FILL_SPANS`. Es lo que permite probar el comportamiento con imágenes de 12×12 y un tope de 5, sin construir 3 millones de tramos.
8. **Orden de comprobaciones de `fill_command`** (el primero que falle gana): `UnknownImage`, `Pixel`, semilla fuera (`None`), `pixel` igual (`None`), y **después** el tope durante la inundación. El tope no cambia nada de lo que ya devolvía.
9. **Sin pánico ni `unsafe`; ninguna reserva por encima de `24 · MAX` bytes** (pila con crecimiento amortizado) ni de `12 · MAX` (salida, exacta) más el mapa. Con `max_spans = 0` cualquier relleno con cambio da error; con `usize::MAX`, el comportamiento actual.

## Casos (12 filas)

Imágenes RGBA salvo que se diga. `A`, `B` colores distintos, `C` de relleno. `fcl(.., L)` es `fill_command_with_limit` con tope `L`. Las filas 1 a 6 y 12 van en `px-raster` (módulo nuevo de `crates/px-raster/tests/fill.rs`, para no enlazar otro ejecutable); 7 a 10, en `px-core` (módulo de `tests/fill_spans.rs`); 11, en `px-ui` (`tests/tool.rs`).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | Peine 5×3 (`S = 7`, fila 9 de la spec del relleno): `fcl(.., 7)` y `fcl(.., 6)` | `Ok(Some)` con los 7 tramos de siempre; `Err(FillTooFragmented { limit: 6 })` |
| 2 | `fcl(.., 0)` en 3×1 todo `A` (1 tramo); `fcl(.., 1)` en tablero 64×64, semilla en el centro | `Err { limit: 0 }`; `Ok` con 1 tramo (el tablero no se acerca al tope) |
| 3 | Con `L = 0`: `ImageId` desconocido; `Indexed(0)` en RGBA; semilla `(-1, 0)`; `C` igual a la semilla | `UnknownImage`; `Pixel { pixel: 0, .. }`; `Ok(None)`; `Ok(None)`: el tope va el último |
| 4 | `flood_fill` (con el tope real) sobre el peine de 16384×368, en un documento que ya tiene un paso de deshacer y uno de rehacer | `Err(FillTooFragmented { .. })` sin llamar a `apply`; imagen, `can_undo`, `can_redo` e `history_bytes` iguales a antes (la pila de rehacer sigue viva) |
| 5 | **Tope real.** Peine de 16384×368 (`S = 3 006 465`) y de 16384×367 (`S = 2 998 273`), semilla `(0, 0)`, con `fill_command` | `Err(FillTooFragmented { limit: 3_000_000 })`; `Ok` con 2 998 273 tramos; con este, `apply` da `Ok(true)`, `history_bytes() ≤ DEFAULT_HISTORY_LIMIT`, `undo` y `redo` restauran la imagen |
| 6 | Constantes | `MAX_FILL_SPANS == Document::MAX_FILL_RUNS`; `20 · MAX_FILL_RUNS ≤ DEFAULT_HISTORY_LIMIT` (aserción en tiempo de compilación) |
| 7 | `px-core`: imagen 16384×368 de un solo valor y `FillSpans` de tramos de 1 píxel en las columnas pares (8192 por fila): 3 000 000 tramos exactos y 3 000 001 | `Ok(true)` con 3 000 000 trozos (y `undo` restaura); `Err(FillTooFragmented { limit: 3_000_000 })` con uno más, sin tocar nada (`can_undo` falso, imagen igual) |
| 8 | `px-core`: pocos tramos, muchos trozos. Imagen 16384×184 con dos valores alternos por columna y un `FillSpans` de 184 tramos de fila completa con `C` distinto de ambos (3 014 656 trozos); y el mismo con 183 tramos (2 998 272 trozos) | `Err(FillTooFragmented { limit: 3_000_000 })` el primero (cuenta trozos, no tramos); `Ok(true)` el segundo, y `undo` restaura |
| 9 | `px-core`: `Batch([S(px), F(demasiados trozos)])` | `Err(InBatch { index: 1, source: FillTooFragmented { limit } })`; el píxel de `S` revertido; historia intacta |
| 10 | `Display` y `source` de `FillTooFragmented { limit: 3_000_000 }` | el texto contiene `3000000`; `source()` es `None`; `Eq` y `Clone` |
| 11 | `px-ui`: `ToolState` con `Tool::Fill` sobre el peine de 16384×368, clic en `(0, 0)`; después el mismo clic sobre un documento de 8×8 | el primero da `Err(FillTooFragmented { .. })`, `take_changes()` vacío, documento e historia iguales y `busy()` falso; el segundo rellena (el estado no se ha roto) |
| 12 | **`#[ignore]`, solo Linux** (una prueba por invocación, porque `VmHWM` es del proceso: `cargo test -p px-raster --test fill -- --ignored --exact <nombre>`): indexado 8192² con peine; indexado 8192² uniforme | peine: `Err` y RSS máximo (`VmHWM` de `/proc/self/status`) con **menos de 128 MiB** sobre el de antes de rellenar; uniforme: `Ok` y menos de 32 MiB sobre la base. Imprime los tiempos; solo falla por encima de 30 s |

## Propiedades (`proptest`, imágenes de 1 a 12 de lado, los casos que fije la spec: 256)

1. **Oráculo del tope.** En los cuatro modos, con tope `L` en `0..=40` y semilla dentro: `fcl(.., L)` da `Err { limit: L }` si y solo si el recorrido ingenuo de la spec del relleno (cola, 4 vecinos) y el recuento de sus tramos máximos da más de `L`; si no, devuelve **los mismos tramos** que `fill_command` sin tope, y `spans.capacity()` es igual a `len ≤ L`.
2. **Atomicidad y orden.** Con `L` cualquiera (incluidos `0` y `usize::MAX`), semilla `i32` arbitraria, `ImageId` y `Pixel` de cualquier modo: sin pánico; el orden `UnknownImage` > `Pixel` > fuera > igual > tope se cumple; con cualquier `Err` el documento, `can_undo`, `can_redo` e `history_bytes` no cambian.
3. **Monotonía.** Si `fcl(.., L)` da `Ok(x)` entonces `fcl(.., L')` con `L' ≥ L` da `Ok(x)`; si da `Err` con `L`, da `Err` con todo `L' < L`.

(`px-core` no lleva propiedad nueva: su tope es una constante de 3 millones, que la fila 7 y la 8 ejercen en el límite; hacerlo configurable solo para probarlo sería más API.)

## Pruebas anteriores que cambian

**Ninguna** (comprobado leyendo y buscando, no ejecutando; se confirmará con la suite entera al implementar). Los rellenos grandes de las pruebas actuales están por debajo del tope: la fila 16 de [`hito-2-relleno.md`](hito-2-relleno.md) tiene como máximo `S = 2 096 129` (peine 2048²) y el resto son imágenes de 12 de lado o menos; `crates/px-core/tests/document.rs` solo comprueba que cada variante de `DocumentError` tiene un mensaje no vacío (`errores_con_display_y_source`) y no enumera todas; ningún `match` de `px-app` ni de `px-ui` sobre `DocumentError` es exhaustivo. Si al implementar alguna necesitara más que cambiar una cifra, **me paro y aviso** (regla 4 de `CLAUDE.md`); si solo cambiara una cifra, va en su commit propio, antes de las pruebas nuevas.

## Orden de commits

1. spec (esta página, con su fila en `INDEX.md` y la nota de «Rellenos» actualizada).
2. adaptación de pruebas anteriores (previsto: ninguno).
3. pruebas en rojo, `test-writer`.
4. lógica: `px-core` (variante, constante, tope en `apply`) y `px-raster` (`MAX_FILL_SPANS`, `fill_command_with_limit`).
5. `px-app`: previsto ninguno.

## Fuera de alcance

- **Rendimiento por píxel** del relleno y de `apply` (tres recorridos con `get_pixel` y `set_pixel`; 1,7 s para toda la imagen a 8192²). Con acceso por filas (`Image::row`, unidad 12) bajaría, pero toca el algoritmo y `execute_fill` y no es un límite: se propone solo si lo quieres (D5).
- El paso de historia de un **trazo larguísimo** (`SetPixels`, 24 B por píxel cambiado) y el de una **conversión de modo** (una imagen entera retenida, hasta 256 MiB a 8192² RGBA): no los he medido ni los toca esta unidad.
- La textura tras el relleno (baldosas visibles regeneradas en el hilo de la interfaz): es de la 14a y no se midió aquí; `dirty_of` cuesta 135 ms con 33 M de tramos y 11 ms con 2,7 M.
- Hilo aparte, cancelación o barra de progreso; tolerancia de color y 8 vecinos (hito 4).

## Decisiones que necesito que confirmes

Las recomendaciones van primero.

1. **D1. Tope de 3 000 000 tramos** (y de trozos por paso), una sola fuente: `Document::MAX_FILL_RUNS` en `px-core` y `MAX_FILL_SPANS` en `px-raster` como alias, para que lo que pasa `fill_command` pase siempre `apply`. Razón: el paso mayor posible (`20 B · 3 M = 57,2 MiB`) cabe en el límite por defecto de la historia (64 MiB); el peor caso que acepta cuesta unos +91 MiB y ≈ 1,7 s; deja pasar el peine de 2448² (`S = 2 995 129`) y la fila 16 de la spec del relleno (peine 2048², `S = 2 096 129`). Alternativas: `2²¹ = 2 097 152` (el peine 2048² pasa por 1000 tramos de margen; pasos de 40 MiB) o `2²² = 4 194 304` (pasos de 80 MiB, por encima del límite por defecto).
2. **D2. Variante nueva `DocumentError::FillTooFragmented { limit }` en `px-core`** (aditiva; **necesita tu aprobación de tocar `px-core`**). Alternativa sin tocar `px-core`: devolver `DocumentError::OutOfMemory` desde `px-raster`; su texto («no hay memoria para guardar el paso en la historia») no dice qué hacer, no permite distinguir el tope de un fallo real de reserva y no cubre D3.
3. **D3. Tope también en `Document::apply`** para `FillSpans` (comprobando los trozos antes de reservar), no solo en `fill_command`. Cubre los `FillSpans` que no vienen de `fill_command` (hito 4: rectángulo y elipse llenos sobre contenido ruidoso, trozos ≫ tramos). Coste: un cambio más en `px-core`, de 3 líneas, y el hito 4 tendrá que elegir otro formato de paso si quiere rellenar un 8192² ruidoso entero. Alternativa: solo el tope de `fill_command`; entonces el peine de la historia queda abierto a cualquier productor de `FillSpans`.
4. **D4. `fill_command_with_limit` pública en `px-raster`** (aditiva; **necesita tu aprobación de tocar `px-raster`**), para probar con topes pequeños. Alternativa: función privada y probar solo con la imagen de 16384×368 (la fila 5 sigue), perdiendo la propiedad 1 y 3 y las filas 1 a 4.
5. **D5. Sin hilo aparte ni comprobación previa; se acepta un bloqueo de ≈ 1,7 s en el peor caso que queda** (toda la imagen de 8192² RGBA, o un disperso de 2,7 M de tramos), medido en una máquina rápida (un hilo, 12 disponibles): en una 3 veces más lenta serían unos 5 s. Una comprobación previa por píxeles de la región (rechazar por área) impediría rellenar toda la imagen, que es un uso legítimo. Si prefieres bajarlo, la unidad corta de rendimiento por píxel (acceso por filas en `execute_fill` y en `extend`) es la vía: no entra aquí salvo que lo pidas.
6. **D6. Una prueba de memoria `#[ignore]` solo en Linux con `VmHWM`** (fila 12) en lugar de un asignador contador, que exigiría `unsafe` fuera de `px-render`. Alcanza para detectar una regresión de cientos de MiB, no de unos pocos.
7. **D7. Texto del aviso** (comportamiento 5): `el relleno se cancela: la región tiene más de 3000000 tramos (demasiado fragmentada)`. ¿Prefieres otro, o con el número con separadores?
8. **D8. Ninguna prueba anterior cambia** y no hay commit de adaptación; si al implementar aparece alguna, me paro.
