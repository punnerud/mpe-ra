# OpenRA Rust

En liten **sanntidsstrategi-grunnmur (RTS)** i OpenRA-ånd, skrevet i **Rust** og
spillbar i nettleseren via **WebAssembly + WebGL2** (rendret med
[`macroquad`](https://macroquad.rs)). Kjører også nativt på desktop.

> ⚠️ Dette er **ikke** en port av OpenRA (som er hundretusenvis av linjer C#).
> Det er en kompakt, fungerende RTS-kjerne som demonstrerer terreng, enheter,
> seleksjon, kommandering og kamp i nettleseren med WebGL — et godt
> utgangspunkt å bygge videre på.

## Hva som er med

- Rutebasert terreng (gress, malm, vann, fjell) — vann/fjell er ufremkommelig
- To lag (blå spiller vs. rød fiende) med enheter
- Boks-seleksjon, enkeltklikk-valg og flyttkommandoer
- Automatisk kamp innen rekkevidde, med HP-barer og skudd-effekter
- Enkel kollisjon mellom enheter og en lett fiende-AI
- Kamera: panorering (WASD/piltaster/skjermkant) og zoom (musehjul)
- Seier/nederlag-tilstand med omstart (R)

## Styring

| Handling | Tast / mus |
|---|---|
| Velg enheter (boks) | Hold venstre museknapp og dra |
| Velg enkeltenhet | Venstreklikk |
| Flytt valgte enheter | Høyreklikk |
| Panorer kamera | WASD / piltaster / skjermkant |
| Zoom | Musehjul |
| Start på nytt | R |

## Kjør i nettleser (WebGL)

Krever Rust-target `wasm32-unknown-unknown`:

```bash
rustup target add wasm32-unknown-unknown
./build-web.sh
cd web && python3 -m http.server 8080
```

Åpne så <http://localhost:8080>.

> Du **må** servere via en webserver (ikke `file://`) — nettlesere nekter å
> laste `.wasm` direkte fra filsystemet.

## Kjør nativt (desktop)

```bash
cargo run --release
```

## Hvordan WebGL-byggingen fungerer

- `macroquad` kompilerer til `wasm32-unknown-unknown` og tegner via WebGL2.
- `web/index.html` laster `mq_js_bundle.js` (miniquad sin loader) som kobler
  WASM mot nettleserens WebGL-kontekst og kaller `load("openrarust.wasm")`.
- GL-funksjonene (`glGenTextures`, …) leveres av JS i runtime, så
  `.cargo/config.toml` setter `--allow-undefined` for å unngå lenkefeil.

## Filstruktur

```
Cargo.toml            # avhengigheter (macroquad) + release-profil
.cargo/config.toml    # wasm linker-flagg (--allow-undefined)
src/main.rs           # hele spillet (terreng, enheter, input, kamp, tegning)
web/index.html        # nettleser-loader
web/openrarust.wasm   # bygget artefakt (genereres av build-web.sh)
build-web.sh          # bygger wasm og kopierer til web/
```

## Mulige neste steg

- Bygninger og produksjonskø (raffineri, fabrikk)
- Malm-høsting og økonomi
- A*-pathfinding rundt hindringer (nå stopper enheter ved blokkert rute)
- Tåke/utforskning (fog of war)
- Sprites/teksturer i stedet for primitiver
- Nettverksspill
```
