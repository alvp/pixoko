#!/usr/bin/env bash
# Corpus del relleno (specs/hito-4-relleno.md, «Referencia»): genera en golden/aseprite-fill/ los 6
# PNG de referencia con Aseprite por lotes, a partir de golden/aseprite-fill/cases.txt.
#
# Uso:
#   tools/corpus-h4/generate-fill.sh --prueba       paso 0: versión y modo por lotes, sin tocar golden/
#   tools/corpus-h4/generate-fill.sh                genera los 6 ficheros (no pisa los que ya existen)
#   tools/corpus-h4/generate-fill.sh --sobrescribir lo mismo, pisando lo que haya
# Variables: ASEPRITE (orden del programa, por defecto `aseprite`) y SALIDA (por defecto
# golden/aseprite-fill). Los resultados no se commitean solos: se revisan y se commitean a mano,
# y entonces se quita el `#[ignore]` de la prueba `matches_aseprite` de
# crates/px-golden/tests/fill_reference.rs.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASEPRITE="${ASEPRITE:-aseprite}"
SALIDA="${SALIDA:-$RAIZ/golden/aseprite-fill}"
LUA="$RAIZ/tools/corpus-h4/fill.lua"
CASOS="$RAIZ/golden/aseprite-fill/cases.txt"

if [[ "${1:-}" == "--prueba" ]]; then
  "$ASEPRITE" --version
  "$ASEPRITE" -b --script-param salida=/dev/null --script <(echo 'print("modo por lotes: ok")')
  exit 0
fi

PRUEBA="$(mktemp -d "${HOME}/.cache/pixoko-corpus-h4-fill.XXXXXX")"
trap 'rm -rf "$PRUEBA"' EXIT
mkdir -p "$SALIDA"
"$ASEPRITE" -b --script-param salida="$PRUEBA" --script-param casos="$CASOS" --script "$LUA"

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
echo "$total ficheros (se esperan 6: uniform, ring, diagonal, stripes, gap e islands)"
