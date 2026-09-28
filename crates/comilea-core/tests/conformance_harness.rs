use comilea_core::{Cpu6510, Machine, StepError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapOutcome {
    Trap {
        pc: u16,
        instructions: u64,
    },
    BudgetExceeded {
        pc: u16,
        instructions: u64,
    },
    IllegalOpcode {
        opcode: u8,
        pc: u16,
        instructions: u64,
    },
}

pub fn run_self_trapping_image(
    image: &[u8],
    entry_pc: u16,
    instruction_budget: u64,
    stable_pc_hits: u32,
) -> TrapOutcome {
    assert_eq!(
        image.len(),
        65_536,
        "conformance image must be exactly 64 KiB"
    );
    assert!(stable_pc_hits > 0, "stable_pc_hits must be non-zero");

    let mut machine = Machine::new();
    machine.load(0, image);
    machine.set_cpu(Cpu6510 {
        pc: entry_pc,
        sp: 0xfd,
        status: 0x24,
        ..Cpu6510::default()
    });

    let mut previous_pc = entry_pc;
    let mut repeated = 0u32;
    for instructions in 0..instruction_budget {
        match machine.step() {
            Ok(_) => {}
            Err(StepError::IllegalOpcode { opcode, pc }) => {
                return TrapOutcome::IllegalOpcode {
                    opcode,
                    pc,
                    instructions,
                };
            }
        }
        let pc = machine.cpu().pc;
        if pc == previous_pc {
            repeated += 1;
            if repeated >= stable_pc_hits {
                return TrapOutcome::Trap {
                    pc,
                    instructions: instructions + 1,
                };
            }
        } else {
            previous_pc = pc;
            repeated = 0;
        }
    }
    TrapOutcome::BudgetExceeded {
        pc: machine.cpu().pc,
        instructions: instruction_budget,
    }
}

#[cfg(test)]
mod tests {
    use super::{run_self_trapping_image, TrapOutcome};

    #[test]
    fn detects_a_jmp_to_self_trap() {
        let mut image = vec![0u8; 65_536];
        image[0x0400] = 0x4c;
        image[0x0401] = 0x00;
        image[0x0402] = 0x04;
        assert_eq!(
            run_self_trapping_image(&image, 0x0400, 100, 3),
            TrapOutcome::Trap {
                pc: 0x0400,
                instructions: 3
            }
        );
    }

    #[test]
    fn reports_instruction_budget_exhaustion() {
        let mut image = vec![0xeau8; 65_536];
        assert_eq!(
            run_self_trapping_image(&image, 0x0400, 4, 3),
            TrapOutcome::BudgetExceeded {
                pc: 0x0404,
                instructions: 4
            }
        );
    }
}
