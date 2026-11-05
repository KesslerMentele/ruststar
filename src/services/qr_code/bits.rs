use bitvec::prelude::*;

/// Appends the low `bit_length` bits of `value` (most-significant bit first, as the QR spec
/// requires) onto `bits`. The storage itself is `Lsb0`; that only affects how bits are packed
/// into each backing byte, not the logical order they're appended in. `value` must already fit in `bit_length` bits — a programmer error, not user input, so it's
/// only checked in debug builds, not propagated as a `Result`.
pub(crate) fn write_bits(bits: &mut BitVec<u8, Lsb0>, value: u32, bit_length: usize) {
    debug_assert!((1..=32).contains(&bit_length), "bit_length must be in 1..=32");
    debug_assert!(
        bit_length == 32 || value < (1u32 << bit_length),
        "value {value} does not fit in {bit_length} bits"
    );
    bits.extend((0..bit_length).rev().map(|i| (value >> i) & 1 == 1));
}

/// Packs a byte-aligned, MSB-first-per-byte bitstream into bytes. Deliberately not
/// `chunk.load_be()`: on `Lsb0` storage, `load_be`/`load_le` both number bits by the storage's
/// own LSB-first convention, not by logical push order, so they'd return each byte bit-reversed.
/// Folding over `.iter()` reads logical order directly and sidesteps that entirely.
pub(crate) fn bits_to_bytes(bits: &BitSlice<u8, Lsb0>) -> Vec<u8> {
    debug_assert_eq!(bits.len() % 8, 0, "bits must be byte-aligned");
    bits.chunks(8).map(|chunk| chunk.iter().fold(0u8, |acc, bit| (acc << 1) | *bit as u8)).collect()
}

/// The inverse of `bits_to_bytes`: appends `bytes` onto a fresh bitstream, each byte MSB-first.
pub(crate) fn bytes_to_bits(bytes: &[u8]) -> BitVec<u8, Lsb0> {
    let mut bits = BitVec::with_capacity(bytes.len() * 8);
    for &byte in bytes {
        write_bits(&mut bits, byte as u32, 8);
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_bits_appends_low_bits_msb_first() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 0b101, 3);
        assert_eq!(bits, bits![1, 0, 1]);
    }

    #[test]
    fn write_bits_pads_to_the_requested_length() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 1, 4);
        assert_eq!(bits, bits![0, 0, 0, 1]);
    }

    #[test]
    fn write_bits_accumulates_across_calls() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 0b11, 2);
        write_bits(&mut bits, 0b0, 1);
        write_bits(&mut bits, 0b101, 3);
        assert_eq!(bits, bits![1, 1, 0, 1, 0, 1]);
    }

    #[test]
    #[should_panic]
    fn write_bits_rejects_a_value_that_does_not_fit() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 0b1000, 3);
    }

    #[test]
    fn bytes_to_bits_and_back_round_trips() {
        let original = [0x10, 0x20, 0xEC, 0x00, 0xFF];
        assert_eq!(bits_to_bytes(&bytes_to_bits(&original)), original);
    }

    #[test]
    fn bytes_to_bits_is_msb_first_per_byte() {
        assert_eq!(bytes_to_bits(&[0b10110010]), bits![1, 0, 1, 1, 0, 0, 1, 0]);
    }
}
