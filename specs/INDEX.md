# Índice de specs

Una línea por spec. Lee este índice y **solo las specs que la unidad cite expresamente**; no abras las demás «por si acaso». Las specs grandes (más de 150 líneas) se leen por secciones: «Casos», «Piezas» y «Visto bueno» suelen bastar. Al añadir una spec, añade su fila aquí en el mismo commit.

Estado: *aprobada* = con visto bueno; *en curso* = unidad abierta; *notas* = sin tests ni comportamiento propio. Crates tocados: sacados del historial de commits de la unidad (sin `docs/` ni `Cargo`).

## Hito 0

| Fichero | Unidad | Estado | Qué decide | Crates |
| --- | --- | --- | --- | --- |
| [hito-0-workspace.md](hito-0-workspace.md) | 0.1 | aprobada | Workspace de Cargo con los 8 crates vacíos, sin dependencias | todos |
| [hito-0-ci.md](hito-0-ci.md) | 0.2 | aprobada | CI de GitHub Actions en Linux, Windows y macOS con fmt, clippy, test y cargo-deny | px-golden |

## Hito 1

| Fichero | Unidad | Estado | Qué decide | Crates |
| --- | --- | --- | --- | --- |
| [hito-1-rgba.md](hito-1-rgba.md) | 1.1 | aprobada | `Rgba` de 8 bits por canal y su texto `#rrggbb[aa]` | px-core |
| [hito-1-palette.md](hito-1-palette.md) | 1.2 | aprobada | `Palette` de hasta 256 entradas con alfa propio e índice transparente opcional | px-core |
| [hito-1-image.md](hito-1-image.md) | 1.3 | aprobada | `Image` de un plano en cuatro modos (RGBA, indexado, grises, grises con alfa) | px-core |
| [hito-1-document.md](hito-1-document.md) | 1.4 | aprobada | `Document` y comandos como datos con deshacer/rehacer (`SetPixels`, ...) | px-core |
| [hito-1-png.md](hito-1-png.md) | 1.5 | aprobada | Lector y escritor PNG sobre bytes, ida y vuelta sin pérdida de valores de píxel | px-formats |
| [hito-1-png-corpus.md](hito-1-png-corpus.md) | 1.6 | aprobada | Corpus de ida y vuelta, conversiones declaradas y errores (puerta del hito 1) | px-golden |
| [hito-1-fuzz.md](hito-1-fuzz.md) | 1.x | aprobada | Fuzzing de `read_png` con `cargo-fuzz`; excepción NCSA solo para `libfuzzer-sys` | fuzz, deny.toml |

## Hito 2

| Fichero | Unidad | Estado | Qué decide | Crates |
| --- | --- | --- | --- | --- |
| [hito-2-reparto-cierre.md](hito-2-reparto-cierre.md) | reparto | aprobada | Orden de las unidades 16 a 21 y 12 a 14c, y decisiones D1 a D12; solo reparto, sin tests | — |
| [hito-2-notas-unidad-14.md](hito-2-notas-unidad-14.md) | 14 | notas | Pendientes consolidados: topes de relleno y memoria, textura, API, PNG, sombreador | — |
| [hito-2-hsl.md](hito-2-hsl.md) | 2.1 | aprobada | `Hsl` y conversión RGB↔HSL de ida y vuelta | px-core |
| [hito-2-ventana.md](hito-2-ventana.md) | 2.2 | aprobada | Ventana con `eframe`, zoom entero y desplazamiento; versiones fijadas | px-app, px-ui |
| [hito-2-ventana-licencias.md](hito-2-ventana-licencias.md) | 2.2 (anexo) | notas | Textos literales de las licencias nuevas del árbol de `eframe` (solo consulta) | — |
| [hito-2-linea.md](hito-2-linea.md) | 2.3 | aprobada | Línea determinista en enteros y su ejecución como `SetPixels` | px-raster |
| [hito-2-historia-regiones.md](hito-2-historia-regiones.md) | 2.4 | aprobada | `FillSpans` y pasos de historia por regiones (coste por tramos, no por píxel) | px-core |
| [hito-2-relleno.md](hito-2-relleno.md) | 2.5 | aprobada | Relleno por inundación como un solo `FillSpans`; `Image::check_pixel` pública | px-core, px-raster |
| [hito-2-lapiz-deshacer.md](hito-2-lapiz-deshacer.md) | 2.6 | aprobada | Lápiz con un paso de historia por trazo, color activo y atajos de deshacer | px-app, px-ui |
| [hito-2-linea-relleno-app.md](hito-2-linea-relleno-app.md) | 2.7 | aprobada | Herramientas línea y relleno en la app, un paso por gesto, orden de `Escape` | px-app, px-ui |
| [hito-2-color-cercano.md](hito-2-color-cercano.md) | 2.8 | aprobada | Búsqueda del color más cercano de una paleta en aritmética entera | px-core |
| [hito-2-conversion-modos.md](hito-2-conversion-modos.md) | 2.9 | aprobada | Conversión entre modos sin pérdida silenciosa: exacta, con informe o con error | px-core |
| [hito-2-paleta-editar.md](hito-2-paleta-editar.md) | 2.10 | aprobada | Editar, reordenar y rotar la paleta indexada como `Command` con deshacer | px-core |
| [hito-2-texto.md](hito-2-texto.md) | 2.11a | aprobada | Fuente de píxeles sin desenfoque, escala, tema y barra de estado | px-app, px-ui |
| [hito-2-panel-paletas.md](hito-2-panel-paletas.md) | 2.11b | aprobada | Panel de paletas a la izquierda; elegir color con ratón y teclas es el mismo mecanismo | px-app, px-ui |
| [hito-2-editar-color.md](hito-2-editar-color.md) | 2.11c | aprobada | Editor de color (RGB, HSL, hex, alfa); un arrastre es un paso de deshacer | px-app, px-ui |
| [hito-2-panel-ventana-baja.md](hito-2-panel-ventana-baja.md) | 2.11c (ajuste) | aprobada | Panel con ventana baja y ancho mínimo; sin salto brusco al aparecer el editor | px-app, px-ui |
| [hito-2-paleta-operaciones.md](hito-2-paleta-operaciones.md) | 2.11d | aprobada | Añadir, quitar, mover y rotar entradas desde el panel; solo en indexado | px-app, px-ui |
| [hito-2-abrir-guardar.md](hito-2-abrir-guardar.md) | 2.15 | aprobada | Abrir y guardar PNG por argumento, soltar y `Ctrl+S`; `Document::state_id` | px-app, px-core, px-formats, px-ui |
| [hito-2-dialogos-archivo.md](hito-2-dialogos-archivo.md) | 2.16 | aprobada | Diálogos nativos con `rfd` no bloqueantes y tras un trait; aviso si faltan portal y `zenity` | px-app, px-ui |
| [hito-2-cuentagotas-borrador.md](hito-2-cuentagotas-borrador.md) | 2.17a | aprobada | Cuentagotas (`I`) y borrador (`E`) por modo; valor de borrado en indexado | px-app, px-ui |
| [hito-2-menu-herramientas.md](hito-2-menu-herramientas.md) | 2.17 | aprobada | Menú, columna de herramientas y zoom con una sola capa de acciones; geometría y ventana mínima | px-app, px-ui |
| [hito-2-damero-lienzo.md](hito-2-damero-lienzo.md) | 2.17b | aprobada | Damero de dos grises bajo la transparencia y fondo del lienzo elegible | px-app, px-ui |
| [hito-2-dialogos-modales.md](hito-2-dialogos-modales.md) | 2.18 | en curso | Formulario como datos, estado puro de diálogos y campos numéricos; foco inicial con clic | px-app, px-ui |
| [hito-2-nueva-imagen.md](hito-2-nueva-imagen.md) | 2.19 | en curso | `Archivo → Nueva imagen…` (`Ctrl/Cmd+N`): fondo por modo, paleta inicial de 32 colores, reserva previa de memoria y límite de la GPU | px-app, px-ui |
| [hito-2-convertir-modo.md](hito-2-convertir-modo.md) | 2.20 | aprobada | `Imagen → Convertir modo…`: informe de `convert_image` sin tocar el documento, permisos explícitos, fondo del alfa elegido, un paso de deshacer; menú `Imagen` nuevo | px-app, px-ui |
| [hito-2-motivo-y-marcadores.md](hito-2-motivo-y-marcadores.md) | 2.20b | aprobada | Motivo de bloqueo con todos los requisitos pendientes en una línea de 37 caracteres; opciones excluyentes redondas (aro y punto) distintas de las casillas | px-app, px-ui |
| [hito-2-chunks-auxiliares.md](hito-2-chunks-auxiliares.md) | 2.13b | aprobada | Informe de los chunks auxiliares ignorados del PNG (`PngRead.aux`); con `iCCP` (o color no sRGB) `Ctrl+S` guarda una copia y el aviso lo explica; el resto solo se informa | px-formats, px-app |
