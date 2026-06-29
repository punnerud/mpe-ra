#!/usr/bin/env python3
"""Bygg en LITEN web/font.ttf som kun inneholder tegnene spillet faktisk bruker.

Spillet viser et fast sett tekster (i18n/strings.csv) + tall/ASCII. Vi subsetter
en Unicode-font ned til nøyaktig disse glyfene, så én liten fil dekker ALLE språk
(inkl. CJK/arabisk/gresk) i stedet for en 23 MB fullfont.

Krever fonttools:  pip install fonttools   (eller: brew install fonttools)

Kjør etter endringer i strings.csv:
    python3 i18n/build_font.py
"""
import csv
import os
import string
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STRINGS = os.path.join(ROOT, "i18n", "strings.csv")
OUT = os.path.join(ROOT, "web", "font.ttf")

# Kilde-font med bred Unicode-dekning. Bytt sti om du er på et annet system.
SRC_CANDIDATES = [
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
]


def main():
    src = next((p for p in SRC_CANDIDATES if os.path.exists(p)), None)
    if not src:
        raise SystemExit("Fant ingen kilde-font. Sett SRC_CANDIDATES i build_font.py.")

    chars = set()
    with open(STRINGS, encoding="utf-8") as f:
        r = csv.reader(f)
        next(r)
        for row in r:
            for cell in row[2:]:  # hopp over key + context
                chars.update(cell)
    # ASCII printable (tall, $, bokstaver i kode-strenger: FPS, [PAUSE], WASD, x ...)
    chars.update(string.printable)
    chars = {c for c in chars if c.isprintable() or c == " "}

    txt = os.path.join(ROOT, "i18n", ".font_chars.txt")
    with open(txt, "w", encoding="utf-8") as f:
        f.write("".join(sorted(chars)))

    subprocess.run([
        "pyftsubset", src,
        "--text-file=" + txt,
        "--output-file=" + OUT,
        "--layout-features=*",
        "--no-hinting",
        "--desubroutinize",
    ], check=True)
    os.remove(txt)
    print("Skrev %s (%d tegn, %d bytes)" % (OUT, len(chars), os.path.getsize(OUT)))


if __name__ == "__main__":
    main()
