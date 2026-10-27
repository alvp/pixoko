#!/usr/bin/env bash
# Corpus de las formas (specs/hito-4-formas.md, «Referencia»): genera en golden/aseprite-shapes/ los
# 30 PNG de referencia con Aseprite por lotes.
#
# Uso:
#   tools/corpus-h4/generate-shapes.sh --prueba       paso 0: versión y modo por lotes, sin tocar golden/
#   tools/corpus-h4/generate-shapes.sh                genera los 30 ficheros (no pisa los que ya existen)
#   tools/corpus-h4/generate-shapes.sh --sobrescribir lo mismo, pisando lo que haya
# Variables: ASEPRITE (orden del programa, por defecto `aseprite`) y SALIDA (por defecto
# golden/aseprite-shapes). Los resultados no se commitean solos: se revisan y se commitean a mano,
# y entonces se quita el `#[ignore]` de crates/px-golden/tests/shapes.rs.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASEPRITE="${ASEPRITE:-aseprite}"
SALIDA="${SALIDA:-$RAIZ/golden/aseprite-shapes}"
LUA="$RAIZ/tools/corpus-h4/shapes.lua"

if [[ "${1:-}" == "--prueba" ]]; then
  "$ASEPRITE" --version
  "$ASEPRITE" -b --script-param salida=/dev/null --script <(echo 'print("modo por lotes: ok")')
  exit 0
fi

PRUEBA="$(mktemp -d "${HOME}/.cache/pixoko-corpus-h4-shapes.XXXXXX")"
trap 'rm -rf "$PRUEBA"' EXIT
mkdir -p "$SALIDA"
"$ASEPRITE" -b --script-param salida="$PRUEBA" --script "$LUA"

total=0
for f in "$PRUEBA"/*.png; do
  nombre="$(basename "$f")"
  total=$((total + 1))
  if [[ -e "$SALIDA/$nombre" && "${1:-}" != "--sobrescribir" ]]; then
    if cmp -s "$f" "$SALIDA/$nombre"; then echo "IGUAL    $nombre"; else echo "DISTINTO $nombre (no se pisa; usa --sobrescribir)"; fi
  else
    cp "$f" "$SALIDA/$nombre"
    echo "NUEVO    $nombre"
  fi
done
echo "$total ficheros (se esperan 30: 2 rectángulos, 10 círculos, 10 elipses y 8 polígonos)"
