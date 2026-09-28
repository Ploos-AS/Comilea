use comilea_core::{Cpu6510, Machine, StepError};
use std::{env, fs, process};

const ENTRY_PC: u16 = 0x0200;
const ERROR_ADDRESS: u16 = 0x000b;
const STOP_OPCODE: u8 = 0xdb;
const INSTRUCTION_BUDGET: u64 = 50_000_000;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: decimal6502 <assembled-image>");
        process::exit(2);
    });
    let image = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("failed to read {path}: {error}");
        process::exit(2);
    });

    let mut machine = Machine::new();
    machine.load(ENTRY_PC, &image);
    machine.set_cpu(Cpu6510 {
        pc: ENTRY_PC,
        sp: 0xfd,
        status: 0x24,
        ..Cpu6510::default()
    });

    for instructions in 1..=INSTRUCTION_BUDGET {
        match machine.step() {
            Ok(_) => {}
            Err(StepError::IllegalOpcode { opcode, pc }) if opcode == STOP_OPCODE => {
                let error = machine.read(ERROR_ADDRESS);
                if error == 0 {
                    println!("Bruce Clark decimal PASS at ${pc:04x} after {instructions} instructions");
                    return;
                }
                eprintln!("Bruce Clark decimal FAIL: ERROR=${error:02x} at ${pc:04x} after {instructions} instructions");
                process::exit(1);
            }
            Err(StepError::IllegalOpcode { opcode, pc }) => {
                eprintln!("decimal test unexpected illegal opcode ${opcode:02x} at ${pc:04x} after {instructions} instructions");
                process::exit(1);
            }
        }
    }

    eprintln!("decimal test instruction budget exceeded at ${:04x}", machine.cpu().pc);
    process::exit(1);
}
