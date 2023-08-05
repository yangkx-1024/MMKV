use crc::{Crc, CRC_8_AUTOSAR};

pub const CRC8: Crc<u8> = Crc::<u8>::new(&CRC_8_AUTOSAR);

#[cfg(test)]
mod tests {
    use crate::core::crc::CRC8;

    #[test]
    fn test_crc32() {
        let mut digest = CRC8.digest();
        digest.update(b"123456789");
        assert_eq!(digest.finalize(), 0xdf);
    }
}