# Architecture Decisions

## Why Rust?
- Memory safety without GC — critical for a game engine
- Excellent cross-platform support (Windows, Linux, macOS)
- Cargo workspace gives us clean crate separation
- `tracing` ecosystem is the best structured logging available
- Performance on par with the C++ engines of the era, with modern ergonomics

## Why SDL2 (not wgpu)?
- SDL2 is battle-tested for 2D sprite games
- Much simpler setup for isometric tile rendering
- Handles windowing, input, audio in one package
- Can migrate to wgpu later if GPU-accelerated effects are needed
- OpenXCOM also uses SDL — proven for this exact use case

## Crate Separation: Why?
- **ow-data** has zero game logic — purely I/O and parsing. Can be tested against file fixtures without running the game.
- **ow-core** has zero rendering deps — game rules are testable as pure functions. Combat resolution can be unit tested deterministically with seeded RNG.
- **ow-render** depends on ow-core for state but never mutates it — rendering is a pure read of game state.
- **ow-app** wires everything together. This is the only crate that has `main()`.

## Initiative-Based Combat (not IGOUGO)
The original game uses initiative ordering where ALL units on the field act in order of their initiative score, regardless of faction. This is distinct from the X-COM "player turn / alien turn" model. Our combat manager uses a max-heap sorted by initiative value, popping units one at a time.

## Data-Driven Design
All game content comes from the original .dat files. The engine hardcodes no weapon stats, no merc names, no mission parameters. This means:
1. The engine is inherently moddable (edit the .dat files)
2. We can validate correctness against known original values
3. The engine is a clean behavioral specification, not a data dump

## Logging Philosophy
Every operation that touches game state or file I/O gets a tracing span. Levels:
- `error` — Something broke, game can't continue
- `warn` — Unexpected but recoverable (missing optional file, clamped value)
- `info` — Major state transitions (phase changes, round starts, file loads)
- `debug` — Per-unit actions, per-record parses, combat rolls
- `trace` — Coordinate conversions, per-pixel operations, field-level parsing
