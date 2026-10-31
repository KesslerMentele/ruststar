// inputs:
//  - &str to encode
//  - EC_MODE to use ( 0 - 3 )
//  - the version number ( 1-40 )
//
// Create Data Bits:
//  - determine the correct encoding mode for input str.
//  - write the first 4 bit type code of the encoding mode into a bitvec.
//  - write the character count bit length into the bitvec.
//  - write the encoded data into the bitvec.
//  - round off the bitvec with 0s to the nearest byte.
//  - pad the remaining space of the code with alternating [236, 17] bytes.
//
// Encode Data Bits:
//  - Generate Error correction bits.
//  - Interleave data and return a bitvec of interleaved EC and data blocks

use bitvec::prelude::*;
use encoding_rs::{EncoderResult, SHIFT_JIS};
use super::bits::write_bits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncodeError {
    /// A character isn't part of the character set the mode being encoded requires.
    InvalidChar(char),
    /// A double-byte Shift-JIS code point outside the two ranges QR Kanji mode can represent.
    KanjiOutOfRange(u16),
    /// An ECI designator outside the 0..=999999 range the three encoding lengths can hold.
    EciOutOfRange(u32),
    /// A structured-append sequence number or total-symbol count outside 0..=15.
    StructuredAppendOutOfRange,
}

/// Maps a character to its value in QR's 45-character alphanumeric set, or `None` if it isn't
/// a member of that set.
pub(crate) fn alphanumeric_value(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        'A'..='Z' => Some(10 + (c as u8 - b'A')),
        ' ' => Some(36),
        '$' => Some(37),
        '%' => Some(38),
        '*' => Some(39),
        '+' => Some(40),
        '-' => Some(41),
        '.' => Some(42),
        '/' => Some(43),
        ':' => Some(44),
        _ => None,
    }
}

/// Encodes `data` (ASCII digits only) as QR Numeric mode data bits: digits are grouped by
/// three, each group packed as 10 bits, except a trailing group of two (7 bits) or one (4 bits).
pub(crate) fn encode_numeric(data: &str) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    let digits = data
        .chars()
        .map(|c| c.to_digit(10).ok_or(EncodeError::InvalidChar(c)))
        .collect::<Result<Vec<_>, _>>()?;

    let mut bits = BitVec::new();
    for group in digits.chunks(3) {
        let value = group.iter().fold(0u32, |acc, &digit| acc * 10 + digit);
        let bit_length = match group.len() {
            3 => 10,
            2 => 7,
            1 => 4,
            _ => unreachable!("chunks(3) never yields an empty or larger-than-3 group"),
        };
        write_bits(&mut bits, value, bit_length);
    }
    Ok(bits)
}

/// Encodes `data` (QR's 45-character alphanumeric set) as QR Alphanumeric mode data bits:
/// characters are grouped in pairs, each pair packed as `first * 45 + second` in 11 bits,
/// except a trailing single character (6 bits).
pub(crate) fn encode_alphanumeric(data: &str) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    let values = data
        .chars()
        .map(|c| alphanumeric_value(c).ok_or(EncodeError::InvalidChar(c)))
        .collect::<Result<Vec<_>, _>>()?;

    let mut bits = BitVec::new();
    for pair in values.chunks(2) {
        match pair {
            &[first, second] => write_bits(&mut bits, first as u32 * 45 + second as u32, 11),
            &[single] => write_bits(&mut bits, single as u32, 6),
            _ => unreachable!("chunks(2) never yields an empty or larger-than-2 group"),
        }
    }
    Ok(bits)
}

/// Maps a character to its double-byte Shift-JIS code point, if it has one in either of the two
/// ranges QR Kanji mode can represent (0x8140..=0x9FFC, 0xE040..=0xEBBF).
pub(crate) fn shift_jis_kanji(c: char) -> Result<u16, EncodeError> {
    let mut utf8 = [0u8; 4];
    let mut sjis = [0u8; 2];
    let (result, _, written) = SHIFT_JIS
        .new_encoder()
        .encode_from_utf8_without_replacement(c.encode_utf8(&mut utf8), &mut sjis, true);

    // Unmappable chars and single-byte ones (ASCII, half-width katakana) aren't Kanji.
    if !matches!(result, EncoderResult::InputEmpty) || written != 2 {
        return Err(EncodeError::InvalidChar(c));
    }

    let code_point = u16::from_be_bytes(sjis);
    match code_point {
        0x8140..=0x9FFC | 0xE040..=0xEBBF => Ok(code_point),
        _ => Err(EncodeError::KanjiOutOfRange(code_point)),
    }
}

/// Encodes `data` as QR Byte mode data bits: every byte is packed as-is, 8 bits each. Every
/// byte value is valid, so this can't fail.
pub(crate) fn encode_byte(data: &[u8]) -> BitVec<u8, Lsb0> {
    let mut bits = BitVec::with_capacity(data.len() * 8);
    for &byte in data {
        write_bits(&mut bits, byte as u32, 8);
    }
    bits
}

/// Encodes `data` as QR Kanji mode data bits: each char is transcoded to its two-byte Shift-JIS
/// code point, reduced into its QR subset range and packed into 13 bits.
pub(crate) fn encode_kanji(data: &str) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    let mut bits = BitVec::new();
    for c in data.chars() {
        let code_point = shift_jis_kanji(c)?;
        let reduced = if code_point < 0xE040 {
            code_point - 0x8140
        } else {
            code_point - 0xC140
        };
        let value = (reduced >> 8) * 0xC0 + (reduced & 0xFF);
        write_bits(&mut bits, value as u32, 13);
    }
    Ok(bits)
}

/// Encodes an ECI assignment number as QR ECI mode data bits, using the UTF-8-like
/// variable-length prefix scheme the spec defines: 1 byte for 0..=127, 2 bytes for
/// 128..=16383, 3 bytes for 16384..=999999.
pub(crate) fn encode_eci(designator: u32) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    let mut bits = BitVec::new();
    match designator {
        0..=127 => {
            write_bits(&mut bits, 0, 1);
            write_bits(&mut bits, designator, 7);
        }
        128..=16_383 => {
            write_bits(&mut bits, 0b10, 2);
            write_bits(&mut bits, designator, 14);
        }
        16_384..=999_999 => {
            write_bits(&mut bits, 0b110, 3);
            write_bits(&mut bits, designator, 21);
        }
        _ => return Err(EncodeError::EciOutOfRange(designator)),
    }
    Ok(bits)
}

/// Encodes FNC1-in-first-position. It carries no data of its own beyond its mode indicator
/// (written by the caller), so there are no data bits to append.
pub(crate) fn encode_fnc1_first_position() -> BitVec<u8, Lsb0> {
    BitVec::new()
}

/// Encodes FNC1-in-second-position's Application Indicator as QR data bits. Resolving an
/// AI (a digit or letter, per AIM spec) to its 8-bit value is the caller's responsibility.
pub(crate) fn encode_fnc1_second_position(application_indicator: u8) -> BitVec<u8, Lsb0> {
    let mut bits = BitVec::new();
    write_bits(&mut bits, application_indicator as u32, 8);
    bits
}

/// Encodes Structured Append's header fields as QR data bits: a 4-bit sequence number, a
/// 4-bit total-symbol count (both 0..=15), and an 8-bit parity byte.
pub(crate) fn encode_structured_append(sequence_number: u8, total_symbols: u8, parity: u8, ) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    if sequence_number > 15 || total_symbols > 15 {
        return Err(EncodeError::StructuredAppendOutOfRange);
    }

    let mut bits = BitVec::new();
    write_bits(&mut bits, sequence_number as u32, 4);
    write_bits(&mut bits, total_symbols as u32, 4);
    write_bits(&mut bits, parity as u32, 8);
    Ok(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_single_digit_is_four_bits() {
        assert_eq!(encode_numeric("1").unwrap(), bits![0, 0, 0, 1]);
    }

    #[test]
    fn numeric_two_digits_is_seven_bits() {
        assert_eq!(encode_numeric("12").unwrap(), bits![0, 0, 0, 1, 1, 0, 0]);
    }

    #[test]
    fn numeric_three_digits_is_ten_bits() {
        assert_eq!(encode_numeric("123").unwrap(), bits![0, 0, 0, 1, 1, 1, 1, 0, 1, 1]);
    }

    #[test]
    fn numeric_matches_the_iso_18004_example() {
        // ISO/IEC 18004 Annex I's worked example for "01234567".
        let expected = bits![
            0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1
        ];
        assert_eq!(encode_numeric("01234567").unwrap(), expected);
    }

    #[test]
    fn numeric_rejects_a_non_digit() {
        assert_eq!(encode_numeric("12a").unwrap_err(), EncodeError::InvalidChar('a'));
    }

    #[test]
    fn alphanumeric_single_char_is_six_bits() {
        assert_eq!(encode_alphanumeric("2").unwrap(), bits![0, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn alphanumeric_matches_the_iso_18004_example() {
        // ISO/IEC 18004 Annex I's worked example for "AC-42".
        let expected = bits![
            0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0
        ];
        assert_eq!(encode_alphanumeric("AC-42").unwrap(), expected);
    }

    #[test]
    fn alphanumeric_rejects_a_char_outside_the_set() {
        assert_eq!(encode_alphanumeric("A!").unwrap_err(), EncodeError::InvalidChar('!'));
    }

    #[test]
    fn byte_packs_every_byte_as_is() {
        assert_eq!(
            encode_byte(&[0x61, 0x62]),
            bits![0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 0]
        );
    }

    #[test]
    fn kanji_matches_the_iso_18004_example() {
        // ISO/IEC 18004 Annex I's worked example: 点, Shift-JIS 0x935F.
        let expected = bits![0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1];
        assert_eq!(encode_kanji("点").unwrap(), expected);
    }

    #[test]
    fn kanji_transcodes_both_shift_jis_ranges() {
        assert_eq!(shift_jis_kanji('点'), Ok(0x935F));
        // 茗 sits in the second range, at Shift-JIS 0xE4AA.
        assert_eq!(shift_jis_kanji('茗'), Ok(0xE4AA));
    }

    #[test]
    fn kanji_rejects_single_byte_shift_jis_chars() {
        assert_eq!(encode_kanji("a").unwrap_err(), EncodeError::InvalidChar('a'));
        // Half-width katakana is a single Shift-JIS byte.
        assert_eq!(encode_kanji("ｱ").unwrap_err(), EncodeError::InvalidChar('ｱ'));
    }

    #[test]
    fn kanji_rejects_a_char_with_no_shift_jis_mapping() {
        assert_eq!(encode_kanji("é").unwrap_err(), EncodeError::InvalidChar('é'));
    }

    #[test]
    fn kanji_rejects_a_code_point_outside_either_range() {
        // 纊 only exists in the IBM extension block, at Shift-JIS 0xFA5C.
        assert_eq!(encode_kanji("纊").unwrap_err(), EncodeError::KanjiOutOfRange(0xFA5C));
    }

    #[test]
    fn eci_one_byte_range_has_a_zero_prefix() {
        let mut expected = bits![u8, Lsb0; 0].to_bitvec();
        write_bits(&mut expected, 127, 7);
        assert_eq!(encode_eci(127).unwrap(), expected);
    }

    #[test]
    fn eci_two_byte_range_has_a_10_prefix() {
        let mut expected = bits![u8, Lsb0; 1, 0].to_bitvec();
        write_bits(&mut expected, 128, 14);
        assert_eq!(encode_eci(128).unwrap(), expected);
    }

    #[test]
    fn eci_three_byte_range_has_a_110_prefix() {
        let mut expected = bits![u8, Lsb0; 1, 1, 0].to_bitvec();
        write_bits(&mut expected, 16_384, 21);
        assert_eq!(encode_eci(16_384).unwrap(), expected);
    }

    #[test]
    fn eci_rejects_a_designator_above_999999() {
        assert_eq!(encode_eci(1_000_000).unwrap_err(), EncodeError::EciOutOfRange(1_000_000));
    }

    #[test]
    fn fnc1_first_position_has_no_data_bits() {
        assert!(encode_fnc1_first_position().is_empty());
    }

    #[test]
    fn fnc1_second_position_packs_the_application_indicator_as_a_byte() {
        assert_eq!(
            encode_fnc1_second_position(100),
            bits![0, 1, 1, 0, 0, 1, 0, 0]
        );
    }

    #[test]
    fn structured_append_packs_its_three_fields() {
        let expected = bits![0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1];
        assert_eq!(encode_structured_append(2, 3, 1).unwrap(), expected);
    }

    #[test]
    fn structured_append_rejects_a_sequence_number_above_15() {
        assert_eq!(
            encode_structured_append(16, 0, 0).unwrap_err(),
            EncodeError::StructuredAppendOutOfRange
        );
    }
}
