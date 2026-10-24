# Hito 4 · Unidad 4.1: operaciones de imagen (voltear, girar 90° y tamaño del lienzo)

Estado: **pendiente de tu visto bueno** (2026-10-24). Cambios previstos: `px-core` y `px-raster` (**solo los listados abajo; necesito tu aprobación expresa**, D02), `px-ui` y `px-app`. Sin dependencias nuevas ni `unsafe`; `docs/DEPENDENCIES.md` no cambia. Como toca `px-core` y `px-raster`: pruebas de esos dos crates por `test-writer` antes del código, las de interfaz las escribe la sesión principal, y un `license-auditor` al final sobre el rango de commits. Código, comentarios y nombres de pruebas nuevos en inglés; textos visibles en español en `*_es.rs`.

Referencias: fila 4.1 y decisiones D02, D04 a D10 de [`mapa-funciones.md`](mapa-funciones.md); [`hito-3-combinar.md`](hito-3-combinar.md) (patrón `based_on`, composición en `px-raster`, pico de memoria, «Combinando…»); [`hito-3-capas-modelo.md`](hito-3-capas-modelo.md) (celdas del tamaño del lienzo, vacías sin memoria, límites, paso de historia nunca descartado); `ConvertAll` (precedente de un comando que cambia todas las celdas en un paso).

## Objetivo

Seis acciones en `Imagen` que transforman **el documento entero** (todas las capas) en **un solo paso de deshacer**: `Voltear horizontal`, `Voltear vertical`, `Girar 90° a la derecha`, `Girar 90° a la izquierda`, `Girar 180°` y `Tamaño del lienzo…` (ancho, alto y ancla de 3 × 3; recorta o amplía, sin remuestrear). Es la primera unidad del hito 4 y no añade herramientas a la caja, así que no toca el problema de capacidad que dejó C3 (se revisa en 4.3).

## Lo comprobado (en el código)

1. **El lienzo es independiente de las celdas** (`Canvas { width, height, template }`; una celda vacía no tiene imagen). Cambiar el tamaño exige cambiar el lienzo y todas las celdas con imagen a la vez: lo mismo que ya hace `ConvertAll`, que intercambia imágenes y plantilla en un paso (`Converted`).
2. **Las funciones puras de píxeles viven en `px-raster`** y producen `Command`s (`fill_command`, `merge_*_command`); `px-core` solo valida y mueve. Seguimos ese patrón.
3. **Límites vigentes:** `Image::MAX_BYTES` = 1 GiB por imagen, `cells_limit` = 2 GiB de celdas, historia de 64 MiB pero **nunca se descarta el último paso**.
4. **`Imagen` hoy** tiene cuatro entradas (`Convertir modo…` y las tres de paleta). No hay selección (llega en 4.8), así que ninguna de las seis acciones puede ser «de la selección».
5. **No comprobado:** Aseprite tiene las seis funciones (voltear y girar el sprite, tamaño del lienzo con ancla), pero **no las uso como oráculo** (ver D9): son geometría exacta y se prueban contra un modelo ingenuo independiente. El único punto donde Aseprite podría diferir en 1 px es el reparto de un sobrante impar con ancla centrada (D3).

## Decisiones (⚠ = decido algo que no me diste; te las pido abajo)

**D1. Todo el documento.** Las seis acciones actúan sobre todas las capas, ocultas y bloqueadas incluidas (como `Convertir modo` y la paleta, decisión 9 de 3.4); las celdas vacías siguen vacías; opacidad, mezcla, nombre, visibilidad y bloqueo no cambian. Voltear o girar solo la capa activa, o una selección, queda para 4.8 y `Capa`.
**D2. Giros.** 90° horario, 90° antihorario y 180°. Los giros de 90° intercambian ancho y alto. Sin ángulo libre (4.12).
**D3. Tamaño del lienzo.** Nuevos ancho y alto (≥ 1) y ancla de 3 × 3. El píxel `(x, y)` del origen cae en `(x + dx, y + dy)` del resultado, con `dx = 0`, `⌊(nuevo − viejo)/2⌋` o `nuevo − viejo` según el ancla sea izquierda, centro o derecha (ídem `dy`, arriba, centro, abajo); con diferencia negativa recorta, con positiva amplía. `⌊·⌋` es hacia −∞, así que con un sobrante impar el contenido queda medio píxel hacia arriba y hacia la izquierda. Lo que ya no cabe se pierde (deshacer lo devuelve). Sin escalado.
**D4. Relleno de la zona nueva.** RGBA y gris con alfa: transparente `(0,0,0,0)`. Indexado: el índice transparente si lo hay. Si el modo no tiene transparencia (gris, o indexado sin índice transparente), el **color de fondo** de la sesión en ese modo, que `px-app` pasa como `Pixel` a la función pura (el núcleo no conoce roles).
**D5. Un paso de deshacer cada una**, marca el documento como modificado y cambia `state_id` una vez. No es estado de sesión (a diferencia de `Intercambiar colores`).
**D6. Dónde se calcula.** Las imágenes las calcula `px-raster`; `px-core` las recibe en `ReplaceImages` con `based_on = state_id()` y solo valida (cantidad, tamaño, modo, paleta, celdas vacías, tope de memoria). Como en 3.5, un cálculo sobre un estado viejo es `StaleImage`.
**D7. Sin teclas por defecto**, editables en la lista de atajos (las letras están agotadas). Seis claves de ayuda nuevas: 40 → 46 entradas.
**D8. Vista.** Se conserva el zoom y la **fracción** del lienzo que está en el centro de la vista (acotada); la capa activa no cambia. Con un giro de 90° ancho y alto se intercambian en la vista.
**D9. Oráculo.** Ninguno externo: modelo ingenuo (bucles `(x, y)` con la fórmula de arriba) escrito por separado en las pruebas, y la propiedad de que **componer conmuta con voltear y girar** (`compose(T(doc)) == T(compose(doc))`, tolerancia 0).
**D10. Memoria y tiempo.** Pico ≈ `materialized_bytes()` + celdas nuevas. Se rechaza antes de reservar (`px-raster` calcula el resultado con la misma fórmula que `px-core`) si el resultado supera `cells_limit()` **y crece** respecto a hoy (un documento al tope puede girarse, porque no crece); mensaje de aviso y documento intacto. El paso de historia cuesta lo que ocupan las celdas viejas y, como siempre, no se descarta el último. Si el total de píxeles de celdas supera 2²⁴, se muestra «Aplicando…» como «Combinando…» en 3.5.
**D11. Tamaño máximo** el de `Image` (1 GiB por celda) y `cells_limit`; la ventana de tamaño explica el motivo del rechazo («demasiado grande») y no deja pasar ancho o alto 0.
**D12. Modos y capas.** Valen los cuatro modos. Gris (sin alfa) solo tiene una capa por regla del núcleo; no hay caso especial.

## Cambios exactos en `px-core` (todos aditivos; tu visto bueno vale solo para esta lista)

- **C-01** `Command::ReplaceImages { width: u32, height: u32, images: Vec<Option<Image>>, based_on: u64 }`: `images[i]` es la celda de la capa `i` (de abajo arriba). Solo como comando suelto (en un `Batch` es `NotBatchable`).
- **C-02** Validación: `images.len() == capas`; cada `Some` tiene `width × height`, el modo y la paleta del lienzo; `images[i].is_some() == celda i con imagen` (no se materializa ni se vacía nada); `based_on == state_id()`; `width`, `height` ≥ 1. Nada cambia hasta que todo pasa.
- **C-03** `DocumentError::{ReplaceCountMismatch, ReplaceImageMismatch, ReplaceEmptiness}`; se reutilizan `StaleImage`, `CelsTooLarge`, `OutOfMemory` y `NotBatchable`.
- **C-04** Paso de historia que intercambia las imágenes de las celdas y el tamaño del lienzo (como `Converted`), con su `cost` contado; deshacer y rehacer exactos, incluido `state_id`.
- **C-05** Regla de tope: se aplica al **resultado** y solo falla si crece (D10).
- Nada no aditivo: ninguna firma pública existente cambia.

## Cambios exactos en `px-raster` (aditivos)

- **R-01** `flip_horizontal`, `flip_vertical`, `rotate_cw`, `rotate_ccw`, `rotate_180`, `resize_canvas(&Image, width, height, Anchor, Pixel)` y `Anchor` (nueve valores), todas puras, `Result` con `ImageError` y sin pánicos.
- **R-02** `image_command(doc, ImageOp, fill) -> Result<Command, ImageOpError>` con `ImageOp { FlipH, FlipV, Cw, Ccw, Half, Canvas { width, height, anchor } }`: aplica la función a cada celda con imagen, comprueba el tope (D10) **antes** de reservar y rellena `based_on`.

## Piezas de `px-ui` y `px-app`

`px-ui`: seis `AppAction`, seis entradas de `Imagen` (tras `Convertir modo…`, sin separadores), seis claves de ayuda, textos en `image_ops_es.rs` (los reviso contigo en `specs/hito-4-imagen-textos.md` antes del commit final) y la ventana `Tamaño del lienzo…` (ancho, alto, cuadrícula de ancla de 3 × 3 con foco de teclado, texto «De A × B a C × D», Aceptar y Cancelar, `Esc` cancela, `Intro` acepta). `px-app`: `dispatch` de las seis, recálculo de la vista (D8), fondo de la sesión como relleno (D4), «Aplicando…» y el aviso de memoria.

## Casos (12; los de `px-core` y `px-raster` son puros, el resto, banco de `egui` sin ventana o `Session`)

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Voltear.** Imagen asimétrica de 3 × 2 en los cuatro modos; horizontal y vertical; dos veces | Igual al modelo ingenuo; dos veces = original; paleta e índice transparente intactos |
| 2 | **Girar.** 3 × 2 y 1 × 4 en los cuatro modos; derecha, izquierda y 180° | Igual al modelo; ancho y alto intercambiados en 90°; cuatro giros a la derecha = original; derecha e izquierda se anulan; 180° = dos de 90° |
| 3 | **Tamaño y ancla.** Ampliar, recortar y mezclar (más ancho y menos alto), las 9 anclas, diferencias pares e impares | Desplazamientos de la decisión D3 (con `⌊·⌋`); solo la ventana de origen sobrevive al recorte; igual al modelo |
| 4 | **Relleno por modo.** RGBA, gris con alfa, indexado con y sin índice transparente, gris | La zona nueva es la de D4; la paleta no cambia; el modo no cambia |
| 5 | **`ReplaceImages` aplica y deshace.** Documento de 3 capas (una vacía, una oculta, una bloqueada) | Un solo paso; ancho y alto del lienzo nuevos; celdas vacías siguen vacías; propiedades de capa intactas; deshacer y rehacer devuelven documento, tamaño y `state_id` exactos |
| 6 | **`ReplaceImages` rechaza.** `based_on` viejo, cantidad errónea, tamaño, modo o paleta de otra imagen, celda vacía que pasa a tener imagen (y al revés), 0 de ancho, dentro de `Batch`, tope de memoria | El error de C-03 o los ya existentes; el documento queda idéntico (incluidos historia y `state_id`) |
| 7 | **Comando completo sobre un documento.** `image_command` con `FlipH`, `Cw` y `Canvas` en un documento con capas, opacidades y mezclas | Todas las celdas transformadas, incluidas ocultas y bloqueadas; `compose` después = transformada de `compose` antes (tolerancia 0 en volteos y giros) |
| 8 | **Memoria.** Documento al tope de `cells_limit` girado; documento que crecería por encima; paso de historia más grande que el límite | Girar al tope vale; crecer por encima = `CelsTooLarge` antes de reservar; el paso se guarda y se puede deshacer; **medido (no puerta de CI):** pico y tiempo a 8192² con 1 y 4 capas |
| 9 | **Menú, teclas y ayuda.** `Imagen` con 10 entradas en su orden, atenuación con gesto o modal, claves de ayuda, edición de atajos sin tecla por defecto | Las seis acciones nuevas en su sitio, sin tecla por defecto y editables; 46 entradas de ayuda; atenuadas con diálogo o gesto en curso |
| 10 | **Ventana `Tamaño del lienzo…`.** Abrir con el tamaño actual; escribir ancho y alto; ancla con ratón y flechas; valores 0 y desmesurados; `Esc`, `Intro` | Muestra «De A × B a C × D»; el ancla elegida se ve y se mueve con el teclado; 0 y demasiado grande no se aceptan con motivo; `Esc` no cambia nada |
| 11 | **Sesión.** Cada acción por `dispatch`; con arrastre de herramienta, con modal y con editor de color abierto; deshacer y rehacer | Un paso por acción y `modified`; la capa activa no cambia; zoom y centro de la vista según D8; las texturas del lienzo se rehacen; sin efecto con gesto o modal; el relleno usa el fondo de la sesión donde toca (D4) |
| 12 | **Guardar y reabrir.** Girar 90° un documento indexado y otro RGBA de 2 capas y guardar PNG (aplanado) y `.aseprite` | El fichero reabierto tiene las dimensiones nuevas y el contenido esperado (por contenido de píxeles) |

## Propiedades (proptest; 256 casos, 500 como máximo)

1. **Inversas.** Para cualquier imagen de hasta 9 × 7 en cualquiera de los cuatro modos: voltear dos veces es la identidad, `rotate_180 = flip_horizontal ∘ flip_vertical`, cuatro giros de 90° a la derecha son la identidad y derecha ∘ izquierda también.
2. **Tamaño del lienzo.** Para cualquier imagen, tamaño nuevo de 1 a 12 y ancla: el píxel `(x, y)` del resultado es el del origen en `(x − dx, y − dy)` si cae dentro, y el relleno si no; ampliar y recortar de vuelta con el mismo ancla (misma `⌊·⌋`) devuelve la imagen original.
3. **Documento.** Para cualquier documento de 1 a 4 capas (celdas vacías, ocultas, bloqueadas, opacidad y mezcla al azar) y cualquier secuencia de operaciones, deshacer y rehacer: cada operación es **un** paso, `state_id` cambia una vez, deshacerlas todas devuelve el documento inicial exacto (capas, celdas y tamaño), rehacerlas el final; **componer conmuta con voltear y girar** con tolerancia 0.
4. **Memoria y errores.** Para cualquier documento y operación: `materialized_bytes()` después es la suma de las celdas con imagen, nunca supera `cells_limit()` salvo que ya lo superase y no crezca, y una operación que falla deja el documento (capas, historia, `state_id`) idéntico.

## Pruebas nuevas

12 filas y 4 propiedades: **16 como mínimo**, hasta unas 24 con varios casos por fila; tope de 40. Reparto: `px-raster/tests/image_ops.rs` (filas 1 a 4, 7 y propiedades 1 y 2), `px-core/tests/replace_images.rs` (filas 5, 6 y 8, propiedades 3 y 4; la 3 usa `px-raster` y va en `px-raster/tests/`), `px-ui/tests/image_ops.rs` (filas 9 y 10), `px-app/src/session/image_ops_tests.rs` (filas 11 y 12).

## Pruebas anteriores que cambian (por lectura, sin copia desechable; el compilador da los números)

**A. Solo cifras, listas y literales:** la lista esperada de `px-ui/tests/menu.rs` (seis entradas más tras `Convertir modo…`), el recuento de entradas de ayuda de `help_texts.rs` (40 → 46) y el de acciones editables y de claves que cuente la tabla, y los recuentos de `px-ui/tests/keymap.rs` y `keymap_file.rs` si cuentan acciones. **No cambian:** `Command`, `Document` ni ninguna firma pública existente. Si alguna necesita más que cifras, listas, literales o firmas: **me paro y te lo digo**.

## Orden de trabajo

Commits separados: (1) esta spec con su fila del `INDEX`; (2) adaptación de pruebas anteriores (el mensaje explica qué comprobaba cada una); (3) pruebas, en rojo; (4) lógica de `px-core` y `px-raster`; (5) `px-ui`; (6) `px-app`. Cierre: `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; después, el `license-auditor` una sola vez sobre el rango. Luego, la corrección única de esta spec y las notas en `PLAN-RESUMEN.md` y `mapa-funciones.md`.

## Prueba manual (la haces tú, sin capturas)

1. Abrir un PNG indexado asimétrico (con un detalle en una esquina): `Imagen → Voltear horizontal` y `Voltear vertical`; `Ctrl+Z` y `Ctrl+Y` un paso cada vez.
2. `Girar 90° a la derecha` cuatro veces: vuelve el original; `Girar 90° a la izquierda`; `Girar 180°`. El título pasa a «modificado».
3. `Tamaño del lienzo…`: ampliar con el ancla arriba a la izquierda, luego en el centro y abajo a la derecha; mirar el relleno en indexada con y sin transparente (fondo de la sesión), en RGBA y en gris.
4. Recortar con un ancla distinta y deshacer: vuelve lo recortado.
5. Dimensiones con 0 o enormes: no se aceptan y dicen por qué; `Esc` cierra sin cambios.
6. Documento con 3 capas (una oculta, una bloqueada, una vacía): girar y voltear las afecta a todas; propiedades intactas.
7. Con el editor de color abierto o a medio trazo: las acciones no hacen nada.
8. Zoom y posición de la vista tras girar un lienzo grande: sigue viendo el centro.
9. Guardar tras girar y reabrir: dimensiones nuevas.
10. `Ayuda → Atajos de teclado…`: las seis acciones están sin tecla y se les puede asignar una; `F1` sobre su entrada de menú abre su ayuda.

## Fuera de alcance

Escalar o remuestrear la imagen, recortar a una selección (4.8), voltear o girar solo una capa o una selección, giro libre (4.12), cuantizador (4.2), cambiar la resolución, y revisar la capacidad de la caja de herramientas (4.3).

## Decisiones que necesito que confirmes

1. **D1:** ¿las seis acciones sobre el documento entero, incluidas ocultas y bloqueadas?
2. **D3:** el reparto de un sobrante impar con ancla centrada hacia arriba y la izquierda (`⌊·⌋`).
3. **D4:** relleno transparente donde se pueda y el color de fondo de la sesión donde no.
4. **D7:** sin tecla por defecto en ninguna de las seis.
5. **D8:** al cambiar el tamaño, mantener el zoom y el centro relativo de la vista.
6. **D10:** rechazar solo si el resultado supera el tope **y crece**; «Aplicando…» con más de 2²⁴ píxeles.
7. **C-01 a C-05 y R-01 y R-02:** tu aprobación expresa para tocar `px-core` y `px-raster` solo con esa lista.
