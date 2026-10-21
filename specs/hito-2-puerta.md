# Hito 2 · Unidad 22: puerta del hito (abrir, editar y guardar un PNG real)

Estado: **aprobada** (2026-10-20; decisiones D1 a D10 con los matices del visto bueno, incorporados al texto y que prevalecen: candidato para el hito 3 (partir `px-app` en lib y binario), tres condiciones de la excepción NCSA, `rustup` lo ejecutas tú, notas autorizadas para el resumen y SHA-256 de `golden/png/`). Renumerada: era la 21 del reparto ([`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md), D3 y D9) hasta que «Fijar transparente» ([`hito-2-fijar-transparente.md`](hito-2-fijar-transparente.md)) tomó ese número. Abrir y guardar: [`hito-2-abrir-guardar.md`](hito-2-abrir-guardar.md). Diálogos: [`hito-2-dialogos-archivo.md`](hito-2-dialogos-archivo.md). PNG: [`hito-1-png.md`](hito-1-png.md).

## Objetivo

Demostrar que un PNG real se abre, se edita y se guarda en Linux, Windows y macOS. La parte **automática** corre en el CI de los tres sistemas con `cargo test --workspace --locked` (sin xvfb y sin tocar `ci.rs`); la parte **manual** la haces tú. Al final, el cierre del hito (resumen, aplazado, `cargo deny`, fuzzer).

**No hay código de producción nuevo**: la unidad añade pruebas y documentación (más, según D9, un `deny.toml` para el fuzzer). Las pruebas ejercen lo que ya existe, así que **no pueden estar en rojo**: la prueba de que saben fallar son los controles negativos de las filas 9 y 11. Si una fila falla contra el código actual, es un defecto de una unidad anterior: me paro y te lo cuento, sin arreglarlo en silencio. No toco `px-core` ni `px-raster`.

## Fuentes y guion

- **Reales (4):** `golden/png/*.png` (16×16; modos y conversiones se leen del fichero, no se suponen).
- **Generados por la prueba** con `write_png` (37×23, que no es múltiplo de nada): RGBA con píxeles de alfa 0 y 128; indexada de 5 colores con alfa propio; gris; gris con alfa.
- **Grandes** (`#[ignore]`): RGBA de 8192² (el tope `MAX_OPEN_PIXELS`) e indexada de 6000².
- **Guion** (lo que cada modo admite): lápiz, línea, relleno y borrador (los gestos pasan por `ToolState::canvas_event` con eventos de lienzo sintéticos, el mismo camino que la app); en indexado, edición de color de una entrada, `PaletteOp::Rotate`, `Move`, `Add` y `SetTransparent`; conversión de modo. Gris: el borrador se rechaza sin paso; indexado sin índice transparente: ídem hasta fijarlo.
- **Modelo independiente:** una rejilla de píxeles más paleta (RGBA expandido, como el `.sha256` del corpus) que aplica las mismas ediciones con código ingenuo (relleno por cola, línea solo horizontal, vertical o a 45°, donde cualquier algoritmo coincide). Lo esperado sale del modelo, no de `Image` ni de `Session`.
- **Normalización:** tras guardar, un indexado con índice transparente vuelve con la entrada resuelta (alfa 0) y sin índice (comportamiento 11 de la spec de PNG); se compara con `normalizada(imagen)`.

## Decisiones (a confirmar)

1. **D1 · Dónde vive la prueba.** `px-app` es un binario sin `lib.rs`: `px-golden` **no puede** usar `Session`. Opciones: (a) partir `px-app` en lib y binario (toca `Cargo.toml`, `main.rs` y el paso `cargo build` del CI); (b) la puerta entera en `px-app` como módulo `#[cfg(test)] mod puerta` de `main.rs` (precedente: `damero_guardado`; un solo ejecutable de pruebas) y en `px-golden` solo lo que no necesita `Session`: un **oráculo SHA-256** de los 4 reales editados con comandos de `Document` (fila 11), con la ayuda `sha256` que ya tiene. **Aprobada (b).** **Anotado como candidato para el hito 3: partir `px-app` en lib y binario** (la puerta podría pasar entonces a `px-golden`). `px-golden/png_interop` ya falla ante un fichero ajeno en `golden/png/`: es la segunda red de la fila 9.
2. **D2 · Qué cubre lo automático y qué queda para lo manual.**

   | Automático (CI, tres sistemas) | Manual (tú) |
   | --- | --- |
   | `Session` (`open`, `save`, `save_as`, `save_to`), `write_atomic`, historia, herramientas con eventos sintéticos, paleta, conversión, `Dialogs` con `FakePicker`, PNG de ida y vuelta, `golden/png` intacto, lienzo grande **lógico** (`#[ignore]`, sin textura) | Ventana (`winit`), OpenGL y subida de texturas, diálogos nativos (`rfd`), soltar ficheros, HiDPI, X11 frente a Wayland, muestreo y costuras a zoom 1/2, 1/4 y 1/8, fluidez real a 6000² y 8192², `Cmd+Q` con cambios, el pegamento de `main.rs` |

   Razón: lo de la derecha necesita pantalla, GPU o el sistema de ventanas, que el CI no tiene, y la regla de `CLAUDE.md` sobre capturas deja la prueba visual en tus manos.
3. **D3 · Rutas.** Todo directorio de trabajo cuelga de `<repo>/target/pixoko-gate/<pid>-<n>` (disco, en `.gitignore`, borrado al terminar), no de `/tmp` (tmpfs). **Ninguna escritura sale de `golden/png/`**: las ediciones y los guardados parten de una copia (`fs::copy`); la apertura directa de un real es solo lectura y su primer guardado es `save_to(<trabajo>/…)`. Una ayuda `fuera_de_golden(ruta)` hace pánico si una ruta de escritura cuelga de `golden/png/`. Los directorios de trabajo se **limpian al terminar** (también si la prueba falla: guardián con `Drop`). Peligro que justifica la regla: `Session::save` de un real sin conversión **lo sobrescribiría**, y con conversión o `iCCP` escribe `-copia` junto a él.
4. **D4 · El pegamento de `main.rs`** (`finish_dialog`, `save_file`, `handle_close`) necesita `egui::Context`: no se extrae. La fila 7 repite sus pocas líneas con `Dialogs` y `Session` y el resto va a la prueba manual. Extraerlo sería un refactor de producción sin pedir.
5. **D5 · Memoria y tiempo de los grandes.** Corren una vez al cierre, en `--release` y con un solo hilo (`cargo test --release -p px-app -- --ignored --test-threads=1`); pico previsto ~1 GiB (lienzo, esperado y relectura). No hay aserciones de tiempo (serían frágiles): se imprimen y el pico lo mido yo en Linux con `/usr/bin/time -v`. Esas cifras entran en el resumen del hito.
6. **D6 · Casos de `proptest`:** 64 en las propiedades 1 y 2 y 32 en la 3 y la 4 (tocan el disco). Se baja con `PROPTEST_CASES` solo al desarrollar.
7. **D7 · Lista manual** (≤15 pasos): al final de esta página.
8. **D8 · Cierre del hito.** Contenido nuevo en **`specs/hito-2-resumen.md`** (fila «notas» en el INDEX): unidades cerradas, descartadas y aplazadas con su nota, aplazado a los hitos siguientes y resumen con cifras. `docs/PLAN-RESUMEN.md` solo gana el estado y un puntero (se lee al empezar cada unidad y debe seguir corto). Hay que corregir en el INDEX las filas 2.14a, 2.18 y 2.19, que figuran «en curso» (se comprueba con `git log` que el código está) y en `PLAN-RESUMEN` el «en curso (unidad 18)» y la línea «Resto del hito 2», que aún lista «21 puerta». El hito pasa a **«cerrado, a falta de la prueba manual de la unidad 22»** y a «cerrado» en un commit de una línea cuando tú confirmes.
9. **D9 · Avisos de `cargo deny`** (medidos hoy con `cargo deny --locked check`: `advisories ok, bans ok, licenses ok, sources ok`, y estos avisos):
   - **`license-exception-not-encountered` (línea 10 de `deny.toml`, NCSA de `libfuzzer-sys`).** Es la excepción que el fuzzer **necesita**: su licencia es `(MIT OR Apache-2.0) AND NCSA`, sin alternativa. Aparece porque `fuzz/` está fuera del workspace. Opciones: (A) dejarla y aceptar el aviso en cada ejecución; (B) quitarla: el fuzzer no pasaría su propio `cargo deny`; (C) **moverla a `fuzz/deny.toml`**, con la misma lista de licencias más la excepción, y comprobar el fuzzer con `--config`. Medido en una copia: con (C) el check de la raíz queda sin aviso de licencias y el del fuzzer da `licenses ok`; sin la excepción, `FAILED`. **Aprobada (C)**, con tres condiciones: (i) `ci.rs` y sus pruebas **no comprueban** esa excepción en el `deny.toml` de la raíz (comprobado antes de aprobar: ningún fichero de `crates/`, `.github/` ni `.claude/` menciona `NCSA` ni `deny.toml`; si apareciera uno, me paro y aviso, y no toco `ci.rs`); (ii) la excepción queda acotada a `libfuzzer-sys`, con comentario y fecha; (iii) `fuzz/deny.toml` repite **la misma lista de licencias permitidas, sin ninguna otra** (nunca GPL, LGPL ni AGPL). La comprobación del fuzzer entra en la lista de cierre con este comando exacto: `cargo deny --manifest-path fuzz/Cargo.toml --locked --config fuzz/deny.toml check`. El coste es mantener dos listas (se anota en `docs/FUZZING.md` y `DEPENDENCIES.md`). Quitar el aviso con `-A` exigiría cambiar `ci.rs` (comando exacto): descartado.
   - **17 avisos `duplicate`** (`bitflags`, `block2`, `getrandom`, `jni-sys`, `linux-raw-sys`, `miniz_oxide`, `objc2` y sus tres `objc2-*`, `r-efi`, `redox_syscall` ×3, `rustix`, `syn`, `thiserror` y `-impl`, `windows-sys` ×3): vienen de las versiones fijadas de `eframe`, `rfd` y `png`. **Aceptados sin `skip` en `deny.toml`** (taparían un duplicado nuevo); se anota en `docs/DEPENDENCIES.md` la línea base y, con `cargo tree -d`, qué dependencia directa arrastra cada uno. Cualquier duplicado fuera de esa lista es un aviso nuevo.
10. **D10 · Fuzzer.** Una sola ejecución de 10 minutos al final, con todo el código ya fijado:
    `cargo +nightly fuzz run read_png fuzz/corpus/read_png golden/png -- -rss_limit_mb=4096 -timeout=10 -create_missing_dirs=1 -max_total_time=600`
    - **Lo lanzas tú.** La nightly instalada **no tiene `rust-src`** (comprobado) y `cargo-fuzz` compila con `-Zbuild-std`: **no ejecuto `rustup`**; instalas `rust-src`, lanzas el fuzzer de 10 minutos al cierre y me pasas el resultado para anotarlo aquí.
    - Comprobado hoy: `cargo check --manifest-path fuzz/Cargo.toml --locked` compila (tras 13b y 13).
    - Arranca con el corpus generado actual (2046 entradas, ignorado por git). Al terminar: `git status golden/` limpio. El resultado que me pases (ejecuciones, ejec/s, cobertura, RSS máximo, fallos) va a la sección «Resultado del fuzzer» de esta spec, en su propio commit.
    - Un fallo sigue [`hito-1-fuzz.md`](hito-1-fuzz.md) («Fallos»): minimizar, prueba de regresión en `px-formats/tests/png_fuzz.rs`, arreglo en `px-formats`, un commit por fallo. Si el arreglo exigiera `px-core` o `px-raster`, me paro. Con un fallo abierto el hito no se cierra.

## Casos

`T` es un directorio de trabajo (D3). «Modelo» es el de «Fuentes y guion». Sin `px-core` ni `px-raster` tocados.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | **px-app.** `Session::open` de los 4 reales (copia en `T`, `Canvas::MAX_SIDE`) y de los 4 generados | `Ok`; imagen = lectura independiente del fichero (modelo); modo y tamaño; `state_id` 0; `modified` falso; sin historia; el aviso lleva nombre, tamaño y modo; `converted()` coincide con que el informe o `aux` de `read_png` no esté vacío |
| 2 | **px-app.** Lápiz, línea, relleno y borrador sobre cada fuente, por `ToolState::canvas_event` y `Document::apply` | imagen = modelo tras cada gesto; **un paso de historia por gesto con cambio**; borrador rechazado en gris sin paso; el relleno de un mismo color no da paso |
| 3 | **px-app.** Indexadas: editar el color de una entrada, `Rotate`, `Move`, `Add`, `SetTransparent(Some)` y `SetTransparent(None)` con `apply_palette_op` | imagen y paleta = modelo; un paso por operación; `SetTransparent` solo mueve la marca (los índices no cambian) |
| 4 | **px-app.** `convert_mode` por cada camino exacto (indexado→RGBA, gris→gris con alfa, gris con alfa→RGBA, RGBA→indexado con ≤256 colores) y por uno que pierde datos sin permiso | las exactas dan el modelo en un paso; la que pierde (`TooManyColors` o falta de permiso) da `Err`, **sin cambiar el documento ni la historia** |
| 5 | **px-app.** Con el guion completo hecho: `undo` hasta el principio, `redo` hasta el final, y `undo` + edición nueva + `redo` | tras cada `undo`/`redo`, imagen = la instantánea de ese paso; al principio, imagen original exacta, `state_id` 0 y `modified` falso; al final, imagen final; con la edición nueva `redo` falla y el estado guardado deja de ser alcanzable (`modified` cierto) |
| 6 | **px-app.** `save` y `save_as` tras el guion, sobre una copia en `T` | `save`: sobrescribe la copia, `read_png` = modelo normalizado, `modified` falso, ningún `.pixoko-tmp`; `save_as`: `x-copia.png` (y `-copia-2` al repetir), el fichero anterior intacto byte a byte, la ruta es la nueva |
| 7 | **px-app.** Diálogos con `FakePicker`: `Ctrl+S` sin ruta (`Save`); luego `SaveAs`; `Cancelled`; elegir `otro` con `otro.png` ya existente; `Open` de lo guardado | `Save` propone `sin-titulo.png`, el resultado `T/mi-dibujo` escribe `mi-dibujo.png` y fija la ruta; `SaveAs` propone `mi-dibujo.png` y escribe lo elegido sin tocar el primero; `Cancelled` no escribe nada y deja `modified`; `Exists` y nada escrito; `Open` da la imagen guardada (el pegamento de `main.rs` se repite aquí, ver D4) |
| 8 | **px-app.** Relectura: otra `Session` abre cada fichero guardado y vuelve a guardar | imagen = modelo normalizado; sin conversiones ni `aux`; `state_id` 0, `modified` falso; **los bytes del segundo guardado son los del primero** (`write_png` es determinista) |
| 9 | **px-app.** `golden/png/` al terminar toda la puerta; control negativo | los 9 ficheros con los mismos bytes que en la instantánea del principio y ninguno más, y **el SHA-256 de cada uno de los 4 PNG igual al del principio** (SHA-256 propio de la prueba, sin dependencias nuevas; los `.sha256` del corpus son de la RGBA expandida y los comprueba `png_interop`); `fuera_de_golden` hace pánico con una ruta de `golden/png/` (`should_panic`); el comparador de instantáneas da error si cambia un byte o aparece un fichero (el control prueba que **sabe fallar**) |
| 10 | **px-app.** PNG que Pixoko convierte: RGB de 8 bits, gris de 4 bits, gris de 16 bits con `tRNS`, uno con `iCCP` (construidos en la prueba) y, con los mismos criterios, los reales que lo hagan | el primer `save` escribe `-copia` y deja intacta la copia de origen (`copy_reason` es `Some`); el segundo `save` sobrescribe la copia; la relectura da la imagen del modelo; un real sin conversión se sobrescribe en `T`, nunca en `golden/png` |
| 11 | **px-golden.** Los 4 reales: editar con `SetPixels`, `FillSpans`, `SetTransparentIndex` y un cambio de paleta de `Document`, `write_png`, `read_png` | el SHA-256 de la RGBA expandida de la relectura es el del modelo (la misma técnica del corpus); un modelo con **un píxel cambiado** da otro SHA-256 (control) |
| 12 | **px-app, `#[ignore]`.** RGBA 8192² e indexada 6000²: generar y guardar con `write_png`, `Session::load`+`install`, 3 píxeles de lápiz, una línea horizontal de lado a lado, un relleno, `undo` ×3, `redo` ×3, `save`, relectura | imagen = esperada por fórmula cerrada sobre `Image::row` (sin segundo relleno), tras cada `undo`/`redo` también; sin pánico ni error de memoria; tiempos por `eprintln`; ningún fichero fuera de `T`, que se borra |

## Propiedades (`proptest`)

1. **Guion aleatorio contra el modelo (px-app).** Imagen de 1 a 24 de lado en los cuatro modos y una secuencia aleatoria de gestos del guion (el modelo restringe la línea a H, V y 45°), operaciones de paleta, conversiones exactas y `save`: tras cada paso `Session` = modelo; al final, relectura = modelo normalizado.
2. **Historia (px-app).** Tras una secuencia aleatoria de ediciones, `undo` k veces da la instantánea anotada tras el paso `n − k` para todo k, y `redo` las vuelve a dar; `modified` es falso si y solo si el `state_id` es el guardado, y **nunca da falso con una imagen distinta de la guardada**.
3. **Solo se escribe lo que se pide (px-app).** Con secuencias aleatorias de `save`, `save_as` y `save_to` (nombres elegidos como los de un diálogo) desde una copia: el directorio de trabajo contiene exactamente los ficheros de los `Saved` devueltos (sin `.pixoko-tmp`), el contenido de cada uno es la relectura esperada, y `golden/png/` queda igual.
4. **Reales mutados (px-app).** Un real con 1 a 4 bytes cambiados: `open` da `Ok` o `Err` sin pánico; con `Err` la sesión queda igual (documento, ruta, `modified`); con `Ok`, editar, guardar y reabrir da el modelo.

## Dónde van las pruebas

- `crates/px-app/src/puerta/` (módulo `#[cfg(test)]` de `main.rs`): `comun` (directorio de trabajo, fuentes, modelo, comparador, `FakePicker` propio), `filas`, `grande` (`#[ignore]`) y `propiedades`. Las construcciones de PNG con chunks auxiliares se toman de `session/aux_tests.rs` en forma, sin importarlas.
- `crates/px-golden/tests/png_interop/gate_oraculo.rs`: módulo nuevo de `png_interop.rs` (que ya tiene `common` y el corpus): menos ejecutables enlazados.
- Sin tocar ninguna prueba anterior. Si alguna necesitara más que cambiar cifras, me paro (regla 4 de `CLAUDE.md`).
- **Windows y macOS solo se comprueban en el CI** tras el primer `push`: aquí solo corro Linux. Las filas con permisos o enlaces siguen saltándose como en la unidad 15.

## Orden de commits

1. Renumeración (hecho, aparte, solo specs). 2. Esta spec y su fila del INDEX. 3. Pruebas (px-app y px-golden). 4. `fuzz/deny.toml`, `deny.toml`, `DEPENDENCIES.md` y `FUZZING.md` (D9). 5. Resultado del fuzzer en esta spec (D10). 6. Cierre del hito: `hito-2-resumen.md`, `PLAN-RESUMEN.md` e INDEX (D8). Cierre de la unidad: `cargo fmt`, `clippy -D warnings`, `cargo test` y `cargo deny` (con la ejecución de los grandes y el fuzzer una vez), y el `license-auditor` una sola vez sobre el rango. Sin `px-app` de producción.

## Cierre del hito: contenido de `hito-2-resumen.md`

- **Unidades:** cerradas (16, 17a, 17, 17b, 18, 19, 19b, 20, 13b, 12, 13, 14a, 14-limpieza, 14c, 21 y esta), **descartada** la 14b (D2 del reparto: la prueba manual con 6000² y 8192² salió fluida; se retoma solo si el ciclo de color de los hitos 5 o 7 lo exige) y **aplazadas** las de abajo, cada una con el enlace a su nota.
- **Aplazado, ya verificado en las specs leídas:** pincel, rectángulo y elipse y el cuantizador de «Convertir» (hito 4); guardar en disco las preferencias, incluido el fondo del lienzo (hito 8, D12); escritor de PNG por flujo (pospuesto en la 13; cifras en las notas de la 14); conservar los chunks auxiliares al guardar y la gestión de color (`gAMA`, `iCCP`…; hoy solo se informa y `iCCP` fuerza copia) cuando el documento tenga dónde guardar metadatos; APNG (con capas y tiempo); ficheros recientes y directorio recordado entre sesiones; `fsync` del directorio y limpieza de temporales; rechazar un diálogo abierto de Linux desde la app.
- **Aplazado, por localizar en las notas:** marca de transparencia al guardar, relleno por filas, realce de la fila marcada y alfa base opaco por campo, más «otros». Lectura **autorizada** (2026-10-20) de `specs/hito-2-notas-unidad-14.md` y `grep` de «aplaz» en `specs/` al escribir el resumen: cada cosa aplazada se cita con su spec y su fila de origen y **no se inventa ninguna**.
- **Cifras medidas:** solo las que ya constan en una spec, con su enlace (p. ej. escritura a 8192² RGBA 257 → 1 MiB y lectura 511 → 256 MiB; fichero truncado de 66 bytes 328 ms y +1024 MiB → 0 ms y +0,3 MiB; peine del relleno +1,25 GiB y 2,5 s), más las nuevas de esta unidad (tiempo y pico de los grandes, ejecuciones del fuzzer).

## Prueba manual (la haces tú, sin capturas; ≤15 pasos)

`Cmd` donde digo `Ctrl` en macOS; `cargo run --release -p px-app`.

1. Copia los 4 PNG de `golden/png/` a una carpeta tuya, fuera del repo y de `/tmp`. Todo lo demás usa las copias.
2. Abre la copia indexada (`-- <ruta>`): ventana con el nombre en el título, píxeles nítidos, zoom entero.
3. Pinta con lápiz, línea, relleno y borrador (`P`, `L`, `F`, `E`), edita el color de una entrada y pulsa *Fijar transparente*: el título lleva `*`.
4. `Ctrl+Z` hasta el principio y rehaz hasta el final: la imagen coincide con lo que dibujaste y el `*` se va al volver al estado guardado.
5. `Ctrl+S` guarda (si el PNG se convierte al abrir, escribe `-copia` y el aviso lo dice). `Ctrl+Mayús+S`: diálogo nativo, modal, con nombre propuesto; un nombre sin extensión se guarda con `.png`.
6. `Ctrl+O` y elige lo guardado: se abre idéntico (compara a ×8 unos píxeles clave).
7. Repite 2 a 6 con la copia RGBA.
8. En Linux, lanza con X11 (`env -u WAYLAND_DISPLAY`; en Wayland soltar ficheros no está disponible) y suelta un PNG desde el explorador sobre la ventana: se abre; con cambios pendientes avisa y solo reemplaza al soltar otra vez en 5 s.
9. Zoom 1/2, 1/4 y 1/8 en la RGBA y en la indexada: sin costuras ni líneas y sin temblor al desplazar.
10. HiDPI: a 100 % y a 150–200 % (en macOS, Retina y una pantalla externa): píxeles nítidos, texto legible, ventana mínima.
11. *Nueva imagen* de 8192×8192 RGBA: pinta, rellena, deshaz, guarda y reabre; igual con 6000×6000 indexada. Sin tirones graves y sin memoria desbocada (monitor del sistema).
12. Con cambios sin guardar, cierra la ventana (la X; `Alt+F4` en Windows): el primer cierre avisa y el segundo cierra.
13. Solo macOS: con cambios sin guardar, `Cmd+Q` debería avisar como el paso 12. **No está probado**: anota qué ocurre.
14. Solo Linux: repite 5 y 8 en X11 (`env -u WAYLAND_DISPLAY`) y en Wayland.
15. Al terminar: `git status golden/` limpio y ningún `.pixoko-tmp-*` en las carpetas usadas.

## Resultado del fuzzer

Ejecutado por el usuario el 2026-10-20, al cierre de la unidad, con el código ya fijado (D10), y anotado tal como lo comunicó:

```
cargo +nightly fuzz run read_png fuzz/corpus/read_png golden/png -- -rss_limit_mb=4096 -timeout=10 -create_missing_dirs=1 -max_total_time=600
```

- **12 733 737 ejecuciones en 601 segundos** (≈ 21 200 por segundo).
- **Ningún fallo**: `fuzz/artifacts/read_png/` vacío (sin `crash-*`, `oom-*`, `timeout-*` ni `slow-unit-*`).
- **`golden/` intacto** al terminar.
- No se comunicaron la cobertura final ni el RSS máximo, y no se inventan aquí.

## Riesgos

- Las pruebas nacen en verde: un fallo real aparecerá como defecto de una unidad anterior y puede abrir trabajo nuevo.
- El modelo es una segunda implementación ingenua: un error común a modelo y código no se vería. Lo limitan las líneas solo H, V y 45° y el oráculo SHA-256 de la fila 11.
- Windows y macOS solo se verán tras el `push`.
- La nightly sin `rust-src` bloquea el fuzzer hasta que lo instales.
- `Cmd+Q` en macOS puede no pasar por la confirmación de cierre (sin probar).
