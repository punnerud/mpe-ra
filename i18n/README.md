# Ordbok / oversettelser (i18n)

Kilden er **CSV** (lett å redigere i regneark / Numbers / Excel). Rust-koden
`src/i18n.rs` er **auto-generert** — ikke rediger den for hånd.

## Filer

- `strings.csv` — én rad per tekstnøkkel. Kolonner: `key, context, en, no, sv, …`
  (én kolonne per språk-ISO-kode). **Engelsk (`en`) er kilde/standard** og må
  være utfylt. Tom celle for et språk → faller automatisk tilbake til engelsk.
- `langs.csv` — språklisten og rekkefølgen i flaggvelgeren. Kolonner:
  `lang, fn, iso, flag, native, english`.
- `generate.py` — leser CSV-ene og skriver `src/i18n.rs`.
- `merge.py` — (valgfritt) fletter `incoming/*.json` (`{"iso":{"Key":"…"}}`) inn
  i `strings.csv`. Nyttig når oversettelser kommer som JSON.

## Arbeidsflyt

```bash
# 1) rediger strings.csv (legg til/endre tekster)
# 2) generer Rust:
python3 i18n/generate.py
# 3) bygg:
cargo test && ./build-web.sh
```

`generate.py` skriver ut dekning per språk (X/Y nøkler oversatt) — slik ser du
lett hva som gjenstår. `cargo test i18n_dekning` gjør det samme fra Rust.

## Legge til et nytt språk

1. Legg en rad i `langs.csv` (velg ISO-kode, flagg-emoji, navn).
2. Legg en kolonne med samme ISO-kode i `strings.csv` og fyll inn.
3. Legg språket i `LANGS`-arrayet i `web/bridge.js` (samme rekkefølge!).
4. `python3 i18n/generate.py && ./build-web.sh`

## Legge til en ny tekst

1. Legg en rad i `strings.csv` med en ny `key` + engelsk tekst.
2. Bruk `Key::DenNyeNokkelen` i `src/main.rs` via `self.t(Key::…)`.
3. Regenerer.

All tekst i spillet tegnes med `web/font.ttf` (Arial Unicode) som dekker latin,
gresk, kyrillisk, CJK, arabisk, hebraisk, indiske skrifter m.m.
