//! Bit-level write helpers at arbitrary byte+bit offsets.
//!
//! Ported from mp3rgain `src/frame.rs` (MIT).

/// Write an 8-bit value at a bit-unaligned position (raw byte + bit offset).
pub(crate) fn write_bits_u8(data: &mut [u8], byte_offset: usize, bit_offset: u8, value: u8) {
    if byte_offset >= data.len() {
        return;
    }

    if bit_offset == 0 {
        data[byte_offset] = value;
    } else if byte_offset + 1 < data.len() {
        let mask_high = 0xFFu8 << (8 - bit_offset);
        let mask_low = 0xFFu8 >> bit_offset;

        data[byte_offset] = (data[byte_offset] & mask_high) | (value >> bit_offset);
        data[byte_offset + 1] = (data[byte_offset + 1] & mask_low) | (value << (8 - bit_offset));
    } else {
        let mask_high = 0xFFu8 << (8 - bit_offset);
        data[byte_offset] = (data[byte_offset] & mask_high) | (value >> bit_offset);
    }
}

/// Saturating gain adjustment, clamped to the 0..=255 `global_gain` range.
pub(crate) fn adjust_gain_value(current: u8, steps: i32) -> u8 {
    (current as i32 + steps).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_bits_u8_for_test(data: &[u8], byte_offset: usize, bit_offset: u8) -> u8 {
        if bit_offset == 0 {
            data[byte_offset]
        } else if byte_offset + 1 < data.len() {
            (data[byte_offset] << bit_offset) | (data[byte_offset + 1] >> (8 - bit_offset))
        } else {
            data[byte_offset] << bit_offset
        }
    }

    #[test]
    fn write_bits_u8_overwrites_aligned_byte() {
        let mut data = [0xAA, 0x55];

        write_bits_u8(&mut data, 0, 0, 0x3C);

        assert_eq!(data, [0x3C, 0x55]);
    }

    #[test]
    fn write_bits_u8_preserves_surrounding_bits_when_unaligned() {
        let mut data = [0b1010_1010, 0b0101_0101];
        let original = data;

        write_bits_u8(&mut data, 0, 3, 0b1100_0011);

        assert_eq!(read_bits_u8_for_test(&data, 0, 3), 0b1100_0011);
        assert_eq!(data[0] & 0b1110_0000, original[0] & 0b1110_0000);
        assert_eq!(data[1] & 0b0000_0111, original[1] & 0b0000_0111);
    }

    #[test]
    fn write_bits_u8_handles_trailing_partial_byte() {
        let mut data = [0b1110_0001];

        write_bits_u8(&mut data, 0, 5, 0b1010_0000);

        assert_eq!(data[0] & 0b1111_1000, 0b1110_0000);
        assert_eq!(data[0] & 0b0000_0111, 0b0000_0101);
    }

    #[test]
    fn adjust_gain_value_saturates_to_aac_gain_range() {
        assert_eq!(adjust_gain_value(120, 5), 125);
        assert_eq!(adjust_gain_value(2, -10), 0);
        assert_eq!(adjust_gain_value(250, 20), 255);
    }
}
