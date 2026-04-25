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
