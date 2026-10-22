"""Posición del mensaje de las aserciones (specs/limpieza-1.md, etapa L3): comparten `l3msgs.py` y `verify.py`."""
from __future__ import annotations

# macro -> número de argumentos antes del mensaje
MACROS = {
    "assert": 1, "assert_eq": 2, "assert_ne": 2, "debug_assert": 1, "debug_assert_eq": 2, "debug_assert_ne": 2,
    "prop_assert": 1, "prop_assert_eq": 2, "prop_assert_ne": 2, "panic": 0, "unreachable": 0, "todo": 0,
    "unimplemented": 0,
}


def message_indices(items: list[tuple[str, str]]) -> list[int]:
    """`items`: lista (clase, texto) sin espacios ni comentarios; devuelve los índices de las cadenas que son el
    mensaje de una macro de MACROS (un solo token de cadena entre comas, en el lugar del mensaje)."""
    out = []
    n = len(items)
    for i in range(n - 2):
        k, t = items[i]
        if k != "id" and k != "ident":
            continue
        if t not in MACROS or items[i + 1][1] != "!" or items[i + 2][1] not in ("(", "["):
            continue
        want = MACROS[t]
        depth, arg, j = 0, 0, i + 2
        while j < n:
            tx = items[j][1]
            kind = items[j][0]
            if tx in ("(", "[", "{") and kind not in ("s", "string"):
                depth += 1
            elif tx in (")", "]", "}") and kind not in ("s", "string"):
                depth -= 1
                if depth == 0:
                    break
            elif tx == "," and depth == 1 and kind not in ("s", "string"):
                arg += 1
            elif depth == 1 and arg == want and kind in ("s", "string"):
                prev = items[j - 1][1]
                nxt = items[j + 1][1] if j + 1 < n else ""
                if prev in ("(", "[", ",") and nxt in (",", ")", "]"):
                    out.append(j)
            j += 1
    return sorted(set(out))
