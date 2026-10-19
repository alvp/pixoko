# Hito 2 · Unidad 13: lectura de PNG sin reserva anticipada (cota de DEFLATE)

Estado: **aprobada** (2026-10-19, con **alcance reducido a la parte 1**, la cota de lectura, y los matices de «Matices del visto bueno», que prevalecen sobre cualquier otra frase de la página). El escritor por flujo (`write_png_to`, `PngError::Io`, `write_atomic_with`) queda **pospuesto**: cifras y decisión en [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «PNG». Cambia solo `px-formats` (sin `px-core`, `px-raster` ni `px-app`), sin `unsafe` ni dependencias nuevas (`docs/DEPENDENCIES.md` no cambia). Parte de [`hito-1-png.md`](hito-1-png.md) (comportamiento 7 y «Para el hito 2»), [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md) y [`hito-1-fuzz.md`](hito-1-fuzz.md).

## Objetivo

`read_png` rechaza, **antes de reservar nada**, los ficheros cuyos datos comprimidos no pueden dar los bytes que declara la cabecera: un flujo DEFLATE no expande más de 1032 veces. `write_png`, `png_dimensions` y la API pública no cambian.

## Medidas (antes de decidir)

Release, Linux, 12 hilos, banco desechable fuera del repo (borrado al terminar) sobre el código tras la unidad 12; un proceso por medida; pico = RSS máximo (`VmHWM`) menos RSS antes de la operación; tiempos: mejor de 3. «Plana»: bloques de 16×16 con 32 colores; «ruido»: bytes pseudoaleatorios.

**Leer** (`read_png` desde bytes en memoria):

| Imagen | Fichero | Tiempo | Pico |
| --- | --- | --- | --- |
| RGBA 8192², plana | 0,40 MiB | 94 ms | +256 MiB (la imagen) |
| RGBA 8192², ruido | 219,8 MiB | 520 ms | +256 MiB |
| RGBA 8192², un solo color | 0,26 MiB | 76 ms | +256 MiB |
| Indexado 6000², plano | 0,06 MiB | 19 ms | +35 MiB |
| Indexado 6000², ruido | 34,3 MiB | 37 ms | +35 MiB |
| Indexado 8192², un solo color | 0,08 MiB | 37 ms | +64 MiB |

La lectura de ficheros **válidos** ya está en el suelo (el pico es la imagen y nada más). Lo que cuesta es el **fichero truncado o falso** (cabecera válida y 66 bytes de IDAT):

| Cabecera declarada | `read_png` hoy | Con la comprobación (prototipo) |
| --- | --- | --- |
| gris 8 de 32768×32768 | 328 ms, +1024 MiB | 0,0 ms, +0,3 MiB |
| RGBA 8 de 16384×16384 | 326 ms, +1024 MiB | 0,0 ms, +0,3 MiB |
| RGBA 8 de 8192×8192 | 49 ms, +256 MiB | 0,0 ms, +0,3 MiB |
| indexado 8 de 6000×6000 | 1,7 ms, +35 MiB | 0,0 ms, +0,3 MiB |

En la app el daño ya está acotado por `png_dimensions` + `check_fits` (2²⁶ píxeles, 256 MiB como mucho, ~50 ms); sin esa defensa (el fuzzer y la CLI del hito 8) son hasta 1 GiB y 330 ms por un fichero de 66 bytes. La comprobación no cuesta nada medible a los ficheros válidos (aritmética sobre números que `read_png` ya tiene). Con la cota, el fuzzer del prototipo (120 s, 3,1 millones de ejecuciones, sin hallazgos) subió de **1478 MiB** de RSS máximo (la ejecución de la unidad 1.x) a **533 MiB**.

**Cifras finales (código final, mismas condiciones, mejor de 3).** Ficheros falsos de 66 bytes: gris 8 de 32768×32768, 0,0 ms y +0,3 MiB (antes 374 ms y +1024 MiB); RGBA 8 de 16384×16384, 0,0 ms y +0,2 MiB (antes ≈ 326 ms y +1024 MiB); RGBA 8192² y gris 8192², 0,0 ms y +0,2 MiB (antes 87 ms y +256 MiB para RGBA). Fichero válido (RGBA 8192² de un solo color, 268 070 bytes): **106 ms antes y 107 ms después**, +256 MiB en los dos: la comprobación no cuesta nada medible. Los 21 PNG (17 de `png-aux` y 4 de `golden/png`) se leen sin error. Pruebas `#[ignore]` en release (8192² y rejilla completa): 3 pasan en 1,2 s. Fuzzer, 60 s con el código final: 1,7 millones de ejecuciones, sin hallazgos, RSS máximo 543 MiB (antes 1478 MiB).

**Lectura por flujo (reservar a medida que llegan las filas): no.** La cota deja la amplificación en 1032× y un fichero legítimo necesita la imagen entera de todos modos; Adam7 obliga a tenerla completa. Fuera de alcance.

## La cota

### La regla

Después de las comprobaciones de tamaño de hoy (`TooLarge`) y **antes** de `try_reserve_exact`:

```text
needed = bytes que el flujo zlib debe producir para la imagen por defecto
         (incluidos los bytes de filtro de cada fila y las pasadas de Adam7;
          NO el búfer de muestras expandido)
idat   = suma de los bytes de IDAT presentes en el fichero
si needed > 1032 × idat   (comparación estricta)   →   Err(Decode)   // sin reservar
```

- **`needed`.** Sin entrelazar: `alto × (1 + ceil(ancho × bits_por_píxel / 8))`. Adam7: la suma de lo mismo para cada una de las 7 pasadas con tamaño propio (se saltan las vacías). `bits_por_píxel = canales × profundidad` (gris 1, RGB 3, indexado 1, gris con alfa 2, RGBA 4). Aritmética de `u64` comprobada; un desbordamiento da `TooLarge` (inalcanzable tras el chequeo previo; defensivo). Las dimensiones son las del **decodificador**: las de `fcTL` del primer cuadro si precede al IDAT y, si no, las de IHDR (un APNG con el primer cuadro menor que el lienzo ya se rechaza hoy; el resultado no cambia).
- **`idat`: bytes presentes, no declarados.** `scan_chunks` ya recorre los chunks antes de reservar y acumula la longitud **del trozo de datos que realmente está en el fichero** (`bytes[pos+8..end-4].len()`). Un chunk cuya longitud declarada pasa del final del fichero ya se rechaza como truncado (`Decode`) antes de llegar aquí, de modo que un IDAT que declara 2 GiB en un fichero de 100 bytes no aporta nada.
- **Error.** `PngError::Decode(String)` sin variante nueva (los tests de truncados esperan `Err(Decode(_))` y la app muestra el texto). El mensaje **empieza por «tamaño declarado inconsistente con el fichero»** y puede añadir tras él los números (`: N bytes de datos comprimidos para M esperados`). El texto no es estable más allá de ese comienzo, que es lo que comprueban las pruebas.
- **Orden.** Va después de `TooLarge` y de `InvalidPalette`/`InvalidTrns`/`TrailingData` (que salen del recorrido de chunks y del plan): ningún error de hoy cambia de variante.

### ¿Es sólida? Derivación

DEFLATE codifica la salida con literales, bloques sin comprimir o coincidencias (longitud 3 a 258, distancia 1 a 32768). Lo que más expande una coincidencia es la longitud 258, y cuesta como mínimo **2 bits**: un código de longitud/literal de ≥ 1 bit (los códigos de Huffman no tienen longitud 0 para un símbolo usado) más un código de distancia de ≥ 1 bit (RFC 1951: «si solo se usa un código de distancia, se codifica con un bit, no con cero»). Un literal cuesta ≥ 1 bit por byte y un bloque sin comprimir ≥ 8 bits por byte: expanden menos. Por tanto 8 bits producen como mucho 4 × 258 = **1032** bytes y

```text
salida ≤ 1032 × bytes_DEFLATE      y      bytes_DEFLATE = bytes_zlib − 6 < bytes_zlib ≤ idat
```

(la envoltura zlib añade 2 bytes de cabecera y 4 de Adler-32). Así que **todo flujo válido cumple `salida ≤ 1032 × idat`**, y un PNG que se decodifica produce al menos `needed` bytes: `needed ≤ salida ≤ 1032 × idat`. La cota es del flujo, no de cómo se divida en IDAT, y `needed` es lo mínimo que el decodificador exige (los datos de sobra dentro del IDAT, que se aceptan, solo aumentan `salida`). Un flujo fuera de la especificación (un código de distancia de 0 bits) que un decodificador permisivo aceptase con razón > 1032 se rechaza: no es un PNG legítimo.

Comprobado con datos reales (scratchpad, sin ficheros en el repo):

| Origen | Razón `needed / idat` |
| --- | --- |
| Corpus `golden/png` (4) y los 17 PNG de `png-aux` de la 13b | 2,0 a 7,2 |
| Plana 8192² RGBA / indexado 6000² | 647 / 565 |
| Un solo color 8192²: indexado / RGBA (`write_png`) | 797 / **1001,6** |
| zlib nivel 9 sobre ceros | 1028,7 |
| Flujo hecho a mano con códigos dinámicos (literal 0 con 2 bits, símbolo 285 con 1, distancia con 1), decodificado por zlib | **1031,2** |
| Límite | 1032 |

`needed` coincide con la longitud realmente inflada en 139 ficheros (los 4 de `golden/png`, los 17 de `png-aux` y 118 entrelazados Adam7 de profundidades 1, 2, 4, 8 y 16, tipos 0, 2, 3 y 6 y tamaños de 1×1 a 257×31, incluidos 1×N y N×1): 0 diferencias. (Los 118 los generé con ImageMagick solo como entrada de esa comprobación, en el scratchpad; las pruebas de la unidad construyen sus PNG en el propio código.)

### Qué no resuelve

Un fichero de N bytes puede declarar hasta 1032·N bytes y costar esa reserva antes de descubrir que está truncado (un fichero de 1 MiB que declara RGBA 8192² y está cortado cuesta 256 MiB y ~50 ms). Es proporcional al fichero, no a 66 bytes.

## Casos (8 filas)

Notación: `Z(n)` es un flujo zlib de razón máxima que produce `1 + 258·k ≥ n` bytes cero (se construye a mano en la prueba: literal 0, símbolo 285 y distancia 1 con códigos dinámicos de 2, 1 y 1 bit; su Adler-32 es `((L mod 65521) << 16) | 1`, con `L = 1 + 258·k` los bytes que realmente produce). Las funciones privadas que se prueban:

```rust
fn inflated_len(width: u32, height: u32, bits_per_pixel: u8, interlaced: bool) -> Option<u64>;
fn stream_is_plausible(needed: u64, idat_len: u64) -> bool;   // needed <= 1032 * idat_len, sin desbordar
```

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `inflated_len` sin entrelazar: gris 8 de 1×1 y de 3×2; RGBA 8 de 1×1; gris 1 bit de 9×2; RGB 16 de 2×2; RGBA 8 de 8192×8192; RGBA 16 de `u32::MAX`×`u32::MAX` | `Some(2)`, `Some(8)`, `Some(5)`, `Some(6)`, `Some(26)`, `Some(268 443 648)`; `None` (sin pánico ni desbordamiento) |
| 2 | `inflated_len` Adam7: gris 8 de 1×1, 5×1, 1×5 y 8×8; RGBA 8 de 3×3; gris 1 bit de 9×9; gris 8 de 8192×8192; RGBA 8 de 8192×8192 | `Some(2)`, `Some(9)`, `Some(10)`, `Some(79)`, `Some(42)`, `Some(42)`, `Some(67 124 224)`, `Some(268 450 816)` (pasadas vacías sin contar) |
| 3 | `stream_is_plausible(n, l)`: `(1032, 1)`; `(1033, 1)`; `(0, 0)`; `(1, 0)`; `(u64::MAX, u64::MAX)`; `(1032 · 2³², 2³²)` | `true`; `false` (estricta); `true`; `false`; `true`; `true` (sin desbordar) |
| 4 | **Rechazo antes de reservar** (`tests/png_reserve.rs`, solo Linux): gris 8 de 32768×32768, RGBA 8 de 16384×16384, RGBA 8 de 8192×8192 e indexado de 6000×6000, y el gris de 32768² entrelazado, todos con IDAT mínimo de 9 bytes; **control:** gris 8192² plano escrito con `write_png` (la imagen se suelta antes de medir) | `Err(Decode(_))` con el mensaje «tamaño declarado inconsistente…» y el pico de RSS del proceso sube **menos de 32 MiB** (hoy 1024, 1024, 256, 35 y 1024); el control es `Ok` y el pico sube **≥ 48 MiB** (el instrumento mide) |
| 5 | **Cota ajustada, flujo `Z`:** gris 8, gris con alfa, RGBA e indexado de **4096×4096** con `Z(needed)`; gris 8 entrelazado de 4096×4096 con `Z(needed)`. La misma fila a **8192×8192** en una prueba `#[ignore]` | `Ok`, con todos los píxeles a 0 e informe vacío (los cuatro primeros) / `Ok` (Adam7): razón ≈ 1031 y ningún rechazo; razón ≥ 1025 a 4096² y ≥ 1030 a 8192² (la cabecera dinámica de `Z` pesa ≈ 55 bytes); el margen de 0,07 % frente al límite 1032 solo se ejercita a 8192² |
| 6 | **Un solo color con el codificador por defecto:** gris, gris con alfa, indexado y RGBA de **4096×4096** en un color distinto de 0, `write_png` y `read_png`. La misma fila a **8192×8192** en una prueba `#[ignore]` | `Ok` con la misma imagen e informe vacío (razones medidas a 8192²: 797 indexado, 1002 RGBA) |
| 7 | **Otros compresores** (usando el crate `png` como codificador, que es dependencia directa): una imagen de **un solo color** y otra de **patrones repetidos** (franjas y bloques que se repiten; las dos con alfa no trivial en RGBA) en gris 8, indexado 8 y RGBA 8, de tamaño que llegue a razón > 500 (≈ 1024² RGBA y 2048² en 1 byte por píxel; el conjunto entero ≤ unos 5 s con el perfil de pruebas), con cada una de estas formas de comprimir: niveles de miniz_oxide por flate2, `FdeflateUltraFast` (fdeflate), `NoCompression` (bloques sin comprimir) y `Z(needed)` (a mano), con los filtros `NoFilter`, `Sub`, `Up` y `Adaptive`. **Suite:** niveles 1, 6 y 9 (≲ 3 s); **`#[ignore]`:** la rejilla completa, niveles 1 a 9 × los cuatro filtros (≈ 9 s con el perfil de pruebas, 273 combinaciones), que se ejecuta al cerrar en release | `Ok` con la misma imagen en todas las combinaciones; ninguna se rechaza |
| 8 | **Orden de errores, truncados e IDAT declarado enorme:** gris 8 de 100000×100000 con IDAT mínimo; indexado de 8192×8192 sin PLTE; indexado con tRNS más largo que PLTE; ruido de 64×64 cortado a media IDAT; sin IDAT (solo IHDR e IEND) de 8×8; gris de 8192×8192 con un IDAT que declara 2 GiB y fichero de ~100 bytes | `TooLarge` (la cota no se mira antes); `InvalidPalette`; `InvalidTrns`; `Decode`; `Decode`; `Decode` (el truncado ya lo rechaza `scan_chunks`: no aporta bytes). Estos casos solo comprueban `Decode`: el mensaje de la cota lo comprueba un caso añadido: gris 8 de 200×200 con el IDAT mínimo de 11 bytes (1032 × 11 < 40 200) → `Decode` con el mensaje de la cota, y con `Z(40200)` → `Ok` |

**Corpus y pruebas anteriores:** no son filas nuevas: `golden/png` (`png_interop`, `png_roundtrip`), las pruebas de la 13b y las de la 17a, y todo lo que lee un PNG, pasan **sin cambios** (con la cota ya puesta, en el prototipo pasan las 2959 pruebas del workspace que no tocan el escritor por flujo). Los 17 PNG de `png-aux` (viven en `~/.cache/pixoko-pruebas`, fuera del repo, así que no pueden ser una prueba) se leen al cerrar con un programa desechable y se anota el resultado.

## Propiedades (3; `proptest`)

1. **`inflated_len` contra un modelo** (256 casos). Para cualquier ancho y alto de 1 a 40, tipo de color y profundidad válidos y entrelazado o no: `inflated_len` es la suma, calculada **contando píxeles**, de lo que ocupa cada pasada: el modelo clasifica cada píxel en su pasada con la matriz 8×8 de Adam7 de la especificación (sin las fórmulas de `div_ceil` del código) y suma `filas × (1 + ceil(columnas × bpp / 8))`. Nunca hay pánico con `u32` arbitrarios (`Some` o `None`).
2. **Razón máxima con dimensiones al azar** (64 casos: cada uno decodifica hasta 8 MiB). Para ancho y alto de 1 a 3000 con `needed ≤ 2²³`, tipo y profundidad válidos, con o sin Adam7 (el indexado con una PLTE de `2^profundidad` entradas) y un IDAT que es `Z(needed)`: `read_png` es `Ok` (la cota nunca rechaza un fichero que se decodifica) y la imagen es toda cero.
3. **Ningún compresor es rechazado** (128 casos). Para una imagen de 1 a 64 de lado, tipo de color y profundidad cualesquiera que el codificador admita (gris de 1, 2, 4, 8 y 16 bits, gris con alfa y RGBA de 8 y 16, RGB de 8 y 16, indexado de 1, 2, 4 y 8; el codificador no entrelaza: Adam7 lo cubren la propiedad 2 y la fila 5), con contenido de un solo color, de franjas repetidas o de ruido, y cualquier forma de comprimir de la fila 7: `read_png` es `Ok` (no hay rechazo por la cota; los modos sin representación directa, por su conversión documentada).

Todas son sin ventana.

## Rendimiento de las pruebas

Con el perfil de `cargo test` (sin optimizar), leer un RGBA de 8192² cuesta ≈ 3,8 s y escribirlo ≈ 10 s (en release, 0,1 y 0,3 s). Por eso, **en la suite**: las filas 1 a 3 (puras), las filas 5 y 6 a 4096², la fila 7 con niveles 1, 6 y 9, la 8 y las propiedades; **`#[ignore]`**: la rejilla completa de la fila 7 y las filas 5 y 6 a 8192² (una prueba cada una, secuenciales, soltando cada imagen antes de la siguiente; pico ≈ 0,5 GiB), que se ejecutan una vez al cerrar con `cargo test --release -p px-formats -- --ignored` y cuyo resultado se anota. La fila 4 (medida de RSS) va en su **propio ejecutable** (`tests/png_reserve.rs`): mide el pico del **proceso**, y otras pruebas en hilos del mismo proceso lo ensuciarían. Solo en Linux (`#[cfg(target_os = "linux")]`; lee `/proc/self/status` y restablece el máximo con `/proc/self/clear_refs`, sin `unsafe`: un asignador contable exigiría `unsafe impl GlobalAlloc`, prohibido), con tolerancia generosa (< 32 MiB para el fichero falso, ≥ 48 MiB para el control) y **`#[ignore]` si resultara inestable en el CI**. La lógica de la fila 4 queda cubierta en todos los sistemas por las filas 1 a 3 (la comprobación es una función pura).

## Dónde van las pruebas

- `crates/px-formats/src/png/tests.rs` (módulo `#[cfg(test)]` de la biblioteca: filas 1 a 3 y propiedad 1; prueban las dos funciones privadas y no añaden ejecutable).
- `crates/px-formats/tests/png/max_ratio.rs` (constructor de `Z(n)`, filas 5, 6 y 8, propiedad 2) y `crates/px-formats/tests/png/compressors.rs` (fila 7 y propiedad 3, con el crate `png` como codificador), declarados con una línea `#[path]` + `mod` en `tests/png.rs` (que ya tiene el ayudante de chunks, CRC y Adler-32 de la unidad 5; la cabecera de ese fichero dice «sin `png`»: esos dos módulos nuevos sí lo usan, y se anota).
- `crates/px-formats/tests/png_reserve.rs` (fila 4).

## Pruebas anteriores que cambian

**Ninguna.** Si alguna necesitara más que no cambiar, me paro y aviso.

## Fuzzing

El objetivo de la unidad 1.x (`fuzz/fuzz_targets/read_png.rs`) **no cambia**: ya ejercita `read_png` con toda entrada, y la cota se dispara con cualquier fichero de cabecera grande y datos mínimos. Al cerrar se repite la prueba corta (`docs/FUZZING.md`, 60 s, `-rss_limit_mb=4096 -timeout=10`, semillas `golden/png`): sin hallazgos y con el RSS máximo muy por debajo de los 1478 MiB de la unidad 1.x. La sesión larga la lanzas tú (`git status golden/` debe quedar limpio).

## Fuera de alcance

El escritor por flujo (pospuesto, ver las notas); lectura por flujo; cambiar `write_png`, `png_dimensions`, la API pública, `px-app` o el objetivo del fuzzer.

## Criterios de aceptación

`cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace --locked` y `cargo deny check` (sin dependencias nuevas); las pruebas `#[ignore]` en release; la prueba corta del fuzzer; los `png-aux` leídos; la tabla de «Medidas» de lectura rehecha con el código final; la nota «Para el hito 2» de `hito-1-png.md` y «PNG» de `hito-2-notas-unidad-14.md` actualizadas; `license-auditor` una vez sobre el rango de la unidad.

Commits, en este orden: spec (con la nota de la 14); pruebas en rojo (`test-writer`); lógica (`inflated_len`, `stream_is_plausible` y su uso en `read_png`); notas y cifras finales.

## Matices del visto bueno (2026-10-19)

- **Alcance.** Solo la parte 1. Se quitan D5 a D7, D9, D10 y D11 de la propuesta (D6, D9 y D10 por petición; D5, D7 y D11 porque eran del escritor por flujo); se conservan **D1, D2 y D8**, y D3 y D4 (factor exacto 1032 sin margen; no se lee por flujo), que no se discutieron.
- **D1.** La cota usa la suma de los IDAT, contando los bytes presentes en el fichero.
- **D2.** `Decode(String)`, con el mensaje «tamaño declarado inconsistente con el fichero…».
- **D8.** Los 8192² van en `#[ignore]`; en la suite, 4096².
- **Cota:** comparación estricta, con la derivación de arriba; sobre los bytes del flujo zlib (filtros y Adam7), no sobre el búfer de muestras.
- **Pruebas de casos extremos** con varios compresores (fila 7 y propiedad 3): niveles 1 a 9, al menos dos implementaciones de deflate, un solo color y patrones repetidos.
