// The inverse of encoder.rs / segmenter.rs: reads a stream of data codewords already stripped
// of ec, terminator and padding back into the original string
use bitvec::prelude::*;
use encoding_rs::SHIFT_JIS;
use crate::services::qr_code::bits::bytes_to_bits;
use crate::services::qr_code::encoder::alphanumeric_char;
use crate::services::qr_code::QRVersion;
use crate::services::qr_code::segmenter::{SegmentMode, VersionGroup};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeError {
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

#[cfg(test)]
mod tests {
    use super::*;

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
