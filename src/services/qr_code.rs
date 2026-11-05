use bitvec::prelude::*;
use crate::services::qr_code::{
    bits::bytes_to_bits,
    blocks::interleave,
    structure::QRMatrix,
    encoder::{apply_formatting, encode_data_by_encoding_plan, encode_single_segment, EncodeError},
    segmenter::{best_single_mode, segment, SegmentError, SegmentMode},
};


pub mod penalty_score;
pub mod gf_math;
pub mod encoder;
pub mod decoder;
pub mod bits;
pub mod segmenter;
mod capacity;
mod structure;
pub mod blocks;
pub(crate) mod prelude;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QRVersionError {
    OutOfBounds(u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QRVersion {
    version: u8,
}

impl TryFrom<u8> for QRVersion {
    type Error = QRVersionError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value >= 1 && value <= 40 {Ok(QRVersion{version:value})} else {Err(QRVersionError::OutOfBounds(value)) }
    }
}
impl Into<u8> for QRVersion {
    fn into(self) -> u8 {
        self.version
    }
}
impl Into<usize> for QRVersion {
    fn into(self) -> usize {
        self.version as usize
    }
}


/// How the caller wants the version chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionChoice {
    /// Use exactly this version, failing if the data doesn't fit.
    Fixed(QRVersion),
    /// Use the smallest version that fits the optimally segmented data.
    Auto,
}

#[derive(Debug)]
pub enum EncodingMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    ECI,
    FNC1,
    SA,
    AutoDetect,
    Segmented
}
impl From<SegmentMode> for EncodingMode {
    fn from(value: SegmentMode) -> Self {
        match value {
            SegmentMode::Numeric => {EncodingMode::Numeric}
            SegmentMode::Alphanumeric => { EncodingMode::Alphanumeric}
            SegmentMode::Byte => { EncodingMode::Byte}
            SegmentMode::Kanji => {EncodingMode::Kanji}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCorrectionLevel {
    L,
    M,
    Q,
    H
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum QRCreationError {
    InvalidQRVersion(QRVersionError),
    NotSegmentable(SegmentError),
    NotImplemented(EncodingMode),
    EncodingFailed(EncodeError)
}

impl From<SegmentError> for QRCreationError {
    fn from(value: SegmentError) -> Self { QRCreationError::NotSegmentable(value) }
}
impl From<QRVersionError> for QRCreationError {
    fn from(value: QRVersionError) -> Self { QRCreationError::InvalidQRVersion(value) }
}
impl From<EncodeError> for QRCreationError {
    fn from(value: EncodeError) -> Self { QRCreationError::EncodingFailed(value) }
}

/// Encoded segment bits (headers and data, no terminator or padding yet), and the version they
/// were sized for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodedData {
    pub(crate) version: QRVersion,
    pub(crate) bits: BitVec<u8, Lsb0>,
}

pub(crate) fn create_qr_code_bits(
    data: &str,
    error_correction_level:ErrorCorrectionLevel,
    encoding_mode: Option<EncodingMode>,
    version:Option<QRVersion>,
) -> Result<EncodedData, QRCreationError> {
    let version_choice = match version {
        Some(version) => VersionChoice::Fixed(version),
        None => VersionChoice::Auto
    };
    let mode = match encoding_mode {
        Some(EncodingMode::Segmented) => {
            let segment_plan = segment(data, version_choice, error_correction_level)?;
            let version = segment_plan.version;
            let bits = encode_data_by_encoding_plan(data, segment_plan)?;
            return Ok(EncodedData { version, bits });
        }
        Some(EncodingMode::AutoDetect) | None => best_single_mode(data),
        Some(EncodingMode::Numeric) => SegmentMode::Numeric,
        Some(EncodingMode::Alphanumeric) => SegmentMode::Alphanumeric,
        Some(EncodingMode::Byte) => SegmentMode::Byte,
        Some(EncodingMode::Kanji) => SegmentMode::Kanji,
        Some(mode @ (EncodingMode::ECI | EncodingMode::FNC1 | EncodingMode::SA)) => {
            return Err(QRCreationError::NotImplemented(mode));
        }
    };
    encode_single_segment(data, mode, version_choice, error_correction_level)
}

pub(crate) fn create_qr_code(
    data:&str,
    error_correction_level:ErrorCorrectionLevel,
    encoding_mode: Option<EncodingMode>,
    version:Option<QRVersion>
) -> Result<QRMatrix, QRCreationError> {
    let encoded_data = create_qr_code_bits(data, error_correction_level, encoding_mode, version)?;
    let mut structure = QRMatrix::new(encoded_data.version, error_correction_level);
    let data_codewords = apply_formatting(encoded_data.bits, encoded_data.version, error_correction_level);
    let formatted_bits = bytes_to_bits(&data_codewords);
    let error_corrected_data = interleave(&formatted_bits, encoded_data.version, error_correction_level);
    structure.place_data(&error_corrected_data);
    Ok(structure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::bits::write_bits;

    fn v(version: u8) -> QRVersion {
        QRVersion { version }
    }

    /// Builds an expected bitstream from `(value, bit_length)` fields, in order.
    fn fields(fields: &[(u32, usize)]) -> BitVec<u8, Lsb0> {
        let mut bits = BitVec::new();
        for &(value, bit_length) in fields {
            write_bits(&mut bits, value, bit_length);
        }
        bits
    }

    #[test]
    fn segmented_single_mode_matches_the_iso_18004_example() {
        // ISO/IEC 18004 Annex I: "01234567" at 1-M is mode 0001, count 8 in 10 bits, then data.
        let expected = fields(&[(0b0001, 4), (8, 10), (12, 10), (345, 10), (67, 7)]);
        let bits = create_qr_code_bits("01234567", ErrorCorrectionLevel::M, Some(EncodingMode::Segmented), Some(v(1))).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_writes_a_header_for_every_segment() {
        // Byte "a", then Numeric "1234567" (see segmenter's worked example).
        let expected = fields(&[
            (0b0100, 4), (1, 8), (b'a' as u32, 8),
            (0b0001, 4), (7, 10), (123, 10), (456, 10), (7, 4),
        ]);
        let bits = create_qr_code_bits("a1234567", ErrorCorrectionLevel::L, Some(EncodingMode::Segmented), None).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_length_matches_the_segment_plan() {
        for data in ["01234567", "a1234567", "AB1234567", "é1234567", "点点abc", "HELLO WORLD 123"] {
            let plan = segment(data, VersionChoice::Auto, ErrorCorrectionLevel::M).unwrap();
            let bits = create_qr_code_bits(data, ErrorCorrectionLevel::M, Some(EncodingMode::Segmented), None).unwrap().bits;
            assert_eq!(bits.len(), plan.data_bits_length, "length mismatch for {data:?}");
        }
    }

    #[test]
    fn segmented_count_indicator_widens_with_the_version() {
        // Byte's count indicator is 8 bits up to version 9, 16 bits from version 10.
        let expected = fields(&[(0b0100, 4), (2, 16), (b'a' as u32, 8), (b'b' as u32, 8)]);
        let bits = create_qr_code_bits("ab", ErrorCorrectionLevel::L, Some(EncodingMode::Segmented), Some(v(10))).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_rejects_data_that_does_not_fit_a_fixed_version() {
        let result = create_qr_code_bits(&"a".repeat(20), ErrorCorrectionLevel::L, Some(EncodingMode::Segmented), Some(v(1)));
        assert!(matches!(result, Err(QRCreationError::NotSegmentable(SegmentError::ExceedsVersionCapacity { .. }))));
    }

    #[test]
    fn auto_detect_matches_the_iso_18004_alphanumeric_example() {
        // ISO/IEC 18004 Annex I: "AC-42" is mode 0010, count 5 in 9 bits, then data.
        let expected = fields(&[(0b0010, 4), (5, 9), (10 * 45 + 12, 11), (41 * 45 + 4, 11), (2, 6)]);
        let bits = create_qr_code_bits("AC-42", ErrorCorrectionLevel::M, Some(EncodingMode::AutoDetect), None).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn no_mode_behaves_like_auto_detect() {
        let auto = create_qr_code_bits("AC-42", ErrorCorrectionLevel::M, Some(EncodingMode::AutoDetect), None).unwrap().bits;
        let none = create_qr_code_bits("AC-42", ErrorCorrectionLevel::M, None, None).unwrap().bits;
        assert_eq!(auto, none);
    }

    #[test]
    fn kanji_count_indicator_counts_chars_not_bytes() {
        // 点 is 3 UTF-8 bytes, but the count is 1 char. Data is 0x935F reduced to 0x0D9F.
        let expected = fields(&[(0b1000, 4), (1, 8), (0x0D9F, 13)]);
        let bits = create_qr_code_bits("点", ErrorCorrectionLevel::M, Some(EncodingMode::Kanji), None).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn byte_count_indicator_counts_utf8_bytes() {
        // é is 2 UTF-8 bytes: 0xC3 0xA9.
        let expected = fields(&[(0b0100, 4), (2, 8), (0xC3, 8), (0xA9, 8)]);
        let bits = create_qr_code_bits("é", ErrorCorrectionLevel::M, Some(EncodingMode::Byte), None).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn fixed_mode_rejects_data_that_does_not_fit_a_fixed_version() {
        let result = create_qr_code_bits(&"a".repeat(20), ErrorCorrectionLevel::L, Some(EncodingMode::Byte), Some(v(1)));
        assert!(result.is_err(), "20 bytes needs 172 bits but 1-L only holds 152");
    }

    #[test]
    fn auto_detect_widens_the_count_in_a_larger_version_group() {
        // 300 bytes overflows every Small version at L, so the count indicator is 16 bits.
        let bits = create_qr_code_bits(&"a".repeat(300), ErrorCorrectionLevel::L, None, None).unwrap().bits;
        assert_eq!(bits.len(), 4 + 16 + 300 * 8);
        assert_eq!(bits[..20], fields(&[(0b0100, 4), (300, 16)])[..]);
    }

    #[test]
    fn auto_detect_rejects_data_too_long_for_version_40() {
        let result = create_qr_code_bits(&"a".repeat(3000), ErrorCorrectionLevel::L, None, None);
        assert!(matches!(result, Err(QRCreationError::NotSegmentable(SegmentError::TooLong { needed_bits: 24020 }))));
    }

    #[test]
    fn fixed_mode_rejects_chars_outside_its_set() {
        let result = create_qr_code_bits("12a", ErrorCorrectionLevel::M, Some(EncodingMode::Numeric), None);
        assert!(matches!(result, Err(QRCreationError::EncodingFailed(EncodeError::InvalidChar('a')))));
    }

    #[test]
    fn prefix_modes_are_not_implemented() {
        for mode in [EncodingMode::ECI, EncodingMode::FNC1, EncodingMode::SA] {
            let result = create_qr_code_bits("1", ErrorCorrectionLevel::M, Some(mode), None);
            assert!(matches!(result, Err(QRCreationError::NotImplemented(_))));
        }
    }

    #[test]
    fn segmented_empty_input_is_empty() {
        let bits = create_qr_code_bits("", ErrorCorrectionLevel::M, Some(EncodingMode::Segmented), None).unwrap().bits;
        assert!(bits.is_empty());
    }

    /// Reads a matrix's placed codewords back out, in transmission order, and un-interleaves
    /// them into one `data ++ error_correction` byte sequence per block (group 1's blocks,
    /// then group 2's). The inverse of what `blocks::interleave` does.
    fn recover_codeword_blocks(matrix: &QRMatrix, info: capacity::EcBlockInfo) -> Vec<Vec<u8>> {
        let total_codewords = capacity::total_codewords(matrix.version());
        let bits: BitVec<u8, Lsb0> = matrix
            .data_module_order()
            .take(total_codewords * 8)
            .map(|(row, col)| matrix.is_dark(row, col))
            .collect();
        let bytes = bits::bits_to_bytes(&bits);
        let mut remaining = bytes.iter().copied();

        let block_lengths: Vec<usize> = std::iter::repeat_n(info.group_1_data_codewords as usize, info.group_1_blocks as usize)
            .chain(std::iter::repeat_n(info.group_2_data_codewords as usize, info.group_2_blocks as usize))
            .collect();
        let mut blocks: Vec<Vec<u8>> = block_lengths.iter().map(|&len| Vec::with_capacity(len)).collect();

        let max_data_len = block_lengths.iter().copied().max().unwrap_or(0);
        for i in 0..max_data_len {
            for (block, &len) in blocks.iter_mut().zip(&block_lengths) {
                if i < len {
                    block.push(remaining.next().unwrap());
                }
            }
        }
        for _ in 0..info.ec_codewords_per_block {
            for block in &mut blocks {
                block.push(remaining.next().unwrap());
            }
        }
        assert!(remaining.next().is_none(), "leftover codewords after un-interleaving every block");
        blocks
    }

    #[test]
    fn create_qr_code_round_trips_through_error_correction() {
        use crate::services::qr_code::gf_math::{correct_block, EcLength};

        let cases = [
            ("01234567", ErrorCorrectionLevel::M),
            ("HELLO WORLD", ErrorCorrectionLevel::Q),
            ("The quick brown fox jumps over the lazy dog.", ErrorCorrectionLevel::L),
            ("点点abc", ErrorCorrectionLevel::H),
        ];
        for (data, level) in cases {
            let matrix = create_qr_code(data, level, None, None).unwrap();
            let info = capacity::ec_block_info(matrix.version(), level);
            let ec_len = EcLength::try_from(info.ec_codewords_per_block as u8).unwrap();

            for block in recover_codeword_blocks(&matrix, info) {
                assert_eq!(correct_block(&block, ec_len), block, "EC mismatch for {data:?} at {level:?}");
            }
        }
    }

    #[test]
    fn create_qr_code_round_trips_to_the_original_string() {
        use crate::services::qr_code::decoder::decode_segments;
        use crate::services::qr_code::gf_math::{correct_block, EcLength};

        let cases = [
            ("01234567", ErrorCorrectionLevel::M),
            ("HELLO WORLD", ErrorCorrectionLevel::Q),
            ("AC-42", ErrorCorrectionLevel::M),
            ("a1234567", ErrorCorrectionLevel::L),
            ("The quick brown fox jumps over the lazy dog.", ErrorCorrectionLevel::L),
            ("点点abc", ErrorCorrectionLevel::H),
            ("é1234567", ErrorCorrectionLevel::M),
            ("", ErrorCorrectionLevel::M),
        ];
        for (original_string, level) in cases {
            let matrix = create_qr_code(original_string, level, None, None).unwrap();
            let info = capacity::ec_block_info(matrix.version(), level);
            let ec_len = EcLength::try_from(info.ec_codewords_per_block as u8).unwrap();
            let ec_per_block = info.ec_codewords_per_block as usize;

            let mut data_codewords = Vec::new();

            for block in recover_codeword_blocks(&matrix, info) {
                let corrected = correct_block(&block, ec_len);
                data_codewords.extend_from_slice(&corrected[..corrected.len() - ec_per_block]);
            }

            let decoded_string = decode_segments(&data_codewords, matrix.version())
                .unwrap_or_else(|e| panic!("decode failed for {original_string:?} at {level:?}: {e:?}"));

            assert_eq!(decoded_string, original_string, "round trip mismatch for {original_string:?} at {level:?}");
        }
    }
}