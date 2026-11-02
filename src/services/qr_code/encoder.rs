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
use crate::services::qr_code::{EncodedData, EncodingMode, ErrorCorrectionLevel, QRCreationError, QRVersion, VersionChoice};
use crate::services::qr_code::capacity;
use crate::services::qr_code::gf_math::{error_correction_codewords, EcLength};
use crate::services::qr_code::segmenter::{choose_version, SegmentMode, SegmentPlan, VersionGroup};
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
    NotImplemented
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
fn encode_numeric(data: &str) -> Result<BitVec<u8, Lsb0>, EncodeError> {
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
fn encode_alphanumeric(data: &str) -> Result<BitVec<u8, Lsb0>, EncodeError> {
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
fn encode_byte(data: &str) -> BitVec<u8, Lsb0> {
    let mut bits = BitVec::with_capacity(data.len() * 8);
    for &byte in data.as_bytes() {
        write_bits(&mut bits, byte as u32, 8);
    }
    bits
}

/// Encodes `data` as QR Kanji mode data bits: each char is transcoded to its two-byte Shift-JIS
/// code point, reduced into its QR subset range and packed into 13 bits.
fn encode_kanji(data: &str) -> Result<BitVec<u8, Lsb0>, EncodeError> {
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
fn encode_eci(designator: u32) -> Result<BitVec<u8, Lsb0>, EncodeError> {
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
fn encode_fnc1_first_position() -> BitVec<u8, Lsb0> {
    BitVec::new()
}

/// Encodes FNC1-in-second-position's Application Indicator as QR data bits. Resolving an
/// AI (a digit or letter, per AIM spec) to its 8-bit value is the caller's responsibility.
fn encode_fnc1_second_position(application_indicator: u8) -> BitVec<u8, Lsb0> {
    let mut bits = BitVec::new();
    write_bits(&mut bits, application_indicator as u32, 8);
    bits
}

/// Encodes Structured Append's header fields as QR data bits: a 4-bit sequence number, a
/// 4-bit total-symbol count (both 0..=15), and an 8-bit parity byte.
fn encode_structured_append(sequence_number: u8, total_symbols: u8, parity: u8, ) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    if sequence_number > 15 || total_symbols > 15 {
        return Err(EncodeError::StructuredAppendOutOfRange);
    }

    let mut bits = BitVec::new();
    write_bits(&mut bits, sequence_number as u32, 4);
    write_bits(&mut bits, total_symbols as u32, 4);
    write_bits(&mut bits, parity as u32, 8);
    Ok(bits)
}

/// Writes a segment's header: its 4-bit mode indicator, then its character count in the width
/// `group` gives `mode`. Byte mode counts UTF-8 bytes; every other mode counts chars.
fn write_segment_header(bits: &mut BitVec<u8, Lsb0>, mode: SegmentMode, text: &str, group: VersionGroup) {
    let indicator = match mode {
        SegmentMode::Numeric => 0b0001,
        SegmentMode::Alphanumeric => 0b0010,
        SegmentMode::Byte => 0b0100,
        SegmentMode::Kanji => 0b1000,
    };
    let count = match mode {
        SegmentMode::Byte => text.len(),
        _ => text.chars().count(),
    };
    write_bits(bits, indicator, 4);
    write_bits(bits, count as u32, group.char_count_bits(mode));
}

/// Encodes every segment of `plan` as a header followed by its data bits, back to back with no
/// byte alignment between segments.
pub(crate) fn encode_data_by_encoding_plan(data:&str, plan: SegmentPlan) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    let group = VersionGroup::of(plan.version);
    let mut bits = BitVec::with_capacity(plan.data_bits_length);
    for segment in plan.segments {
        let text = &data[segment.range];
        write_segment_header(&mut bits, segment.mode, text, group);
        bits.append(&mut encode_data_with_mode(text, EncodingMode::from(segment.mode))?);
    }
    Ok(bits)
}

/// Encodes all of `data` as one segment in `mode`, header included, in the version chosen per
/// `choice`. Fails if `data` has a char `mode` can't encode, or doesn't fit the version.
pub(crate) fn encode_single_segment(
    data: &str,
    mode: SegmentMode,
    choice: VersionChoice,
    error_correction_level: ErrorCorrectionLevel,
) -> Result<EncodedData, QRCreationError> {
    let mut data_bits = encode_data_with_mode(data, EncodingMode::from(mode))?;
    let (version, needed_bits) = choose_version(choice, error_correction_level, |group| {
        4 + group.char_count_bits(mode) + data_bits.len()
    })?;

    let mut bits = BitVec::with_capacity(needed_bits);
    write_segment_header(&mut bits, mode, data, VersionGroup::of(version));
    bits.append(&mut data_bits);
    Ok(EncodedData { version, bits })
}

/// Fills out `bits` to exactly the data codewords `version` holds at `error_correction_level`:
/// a terminator of up to four zero bits (fewer if capacity runs out first), zero bits to the next
/// byte boundary, then alternating 0xEC, 0x11 pad codewords. Returns the data codewords.
///
/// Panics if `bits` is already longer than the data codewords; segmenting checks capacity first.
pub(crate) fn apply_formatting(
    mut bits: BitVec<u8, Lsb0>,
    version: QRVersion,
    error_correction_level: ErrorCorrectionLevel,
) -> Vec<u8> {
    let data_codewords = capacity::data_codewords(version, error_correction_level);
    let capacity_bits = data_codewords * 8;
    assert!(
        bits.len() <= capacity_bits,
        "{} bits don't fit in {} data codewords",
        bits.len(),
        data_codewords
    );

    let terminator_length = (capacity_bits - bits.len()).min(4);
    bits.resize(bits.len() + terminator_length, false);
    // Capacity is whole bytes, so this never overruns it.
    bits.resize(bits.len().next_multiple_of(8), false);

    let mut codewords: Vec<u8> = bits
        .chunks(8)
        .map(|byte| byte.iter().fold(0u8, |acc, bit| acc << 1 | *bit as u8))
        .collect();
    let pad_length = data_codewords - codewords.len();
    codewords.extend([0xEC, 0x11].into_iter().cycle().take(pad_length));
    codewords
}

/// Splits `data_codewords` into `version`'s error correction blocks, computes each block's error
/// correction codewords, and interleaves them into the final bitstream: the first data codeword
/// of every block, then the second, and so on, followed by the error correction codewords in the
/// same pattern. Ready for `QRMatrix::place_data`.
///
/// Panics if `data_codewords` isn't exactly the data codeword count `apply_formatting` produces.
pub(crate) fn apply_error_correction(
    data_codewords: &[u8],
    version: QRVersion,
    error_correction_level: ErrorCorrectionLevel,
) -> BitVec<u8, Lsb0> {
    assert_eq!(
        data_codewords.len(),
        capacity::data_codewords(version, error_correction_level),
        "data codewords must fill the version exactly"
    );
    let block_count = capacity::error_correction_blocks(version, error_correction_level);
    let ec_per_block = capacity::ecc_codewords_per_block(version, error_correction_level);
    let ec_length = EcLength::try_from(ec_per_block as u8)
        .expect("every error correction codewords per block entry is a valid EcLength");

    // Blocks share the data as evenly as possible; the short ones come first, and the last
    // `data_codewords.len() % block_count` blocks each hold one extra codeword.
    let short_length = data_codewords.len() / block_count;
    let short_blocks = block_count - data_codewords.len() % block_count;
    let mut blocks = Vec::with_capacity(block_count);
    let mut rest = data_codewords;
    for i in 0..block_count {
        let (block, tail) = rest.split_at(short_length + usize::from(i >= short_blocks));
        blocks.push(block);
        rest = tail;
    }
    let ec_blocks: Vec<Vec<u8>> = blocks
        .iter()
        .map(|block| error_correction_codewords(block, ec_length))
        .collect();

    let mut bits = BitVec::with_capacity(capacity::total_codewords(version) * 8);
    // Short blocks run out one column early; `get` skips them in the last data column.
    for i in 0..=short_length {
        for block in &blocks {
            if let Some(&codeword) = block.get(i) {
                write_bits(&mut bits, codeword as u32, 8);
            }
        }
    }
    for i in 0..ec_per_block {
        for ec_block in &ec_blocks {
            write_bits(&mut bits, ec_block[i] as u32, 8);
        }
    }
    bits
}

pub(crate) fn encode_data_with_mode(data:&str, mode:EncodingMode) -> Result<BitVec<u8, Lsb0>, EncodeError> {
    match mode {
        EncodingMode::Numeric => {
            Ok(encode_numeric(data)?)
        }
        EncodingMode::Alphanumeric => {
            Ok(encode_alphanumeric(data)?)
        }
        EncodingMode::Byte => {
            Ok(encode_byte(data))
        }
        EncodingMode::Kanji => {
            Ok(encode_kanji(data)?)
        }
        _ => {
            Err(EncodeError::NotImplemented)
        }
    }
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
            encode_byte(&"ab"),
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

    fn single_segment_plan(mode: SegmentMode, data: &str, version: u8) -> SegmentPlan {
        use crate::services::qr_code::{segmenter::Segment, QRVersion};
        SegmentPlan {
            version: QRVersion { version },
            segments: vec![Segment { mode, range: 0..data.len() }],
            data_bits_length: 0,
        }
    }

    #[test]
    fn plan_header_counts_kanji_chars_not_bytes() {
        // 点 is 3 UTF-8 bytes but 1 char.
        let mut expected = BitVec::new();
        write_bits(&mut expected, 0b1000, 4);
        write_bits(&mut expected, 1, 8);
        write_bits(&mut expected, 0x0D9F, 13);
        let plan = single_segment_plan(SegmentMode::Kanji, "点", 1);
        assert_eq!(encode_data_by_encoding_plan("点", plan).unwrap(), expected);
    }

    #[test]
    fn plan_header_counts_byte_mode_in_utf8_bytes() {
        // é is 2 UTF-8 bytes: 0xC3 0xA9.
        let mut expected = BitVec::new();
        write_bits(&mut expected, 0b0100, 4);
        write_bits(&mut expected, 2, 8);
        write_bits(&mut expected, 0xC3, 8);
        write_bits(&mut expected, 0xA9, 8);
        let plan = single_segment_plan(SegmentMode::Byte, "é", 1);
        assert_eq!(encode_data_by_encoding_plan("é", plan).unwrap(), expected);
    }

    #[test]
    fn plan_header_widens_numeric_count_in_the_large_group() {
        // Numeric's count indicator is 14 bits from version 27.
        let mut expected = BitVec::new();
        write_bits(&mut expected, 0b0001, 4);
        write_bits(&mut expected, 1, 14);
        write_bits(&mut expected, 7, 4);
        let plan = single_segment_plan(SegmentMode::Numeric, "7", 27);
        assert_eq!(encode_data_by_encoding_plan("7", plan).unwrap(), expected);
    }

    fn v(version: u8) -> QRVersion {
        QRVersion { version }
    }

    /// Packs a bitstream back into bytes, MSB first.
    fn to_bytes(bits: &BitSlice<u8, Lsb0>) -> Vec<u8> {
        bits.chunks(8).map(|byte| byte.iter().fold(0u8, |acc, bit| acc << 1 | *bit as u8)).collect()
    }

    /// ISO/IEC 18004 Annex I's "01234567" at 1-M, as segment bits.
    fn iso_example_bits() -> BitVec<u8, Lsb0> {
        let mut bits = BitVec::new();
        for (value, bit_length) in [(0b0001, 4), (8, 10), (12, 10), (345, 10), (67, 7)] {
            write_bits(&mut bits, value, bit_length);
        }
        bits
    }

    #[test]
    fn formatting_matches_the_iso_18004_example() {
        let codewords = apply_formatting(iso_example_bits(), v(1), ErrorCorrectionLevel::M);
        assert_eq!(
            codewords,
            [0x10, 0x20, 0x0C, 0x56, 0x61, 0x80, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11]
        );
    }

    #[test]
    fn formatting_empty_data_is_a_terminator_then_padding() {
        // 1-L holds 19 data codewords: one zero byte (terminator + alignment), then 18 pads.
        let codewords = apply_formatting(BitVec::new(), v(1), ErrorCorrectionLevel::L);
        let mut expected = vec![0x00];
        expected.extend([0xEC, 0x11].into_iter().cycle().take(18));
        assert_eq!(codewords, expected);
    }

    #[test]
    fn formatting_shortens_the_terminator_when_capacity_runs_out() {
        // 1-H holds 72 bits; 70 data bits leave room for only 2 terminator bits and no padding.
        let codewords = apply_formatting(bitvec![u8, Lsb0; 1; 70], v(1), ErrorCorrectionLevel::H);
        assert_eq!(codewords, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFC]);
    }

    #[test]
    fn formatting_adds_nothing_to_data_that_fills_capacity() {
        let codewords = apply_formatting(bitvec![u8, Lsb0; 1; 72], v(1), ErrorCorrectionLevel::H);
        assert_eq!(codewords, [0xFF; 9]);
    }

    #[test]
    #[should_panic(expected = "73 bits don't fit in 9 data codewords")]
    fn formatting_rejects_data_over_capacity() {
        apply_formatting(bitvec![u8, Lsb0; 0; 73], v(1), ErrorCorrectionLevel::H);
    }

    #[test]
    fn error_correction_matches_the_iso_18004_example() {
        let codewords = apply_formatting(iso_example_bits(), v(1), ErrorCorrectionLevel::M);
        let bits = apply_error_correction(&codewords, v(1), ErrorCorrectionLevel::M);
        let mut expected = codewords.clone();
        expected.extend([0xA5, 0x24, 0xD4, 0xC1, 0xED, 0x36, 0xC7, 0x87, 0x2C, 0x55]);
        assert_eq!(to_bytes(&bits), expected);
    }

    #[test]
    fn error_correction_matches_hello_world_at_1_q() {
        // thonky.com's QR tutorial worked example.
        let data = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236];
        let bits = apply_error_correction(&data, v(1), ErrorCorrectionLevel::Q);
        let mut expected = data.to_vec();
        expected.extend([168, 72, 22, 82, 217, 54, 156, 0, 46, 15, 180, 122, 16]);
        assert_eq!(to_bytes(&bits), expected);
    }

    #[test]
    fn error_correction_interleaves_blocks_of_two_lengths() {
        // 5-Q: two blocks of 15 data codewords, then two of 16; 18 EC codewords each.
        let data: Vec<u8> = (0..62).collect();
        let bytes = to_bytes(&apply_error_correction(&data, v(5), ErrorCorrectionLevel::Q));
        assert_eq!(bytes.len(), capacity::total_codewords(v(5)));

        let mut expected_data: Vec<u8> = (0..15).flat_map(|i| [i, 15 + i, 30 + i, 46 + i]).collect();
        expected_data.extend([45, 61]);
        assert_eq!(bytes[..62], expected_data);

        let ec_length = EcLength::try_from(18).unwrap();
        let ec_blocks = [&data[0..15], &data[15..30], &data[30..46], &data[46..62]]
            .map(|block| error_correction_codewords(block, ec_length));
        let expected_ec: Vec<u8> = (0..18).flat_map(|i| ec_blocks.iter().map(move |ec| ec[i])).collect();
        assert_eq!(bytes[62..], expected_ec);
    }

    #[test]
    fn error_correction_fills_every_codeword_of_every_version() {
        for version in 1..=40u8 {
            for level in [ErrorCorrectionLevel::L, ErrorCorrectionLevel::M, ErrorCorrectionLevel::Q, ErrorCorrectionLevel::H] {
                let codewords = apply_formatting(BitVec::new(), v(version), level);
                let bits = apply_error_correction(&codewords, v(version), level);
                assert_eq!(bits.len(), capacity::total_codewords(v(version)) * 8, "version {version} {level:?}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "data codewords must fill the version exactly")]
    fn error_correction_rejects_unformatted_data() {
        apply_error_correction(&[0x10, 0x20], v(1), ErrorCorrectionLevel::M);
    }

    #[test]
    fn structured_append_rejects_a_sequence_number_above_15() {
        assert_eq!(
            encode_structured_append(16, 0, 0).unwrap_err(),
            EncodeError::StructuredAppendOutOfRange
        );
    }
}
