# Comilea

Deterministic, embeddable Commodore 64 emulation for testing, tooling, automation and interactive frontends.

## M0 scope

M0 establishes the architecture, not a complete C64 emulator:

- Rust workspace with `comilea-core` and `comilea-cli`
- deterministic machine state with no wall-clock dependency
- 64 KiB memory model
- minimal MOS 6510 register/state model
- cycle counter and deterministic stepping API
- headless CLI smoke runner
- unit tests and GitHub Actions CI

No Commodore ROM images are included. ROMs, software and disk images remain user-supplied and must be legally obtained.

## Architecture

`comilea-core` is the canonical machine library. It must stay independent of GUI frameworks, host timing, audio devices and global mutable state. Frontends are clients of the core.

Planned layers:

```text
comilea-core
    |
    +-- comilea-cli
    +-- future SDL frontend
    +-- future server/API frontend
    +-- future WASM frontend
```

The long-term machine model will grow toward the 6510, VIC-II, SID, CIA, IEC bus and a separately scheduled 1541 machine while preserving deterministic replay and CI use.

## Build

```sh
cargo build --workspace
cargo test --workspace
cargo run -p comilea-cli -- --cycles 1000
```

## License

Software is licensed under the MIT License. See `LICENSE`.
