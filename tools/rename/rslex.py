"""Léxico de Rust para el renombrado (specs/limpieza-1.md, apartado 1).

Parte un fichero en tokens sin perder un solo carácter (`''.join(t.text) == fuente`): identificadores,
comentarios (de línea y de bloque anidados, con su bandera de documentación), cadenas (normales,
crudas, de bytes y C), caracteres, lifetimes, números, signos y espacios. No interpreta la
gramática: solo lo que hace falta para no confundir un identificador con otra cosa.
"""
from __future__ import annotations

from dataclasses import dataclass

IDENT_START = lambda c: c == "_" or c.isalpha()
IDENT_CONT = lambda c: c == "_" or c.isalnum()


@dataclass
class Tok:
    kind: str  # ident | comment | string | char | lifetime | num | punct | ws
    text: str
    start: int

    @property
    def end(self) -> int:
        return self.start + len(self.text)


def lex(src: str) -> list[Tok]:
    out: list[Tok] = []
    i, n = 0, len(src)

    def add(kind: str, j: int) -> None:
        nonlocal i
        out.append(Tok(kind, src[i:j], i))
        i = j

    while i < n:
        c = src[i]
        if c.isspace():
            j = i
            while j < n and src[j].isspace():
                j += 1
            add("ws", j)
        elif src.startswith("//", i):
            j = src.find("\n", i)
            add("comment", n if j < 0 else j)
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            add("comment", j)
        elif c.isdigit():
            j = i
            while j < n and (IDENT_CONT(src[j])):
                j += 1
            # parte decimal: `1.5`, no `1..2` ni `1.max(2)`
            if j + 1 < n and src[j] == "." and src[j + 1].isdigit():
                j += 1
                while j < n and IDENT_CONT(src[j]):
                    j += 1
            add("num", j)
        elif c == '"' or (c in "bc" and src.startswith('"', i + 1)):
            j = i + (1 if c == '"' else 2)
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            add("string", min(j + 1, n))
        elif c in "rbc" and _raw_start(src, i) is not None:
            hashes, q = _raw_start(src, i)
            close = '"' + "#" * hashes
            j = src.find(close, q + 1)
            add("string", n if j < 0 else j + len(close))
        elif c == "'":
            # carácter o lifetime
            if i + 1 < n and src[i + 1] == "\\":
                j = i + 2
                while j < n and src[j] != "'":
                    j += 2 if src[j] == "\\" else 1
                add("char", min(j + 1, n))
            elif i + 2 < n and src[i + 2] == "'" :
                add("char", i + 3)
            elif i + 1 < n and IDENT_START(src[i + 1]):
                j = i + 1
                while j < n and IDENT_CONT(src[j]):
                    j += 1
                add("lifetime", j)
            else:
                add("char", min(i + 3, n))
        elif c == "b" and src.startswith("b'", i):
            j = i + 2
            if j < n and src[j] == "\\":
                j += 2
            else:
                j += 1
            while j < n and src[j] != "'":
                j += 1
            add("char", min(j + 1, n))
        elif IDENT_START(c):
            j = i
            if src.startswith("r#", i) and i + 2 < n and IDENT_START(src[i + 2]):
                j = i + 2
            while j < n and IDENT_CONT(src[j]):
                j += 1
            add("ident", j)
        else:
            add("punct", i + 1)
    return out


def _raw_start(src: str, i: int):
    """`r"`, `r#"`, `br"`, `cr#"`…: (nº de #, posición de la comilla) o None."""
    j = i
    if src[j] in "bc":
        j += 1
    if j >= len(src) or src[j] != "r":
        return None
    j += 1
    h = 0
    while j < len(src) and src[j] == "#":
        h += 1
        j += 1
    if j < len(src) and src[j] == '"':
        return h, j
    return None


def ident_name(tok: Tok) -> str:
    """El nombre sin el prefijo `r#`."""
    return tok.text[2:] if tok.text.startswith("r#") else tok.text


if __name__ == "__main__":  # autocomprobación: el léxico no pierde caracteres
    import sys, pathlib

    bad = 0
    for p in sys.argv[1:]:
        s = pathlib.Path(p).read_text(encoding="utf-8")
        if "".join(t.text for t in lex(s)) != s:
            bad += 1
            print("PIERDE", p)
    print("ok" if not bad else f"{bad} con fallos")
