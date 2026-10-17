# Plan Claude Code: Pixoko, editor de pixel art en Rust

Oct 17, 2026 · @Álvaro Pérez Pozo

## Decisiones de partida

El plan parte de seis decisiones; cada una trae un valor por defecto que puedes cambiar antes del hito 0.

| Decisión | Por defecto | Por qué | Cuándo cambiarla |
| --- | --- | --- | --- |
| Nombre | Pixoko | Es el nombre elegido. No apareció ningún programa llamado Pixoko, pero "Pixiko" (plataforma de edición de vídeo y fotos en línea) se diferencia en una sola letra, así que conviene comprobar la marca con cuidado; una búsqueda web no equivale a un registro de marca | Buscar la marca (EUIPO, OEPM y USPTO), el dominio y el nombre en crates.io y GitHub antes de publicar; si hay conflicto, elegir otro: los crates (`px-`) no cambian |
| Ruta de código | Todo en Rust, desde cero y con licencia MIT. Grafx2, LibreSprite, Aseprite y DPaint V se usan solo como programas de referencia, sin leer, copiar ni portar su código | Grafx2 es [GPL-2.0-only](https://en.wikipedia.org/wiki/GrafX2) y LibreSprite es GPL: copiar de ellos obligaría a publicar bajo GPL. El código de [Aseprite](https://github.com/aseprite/aseprite) va bajo una EULA que impide redistribuirlo | Solo si renuncias a MIT |
| Licencia del proyecto | MIT | Máxima libertad de uso. Las dependencias con Apache-2.0, como [`winit`](https://crates.io/crates/winit), pueden usarse en un proyecto MIT incluyendo su aviso de licencia | GPL-3.0-or-later si prefieres copyleft |
| Interfaz | `eframe` (egui) sobre `winit` y `wgpu`, con tema y fuente de píxeles propios | Una sola base cubre Linux (X11 y Wayland), Windows y macOS, y la web después | Si el aspecto no llega a lo que quieres, dibujar los paneles clave sobre tu propio lienzo |
| Plataformas | Linux, Windows y macOS desde el hito 0, con CI en los tres | Evita descubrir tarde los problemas de cada sistema | Quitar uno solo si no puedes probarlo |
| Estilo DPaint V | Pista opcional (hito 7): se automatiza el programa original en un emulador y se compara lo que guarda, sin descompilarlo | La [Directiva 2009/24/CE](https://www.legislation.gov.uk/eudr/2009/24/article/6/data.html) solo permite descompilar para lograr interoperabilidad, no para replicar su comportamiento | Descartarla si no quieres depender de un emulador |

## Alcance

La meta es reunir lo mejor de los editores de píxeles en uno solo, con cada función asignada a un hito. Todo esto es funcionalidad, que no está protegida por derechos de autor; no se copian código, iconos ni textos de otros programas.

| Área | Funciones | Hito |
| --- | --- | --- |
| Lienzo y color | Modos RGBA, indexado (hasta 256 colores) y escala de grises; alfa e índice transparente; editor de paletas (RGB, HSL, rangos de color, reordenar sin alterar la imagen); lienzos grandes | 1 y 2 |
| Herramientas básicas | Lápiz, pincel, línea, rectángulo, elipse, relleno, cuentagotas, borrador, zoom y desplazamiento | 2 |
| Capas | Capas, grupos, opacidad, modos de mezcla, bloqueo | 3 |
| Referencias | Imágenes de referencia con opacidad y posición, capas de referencia, varios documentos en pestañas | 3 y 6 |
| Herramientas de pixel art | Trazo pixel perfect, formas y polígonos rellenos, contorno, degradados, sombreado, pincel propio, simetría, patrones, modo mosaico, selección y transformaciones | 4 |
| Animación | Fotogramas, celdas, línea de tiempo, onion skin, etiquetas, vista previa, exportar GIF y hojas de sprites | 5 |
| Formatos | PNG (indexado y color), GIF, `.aseprite` desde su especificación pública, IFF ILBM y ANIM, BMP, PCX | 1, 5 y 6 |
| Estilo DPaint V (opcional) | Ciclo de color, stencil, pinceles de animación, perspectiva: solo lo que el manual liste y el corpus confirme | 7 |
| Automatización | CLI sin ventana, scripting, servidor MCP para agentes | 8 |
| Aplicación | Ventana y pantalla completa, HiDPI, atajos configurables, historial con panel, portapapeles, recuperación tras un cierre inesperado, traducciones, instaladores | 2 y 8 |

## Arquitectura

El núcleo no sabe nada de ventanas, así que la aplicación con interfaz y la CLI sin ventana usan el mismo documento y los mismos comandos.

&#91;embedded content: arquitectura · 8 crates en 3 capas\]

Toda edición pasa por comandos con deshacer, y los algoritmos de las herramientas son deterministas: por eso se pueden comparar píxel a píxel con las referencias. Los crates llevan el prefijo `px-`, independiente del nombre del producto.

## Hoja de ruta

Nueve hitos, cada uno con una puerta de salida que se comprueba con pruebas, no con impresiones.

&#91;embedded content: hoja de ruta · 9 hitos, 9 puertas\]

Sin fechas: dependen del ritmo y del gasto que decidas. Los hitos 3 a 6 pueden solaparse en worktrees distintos una vez cerrado el 2, y el 7 es opcional: puede quedar fuera sin afectar al resto.

Versión en texto de la hoja de ruta, para quien lea el documento exportado a Markdown (los diagramas no se exportan):

| Hito | Contenido | Puerta de salida |
| --- | --- | --- |
| 0 · Fundaciones | Repo, CLAUDE.md, CI en tres sistemas, cargo-deny | CI verde: Linux, Windows y macOS compilan |
| 1 · Núcleo y formatos | Color (`Rgba`), paleta con índice transparente, imagen en modos RGBA, indexado y grises, documento y comandos con deshacer, PNG indexado y RGBA | Ida y vuelta de PNG sin pérdida en el corpus |
| 2 · Editor mínimo | Ventana, lienzo, zoom, lápiz, línea, relleno y paleta; conversión RGB y HSL, reordenar la paleta, conversión entre modos, lienzos grandes y panel de paletas | Abrir, editar y guardar un PNG real en tres sistemas |
| 3 · Capas, alfa y referencias | Capas, opacidad, mezcla, imágenes de referencia | Dorados de mezcla en verde |
| 4 · Herramientas de pixel art | Formas, degradados, sombreado, simetría, selección | Dorados contra referencias |
| 5 · Animación | Fotogramas, línea de tiempo, onion skin, GIF | GIF y hojas de sprites verificados |
| 6 · Más formatos | `.aseprite` desde su especificación, IFF, BMP, PCX | Corpus de ficheros ajenos se abre |
| 7 · Estilo DPaint (opcional) | Ciclo de color, stencil, perspectiva, por caja negra | Casos del emulador en verde |
| 8 · Automatización y lanzamiento | CLI, scripting, MCP, traducciones, instaladores | Primera versión: instaladores y licencias revisadas |

## Repositorio y Claude Code

Un solo repositorio con un `CLAUDE.md` corto, las reglas en `.claude/` y una unidad de trabajo pequeña por sesión.

**Estructura**

```text
pixoko/                         (raíz del repositorio)
├── CLAUDE.md                   reglas del proyecto, cortas
├── .claude/
│   ├── settings.json           hooks y permisos
│   ├── hooks/guard.sh
│   ├── agents/                 test-writer.md, license-auditor.md, ui-reviewer.md
│   └── commands/               milestone.md
├── Cargo.toml                  workspace
├── crates/                     px-core, px-raster, px-formats, px-render,
│                               px-ui, px-app, px-cli, px-golden
├── specs/                      una página por función: comportamiento y casos límite
├── golden/                     entradas y salidas de referencia
├── deny.toml                   política de licencias (cargo-deny)
└── docs/                       PLAN.md, DEPENDENCIES.md, COSTES.md
```

**CLAUDE.md**

```markdown
# Pixoko: pixel art en Rust, licencia MIT
- Rust estable. `unsafe` prohibido salvo en px-render, con comentario.
- Antes de codificar una función: página en specs/ y tests; después, el código.
- Proyecto MIT: nada de código GPL, LGPL ni AGPL.
- Nunca copies ni traduzcas código de Grafx2, LibreSprite, Aseprite ni DPaint.
  Para ver su comportamiento, ejecuta el programa y guarda la salida en golden/.
- Toda dependencia nueva pasa `cargo deny check` y se anota en docs/DEPENDENCIES.md.
- Antes de cada commit: cargo fmt, cargo clippy -- -D warnings, cargo test --workspace.
- Una unidad de trabajo es una función o un panel; commits pequeños.
```

**Hook de guardia.** Los hooks se configuran en `.claude/settings.json`; con código de salida 2 el hook bloquea la llamada y devuelve el mensaje a Claude ([referencia de hooks](https://docs.anthropic.com/en/docs/claude-code/hooks)). Un [informe de errores](https://github.com/anthropics/claude-code/issues/13744) describe que en una versión no bloqueaba Write y Edit: pruébalo en la tuya.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Read|Glob|Grep|Bash|WebFetch",
        "hooks": [
          { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/guard.sh" }
        ]
      }
    ]
  }
}
```

```bash
#!/bin/bash
INPUT=$(cat)
TARGET=$(echo "$INPUT" | jq -r '[.tool_input.file_path, .tool_input.path, .tool_input.command, .tool_input.url] | map(select(. != null)) | join(" ")')
# La especificación pública del formato .aseprite sí se puede leer
echo "$TARGET" | grep -qi "ase-file-specs" && exit 0
for p in "GrafX2/grafX2" "LibreSprite/LibreSprite" "aseprite/aseprite" "dpaint"; do
  if echo "$TARGET" | grep -qi -- "$p"; then
    echo "Bloqueado: no se lee ni se clona código de programas de referencia ($p)" >&2
    exit 2
  fi
done
exit 0
```

**Subagentes.** Se guardan como Markdown con cabecera YAML en `.claude/agents/`; si omites `tools`, heredan todas las herramientas ([documentación de subagentes](https://docs.claude.com/en/docs/claude-code/sub-agents)), así que define siempre la lista mínima.

```markdown
---
name: test-writer
description: Escribe tests (unitarios, de propiedades y dorados) a partir de una página de specs/ antes de que exista el código. Úsalo al empezar cada unidad.
tools: Read, Write, Edit, Grep, Glob, Bash
model: sonnet
---
Lee solo la página indicada de specs/ y los archivos de golden/ que cite. Escribe los tests; no implementes la función.
```

```markdown
---
name: license-auditor
description: Revisa dependencias y archivos nuevos antes de cada commit. Úsalo tras añadir una dependencia o un formato.
tools: Read, Grep, Glob, Bash
model: sonnet
---
Ejecuta cargo deny check. Busca en el diff texto o estructuras copiadas de otros programas. Informa de cada hallazgo; no edites archivos.
```

**Comando de hito** (`.claude/commands/milestone.md`)

```markdown
Trabaja en el hito $ARGUMENTS de docs/PLAN.md.
1. Lista las unidades pendientes y propón la primera (una función o un panel).
2. Escribe su página en specs/ y pide mi visto bueno.
3. Usa el subagente test-writer, implementa, ejecuta cargo fmt, clippy y test, y haz commit.
```

**Sesiones en paralelo.** Usa un `git worktree` por hito y una sesión de Claude Code en cada uno. Una vez cerrado el hito 2, los hitos 3 a 6 pueden solaparse.

## Bucle de trabajo y coste

Cada unidad sigue el mismo bucle, y el gasto de tokens se controla con unidades pequeñas, sesiones cortas y el modelo adecuado para cada tarea.

1. **Plan corto.** Pide a Claude un plan de la unidad antes de tocar código.
2. **Especificación.** Una página en `specs/` con comportamiento, entradas, salidas y casos límite.
3. **Tests primero.** Unitarios, de propiedades y, si hay referencia, dorados (subagente `test-writer`).
4. **Implementación mínima** que haga pasar los tests.
5. **Comprobaciones.** `cargo fmt`, `cargo clippy -- -D warnings` y `cargo test --workspace`.
6. **Revisión** con `license-auditor` y una lectura humana del diff.
7. **Commit** y sesión nueva para la siguiente unidad.

**Control del gasto**

- Mantén `CLAUDE.md` corto: se carga en cada sesión.
- Una unidad por sesión. Al cerrarla, empieza otra (o usa `/clear`) en vez de arrastrar el contexto.
- Modelo según la tarea: tests, formatos y código repetitivo con uno más barato (por ejemplo `model: sonnet` en el subagente); arquitectura y revisiones con el más capaz.
- No hagas leer a Claude logs largos: resume el error o pega solo las líneas relevantes. Para dependencias, apunta a docs.rs.
- Las tandas mecánicas (un lector de formato con su batería de tests) pueden ir con `claude -p` y el mismo `settings.json`; pruébalo antes en tu versión.
- Mide: anota el gasto por hito en `docs/COSTES.md` (con `/`usage y /context; /cost solo existe como alias en algunas versiones) y ajusta el tamaño de las unidades.

No hay una cifra fiable de antemano, porque depende del tamaño de cada unidad; por eso se mide desde el hito 0.

## Pruebas y referencias

Cada función con comportamiento comparable se verifica contra un programa de referencia sin leer su código: se ejecuta, se guarda la salida y se compara.

| Referencia | Cómo se usa | Límite |
| --- | --- | --- |
| Grafx2 y LibreSprite (GPL) | Se instalan como binarios, se ejecutan con entradas fijas y la salida va a `golden/` | No se clona ni se lee su código; solo se comparan salidas |
| Aseprite | Formato `.aseprite` según su especificación pública; salidas de una copia que uses con licencia, solo para comparar | No se usa su código; su EULA impide redistribuirlo |
| DPaint V en emulador | Se automatizan casos "acción a imagen guardada" y las salidas van a `golden/`, para el hito 7 | No se descompila ni se desensambla |
| Especificaciones públicas | PNG, GIF, IFF ILBM y ANIM | Se cita la versión en la página de `specs/` |

Otras reglas de prueba:

- Un dorado es entrada, acción y salida esperada. La igualdad es exacta en modo indexado y lleva una tolerancia explícita por prueba en RGBA con mezcla.
- Pruebas de propiedades (`proptest`): hacer y deshacer deja el documento igual; guardar y cargar no cambia nada.
- Fuzzing (`cargo-fuzz`) de cada lector de formatos.
- CI en Linux, Windows y macOS con las mismas pruebas.
- Si dos referencias discrepan, se anota la decisión y su motivo en la página de `specs/`.

## Licencias y dependencias

La política es simple: ningún código ajeno copiado y solo dependencias que `cargo deny` acepte. La licencia del proyecto es MIT.

```toml
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "Unicode-3.0"]
confidence-threshold = 0.9
```

Todo lo que no esté en la lista se rechaza; cada excepción se aprueba a mano y se anota en `docs/DEPENDENCIES.md`. Los nombres de campo pueden variar según la versión de `cargo-deny`.

| Crate | Licencia | Nota |
| --- | --- | --- |
| [`winit`](https://crates.io/crates/winit) | Apache-2.0 | Se puede usar en un proyecto MIT; hay que incluir su aviso de licencia |
| [`egui-winit`](https://crates.io/crates/egui-winit) y la familia `egui` / `eframe` | MIT OR Apache-2.0 | `eframe` se apoya en `winit` |
| Resto (`png`, `gif`, `wgpu`, `arboard`, `rfd`…) | Por confirmar con `cargo deny` | La mayoría de crates de Rust son MIT OR Apache-2.0 |

**Cómo se mantiene MIT.** No entra ninguna dependencia GPL, LGPL ni AGPL. Las que usan Apache-2.0 (como `winit`) valen, y sus avisos van en un archivo de licencias de terceros, por ejemplo generado con `cargo-about`. El código de [Grafx2](https://en.wikipedia.org/wiki/GrafX2) y de [LibreSprite](https://github.com/joyel24/LibreSprite) es GPL y el de [Aseprite](https://github.com/aseprite/aseprite) va bajo una EULA que impide redistribuirlo, así que no se copia, traduce ni consulta: solo se ejecutan los programas.

## Riesgos y mitigaciones

El mayor riesgo es el alcance: un editor completo no tiene fin si no se acota con hitos y puertas.

| Riesgo | Mitigación |
| --- | --- |
| Alcance sin fin | Hitos con puerta de salida; cada función con página de especificación; recortar antes de añadir |
| Coste de tokens | Unidades pequeñas, sesiones cortas, modelo por tarea y gasto anotado por hito |
| Aspecto pixel en egui | Tema y fuente propios; si no basta, lienzo y paneles críticos dibujados a mano |
| Rendimiento con lienzos grandes y muchas capas | Compositor por regiones modificadas, GPU opcional y benchmarks en CI |
| Código ajeno colado en el proyecto, que rompería la licencia MIT | Hook de guardia, especificaciones propias, `license-auditor` y revisión humana del diff |
| Licencia de dependencias | `cargo deny` en CI y registro en `docs/DEPENDENCIES.md` |
| Nombre "Pixoko" | Existe Pixiko (edición de vídeo y fotos en línea), a una letra de distancia, y una búsqueda web no equivale a un registro de marca: buscar en EUIPO, OEPM y USPTO, comprobar dominio, crates.io y GitHub, y tener un nombre alternativo |
| Pista DPaint V | Solo caja negra, sin descompilar ni desensamblar; consultar a un abogado antes de publicar |
| Cambios de API en `winit` y `egui` | Versiones fijadas y actualización al cerrar cada hito |

## Primeros pasos

La primera sesión termina con el hito 0 cerrado: un workspace vacío que compila y pasa CI en los tres sistemas.

- [ ] Confirmar las seis decisiones de partida y comprobar que "Pixoko" está libre (marca, dominio, crates.io y GitHub).
- [ ] Crear el repositorio `pixoko` con la estructura de "Repositorio y Claude Code", su `CLAUDE.md` y un archivo `LICENSE` con la licencia MIT.
- [ ] Añadir `.claude/settings.json`, el hook de guardia, los dos subagentes y el comando de hito.
- [ ] Escribir `deny.toml` y probar `cargo deny check`.
- [ ] Configurar CI en Linux, Windows y macOS con fmt, clippy, test y cargo-deny.
- [ ] Instalar Grafx2 y LibreSprite solo como binarios de referencia.
- [ ] Hito 0: crear los crates vacíos y comprobar que compilan en los tres sistemas.
- [ ] Probar el hook: pedir a Claude que clone el repositorio de Grafx2; debe quedar bloqueado.

Requisitos previos: Rust (rustup), git, `jq` y `cargo-deny` (`cargo install cargo-deny`); en Windows, además, bash para el hook (WSL o Git Bash). Con `docs/PLAN.md` ya en la carpeta, empieza con este prompt de arranque:

```text
Lee docs/PLAN.md. Crea lo descrito en "Repositorio y Claude Code": CLAUDE.md, .claude/settings.json, .claude/hooks/guard.sh, los subagentes test-writer y license-auditor, el comando milestone, deny.toml y un LICENSE MIT. Después ejecuta git init y haz el primer commit. No empieces el hito 0: espera a que yo lo revise.
```

Cuando hayas revisado lo que creó, usa este prompt para el hito 0:

```text
Lee CLAUDE.md y docs/PLAN.md. Trabaja en el hito 0: crea el workspace con los crates del plan, vacíos pero compilando, y el CI de tres sistemas. No añadas dependencias sin pasar cargo deny. Cuando acabes, enséñame el resultado y espera.
```
