// The inverse of `create_qr_code`. `decode_modules` takes a symbol's modules as plain row-major
// bits, as a QR code reader would produce them, and works out everything else from those bits.
use bitvec::prelude::*;
use encoding_rs::SHIFT_JIS;
use crate::services::qr_code::bits::{bits_to_bytes, bytes_to_bits};
use crate::services::qr_code::blocks::deinterleave;
use crate::services::qr_code::capacity::{ec_block_info, total_codewords};
use crate::services::qr_code::encoder::alphanumeric_char;
use crate::services::qr_code::gf_math::{correct_block, error_correction_codewords, EcLength};
use crate::services::qr_code::{ErrorCorrectionLevel, QRVersion};
use crate::services::qr_code::segmenter::{SegmentMode, VersionGroup};
use crate::services::qr_code::structure::{bch_format_information, data_module_positions, format_information_positions, mask_predicate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeError {
    /// This many modules isn't a square of any QR version's width.
    InvalidModuleCount(usize),
    /// Neither copy of the format information is close enough to a valid codeword to trust, so
    /// the error correction level and mask are unknown.
    UnreadableFormatInformation,
    /// A block has more errors than its error correction codewords can repair.
    UncorrectableBlock,
    /// Ran out of bits midsegment.
    UnexpectedEnd,
    /// A mode indicator this decoder doesn't handle (ECI, FNC1, Structured Append, or a value the spec doesn't define)
    UnsupportedMode(u8),
    /// A Byte mode segment's bytes aren't valid UTF-8.
    InvalidUtf8,
    /// An Alphanumeric value outside 0..45, the width of QR's alphanumeric character set.
    InvalidAlphanumericValue(u8),
    /// A Kanji value that doesn't correspond to a Shift-JIS code point QR can represent.
    InvalidKanjiCodePoint(u16),
}

/// A cursor over a bitstream, reading big-endian integers: the convention `write_bits` writes in.
struct BitReader<'a> {
    bits: &'a BitSlice<u8, Lsb0>,
    pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(bits: &'a BitSlice<u8, Lsb0>) -> Self {
        BitReader { bits, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bits.len() - self.pos
    }

    /// Reads `length` bits as a big-endian integer.
    fn read(&mut self, length: usize) -> Result<u32, DecodeError> {
        if self.remaining() < length {
            return Err(DecodeError::UnexpectedEnd);
        }
        let value = self.bits[self.pos..self.pos + length]
            .iter()
            .fold(0u32, |acc, bit| (acc << 1) | *bit as u32);
        self.pos += length;
        Ok(value)
    }
}

/// Inverse of `encode_numeric`: a count, then digits in groups of three (10 bits), with a
/// trailing group of two (7 bits) or one (4 bits).
fn decode_numeric(reader: &mut BitReader, group: VersionGroup, output: &mut String) -> Result<(), DecodeError> {
    let mut remaining = reader.read(group.char_count_bits(SegmentMode::Numeric))? as usize;
    while remaining >= 3 {
        output.push_str(&format!("{:03}", reader.read(10)?));
        remaining -= 3;
    }
    match remaining {
        2 => output.push_str(&format!("{:02}", reader.read(7)?)),
        1 => output.push_str(&format!("{}", reader.read(4)?)),
        0 => {}
        _ => unreachable!("remaining only ever shrinks by 3 from a usize, so it ends in 0, 1 or 2"),
    }
    Ok(())
}

/// Inverse of `encode_alphanumeric`: a count, then characters in pairs (11 bits, `first * 45 +
/// second`), with a trailing single character (6 bits).
fn decode_alphanumeric(reader: &mut BitReader, group: VersionGroup, output: &mut String) -> Result<(), DecodeError> {
    let mut remaining = reader.read(group.char_count_bits(SegmentMode::Alphanumeric))? as usize;
    while remaining >= 2 {
        let value = reader.read(11)?;
        let (first, second) = ((value / 45) as u8, (value % 45) as u8);
        output.push(alphanumeric_char(first).ok_or(DecodeError::InvalidAlphanumericValue(first))?);
        output.push(alphanumeric_char(second).ok_or(DecodeError::InvalidAlphanumericValue(second))?);
        remaining -= 2;
    }
    if remaining == 1 {
        let value = reader.read(6)? as u8;
        output.push(alphanumeric_char(value).ok_or(DecodeError::InvalidAlphanumericValue(value))?);
    }
    Ok(())
}

/// Inverse of `encode_byte`: a byte count, then that many raw bytes, interpreted as UTF-8 (every
/// Byte segment's range sits on char boundaries, so this is always a complete, valid string).
fn decode_byte(reader: &mut BitReader, group: VersionGroup, output: &mut String) -> Result<(), DecodeError> {
    let count = reader.read(group.char_count_bits(SegmentMode::Byte))? as usize;
    let mut bytes = Vec::with_capacity(count);
    for _ in 0..count {
        bytes.push(reader.read(8)? as u8);
    }
    output.push_str(&String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?);
    Ok(())
}

/// Reverses `encode_kanji`'s transform on a single 13-bit value: back to the reduced Shift-JIS
/// offset, then the code point, then the char it decodes to.
fn decode_kanji_char(value: u16) -> Result<char, DecodeError> {
    let reduced = (value / 0xC0) * 0x100 + value % 0xC0;
    let code_point = if reduced <= 0x9FFC - 0x8140 { reduced + 0x8140 } else { reduced + 0xC140 };

    let bytes = code_point.to_be_bytes();
    let (decoded, _, had_errors) = SHIFT_JIS.decode(&bytes);
    let mut chars = decoded.chars();
    match (had_errors, chars.next(), chars.next()) {
        (false, Some(c), None) => Ok(c),
        _ => Err(DecodeError::InvalidKanjiCodePoint(code_point)),
    }
}

/// Inverse of `encode_kanji`: a count, then that many 13-bit values.
fn decode_kanji(reader: &mut BitReader, group: VersionGroup, output: &mut String) -> Result<(), DecodeError> {
    let count = reader.read(group.char_count_bits(SegmentMode::Kanji))? as usize;
    for _ in 0..count {
        output.push(decode_kanji_char(reader.read(13)? as u16)?);
    }
    Ok(())
}

/// Decodes `data_codewords` that have already been stripped of ec, terminator and padding
/// back into the original string. Stops at the terminator (a `0000` mode indicator) or once
/// fewer than 4 bits remain, whichever comes first.
pub(crate) fn decode_segments(data_codewords: &[u8], version: QRVersion) -> Result<String, DecodeError> {
    let bits = bytes_to_bits(data_codewords);
    let group = VersionGroup::of(QRVersion::try_from(version).unwrap());
    let mut reader = BitReader::new(&bits);
    let mut output = String::new();

    while reader.remaining() >= 4 {
        match reader.read(4)? {
            0b0000 => break,
            0b0001 => decode_numeric(&mut reader, group, &mut output)?,
            0b0010 => decode_alphanumeric(&mut reader, group, &mut output)?,
            0b0100 => decode_byte(&mut reader, group, &mut output)?,
            0b1000 => decode_kanji(&mut reader, group, &mut output)?,
            other => return Err(DecodeError::UnsupportedMode(other as u8)),
        }
    }
    Ok(output)
}

/// The version whose symbol has exactly `module_count` modules, and that symbol's width.
fn version_and_size(module_count: usize) -> Result<(QRVersion, usize), DecodeError> {
    (1..=40u8)
        .map(|version| (version, version as usize * 4 + 17))
        .find(|&(_, size)| size * size == module_count)
        .map(|(version, size)| (QRVersion::try_from(version).expect("1..=40 are all valid versions"), size))
        .ok_or(DecodeError::InvalidModuleCount(module_count))
}

/// Reads the error correction level and mask pattern from the format information. Picks the valid
/// codeword closest to either copy, and gives up if even that is more than 3 bits away: the
/// furthest BCH(15,5), with its minimum distance of 7, can reliably correct.
fn read_format_information(modules: &BitSlice<u8, Lsb0>, size: usize) -> Result<(ErrorCorrectionLevel, u8), DecodeError> {
    let read_copy = |copy: usize| {
        (0..15).fold(0u32, |word, i| {
            let (row, col) = format_information_positions(size, i)[copy];
            word | (modules[row * size + col] as u32) << i
        })
    };
    let copies = [read_copy(0), read_copy(1)];
    let (distance, data) = (0..32u32)
        .flat_map(|data| copies.map(|copy| ((bch_format_information(data) ^ copy).count_ones(), data)))
        .min()
        .expect("0..32 is not empty");
    if distance > 3 {
        return Err(DecodeError::UnreadableFormatInformation);
    }
    let level = match data >> 3 {
        0b01 => ErrorCorrectionLevel::L,
        0b00 => ErrorCorrectionLevel::M,
        0b11 => ErrorCorrectionLevel::Q,
        _ => ErrorCorrectionLevel::H,
    };
    Ok((level, (data & 0b111) as u8))
}

/// Decodes a symbol's modules, row-major with `true` for dark, back into the original string.
pub(crate) fn decode_modules(modules: &BitSlice<u8, Lsb0>) -> Result<String, DecodeError> {
    let (version, size) = version_and_size(modules.len())?;
    let (level, mask) = read_format_information(modules, size)?;

    let is_masked = mask_predicate(mask).expect("format information's 3-bit mask is always valid");
    let bits: BitVec<u8, Lsb0> = data_module_positions(version)
        .into_iter()
        .take(total_codewords(version) * 8)
        .map(|(row, col)| modules[row * size + col] ^ is_masked(row, col))
        .collect();
    let codewords = bits_to_bytes(&bits);

    let info = ec_block_info(version, level);
    let ec_per_block = info.ec_codewords_per_block as usize;
    let ec_len = EcLength::try_from(info.ec_codewords_per_block as u8)
        .expect("ec_codewords_per_block is always a valid EcLength");

    let mut data_codewords = Vec::with_capacity(info.total_data_codewords as usize);
    for block in deinterleave(&codewords, info) {
        let corrected = correct_block(&block, ec_len);
        let (data, error_correction) = corrected.split_at(corrected.len() - ec_per_block);
        // `correct_block` hands back its input when it gives up, so check the result is valid.
        if error_correction_codewords(data, ec_len) != error_correction {
            return Err(DecodeError::UncorrectableBlock);
        }
        data_codewords.extend_from_slice(data);
    }
    decode_segments(&data_codewords, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::qr_code::capacity::data_codewords;
    use crate::services::qr_code::{create_qr_code, EncodingMode, MaskSetting};

    /// What a reader would hand over for `data` at 1-M under `mask`: bits and nothing else.
    fn scanned(data: &str, mask: u8) -> BitVec<u8, Lsb0> {
        let matrix = create_qr_code(data, ErrorCorrectionLevel::M, None, None, Some(MaskSetting::Exact(mask))).unwrap();
        matrix.modules().to_bitvec()
    }

    fn toggle(modules: &mut BitVec<u8, Lsb0>, size: usize, (row, col): (usize, usize)) {
        let index = row * size + col;
        let dark = modules[index];
        modules.set(index, !dark);
    }

    #[test]
    fn decode_modules_removes_every_mask_pattern() {
        for mask in 0..8 {
            assert_eq!(decode_modules(&scanned("HELLO WORLD", mask)), Ok("HELLO WORLD".to_string()), "mask {mask}");
        }
    }

    #[test]
    fn decode_modules_reads_every_level_from_the_format_information() {
        use ErrorCorrectionLevel::*;
        for level in [L, M, Q, H] {
            let matrix = create_qr_code("HELLO", level, None, None, None).unwrap();
            let (read_level, _) = read_format_information(matrix.modules(), matrix.size()).unwrap();
            assert_eq!(read_level, level);
            assert_eq!(decode_modules(matrix.modules()), Ok("HELLO".to_string()));
        }
    }

    #[test]
    fn decode_modules_reads_a_nearly_full_symbol_at_every_version_and_level() {
        use ErrorCorrectionLevel::*;
        for version in 1..=40u8 {
            for level in [L, M, Q, H] {
                let qr_version = QRVersion::try_from(version).unwrap();
                // 3 bytes covers the longest Byte mode header.
                let original: String = (0..data_codewords(qr_version, level) - 3)
                    .map(|i| char::from(b'!' + (i % 90) as u8))
                    .collect();
                let mask = MaskSetting::Exact(version % 8);
                let matrix = create_qr_code(&original, level, Some(EncodingMode::Byte), Some(qr_version), Some(mask)).unwrap();
                assert_eq!(decode_modules(matrix.modules()).as_ref(), Ok(&original), "version {version} at {level:?}");
            }
        }
    }

    #[test]
    fn decode_modules_repairs_damaged_data_modules() {
        // 1-M has 10 EC codewords, so it can repair errors in up to 5 of them.
        let mut modules = scanned("01234567", 3);
        let positions = data_module_positions(QRVersion::try_from(1).unwrap());
        for codeword in 0..5 {
            toggle(&mut modules, 21, positions[codeword * 8]);
        }
        assert_eq!(decode_modules(&modules), Ok("01234567".to_string()));
    }

    #[test]
    fn decode_modules_reports_damage_beyond_what_error_correction_can_repair() {
        let mut modules = scanned("01234567", 3);
        for position in data_module_positions(QRVersion::try_from(1).unwrap()) {
            toggle(&mut modules, 21, position);
        }
        assert_eq!(decode_modules(&modules), Err(DecodeError::UncorrectableBlock));
    }

    #[test]
    fn decode_modules_tolerates_three_bad_format_bits_in_each_copy() {
        let mut modules = scanned("01234567", 5);
        for i in [0, 7, 14] {
            for position in format_information_positions(21, i) {
                toggle(&mut modules, 21, position);
            }
        }
        assert_eq!(decode_modules(&modules), Ok("01234567".to_string()));
    }

    #[test]
    fn decode_modules_falls_back_to_the_other_format_copy() {
        let mut modules = scanned("01234567", 2);
        for i in 0..15 {
            toggle(&mut modules, 21, format_information_positions(21, i)[0]);
        }
        assert_eq!(decode_modules(&modules), Ok("01234567".to_string()));
    }

    #[test]
    fn decode_modules_reports_unreadable_format_information() {
        let unreadable = (0..1u32 << 15)
            .find(|word| (0..32).all(|data| (bch_format_information(data) ^ word).count_ones() > 3))
            .expect("BCH(15,5) leaves some words more than 3 bits from every codeword");
        let mut modules = scanned("01234567", 3);
        for i in 0..15 {
            for (row, col) in format_information_positions(21, i) {
                modules.set(row * 21 + col, (unreadable >> i) & 1 == 1);
            }
        }
        assert_eq!(decode_modules(&modules), Err(DecodeError::UnreadableFormatInformation));
    }

    #[test]
    fn decode_modules_rejects_counts_that_are_not_a_version_size() {
        for count in [0, 440, 23 * 23, 181 * 181] {
            let modules = bitvec![u8, Lsb0; 0; count];
            assert_eq!(decode_modules(&modules), Err(DecodeError::InvalidModuleCount(count)));
        }
    }

    #[test]
    fn decodes_the_iso_18004_numeric_example() {
        let codewords = [0x10, 0x20, 0x0C, 0x56, 0x61, 0x80, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11];
        assert_eq!(decode_segments(&codewords, QRVersion::try_from(1).unwrap()), Ok("01234567".to_string()));
    }

    #[test]
    fn decodes_an_alphanumeric_segment_with_an_odd_trailing_character() {
        // ISO/IEC 18004 Annex I: "AC-42" is mode 0010, count 5 in 9 bits, then data.
        use crate::services::qr_code::bits::write_bits;
        let mut bits = BitVec::new();
        for (value, bit_length) in [(0b0010, 4), (5, 9), (10 * 45 + 12, 11), (41 * 45 + 4, 11), (2, 6)] {
            write_bits(&mut bits, value, bit_length);
        }
        let codewords = bits_to_bytes_for_test(&bits);
        assert_eq!(decode_segments(&codewords, QRVersion::try_from(1).unwrap()), Ok("AC-42".to_string()));
    }

    #[test]
    fn decodes_a_byte_segment_with_multibyte_utf8() {
        let mut bits = BitVec::new();
        use crate::services::qr_code::bits::write_bits;
        write_bits(&mut bits, 0b0100, 4);
        write_bits(&mut bits, 2, 8);
        write_bits(&mut bits, 0xC3, 8);
        write_bits(&mut bits, 0xA9, 8);
        let codewords = bits_to_bytes_for_test(&bits);
        assert_eq!(decode_segments(&codewords, QRVersion::try_from(1).unwrap()), Ok("é".to_string()));
    }

    #[test]
    fn decodes_a_kanji_segment() {
        let mut bits = BitVec::new();
        use crate::services::qr_code::bits::write_bits;
        write_bits(&mut bits, 0b1000, 4);
        write_bits(&mut bits, 1, 8);
        write_bits(&mut bits, 0x0D9F, 13);
        let codewords = bits_to_bytes_for_test(&bits);
        assert_eq!(decode_segments(&codewords, QRVersion::try_from(1).unwrap()), Ok("点".to_string()));
    }

    #[test]
    fn stops_at_a_terminator_without_erroring_on_padding() {
        let codewords = [0x10, 0x20, 0x0C, 0x56, 0x61, 0x80, 0xEC, 0x11];
        assert_eq!(decode_segments(&codewords, QRVersion::try_from(1).unwrap()), Ok("01234567".to_string()));
    }

    #[test]
    fn empty_data_decodes_to_an_empty_string() {
        assert_eq!(decode_segments(&[], QRVersion::try_from(1).unwrap()), Ok(String::new()));
    }

    /// Pads `bits` to a byte boundary and packs it.
    fn bits_to_bytes_for_test(bits: &BitVec<u8, Lsb0>) -> Vec<u8> {
        let mut bits = bits.clone();
        bits.resize(bits.len().next_multiple_of(8), false);
        crate::services::qr_code::bits::bits_to_bytes(&bits)
    }
}
