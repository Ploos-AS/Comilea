use comilea_core::{Cpu6510, Machine};
use serde::Deserialize;
use std::{env, fs, process};

#[derive(Deserialize)]
struct Case {
    name: String,
    initial: State,
    #[serde(rename = "final")]
    final_state: State,
}

#[derive(Deserialize)]
struct State {
    pc: u16,
    s: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    ram: Vec<(u16, u8)>,
}

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: singlestep6502 <vectors.json>");
        process::exit(2);
    });
    let data = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("failed to read {path}: {error}");
        process::exit(2);
    });
    let cases: Vec<Case> = serde_json::from_slice(&data).unwrap_or_else(|error| {
        eprintln!("failed to parse {path}: {error}");
        process::exit(2);
    });

    for (index, case) in cases.iter().enumerate() {
        let mut machine = Machine::new();
        for &(address, value) in &case.initial.ram {
            machine.write(address, value);
        }
        machine.set_cpu(Cpu6510 {
            pc: case.initial.pc,
            sp: case.initial.s,
            a: case.initial.a,
            x: case.initial.x,
            y: case.initial.y,
            status: case.initial.p,
        });
        if let Err(error) = machine.step() {
            eprintln!("single-step FAIL {} #{}: {error:?}", case.name, index + 1);
            process::exit(1);
        }

        let expected = &case.final_state;
        let actual = machine.cpu();
        let registers_match = actual.pc == expected.pc
            && actual.sp == expected.s
            && actual.a == expected.a
            && actual.x == expected.x
            && actual.y == expected.y
            && actual.status == expected.p;
        let memory_matches = expected
            .ram
            .iter()
            .all(|&(address, value)| machine.read(address) == value);
        if !registers_match || !memory_matches {
            eprintln!(
                "single-step FAIL {} #{}: initial pc=${:04x} s=${:02x} a=${:02x} x=${:02x} y=${:02x} p=${:02x}; actual={actual:?}, expected pc=${:04x} s=${:02x} a=${:02x} x=${:02x} y=${:02x} p=${:02x}",
                case.name,
                index + 1,
                case.initial.pc,
                case.initial.s,
                case.initial.a,
                case.initial.x,
                case.initial.y,
                case.initial.p,
                expected.pc,
                expected.s,
                expected.a,
                expected.x,
                expected.y,
                expected.p
            );
            process::exit(1);
        }
    }

    println!("SingleStep PASS: {} vectors", cases.len());
}
