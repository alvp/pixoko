# Unidad L1: limpiezas pendientes y código en inglés

Estado: **aprobada** (2026-10-22, en tres etapas L1, L2 y L3 con un freno entre ellas; las condiciones de «Visto bueno» al final **prevalecen sobre cualquier otra frase de la página**).

Referencias: sección 11 de [`mapa-funciones.md`](mapa-funciones.md) (lista de limpiezas, con su spec de origen), [`hito-3-resumen.md`](hito-3-resumen.md) y `docs/PLAN-RESUMEN.md`. Clase: deseable. Entre el hito 3 y el 4.

Cambios: **`px-core` y `px-raster` (hace falta tu aprobación expresa, ver decisión 1)**, `px-formats`, `px-ui`, `px-app`, `px-golden`, `fuzz/` y `tools/`. Sin dependencias nuevas ni `unsafe`; `Cargo.toml`, `Cargo.lock`, `deny.toml`, `ci.yml` y `docs/DEPENDENCIES.md` no cambian. **No hay lógica nueva.** Sin `test-writer` ni `license-auditor` (modo ahorro). Comportamiento visible: **solo dos cambios, ambos en la parte A y declarados** (título del diálogo de exportar y variante de error de `px-core`); la parte B no cambia ninguno.

## Objetivo

- **A.** Cerrar las limpiezas anotadas en el mapa (sección 11) con la mínima superficie de cambio.
- **B.** Que identificadores, ficheros, nombres de prueba, comentarios y variables de depuración estén en inglés, sin alterar el programa. Los textos que ve el usuario siguen en español.

La A va antes que la B: así no se traducen nombres que la A borra, y la B parte de un estado cuyo comportamiento ya está medido.

## Lo comprobado (medido hoy, `main` limpio en `c715cd6`)

- **205 000 líneas de Rust** en 288 ficheros (`px-ui` 137 ficheros y 99 700 líneas; `px-app` 80 y 59 200; `px-core` 25 y 24 700; `px-formats` 18 y 9 900; `px-raster` 12 y 6 800; `px-golden` 12 y 4 900; `fuzz` 2). `tools/` no tiene Rust: son 4 scripts de Lua y uno de `bash` (1 023 líneas).
- **Identificadores con alguna palabra española: unos 6 600 distintos y unas 60 000 apariciones, en 217 ficheros; vocabulario de unas 1 150 palabras.** Medido con un léxico de Rust (cadenas y comentarios aparte) y los diccionarios es/en del sistema (cifra aproximada: la exacta la da el paso 1 de la B). Unos **4 000 son nombres de prueba** (3 993 de 4 182 `#[test]` llevan una palabra española; son frases enteras: `fila_12_la_variable_pide_de_2_a_256_capas_…`). El resto: unas 2 000 funciones, 185 `struct`, 46 `enum`, 324 `const`, 30 `mod`. Hay un tipo de banco de pruebas (`banco`, unas 5 600 apariciones) y otro de «mundo» (1 200).
- **Comentarios: 18 262** (1,26 M de caracteres): `px-ui` 8 869, `px-app` 4 937, `px-core` 2 108, `px-formats` 1 052, `px-raster` 735, `px-golden` 542, `fuzz` 18. Casi todos en español.
- **Ficheros con nombre español** (estimación): unos 24 `.rs` (`navegacion`, `panel_baja`, `puerta/` con 8, `escritor`, `mezcla`, `arranque`, `damero_guardado`, `modelo`, `grande`, `capas`, `*_tests` de `ayuda` y `teclado`…) y 2 carpetas (`puerta/`, `puerta/capas/`), más `tools/corpus-h3/{generar,comprobar,verificar}.*` y `sonda-combinar.lua`. En `golden/aseprite/`: **29 casos × 3 ficheros + README = 88 ficheros** con nombre español (`golden/png/` ya está en inglés: `grafx2-golden1.png`…). Ningún `.proptest-regressions` pertenece a un fichero que se renombre (los 8 siguen a ficheros ya en inglés).
- **Variables de entorno** (solo depuración): `PIXOKO_SIN_MINIMO` (6 menciones), `PIXOKO_DAMERO_MALLA` (8), `PIXOKO_DIALOGO_PRUEBA` (8); las tres en `px-app/src/lib.rs`; sus pasos manuales están en specs antiguas.
- **El código no usa** `stringify!`, `type_name`, `file!()` ni `module_path!`; los cuatro `{:?}` de producción que encontré (`document.rs`, `palette_edit.rs`, `compose.rs`, `rgba.rs`) imprimen `Mode` (variantes ya en inglés) o un `char`: renombrar tipos con `Debug` no cambia ningún texto de producción hoy. Las rutas por texto son: `#[path = "…"]` (27), `include_str!`/`include_bytes!` (22 de producción: `docs/THIRD-PARTY.md` y 21 iconos de `assets/icons/` con nombre español, `lapiz.png`…) y `env!("CARGO_MANIFEST_DIR")` con rutas de `golden/` (20).
- **Los literales de cadena llevan identificadores dentro**: `"{ancho}"`, `"{contexto}"`, `"{tool:?}"` (argumentos en línea de `format!`, `assert!`, `panic!`…). Si se renombra la variable hay que cambiar el literal, y el texto que sale es idéntico. Lo trata P2.
- `ci.rs` tiene **24 líneas de comentario y ningún identificador ni literal español**; los dos `fuzz_targets` 18 líneas de comentario.
- `rust-analyzer` **no está instalado** (solo el proxy de `rustup`). `lua`, `luac` y `luajit` sí.

## Parte A · limpiezas pendientes

Se hace primero y en commits propios. «Forma» = ninguna prueba cambia de intención.

| # | Qué se hace | Pruebas | ¿Solo forma? |
| --- | --- | --- | --- |
| A1 | **`window_title`** (alias de `title_text`, `px-app/src/title.rs`; origen `hito-2-limpieza-api.md`): se borra; `lib.rs:1655` llama a `title_text`. | 33 usos en 3 ficheros: solo el nombre de la llamada. | **Sí.** |
| A2 | **`Canvas::interact` e `interact_ordered`** (`px-ui/src/canvas.rs`, `TODO(hito 8)`; origen `interfaz-raton.md` punto 8 e `interfaz-dos-colores.md` decisión 2): se borran; queda `interact_roles` (que ya usa la app) y `interact_inner` pierde su bandera. `Canvas::show` (hoy `interact` + `paint`, sin usos de producción) pasa a `interact_roles`. | 6 usos de `interact` y 20 de `interact_ordered` en unos 20 ficheros de pruebas, más el banco de `px-app/src/damero_guardado.rs`: pasan a `interact_roles` y se quedan con el primer campo de cada par. | **No del todo.** `interact*` **ignoran el botón derecho** (`secondary = false`) y `interact_roles` lo dibuja con `Role::Background`. Las pruebas que afirman que un botón secundario «no hace nada» **invierten su intención**: candidatas `pencil.rs` (`los_botones_que_no_son_ni_izquierdo_ni_central_no_hacen_nada` y `un_segundo_boton_durante_un_trazo_se_ignora`) y las de `canvas_roles.rs`, `window_stack.rs` y `palette_panel.rs` que contrastan los dos caminos (el recuento exacto se mide al implementar). **Decisión 4.** |
| A3 | **`info_flags*`** (`px-ui/src/color.rs`; origen `hito-2-fijar-transparente.md` e `interfaz-dos-colores.md` decisión 7): se borran `info_flags` e `info_flags_range` (solo las usan pruebas: 19 y 20 apariciones). | `info_flags_roles` da `frente`/`fondo` donde estas daban `activa`: se cambian los literales esperados (forma, unas 35 aserciones en `color.rs` y `palette_ops.rs`). | **Sí para esas dos.** **`info_flags_alpha0` no es código muerto**: `palette_panel.rs:768` lo llama en la rama `else` de `roles.background.is_none()`. Quitarlo exige quitar esa rama y el `Option` del rol de fondo (cambia una firma pública y las pruebas del panel que pasan `None`). **Se deja** hasta esa unidad (decisión 3). |
| A4 | **`show_toolbar` y `show_toolbar_with`**: **ya no existen** (I.4); quedan 5 menciones en comentarios de `px-ui/tests/` (`keymap.rs`, `window_stack.rs`, `common/caja.rs`). | Ninguna: son comentarios. | **Sí.** No tiene commit propio: se reescriben al traducir los comentarios (B2). |
| A5 | **`Analysis` y `DialogKind`** (`px-app/src/convert_mode.rs`, `dialogs.rs`; origen `hito-3-combinar.md` decisión 3 de «Decisiones»). (a) `Analysis::Blocked(LayerBlock)`: `layer_block` deja de ser una comprobación aparte y devuelve el análisis (el motivo y la explicación siguen saliendo de `LayerBlock`; con bloqueo no se calcula informe, como hoy). (b) `DialogKind::Export`: el diálogo de exportar deja de ser un `SaveAs` con la bandera `exporting` de `App` y se llama por su nombre. | (a) un oráculo de `convert_mode/tests.rs` (`match analysis`, línea 990, que reimplementa la explicación) necesita un brazo nuevo y una fila propia; (b) `puerta/filas.rs:568` y `dialogs.rs` (8 `match`): un brazo `| Export`. | **No.** (b) **cambia lo que se ve**: el título del diálogo del sistema pasa de «Guardar PNG» a «Exportar PNG aplanado» (única razón de la variante). (a) no cambia nada visible pero añade brazos a `match` de pruebas anteriores (la razón por la que 3.5 la evitó). **Decisión 5.** Con pruebas en rojo antes del código (filas 4 y 5). |
| A6 | **`DocumentError::CelMismatch`** (`px-core/src/document.rs`; origen `hito-3-aseprite.md`, corrección 3): se añade `DocumentError::InvalidCanvas` para el lienzo imposible (ancho o alto 0, más de `Image::MAX_BYTES`) y la paleta que no corresponde al modo; `CelMismatch { layer }` queda solo para una celda que no encaja. Sale de `from_layers` (3 sitios) y del `Display`. | `px-core/tests/layers.rs:4015` y `:4026` esperan `CelMismatch { layer: 0 }` para un lienzo imposible: cambia el literal esperado (forma). Fila 6, nueva. | **No.** Cambia la variante y el texto de un error de `px-core` (hoy «la celda de la capa 0 (o el lienzo) no encaja…»). **Alcanzable desde la interfaz solo si el lector de `.aseprite` deja pasar un lienzo imposible**; no lo he demostrado, se mide al implementar. **Decisión 6.** |
| A7 | **`Document::main_image()`** (`px-core`, `TODO(limpieza)`; origen `hito-3-capas-modelo.md` decisión 14 y `hito-3-capas-vista.md` C-02): se borra (siempre devuelve `ImageId(0)`). | **475 usos en 58 ficheros** (`px-core` 163 en 7, `px-app` 158 en 27, `px-ui` 111 en 20, `px-raster` 40 en 3, `px-golden` 3 en 1; producción: `demo.rs`). Cada `x.main_image()` pasa a `ImageId::from_raw(0)` (equivalente por construcción; `ImageId(0)` es privado). Con un ayudante local en los ficheros con 5 usos o más. | **Sí**, y comprobable: cada sustitución es idéntica. **Decisión 7.** |
| A8 | **`action_for_key`, `file_action_for_key`, `tool_action_for_key`** (`px-ui`, `TODO(hito 8)`; origen `interfaz-teclado.md` decisión 8). **No estaba en tu lista, sí en el mapa.** | 140 + 46 + 35 usos, todos de pruebas (producción: solo las definiciones). Las vistas devuelven `Action`/`FileAction` y fijan `Platform::Linux`; `Keymap::action_for` devuelve `AppAction` y recibe la plataforma. | **No**: cambia el tipo devuelto en unos 220 sitios y lo que se prueba. **Se aplaza** (hito 8, como dice su `TODO`) salvo que digas otra cosa (decisión 2). |

## Parte B · renombrado a inglés

### 1. Método

**Un mapa versionado aplicado por un script con léxico de Rust.** No `rust-analyzer`: no está instalado; renombra un símbolo por vez (unos 6 600 símbolos en 205 000 líneas, con `egui` y `proptest` en el árbol: horas); no toca comentarios, `#[path]`, `fuzz/` (fuera del workspace) ni los scripts de Lua; y su resultado no se puede auditar con una regla simple. El script, en cambio, tiene una garantía propia: **sustituir uniformemente cada identificador `X` por `Y` en todo el código es un renombrado alfa (conserva el programa) si el mapa es inyectivo y `Y` no choca con ningún nombre ya visible en ese sitio.** Todo el trabajo está en esas dos condiciones; las comprueba el script y las confirma el compilador, y P4 lo demuestra.

- **Mapa** (`tools/rename/map.tsv`, versionado; trozos por crate en `tools/rename/map/<crate>.tsv`): columnas `clase` (`id`, `fichero`, `carpeta`, `env`, `corpus`), `viejo`, `nuevo`, y opcionalmente `fichero:` (sustitución solo en ese fichero, para resolver choques). Se genera en dos partes:
  1. **Glosario** (`tools/rename/glossary.tsv`, unas 1 150 palabras): cada palabra española → una inglesa, con el estilo del identificador intacto (`snake_case`, `CamelCase`, `SCREAMING`). Lo revisas tú entero (unas 1 150 líneas). Las palabras que ya son inglés o valen en los dos idiomas (`color`, `error`, `normal`, `total`, `base`…) llevan una fila identidad y no se tocan. Las **falsas amigas** (`sin` = «without», `pie` = «footer», `par`, `fin`) se resuelven por la palabra, nunca por el hueco: llevan fila propia.
  2. **Nombres de prueba y compuestos que no salen palabra por palabra** (`fila_12_la_variable_pide_de_2_a_256_capas…`): **traducción semántica, no mecánica** (unos 4 000 nombres). La hago yo por crate, en trozos, y el script solo la comprueba: `snake_case` ASCII, ninguna palabra española según los diccionarios, único dentro de su módulo, y sin perder el número de fila. **Es el único paso de producción del mapa que no es mecánico (decisión 9).**
- **Aplicación** (`tools/rename/rename.py`, versionado: no es un oráculo, es una herramienta). Recorre cada `.rs` con un léxico propio (cadenas normales, crudas y de bytes, comentarios anidados, lifetimes frente a literales de carácter, `r#ident`, `$campos` de `macro_rules!`) y cambia solo **identificadores en código**, más:
  - en **comentarios y comentarios de documentación**: solo lo que va entre comillas invertidas o en enlaces `[`…`]` y los bloques `rust` (el resto es prosa y se traduce en B2; aplicar el mapa a la prosa mezclaría idiomas);
  - **dentro de literales**, solo `{viejo` / `{viejo:` en los argumentos en línea de las macros de formato (`format!`, `write!`, `println!`, `assert*!`, `panic!`…), y cada literal tocado va a un informe;
  - nombres de ficheros y carpetas (con `git mv`), las líneas `mod x;` y los `#[path = "…"]` correspondientes.
- **Colisiones.** (i) Mapa no inyectivo: el script se niega. (ii) **Un nombre nuevo que ya existe como token en el mismo fichero** (por ejemplo `imagen` → `image` donde ya hay un `image`): es la trampa seria, porque un `let` que oculta a otro **compila y cambia el significado**. El script lo detecta en seco, lista cada caso y exige una fila `fichero:` (por ejemplo `source_image`) antes de aplicar. (iii) Un nombre nuevo igual a una palabra reservada, a un nombre del preludio de `std` o a un método de un trait habitual (`clone`, `len`, `iter`, `min`, `from`, `default`…) o de `egui`/`proptest` usado en el fichero: igual, lista y resuelve a mano. (iv) `TODOS` (español «todos») ≠ `TODO(`: el marcador `TODO(` no se toca y el nombre nuevo no puede ser `TODO`. Ningún renombrado se aplica con avisos sin resolver.
- **Palabras que ya son inglés**: identidad en el glosario y no aparecen en el mapa de aplicación. Los identificadores mixtos (`is_blocked`, `rect_lienzo`, `imagen_rgba`) se traducen solo en su parte española.
- **Después de aplicar**: `cargo fmt` (nombres más largos mueven saltos de línea; lo hace cada commit) y se compila. Un error de compilación se corrige **en el mapa**, no a mano en el código.

### 2. Reparto en commits (workspace en verde en cada uno; sin mezclar tipos)

**Dueño de un nombre = el primer crate, en orden `px-core` → `px-raster` → `px-formats` → `px-ui` → `px-app` → `px-golden` → `fuzz`+`tools`, donde aparece.** Un nombre que aparece en varios crates (una API pública y sus llamadores) se renombra en **todo el workspace** en el commit de su dueño; uno que aparece en un solo crate, solo en ese. Así cada commit compila sin alias provisionales, y cada crate solo ve en su commit lo que es suyo. El `px-core` es el que más toca fuera de sí (es el vocabulario de todos).

Por crate, tres commits (en `fuzz`+`tools`, dos; no hay ficheros que mover en `fuzz`):
- **a. Ficheros y módulos**: `git mv` y las líneas `mod`/`#[path]` (similitud 100 % en el historial).
- **b. Identificadores**: el mapa del crate, con `cargo fmt`.
- **c. Comentarios** (B2): traducción de la prosa; el código no cambia.

Orden global: spec → **A** (por cada limpieza: adaptación de pruebas, pruebas en rojo si las hay, lógica) → **B** `px-core` → `px-raster` → `px-formats` → `px-ui` → `px-app` → `px-golden` → `fuzz`+`tools` → **corpus** (decisión 4) → documentos (`CLAUDE.md`, `INDEX`). Unos **36 commits**: A ≈ 13, B ≈ 20, corpus y documentos 3.

**Si un paso necesita algo que no sea mecánico, me paro.** Son paradas ya previstas: la lista de colisiones sin resolver, un `Debug` de un tipo renombrado que aparezca en un texto de producción, una prueba que falle (solo puede ser un nombre mal mapeado) y la sustitución de A2 si invierte más pruebas que las listadas.

### 3. Verificaciones (la línea base se toma al empezar B, tras la A, y se guarda fuera del repositorio en `~/.cache/pixoko-l1/`; las compilaciones, con `CARGO_TARGET_DIR=~/.cache/pixoko-scratch`)

- **P1 · Nombres de prueba.** `cargo test --workspace --locked -- --list` (con la cabecera `Running …` de cada ejecutable), antes y después: **mismas líneas, ordenadas, salvo el mapa** (se aplica `viejo → nuevo` a la lista de antes, también a las rutas de módulo y a los ejecutables de `tests/`). Mismo número (hoy 4 144 + 26 ignoradas).
- **P2 · Literales de cadena de producción.** Script fuera del repositorio (`~/.cache/pixoko-l1/literals.py`): extrae con el mismo léxico todos los literales de los `.rs`, separados en **producción** (`src/` sin los `#[cfg(test)]`, ni `tests.rs`, `*_tests.rs`, `puerta`, `damero_guardado`) y **pruebas**. Antes y después, **producción: conjunto idéntico**, tras normalizar los argumentos en línea (`{ancho:?}` → `{:?}`) y salvo **exactamente**: los tres nombres de variable de entorno y nada más. **Pruebas**: el informe de diferencias lo reviso yo línea a línea: solo caben los argumentos en línea, los nombres de ficheros del corpus (decisión 4) y los `#[path]`. Un literal de producción distinto es una parada.
- **P3 · Comportamiento del binario y del corpus.** (a) `px-app --licencias` y los errores de uso (argumento desconocido, dos ficheros, fichero inexistente, vacío): **mismos bytes en `stdout` y `stderr` y mismo código** antes y después (compilación de depuración; se ejecutan sin ventana). (b) Un control más fuerte: compilación `--release` de `px-app` antes y después y **diferencia de las cadenas del ejecutable** (`strings` de la sección de solo lectura): solo pueden diferir las rutas de fichero de los mensajes de `panic` y los nombres de tipo, variante y campo de los `derive(Debug)` renombrados, y el script lo comprueba aplicando el mapa a lo que solo estaba «antes»: tiene que dar lo que solo está «después». (c) `golden/`: la lista de SHA-256 de los 97 ficheros, ordenada por hash, **idéntica**; las rutas, salvo el mapa de la decisión 4.
- **P4 · Equivalencia alfa (nueva).** Por fichero, la secuencia de tokens (sin comentarios) tras el commit de identificadores es la de antes **con el mapa aplicado** (y los literales normalizados como en P2); tras el commit de comentarios, **la misma que antes, sin una diferencia**. Esto demuestra «sin cambios de comportamiento» mejor que ninguna prueba, y es lo único que garantiza que la traducción de comentarios no ha tocado código.
- **Comprobaciones por commit de B**: `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test -q --workspace --locked`, `git diff --stat -- Cargo.toml Cargo.lock deny.toml .github` vacío, y `cargo deny check` una vez, al cerrar.
- **Comentarios (B2), comprobaciones propias**: ningún carácter propio del español en un comentario (`á é í ó ú ñ ¿ ¡`; se permiten `× → ≤ ≥ ·` y el resto de símbolos ya usados), cada mención a `Aseprite`, `Grafx2`, `LibreSprite`, `DPaint`, `ase-file-specs`, `specs/…` y `docs/…` aparece **igual de veces por fichero**, y toda referencia entre comillas invertidas a un nombre ya renombrado coincide con el mapa.

### 4. Ficheros del corpus (`golden/aseprite/`, 88; `golden/png/`, 9)

**Se renombran solo los ficheros de `golden/aseprite/` (29 casos × `.aseprite`, `.png`, `.png.sha256`) con `git mv`: contenido idéntico byte a byte.** Razón: el nombre del caso es la clave que une 10 ficheros de pruebas, el generador y la lista de `generar.sh`; dejarlos en español deja 29 cadenas españolas en código inglés. Es seguro porque **el `.sha256` solo contiene el hash de los píxeles** (no el nombre del fichero) y porque el contenido de los `.aseprite` (nombres de capa en español, paleta, celdas) no se toca. `golden/png/` ya está en inglés (`grafx2-golden1`, `libresprite-golden-indexed`…): no se mueve.
- Se actualizan la lista de 29 nombres de las pruebas (`aseprite_corpus.rs` y las que citan casos por nombre), las cadenas `CASOS["…"]`/`guardar(s, "…")` de `generar.lua`, la lista de `generar.sh` y su `NUEVOS`. En el Lua **solo cambian los nombres de caso, los identificadores y los comentarios**; los literales que acaban dentro del `.aseprite` (nombres de capa) no se tocan, y el script lo comprueba (ningún literal de capa figura en el mapa).
- Va en **un commit propio, después de los de código**, porque es el único que cambia literales de pruebas (nombres de fichero) y el Lua.
- **Verificación**: P3(c) con el mapa; `luac -p` de los 4 scripts; y **te pido una ejecución de `tools/corpus-h3/generar.sh`** (lo único de la unidad que necesita Aseprite): tiene que dar los 29 casos `IGUAL` y salir con 0 sin tocar `golden/`. Si prefieres no ejecutarlo, no renombro los ficheros ni el Lua (decisión 8).
- Las specs antiguas citan los nombres viejos: se dejan (apartado 8).

### 5. `atajos.txt` y su cabecera `pixoko-atajos 1`

**Se mantienen, y `atajos.txt` también.** Son un formato de datos del usuario, no código: las claves ya son inglés (`tool.pencil`, `color.next`) y solo la cabecera y el nombre del fichero son españoles. Cambiar la cabecera a `pixoko-shortcuts 1` haría que **todo fichero existente falle la comprobación de versión y se ignore entero** (`interfaz-ayuda.md`: «versión distinta se ignora entero, sin tocarlo»): el usuario perdería sus atajos sin aviso. Evitarlo exige que el lector acepte las dos cabeceras y que el escritor migre: **comportamiento nuevo y pruebas nuevas**, justo lo que esta unidad no hace. El momento natural es el hito 8 (preferencias y traducciones), con subida de versión y migración. Consecuencia: los literales `"pixoko-atajos 1"` y `"atajos.txt"` quedan como están en producción (P2 los cuenta como no cambiados).

### 6. Lo que tocan `ci.rs`, `ci.yml`, `deny.toml` y `fuzz/` (lista exacta)

| Fichero | Qué cambia | Por qué |
| --- | --- | --- |
| `.github/workflows/ci.yml` | **nada** | Solo ejecuta `cargo fmt/clippy/test/build -p px-app/deny`; ninguna ruta de `src/` ni nombre de prueba. Su paso «Instalar Rust» está en español y está fuera del alcance (no es `crates/`, `fuzz/` ni `tools/`). |
| `crates/px-golden/tests/ci.rs` | **solo sus 24 líneas de comentario** (commit c de `px-golden`). Cero identificadores y cero literales. **No lo toco sin que lo apruebes (decisión 10).** | Fija `EXPECTED_COMMANDS` y el texto de `ci.yml`; esos literales no cambian. |
| `deny.toml` (raíz) | **nada** | Fuera del alcance (raíz); sus comentarios citan specs. |
| `fuzz/deny.toml` y `fuzz/Cargo.toml` | solo comentarios (en el commit de `fuzz`) | Están en `fuzz/`; sin claves nuevas ni licencias. Los `[[bin]]` (`read_png`, `read_aseprite`) ya están en inglés. |
| `fuzz/fuzz_targets/*.rs` | solo 18 líneas de comentario | Sin identificadores españoles. |
| `Cargo.toml`, `Cargo.lock` (los 2) | **nada** | `git diff --stat` vacío es una condición de cada commit. |
| `crates/px-render/Cargo.toml` | 1 línea de comentario | Trivial. |
| `px-golden/tests/structure.rs` | nada | Solo exige que existan los 8 directorios de `crates/`. |
| Corpus del fuzzer | nada | `fuzz/corpus` y `fuzz/artifacts` están en `.gitignore`. |

Sin efecto en CI: ningún nombre de trabajo, ruta ni comando cambia. Riesgos en Windows y macOS: rutas más largas (se comprueba que la más larga del repositorio no pasa de 200 caracteres) y ninguna renombración solo de mayúsculas.

### 7. `CLAUDE.md`, `guard.sh` y tus comandos de comprobación

- **`.claude/hooks/guard.sh`: no cambia.** Bloquea por nombre de programa y repositorio (`GrafX2/grafX2`, `LibreSprite/LibreSprite`, `aseprite/aseprite`, `dpaint`), no por nombre de fichero; permite `ase-file-specs`.
- **`CLAUDE.md`: no menciona ningún nombre que cambie.** Propongo **una línea nueva en «Reglas permanentes»**: «Código en inglés: identificadores, ficheros, comentarios, nombres de prueba (`row_NN_…`) y variables de entorno; los textos que ve el usuario, en español (`*_es.rs`)». Y la misma línea en `.claude/agents/test-writer.md` (hoy no fija ningún idioma de nombres). Va en el último commit (decisión 12).
- **Tu `grep` de nombres, con lista de ficheros permitidos.** Hoy mencionan algún programa de referencia 42 ficheros de `crates/`, `fuzz/`, `tools/` más `CLAUDE.md`. **La lista permitida es la misma bajo el mapa de ficheros: la traducción conserva cada mención** (lo comprueba la parte B2, «igual de veces por fichero»). Cambian de nombre solo los que están bajo `puerta/` (`puerta.rs`, `puerta/{comun,filas,capas,propiedades,grande}.rs`, `puerta/capas/{filas,grande,guiones,modelo,propiedades}.rs`), `px-formats/tests/aseprite/escritor.rs`, `px-golden/tests/mezcla.rs`, `px-golden/tests/png_interop/aseprite_puerta.rs` y los de `tools/corpus-h3/` (`comprobar`, `generar`, `verificar`, `sonda-combinar`). Los nombres nuevos salen del glosario (propuesta: `puerta`→`gate`, `comun`→`common`, `filas`→`rows`, `capas`→`layers`, `propiedades`→`properties`, `grande`→`large`, `guiones`→`scripts`, `modelo`→`model`, `escritor`→`writer`, `mezcla`→`blend`, `comprobar`→`check`, `generar`→`generate`, `verificar`→`verify`, `sonda-combinar`→`merge-probe`). **La lista exacta nueva la genera el script en el commit de cada crate (`tools/rename/allowed-files.txt`) y te la doy en el informe final.** Y `golden/aseprite/*` también cambia de nombre (decisión 4): si tu `grep` las excluye por directorio, no cambia nada.

### 8. Specs y documentos antiguos

Se dejan como histórico: citan nombres de pruebas, de ficheros y de variables de entorno **que ya no existen**. Se añade a `specs/INDEX.md`, en la cabecera: «Desde L1 (`limpieza-1.md`) el código, los ficheros y los nombres de prueba están en inglés; las specs anteriores citan los nombres viejos: se buscan con `tools/rename/map.tsv` (`viejo → nuevo`)». Las listas de pasos manuales de specs antiguas con `PIXOKO_SIN_MINIMO`, `PIXOKO_DAMERO_MALLA` y `PIXOKO_DIALOGO_PRUEBA` dejan de valer: los nombres nuevos (propuesta: `PIXOKO_NO_MIN_SIZE`, `PIXOKO_CHECKER_MESH`, `PIXOKO_TEST_DIALOG`) están en el mapa. **No hay alias de los nombres viejos** (sería comportamiento nuevo); P3 comprueba el binario sin ninguna variable puesta.

## Casos (12 filas)

Sin `test-writer`. Las filas 4, 5 y 6 son pruebas nuevas (en rojo antes del código, en `px-app/src/convert_mode/tests.rs`, `px-app/src/dialogs/tests.rs` y `px-core/tests/layers.rs`); el resto son comprobaciones de cierre de cada commit.

| # | Parte | Caso |
| --- | --- | --- |
| 1 | A1 | `window_title` no existe (`rg` da 0), `lib.rs` usa `title_text` y `cargo test -p px-app` pasa con las mismas pruebas. |
| 2 | A2 | `interact` e `interact_ordered` no existen; `show` e `interact_roles` dan el mismo resultado que antes con botón izquierdo, central y teclas; las pruebas del botón secundario cambian solo si la decisión 4 lo aprueba, listadas una por una en el commit. |
| 3 | A3 | `info_flags` e `info_flags_range` no existen; `info_flags_roles` da `frente`, `fondo`, `rango`, `transparente` y `alfa 0` en el orden de siempre; `info_flags_alpha0` y el panel sin rol de fondo siguen igual. |
| 4 | A5 | `Analysis::Blocked(LayerBlock::MultiLayerGray)` con destino Gris y varias capas, y `Blocked(LayerProps)` con destino indexado y capas de opacidad o mezcla; **mismo motivo (≤ 37 caracteres) y misma explicación que hoy**, sin calcular informe. |
| 5 | A5 | `DialogKind::Export` pide el título «Exportar PNG aplanado» (con cualquier número de capas y de ruta), el nombre `<nombre>-aplanado.png` y solo el filtro PNG; `Save` y `SaveAs` siguen con sus tres títulos de hoy («Guardar .aseprite», «Guardar PNG» y el de varias capas); `Open` igual; el flujo de `lib.rs` ya no necesita la bandera `exporting`. |
| 6 | A6 | `from_layers` con lienzo de ancho 0, de alto 0, de más de `Image::MAX_BYTES` o con una paleta que no corresponde al modo da `InvalidCanvas`; una celda de la capa *i* que no encaja da `CelMismatch { layer: i }`; los dos mensajes son distintos. |
| 7 | A7 | `main_image` no existe (`rg` da 0); las 475 sustituciones compilan; `cargo test` de cada crate pasa con las mismas pruebas. |
| 8 | B | **Mapa válido**: inyectivo; sin nuevo nombre ya presente en el mismo fichero (o resuelto con fila `fichero:`); sin palabra española en un identificador nuevo; sin identificador viejo en código; ningún identificador de `Aseprite`/`Grafx2`/`LibreSprite`/`DPaint` tocado. |
| 9 | B | **Cada commit de B en verde**: `fmt`, `clippy -D warnings` y `cargo test -q --workspace --locked` pasan; `Cargo.toml`/`Cargo.lock`/`deny.toml` sin diferencia; P1 a P4 pasan con la línea base del comienzo de B. |
| 10 | B2 | **Comentarios**: sin caracteres propios del español, menciones de programas de referencia y de `specs/`/`docs/` iguales por fichero, y tokens de código idénticos (P4). |
| 11 | B | **Corpus y herramientas**: `golden/` con los 97 hashes de antes (P3c); `luac -p` de los 4 `.lua`; `generar.sh` con los 29 casos `IGUAL` (lo ejecutas tú). |
| 12 | B | **CI y entorno**: `ci.yml`, `deny.toml`, `Cargo.toml` y `Cargo.lock` sin diferencia; `px-app --licencias` y los errores de uso, mismos bytes y códigos; compilación `--release` con la diferencia de cadenas explicada (P3b). |

## Propiedades (4; no son de `proptest`: se comparan dos estados del repositorio)

1. **P1 · Mismos nombres de prueba.** Lista antes y después, idéntica salvo el mapa (mismas 4 144 + 26).
2. **P2 · Mismos literales de producción.** Conjunto idéntico salvo los tres nombres de variable de entorno y los argumentos en línea normalizados; en pruebas, solo diferencias revisadas.
3. **P3 · Mismo comportamiento.** `px-app --licencias` y los errores de uso dan los mismos bytes y códigos; las cadenas del ejecutable `--release` solo difieren por el mapa y las rutas; los 97 ficheros de `golden/` tienen los mismos SHA-256.
4. **P4 · Equivalencia alfa.** Tokens de cada fichero tras renombrar = tokens de antes con el mapa aplicado; tras traducir comentarios = los mismos, sin diferencia.

## Estimación

| Concepto | Cifra |
| --- | --- |
| Ficheros de Rust tocados | **288** (todos, por los comentarios); 217 por identificadores; unos 24 renombrados y 2 carpetas |
| Otros ficheros | `tools/` 5 (Lua y `bash`), `fuzz/` 2 `.toml`, `px-render/Cargo.toml` 1, corpus 88 (renombrado), `INDEX`, `CLAUDE.md`, `test-writer.md` |
| Identificadores | unos 6 600 distintos y unas 60 000 apariciones; unos 4 000 son nombres de prueba |
| Comentarios | 18 262 (1,26 M de caracteres) |
| Commits | unos 36 (A ≈ 13; B ≈ 20; corpus y documentos 3) |
| Sesiones (una = una ventana de contexto) | **≈ 14**: A 2 (A2 y A7 son las pesadas), herramienta y glosario 1, mapa de nombres de prueba 2, identificadores `px-core`+`px-raster` 1, `px-formats`+`px-golden`+`fuzz`/`tools` 1, `px-ui` 1,5, `px-app` 1,5, comentarios 4 a 5 (`px-ui` ocupa la mitad), cierre y corpus 1 |

La traducción de comentarios es la parte cara y se paraleliza bien por fichero; en la sesión principal va en serie. **Se pide permiso** para usar subagentes solo en eso (decisión 11).

## Prueba manual (la haces tú, sin capturas)

1. `cargo run -p px-app` y abrir un PNG de `golden/png/`: la ventana, el menú y los diálogos se ven y dicen lo mismo que antes (los textos siguen en español).
2. `Archivo → Exportar PNG aplanado…` con 2 capas: el diálogo del sistema se titula «Exportar PNG aplanado» (único cambio visible; si se rechazó la decisión 5, «Guardar PNG»).
3. Pintar con el botón izquierdo y con el derecho (colores de frente y de fondo), con rueda y `Espacio`: igual que antes (A2).
4. Paleta: elegir, rotar y `Fijar transparente`: la línea de abajo dice `cursor · frente · fondo · rango · transparente · alfa 0` como siempre.
5. `Ctrl+S` sobre un documento de 2 capas propone `.aseprite`; abrir ese fichero devuelve las capas.
6. `Ayuda → Atajos de teclado…`: reasignar un atajo; cerrar y volver a abrir: se conserva (`atajos.txt` intacto en formato y nombre).
7. `PIXOKO_NO_MIN_SIZE=1 cargo run -p px-app` encoge la ventana por debajo del mínimo; con el nombre viejo ya no (comportamiento esperado).
8. `PIXOKO_TEST_DIALOG=1 cargo run -p px-app` abre el diálogo de prueba y `PIXOKO_CHECKER_MESH=1` dibuja el damero por malla.
9. `tools/corpus-h3/generar.sh`: 29 casos `IGUAL` y salida 0; `git status golden/` limpio.
10. `cargo run -p px-app -- --licencias` imprime lo de siempre; `cargo run -p px-app -- --no-existe` da el mismo error y el código 2.

## Fuera de alcance

Traducir la interfaz (hito 8); el formato de `atajos.txt`; los nombres de `assets/icons/` (decisión 13); `docs/` y `specs/` (histórico); el `deny.toml` y el `ci.yml` de la raíz; quitar el `Option` del rol de fondo y `info_flags_alpha0`; las vistas `action_for_key*`; cualquier cambio de lógica o de dependencias.

## Decisiones que necesito que confirmes

1. **Aprobación de `px-core` y `px-raster`** (regla de `CLAUDE.md`). Valen **solo** estos cambios: A6 (`InvalidCanvas`, aditiva y con el literal de dos pruebas), A7 (borrar `main_image`) y los renombrados del mapa de cada crate, aplicados con las comprobaciones P1 a P4. `px-raster` solo recibe renombrados.
2. **A8** (`action_for_key*`): propongo **aplazar** al hito 8, como dice su `TODO`. ¿O la haces ahora, con los 220 cambios de tipo en pruebas?
3. **A3**: propongo borrar solo `info_flags` e `info_flags_range` y **dejar `info_flags_alpha0`** (producción lo usa).
4. **A2**: ¿apruebas que 2 a 5 pruebas del botón secundario **cambien de intención** (hoy «ignora el botón derecho»; después «lo dibuja con el rol de fondo»), con su lista exacta en el commit de adaptación? Si no, A2 se aplaza con A8.
5. **A5(b) `DialogKind::Export`**: cambia el título visible a «Exportar PNG aplanado». ¿Sí? (A5(a) `Analysis::Blocked` es sin cambio visible, pero añade brazos de `match` en pruebas anteriores: ¿también sí?)
6. **A6**: `InvalidCanvas` con un mensaje nuevo en español, y `CelMismatch` solo para celdas. ¿Sí?
7. **A7**: `x.main_image()` → `ImageId::from_raw(0)` inline (con ayudante local donde haya 5 usos o más). ¿Sí?
8. **Corpus** (apartado 4): renombrar los 88 ficheros de `golden/aseprite/` y los nombres de caso del Lua, a cambio de **ejecutar tú `generar.sh` una vez**. ¿Sí? (`golden/png/` no se toca.)
9. **Método de B** (apartados 1 y 2): script con léxico propio y no `rust-analyzer`; mapa = glosario de unas 1 150 palabras (lo revisas entero) más unos 4 000 nombres de prueba **traducidos por mí** (paso no mecánico) con muestra de 100 por crate para tu revisión antes de cada commit b; commits `a` (ficheros), `b` (identificadores), `c` (comentarios) por crate, con el dueño de un nombre compartido = primer crate del orden. ¿Conforme?
10. **`ci.rs`**: solo sus 24 comentarios, en el commit c de `px-golden`. ¿Lo apruebas expresamente? (Si no, queda en español y la regla de comentarios lo exime.)
11. **Comentarios**: traducción por mí en la sesión principal (≈ 5 sesiones) o con subagentes por crate en paralelo (más rápido y más caro; solo para esto, con las comprobaciones B2 sobre su resultado).
12. **Documentos**: la línea de idioma en `CLAUDE.md` y en `.claude/agents/test-writer.md`, y la nota de `INDEX` del apartado 8. ¿Sí?
13. **`assets/icons/*.png`** (`lapiz.png`, `cuentagotas.png`…): **se quedan** (fuera de `crates/`, `fuzz/` y `tools/`, y sus rutas son literales de producción que P2 contaría). ¿O los renombro y anoto la excepción en P2?
14. **Variables de entorno**: `PIXOKO_NO_MIN_SIZE`, `PIXOKO_CHECKER_MESH` y `PIXOKO_TEST_DIALOG`, sin alias de las viejas. ¿Nombres y falta de alias bien?
15. **`*_es.rs`** (`help_texts_es.rs`, `help_texts_layers_es.rs`, `help_texts_merge_es.rs`): sus **literales no se tocan**; sus identificadores y comentarios, sí, como los demás. ¿Lo entiendes así?

## Hallazgos anotados (no son de la unidad)

- `px-app/src/title.rs:978` hace `include_str!("main.rs")` para vigilar que no se construya un literal; desde 3.0 `main.rs` tiene 5 líneas y el cableado vive en `lib.rs`, así que **esa prueba ya no vigila nada** (siempre pasa). Arreglarla es cambiarle el objeto (`lib.rs`); no lo hago aquí.
- Varios comentarios citan `main.rs` para el cableado (`puerta/filas.rs`, `title.rs`, `keys/tests.rs`, `dialogs/tests.rs`, `background_color/tests.rs`) y hoy es `lib.rs`; se corrigen al traducir los comentarios (es prosa: P4 no se ve afectada).

## Visto bueno (2026-10-22)

**Tres etapas, con freno entre ellas.** **L1**: parte A completa; identificadores del código de producción (módulos, tipos, funciones, variables, constantes, `static`, campos); variables de entorno (`PIXOKO_NO_MIN_SIZE`, `PIXOKO_CHECKER_MESH`, `PIXOKO_TEST_DIALOG`, sin alias); nombres de ficheros (24 ficheros, 2 carpetas y los 88 de `golden/aseprite/`); regla de idioma en `CLAUDE.md` y `test-writer.md`; arreglo de la prueba de `title.rs:978`. **L2**: nombres de prueba. **L3**: comentarios. Al terminar L1 se para y se espera la decisión sobre L2.

Decisiones: (2) `action_for_key*` se aplaza (anotado en la sección 11 del mapa). (3) `info_flags_alpha0` se queda; se borran solo `info_flags` e `info_flags_range`. (4) `interact` e `interact_ordered` pasan a `interact_roles`: entre 2 y 5 pruebas pueden cambiar de intención, cada una con nombre explícito y su antes y después en el mensaje del commit; **si pasan de 5, parar**. (5) `DialogKind::Export` con el título «Exportar PNG aplanado»: P2 permite exactamente ese literal. (6) `InvalidCanvas` en lugar de `CelMismatch` para el lienzo imposible: **aprobado expresamente el cambio de `px-core`** (una variante nueva, `from_layers` la devuelve, y los brazos de `match` de las pruebas); nada más en `px-core`, `px-raster` ni `px-formats` salvo los renombrados del mapa. (8) Los 88 ficheros de `golden/aseprite/` se renombran **a condición de que el usuario ejecute `generar.sh` y salgan los 29 casos `IGUAL`** con los nombres nuevos; se actualizan `generar.lua`, `generar.sh`, la lista fija de `px-golden` y los `.sha256` si hiciera falta. (10) `ci.rs`: aprobados **expresamente solo sus 24 comentarios** (en L3); `ALLOWED_ACTIONS` y `EXPECTED_COMMANDS` no se tocan y P4 lo demuestra. `atajos.txt` y su cabecera se mantienen.

Método de L1: mapa y script con léxico de Rust, con la lista previa de nombres nuevos que ocultarían a otro del mismo fichero; en L1 el vocabulario se traduce **palabra a palabra con un diccionario versionado** y los identificadores públicos (tipos, funciones, constantes, campos) se revisan a mano. Cada commit compila y pasa `cargo test` del crate tocado antes de seguir; el workspace queda verde por commit. P1 a P4 y P3b como en la spec. Informe final de 20 líneas como máximo, con la lista nueva de ficheros permitidos del `grep` de nombres y los comandos de P1 a P4. En L1 el mapa va a `tools/rename/` y se anota en `INDEX`.

## Estado de la etapa L1 (2026-10-22)

**Hecho:** la parte A (A1, A2, A3 a medias como se aprobó, A5, A6, A7; A4 va con L3 y A8 queda aplazada), `DialogKind::Export`, la guarda de `title.rs`, la regla de idioma, y de la parte B: ficheros y módulos (30 ficheros y 2 carpetas en `px-app`, `px-formats`, `px-ui` y `px-golden`; los 88 de `golden/aseprite/` y los 5 de `tools/corpus-h3/`), los identificadores de producción, las tres variables de entorno y los nombres de caso. Mapa, glosario, herramienta y verificador en `tools/rename/` (`README.md`). **Falta tu parte:** ejecutar `tools/corpus-h3/generate.sh` (tienen que salir los 29 casos `IGUAL`).

**Lo medido que cambia las estimaciones.** Los identificadores de **producción** ya estaban en inglés: 3 806 identificadores y 1 346 palabras, de las que solo `capas`, `atajo`, `por_defecto` y `si` eran españolas (la spec estimaba 217 ficheros y 6 600 identificadores para todo el código; en realidad **casi todo el español está en el código de pruebas**: ≈ 6 570 identificadores distintos en 215 ficheros, de ellos 3 998 nombres de prueba y ≈ 2 500 ayudantes, variables, tipos y constantes de pruebas). L1 queda por eso pequeña, y **L2 pasa a ser el grueso**: se propone que cubra **todos** los identificadores españoles que quedan en pruebas (no solo los `#[test]`), con el mismo método (mapa con ámbito, colisiones revisadas y P1 a P4).

**Desvíos de la spec (todos menores):**
- **A3**: las pruebas pasaron a `info_flags_alpha0` (mismo texto, solo forma) y no a `info_flags_roles` (que da `frente`/`fondo`).
- **A6**: **ninguna** prueba anterior esperaba `CelMismatch` para un lienzo imposible (las de `layers.rs` son de celdas); no hubo que adaptar nada.
- **A7**: sin ayudante local; la sustitución en línea (`ImageId::from_raw(0)`, 422 llamadas en 57 ficheros) basta. Se borró una prueba que comprobaba `main_image` (`la_imagen_principal_tiene_el_valor_crudo_0`) y se limpiaron cuatro variables y parámetros que solo existían para llamarla.
- **A2**: 2 pruebas cambian de intención (`los_botones_que_no_son_ni_izquierdo_ni_central_no_hacen_nada`, ahora con `Extra1`; y se borra `fila_2_interact_e_interact_ordered_siguen_sin_emitir_nada_con_el_derecho`), dentro del tope de 5; el banco de `tool_canvas.rs` deja de comparar con `interact`.
- **A5(a)**: `Analysis::Blocked` lo produce `analyze_document`; `sync_convert_all` no cambia (sigue comprobando `layer_block` antes de esperar).
- **Herramienta**: `tools/rename/` se versiona (la spec pedía un script fuera del repositorio solo para P2; el verificador entero está dentro para que puedas repetir P1 a P4 y P3b). Su léxico de `verify.py` (expresiones regulares) es independiente del de `rename.py`.
- **P4** admite que `rustfmt` reordene `mod` y `use` (mismos tokens, otro orden): 8 ficheros. Dos ficheros llevan una edición a mano declarada (`case_word`, abajo).
- **P3b** deja 3 cadenas del ejecutable que no son literales (bordes de bloque de la sección de solo lectura: un literal pegado a bytes sueltos gana o pierde 1 carácter).
- **Edición no mecánica**: `px-formats/tests/aseprite.rs` y `px-golden/tests/blend.rs` construían el nombre del caso con `format!("mezcla-{nombre}")`, donde `nombre` es también el nombre (español) de la capa dentro del fichero; la función `case_word` da el sufijo inglés (`blend-multiply`…) y deja la capa como está.
- **README de `golden/aseprite/`**: su tabla de nombres se actualizó con los nombres nuevos (el hash de ese fichero cambia; los 96 restantes, no).
- Un ejemplo de colisión conservadora revisada a mano: `tools/rename/reviewed-collisions.tsv` (3 filas, ninguna real).
