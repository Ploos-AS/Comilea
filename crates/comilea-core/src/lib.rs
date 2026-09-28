#![forbid(unsafe_code)]

const FLAG_CARRY: u8 = 0x01;
const FLAG_ZERO: u8 = 0x02;
const FLAG_OVERFLOW: u8 = 0x40;
const FLAG_NEGATIVE: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cpu6510 {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub status: u8,
    pub pc: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepError {
    IllegalOpcode { opcode: u8, pc: u16 },
}

#[derive(Clone)]
pub struct Machine {
    cpu: Cpu6510,
    memory: Box<[u8; 65_536]>,
    cycles: u64,
}

impl Default for Machine {
    fn default() -> Self { Self::new() }
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
    pub const fn cpu(&self) -> &Cpu6510 { &self.cpu }

    #[must_use]
    pub const fn cycles(&self) -> u64 { self.cycles }

    #[must_use]
    pub fn read(&self, address: u16) -> u8 { self.memory[usize::from(address)] }

    pub fn write(&mut self, address: u16, value: u8) {
        self.memory[usize::from(address)] = value;
    }

    pub fn load(&mut self, address: u16, bytes: &[u8]) {
        for (offset, byte) in bytes.iter().copied().enumerate() {
            let offset = u16::try_from(offset).expect("program is too large for 16-bit address space");
            self.write(address.wrapping_add(offset), byte);
        }
    }

    pub fn reset(&mut self) {
        self.cpu = Cpu6510 { sp: 0xfd, status: 0x24, ..Cpu6510::default() };
        self.cpu.pc = u16::from_le_bytes([self.read(0xfffc), self.read(0xfffd)]);
        self.cycles = 0;
    }

    pub fn step_cycles(&mut self, cycles: u64) {
        self.cycles = self.cycles.saturating_add(cycles);
    }

    pub fn step(&mut self) -> Result<u8, StepError> {
        let instruction_pc = self.cpu.pc;
        let opcode = self.fetch_byte();
        let used = match opcode {
            0xea => 2, // NOP
            0xa9 => { let value = self.fetch_byte(); self.load_a(value); 2 }
            0xa5 => { let address = self.addr_zero_page(); self.load_a(self.read(address)); 3 }
            0xb5 => { let address = self.addr_zero_page_x(); self.load_a(self.read(address)); 4 }
            0xad => { let address = self.addr_absolute(); self.load_a(self.read(address)); 4 }
            0xbd => { let (address, crossed) = self.addr_absolute_x(); self.load_a(self.read(address)); 4 + u64::from(crossed) }
            0xb9 => { let (address, crossed) = self.addr_absolute_y(); self.load_a(self.read(address)); 4 + u64::from(crossed) }
            0xa2 => { let value = self.fetch_byte(); self.cpu.x = value; self.set_zn(value); 2 }
            0xa0 => { let value = self.fetch_byte(); self.cpu.y = value; self.set_zn(value); 2 }
            0x69 => { let value = self.fetch_byte(); self.adc(value); 2 }
            0xe9 => { let value = self.fetch_byte(); self.sbc(value); 2 }
            0x85 => { let address = self.addr_zero_page(); self.write(address, self.cpu.a); 3 }
            0x95 => { let address = self.addr_zero_page_x(); self.write(address, self.cpu.a); 4 }
            0x8d => { let address = self.addr_absolute(); self.write(address, self.cpu.a); 4 }
            0x9d => { let (address, _) = self.addr_absolute_x(); self.write(address, self.cpu.a); 5 }
            0x99 => { let (address, _) = self.addr_absolute_y(); self.write(address, self.cpu.a); 5 }
            0x4c => { self.cpu.pc = self.fetch_word(); 3 }
            0x20 => { let target = self.fetch_word(); let return_pc = self.cpu.pc.wrapping_sub(1); self.push((return_pc >> 8) as u8); self.push(return_pc as u8); self.cpu.pc = target; 6 }
            0x60 => { let lo = self.pop(); let hi = self.pop(); self.cpu.pc = u16::from_le_bytes([lo, hi]).wrapping_add(1); 6 }
            0xd0 => self.branch(self.cpu.status & FLAG_ZERO == 0),
            0xf0 => self.branch(self.cpu.status & FLAG_ZERO != 0),
            _ => {
                self.cpu.pc = instruction_pc;
                return Err(StepError::IllegalOpcode { opcode, pc: instruction_pc });
            }
        };
        self.cycles = self.cycles.saturating_add(used);
        Ok(used as u8)
    }

    fn fetch_byte(&mut self) -> u8 {
        let value = self.read(self.cpu.pc);
        self.cpu.pc = self.cpu.pc.wrapping_add(1);
        value
    }

    fn fetch_word(&mut self) -> u16 {
        let lo = self.fetch_byte();
        let hi = self.fetch_byte();
        u16::from_le_bytes([lo, hi])
    }

    fn addr_zero_page(&mut self) -> u16 { u16::from(self.fetch_byte()) }

    fn addr_zero_page_x(&mut self) -> u16 {
        u16::from(self.fetch_byte().wrapping_add(self.cpu.x))
    }

    fn addr_absolute(&mut self) -> u16 { self.fetch_word() }

    fn addr_absolute_x(&mut self) -> (u16, bool) {
        let base = self.fetch_word();
        let address = base.wrapping_add(u16::from(self.cpu.x));
        (address, (base & 0xff00) != (address & 0xff00))
    }

    fn addr_absolute_y(&mut self) -> (u16, bool) {
        let base = self.fetch_word();
        let address = base.wrapping_add(u16::from(self.cpu.y));
        (address, (base & 0xff00) != (address & 0xff00))
    }

    fn load_a(&mut self, value: u8) {
        self.cpu.a = value;
        self.set_zn(value);
    }

    fn push(&mut self, value: u8) {
        self.write(0x0100 | u16::from(self.cpu.sp), value);
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    }

    fn pop(&mut self) -> u8 {
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        self.read(0x0100 | u16::from(self.cpu.sp))
    }

    fn branch(&mut self, condition: bool) -> u64 {
        let offset = self.fetch_byte() as i8;
        if !condition { return 2; }
        let old = self.cpu.pc;
        self.cpu.pc = self.cpu.pc.wrapping_add_signed(i16::from(offset));
        3 + u64::from((old & 0xff00) != (self.cpu.pc & 0xff00))
    }

    fn adc(&mut self, value: u8) {
        let carry = u16::from(self.cpu.status & FLAG_CARRY != 0);
        let a = self.cpu.a;
        let sum = u16::from(a) + u16::from(value) + carry;
        let result = sum as u8;
        self.cpu.status &= !(FLAG_CARRY | FLAG_OVERFLOW);
        if sum > 0xff { self.cpu.status |= FLAG_CARRY; }
        if (!(a ^ value) & (a ^ result) & 0x80) != 0 { self.cpu.status |= FLAG_OVERFLOW; }
        self.cpu.a = result;
        self.set_zn(result);
    }

    fn sbc(&mut self, value: u8) {
        self.adc(!value);
    }

    fn set_zn(&mut self, value: u8) {
        self.cpu.status &= !(FLAG_ZERO | FLAG_NEGATIVE);
        if value == 0 { self.cpu.status |= FLAG_ZERO; }
        if value & 0x80 != 0 { self.cpu.status |= FLAG_NEGATIVE; }
    }
}

#[cfg(test)]
mod tests {
    use super::{Machine, StepError};

    fn machine_with(program: &[u8]) -> Machine {
        let mut m = Machine::new();
        m.load(0x0801, program);
        m.write(0xfffc, 0x01);
        m.write(0xfffd, 0x08);
        m.reset();
        m
    }

    #[test]
    fn memory_round_trip() {
        let mut m = Machine::new();
        m.write(0x0400, 0x41);
        assert_eq!(m.read(0x0400), 0x41);
    }

    #[test]
    fn reset_reads_vector_and_resets_cycle_count() {
        let mut m = Machine::new();
        m.write(0xfffc, 0x34); m.write(0xfffd, 0x12);
        m.step_cycles(123); m.reset();
        assert_eq!(m.cpu().pc, 0x1234);
        assert_eq!(m.cycles(), 0);
    }

    #[test]
    fn immediate_loads_set_registers_flags_and_cycles() {
        let mut m = machine_with(&[0xa9, 0x80, 0xa2, 0x00, 0xa0, 0x42]);
        assert_eq!(m.step(), Ok(2)); assert_eq!(m.cpu().a, 0x80); assert_eq!(m.cpu().status & 0x80, 0x80);
        assert_eq!(m.step(), Ok(2)); assert_eq!(m.cpu().x, 0x00); assert_eq!(m.cpu().status & 0x02, 0x02);
        assert_eq!(m.step(), Ok(2)); assert_eq!(m.cpu().y, 0x42);
        assert_eq!(m.cycles(), 6);
    }

    #[test]
    fn sta_absolute_and_jmp_absolute_work() {
        let mut m = machine_with(&[0xa9, 0x41, 0x8d, 0x00, 0x04, 0x4c, 0x01, 0x08]);
        m.step().unwrap(); m.step().unwrap();
        assert_eq!(m.read(0x0400), 0x41);
        assert_eq!(m.step(), Ok(3));
        assert_eq!(m.cpu().pc, 0x0801);
        assert_eq!(m.cycles(), 9);
    }

    #[test]
    fn nop_advances_pc_and_cycles() {
        let mut m = machine_with(&[0xea]);
        assert_eq!(m.step(), Ok(2));
        assert_eq!(m.cpu().pc, 0x0802);
        assert_eq!(m.cycles(), 2);
    }

    #[test]
    fn illegal_opcode_is_reported_without_consuming_instruction() {
        let mut m = machine_with(&[0x02]);
        assert_eq!(m.step(), Err(StepError::IllegalOpcode { opcode: 0x02, pc: 0x0801 }));
        assert_eq!(m.cpu().pc, 0x0801);
        assert_eq!(m.cycles(), 0);
    }

    #[test]
    fn jsr_and_rts_round_trip_through_stack() {
        let mut m = machine_with(&[0x20, 0x06, 0x08, 0xea, 0xea, 0xea, 0x60]);
        assert_eq!(m.step(), Ok(6)); assert_eq!(m.cpu().pc, 0x0806); assert_eq!(m.cpu().sp, 0xfb);
        assert_eq!(m.step(), Ok(6)); assert_eq!(m.cpu().pc, 0x0804); assert_eq!(m.cpu().sp, 0xfd);
    }

    #[test]
    fn branches_apply_signed_offset_and_cycles() {
        let mut m = machine_with(&[0xa9, 0x00, 0xf0, 0x02, 0xea, 0xea, 0xea]);
        m.step().unwrap();
        assert_eq!(m.step(), Ok(3));
        assert_eq!(m.cpu().pc, 0x0807);
    }

    #[test]
    fn adc_and_sbc_update_accumulator_and_flags() {
        let mut m = machine_with(&[0xa9, 0x7f, 0x69, 0x01, 0xe9, 0x01]);
        m.step().unwrap(); m.step().unwrap();
        assert_eq!(m.cpu().a, 0x80); assert_ne!(m.cpu().status & FLAG_OVERFLOW, 0);
        m.step().unwrap();
        assert_eq!(m.cpu().a, 0x7e);
    }

    #[test]
    fn lda_addressing_modes_read_expected_values() {
        let mut m = machine_with(&[0xa2, 0x02, 0xa5, 0x10, 0xb5, 0x10, 0xad, 0x00, 0x20, 0xbd, 0xff, 0x20]);
        m.write(0x0010, 0x11); m.write(0x0012, 0x22); m.write(0x2000, 0x33); m.write(0x2101, 0x44);
        m.step().unwrap();
        assert_eq!(m.step(), Ok(3)); assert_eq!(m.cpu().a, 0x11);
        assert_eq!(m.step(), Ok(4)); assert_eq!(m.cpu().a, 0x22);
        assert_eq!(m.step(), Ok(4)); assert_eq!(m.cpu().a, 0x33);
        assert_eq!(m.step(), Ok(5)); assert_eq!(m.cpu().a, 0x44);
    }

    #[test]
    fn sta_indexed_modes_write_expected_addresses() {
        let mut m = machine_with(&[0xa9, 0x5a, 0xa2, 0x02, 0x95, 0xfe, 0x9d, 0xff, 0x20]);
        m.step().unwrap(); m.step().unwrap();
        assert_eq!(m.step(), Ok(4)); assert_eq!(m.read(0x0000), 0x5a);
        assert_eq!(m.step(), Ok(5)); assert_eq!(m.read(0x2101), 0x5a);
    }

    #[test]
    fn instruction_execution_is_deterministic() {
        let program = [0xa9, 0x2a, 0x8d, 0x00, 0x04, 0xea];
        let mut a = machine_with(&program);
        let mut b = machine_with(&program);
        for _ in 0..3 { a.step().unwrap(); b.step().unwrap(); }
        assert_eq!(a.cpu(), b.cpu());
        assert_eq!(a.cycles(), b.cycles());
        assert_eq!(a.read(0x0400), b.read(0x0400));
    }
}
