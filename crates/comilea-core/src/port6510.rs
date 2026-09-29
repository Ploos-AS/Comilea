#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessorPort6510 {
    ddr: u8,
    data: u8,
    input: u8,
}

impl Default for ProcessorPort6510 {
    fn default() -> Self {
        Self {
            ddr: 0,
            data: 0,
            input: 0xff,
        }
    }
}

impl ProcessorPort6510 {
    #[must_use]
    pub const fn ddr(&self) -> u8 {
        self.ddr
    }

    #[must_use]
    pub const fn data_latch(&self) -> u8 {
        self.data
    }

    #[must_use]
    pub const fn input(&self) -> u8 {
        self.input
    }

    pub fn set_input(&mut self, value: u8) {
        self.input = value;
    }

    pub fn write_ddr(&mut self, value: u8) {
        self.ddr = value;
    }

    pub fn write_data(&mut self, value: u8) {
        self.data = value;
    }

    #[must_use]
    pub const fn read_data(&self) -> u8 {
        (self.data & self.ddr) | (self.input & !self.ddr)
    }

    #[must_use]
    pub const fn output(&self) -> u8 {
        self.data & self.ddr
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessorPort6510;

    #[test]
    fn inputs_default_high() {
        assert_eq!(ProcessorPort6510::default().read_data(), 0xff);
    }

    #[test]
    fn ddr_selects_latch_or_external_input_per_bit() {
        let mut port = ProcessorPort6510::default();
        port.set_input(0x55);
        port.write_data(0xa3);
        port.write_ddr(0xf0);
        assert_eq!(port.read_data(), 0xa5);
        assert_eq!(port.output(), 0xa0);
    }

    #[test]
    fn data_latch_is_preserved_while_pin_is_input() {
        let mut port = ProcessorPort6510::default();
        port.write_data(0x01);
        port.set_input(0x00);
        assert_eq!(port.read_data() & 1, 0);
        port.write_ddr(0x01);
        assert_eq!(port.read_data() & 1, 1);
    }
}
