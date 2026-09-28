#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cpu6510 {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub pc: u16,
}

#[derive(Clone)]
pub struct Machine {
    cpu: Cpu6510,
    memory: Box<[u8; 65_536]>,
    cycles: u64,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cpu: Cpu6510 { sp: 0xfd, status: 0x24, ..Cpu6510::default() },
            memory: Box::new([0; 65_536]),
            cycles: 0,
        }
    }

    #[must_use]
    pub const fn cpu(&self) -> &Cpu6510 {
        &self.cpu
    }

    #[must_use]
    pub const fn cycles(&self) -> u64 {
        self.cycles
    }

    #[must_use]
    pub fn read(&self, address: u16) -> u8 {
        self.memory[usize::from(address)]
    }

    pub fn write(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    pub fn reset(&mut self) {
        self.cpu = Cpu6510 { sp: 0xfd, status: 0x24, ..Cpu6510::default() };
        self.cpu.pc = u16::from_le_bytes([self.read(0xfffc), self.read(0xfffd)]);
        self.cycles = 0;
    }

    pub fn step_cycles(&mut self, cycles: u64) {
        self.cycles = self.cycles.saturating_add(cycles);
    }
}

#[cfg(test)]
mod tests {
    use super::Machine;

    #[test]
    fn memory_round_trip() {
        let mut machine = Machine::new();
        machine.write(0x0400, 0x41);
        assert_eq!(machine.read(0x0400), 0x41);
    }

    #[test]
    fn reset_reads_vector_and_resets_cycle_count() {
        let mut machine = Machine::new();
        machine.write(0xfffc, 0x34);
        machine.write(0xfffd, 0x12);
        machine.step_cycles(123);
        machine.reset();
        assert_eq!(machine.cpu().pc, 0x1234);
        assert_eq!(machine.cycles(), 0);
    }

    #[test]
    fn stepping_is_deterministic() {
        let mut a = Machine::new();
        let mut b = Machine::new();
        a.step_cycles(10_000);
        b.step_cycles(10_000);
        assert_eq!(a.cycles(), b.cycles());
        assert_eq!(a.cpu(), b.cpu());
    }
}
