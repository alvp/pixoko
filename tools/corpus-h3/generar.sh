#!/usr/bin/env bash
# Corpus del hito 3 (specs/corpus-hito-3.md): genera en golden/aseprite/ los 25 casos con Aseprite
# por lotes y deja, para cada uno, el .aseprite, el PNG aplanado y el .sha256 del PNG.
#
# Uso:
#   tools/corpus-h3/generar.sh                  todos los casos
#   tools/corpus-h3/generar.sh caso [caso...]   solo esos (README.md se rehace igual)
#   tools/corpus-h3/generar.sh --prueba         paso 0: versión y modo por lotes, sin tocar golden/
#   tools/corpus-h3/generar.sh --verificar      vuelca capas y celdas de lo ya generado
# Variables: ASEPRITE (orden del programa, por defecto `aseprite`) y SALIDA (por defecto
# golden/aseprite).
#
# Cada caso lo dibuja generar.lua desde cero (colores y formas propios, paleta propia). El PNG sale
# de abrir el .aseprite ya guardado y exportarlo con --save-as (aplana las capas visibles). El hash
# lo calcula Pillow, no Pixoko: SHA-256 de la imagen en RGBA de 8 bits, sin cabecera, como en
# golden/png/. Los resultados no se commitean solos: se revisan y se commitean a mano.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASEPRITE="${ASEPRITE:-aseprite}"
SALIDA="${SALIDA:-$RAIZ/golden/aseprite}"
LUA="$RAIZ/tools/corpus-h3/generar.lua"

# Misma lista que la tabla de specs/corpus-hito-3.md (un caso por fila, 25 como máximo).
CASOS=(
  rgba-una-capa capas-normal-tres capas-opacidad capas-alfa-origen capas-alfa-fondo
  mezcla-normal mezcla-multiplicar mezcla-trama mezcla-superponer
  mezcla-oscurecer mezcla-aclarar mezcla-sumar mezcla-diferencia
  mezcla-ocho-opacidad mezcla-ocho-alfa mezcla-ocho-fondo-translucido
  indexado-capas indexado-transparente-7 indexado-paleta-256 indexado-celdas
  celda-desplazada celda-recortada celda-vacia capas-ocultas mixto-rgba
)

falta() { echo "falta $1" >&2; exit 1; }
command -v "$ASEPRITE" >/dev/null || falta "$ASEPRITE (pasa ASEPRITE=/ruta/al/programa)"
command -v python3 >/dev/null || falta python3
python3 -I -c 'import PIL' 2>/dev/null || falta "Pillow (python3 -I -c 'import PIL')"

# SHA-256 de la RGBA de 8 bits de un PNG (alfa directo, filas de arriba abajo, sin cabecera).
hash_rgba() {
  python3 -I -c '
import hashlib, sys
from PIL import Image
print(hashlib.sha256(Image.open(sys.argv[1]).convert("RGBA").tobytes()).hexdigest())
' "$1"
}

if [[ "${1:-}" == "--prueba" ]]; then
  # No es /tmp (tmpfs): carpeta en disco, borrada al terminar.
  PRUEBA="${XDG_CACHE_HOME:-$HOME/.cache}/pixoko-corpus-h3-prueba"
  rm -rf "$PRUEBA"; mkdir -p "$PRUEBA"
  echo "== versión"; "$ASEPRITE" --version
  echo "== script de Lua por lotes"
  "$ASEPRITE" -b --script "$RAIZ/tools/corpus-h3/comprobar.lua" --script-param "salida=$PRUEBA"
  [[ -s "$PRUEBA/humo.aseprite" ]] || falta "humo.aseprite (el script no lo escribió)"
  echo "== exportar PNG aplanado"
  "$ASEPRITE" -b "$PRUEBA/humo.aseprite" --save-as "$PRUEBA/humo.png"
  python3 -I - "$PRUEBA/humo.png" <<'PY'
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGBA")
assert im.size == (4, 4), im.size
assert im.getpixel((0, 0)) == (196, 46, 52, 255), im.getpixel((0, 0))
assert im.getpixel((3, 3)) == (196, 46, 52, 255), im.getpixel((3, 3))
print("centro (1,1):", im.getpixel((1, 1)), "(esperado: azul con alfa 128 sobre rojo, alfa 255)")
assert im.getpixel((1, 1))[3] == 255
print("prueba mínima correcta")
PY
  rm -rf "$PRUEBA"
  exit 0
fi

if [[ "${1:-}" == "--verificar" ]]; then
  for f in "$SALIDA"/*.aseprite; do
    "$ASEPRITE" -b "$f" --script "$RAIZ/tools/corpus-h3/verificar.lua"
  done
  exit 0
fi

if [[ $# -gt 0 ]]; then CASOS=("$@"); fi
mkdir -p "$SALIDA"

for caso in "${CASOS[@]}"; do
  ase="$SALIDA/$caso.aseprite"; png="$SALIDA/$caso.png"
  rm -f "$ase" "$png" "$png.sha256"
  "$ASEPRITE" -b --script "$LUA" --script-param "caso=$caso" --script-param "salida=$SALIDA"
  [[ -s "$ase" ]] || falta "$caso.aseprite (el script no lo escribió; mira el error de arriba)"
  "$ASEPRITE" -b "$ase" --save-as "$png"
  [[ -s "$png" ]] || falta "$caso.png (no se exportó)"
  hash_rgba "$png" > "$png.sha256"
  echo "$caso: $(cat "$png.sha256")"
done

# README.md: versión y fecha reales de esta ejecución.
VERSION="$("$ASEPRITE" --version | head -n 1)"
FECHA="$(date +%F)"
TODOS=("$SALIDA"/*.aseprite)
{
  cat <<EOF
# golden/aseprite

Corpus del hito 3 (\`specs/corpus-hito-3.md\`): capas, opacidad, modos de mezcla, celdas con
desplazamiento y con recorte, capas ocultas, paleta y canal alfa, en ${#TODOS[@]} casos.

- **Programa:** $VERSION
- **Fecha:** $FECHA
- **Cómo se generó:** \`tools/corpus-h3/generar.sh\` (MIT, en este repositorio): un script de Lua
  propio crea cada caso desde cero y guarda el \`.aseprite\`; el PNG aplanado sale de abrir ese
  fichero y exportarlo con el modo por lotes del programa.
- **Origen de las imágenes:** son dibujos propios de prueba (rectángulos, discos y diagonales con
  colores definidos en el script). No se copió ningún recurso del programa: ni la paleta por
  defecto, ni imágenes, ni ficheros de ejemplo. Se publican bajo la licencia MIT del repositorio.
- **Hash:** cada \`X.png\` lleva al lado \`X.png.sha256\`: SHA-256 de la imagen expandida a RGBA de
  8 bits (alfa directo, sin cabecera), calculado con Pillow, como en \`golden/png/\`.

| Fichero | PNG aplanado |
| --- | --- |
EOF
  for f in "${TODOS[@]}"; do n="$(basename "$f" .aseprite)"; echo "| $n.aseprite | $n.png |"; done
} > "$SALIDA/README.md"
echo "listo: $SALIDA (revisa y commitea a mano)"
