#!/usr/bin/env python3
"""Flett innkommende oversettelser (i18n/incoming/*.json) inn i strings.csv.

Hver JSON-fil har formen {"iso": {"Key": "tekst", ...}, ...}. Kjor:

    python3 i18n/merge.py
    python3 i18n/generate.py

Tomme verdier overskriver ikke eksisterende. Ukjente nokler/sprak ignoreres
med en advarsel.
"""
import csv
import glob
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STRINGS = os.path.join(ROOT, "i18n", "strings.csv")
INCOMING = os.path.join(ROOT, "i18n", "incoming")


def main():
    with open(STRINGS, encoding="utf-8") as f:
        rows = list(csv.reader(f))
    header = rows[0]
    isos = header[2:]
    iso_col = {iso: 2 + i for i, iso in enumerate(isos)}
    key_row = {r[0]: r for r in rows[1:]}

    updated = 0
    for path in sorted(glob.glob(os.path.join(INCOMING, "*.json"))):
        data = json.load(open(path, encoding="utf-8"))
        for iso, kv in data.items():
            if iso not in iso_col:
                print("ADVARSEL: ukjent sprak", iso, "i", os.path.basename(path))
                continue
            col = iso_col[iso]
            for k, v in kv.items():
                if k not in key_row:
                    print("ADVARSEL: ukjent nokkel", k, "(", iso, ")")
                    continue
                if v and v.strip():
                    key_row[k][col] = v
                    updated += 1

    with open(STRINGS, "w", newline="", encoding="utf-8") as f:
        wt = csv.writer(f)
        wt.writerow(header)
        for r in rows[1:]:
            wt.writerow(r)

    print("Flettet", updated, "celler inn i strings.csv")


if __name__ == "__main__":
    main()
