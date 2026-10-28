use bitvec::prelude::*;

/// Appends the low `bit_length` bits of `value` (most-significant bit first) onto `bits`.
/// `value` must already fit in `bit_length` bits — a programmer error, not user input, so it's
/// only checked in debug builds, not propagated as a `Result`.
pub(crate) fn write_bits(bits: &mut BitVec<u8, Msb0>, value: u32, bit_length: usize) {
    debug_assert!((1..=32).contains(&bit_length), "bit_length must be in 1..=32");
    debug_assert!(
        bit_length == 32 || value < (1u32 << bit_length),
        "value {value} does not fit in {bit_length} bits"
    );
    bits.extend_from_bitslice(&value.view_bits::<Msb0>()[32 - bit_length..]);
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
}
