#![forbid(unsafe_code)]
#![allow(clippy::too_many_lines)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

pub mod opcode;
pub use opcode::{opcode_info, OpcodeClass, OpcodeInfo, OFFICIAL_OPCODE_COUNT};

const FLAG_CARRY: u8 = 0x01;
const FLAG_ZERO: u8 = 0x02;
const FLAG_INTERRUPT_DISABLE: u8 = 0x04;
const FLAG_DECIMAL: u8 = 0x08;
const FLAG_BREAK: u8 = 0x10;
const FLAG_UNUSED: u8 = 0x20;
const FLAG_OVERFLOW: u8 = 0x40;
const FLAG_NEGATIVE: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BusAccess {
    pub address: u16,
    pub value: u8,
    pub write: bool,
}

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
    memory: Box<[u8]>,
    cycles: u64,
    bus_trace: Vec<BusAccess>,
    tracing: bool,
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
            cpu: Cpu6510 {
                sp: 0xfd,
                status: 0x24,
                ..Cpu6510::default()
            },
            memory: vec![0; 65_536].into_boxed_slice(),
            cycles: 0,
            bus_trace: Vec::new(),
            tracing: false,
        }
    }

    #[must_use]
    pub const fn cpu(&self) -> &Cpu6510 {
        &self.cpu
    }

    /// Replaces the CPU register state. Intended for deterministic test and debugger setup.
    pub fn set_cpu(&mut self, cpu: Cpu6510) {
        self.cpu = cpu;
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
        if self.tracing {
            self.bus_trace.push(BusAccess {
                address,
                value,
                write: true,
            });
        }
        self.memory[usize::from(address)] = value;
    }

    #[must_use]
    pub fn bus_trace(&self) -> &[BusAccess] {
        &self.bus_trace
    }

    pub fn begin_bus_trace(&mut self) {
        self.bus_trace.clear();
        self.tracing = true;
    }

    pub fn end_bus_trace(&mut self) {
        self.tracing = false;
    }

    fn traced_read(&mut self, address: u16) -> u8 {
        let value = self.read(address);
        if self.tracing {
            self.bus_trace.push(BusAccess {
                address,
                value,
                write: false,
            });
        }
        value
    }

    /// Loads bytes starting at `address`, wrapping across the 16-bit address space.
    ///
    /// # Panics
    /// Panics when `bytes` contains more than 65,536 bytes.
    pub fn load(&mut self, address: u16, bytes: &[u8]) {
        for (offset, byte) in bytes.iter().copied().enumerate() {
            let offset =
                u16::try_from(offset).expect("program is too large for 16-bit address space");
            self.write(address.wrapping_add(offset), byte);
        }
    }

    pub fn reset(&mut self) {
        self.cpu = Cpu6510 {
            sp: 0xfd,
            status: 0x24,
            ..Cpu6510::default()
        };
        self.cpu.pc = u16::from_le_bytes([self.read(0xfffc), self.read(0xfffd)]);
        self.cycles = 0;
    }

    pub fn irq(&mut self) -> bool {
        if self.cpu.status & FLAG_INTERRUPT_DISABLE != 0 {
            return false;
        }
        self.interrupt(0xfffe);
        true
    }

    pub fn nmi(&mut self) {
        self.interrupt(0xfffa);
    }

    pub fn step_cycles(&mut self, cycles: u64) {
        self.cycles = self.cycles.saturating_add(cycles);
    }

    /// Executes one 6510 instruction and returns the cycles consumed.
    ///
    /// # Errors
    /// Returns [`StepError::IllegalOpcode`] when the fetched opcode has no implemented decode path.
    pub fn step(&mut self) -> Result<u8, StepError> {
        let instruction_pc = self.cpu.pc;
        let opcode = self.fetch_byte();
        let used = match opcode {
            0x00 => {
                let p = self.cpu.pc.wrapping_add(1);
                self.push((p >> 8) as u8);
                self.push(p as u8);
                self.push(self.cpu.status | FLAG_BREAK | FLAG_UNUSED);
                self.cpu.status |= FLAG_INTERRUPT_DISABLE;
                self.cpu.pc = u16::from_le_bytes([self.read(0xfffe), self.read(0xffff)]);
                7
            }
            0x40 => {
                self.cpu.status = (self.pop() | FLAG_UNUSED) & !FLAG_BREAK;
                let lo = self.pop();
                let hi = self.pop();
                self.cpu.pc = u16::from_le_bytes([lo, hi]);
                6
            }
            0xea => {
                self.implied_cycle();
                2
            } // NOP
            0xa9 => {
                let value = self.fetch_byte();
                self.load_a(value);
                2
            }
            0xa5 => {
                let address = self.addr_zero_page();
                let value = self.traced_read(address);
                self.load_a(value);
                3
            }
            0xb5 => {
                let address = self.addr_zero_page_x();
                let value = self.traced_read(address);
                self.load_a(value);
                4
            }
            0xad => {
                let address = self.addr_absolute();
                let value = self.traced_read(address);
                self.load_a(value);
                4
            }
            0xbd => {
                let (address, crossed) = self.addr_absolute_x();
                let value = self.traced_read(address);
                self.load_a(value);
                4 + u64::from(crossed)
            }
            0xb9 => {
                let (address, crossed) = self.addr_absolute_y();
                let value = self.traced_read(address);
                self.load_a(value);
                4 + u64::from(crossed)
            }
            0xa1 => {
                let address = self.addr_indexed_indirect();
                let value = self.traced_read(address);
                self.load_a(value);
                6
            }
            0xb1 => {
                let (address, crossed) = self.addr_indirect_indexed();
                let value = self.traced_read(address);
                self.load_a(value);
                5 + u64::from(crossed)
            }
            0xa2 => {
                let value = self.fetch_byte();
                self.load_x(value);
                2
            }
            0xa6 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.load_x(value);
                3
            }
            0xb6 => {
                let a = self.addr_zero_page_y();
                let value = self.traced_read(a);
                self.load_x(value);
                4
            }
            0xae => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.load_x(value);
                4
            }
            0xbe => {
                let (a, crossed) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.load_x(value);
                4 + u64::from(crossed)
            }
            0xa0 => {
                let value = self.fetch_byte();
                self.load_y(value);
                2
            }
            0xa4 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.load_y(value);
                3
            }
            0xb4 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.load_y(value);
                4
            }
            0xac => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.load_y(value);
                4
            }
            0xbc => {
                let (a, crossed) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.load_y(value);
                4 + u64::from(crossed)
            }
            0x69 => {
                let v = self.fetch_byte();
                self.adc(v);
                2
            }
            0x65 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.adc(value);
                3
            }
            0x75 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.adc(value);
                4
            }
            0x6d => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.adc(value);
                4
            }
            0x7d => {
                let (a, x) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.adc(value);
                4 + u64::from(x)
            }
            0x79 => {
                let (a, x) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.adc(value);
                4 + u64::from(x)
            }
            0x61 => {
                let a = self.addr_indexed_indirect();
                let value = self.traced_read(a);
                self.adc(value);
                6
            }
            0x71 => {
                let (a, x) = self.addr_indirect_indexed();
                let value = self.traced_read(a);
                self.adc(value);
                5 + u64::from(x)
            }
            0xe9 => {
                let v = self.fetch_byte();
                self.sbc(v);
                2
            }
            0xe5 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.sbc(value);
                3
            }
            0xf5 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.sbc(value);
                4
            }
            0xed => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.sbc(value);
                4
            }
            0xfd => {
                let (a, x) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.sbc(value);
                4 + u64::from(x)
            }
            0xf9 => {
                let (a, x) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.sbc(value);
                4 + u64::from(x)
            }
            0xe1 => {
                let a = self.addr_indexed_indirect();
                let value = self.traced_read(a);
                self.sbc(value);
                6
            }
            0xf1 => {
                let (a, x) = self.addr_indirect_indexed();
                let value = self.traced_read(a);
                self.sbc(value);
                5 + u64::from(x)
            }
            0x29 => {
                let v = self.fetch_byte();
                self.and_a(v);
                2
            }
            0x25 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.and_a(value);
                3
            }
            0x35 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.and_a(value);
                4
            }
            0x2d => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.and_a(value);
                4
            }
            0x3d => {
                let (a, x) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.and_a(value);
                4 + u64::from(x)
            }
            0x39 => {
                let (a, x) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.and_a(value);
                4 + u64::from(x)
            }
            0x21 => {
                let a = self.addr_indexed_indirect();
                let value = self.traced_read(a);
                self.and_a(value);
                6
            }
            0x31 => {
                let (a, x) = self.addr_indirect_indexed();
                let value = self.traced_read(a);
                self.and_a(value);
                5 + u64::from(x)
            }
            0x09 => {
                let v = self.fetch_byte();
                self.ora_a(v);
                2
            }
            0x05 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.ora_a(value);
                3
            }
            0x15 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.ora_a(value);
                4
            }
            0x0d => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.ora_a(value);
                4
            }
            0x1d => {
                let (a, x) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.ora_a(value);
                4 + u64::from(x)
            }
            0x19 => {
                let (a, x) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.ora_a(value);
                4 + u64::from(x)
            }
            0x01 => {
                let a = self.addr_indexed_indirect();
                let value = self.traced_read(a);
                self.ora_a(value);
                6
            }
            0x11 => {
                let (a, x) = self.addr_indirect_indexed();
                let value = self.traced_read(a);
                self.ora_a(value);
                5 + u64::from(x)
            }
            0x49 => {
                let v = self.fetch_byte();
                self.eor_a(v);
                2
            }
            0x45 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.eor_a(value);
                3
            }
            0x55 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.eor_a(value);
                4
            }
            0x4d => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.eor_a(value);
                4
            }
            0x5d => {
                let (a, x) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.eor_a(value);
                4 + u64::from(x)
            }
            0x59 => {
                let (a, x) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.eor_a(value);
                4 + u64::from(x)
            }
            0x41 => {
                let a = self.addr_indexed_indirect();
                let value = self.traced_read(a);
                self.eor_a(value);
                6
            }
            0x51 => {
                let (a, x) = self.addr_indirect_indexed();
                let value = self.traced_read(a);
                self.eor_a(value);
                5 + u64::from(x)
            }
            0xc9 => {
                let v = self.fetch_byte();
                self.cmp_a(v);
                2
            }
            0xc5 => {
                let a = self.addr_zero_page();
                let value = self.traced_read(a);
                self.cmp_a(value);
                3
            }
            0xd5 => {
                let a = self.addr_zero_page_x();
                let value = self.traced_read(a);
                self.cmp_a(value);
                4
            }
            0xcd => {
                let a = self.addr_absolute();
                let value = self.traced_read(a);
                self.cmp_a(value);
                4
            }
            0xdd => {
                let (a, x) = self.addr_absolute_x();
                let value = self.traced_read(a);
                self.cmp_a(value);
                4 + u64::from(x)
            }
            0xd9 => {
                let (a, x) = self.addr_absolute_y();
                let value = self.traced_read(a);
                self.cmp_a(value);
                4 + u64::from(x)
            }
            0xc1 => {
                let a = self.addr_indexed_indirect();
                let value = self.traced_read(a);
                self.cmp_a(value);
                6
            }
            0xd1 => {
                let (a, x) = self.addr_indirect_indexed();
                let value = self.traced_read(a);
                self.cmp_a(value);
                5 + u64::from(x)
            }
            0xe0 => {
                let v = self.fetch_byte();
                self.compare(self.cpu.x, v);
                2
            }
            0xe4 => {
                let a = self.addr_zero_page();
                self.compare(self.cpu.x, self.read(a));
                3
            }
            0xec => {
                let a = self.addr_absolute();
                self.compare(self.cpu.x, self.read(a));
                4
            }
            0xc0 => {
                let v = self.fetch_byte();
                self.compare(self.cpu.y, v);
                2
            }
            0xc4 => {
                let a = self.addr_zero_page();
                self.compare(self.cpu.y, self.read(a));
                3
            }
            0xcc => {
                let a = self.addr_absolute();
                self.compare(self.cpu.y, self.read(a));
                4
            }
            0x24 => {
                let address = self.addr_zero_page();
                self.bit(self.read(address));
                3
            }
            0x2c => {
                let address = self.addr_absolute();
                self.bit(self.read(address));
                4
            }
            0x0a => {
                self.implied_cycle();
                self.cpu.a = self.asl(self.cpu.a);
                2
            }
            0x4a => {
                self.implied_cycle();
                self.cpu.a = self.lsr(self.cpu.a);
                2
            }
            0x2a => {
                self.implied_cycle();
                self.cpu.a = self.rol(self.cpu.a);
                2
            }
            0x6a => {
                self.implied_cycle();
                self.cpu.a = self.ror(self.cpu.a);
                2
            }
            0x06 => {
                let a = self.addr_zero_page();
                let old = self.rmw_read(a);
                let v = self.asl(old);
                self.write(a, v);
                5
            }
            0x16 => {
                let a = self.addr_zero_page_x();
                let old = self.rmw_read(a);
                let v = self.asl(old);
                self.write(a, v);
                6
            }
            0x0e => {
                let a = self.addr_absolute();
                let old = self.rmw_read(a);
                let v = self.asl(old);
                self.write(a, v);
                6
            }
            0x1e => {
                let a = self.addr_absolute_indexed_rmw();
                let old = self.rmw_read(a);
                let v = self.asl(old);
                self.write(a, v);
                7
            }
            0x46 => {
                let a = self.addr_zero_page();
                let old = self.rmw_read(a);
                let v = self.lsr(old);
                self.write(a, v);
                5
            }
            0x56 => {
                let a = self.addr_zero_page_x();
                let old = self.rmw_read(a);
                let v = self.lsr(old);
                self.write(a, v);
                6
            }
            0x4e => {
                let a = self.addr_absolute();
                let old = self.rmw_read(a);
                let v = self.lsr(old);
                self.write(a, v);
                6
            }
            0x5e => {
                let a = self.addr_absolute_indexed_rmw();
                let old = self.rmw_read(a);
                let v = self.lsr(old);
                self.write(a, v);
                7
            }
            0x26 => {
                let a = self.addr_zero_page();
                let old = self.rmw_read(a);
                let v = self.rol(old);
                self.write(a, v);
                5
            }
            0x36 => {
                let a = self.addr_zero_page_x();
                let old = self.rmw_read(a);
                let v = self.rol(old);
                self.write(a, v);
                6
            }
            0x2e => {
                let a = self.addr_absolute();
                let old = self.rmw_read(a);
                let v = self.rol(old);
                self.write(a, v);
                6
            }
            0x3e => {
                let a = self.addr_absolute_indexed_rmw();
                let old = self.rmw_read(a);
                let v = self.rol(old);
                self.write(a, v);
                7
            }
            0x66 => {
                let a = self.addr_zero_page();
                let old = self.rmw_read(a);
                let v = self.ror(old);
                self.write(a, v);
                5
            }
            0x76 => {
                let a = self.addr_zero_page_x();
                let old = self.rmw_read(a);
                let v = self.ror(old);
                self.write(a, v);
                6
            }
            0x6e => {
                let a = self.addr_absolute();
                let old = self.rmw_read(a);
                let v = self.ror(old);
                self.write(a, v);
                6
            }
            0x7e => {
                let a = self.addr_absolute_indexed_rmw();
                let old = self.rmw_read(a);
                let v = self.ror(old);
                self.write(a, v);
                7
            }
            0xe6 => {
                let a = self.addr_zero_page();
                let v = self.rmw_read(a).wrapping_add(1);
                self.write(a, v);
                self.set_zn(v);
                5
            }
            0xf6 => {
                let a = self.addr_zero_page_x();
                let v = self.rmw_read(a).wrapping_add(1);
                self.write(a, v);
                self.set_zn(v);
                6
            }
            0xee => {
                let a = self.addr_absolute();
                let v = self.rmw_read(a).wrapping_add(1);
                self.write(a, v);
                self.set_zn(v);
                6
            }
            0xfe => {
                let a = self.addr_absolute_indexed_rmw();
                let v = self.rmw_read(a).wrapping_add(1);
                self.write(a, v);
                self.set_zn(v);
                7
            }
            0xc6 => {
                let a = self.addr_zero_page();
                let v = self.rmw_read(a).wrapping_sub(1);
                self.write(a, v);
                self.set_zn(v);
                5
            }
            0xd6 => {
                let a = self.addr_zero_page_x();
                let v = self.rmw_read(a).wrapping_sub(1);
                self.write(a, v);
                self.set_zn(v);
                6
            }
            0xce => {
                let a = self.addr_absolute();
                let v = self.rmw_read(a).wrapping_sub(1);
                self.write(a, v);
                self.set_zn(v);
                6
            }
            0xde => {
                let a = self.addr_absolute_indexed_rmw();
                let v = self.rmw_read(a).wrapping_sub(1);
                self.write(a, v);
                self.set_zn(v);
                7
            }
            0x48 => {
                self.implied_cycle();
                self.push(self.cpu.a);
                3
            }
            0x68 => {
                self.implied_cycle();
                self.traced_read(0x0100 | u16::from(self.cpu.sp));
                let v = self.pop_traced();
                self.cpu.a = v;
                self.set_zn(v);
                4
            }
            0x08 => {
                self.implied_cycle();
                self.push(self.cpu.status | FLAG_BREAK | FLAG_UNUSED);
                3
            }
            0x28 => {
                self.implied_cycle();
                self.traced_read(0x0100 | u16::from(self.cpu.sp));
                self.cpu.status = (self.pop_traced() | FLAG_UNUSED) & !FLAG_BREAK;
                4
            }
            0x18 => {
                self.implied_cycle();
                self.cpu.status &= !FLAG_CARRY;
                2
            }
            0x38 => {
                self.implied_cycle();
                self.cpu.status |= FLAG_CARRY;
                2
            }
            0x58 => {
                self.implied_cycle();
                self.cpu.status &= !FLAG_INTERRUPT_DISABLE;
                2
            }
            0x78 => {
                self.implied_cycle();
                self.cpu.status |= FLAG_INTERRUPT_DISABLE;
                2
            }
            0xb8 => {
                self.implied_cycle();
                self.cpu.status &= !FLAG_OVERFLOW;
                2
            }
            0xd8 => {
                self.implied_cycle();
                self.cpu.status &= !FLAG_DECIMAL;
                2
            }
            0xf8 => {
                self.implied_cycle();
                self.cpu.status |= FLAG_DECIMAL;
                2
            }
            0x85 => {
                let address = self.addr_zero_page();
                self.write(address, self.cpu.a);
                3
            }
            0x95 => {
                let address = self.addr_zero_page_x();
                self.write(address, self.cpu.a);
                4
            }
            0x8d => {
                let address = self.addr_absolute();
                self.write(address, self.cpu.a);
                4
            }
            0x9d => {
                let address = self.addr_absolute_indexed_write(self.cpu.x);
                self.write(address, self.cpu.a);
                5
            }
            0x99 => {
                let address = self.addr_absolute_indexed_write(self.cpu.y);
                self.write(address, self.cpu.a);
                5
            }
            0x81 => {
                let address = self.addr_indexed_indirect();
                self.write(address, self.cpu.a);
                6
            }
            0x91 => {
                let address = self.addr_indirect_indexed_write();
                self.write(address, self.cpu.a);
                6
            }
            0x86 => {
                let a = self.addr_zero_page();
                self.write(a, self.cpu.x);
                3
            }
            0x96 => {
                let a = self.addr_zero_page_y();
                self.write(a, self.cpu.x);
                4
            }
            0x8e => {
                let a = self.addr_absolute();
                self.write(a, self.cpu.x);
                4
            }
            0x84 => {
                let a = self.addr_zero_page();
                self.write(a, self.cpu.y);
                3
            }
            0x94 => {
                let a = self.addr_zero_page_x();
                self.write(a, self.cpu.y);
                4
            }
            0x8c => {
                let a = self.addr_absolute();
                self.write(a, self.cpu.y);
                4
            }
            0xaa => {
                self.implied_cycle();
                self.cpu.x = self.cpu.a;
                self.set_zn(self.cpu.x);
                2
            }
            0x8a => {
                self.implied_cycle();
                self.cpu.a = self.cpu.x;
                self.set_zn(self.cpu.a);
                2
            }
            0xa8 => {
                self.implied_cycle();
                self.cpu.y = self.cpu.a;
                self.set_zn(self.cpu.y);
                2
            }
            0x98 => {
                self.implied_cycle();
                self.cpu.a = self.cpu.y;
                self.set_zn(self.cpu.a);
                2
            }
            0xba => {
                self.implied_cycle();
                self.cpu.x = self.cpu.sp;
                self.set_zn(self.cpu.x);
                2
            }
            0x9a => {
                self.implied_cycle();
                self.cpu.sp = self.cpu.x;
                2
            }
            0xe8 => {
                self.implied_cycle();
                self.cpu.x = self.cpu.x.wrapping_add(1);
                self.set_zn(self.cpu.x);
                2
            }
            0xca => {
                self.implied_cycle();
                self.cpu.x = self.cpu.x.wrapping_sub(1);
                self.set_zn(self.cpu.x);
                2
            }
            0xc8 => {
                self.implied_cycle();
                self.cpu.y = self.cpu.y.wrapping_add(1);
                self.set_zn(self.cpu.y);
                2
            }
            0x88 => {
                self.implied_cycle();
                self.cpu.y = self.cpu.y.wrapping_sub(1);
                self.set_zn(self.cpu.y);
                2
            }
            0x4c => {
                self.cpu.pc = self.fetch_word();
                3
            }
            0x6c => {
                self.cpu.pc = self.addr_jmp_indirect();
                5
            }
            0x20 => {
                // JSR fetches the low target byte before its stack writes, then
                // fetches the high target byte afterwards. This ordering is
                // observable when the stack overlaps the instruction stream.
                let target_lo = self.fetch_byte();
                let return_pc = self.cpu.pc;
                self.push((return_pc >> 8) as u8);
                self.push(return_pc as u8);
                let target_hi = self.fetch_byte();
                self.cpu.pc = u16::from_le_bytes([target_lo, target_hi]);
                6
            }
            0x60 => {
                let lo = self.pop();
                let hi = self.pop();
                self.cpu.pc = u16::from_le_bytes([lo, hi]).wrapping_add(1);
                6
            }
            0x10 => self.branch(self.cpu.status & FLAG_NEGATIVE == 0),
            0x30 => self.branch(self.cpu.status & FLAG_NEGATIVE != 0),
            0x50 => self.branch(self.cpu.status & FLAG_OVERFLOW == 0),
            0x70 => self.branch(self.cpu.status & FLAG_OVERFLOW != 0),
            0x90 => self.branch(self.cpu.status & FLAG_CARRY == 0),
            0xb0 => self.branch(self.cpu.status & FLAG_CARRY != 0),
            0xd0 => self.branch(self.cpu.status & FLAG_ZERO == 0),
            0xf0 => self.branch(self.cpu.status & FLAG_ZERO != 0),
            _ => {
                self.cpu.pc = instruction_pc;
                return Err(StepError::IllegalOpcode {
                    opcode,
                    pc: instruction_pc,
                });
            }
        };
        self.cycles = self.cycles.saturating_add(used);
        Ok(used as u8)
    }

    fn rmw_read(&mut self, address: u16) -> u8 {
        let value = self.traced_read(address);
        self.write(address, value);
        value
    }

    fn implied_cycle(&mut self) {
        self.traced_read(self.cpu.pc);
    }

    fn fetch_byte(&mut self) -> u8 {
        let value = self.traced_read(self.cpu.pc);
        self.cpu.pc = self.cpu.pc.wrapping_add(1);
        value
    }

    fn fetch_word(&mut self) -> u16 {
        let lo = self.fetch_byte();
        let hi = self.fetch_byte();
        u16::from_le_bytes([lo, hi])
    }

    fn addr_zero_page(&mut self) -> u16 {
        u16::from(self.fetch_byte())
    }

    fn addr_zero_page_x(&mut self) -> u16 {
        let base = self.fetch_byte();
        self.traced_read(u16::from(base));
        u16::from(base.wrapping_add(self.cpu.x))
    }

    fn addr_zero_page_y(&mut self) -> u16 {
        let base = self.fetch_byte();
        self.traced_read(u16::from(base));
        u16::from(base.wrapping_add(self.cpu.y))
    }

    fn addr_absolute(&mut self) -> u16 {
        self.fetch_word()
    }

    fn addr_absolute_x(&mut self) -> (u16, bool) {
        let base = self.fetch_word();
        let address = base.wrapping_add(u16::from(self.cpu.x));
        let crossed = (base & 0xff00) != (address & 0xff00);
        if crossed {
            let dummy = (base & 0xff00) | (address & 0x00ff);
            self.traced_read(dummy);
        }
        (address, crossed)
    }

    fn addr_absolute_indexed_write(&mut self, index: u8) -> u16 {
        let base = self.fetch_word();
        let address = base.wrapping_add(u16::from(index));
        let dummy = (base & 0xff00) | (address & 0x00ff);
        self.traced_read(dummy);
        address
    }

    fn addr_absolute_indexed_rmw(&mut self) -> u16 {
        self.addr_absolute_indexed_write(self.cpu.x)
    }

    fn addr_absolute_y(&mut self) -> (u16, bool) {
        let base = self.fetch_word();
        let address = base.wrapping_add(u16::from(self.cpu.y));
        let crossed = (base & 0xff00) != (address & 0xff00);
        if crossed {
            let dummy = (base & 0xff00) | (address & 0x00ff);
            self.traced_read(dummy);
        }
        (address, crossed)
    }

    fn read_zero_page_word(&mut self, pointer: u8) -> u16 {
        let lo = self.traced_read(u16::from(pointer));
        let hi = self.traced_read(u16::from(pointer.wrapping_add(1)));
        u16::from_le_bytes([lo, hi])
    }

    fn addr_indexed_indirect(&mut self) -> u16 {
        let base = self.fetch_byte();
        self.traced_read(u16::from(base));
        let pointer = base.wrapping_add(self.cpu.x);
        self.read_zero_page_word(pointer)
    }

    fn addr_indirect_indexed(&mut self) -> (u16, bool) {
        let pointer = self.fetch_byte();
        let base = self.read_zero_page_word(pointer);
        let address = base.wrapping_add(u16::from(self.cpu.y));
        let crossed = (base & 0xff00) != (address & 0xff00);
        if crossed {
            let dummy = (base & 0xff00) | (address & 0x00ff);
            self.traced_read(dummy);
        }
        (address, crossed)
    }

    fn addr_indirect_indexed_write(&mut self) -> u16 {
        let pointer = self.fetch_byte();
        let base = self.read_zero_page_word(pointer);
        let address = base.wrapping_add(u16::from(self.cpu.y));
        let dummy = (base & 0xff00) | (address & 0x00ff);
        self.traced_read(dummy);
        address
    }

    fn addr_jmp_indirect(&mut self) -> u16 {
        let pointer = self.fetch_word();
        let lo = self.read(pointer);
        // NMOS 6502/6510 wraps the high-byte fetch within the same page.
        let hi_address = (pointer & 0xff00) | u16::from((pointer as u8).wrapping_add(1));
        let hi = self.read(hi_address);
        u16::from_le_bytes([lo, hi])
    }

    fn load_a(&mut self, value: u8) {
        self.cpu.a = value;
        self.set_zn(value);
    }

    fn load_x(&mut self, value: u8) {
        self.cpu.x = value;
        self.set_zn(value);
    }

    fn load_y(&mut self, value: u8) {
        self.cpu.y = value;
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

    fn pop_traced(&mut self) -> u8 {
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        self.traced_read(0x0100 | u16::from(self.cpu.sp))
    }

    fn branch(&mut self, condition: bool) -> u64 {
        let offset = self.fetch_byte() as i8;
        if !condition {
            return 2;
        }
        let old = self.cpu.pc;
        self.traced_read(old);
        self.cpu.pc = self.cpu.pc.wrapping_add_signed(i16::from(offset));
        let crossed = (old & 0xff00) != (self.cpu.pc & 0xff00);
        if crossed {
            let dummy = (old & 0xff00) | (self.cpu.pc & 0x00ff);
            self.traced_read(dummy);
        }
        3 + u64::from(crossed)
    }

    fn interrupt(&mut self, vector: u16) {
        self.push((self.cpu.pc >> 8) as u8);
        self.push(self.cpu.pc as u8);
        self.push((self.cpu.status | FLAG_UNUSED) & !FLAG_BREAK);
        self.cpu.status |= FLAG_INTERRUPT_DISABLE;
        self.cpu.pc = u16::from_le_bytes([self.read(vector), self.read(vector.wrapping_add(1))]);
        self.cycles = self.cycles.saturating_add(7);
    }

    fn adc(&mut self, value: u8) {
        let carry_in = u16::from(self.cpu.status & FLAG_CARRY != 0);
        let a = self.cpu.a;
        let binary_sum = u16::from(a) + u16::from(value) + carry_in;
        let binary_result = binary_sum as u8;

        self.cpu.status &= !(FLAG_CARRY | FLAG_OVERFLOW | FLAG_NEGATIVE | FLAG_ZERO);

        if self.cpu.status & FLAG_DECIMAL != 0 {
            // NMOS 6502 decimal ADC exposes flags from different internal stages:
            // Z from the unadjusted binary sum, N/V after low-digit correction
            // but before the final high-digit correction, and C after that correction.
            if binary_result == 0 {
                self.cpu.status |= FLAG_ZERO;
            }

            let mut low = u16::from(a & 0x0f) + u16::from(value & 0x0f) + carry_in;
            if low > 9 {
                low += 6;
            }
            let carry_to_high = u16::from(low > 0x0f);
            let mut intermediate =
                u16::from(a & 0xf0) + u16::from(value & 0xf0) + (carry_to_high << 4);
            intermediate += low & 0x0f;

            if intermediate & 0x80 != 0 {
                self.cpu.status |= FLAG_NEGATIVE;
            }
            if (!(a ^ value) & (a ^ intermediate as u8) & 0x80) != 0 {
                self.cpu.status |= FLAG_OVERFLOW;
            }

            if intermediate > 0x9f {
                intermediate += 0x60;
            }
            if intermediate > 0xff {
                self.cpu.status |= FLAG_CARRY;
            }
            self.cpu.a = intermediate as u8;
        } else {
            if binary_sum > 0xff {
                self.cpu.status |= FLAG_CARRY;
            }
            if (!(a ^ value) & (a ^ binary_result) & 0x80) != 0 {
                self.cpu.status |= FLAG_OVERFLOW;
            }
            self.cpu.a = binary_result;
            self.set_zn(binary_result);
        }
    }

    fn sbc(&mut self, value: u8) {
        if self.cpu.status & FLAG_DECIMAL == 0 {
            self.adc(!value);
            return;
        }
        let a = self.cpu.a;
        let borrow = i16::from(self.cpu.status & FLAG_CARRY == 0);
        let binary = i16::from(a) - i16::from(value) - borrow;
        let result = binary as u8;
        self.cpu.status &= !(FLAG_CARRY | FLAG_OVERFLOW);
        if binary >= 0 {
            self.cpu.status |= FLAG_CARRY;
        }
        if ((a ^ result) & (a ^ value) & 0x80) != 0 {
            self.cpu.status |= FLAG_OVERFLOW;
        }
        let mut lo = i16::from(a & 0x0f) - i16::from(value & 0x0f) - borrow;
        let mut hi = i16::from(a >> 4) - i16::from(value >> 4);
        if lo < 0 {
            lo -= 6;
            hi -= 1;
        }
        if hi < 0 {
            hi -= 6;
        }
        self.cpu.a = (((hi as u8) << 4) & 0xf0) | ((lo as u8) & 0x0f);
        self.set_zn(result);
    }

    fn and_a(&mut self, value: u8) {
        self.cpu.a &= value;
        self.set_zn(self.cpu.a);
    }
    fn ora_a(&mut self, value: u8) {
        self.cpu.a |= value;
        self.set_zn(self.cpu.a);
    }
    fn eor_a(&mut self, value: u8) {
        self.cpu.a ^= value;
        self.set_zn(self.cpu.a);
    }

    fn cmp_a(&mut self, value: u8) {
        self.compare(self.cpu.a, value);
    }

    fn compare(&mut self, register: u8, value: u8) {
        let result = register.wrapping_sub(value);
        self.cpu.status &= !FLAG_CARRY;
        if register >= value {
            self.cpu.status |= FLAG_CARRY;
        }
        self.set_zn(result);
    }

    fn bit(&mut self, value: u8) {
        self.cpu.status &= !(FLAG_ZERO | FLAG_OVERFLOW | FLAG_NEGATIVE);
        if self.cpu.a & value == 0 {
            self.cpu.status |= FLAG_ZERO;
        }
        self.cpu.status |= value & (FLAG_OVERFLOW | FLAG_NEGATIVE);
    }

    fn asl(&mut self, value: u8) -> u8 {
        self.cpu.status = (self.cpu.status & !FLAG_CARRY) | ((value >> 7) & FLAG_CARRY);
        let result = value << 1;
        self.set_zn(result);
        result
    }

    fn lsr(&mut self, value: u8) -> u8 {
        self.cpu.status = (self.cpu.status & !FLAG_CARRY) | (value & FLAG_CARRY);
        let result = value >> 1;
        self.set_zn(result);
        result
    }

    fn rol(&mut self, value: u8) -> u8 {
        let carry_in = self.cpu.status & FLAG_CARRY;
        let carry_out = (value >> 7) & FLAG_CARRY;
        let result = (value << 1) | carry_in;
        self.cpu.status = (self.cpu.status & !FLAG_CARRY) | carry_out;
        self.set_zn(result);
        result
    }

    fn ror(&mut self, value: u8) -> u8 {
        let carry_in = (self.cpu.status & FLAG_CARRY) << 7;
        let carry_out = value & FLAG_CARRY;
        let result = (value >> 1) | carry_in;
        self.cpu.status = (self.cpu.status & !FLAG_CARRY) | carry_out;
        self.set_zn(result);
        result
    }

    fn set_zn(&mut self, value: u8) {
        self.cpu.status &= !(FLAG_ZERO | FLAG_NEGATIVE);
        if value == 0 {
            self.cpu.status |= FLAG_ZERO;
        }
        if value & 0x80 != 0 {
            self.cpu.status |= FLAG_NEGATIVE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Machine, StepError, FLAG_BREAK, FLAG_CARRY, FLAG_DECIMAL, FLAG_INTERRUPT_DISABLE,
        FLAG_NEGATIVE, FLAG_OVERFLOW, FLAG_UNUSED,
    };

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
        m.write(0xfffc, 0x34);
        m.write(0xfffd, 0x12);
        m.step_cycles(123);
        m.reset();
        assert_eq!(m.cpu().pc, 0x1234);
        assert_eq!(m.cycles(), 0);
    }

    #[test]
    fn immediate_loads_set_registers_flags_and_cycles() {
        let mut m = machine_with(&[0xa9, 0x80, 0xa2, 0x00, 0xa0, 0x42]);
        assert_eq!(m.step(), Ok(2));
        assert_eq!(m.cpu().a, 0x80);
        assert_eq!(m.cpu().status & 0x80, 0x80);
        assert_eq!(m.step(), Ok(2));
        assert_eq!(m.cpu().x, 0x00);
        assert_eq!(m.cpu().status & 0x02, 0x02);
        assert_eq!(m.step(), Ok(2));
        assert_eq!(m.cpu().y, 0x42);
        assert_eq!(m.cycles(), 6);
    }

    #[test]
    fn sta_absolute_and_jmp_absolute_work() {
        let mut m = machine_with(&[0xa9, 0x41, 0x8d, 0x00, 0x04, 0x4c, 0x01, 0x08]);
        m.step().unwrap();
        m.step().unwrap();
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
        assert_eq!(
            m.step(),
            Err(StepError::IllegalOpcode {
                opcode: 0x02,
                pc: 0x0801
            })
        );
        assert_eq!(m.cpu().pc, 0x0801);
        assert_eq!(m.cycles(), 0);
    }

    #[test]
    fn jsr_and_rts_round_trip_through_stack() {
        let mut m = machine_with(&[0x20, 0x07, 0x08, 0xea, 0xea, 0xea, 0x60]);
        assert_eq!(m.step(), Ok(6));
        assert_eq!(m.cpu().pc, 0x0807);
        assert_eq!(m.cpu().sp, 0xfb);
        assert_eq!(m.step(), Ok(6));
        assert_eq!(m.cpu().pc, 0x0804);
        assert_eq!(m.cpu().sp, 0xfd);
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
        m.step().unwrap();
        m.step().unwrap();
        assert_eq!(m.cpu().a, 0x80);
        assert_ne!(m.cpu().status & FLAG_OVERFLOW, 0);
        m.step().unwrap();
        assert_eq!(m.cpu().a, 0x7e);
    }

    #[test]
    fn lda_addressing_modes_read_expected_values() {
        let mut m = machine_with(&[
            0xa2, 0x02, 0xa5, 0x10, 0xb5, 0x10, 0xad, 0x00, 0x20, 0xbd, 0xff, 0x20,
        ]);
        m.write(0x0010, 0x11);
        m.write(0x0012, 0x22);
        m.write(0x2000, 0x33);
        m.write(0x2101, 0x44);
        m.step().unwrap();
        assert_eq!(m.step(), Ok(3));
        assert_eq!(m.cpu().a, 0x11);
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.cpu().a, 0x22);
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.cpu().a, 0x33);
        assert_eq!(m.step(), Ok(5));
        assert_eq!(m.cpu().a, 0x44);
    }

    #[test]
    fn sta_indexed_modes_write_expected_addresses() {
        let mut m = machine_with(&[0xa9, 0x5a, 0xa2, 0x02, 0x95, 0xfe, 0x9d, 0xff, 0x20]);
        m.step().unwrap();
        m.step().unwrap();
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.read(0x0000), 0x5a);
        assert_eq!(m.step(), Ok(5));
        assert_eq!(m.read(0x2101), 0x5a);
    }

    #[test]
    fn indirect_addressing_wraps_zero_page_and_counts_page_cross() {
        let mut m = machine_with(&[0xa2, 0x01, 0xa1, 0xff, 0xa0, 0x01, 0xb1, 0x10]);
        m.write(0x0000, 0x00);
        m.write(0x0001, 0x20);
        m.write(0x2000, 0x66);
        m.write(0x0010, 0xff);
        m.write(0x0011, 0x20);
        m.write(0x2100, 0x77);
        m.step().unwrap();
        assert_eq!(m.step(), Ok(6));
        assert_eq!(m.cpu().a, 0x66);
        m.step().unwrap();
        assert_eq!(m.step(), Ok(6));
        assert_eq!(m.cpu().a, 0x77);
    }

    #[test]
    fn jmp_indirect_preserves_nmos_page_wrap_quirk() {
        let mut m = machine_with(&[0x6c, 0xff, 0x20]);
        m.write(0x20ff, 0x34);
        m.write(0x2000, 0x12);
        m.write(0x2100, 0x99);
        assert_eq!(m.step(), Ok(5));
        assert_eq!(m.cpu().pc, 0x1234);
    }

    #[test]
    fn transfers_and_index_updates_set_expected_state() {
        let mut m = machine_with(&[0xa9, 0x7f, 0xaa, 0xe8, 0x8a, 0xa8, 0x88, 0xca]);
        for _ in 0..8 {
            m.step().unwrap();
        }
        assert_eq!(m.cpu().a, 0x80);
        assert_eq!(m.cpu().x, 0x7f);
        assert_eq!(m.cpu().y, 0x7f);
    }

    #[test]
    fn logical_compare_and_bit_operations_update_flags() {
        let mut m = machine_with(&[
            0xa9, 0xf0, 0x29, 0x0f, 0x09, 0x80, 0x49, 0xff, 0xc9, 0x7f, 0x24, 0x10,
        ]);
        m.write(0x0010, 0xc0);
        for _ in 0..6 {
            m.step().unwrap();
        }
        assert_eq!(m.cpu().a, 0x7f);
        assert_ne!(m.cpu().status & FLAG_CARRY, 0);
        assert_ne!(m.cpu().status & FLAG_OVERFLOW, 0);
        assert_ne!(m.cpu().status & FLAG_NEGATIVE, 0);
    }

    #[test]
    fn accumulator_shifts_rotate_through_carry() {
        let mut m = machine_with(&[0xa9, 0x81, 0x0a, 0x6a, 0x4a, 0x38, 0x2a]);
        for _ in 0..7 {
            m.step().unwrap();
        }
        assert_eq!(m.cpu().a, 0x81);
    }

    #[test]
    fn flag_instructions_toggle_control_flags() {
        let mut m = machine_with(&[0x38, 0x78, 0xf8, 0x18, 0x58, 0xd8]);
        for _ in 0..3 {
            m.step().unwrap();
        }
        assert_ne!(m.cpu().status & FLAG_CARRY, 0);
        assert_ne!(m.cpu().status & FLAG_INTERRUPT_DISABLE, 0);
        assert_ne!(m.cpu().status & FLAG_DECIMAL, 0);
        for _ in 0..3 {
            m.step().unwrap();
        }
        assert_eq!(
            m.cpu().status & (FLAG_CARRY | FLAG_INTERRUPT_DISABLE | FLAG_DECIMAL),
            0
        );
    }

    #[test]
    fn memory_rmw_and_inc_dec_modify_zero_page() {
        let mut m = machine_with(&[0x06, 0x10, 0x46, 0x10, 0xe6, 0x10, 0xc6, 0x10]);
        m.write(0x0010, 0x81);
        for _ in 0..4 {
            assert_eq!(m.step(), Ok(5));
        }
        assert_eq!(m.read(0x0010), 0x01);
    }

    #[test]
    fn stack_accumulator_and_status_round_trip() {
        let mut m = machine_with(&[0xa9, 0x42, 0x48, 0xa9, 0x00, 0x68, 0x38, 0x08, 0x18, 0x28]);
        for _ in 0..10 {
            m.step().unwrap();
        }
        assert_eq!(m.cpu().a, 0x42);
        assert_ne!(m.cpu().status & FLAG_CARRY, 0);
        assert_eq!(m.cpu().status & FLAG_BREAK, 0);
        assert_ne!(m.cpu().status & FLAG_UNUSED, 0);
    }

    #[test]
    fn all_branch_conditions_have_decode_paths() {
        let programs = [
            [0x10, 0x00],
            [0x30, 0x00],
            [0x50, 0x00],
            [0x70, 0x00],
            [0x90, 0x00],
            [0xb0, 0x00],
            [0xd0, 0x00],
            [0xf0, 0x00],
        ];
        for program in programs {
            let mut m = machine_with(&program);
            assert!(m.step().is_ok());
        }
    }

    #[test]
    fn opcode_catalog_tracks_all_256_values() {
        for opcode in 0u8..=u8::MAX {
            assert_eq!(super::opcode_info(opcode).opcode, opcode);
        }
        assert_eq!(super::OFFICIAL_OPCODE_COUNT, 151);
    }

    #[test]
    fn implemented_decode_is_marked_in_catalog() {
        let implemented = [
            0xea, 0xa9, 0xa5, 0xb5, 0xad, 0xbd, 0xb9, 0xa1, 0xb1, 0x69, 0xe9, 0x20, 0x60, 0x6c,
        ];
        for opcode in implemented {
            assert!(
                super::opcode_info(opcode).implemented,
                "opcode {opcode:02x}"
            );
        }
    }

    #[test]
    fn ldx_ldy_and_stx_sty_addressing_modes_work() {
        let mut m = machine_with(&[
            0xa0, 0x02, 0xb6, 0xfe, 0x96, 0x10, 0xa2, 0x03, 0xb4, 0xfd, 0x94, 0x20,
        ]);
        m.write(0x0000, 0x44);
        m.step().unwrap();
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.cpu().x, 0x44);
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.read(0x0012), 0x44);
        m.step().unwrap();
        m.write(0x0000, 0x55);
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.cpu().y, 0x55);
        assert_eq!(m.step(), Ok(4));
        assert_eq!(m.read(0x0023), 0x55);
    }

    #[test]
    fn decimal_adc_and_sbc_handle_bcd() {
        let mut m = machine_with(&[0xf8, 0x18, 0xa9, 0x45, 0x69, 0x55, 0x38, 0xe9, 0x01]);
        for _ in 0..5 {
            m.step().unwrap();
        }
        assert_eq!(m.cpu().a, 0x00);
        assert_ne!(m.cpu().status & FLAG_CARRY, 0);
        m.step().unwrap();
        m.step().unwrap();
        assert_eq!(m.cpu().a, 0x99);
    }

    #[test]
    fn irq_and_nmi_use_vectors_and_stack() {
        let mut m = machine_with(&[0x58, 0xea]);
        m.write(0xfffe, 0x00);
        m.write(0xffff, 0x20);
        m.write(0xfffa, 0x00);
        m.write(0xfffb, 0x30);
        m.step().unwrap();
        assert!(m.irq());
        assert_eq!(m.cpu().pc, 0x2000);
        m.nmi();
        assert_eq!(m.cpu().pc, 0x3000);
    }

    #[test]
    fn instruction_execution_is_deterministic() {
        let program = [0xa9, 0x2a, 0x8d, 0x00, 0x04, 0xea];
        let mut a = machine_with(&program);
        let mut b = machine_with(&program);
        for _ in 0..3 {
            a.step().unwrap();
            b.step().unwrap();
        }
        assert_eq!(a.cpu(), b.cpu());
        assert_eq!(a.cycles(), b.cycles());
        assert_eq!(a.read(0x0400), b.read(0x0400));
    }
}
