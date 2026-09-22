# ◈ SLAY — hex strategy remake

A faithful remake of Sean O'Connor's 1995 turn-based classic **Slay**: buy peasants,
combine them into barons, and bankrupt your enemies by cutting their empires in half.

![icon](src-tauri/icons/icon.png)

## Play

Download for your system from [Releases](../../releases):

- **macOS**: `Slay_*.dmg` → drag to Applications, double-click.
  First launch may ask for right-click → Open (unsigned indie build).
- **Windows**: run the `*-setup.exe` installer (unsigned: SmartScreen → *More info → Run anyway*).
- **Linux / Omarchy**: download the `.AppImage`, `chmod +x`, double-click — runs
  anywhere including Arch-based distros (a `.deb` is attached too for Debian/Ubuntu).
  If `~/.config/omarchy` exists, the app offers your active Omarchy theme in-game.

## Rules (the real ones)

- Every connected region you own is a **territory** with its own treasury.
- Each turn a territory earns **1 gold per clear hex**, then pays wages:
  **♟ 2 · ♝ 6 · ♞ 18 · ♚ 54**. Can't pay? **Every man starves** into graves, then pines.
- Buy **X peasant (10g)** or **C castle (15g, defends 2, no wages)** — only peasants are
  sold. Stack two men to combine (1+1=♝, 1+2=♞, max ♚).
- Attacks must be **strictly stronger** than the defence. Men, castles (2), houses (1)
  and graves (1) each guard their hex **plus every neighbouring own hex**.
- ⌂ houses hold their territory's gold (**gold** = can buy, grey = broke).
  Destroy one and its treasury is lost.
- **Cut a foe in two**: the half without the house starts at 0 gold and starves.
- Win by conquering the whole island.

## Controls

| Input | Action |
|---|---|
| Click / Enter | pick up a man · order it to a glowing hex |
| X / C | recruit peasant (10g) / build castle (15g) |
| Space / E | end turn |
| H / N / M | help · new island · sound on/off |

Gold hex = picked man · green edge = legal target · pulsing ring = territory outline.

## Stack

- **Engine**: Rust (`src-tauri/src/game.rs`) — hex grid, flood-fill territories, economy,
  cut-hunting AI with choke-point detection. `cargo test` (8 tests).
- **Shell**: [Tauri 2](https://tauri.app) — one Rust backend, one web frontend, zero bundler.
- **UI**: vanilla HTML/CSS/JS (`src/`) — true polygon hexagons, 22 real Omarchy palettes
  baked in (`src/omarchy-themes.json`), WebAudio synth SFX, no assets besides the icon.

## Develop

```bash
npm install
npm run tauri dev        # UI hot-reloads against src/
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build      # → Slay.app + .dmg (macOS) / bundles (Linux)
```

Releases are built by CI ([`release.yml`](.github/workflows/release.yml)) on every
`v*` tag: Windows, Linux (AppImage + deb) and macOS binaries all attach automatically.
**iOS** (free Apple ID, no paid dev account): `npm run tauri ios init`, open the
Xcode project, pick your Personal Team, run on your device. Re-sign weekly.

Icon: handmade, `Graphic_Design/Handmade Icons/slay icon.png` (not in repo).

## Credits

Game design: Sean O'Connor's *Slay* (1995). This is an unofficial fan remake for
learning and play — no affiliation. If you love it, buy the original on Steam.
