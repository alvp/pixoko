# Hito 1 · Unidad extra: fuzzing del lector PNG (`cargo-fuzz`)

Estado: **aprobada** (2026-10-17; decisiones 1 a 7 aceptadas, con los matices: la excepción de NCSA es solo para `libfuzzer-sys`, con un comentario en `deny.toml` con el motivo y la fecha y una nota en `docs/DEPENDENCIES.md`; `docs/FUZZING.md` termina la sesión larga con «`git status golden/` debe quedar limpio»; y la reserva de hasta 1 GiB del lector se anota en «Para el hito 2» de `specs/hito-1-png.md` sin cambiar el código)

## Objetivo

Cumplir la línea «Fuzzing (`cargo-fuzz`) de cada lector de formatos» de `docs/PLAN.md` para el primer lector, `read_png` (`specs/hito-1-png.md`). El fuzzer busca lo que las propiedades de `proptest` (azar sin retroalimentación) no alcanzan: entradas que recorren caminos poco probables del lector y de la crate `png`. Cada fallo se convierte en un test de regresión en `px-formats` y se corrige en el lector. El CI no ejecuta el fuzzer.

## Entradas y salidas

| Ruta | Qué |
| --- | --- |
| `fuzz/Cargo.toml` | Crate `pixoko-fuzz`, **fuera del workspace** (lleva su propio `[workspace]` vacío; sin él Cargo se queja de que está dentro del workspace raíz sin ser miembro). Dependencias: `libfuzzer-sys`, `px-core` y `px-formats` (rutas). Sin `lints.workspace` |
| `fuzz/fuzz_targets/read_png.rs` | El objetivo (ver «Comportamiento») |
| `fuzz/Cargo.lock` | Se commitea (reproducibilidad de las dependencias del fuzzer; no lo toca el CI) |
| `.gitignore` | `/fuzz/corpus`, `/fuzz/artifacts`, y también `/fuzz/target` y `/fuzz/coverage` (decisión 4) |
| `crates/px-formats/tests/png_fuzz.rs` | Un test por fallo encontrado (vacío hasta que haya alguno) |
| `docs/FUZZING.md` | Cómo lanzarlo y cómo reproducir un fallo (ver «Documentación») |
| `docs/DEPENDENCIES.md` | Sección nueva «Fuzzing»: `libfuzzer-sys` y sus transitivas |
| `deny.toml` | Excepción de licencia solo para `libfuzzer-sys` (decisión 1, opción A), con un comentario con el motivo y la fecha |
| `specs/hito-1-png.md` | Una viñeta nueva en «Para el hito 2» sobre la reserva de hasta 1 GiB antes de detectar un fichero truncado (solo texto) |

Ningún cambio en `px-core`, `px-formats/src` ni en el CI, salvo los arreglos que salgan de los fallos (punto 5).

## Comportamiento

### El objetivo `read_png`

Con `fuzz_target!(|data: &[u8]| …)`:

1. `read_png(data)`. Si es `Err`, termina: un error es un resultado correcto.
2. Si es `Ok(read)`, y la imagen cabe en 16 MiB (`Image::byte_len`; decisión 3), comprueba la ida y vuelta:
   - `write_png(&read.image)` debe ser `Ok` (cualquier imagen válida se puede escribir);
   - `read_png` de esos bytes debe ser `Ok`, con `is_exact()` y una `Image` **igual** a `read.image`.

   La comparación es la `PartialEq` de `Image` (modo, tamaño, todos los píxeles y la paleta), así que es más estricta que «valores de píxel». Como `read_png` nunca devuelve `transparent_index`, no hace falta la «normalización» de la unidad 5: la igualdad es exacta.
3. **Pánico:** `libfuzzer-sys` aborta el proceso y guarda la entrada, y `cargo-fuzz` compila con aserciones de depuración y desbordamiento de enteros activos, así que una aritmética que desborda también cuenta. No hace falta código para detectarlo.
4. **Memoria:** lo detecta libFuzzer con `-rss_limit_mb`. Se lanza con `-rss_limit_mb=4096`: el pico documentado de la lectura es de hasta `2 × Image::MAX_BYTES` = 2 GiB (comportamiento 7 de la spec de PNG), así que el límite por defecto de libFuzzer (2048 MiB) estaría al borde de lo permitido por diseño. Con 4096 solo salta lo que supera lo que la spec promete (decisión 2). Además `-timeout=10`: una entrada que tarda más de 10 s es un fallo.
5. El objetivo no tiene `unsafe` propio (`#![forbid(unsafe_code)]`; comprobado que `fuzz_target!` compila con él). La macro de `libfuzzer-sys` genera por dentro la función `extern "C"` que llama libFuzzer: es código de la dependencia, no del proyecto.

### Semillas

Se usa `golden/png/` tal cual, **por la línea de comandos**, sin copiar ficheros ni versionar el corpus generado. Dos hechos comprobados en una copia del montaje (en el scratchpad, fuera del repo) que obligan a cambiar la línea que pedías:

- **libFuzzer escribe las entradas nuevas en el primer directorio de corpus.** Con `cargo fuzz run read_png golden/png` lo hace en `golden/png/`: en 15 s pasó de 9 a 289 ficheros. Eso ensuciaría el corpus dorado y haría fallar `png_interop` (un fichero ajeno es un fallo). La línea correcta pone **delante** el corpus generado, que sí es ignorado por git: `fuzz/corpus/read_png golden/png` (comprobado: el directorio dorado se queda en 9 ficheros y las entradas nuevas van a `fuzz/corpus/read_png`).
- libFuzzer exige que el primer directorio exista; `cargo-fuzz` solo lo crea cuando no se le pasa ninguno. Se añade `-create_missing_dirs=1` (opción de libFuzzer) para no depender de un `mkdir`.

`golden/png/` contiene también `README.md` y los `.sha256`: libFuzzer los lee como semillas (son entradas que fallan pronto; inofensivas). `fuzz/artifacts/` (los `crash-*`, `oom-*`, `timeout-*`) también va ignorado.

### Línea de comandos

Desde la raíz del repo:

```
cargo +nightly fuzz run read_png fuzz/corpus/read_png golden/png -- \
  -rss_limit_mb=4096 -timeout=10 -create_missing_dirs=1 -max_total_time=60
```

La prueba corta que ejecuto yo es esta (60 s). La sesión larga la lanzas tú: la misma línea sin `-max_total_time`. Hace falta la toolchain nightly (con `rust-src`: `cargo-fuzz` compila con `-Zbuild-std` y AddressSanitizer) y un compilador de C++ (hoy hay `clang` y `g++`). Ya están `cargo-fuzz` 0.13.2 y nightly 1.101.0 (2026-10-16).

### Fallos

Cada fallo que encuentre el fuzzer (pánico, desbordamiento, memoria, tiempo o ida y vuelta distinta) sigue este camino, **uno por commit**:

1. Se minimiza (`cargo +nightly fuzz tmin read_png <artefacto>`) y, si se puede, se acorta a mano.
2. Test de regresión en `crates/px-formats/tests/png_fuzz.rs`: los bytes mínimos como `const` en el propio código (con un comentario de qué falló y cómo se encontró), **sin ficheros `.png` nuevos**. El test comprueba el **resultado correcto** según la spec (la variante exacta de `PngError`, o la imagen y el informe), no solo «no hace pánico». Debe fallar antes del arreglo: se ejecuta y se anota.
3. Se corrige en el lector.
4. Si el arreglo cambia algo que dice `specs/hito-1-png.md` (un caso que hoy se acepta y pasaría a rechazarse, por ejemplo), **paro y te lo consulto** antes de tocar el código: la spec está aprobada.

Los fallos que se encuentren después de esta unidad (en tu sesión larga) siguen el mismo camino.

## Documentación: `docs/FUZZING.md`

Corto (una página): qué cubre el objetivo; requisitos (nightly con `rust-src`, compilador de C++, `cargo install cargo-fuzz --version 0.13.2 --locked`); la línea de comandos de arriba y por qué el corpus generado va primero; cómo reproducir un fallo y qué hacer con él:

```
cargo +nightly fuzz run read_png fuzz/artifacts/read_png/crash-<hash>
cargo +nightly fuzz tmin read_png fuzz/artifacts/read_png/crash-<hash>
xxd -i <entrada minimizada>      # los bytes para el test de regresión
```

Y las dos reglas: no se commitean `corpus/` ni `artifacts/`, y «el CI no ejecuta el fuzzer». También las comprobaciones locales de la crate del fuzzer, que `cargo fmt --all` y `cargo clippy --workspace` no cubren por estar fuera del workspace: `cargo fmt --manifest-path fuzz/Cargo.toml --check`, `cargo clippy --manifest-path fuzz/Cargo.toml -- -D warnings` (compilan con la toolchain estable) y `cargo deny --manifest-path fuzz/Cargo.toml --locked check`.

## Dependencias

Solo `libfuzzer-sys` es nueva (más lo que arrastra). `px-core`, `px-formats` y `png` y sus transitivas ya están anotadas.

| Crate | Versión | Licencia | Notas |
| --- | --- | --- | --- |
| `libfuzzer-sys` | 0.4.13 | `(MIT OR Apache-2.0) AND NCSA` | **Fuera de `deny.toml`.** Ver abajo |
| `arbitrary` | 1.5.0 | MIT OR Apache-2.0 | Dependencia normal de `libfuzzer-sys` |
| `cc` | 1.8.0 | MIT OR Apache-2.0 | Solo build |
| `find-msvc-tools` | 0.1.14 | MIT OR Apache-2.0 | Solo build (de `cc`) |
| `jobserver` | 0.1.35 | MIT OR Apache-2.0 | Solo build (de `cc`) |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | Solo build (de `jobserver`) |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 | Solo build (de `cc`) |

**Resultado de `cargo deny --manifest-path fuzz/Cargo.toml --locked check`** (cargo-deny 0.20.2 con el `deny.toml` del repo, sobre una copia del montaje en el scratchpad): `advisories ok, bans ok, licenses FAILED, sources ok`. El **único** fallo es `libfuzzer-sys 0.4.13`: su licencia es `(MIT OR Apache-2.0) AND NCSA`, con `AND`, así que NCSA es obligatoria y no hay rama alternativa (a diferencia de `adler2` o `r-efi`, que son disyunciones). Todas las demás crates caben en la lista actual. Sigue el aviso de `miniz_oxide` duplicado, ya conocido.

**NCSA** (University of Illinois/NCSA Open Source License): es permisiva, del estilo de MIT/BSD-3 (exige conservar el aviso de copyright y el descargo en las redistribuciones); está aprobada por la OSI y la FSF la considera libre. Viene del código de libFuzzer (LLVM) que `libfuzzer-sys` empaqueta. Solo se compila en el binario del fuzzer, que no se distribuye: no entra en `pixoko`. No es GPL, LGPL ni AGPL, así que no choca con la regla de `CLAUDE.md`, pero la lista de licencias es decisión tuya (decisión 1: solo una excepción para `libfuzzer-sys`, no un `allow` general).

`cargo-fuzz` (0.13.2, la herramienta) no es una dependencia de Cargo: se instala aparte, como `cargo-deny`; se anota en «Herramientas» de `docs/DEPENDENCIES.md` junto con la nightly.

## Casos y comprobaciones

Esta unidad no añade funciones: el «test» del objetivo es que **sabe fallar**, igual que en la unidad 6 («un corpus que no sabe fallar no protege»).

| # | Comprobación | Resultado esperado |
| --- | --- | --- |
| 1 | `cargo check` y `cargo clippy -- -D warnings` sobre `fuzz/Cargo.toml` con la toolchain estable | Sin avisos |
| 2 | Prueba corta: la línea de «Línea de comandos» con `-max_total_time=60` | Termina con `Done N runs`; `golden/png/` sigue con sus 9 ficheros (`git status` limpio) y no hay `crash-*` en `fuzz/artifacts/read_png/`; si hay, se sigue «Fallos» |
| 3 | **El objetivo sabe fallar:** se estropea a mano, sin commitear, la comprobación (p. ej. `assert!(read.image.width() != 16)`, que las semillas de 16 × 16 disparan al instante) | La prueba corta termina con `crash-*`; la línea de reproducción lo repite; se restaura y `git diff` queda vacío |
| 4 | **La ida y vuelta se comprueba de verdad:** igual, con una aserción que no se cumple en la relectura (p. ej. comparar contra una imagen con el píxel (0, 0) cambiado) | `crash-*` por la ida y vuelta, no por el pánico del paso 3 |
| 5 | `git status` tras las pruebas 2 a 4 | Solo lo de la unidad; `fuzz/corpus`, `fuzz/artifacts` y `fuzz/target` ignorados |

## Resultados previos (en una copia fuera del repo)

Para escribir esta spec monté el mismo objetivo (solo con el paso 1: `read_png`, sin la ida y vuelta) en el scratchpad y lo ejecuté 15 s con `golden/png/` de semilla: 90 004 ejecuciones, sin fallos, 1592 puntos de cobertura. En una pasada de 11 s con la línea final: 286 735 ejecuciones (26 000/s), sin fallos, pero **RSS de hasta 1478 MiB**. Es la reserva de diseño del comportamiento 7 de la spec de PNG: un fichero de 70 bytes que declara gris de 32768 × 32768 hace que `read_png` reserve y ponga a cero 1 GiB antes de descubrir que el IDAT está truncado, y tarda unos 127 ms (medido). No es un fallo de la spec, pero explica el RSS y es la razón del `-rss_limit_mb=4096`. Si en la sesión larga el rendimiento cae por ese tipo de entradas, lo trataremos como una unidad aparte (por ejemplo, no reservar el búfer hasta ver datos), no como parte de esta.

## Criterios de aceptación

- Las comprobaciones 1 a 5 de arriba.
- Antes de cada commit, lo de `CLAUDE.md` en el workspace (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test --workspace`) más las tres comprobaciones locales de la crate del fuzzer.
- `cargo deny --manifest-path fuzz/Cargo.toml --locked check` en verde **tras** la decisión 1, y `cargo deny --locked check` del workspace sin cambios.
- `license-auditor` sobre `fuzz/`, `docs/FUZZING.md` y los cambios de `docs/DEPENDENCIES.md` antes del commit.
- Commits pequeños: (1) esta spec, ya aprobada; (2) `fuzz/` + `.gitignore` + `docs/` (+ `deny.toml` si lo decides); (3 en adelante) un commit por fallo: test de regresión y arreglo.

## Fuera de alcance

- Ejecutar el fuzzer en el CI (ni siquiera una pasada corta); un paso de `cargo deny` sobre el manifiesto del fuzzer en el CI (decisión 5).
- Fuzzing de `write_png` por separado, objetivos estructurados con `arbitrary` (un `Image` aleatorio) y diccionarios de chunks PNG.
- Los lectores de otros formatos (GIF, BMP, PCX, IFF…): cada uno añadirá su propio objetivo en su hito.
- Optimizar la reserva de 1 GiB del lector (ver «Resultados previos»).

## Decisiones que necesito que confirmes

1. **Licencia NCSA (bloqueante).** `libfuzzer-sys` exige `NCSA` y no está en `deny.toml`; sin resolverlo, `cargo deny` falla sobre el manifiesto del fuzzer. Opciones:
   - **A. Excepción solo para esa crate** (recomendada): `[[licenses.exceptions]] crate = "libfuzzer-sys", allow = ["NCSA"]`. NCSA no se admite en general, solo donde la trae una dependencia del fuzzer.
   - B. Añadir `"NCSA"` a `allow`: vale para cualquier crate futura.
   - C. No usar `libfuzzer-sys`: no es viable con `cargo-fuzz`.

   Elegida la A. Con ella, el `deny` del CI sigue en verde (la excepción no se usa y solo da un aviso de «no encontrada»).
2. **`-rss_limit_mb=4096` y `-timeout=10`** en la línea de comandos (en lugar de los valores por defecto de libFuzzer: 2048 y 1200). Razón arriba; el coste es que un fallo de memoria entre 2 y 4 GiB no se vería. Como el pico por diseño es de 2 GiB, lo que pase de 4 GiB es una violación clara.
3. **Tope de 16 MiB en la ida y vuelta del objetivo:** si `read_png` da una imagen mayor, se omite `write_png` y la relectura (con `-max_len` por defecto, 4096 bytes, no se alcanza; el tope protege cuando alguien suba `-max_len`, porque `write_png` llega a unos 2 GiB de pico en el límite). Sin tope, una imagen de 1 GiB haría la comprobación lenta y daría un falso fallo de memoria.
4. **`.gitignore`:** además de `fuzz/corpus` y `fuzz/artifacts` (lo que pedías), añado `fuzz/target` y `fuzz/coverage`. El `/target` actual está anclado a la raíz y no cubre `fuzz/target`, que se llena de binarios con sanitizadores.
5. **El CI no toca el fuzzer ni su `deny`.** El `deny` del CI (`cargo deny --locked check` en la raíz) no ve `fuzz/`, así que sus dependencias solo se comprueban en local, con el procedimiento de `docs/FUZZING.md`. Añadir un paso de CI obligaría a cambiar `crates/px-golden/tests/ci.rs`, que fija el orden exacto de los comandos. Mi propuesta es dejarlo así por ahora.
6. **La línea de comandos cambia respecto a la que pedías** (`… read_png -- -max_total_time=60` con `golden/png` como corpus): el corpus generado va primero (`fuzz/corpus/read_png golden/png`) y se añade `-create_missing_dirs=1`, porque si no, libFuzzer escribe en `golden/png/` y rompe `png_interop`. Es lo que he comprobado; confírmame que te parece bien.
7. **Dónde van los tests de regresión:** un fichero propio, `crates/px-formats/tests/png_fuzz.rs`, en vez de seguir creciendo `tests/png.rs` (la unidad 5), para distinguir de un vistazo lo que viene del fuzzer. Y el comportamiento si un arreglo choca con la spec de PNG: paro y te lo consulto (punto 4 de «Fallos»).
