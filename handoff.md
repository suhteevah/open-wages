# HANDOFF.md — Open Wages → Claude Code Session

## TL;DR
You're picking up a clean-room Rust reimplementation of *Wages of War: The Business of Battle* (1996). The scaffold is built, skills are written, Cargo workspace compiles. **Your first job is Phase 1: get the original game files and run the survey tool to classify every file.**

## What's Done
- [x] Full Cargo workspace scaffold (5 crates: ow-data, ow-core, ow-render, ow-audio, ow-app)
- [x] CLAUDE.md with full project rules, tech stack, conventions
- [x] README.md
- [x] Three `.skill` files with deep reference material:
  - `skills/win95-binary-re.skill` — PE32 reverse engineering workflow (Ghidra, x32dbg, import tracing)
  - `skills/game-dat-parser.skill` — .dat file triage, text/binary parsers, survey tool reference, format doc templates
  - `skills/isometric-engine.skill` — Full isometric engine architecture, combat system, pathfinding, renderer
- [x] Rust survey tool at `crates/ow-tools` (`cargo run -p ow-tools --bin survey`) — classify all files in game directory
- [x] Rust triage tool at `crates/ow-tools` (`cargo run -p ow-tools --bin triage`) — deep-inspect individual files
- [x] Stub lib.rs in all crates
- [x] Workspace Cargo.toml with shared dependencies

## What's NOT Done — Immediate Next Steps

### Phase 1: Data Reconnaissance (DO THIS FIRST)
1. **Obtain the game files.** The ISO is on Archive.org: https://archive.org/details/wages-of-war-the-business-of-battle
   - Download the ISO
   - Extract/mount it
   - The installer is 16-bit — either use `otvdm`/`winevdm` or just extract files with 7-Zip
   - The game .exe is 32-bit PE even though the installer is 16-bit
2. **Run the survey tool:**
   ```powershell
   cargo run -p ow-tools --bin survey -- "C:\Games\WagesOfWar"
   ```
   This classifies every file as text/binary/mixed and dumps `file_survey.json`.
3. **For each .dat file identified as text:** Open it, document the schema in `docs/FORMAT_<name>.md`
4. **For each binary file:** Run `cargo run -p ow-tools --bin triage -- <file>`, note magic bytes, struct patterns, file sizes
5. **Run `strings` on the main .exe:** `strings wow.exe > docs/exe_strings.txt` — this reveals filenames, error messages, format identifiers

### Phase 2: Data Parsers (ow-data crate)
1. Implement text .dat parsers based on documented schemas
2. Implement sprite/BMP loader (likely 256-color indexed with a .pal or embedded palette)
3. Implement map/terrain loader (binary format TBD from Phase 1)
4. Implement data validator (checks all required files present)
5. Write unit tests against known values from the original files

### Phase 3: Core Rules (ow-core crate)
1. Mercenary stat system (loaded from .dat)
2. Weapon/equipment system
3. Initiative-based combat resolution
4. Suppression + morale
5. Pathfinding (A* on isometric grid)
6. Line of sight (Bresenham)
7. Weather effects
8. Economy (contracts, payments, reputation)
9. AI decision trees

### Phase 4: Renderer (ow-render crate)
1. Isometric tile rendering (diamond projection, 64×32 tiles)
2. Sprite rendering with palette
3. Camera (scroll, zoom)
4. HUD/UI panels
5. Animation state machine

### Phase 5: Integration (ow-app)
1. Wire data→core→render
2. Mission flow: deploy→fight→extract
3. Office/strategic layer UI
4. Save/load
5. Sound

## Key Technical Facts
- **The .dat files are plaintext.** Community confirmed: editable with Notepad++. This is huge — the data layer is CSV/INI-style, not packed binary.
- **Engine:** "Random Games 1996-2000 Strategy Engine" per MobyGames
- **Combat is initiative-based**, NOT I-go-you-go. All units sorted by (Experience + Will) each round.
- **Suppression is a core mechanic** — incoming fire reduces AP even on a miss.
- **Weather matters** — affects accuracy, sight range, smoke grenades.
- **The game has a strategic office layer** (hiring, equipment, contracts, intel) AND a tactical mission layer.
- **Isometric diamond projection**, standard 2:1 tile ratio.

## Environment
- **Machine:** kokonoe (i9-11900K, RTX 3070 Ti, 64GB, Win11)
- **Toolchain:** Rust stable
- **IDE:** VS Code / Claude Code
- **RE tools available:** Ghidra 11.x, x32dbg, HxD, PE-bear, Process Monitor, Strings (Sysinternals)
- **SDL2:** Install via vcpkg or pre-built binaries

## File Layout
```
open-wages/
├── CLAUDE.md              # Project soul document — read this first
├── README.md              # Public-facing readme
├── HANDOFF.md             # This file
├── Cargo.toml             # Workspace root
├── crates/
│   ├── ow-core/           # Game logic (combat, economy, AI)
│   ├── ow-data/           # Original file parsers
│   ├── ow-render/         # Isometric renderer
│   ├── ow-audio/          # Sound/music
│   └── ow-app/            # Main binary
├── crates/ow-tools/       # Rust RE helper binaries
│   └── src/bin/
│       ├── survey.rs      # Classify all files in game dir
│       └── triage.rs      # Deep-inspect individual files
├── docs/                  # Format specs, mechanics docs
│   └── architecture.md    # Engine architecture decisions
├── assets/                # Placeholder/test assets only
└── skills/                # Reference .skill files
    ├── win95-binary-re.skill
    ├── game-dat-parser.skill
    └── isometric-engine.skill
```

## GitHub
Repo should be created at `suhteevah/open-wages`. Public from day one — this is an open-source project.
