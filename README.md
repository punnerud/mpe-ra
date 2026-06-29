# MPE-RA

*Morten Punnerud-Engelstad RA* — a small real-time-strategy game in the spirit of
Red Alert / OpenRA, written in **Rust** with [macroquad](https://macroquad.rs/)
and compiled to **WebAssembly** so it runs in the browser, on desktop, and on
mobile (iPhone/Android). All UI (joystick, zoom, build menu, dev menu, language
picker, campaign menus) is drawn in Rust — the web page is just a thin shell.

## ▶ Play in the browser

**https://punnerud.github.io/mpe-ra/**

Works on desktop and on phones (touch). Pan with the on-screen joystick or by
pushing the screen edges; zoom with `+` / `-`; open the build menu with the `≡`
button.

## Features

- 100-level campaign with hand-designed maps, gradually increasing difficulty,
  water/terrain variation and up to two enemy bases with distinct styles
  (Balanced / Armor / Swarm).
- Build economy (harvesters → refinery → credits), units (Rifleman, Tank,
  Harvester) and buildings (Refinery, Factory, walls, defenses).
- Start screen with level select (levels unlock as you win), a language-aware
  how-to-play guide, and 40+ UI languages.
- Win without cheating to record your completion time as the level score.

## How to play

- **Goal:** destroy every enemy HQ while protecting your own.
- **Build:** open the menu (`≡`) to train units and place buildings.
- **Economy:** harvesters gather ore and bring it to the refinery for credits.
- **Move:** drag to select units, tap the map to move them.
- **Navigate:** use the joystick or push the screen edges to pan.
- **Rally:** select the factory and tap the map to set where new units gather.

## Build from source

Requires the Rust toolchain.

```sh
# Native (desktop)
cargo run --release

# Run the tests
cargo test

# WebAssembly build (output copied into web/)
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/openrarust.wasm web/openrarust.wasm

# Serve locally (any static server), then open http://localhost:8088
python3 -m http.server 8088 --directory web
```

The web build is published automatically to GitHub Pages from `web/` via the
workflow in `.github/workflows/pages.yml` on every push to `main`.

## Internationalization

Translations live in `i18n/strings.csv`. Regenerate the Rust/JS tables with:

```sh
python3 i18n/generate.py
```

## License

[PolyForm Noncommercial License 1.0.0](LICENSE.md).

You may use, modify and share this for **any noncommercial purpose**.
**Commercial use by others is not permitted.** The copyright holder,
Morten Punnerud-Engelstad, retains all rights, including commercial use.
