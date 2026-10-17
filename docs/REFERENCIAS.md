# Binarios de referencia

Programas instalados en la máquina de desarrollo que se usan solo para **observar su comportamiento**. Su salida se guarda en `golden/` y los tests de Pixoko la comparan.

## Regla de uso

Ambos programas son GPL. Pixoko es MIT, así que:

- Se ejecutan como cajas negras: entrada → salida.
- No se abre, lee, copia ni traduce su código fuente (ni el de Grafx2, LibreSprite, Aseprite o DPaint).
- Lo que se captura va a `golden/`, con una nota de qué comando lo produjo y con qué versión.

## Versiones instaladas

Salida de `pacman -Q grafx2 libresprite` (consultada el 2026-10-17):

| Paquete | Versión | Licencia | Binarios |
| --- | --- | --- | --- |
| `grafx2` | 2.9-2 | GPL-2.0-or-later | `/usr/bin/grafx2` |
| `libresprite` | 1.3-1 | GPL-2.0-only | `/usr/bin/libresprite`, `/usr/bin/libresprite-thumbnailer` |

Al anotar una captura en `golden/`, se cita la versión del paquete (`pacman -Q`). `libresprite --version` imprime `LibreSprite 1.2-dev (local build)` aunque el paquete sea 1.3-1, así que esa cadena no sirve para identificar la versión.

## Cómo se ejecutan

### LibreSprite

Tiene modo por línea de comandos sin interfaz (`-b` / `--batch`), que es el que sirve para generar capturas reproducibles. Lista completa de opciones: `libresprite --help`.

```sh
# Convertir un fichero a otro formato
libresprite -b entrada.ase --save-as salida.png

# Exportar una hoja de sprites con sus metadatos
libresprite -b entrada.ase --sheet hoja.png --data hoja.json --format json-array

# Listar capas y etiquetas de un sprite
libresprite -b entrada.ase --list-layers --list-tags

# Ejecutar un script (`--script <fichero>`; el lenguaje aún no está comprobado)
```

Estos comandos salen de `libresprite --help`; todavía no se han ejecutado con ficheros reales.

Con `-v` / `--verbose` explica lo que hace. Sin `-b` abre la interfaz gráfica, que necesita sesión gráfica (aquí `DISPLAY=:1` / `WAYLAND_DISPLAY=wayland-1`).

### Grafx2

Se ha comprobado que `grafx2 --help` no imprime ayuda: arranca la interfaz gráfica y se queda abierto (con `timeout` termina con código 124). `grafx2 --version` falla con `Error: Invalid parameter or file not found.`

Por tanto Grafx2 se trata como programa gráfico:

```sh
grafx2 imagen.png
```

Cualquier captura de su comportamiento exige sesión gráfica y se hace a mano (acciones en la interfaz → fichero guardado → `golden/`). No hay `xvfb-run` instalado, así que no se puede lanzar sin pantalla.

## Pendiente

- Decidir el formato de las notas que acompañan a cada fichero de `golden/` (comando, versión del paquete, fecha).
- Comprobar si Grafx2 admite alguna opción útil de línea de comandos más allá de abrir un fichero. Debe averiguarse ejecutándolo, no leyendo su código.
