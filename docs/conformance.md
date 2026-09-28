# M0.13 CPU conformance

Comilea's next qualification layer uses external, reproducible CPU tests rather than treating its internal unit tests as proof of NMOS 6502/6510 correctness.

## Planned suites

1. **Klaus Dormann 6502 functional test** — broad official-instruction functional coverage.
2. **ProcessorTests / SingleStepTests** — deterministic per-instruction state vectors; add after the harness can initialize and inspect complete CPU state.
3. **NMOS decimal-mode vectors** — qualify ADC/SBC BCD behavior separately before claiming decimal correctness.
4. **C64/6510-specific tests** — add after the generic CPU layer, including the 6510 I/O port at $0000/$0001 and later machine-level timing.

## Reproducibility policy

External suites must be pinned to an immutable upstream revision. CI must verify the expected checksum before executing downloaded/generated test artifacts. Third-party test data is not silently relicensed as Comilea code; provenance and upstream licensing must remain documented.

## Qualification language

Passing Comilea's unit tests means **internal regression coverage**. Passing an external suite means only the behavior covered by that pinned suite is qualified. Do not claim full 6510 or C64 conformance from opcode decode coverage alone.

## Harness requirements

The core must support deterministic setup of registers, status, PC, memory, and cycle count plus deterministic observation after one instruction. M0.13 begins by exposing explicit CPU-state setup; the external adapters should remain test-only code.

## Klaus functional-test pin

Upstream: `Klaus2m5/6502_65C02_functional_tests`

Pinned revision: `7954e2dbb49c469ea286070bf46cdd71aeb29e4b`

Source: `6502_functional_test.a65` (GPL-3.0-or-later, Klaus Dormann). The source documents an entry PC of `$0400`, requires writable memory for the default self-modifying configuration, and reports both failures and final success by looping at the current PC. The default configuration exercises documented NMOS 6502 opcodes only; decimal ADC/SBC uses valid BCD operands and does not qualify N/V/Z decimal flags.

Comilea does not vendor this GPL test source into the MIT core. The CI adapter fetches the upstream prebuilt 64 KiB image from the pinned revision. Its Git blob SHA is `c9a35e1d6bd2e7d85844da2abf7034d5ed820e6e`; the corresponding pinned listing identifies `$3469` as the final success self-loop. CI verifies the downloaded image against the pinned Git object identity before execution and keeps third-party licensing/provenance explicit.
