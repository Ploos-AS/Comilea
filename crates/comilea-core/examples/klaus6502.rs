use comilea_core::{Cpu6510, Machine, StepError};
use std::{env, fs, process};

const ENTRY_PC: u16 = 0x0400;
const SUCCESS_PC: u16 = 0x3469;
const INSTRUCTION_BUDGET: u64 = 100_000_000;
const STABLE_PC_HITS: u32 = 3;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: klaus6502 <64k-image>");
        process::exit(2);
    });
    let image = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("failed to read {path}: {error}");
        process::exit(2);
    });
    if image.len() != 65_536 {
        eprintln!("expected 65536-byte image, got {}", image.len());
        process::exit(2);
    }

    let mut machine = Machine::new();
    machine.load(0, &image);
    machine.set_cpu(Cpu6510 {
        pc: ENTRY_PC,
        sp: 0xfd,
        status: 0x24,
        ..Cpu6510::default()
    });

    let mut previous_pc = ENTRY_PC;
    let mut repeated = 0u32;
    for instructions in 1..=INSTRUCTION_BUDGET {
        if let Err(StepError::IllegalOpcode { opcode, pc }) = machine.step() {
            eprintln!("Klaus FAIL: illegal opcode ${opcode:02x} at ${pc:04x} after {instructions} instructions");
            process::exit(1);
        }
        let pc = machine.cpu().pc;
        if pc == previous_pc {
            repeated += 1;
            if repeated >= STABLE_PC_HITS {
                if pc == SUCCESS_PC {
                    println!("Klaus PASS at ${pc:04x} after {instructions} instructions");
                    return;
                }
                eprintln!("Klaus FAIL trap at ${pc:04x} after {instructions} instructions");
                process::exit(1);
            }
        } else {
            previous_pc = pc;
            repeated = 0;
        }
    }
    eprintln!(
        "Klaus FAIL: instruction budget exceeded at ${:04x}",
        machine.cpu().pc
    );
    process::exit(1);
}
