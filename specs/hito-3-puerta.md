# Hito 3 · Unidad 3.9: puerta del hito (sesión con capas, mezclas, combinar y aplanar, guardar y reabrir)

Estado: **aprobada** (2026-10-22; decisiones 1 a 8 con las condiciones de «Visto bueno» al final, que **prevalecen sobre cualquier otra frase de la página**). Sin código de producción nuevo salvo, si la fila 10 lo exige, el aviso «Guardando…» al guardar un `.aseprite` grande (condición a); solo pruebas y documentación; no se toca `px-core` (salvo el comentario de `main_image`, condición 4), `px-raster` ni `px-formats`, y `px-golden` no gana dependencias. Si una fila falla contra el código actual es un defecto de una unidad anterior: me paro y te lo cuento (regla 4 de `CLAUDE.md`), sin arreglarlo en silencio.

Referencias: fila 3.9 y sección 3 de [`mapa-funciones.md`](mapa-funciones.md); modelo [`hito-2-puerta.md`](hito-2-puerta.md) y [`hito-2-resumen.md`](hito-2-resumen.md); [`hito-3-px-app-lib.md`](hito-3-px-app-lib.md) (condición 2: esta unidad decide si la puerta pasa a `px-golden`); [`hito-3-mezcla.md`](hito-3-mezcla.md) (3.2: tolerancias y «Datos conocidos»); [`hito-3-aseprite.md`](hito-3-aseprite.md) (3.8); [`corpus-hito-3.md`](corpus-hito-3.md) (los 29 casos).

**Fuera del hito (anotado):** **3.6 (página de reserva) y 3.7 (imágenes de referencia)** son deseables, no estaban en la puerta y quedan **aplazadas fuera del hito 3**; el hito se cierra sin ellas. Origen: filas 3.6 y 3.7 de `mapa-funciones.md` (decisiones D09 y D11, «en su spec», sin escribir). Se anotan así en `hito-3-resumen.md` y en el INDEX.

## Objetivo

Demostrar el hito 3 de punta a punta: una sesión con capas, mezclas y opacidad, combinar y aplanar, convertir de modo con varias capas, deshacer y rehacer todo, guardar en `.aseprite` y en PNG aplanado y reabrir, con el documento reabierto igual; en RGBA y en indexado, en Linux, Windows y macOS. La parte **automática** corre en el CI de los tres sistemas con `cargo test --workspace --locked` (sin xvfb y sin tocar `ci.rs`); la **manual** la haces tú. Al final, el cierre del hito.

## Lo medido (programa desechable en `~/.cache/pixoko-scratch`, fuera del repositorio y de `/tmp`, **borrado al terminar**; solo lee `golden/aseprite/`)

Con `read_aseprite`, `write_aseprite`, `compose`, `flatten_command` y `merge_visible_command` sobre los 29 `.aseprite`:

| Qué | Resultado |
| --- | --- |
| Escribir, leer y escribir otra vez | **29 de 29** con los mismos bytes en la segunda escritura |
| Campos de capa (banderas, tipo, nivel, mezcla, opacidad, nombre) del fichero escrito frente al que escribió Aseprite | **29 de 29 idénticos** (los de celda: capa, opacidad 255, tipo 2 y z-index 0, también) |
| Diferencias del fichero escrito frente al de Aseprite (todas declaradas en 3.8) | sin chunk `0x0004` (la paleta va en `0x2019` solo en indexado, 4 casos), número de colores 0 en RGBA (Aseprite: 16), una celda menos donde la celda del todo fuera o transparente no se escribe (`celda-recortada` 6 → 4, `celda-vacia` 2 → 1) |
| `compose` del documento reabierto frente al original | igual en 25; **distinto en 4** (`fondo-con-opacidad` 112 píxeles, las dos rejillas 18 cada una, `rgba-una-capa` 3) y **en los 133 solo cambia el RGB de un píxel con alfa 0** (con alfa > 0: 0 diferencias) |
| `flatten_command` y `merge_visible_command` aplicados al documento leído | `Ok` en 27 y `NothingToMerge` en 2 (`rgba-una-capa`, `indexado-paleta-256`, de una sola capa); en los 27, `compose` antes y después **igual byte a byte** y `undo` devuelve las capas y el mismo `compose` |

Del 4.º renglón sale la **decisión 3**: al escribir se guarda el rectángulo mínimo no transparente (3.8, decisión 9) y se pierde el RGB de los píxeles con alfa 0 fuera de él; la condición 1 de 3.8 ya los declara iguales. No es un defecto, pero hay que escribirlo en las comparaciones.

## Dónde va cada pedido (decisión 1, con datos)

**La puerta del hito 2 se queda en `px-app`** (`#[cfg(test)] mod puerta`), con los goldens leídos por ruta (`raiz_repo()` ya existe). Datos:

| | Mover a `px-golden` | Dejarla en `px-app` |
| --- | --- | --- |
| Qué mueve | 3726 líneas (`comun` 1865, `filas` 1028, `grande` 349, `propiedades` 484) y **30 pruebas** (22 filas, 4 propiedades, 4 grandes); `damero_guardado` (411 líneas, usa `App`) no puede moverse | nada |
| API que hay que abrir | los 11 nombres de la 3.0 (`Session`, `Saved`, `FileError`, `Confirm`, `DialogKind`, `DialogRequest`, `Dialogs`, `FilePicker`, `Finished`, `Outcome`, `open_gate`). **`Session` arrastra** 9 campos públicos, 7 de ellos tipos de `px-ui` (`ColorChoice`, `Role`, `Scroll`, `PanelView`, `SlotBank`, `ColorEditor`, `PaletteOps`), y la puerta usa unos 14 de sus miembros (`document`, `path`, `modified`, `converted`, `open`, `install`, `undo`, `redo`, `save*`, `convert_mode`, `absorb`…) | ninguna: la lib sigue exportando solo `run` |
| Coste en `px-golden` | `Cargo.toml` y `Cargo.lock` cambian (`px-app` y `px-ui` como dev-dependencias; sin crates externas nuevas); `cargo test -p px-golden` pasa de compilar **41 a unos 180 paquetes** (`eframe`, `winit`, `glow`…) | nada |
| Cobertura que gana | **ninguna**: `cargo test --workspace` ejecuta las mismas 30 pruebas en los tres sistemas | — |
| Lo que la puerta de capas necesita | `Trabajo`, instantáneas SHA-256, `Mando`/`FakePicker`, `Lcg`: viven en `puerta::comun`, y habría que moverlos con ella | los reutiliza tal cual |

Reparto de la unidad:

| Dónde | Qué |
| --- | --- |
| **`px-golden`**, módulo nuevo `tests/png_interop/aseprite_puerta.rs` (sin ejecutable nuevo; reutiliza `CASOS`, `common::sha256` y las funciones de comparación de `aseprite_read`) | lo que **no necesita `Session`**: los 29 del corpus con el lector y el escritor, la estructura del fichero escrito, los controles negativos de los ficheros y una propiedad. Todo en memoria: **no escribe nada en disco** |
| **`px-app`**, módulo nuevo `#[cfg(test)] mod capas` dentro de `puerta` (`crates/px-app/src/puerta/capas/`; `puerta.rs` solo gana `mod capas;`) | todo lo que usa `Session`: guion de punta a punta, deshacer y rehacer, guardar y reabrir, los 29 por la sesión, los grandes y la comprobación de `golden/aseprite/` |

Ninguna prueba anterior cambia (ni `comun.rs`: `fuera_de_golden` sigue vigilando `golden/png/`; la nueva ayuda es aparte).

## Modelo independiente de la pila (px-app)

`ModeloPila`: ancho, alto, modo, paleta y una lista de capas (nombre, visible, opacidad, mezcla, bloqueada, rejilla de celdas o `None`) más la capa activa; aplica las mismas órdenes con código ingenuo (rejilla de celdas; relleno por cola; línea solo H, V y 45°, como en la puerta 2). **No reimplementa la composición**: las fórmulas de mezcla ya las fijan 3.2 (17 + 29 goldens contra Aseprite) y 3.5. Lo esperado sale de tres cosas:
1. **La pila** (propiedades, orden, contenido de cada celda, activa, `state_id`, «modificado» y un paso de historia por orden con cambio): del modelo.
2. **La composición**: `compose` de un documento que el modelo construye con `Document::from_layers` (un camino distinto del de la sesión: sin historia y sin comandos).
3. **Vectores a mano** (sin ningún código): los valores de la fila 2 de 3.2 (fondo `(196,46,52,255)` y origen `(90,60,220,255)`: Multiplicar `(69,11,45)`, Trama `(217,95,227)`, Sumar `(255,106,255)`, Diferencia `(106,14,168)`; opacidad 64 de rojo sobre negro `(49,11,13)`), leídos en la composición de la sesión, en el PNG exportado y en el `.aseprite` reabierto.
Combinar abajo, combinar visibles y aplanar: el modelo calcula la celda resultante con `compose` de las capas que se unen (como `merge_*_command`); la capa que queda es la de abajo (o la visible más baja) con opacidad 255, Normal y desbloqueada, y el nombre de la de abajo.

**Igualdad «sin ids»** (condición 1 de 3.8): `mismo_contenido`, copia propia en `px-golden` y en `px-app` (≈ 30 líneas cada una; no se exporta nada de `px-formats`): mismas capas en el mismo orden con nombre, visibilidad, opacidad, mezcla y bloqueo iguales, mismo contenido de píxeles (una celda transparente equivale a una vacía; dos píxeles transparentes son iguales aunque su RGB difiera), mismo lienzo, paleta (colores y alfa) e índice transparente.

## Casos (12 filas)

`T` es un directorio de trabajo `<repo>/target/pixoko-gate/<pid>-<n>` (el de la puerta 2, borrado siempre con un guardián `Drop`; nunca `/tmp`). Filas 1 a 4 en `px-golden`; 5 a 11 en `px-app`; la 12, órdenes del cierre.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | **px-golden.** Los 29 `.aseprite` leídos: `flatten_command` y `merge_visible_command` aplicados a copias del documento | los 27 con `Ok`: una sola capa (aplanar) o las ocultas intactas (combinar visibles), `compose` igual al de antes **y igual al PNG** con las tolerancias de 3.2 (0 fuera del dominio empírico; techo 2 y suelo 1 por caso agregado dentro, **sin ensancharlo**); los 2 de una capa dan `NothingToMerge` (lista fija); `undo` devuelve las capas |
| 2 | **px-golden.** Los 29: `write_aseprite`, `read_aseprite`, `write_aseprite` | `mismo_contenido` con el original (sin ids); `lossy` falso y un fotograma; la segunda escritura **mismos bytes**; `compose` del reabierto igual al del original con alfa exacto y RGB exacto salvo en los píxeles de alfa 0 (decisión 3), e igual al PNG con las tolerancias de la fila 1 |
| 3 | **px-golden.** Estructura del fichero escrito frente a la del que escribió Aseprite, con un lector de cabecera, chunks, capas y celdas propio de la prueba, escrito **solo con las tablas de la especificación pública** (sin descomprimir) | en los 29: cabecera (ancho, alto, profundidad, banderas 1, un fotograma, duración 100, cuadrícula `0, 0, 16, 16`), mismas capas en el mismo orden con **banderas, tipo, nivel hijo, mezcla, opacidad y nombre idénticos**, celdas con capa, opacidad 255, tipo 2 y z-index 0; **diferencias permitidas, lista cerrada** (las 4 del renglón de «Lo medido»); cualquier otra, `Err` con el campo, el caso y los dos valores |
| 4 | **px-golden, controles negativos.** (a) la lista de nombres de `golden/aseprite/` sin un caso, y con uno de más; (b) un PNG del corpus con **un píxel cambiado** (se reescribe con `write_png`) y con un `.sha256` de otra imagen; (c) el fichero escrito de `mixto-rgba` con **un campo cambiado** de cada vez en su desplazamiento (el lector de la fila 3 los localiza): banderas de capa (visible, editable), mezcla, opacidad, tipo de celda, z-index, profundidad y magia | (a) `Err` con el nombre del caso (la lista de 29 es la de `aseprite_corpus`); (b) la composición contra el PNG da `Err` con el primer píxel y el corpus da `Err` por el `.sha256`; (c) la fila 3 da `Err` con el campo que cambió, uno por uno: **la prueba sabe fallar si el escritor se aparta de la especificación**. Además, al cierre, **una mutación real** del escritor (un código de la tabla de mezcla o la bandera «Editable»): las pruebas fallan; se restaura y pasan (se anota el resultado, como en 3.8) |
| 5 | **px-app, RGBA.** Guion de punta a punta con `Session` y órdenes reales: `new_layer`, `rename_layer`, `set_layer_blend` (los ocho modos, uno por capa), `set_layer_opacity`, `set_layer_visible`, `set_layer_locked`, `duplicate_layer`, `move_layer`, `remove_layer`; lápiz, línea, relleno y borrador por `ToolState::canvas_event` (el camino de la puerta 2) sobre la capa activa; `merge(Down)`, `merge(Visible)`, `merge(Flatten)` (con ocultas y con la confirmación de 5 s de `flatten_gate`); `convert_all` | pila, composición y vectores a mano = modelo tras cada orden; **un paso de historia por orden con cambio** y ninguno si se rechaza (pintar en capa oculta o bloqueada, combinar abajo sobre oculta o bloqueada, aplanar con una capa); `convert_mode` con varias capas sigue dando `Err` sin tocar nada; la capa activa es la del modelo; `modified` cierto |
| 6 | **px-app, indexado.** El mismo guion con lo que el indexado admite: capas, visibilidad, bloqueo, lápiz, línea, relleno, borrador, combinar y aplanar; `set_layer_opacity` y `set_layer_blend` ≠ valor por defecto; `convert_all` indexado → RGBA (exacto, paleta común) y RGBA → indexado con ≤ 256 colores; uno que pierde datos sin permiso | opacidad y mezcla: `Err(IndexedOpacity)`/`Err(IndexedBlend)` sin paso; combinar y aplanar = modelo (la visible más alta con índice no transparente gana); **la paleta idéntica en todas las celdas** tras cada orden; las conversiones exactas dan el modelo en **un paso**; la que pierde da `Err` sin cambiar documento ni historia |
| 7 | **px-app.** Con el guion de las filas 5 y 6 hecho: `undo` hasta el principio, `redo` hasta el final, y `undo` + orden nueva + `redo` | tras cada `undo` y `redo`, pila (con la capa activa) = la instantánea de ese paso, y `compose` = el del modelo en ese paso; al principio, el documento original exacto, `state_id` 0 y `modified` falso; al final, el final; con la orden nueva `redo` falla y el estado guardado deja de ser alcanzable |
| 8 | **px-app.** Guardar y reabrir, en RGBA y en indexado: `save_as`/`Ctrl+S` por el diálogo (`Mando`, el selector falso) a `.aseprite`; `save_to` a `.png` (aplanado, con su aviso); `export_flat`; `Session::open` de lo escrito | el `.aseprite` reabierto es `mismo_contenido` con el documento de la sesión y su capa activa es la de arriba; el PNG leído = `compose` (alfa exacto y RGB exacto salvo alfa 0) y los vectores a mano; `Ctrl+S` de un PNG con ≥ 2 capas abre el diálogo proponiendo `.aseprite`; tras guardar como `.aseprite` la sesión está en ese fichero y no modificada; `export_flat` no cambia ruta, historia ni «modificado»; ningún `.pixoko-tmp` |
| 9 | **px-app.** Los 29 por la sesión: copia en `T`, `Session::open` (por contenido), aviso, `save`, `Session::open` otra vez | `Ok`; el aviso lleva nombre, tamaño, capas y modo; `converted()` falso y sin `lossy` (ninguno del corpus los tiene); `state_id` 0 y `modified` falso; capa activa la de arriba; el `save` sobrescribe la copia con `.aseprite` y la relectura es `mismo_contenido`; `golden/aseprite/` no se toca |
| 10 | **px-app, `#[ignore]`, release, un hilo, sin aserciones de tiempo.** **RGBA 8192² con 4 capas** (relleno total plano; franjas por fórmula que cubren todo; trazo y línea larga; Multiplicar con opacidad 128 y medio lienzo) e **indexado 6000² con 4 capas** (mismo guion): componer, `merge(Visible)`, `undo`, `redo`, `export_flat`, `save` a `.aseprite`, **soltar la sesión** y `Session::open` | cada fila de cada capa tras abrir = la fórmula cerrada (sin un segundo documento: el pico no se duplica), y tras cada `undo`/`redo` también; sin pánico ni error de memoria; tiempos por `eprintln` (con el **tiempo de guardar el `.aseprite` de 8192² RGBA aparte**, condición 6) y pico de RSS (`ru_maxrss` del proceso, como en la 22); **las cifras van a esta spec** (corrección posterior); si guardar pasa de 2 s, se añade «Guardando…» con su prueba de la secuencia; todo en `T` |
| 11 | **px-app.** `golden/aseprite/` al terminar toda la puerta de capas; control negativo | los 88 ficheros con los mismos bytes **y el SHA-256 de cada uno** igual al de la instantánea del principio (misma técnica y mismo SHA-256 propio que la puerta 2: `instantanea_de` y `comparar_instantaneas`); la ayuda `fuera_de_golden_aseprite` hace pánico con una ruta de `golden/aseprite/` (`should_panic`) y toda ruta de escritura de la unidad pasa por ella; el comparador da `Err` si cambia un byte o aparece un fichero (**sabe fallar**); `T` se borra aunque la prueba falle |
| 12 | **Cierre.** Órdenes, no pruebas | `git diff` no toca `px-core`, `px-raster`, `px-formats`, `px-golden/Cargo.toml`, `Cargo.lock`, `deny.toml`, `ci.rs` ni `ci.yml`; la lista de nombres de prueba de antes (`cargo test --workspace -- --list`, guardada fuera del repo) es un subconjunto de la de después; `cargo deny` igual que antes; fmt, clippy `-D warnings` y test del workspace limpios |

## Propiedades (4; `proptest`, 64 casos las de `px-app` salvo la 3 con 32 porque tocan el disco, y 128 la de `px-golden`; se baja con `PROPTEST_CASES` solo al desarrollar)

1. **Guion aleatorio contra el modelo (px-app).** Lienzo de 1 a 12 de lado, RGBA o indexado, 1 a 6 capas y una secuencia aleatoria de órdenes de capa, pintura, mezcla, combinar, aplanar y conversión exacta: tras cada paso la pila de `Session` = el modelo y `compose` = el de `from_layers`; al final, guardar en `.aseprite` y reabrir da `mismo_contenido`.
2. **Historia con capas (px-app).** Tras una secuencia aleatoria, `undo` k veces da la instantánea anotada tras el paso `n − k` para todo k (con su capa activa, que **existe siempre**), y `redo` las vuelve a dar; `modified` es falso si y solo si el `state_id` es el guardado.
3. **Solo se escribe lo que se pide (px-app).** Con secuencias aleatorias de `save`, `save_as`, `save_to` y `export_flat` (nombres `.aseprite` y `.png` elegidos como los de un diálogo) desde copias: `T` contiene exactamente los ficheros de los `Saved` devueltos y los exportados (sin `.pixoko-tmp`), el contenido de cada uno es la relectura esperada, y `golden/aseprite/` y `golden/png/` quedan iguales.
4. **La composición sobrevive a guardar y reabrir (px-golden).** Para un documento aleatorio (RGBA e indexado, de 1 a 12 de lado, 1 a 6 capas con mezcla, opacidad, ocultas y celdas vacías): `compose` del reabierto es el del original con alfa exacto y RGB exacto salvo en los píxeles de alfa 0; `flatten`, `merge_visible` y `write`→`read` conmutan (aplanar y escribir da lo mismo que escribir y aplanar, salvo ese RGB).

Tope: unas **30 pruebas** (12 filas con sus variantes, 4 propiedades y 2 `#[ignore]`), por debajo de las 40 del modo ahorro. Sin `test-writer` y sin `license-auditor` (no hay dependencias ni código de producción nuevos), sin subagentes.

## Qué cubre lo automático y qué queda para la manual (decisión 2)

| Automático (CI, tres sistemas) | Manual (tú) |
| --- | --- |
| `Session` con capas, mezcla, opacidad, bloqueo, combinar, aplanar, `convert_all`, historia, herramientas con eventos sintéticos, diálogos con selector falso, `.aseprite` y PNG de ida y vuelta, los 29 del corpus, la estructura del fichero escrito frente a la de Aseprite, lienzos grandes **lógicos** (`#[ignore]`, sin textura) | Ventana (`winit`), OpenGL y textura por baldosas con capas, panel de capas flotante (arrastre, geometría, foco, `F1`), teclas `'` `;` `)` `(` con el teclado real, diálogos nativos (`rfd`) con sus filtros, soltar ficheros, HiDPI, X11 frente a Wayland, **Aseprite abriendo lo que escribe Pixoko** (el CI no lo ejecuta), fluidez real con capas a 8192² y 6000², `Cmd+Q` con cambios |

Razón: lo de la derecha necesita pantalla, GPU, el sistema de ventanas o el programa de referencia, que el CI no tiene; y la regla de `CLAUDE.md` sobre capturas deja la prueba visual en tus manos.

## Prueba manual (la haces tú, sin capturas; ≤ 15 pasos; `Cmd` donde digo `Ctrl` en macOS)

**Preparación.** Una carpeta tuya fuera del repo y de `/tmp` (p. ej. `~/pixoko-prueba-h3/`) con **copias** (nunca abras los de `golden/`): de `golden/aseprite/`, `mixto-rgba`, `indexado-capas`, `capas-ocultas`, `celda-recortada`, `mezcla-ocho-fondo-translucido` y `capa-bloqueada`, cada uno con su `.aseprite` y su `.png`; de `golden/png/`, `libresprite-golden-rgba.png`. Por ejemplo: `mkdir -p ~/pixoko-prueba-h3 && cp golden/aseprite/{mixto-rgba,indexado-capas,capas-ocultas,celda-recortada,mezcla-ocho-fondo-translucido,capa-bloqueada}.* golden/png/libresprite-golden-rgba.png ~/pixoko-prueba-h3/` (en Windows, `Copy-Item` o el explorador). `cargo run --release -p px-app -- <ruta>`.

1. Abre `mixto-rgba.aseprite` y el panel (`'`): 6 capas en su orden, nombres, la oculta con el ojo cerrado, y la vista igual que `mixto-rgba.png` en un visor externo (a ojo, sin capturas).
2. Lo mismo con `indexado-capas.aseprite` (en el editor de paleta, índice transparente el 0) y `capas-ocultas.aseprite`.
3. Sobre `mixto-rgba`: `;` (capa nueva), `(` y `)` (bajar y subir), pinta con lápiz, línea y relleno; cambia la mezcla a Multiplicar y la opacidad a 128 desde el panel: la vista cambia al instante, sin costuras.
4. `Capa → Combinar abajo`, `Combinar visibles` y `Aplanar` (con una capa oculta con píxeles pide repetir en 5 s). `Ctrl+Z` devuelve las capas y `Ctrl+Mayús+Z` las vuelve a combinar.
5. `Ctrl+Z` hasta el principio y rehaz hasta el final: la vista coincide en cada punto y el `*` del título se va al volver al estado guardado.
6. `Ctrl+Mayús+S`: diálogo nativo, modal, con nombre propuesto `.aseprite`; guarda `prueba.aseprite`, ciérralo y ábrelo: idéntico. Con la RGBA de `golden/png/` (una capa) añade una capa y `Ctrl+S`: propone `.aseprite`; elige `.png` y el aviso dice «se guarda aplanado».
7. `Archivo → Exportar PNG aplanado…`: ábrelo en un visor externo y compáralo con la vista.
8. `Imagen → Convertir modo…` con varias capas: RGBA → indexado en una imagen de pocos colores e indexado → RGBA (un solo paso de deshacer); a Gris con varias capas avisa «aplana antes».
9. **En los tres sistemas, con Aseprite:** anota su versión (`aseprite --version` o Ayuda → Acerca de) y abre `prueba.aseprite` (la de Pixoko): orden de capas, nombres, opacidades, mezclas, capa oculta, capa bloqueada y paleta correctos, sin avisos; bloquea una capa allí, guarda y ábrela en Pixoko.
10. `Nueva imagen` de 8192×8192 RGBA con 4 capas (a la quinta avisa del presupuesto): pinta, rellena, `Combinar visibles` («Combinando…»), deshaz, guarda `.aseprite` y reabre; igual con 6000×6000 indexada. Sin tirones graves y sin memoria desbocada (monitor del sistema).
11. Panel: arrástralo, achica la ventana al mínimo y cambia la escala: sigue dentro; un clic fuera del panel pinta; `F1` sobre el panel abre su ayuda.
12. HiDPI: a 100 % y a 150–200 % (en macOS, Retina y pantalla externa): iconos, texto y panel nítidos.
13. Con cambios sin guardar con capas, cierra la ventana: el primer cierre avisa y el segundo cierra. Solo macOS: `Cmd+Q` debería avisar igual; **no está probado**: anota qué ocurre.
14. Linux: suelta un `.aseprite` sobre la ventana solo en X11 (`env -u WAYLAND_DISPLAY`; en Wayland no está disponible, causa en `hito-2-abrir-guardar.md`); Windows y macOS: abrir y guardar con filtros «Aseprite» y «PNG» y con rutas con espacios y acentos.
15. Al terminar: `git status golden/` limpio y ningún `.pixoko-tmp-*` en las carpetas usadas.

## Cierre del hito (decisión 8)

Contenido de **`specs/hito-3-resumen.md`** (fila «notas» en el INDEX; **estado: cerrado, a falta de la prueba manual de la 3.9**, y «cerrado» en un commit de una línea cuando tú confirmes):

- **Unidades:** cerradas 3.0, 3.1, 3.2, 3.3, 3.4, 3.5, 3.8 y 3.9, cada una con el enlace a su spec; **aplazadas fuera del hito 3: 3.6 y 3.7** (deseables; origen: `mapa-funciones.md`, filas 3.6 y 3.7, D09 y D11).
- **Aplazado a los hitos siguientes**, cada cosa con su spec y su fila de origen y **sin inventar ninguna**. Ya verificado en las specs que he leído: gris con alfa del `.aseprite` (`GrayAlpha`, caso `gris-alfa-capas`), grupos, mapas de tiles, referencia de capa, opacidad de celda, z-index, los diez modos de mezcla restantes, paleta de 6 bits, perfil ICC, propiedades de usuario y conservar los chunks no entendidos (hito 6; `hito-3-aseprite.md`, «Fuera de alcance»); fotogramas, duraciones, etiquetas y celdas enlazadas (hito 5, 5.5); teclas por defecto de combinar y aplanar, miniaturas, combinar varias capas elegidas a mano, composición con hilos y barra de progreso (`hito-3-combinar.md`, «Fuera de alcance»; hilos y barra, hito 8); caché de baldosas compuestas y composición con hilos (`hito-3-mezcla.md`, «Fuera de alcance»); el tiempo de guardar y abrir con muchas capas grandes (`hito-3-aseprite.md`, corrección 10; **lo mide la fila 10**). El resto se localiza al escribir el resumen con `grep` de «aplaz» y «Fuera de alcance» en las specs 3.1, 3.3 y 3.4 (**te pido autorización de lectura**, como en la 22) y cada fila lleva su origen.
- **Cifras medidas:** solo las que ya constan en una spec (con enlace) más las nuevas de la fila 10 (tiempo de cada paso y pico de RSS) y las ejecuciones de los fuzzers.
- **`docs/PLAN-RESUMEN.md`:** el hito 3 pasa a «cerrado, a falta de la prueba manual de la unidad 3.9», con un puntero a `hito-3-resumen.md`; la tabla de crates y la nota «puerta a `px-golden` en 3.9» se corrigen («decidido: se queda en `px-app`»). `mapa-funciones.md`: fila 3.9 y la sección 9 (aplazadas).
- **`main_image()` (decisión 7).**

## `cargo deny` (decisión 5)

Medido hoy con `cargo deny --locked check`: `advisories ok, bans ok, licenses ok, sources ok` y **17 avisos `duplicate`: exactamente los 17 de la línea base** de `docs/DEPENDENCIES.md` (`bitflags`, `block2`, `getrandom`, `jni-sys`, `linux-raw-sys`, `miniz_oxide`, `objc2` y sus tres `objc2-*`, `r-efi`, `redox_syscall` ×3, `rustix`, `syn`, `thiserror` y `-impl`, `windows-sys` ×3). El hito 3 no añadió ninguno: `miniz_oxide` ya estaba en la lista (0.8.9 directa de `px-formats` y de `png`, 0.9.1 de `flate2`). **Propuesta: los 17 siguen aceptados sin `skip`** (un `skip` taparía un duplicado nuevo; se vuelve a mirar al actualizar `eframe`, `rfd` o `png`) y la línea base no cambia; se repite el del fuzzer (`cargo deny --manifest-path fuzz/Cargo.toml --locked --config fuzz/deny.toml check`). Cualquier aviso fuera de la lista es nuevo y me paro.

## Fuzzers (decisión 9; **los lanzas tú**, uno tras otro, al cierre y con el código ya fijado)

```
cargo +nightly fuzz run read_png fuzz/corpus/read_png golden/png -- -rss_limit_mb=4096 -timeout=10 -create_missing_dirs=1 -max_total_time=600
cargo +nightly fuzz run read_aseprite fuzz/corpus/read_aseprite golden/aseprite -- -rss_limit_mb=4096 -timeout=10 -create_missing_dirs=1 -max_total_time=600
```

No ejecuto `rustup`. Me pasas el resultado de cada uno (ejecuciones, segundos, fallos) y va a «Resultado de los fuzzers» de esta spec, en su propio commit; no se inventan cobertura ni RSS que no comuniques. Al terminar, `git status golden/` limpio. Un fallo sigue `hito-1-fuzz.md` («Fallos») y deja el hito abierto. El último de `read_aseprite` (22 de octubre, 578 371 ejecuciones) fue antes de la prueba de 1032× y de esta unidad; el de `read_png`, del 20 de octubre (12 733 737), antes de los cambios de `Image` y de capas: por eso se repiten los dos.

## Orden de commits (modo ahorro: sin `test-writer`, sin `license-auditor`, sin subagentes)

1. Esta spec y su fila del INDEX (una sola corrección posterior, al final). 2. Pruebas de `px-golden` (filas 1 a 4 y propiedad 4; nacen en verde y sus controles prueban que saben fallar). 3. Pruebas de `px-app` (filas 5 a 11 y propiedades 1 a 3, con `ModeloPila` y la ayuda `fuera_de_golden_aseprite`). 4. Cifras de la fila 10 y la mutación del escritor, en esta spec (la corrección posterior). 5. Resultado de los fuzzers, en su propio commit (cuando me los pases). 6. Cierre del hito: `hito-3-resumen.md`, `PLAN-RESUMEN.md`, `mapa-funciones.md` e INDEX. Sin lógica, sin `px-app` de producción y sin adaptación de pruebas anteriores. Cierre de la unidad, una sola vez: `cargo fmt`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (con `-q` y recortado), `cargo deny check` y la ejecución de los grandes (`cargo test --release -p px-app -- --ignored --test-threads=1 capas`).

## Riesgos

- **Las pruebas nacen en verde:** un fallo real aparecerá como un defecto de una unidad anterior y puede abrir trabajo nuevo; el techo de 2 de 3.2 es un máximo observado: si algún caso de la fila 1 o 2 lo supera, **se re-evalúa con datos y nunca se ensancha** (`hito-3-mezcla.md`, «Datos conocidos»).
- **El modelo no reimplementa la mezcla:** un error de composición común al modelo y al código no se vería; lo cubren los vectores a mano y los 29 goldens de Aseprite.
- **Fila 10:** pico estimado (mío, sin medir) de 1,3 a 2 GiB para el RGBA de 8192² con 4 capas (1 GiB de celdas más el resultado de componer y el búfer comprimido); por eso se suelta la sesión antes de reabrir y se compara por fórmula. **El tiempo de comprimir con `miniz_oxide` nivel 6 a 8192² no está medido** (3.8, corrección 10): si resulta de minutos, lo anoto y la palanca (nivel, recorte de celdas) es una unidad aparte, no de esta.
- Windows y macOS solo se comprueban en el CI tras el primer `push`; las filas con permisos o enlaces se saltan como en la 15.
- Nada de lo que el CI no ejecuta (Aseprite, ventana, GPU) se prueba automáticamente: está en la lista manual.

## Fuera de alcance

3.6 y 3.7 (arriba); mover la puerta del hito 2; retirar `main_image()`; cambiar el escritor o el lector; conservar el RGB de los píxeles con alfa 0 fuera del rectángulo mínimo; cualquier cambio de `px-core`, `px-raster` o `px-formats`.

## Decisiones que necesito que confirmes

1. **La puerta del hito 2 se queda en `px-app`** (con los goldens leídos por ruta) y `px-golden` no usa `Session`: mover 3726 líneas y 30 pruebas no gana cobertura, abre `Session` con sus 9 campos públicos (7 de `px-ui`), cambia `Cargo.toml` y `Cargo.lock` y pasa `cargo test -p px-golden` de 41 a unos 180 paquetes. Alternativa: exportar los 11 nombres y moverla (la 3.0 lo dejó a tu elección).
2. **Reparto:** lo que no necesita `Session` (filas 1 a 4 y propiedad 4) en `px-golden`, en memoria y sin escribir en disco; lo demás en `px-app`, módulo nuevo `puerta::capas`. Ninguna prueba anterior cambia.
3. **RGB bajo alfa 0:** guardar y reabrir cambia el RGB de los píxeles con alfa 0 fuera del rectángulo mínimo (medido: 4 casos, 133 píxeles, todos con alfa 0). Se declara y las comparaciones lo exentan **solo** en alfa 0; no se cambia el escritor (3.8, decisión 9).
4. **Corpus:** 3.9 **no repite** la fila 3 de 3.8 (los 29 componen igual que su PNG), que ya cierra lo que 3.2 dejó pendiente (formas e indexados); añade lo que 3.5 dejó (aplanar y combinar visibles contra el PNG), escribir y reabrir los 29, y la estructura del fichero escrito frente a la que escribió Aseprite.
5. **`cargo deny`:** los 17 duplicados quedan aceptados sin `skip`, igual que en el hito 2; línea base sin cambios.
6. **Modelo:** pila ingenua con la composición delegada en `compose` sobre un documento de `from_layers` y vectores a mano; no se reescriben las fórmulas.
7. **`main_image()`** (px-core, obsoleta con `TODO(3.9)`; 58 ficheros con 475 usos, 2 de producción en `demo.rs`): **propongo no retirarla ahora** y anotarla como aplazada en el resumen, porque retirarla es un cambio no aditivo de `px-core` que toca casi todas las pruebas (más que firmas y cifras: sin visto bueno expreso no la toco). Alternativa: una unidad de limpieza aparte, con su spec. El `TODO(3.9)` quedaría desactualizado hasta entonces.
8. **Controles negativos:** los de la fila 4 más una mutación real del escritor al cierre (se restaura y se anota).
9. **Fuzzers:** `read_png` y `read_aseprite`, 10 minutos cada uno, los lanzas tú.
10. **Grandes (fila 10):** `#[ignore]`, release, un hilo, sin aserciones de tiempo, soltando la sesión antes de reabrir; las cifras (y el tiempo de comprimir) entran en esta spec.
11. **Casos de `proptest`:** 64 (propiedades 1 y 2), 32 (la 3) y 128 (la 4).
12. **Lista manual de 15 pasos** (arriba) y los ficheros de prueba: copias de 6 casos de `golden/aseprite/` y de un PNG real. Aseprite está en los tres sistemas: el paso 9 se hace en todos.
13. **Cierre del hito:** `hito-3-resumen.md` con 3.6 y 3.7 aplazadas fuera del hito y la lectura autorizada de las specs 3.1, 3.3 y 3.4 para completar «lo aplazado», sin inventar.

## Visto bueno (2026-10-22; condiciones que prevalecen)

Decisiones 1 a 8 aprobadas (la 9, fuzzers, y la 10 a 13 se siguen como están escritas). Ningún cambio de `px-core`, `px-raster` ni `px-formats` (salvo el comentario de `main_image`, condición 4); si hiciera falta alguno, me paro.

1. **La puerta del hito 2 NO se mueve a `px-golden` y los 11 nombres NO se exportan**: decisión con datos (3726 líneas, 30 pruebas, 9 campos públicos de `Session`, `px-golden` de 41 a unos 180 paquetes). Se actualizan la spec de la 3.0 y el mapa.
2. **Exención del RGB bajo alfa 0, exacta:** un píxel con alfa mayor que 0 que difiera **falla siempre**; solo se exime el RGB de los píxeles con alfa 0 en los dos lados. Se anota en `docs/DISENO-INTERFAZ.md` como límite conocido del `.aseprite`: «el RGB bajo alfa 0 fuera del rectángulo mínimo no se guarda; no afecta a nada visible».
3. **`main_image()` no se retira:** `TODO(3.9)` pasa a `TODO(limpieza)` y `mapa-funciones.md` reúne **una** lista de limpiezas pendientes para una unidad corta entre hitos (`window_title`, `interact` e `interact_ordered`, `info_flags*`, `show_toolbar*`, las variantes `Analysis` y `DialogKind` que 3.5 evitó, el nombre `CelMismatch` para lienzos imposibles y `main_image`: 475 usos en 58 ficheros), cada una con su spec de origen.
4. **`cargo deny`:** los 17 duplicados se aceptan sin `skip`; la línea base se anota en `docs/DEPENDENCIES.md` y se comprueba que `miniz_oxide` no añadió ninguno (ya estaba).
5. **«Lo aplazado a los hitos siguientes»** sale de leer `specs/hito-3-capas-*.md` y de un `grep` de «pulido», «más adelante» y «TODO» sobre `specs/` y `docs/`: cada elemento con su spec y su fila de origen; ninguno inventado.
6. **(a) Tiempo de guardar (fila 10):** se mide guardar un `.aseprite` de 8192² RGBA con 4 capas. Si pasa de **2 s**, el guardado muestra «Guardando…» antes de empezar (el mecanismo de dos fotogramas que ya existe), con su prueba de la secuencia. Si pasa de **un minuto**, se anota como riesgo del hito 8 (barra de progreso).
7. **(b) Lista manual** (≤ 15 pasos) con: en macOS, `Cmd+Q` con cambios sin guardar (anotar qué pasa); en Linux, soltar ficheros solo en X11 (en Wayland no está disponible: causa en `hito-2-abrir-guardar.md`, «Para más adelante»); y **Aseprite abriendo lo que escribe Pixoko en los tres sistemas** (hay Aseprite en Linux, Windows y macOS): en cada uno se anota la versión (`aseprite --version` o Ayuda → Acerca de) y se comprueban orden de capas, nombres, opacidades, mezclas, capa oculta, capa bloqueada y paleta.
8. Flujo: commit de spec; pruebas (en `px-golden` y `px-app` `puerta::capas`; nacen en verde y sus controles negativos son su prueba de que saben fallar); la mutación real del escritor al cierre; resumen del hito, `PLAN-RESUMEN` e INDEX (hito 3 «cerrado a falta de la prueba manual»; 3.6 y 3.7 aplazadas con su motivo). Cierre completo; sin `license-auditor` (no cambia código de producción).
