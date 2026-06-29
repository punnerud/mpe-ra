#!/usr/bin/env python3
"""Generer src/i18n.rs fra CSV-ordboken.

Kilder:
  - i18n/langs.csv    : kolonner lang,fn,iso,flag,native,english (rekkefolge = flaggvelger)
  - i18n/strings.csv  : kolonner key,context,<iso1>,<iso2>,...  (en rad pr. tekstnokkel)

Engelsk (en) MA ha alle nokler. For andre sprak: tom celle -> "" i Rust ->
engelsk fallback i spillet. Kjor:

    python3 i18n/merge.py       # (valgfritt) flett inn i18n/incoming/*.json
    python3 i18n/generate.py    # skriv src/i18n.rs

Skriptet rapporterer dekning og hva som gjenstar pr. sprak.
"""
import csv
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LANGS_CSV = os.path.join(ROOT, "i18n", "langs.csv")
STRINGS_CSV = os.path.join(ROOT, "i18n", "strings.csv")
OUT = os.path.join(ROOT, "src", "i18n.rs")


def rs_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main():
    with open(LANGS_CSV, encoding="utf-8") as f:
        langs = list(csv.DictReader(f))
    with open(STRINGS_CSV, encoding="utf-8") as f:
        rows = list(csv.reader(f))
    header = rows[0]
    isos = header[2:]
    keys = [r[0] for r in rows[1:]]
    # tr[iso][key] = tekst
    tr = {iso: {} for iso in isos}
    for r in rows[1:]:
        for i, iso in enumerate(isos):
            tr[iso][r[0]] = r[2 + i] if 2 + i < len(r) else ""

    # Validering: engelsk komplett
    missing_en = [k for k in keys if not tr.get("en", {}).get(k)]
    if missing_en:
        print("FEIL: engelsk mangler nokler:", missing_en, file=sys.stderr)
        sys.exit(1)

    o = []
    w = o.append
    w("//! AUTO-GENERERT av i18n/generate.py fra i18n/langs.csv + i18n/strings.csv.")
    w("//! IKKE rediger for hand -- endre CSV-ene og kjor generatoren pa nytt.")
    w("//!")
    w("//! Engelsk (`Lang::En`) er kilde/standard. Tom oversettelse -> engelsk")
    w("//! fallback. `cargo test i18n_dekning` viser hva som gjenstar.")
    w("")
    w("#![allow(dead_code)]")
    w("")
    w("#[derive(Clone, Copy, PartialEq, Eq, Debug)]")
    w("pub enum Lang {")
    w("    " + ", ".join(l["lang"] for l in langs) + ",")
    w("}")
    w("")
    w("/// (sprak, ISO-kode, flagg-emoji, eget navn, engelsk navn). Rekkefolgen =")
    w("/// flaggvelgeren og MA matche LANGS-listen i web/bridge.js.")
    w("pub const LANGS: &[(Lang, &str, &str, &str, &str)] = &[")
    for l in langs:
        w("    (Lang::%s, %s, %s, %s, %s)," % (
            l["lang"], rs_str(l["iso"]), rs_str(l["flag"]),
            rs_str(l["native"]), rs_str(l["english"])))
    w("];")
    w("")
    w("pub fn from_index(i: usize) -> Lang {")
    w("    LANGS.get(i).map(|r| r.0).unwrap_or(Lang::En)")
    w("}")
    w("")
    w("pub fn index_of(lang: Lang) -> usize {")
    w("    LANGS.iter().position(|r| r.0 == lang).unwrap_or(0)")
    w("}")
    w("")
    w("#[derive(Clone, Copy, PartialEq, Eq, Debug)]")
    w("pub enum Key {")
    for k in keys:
        w("    %s," % k)
    w("}")
    w("")
    w("pub const ALL_KEYS: &[Key] = &[")
    for k in keys:
        w("    Key::%s," % k)
    w("];")
    w("")
    w("/// Hovedoppslag: oversettelse for (sprak, nokkel) med engelsk fallback.")
    w("pub fn t(lang: Lang, key: Key) -> &'static str {")
    w("    let s = match lang {")
    for l in langs:
        if l["fn"] == "en":
            w('        Lang::En => "",')
        else:
            w("        Lang::%s => %s(key)," % (l["lang"], l["fn"]))
    w("    };")
    w("    if s.is_empty() { en(key) } else { s }")
    w("}")
    w("")
    w("/// Raa oversettelse uten fallback (for dekningstellingen).")
    w("pub fn raw(lang: Lang, key: Key) -> &'static str {")
    w("    match lang {")
    for l in langs:
        w("        Lang::%s => %s(key)," % (l["lang"], l["fn"]))
    w("    }")
    w("}")
    w("")
    for l in langs:
        iso = l["iso"]
        m = tr.get(iso, {})
        w("fn %s(k: Key) -> &'static str {" % l["fn"])
        w("    match k {")
        for k in keys:
            w("        Key::%s => %s," % (k, rs_str(m.get(k, "") or "")))
        w("    }")
        w("}")
        w("")
    # tester
    w("#[cfg(test)]")
    w("mod tests {")
    w("    use super::*;")
    w("")
    w("    #[test]")
    w("    fn engelsk_er_komplett() {")
    w("        for &k in ALL_KEYS { assert!(!en(k).is_empty(), \"en mangler {:?}\", k); }")
    w("    }")
    w("")
    w("    #[test]")
    w("    fn norsk_er_komplett() {")
    w("        for &k in ALL_KEYS { assert!(!no(k).is_empty(), \"no mangler {:?}\", k); }")
    w("    }")
    w("")
    w("    #[test]")
    w("    fn fallback_gir_alltid_ikke_tom() {")
    w("        for &(lang, _i, _f, _n, _e) in LANGS {")
    w("            for &k in ALL_KEYS { assert!(!t(lang, k).is_empty(), \"{:?} {:?} tom\", lang, k); }")
    w("        }")
    w("    }")
    w("")
    w("    #[test]")
    w("    fn i18n_dekning() {")
    w("        for &(lang, iso, _f, _n, _e) in LANGS {")
    w("            let dekket = ALL_KEYS.iter()")
    w("                .filter(|&&k| lang == Lang::En || !raw(lang, k).is_empty()).count();")
    w("            let total = ALL_KEYS.len();")
    w("            println!(\"[i18n] {:<6} {}/{} oversatt\", iso, dekket, total);")
    w("            if dekket < total {")
    w("                let mangler: Vec<&Key> = ALL_KEYS.iter()")
    w("                    .filter(|&&k| lang != Lang::En && raw(lang, k).is_empty()).collect();")
    w("                println!(\"        mangler: {:?}\", mangler);")
    w("            }")
    w("        }")
    w("    }")
    w("}")
    w("")

    with open(OUT, "w", encoding="utf-8") as f:
        f.write("\n".join(o))

    # --- JS-tabell for dev-meny / HTML (web/i18n.js) ---
    import json
    OUT_JS = os.path.join(ROOT, "web", "i18n.js")
    maps = []
    for l in langs:
        iso = l["iso"]
        if iso == "en":
            maps.append({k: tr["en"][k] for k in keys})  # engelsk = komplett
        else:
            maps.append({k: tr[iso][k] for k in keys if tr.get(iso, {}).get(k)})
    js = (
        "// AUTO-GENERERT av i18n/generate.py -- ikke rediger for hand.\n"
        "// Oversettelser for dev-meny/HTML. Indeks = sprak (samme som LANGS).\n"
        "window.I18N_T = " + json.dumps(maps, ensure_ascii=False) + ";\n"
        "window.I18Nt = function (i, k) {\n"
        "  var L = window.I18N_T; var m = L[i] || {}; var v = m[k];\n"
        "  return (v && v.length) ? v : ((L[0] || {})[k] || k);\n"
        "};\n"
    )
    with open(OUT_JS, "w", encoding="utf-8") as f:
        f.write(js)
    print("Skrev", OUT_JS)

    # rapport
    print("Skrev", OUT)
    total = len(keys)
    for l in langs:
        iso = l["iso"]
        have = sum(1 for k in keys if tr.get(iso, {}).get(k))
        mark = "OK " if (iso == "en" or have == total) else "!! "
        print("  %s %-7s %d/%d" % (mark, iso, have, total))


if __name__ == "__main__":
    main()
