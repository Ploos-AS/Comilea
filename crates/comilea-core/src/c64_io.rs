#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C64IoTarget {
    Vic { register: u8 },
    Sid { register: u8 },
    ColorRam { offset: u16 },
    Cia1 { register: u8 },
    Cia2 { register: u8 },
    Io1 { offset: u8 },
    Io2 { offset: u8 },
}

#[must_use]
pub fn decode(address: u16) -> Option<C64IoTarget> {
    match address {
        0xd000..=0xd3ff => Some(C64IoTarget::Vic {
            register: (address & 0x003f) as u8,
        }),
        0xd400..=0xd7ff => Some(C64IoTarget::Sid {
            register: (address & 0x001f) as u8,
        }),
        0xd800..=0xdbff => Some(C64IoTarget::ColorRam {
            offset: address - 0xd800,
        }),
        0xdc00..=0xdcff => Some(C64IoTarget::Cia1 {
            register: (address & 0x000f) as u8,
        }),
        0xdd00..=0xddff => Some(C64IoTarget::Cia2 {
            register: (address & 0x000f) as u8,
        }),
        0xde00..=0xdeff => Some(C64IoTarget::Io1 {
            offset: (address & 0x00ff) as u8,
        }),
        0xdf00..=0xdfff => Some(C64IoTarget::Io2 {
            offset: (address & 0x00ff) as u8,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, C64IoTarget};

    #[test]
    fn decodes_c64_io_regions_and_mirrors() {
        assert_eq!(decode(0xd000), Some(C64IoTarget::Vic { register: 0 }));
        assert_eq!(decode(0xd040), Some(C64IoTarget::Vic { register: 0 }));
        assert_eq!(decode(0xd41f), Some(C64IoTarget::Sid { register: 0x1f }));
        assert_eq!(decode(0xd43f), Some(C64IoTarget::Sid { register: 0x1f }));
        assert_eq!(decode(0xd800), Some(C64IoTarget::ColorRam { offset: 0 }));
        assert_eq!(decode(0xdbff), Some(C64IoTarget::ColorRam { offset: 0x3ff }));
        assert_eq!(decode(0xdc10), Some(C64IoTarget::Cia1 { register: 0 }));
        assert_eq!(decode(0xdd1f), Some(C64IoTarget::Cia2 { register: 0x0f }));
        assert_eq!(decode(0xde42), Some(C64IoTarget::Io1 { offset: 0x42 }));
        assert_eq!(decode(0xdf42), Some(C64IoTarget::Io2 { offset: 0x42 }));
        assert_eq!(decode(0xcfff), None);
    }
}
