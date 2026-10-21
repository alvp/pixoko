# Pixoko: resumen del plan

Resumen de `docs/PLAN.md` para empezar una unidad sin leer el plan entero (17 KB). `PLAN.md` sigue siendo la fuente; solo se abre para cambiar el plan o para algo que este resumen no cubra. Si ambos discrepan, manda `PLAN.md`: corrige aquí.

## Hitos

| Hito | Contenido | Puerta de salida | Estado |
| --- | --- | --- | --- |
| 0 · Fundaciones | Repo, CLAUDE.md, CI en tres sistemas, cargo-deny | CI verde en Linux, Windows y macOS | cerrado |
| 1 · Núcleo y formatos | `Rgba`, paleta, imagen en 4 modos, documento con deshacer, PNG | Ida y vuelta de PNG sin pérdida en el corpus | cerrado |
| 2 · Editor mínimo | Ventana, lienzo, zoom, lápiz, línea, relleno, cuentagotas, borrador, paleta, conversión de modos, lienzos grandes | Abrir, editar y guardar un PNG real en tres sistemas | cerrado (prueba manual confirmada; resumen en `specs/hito-2-resumen.md`) |
| Interfaz estilo programa de referencia | Base de interfaz para las herramientas del hito 4, en cinco unidades (abajo) y con el diseño en `docs/DISENO-INTERFAZ.md` | Por definir en la spec de cada unidad | pendiente |
| ↳ I.1 · Teclado como datos | Tabla de atajos como datos, con las letras del programa de referencia | Pruebas y cierre en verde; prueba manual en `specs/interfaz-teclado.md` | hecha, a falta de la prueba manual |
| ↳ I.2a · Dos colores y botón derecho | Primer plano y fondo, botón derecho, indicador de color, `}` `{` y `K` | Spec en `specs/interfaz-dos-colores.md`; pruebas y cierre en verde | hecha, a falta de la prueba manual |
| ↳ I.2b · Navegación y botones extra del ratón | Tabla de gestos (`GestureMap`): rueda por la unidad, `Mayús`+rueda, `Espacio` como mano (cancela trazos), trackpad, pellizco, botones laterales 4 y 5 (deshacer y rehacer; del 6 en adelante `egui-winit` los descarta) | Spec en `specs/interfaz-raton.md`; pruebas y cierre en verde | hecha, a falta de la prueba manual |
| ↳ I.3 · Caja a la derecha y panel de opciones | Orden lienzo · paleta · caja (borde derecho) con un recuadro fijo de opciones; clic derecho en una herramienta (la elige y da el foco) y en `Acercar`/`Alejar` (la acción contraria); tooltips con nombre y atajo | Spec en `specs/interfaz-caja-derecha.md`; pruebas y cierre en verde | hecha, a falta de la prueba manual |
| ↳ I.4 · Iconos propios | 14 iconos propios de 16×16 (`assets/icons/`, PNG de 1 bit, MIT) en la caja, el zoom y el editor de la paleta, sobre botones de 24 u con relieve (normal, encima, hundido, desactivado); rejilla de 4 columnas, editor 260 → 214 u, ventana mínima 387 → 341 u y el criterio del alto vuelve al 65 % de 720 pt | Spec en `specs/interfaz-iconos.md`; pruebas y cierre en verde | hecha, a falta de la prueba manual |
| ↳ I.5 · Ayuda con F1 y atajos editables | `F1` abre la ayuda de la herramienta o del botón bajo el puntero; cada atajo se edita (principal y secundario, captura, conflictos) y se guarda en `atajos.txt` (formato propio, versionado y tolerante, con copia `.bak`); `Ayuda → Atajos de teclado…` con `Restablecer todo`; modos de pincel a `Mayús`+`F1` a `F8` | Spec en `specs/interfaz-ayuda.md`; pruebas y cierre en verde | hecha (22 textos confirmados), a falta de la prueba manual |
| 3 · Capas, alfa y referencias | 10 unidades (7 núcleo, 3 deseables): `px-app` en lib y binario, modelo de capas (no aditivo), composición con ocho modos de mezcla, pintar sobre capas, panel de capas flotante, combinar y aplanar, página de reserva, referencias, `.aseprite` etapa 1 (capas, celdas y paleta) | Dorados de mezcla en verde (fórmulas propias; LibreSprite solo contra divergencias grandes) y ida y vuelta de un documento con capas | pendiente |
| 4 · Herramientas de pixel art | 14 unidades (8 núcleo, 4 deseables, 2 opcionales): imagen, cuantizador de color, pincel y trazos, formas, relleno ampliado y por filas, paleta, rangos y degradados, selector y menú Pincel, rejilla y simetría, modos de pintura (con Sombrear), aerógrafo; opcionales: giro libre y texto | Dorados contra referencias | pendiente |
| 5 · Animación | 14 unidades (9 núcleo, 2 deseables, 3 opcionales): fotogramas, navegación, línea de tiempo, reproducción, mesa de luz, etiquetas y `.aseprite` etapa 2, GIF, hojas de sprites, ciclo de color (fps propios), pintar animando sin `Alt`; opcionales: pincel animado, paleta por fotograma y mover 2D | GIF y hojas de sprites verificados | pendiente |
| 6 · Más formatos | 5 unidades (3 deseables, 2 opcionales): resto de `.aseprite` (lo básico llega en el 3 y el 5), IFF ILBM, IFF ANIM, BMP y PCX | Corpus de ficheros ajenos se abre | pendiente |
| 7 · Estilo DPaint (opcional) | 8 unidades opcionales, por caja negra: plantillas, perspectiva, mover y cámara en 3D, texturas, difuminar, mezclar, suavizar, mezcla viva, calcar, proceso, ciclo avanzado | Casos del emulador en verde | pendiente |
| 8 · Automatización y lanzamiento | 10 unidades (3 núcleo: preferencias, instaladores y puerta); CLI, traducciones, persistencia PNG y pulidos son deseables; scripting, MCP y macros, opcionales | Instaladores y licencias revisadas | pendiente |

Los hitos 3 a 6 pueden solaparse en worktrees una vez cerrado el 2; el 7 es opcional. Reparto, clases y decisiones de los hitos 3 a 8 en [`specs/mapa-funciones.md`](../specs/mapa-funciones.md) (aprobado el 21 de octubre de 2026; 61 unidades: 27 núcleo, 16 deseables, 18 opcionales). **Hasta que se actualice `PLAN.md`, para los hitos 3 a 8 manda el mapa.**

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

## Hitos 3 a 8: decisiones del 21 de octubre de 2026

Detalle y tabla de decisiones en `specs/mapa-funciones.md`. Las que cambian el plan:

- **`.aseprite` por etapas (D10):** capas, celdas y paleta con un fotograma en el hito 3 (unidad 3.8); fotogramas y etiquetas en el 5 (5.5); el resto en el 6. Antes estaba entero en el 6. Zlib sale de `miniz_oxide` (ya en el árbol; se anota en `docs/DEPENDENCIES.md` al hacerla dependencia directa). Los PNG de referencia de LibreSprite los genera el usuario por lotes y se commitean como corpus; el CI no los ejecuta.
- **Ciclo de color (D23):** en el hito 5, con velocidad propia en fps; antes figuraba en el 7. La curva de velocidad exacta del original queda como opcional del 7.
- **Modos de color (D16):** Sombrear en el hito 4; difuminar, mezclar, suavizar y mezcla viva en el 7.
- **Capas (D04 a D08):** celdas del tamaño del lienzo (las vacías no reservan memoria, con aviso de presupuesto); un modo por documento y la paleta en cada imagen, con una prueba de que todas las paletas son idénticas; indexado sin opacidad ni mezcla; pintar con alfa reemplaza; ocho modos de mezcla con fórmulas propias de enteros; panel de capas flotante y menú `Capa`.
- **Pintar animando (D24):** sin `Alt`, con un botón y una tecla de alternancia en el panel de animación.
- **`px-core` y `px-raster` (D02):** el visto bueno a la spec de una unidad marcada vale solo para los cambios exactos que liste; las specs 3.1 y 3.2 no son aditivas y traen plan de migración.
- **Corpus manual:** una lista única de casos al empezar cada hito (`specs/corpus-hito-N.md`), para generarlos de una sentada.
- **Clases:** cada unidad es núcleo, deseable u opcional; la primera versión pública (hito 8) puede salir sin las opcionales.
- **Después del hito 4:** revisar la composición de la pantalla (editor de color a demanda, indicador de color grande, paleta compacta) y si hace falta un tema gris con biseles.

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
