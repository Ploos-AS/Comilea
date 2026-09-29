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

## Bruce Clark decimal test

The pinned upstream `6502_decimal_test.a65` is explicitly public domain and defaults to NMOS 6502 mode (`cputype = 0`). Its default configuration checks ADC/SBC accumulator and carry behavior across all byte values, including invalid BCD digits; N, V, and Z decimal flags are disabled in that configuration. The program starts at `$0200`, leaves `ERROR` at zero-page `$000B` as 0 on success and 1 on failure, and terminates with byte `$DB` as a deliberate stop marker. Comilea therefore treats an illegal opcode `$DB` at the terminal PC as the harness stop condition and inspects `$000B` for pass/fail.


## M0.13 qualification result

M0.13 qualifies the documented NMOS 6502 instruction-state core against the MIT-licensed `SingleStepTests/65x02` vectors pinned at commit `2f6980a2d95757486c7bee24355c360e40e2a224`.

All **151 documented NMOS 6502 opcodes** pass **10,000 vectors per opcode**, for **1,510,000 passing single-step vectors**. The harness verifies final PC, stack pointer, A/X/Y registers, processor status and the final values of memory locations supplied by each vector.

The same strict CI gate also passes:

- Klaus Dormann's pinned 6502 functional test: success trap at `$3469` after 30,646,179 instructions.
- Bruce Clark's NMOS decimal test: success stop at `$024B` after 14,464,188 instructions.

SingleStep qualification exposed and fixed observable NMOS behavior including decimal ADC flag staging and JSR ordering when stack writes overlap the instruction stream.

## M0.14 exact cycle and bus qualification result

M0.14 extends the same pinned SingleStepTests corpus from architectural final-state qualification to cycle-accurate CPU bus qualification.

All **151 documented NMOS 6502 opcodes** pass **10,000 vectors per opcode**, for **1,510,000 passing exact-bus single-step vectors**. For every vector the harness now verifies:

- final PC, SP, A, X, Y and processor status;
- relevant final RAM values;
- total instruction cycle count;
- the complete ordered bus-access sequence;
- address and data value for every observed bus access;
- read versus write direction.

This includes observable NMOS details such as indexed and page-cross dummy reads, zero-page indexed dummy cycles, read-modify-write read/dummy-write/final-write sequencing, stack push/pull cycles, taken-branch cycles, JSR/RTS ordering, BRK/RTI sequencing and the NMOS indirect-JMP page-wrap behavior.

CI uses one canonical 151-opcode exact-bus gate against the immutable SingleStepTests revision already pinned above. The Klaus functional and Bruce Clark decimal gates remain independent complementary checks.

### Scope boundary

M0.14 qualifies the documented **NMOS 6502 CPU instruction bus behavior represented by the pinned vectors**. It does not claim undocumented-opcode conformance, asynchronous line-level interrupt timing, or complete 6510/C64 machine conformance.

M0.14 also does not by itself qualify C64 machine behavior. The 6510 I/O port at `$0000/$0001`, PLA/memory banking, VIC-II, SID, CIA devices, interrupts at machine timing boundaries and complete C64 system timing remain separate machine-level work.
