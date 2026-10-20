# Pixoko: resumen del plan

Resumen de `docs/PLAN.md` para empezar una unidad sin leer el plan entero (17 KB). `PLAN.md` sigue siendo la fuente; solo se abre para cambiar el plan o para algo que este resumen no cubra. Si ambos discrepan, manda `PLAN.md`: corrige aquí.

## Hitos

| Hito | Contenido | Puerta de salida | Estado |
| --- | --- | --- | --- |
| 0 · Fundaciones | Repo, CLAUDE.md, CI en tres sistemas, cargo-deny | CI verde en Linux, Windows y macOS | cerrado |
| 1 · Núcleo y formatos | `Rgba`, paleta, imagen en 4 modos, documento con deshacer, PNG | Ida y vuelta de PNG sin pérdida en el corpus | cerrado |
| 2 · Editor mínimo | Ventana, lienzo, zoom, lápiz, línea, relleno, cuentagotas, borrador, paleta, conversión de modos, lienzos grandes | Abrir, editar y guardar un PNG real en tres sistemas | cerrado (prueba manual confirmada; resumen en `specs/hito-2-resumen.md`) |
| Interfaz estilo programa de referencia | Base de interfaz para las herramientas del hito 4, en cuatro unidades (abajo) y con el diseño en `docs/DISENO-INTERFAZ.md` | Por definir en la spec de cada unidad | pendiente |
| ↳ I.1 · Teclado como datos | Tabla de atajos como datos, con las letras del programa de referencia | Pruebas y cierre en verde; prueba manual en `specs/interfaz-teclado.md` | hecha, a falta de la prueba manual |
| ↳ I.2a · Dos colores y botón derecho | Primer plano y fondo, botón derecho, indicador de color, `}` `{` y `K` | Spec en `specs/interfaz-dos-colores.md`; pruebas y cierre en verde | hecha, a falta de la prueba manual |
| ↳ I.2b · Navegación y botones extra del ratón | Tabla de gestos (`GestureMap`): rueda por la unidad, `Mayús`+rueda, `Espacio` como mano (cancela trazos), trackpad, pellizco, botones laterales 4 y 5 (deshacer y rehacer; del 6 en adelante `egui-winit` los descarta) | Spec en `specs/interfaz-raton.md` (aprobada) | en curso |
| ↳ I.3 · Caja a la derecha y panel de opciones | Caja de herramientas a la derecha con un panel fijo de opciones; clic derecho en una herramienta (foco al panel) y en los botones de zoom (aleja) | Por definir en su spec | pendiente |
| ↳ I.4 · Iconos propios | Iconos dibujados con Pixoko, antes de las herramientas de formas del hito 4 | Por definir en su spec | pendiente |
| 3 · Capas, alfa y referencias | Capas, opacidad, mezcla, imágenes de referencia | Dorados de mezcla en verde | pendiente |
| 4 · Herramientas de pixel art | Pincel, rectángulo, elipse, degradados, sombreado, simetría, selección | Dorados contra referencias | pendiente |
| 5 · Animación | Fotogramas, línea de tiempo, onion skin, GIF | GIF y hojas de sprites verificados | pendiente |
| 6 · Más formatos | `.aseprite` (especificación pública), IFF, BMP, PCX | Corpus de ficheros ajenos se abre | pendiente |
| 7 · Estilo DPaint (opcional) | Ciclo de color, stencil, perspectiva, por caja negra | Casos del emulador en verde | pendiente |
| 8 · Automatización y lanzamiento | CLI, scripting, MCP, traducciones, instaladores | Instaladores y licencias revisadas | pendiente |

Los hitos 3 a 6 pueden solaparse en worktrees una vez cerrado el 2; el 7 es opcional.

**Hito 2:** cerrado. Todas las unidades hechas, la 14b descartada y la puerta (unidad 22: prueba automática en `px-app` y `px-golden`, lista manual de 15 pasos en `specs/hito-2-puerta.md`) superada: la prueba manual salió bien en Linux, Windows y macOS. Unidades, aplazado a los hitos siguientes con su spec de origen y cifras medidas: `specs/hito-2-resumen.md`. Candidato para el hito 3: partir `px-app` en lib y binario.

## Interfaz estilo programa de referencia

Entre el hito 2 y el 3. Diseño completo en `docs/DISENO-INTERFAZ.md` (sección del plan: «Interfaz estilo programa de referencia» en `PLAN.md`). El manual de referencia se queda fuera del repositorio; las specs describen con palabras propias, sin copiar texto ni imágenes. Los menús del original se añaden con cada función, no son una unidad de este bloque.

**Decisiones tomadas el 20 de octubre de 2026** (las cinco, tal como se proponían):
1. Menú de desbordamiento al final de la barra, porque doce menús no caben en la ventana mínima de 468 u; los menús se añaden cuando existe su función.
2. Fondo por defecto según el modo de la imagen.
3. Rangos de color con `Mayús` (rango contiguo) y `Ctrl` (añadir o quitar); sin `Alt`, que en KDE mueve la ventana.
4. Con el botón pulsado solo se permiten zoom, desplazamiento, cambio de color y cancelar.
5. Iconos propios antes de las herramientas de formas del hito 4.

**Soporte mínimo de ratón:** tres botones (izquierdo, central y derecho) con asignación propia para cada uno, y todos los botones extra que tenga el ratón, reasignables a cualquier acción de la tabla de atajos. Quien no tenga tres botones o un trackpad llega a lo mismo con gestos y teclas. El detalle de la unidad del ratón se decide después del hito 2.

**Dependencias nuevas:** ninguna.

## Arquitectura

El núcleo no sabe nada de ventanas: la interfaz y la CLI usan el mismo documento y los mismos comandos. Toda edición es un `Command` con deshacer; los algoritmos son deterministas. Prefijo `px-` fijo, independiente del nombre del producto.

| Crate | Papel | Depende de |
| --- | --- | --- |
| `px-core` | `Rgba`, `Palette`, `Image`, `Document`, comandos e historia | — |
| `px-raster` | Algoritmos de píxeles (línea, relleno) | px-core |
| `px-formats` | Lectores y escritores de formatos (PNG hoy) | px-core, `png` |
| `px-ui` | Lógica de interfaz pura y pintado con egui; se prueba sin ventana | px-core, px-raster, `egui` |
| `px-app` | Ventana (`eframe`), diálogos de archivo, bucle de eventos | px-core, px-ui, px-formats, `eframe`, `rfd` |
| `px-render` | Render; el único crate donde se permite `unsafe` (con comentario). Vacío hoy | — |
| `px-cli` | CLI sin ventana (hito 8). Vacío hoy | — |
| `px-golden` | Corpus y pruebas dorados | px-core, px-formats (dev) |

Fuera del workspace: `fuzz/` (`cargo-fuzz` del lector PNG, `specs/hito-1-fuzz.md`).

## Decisiones vigentes

- **Licencia:** MIT. Nada de código GPL, LGPL ni AGPL en el árbol ni en las dependencias.
- **Ruta de código:** todo en Rust, desde cero. Grafx2, LibreSprite, Aseprite y DPaint V son solo programas de referencia: se ejecutan y su salida va a `golden/`; no se lee, copia ni porta su código (`.claude/hooks/guard.sh` lo bloquea). La especificación pública de `.aseprite` sí se puede leer. DPaint V (hito 7): solo caja negra en emulador, sin descompilar. Ver `docs/REFERENCIAS.md`.
- **Interfaz:** `eframe`/`egui` sobre `winit` y `glow`, con tema y fuente de píxeles propios; menú dentro de la ventana en los tres sistemas.
- **Plataformas:** Linux, Windows y macOS con CI desde el hito 0.
- **Dependencias:** `deny.toml` permite MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib y Unicode-3.0; cualquier otra es una excepción aprobada a mano y anotada en `docs/DEPENDENCIES.md` (hoy solo NCSA para `libfuzzer-sys`, en `fuzz/deny.toml`, no en el de la raíz). Fijadas hoy: `png` 0.18.1, `eframe` y `egui` `=0.37.0`, `rfd` 0.17.2, `proptest` 1.12.0. El material ajeno que no es una dependencia de Cargo va en `docs/THIRD-PARTY.md`.
- **Pruebas:** dorados exactos en indexado y con tolerancia explícita en RGBA con mezcla; propiedades con `proptest` (hacer/deshacer, guardar/cargar); fuzzing de cada lector; si dos referencias discrepan, la decisión se anota en la spec.
- **Alcance del hito 2:** pincel, rectángulo y elipse pasaron al hito 4; el cuantizador de «Convertir» también (hito 4).
- **Nombre «Pixoko»:** pendiente de comprobar marca, dominio, crates.io y GitHub antes de publicar (existe «Pixiko», a una letra).
