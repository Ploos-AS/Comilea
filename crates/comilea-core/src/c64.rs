use crate::{Machine, ProcessorPort6510};

const BASIC_START: u16 = 0xa000;
const BASIC_END: u16 = 0xbfff;
const IO_START: u16 = 0xd000;
const IO_END: u16 = 0xdfff;
const KERNAL_START: u16 = 0xe000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C64BankLines {
    pub loram: bool,
    pub hiram: bool,
    pub charen: bool,
}

/// Commodore 64 machine profile.
///
/// C64-specific hardware belongs here rather than in the reusable CPU execution
/// core. ROM contents are supplied by the caller and are never bundled.
#[derive(Clone)]
pub struct C64 {
    cpu: Machine,
    processor_port: ProcessorPort6510,
    basic_rom: Box<[u8]>,
    kernal_rom: Box<[u8]>,
    char_rom: Box<[u8]>,
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
            basic_rom: vec![0; 0x2000].into_boxed_slice(),
            kernal_rom: vec![0; 0x2000].into_boxed_slice(),
            char_rom: vec![0; 0x1000].into_boxed_slice(),
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
    pub fn bank_lines(&self) -> C64BankLines {
        let value = self.processor_port.read_data();
        C64BankLines {
            loram: value & 0x01 != 0,
            hiram: value & 0x02 != 0,
            charen: value & 0x04 != 0,
        }
    }

    /// Loads a caller-supplied C64 BASIC ROM.\n    ///\n    /// # Errors\n    /// Returns an error unless the image is exactly 8192 bytes.\n    pub fn load_basic_rom(&mut self, rom: &[u8]) -> Result<(), &'static str> {
        if rom.len() != 0x2000 {
            return Err("BASIC ROM must be exactly 8192 bytes");
        }
        self.basic_rom.copy_from_slice(rom);
        Ok(())
    }

    /// Loads a caller-supplied C64 KERNAL ROM.\n    ///\n    /// # Errors\n    /// Returns an error unless the image is exactly 8192 bytes.\n    pub fn load_kernal_rom(&mut self, rom: &[u8]) -> Result<(), &'static str> {
        if rom.len() != 0x2000 {
            return Err("KERNAL ROM must be exactly 8192 bytes");
        }
        self.kernal_rom.copy_from_slice(rom);
        Ok(())
    }

    /// Loads a caller-supplied C64 character ROM.\n    ///\n    /// # Errors\n    /// Returns an error unless the image is exactly 4096 bytes.\n    pub fn load_char_rom(&mut self, rom: &[u8]) -> Result<(), &'static str> {
        if rom.len() != 0x1000 {
            return Err("character ROM must be exactly 4096 bytes");
        }
        self.char_rom.copy_from_slice(rom);
        Ok(())
    }

    /// Reads the current C64 CPU-visible memory map.
    ///
    /// I/O space currently returns zero until the C64 device bus is connected.
    #[must_use]
    pub fn read(&self, address: u16) -> u8 {
        if let Some(value) = self.read_cpu_port(address) {
            return value;
        }
        let lines = self.bank_lines();
        match address {
            BASIC_START..=BASIC_END if lines.loram && lines.hiram => {
                self.basic_rom[usize::from(address - BASIC_START)]
            }
            IO_START..=IO_END if (lines.loram || lines.hiram) && lines.charen => 0,
            IO_START..=IO_END if (lines.loram || lines.hiram) && !lines.charen => {
                self.char_rom[usize::from(address - IO_START)]
            }
            KERNAL_START..=u16::MAX if lines.hiram => {
                self.kernal_rom[usize::from(address - KERNAL_START)]
            }
            _ => self.cpu.read(address),
        }
    }

    /// Writes the C64 CPU-visible address space. ROM overlays do not block RAM
    /// writes underneath them; I/O writes are reserved for the future device bus.
    pub fn write(&mut self, address: u16, value: u8) {
        if self.write_cpu_port(address, value) {
            return;
        }
        let lines = self.bank_lines();
        if (IO_START..=IO_END).contains(&address) && (lines.loram || lines.hiram) && lines.charen {
            return;
        }
        self.cpu.write(address, value);
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

    fn mapped_c64() -> C64 {
        let mut c64 = C64::new();
        c64.load_basic_rom(&vec![0xba; 0x2000]).unwrap();
        c64.load_kernal_rom(&vec![0xe0; 0x2000]).unwrap();
        c64.load_char_rom(&vec![0xd0; 0x1000]).unwrap();
        c64.write(0x0000, 0x07);
        c64
    }

    #[test]
    fn c64_owns_6510_processor_port() {
        let mut c64 = C64::new();
        c64.set_processor_port_input(0x55);
        assert!(c64.write_cpu_port(0x0001, 0xa3));
        assert!(c64.write_cpu_port(0x0000, 0xf0));
        assert_eq!(c64.read_cpu_port(0x0000), Some(0xf0));
        assert_eq!(c64.read_cpu_port(0x0001), Some(0xa5));
    }

    #[test]
    fn all_three_bank_lines_expose_basic_io_and_kernal() {
        let mut c64 = mapped_c64();
        c64.write(0x0001, 0x07);
        assert_eq!(c64.read(0xa000), 0xba);
        assert_eq!(c64.read(0xd000), 0x00);
        assert_eq!(c64.read(0xe000), 0xe0);
    }

    #[test]
    fn charen_low_exposes_character_rom() {
        let mut c64 = mapped_c64();
        c64.write(0x0001, 0x03);
        assert_eq!(c64.read(0xd000), 0xd0);
    }

    #[test]
    fn banking_out_rom_reveals_underlying_ram() {
        let mut c64 = mapped_c64();
        c64.write(0x0001, 0x00);
        c64.write(0xa000, 0x11);
        c64.write(0xd000, 0x22);
        c64.write(0xe000, 0x33);
        assert_eq!(c64.read(0xa000), 0x11);
        assert_eq!(c64.read(0xd000), 0x22);
        assert_eq!(c64.read(0xe000), 0x33);
    }

    #[test]
    fn writes_under_rom_are_preserved_in_ram() {
        let mut c64 = mapped_c64();
        c64.write(0x0001, 0x07);
        c64.write(0xa000, 0x42);
        assert_eq!(c64.read(0xa000), 0xba);
        c64.write(0x0001, 0x00);
        assert_eq!(c64.read(0xa000), 0x42);
    }

    #[test]
    fn rom_loader_rejects_wrong_sizes() {
        let mut c64 = C64::new();
        assert!(c64.load_basic_rom(&[0; 1]).is_err());
        assert!(c64.load_kernal_rom(&[0; 1]).is_err());
        assert!(c64.load_char_rom(&[0; 1]).is_err());
    }
}
