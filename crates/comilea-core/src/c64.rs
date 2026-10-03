use crate::{Machine, ProcessorPort6510};

/// Commodore 64 machine profile.
///
/// C64-specific hardware belongs here rather than in the reusable CPU execution
/// core. The profile starts with the MOS 6510 processor port; PLA banking,
/// VIC-II, SID and CIA devices are added behind this machine boundary.
#[derive(Clone)]
pub struct C64 {
    cpu: Machine,
    processor_port: ProcessorPort6510,
}

impl Default for C64 {
    fn default() -> Self {
        Self::new()
    }
}

impl C64 {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cpu: Machine::new(),
            processor_port: ProcessorPort6510::default(),
        }
    }

    #[must_use]
    pub const fn cpu(&self) -> &Machine {
        &self.cpu
    }

    pub fn cpu_mut(&mut self) -> &mut Machine {
        &mut self.cpu
    }

    #[must_use]
    pub const fn processor_port(&self) -> &ProcessorPort6510 {
        &self.processor_port
    }

    pub fn set_processor_port_input(&mut self, value: u8) {
        self.processor_port.set_input(value);
    }

    #[must_use]
    pub fn read_cpu_port(&self, address: u16) -> Option<u8> {
        match address {
            0x0000 => Some(self.processor_port.ddr()),
            0x0001 => Some(self.processor_port.read_data()),
            _ => None,
        }
    }

    pub fn write_cpu_port(&mut self, address: u16, value: u8) -> bool {
        match address {
            0x0000 => self.processor_port.write_ddr(value),
            0x0001 => self.processor_port.write_data(value),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::C64;

    #[test]
    fn c64_owns_6510_processor_port() {
        let mut c64 = C64::new();
        c64.set_processor_port_input(0x55);
        assert!(c64.write_cpu_port(0x0001, 0xa3));
        assert!(c64.write_cpu_port(0x0000, 0xf0));
        assert_eq!(c64.read_cpu_port(0x0000), Some(0xf0));
        assert_eq!(c64.read_cpu_port(0x0001), Some(0xa5));
        assert_eq!(c64.read_cpu_port(0x0002), None);
    }

    #[test]
    fn generic_cpu_memory_is_not_the_c64_port() {
        let mut c64 = C64::new();
        c64.cpu_mut().write(0x0000, 0x12);
        c64.cpu_mut().write(0x0001, 0x34);
        assert_eq!(c64.cpu().read(0x0000), 0x12);
        assert_eq!(c64.cpu().read(0x0001), 0x34);
        assert_eq!(c64.read_cpu_port(0x0000), Some(0x00));
        assert_eq!(c64.read_cpu_port(0x0001), Some(0xff));
    }
}
