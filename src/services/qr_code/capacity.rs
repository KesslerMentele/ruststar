// Codeword counts for every version and error correction level, from ISO/IEC 18004's error
// correction block characteristics. Every block of a given symbol has the same number of error
// correction codewords; data codewords are whatever is left, spread as evenly as possible across
// the blocks.

use super::{ErrorCorrectionLevel, QRVersion};

/// Error correction codewords in each block, indexed by `[level][version - 1]`.
const ECC_CODEWORDS_PER_BLOCK: [[u8; 40]; 4] = [
    // 1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40
    [  7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30], // L
    [ 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28], // M
    [ 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30], // Q
    [ 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30], // H
];

/// Number of error correction blocks, indexed by `[level][version - 1]`.
const ERROR_CORRECTION_BLOCKS: [[u8; 40]; 4] = [
    // 1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40
    [  1,  1,  1,  1,  1,  2,  2,  2,  2,  4,  4,  4,  4,  4,  6,  6,  6,  6,  7,  8,  8,  9,  9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25], // L
    [  1,  1,  1,  2,  2,  4,  4,  4,  5,  5,  5,  8,  9,  9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49], // M
    [  1,  1,  2,  2,  4,  4,  6,  6,  8,  8,  8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68], // Q
    [  1,  1,  2,  4,  4,  4,  5,  6,  8,  8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81], // H
];

/// Modules left for data and error correction once the finder, separator, timing and alignment
/// patterns, the format information and (from version 7) the version information are removed.
/// Not always a multiple of 8: the 0, 3, 4 or 7 leftover remainder bits hold no codeword.
fn raw_data_modules(version: QRVersion) -> usize {
    let v = version.version as usize;
    let mut modules = (16 * v + 128) * v + 64;
    if v >= 2 {
        let alignment_per_side = v / 7 + 2;
        modules -= (25 * alignment_per_side - 10) * alignment_per_side - 55;
        if v >= 7 {
            modules -= 36;
        }
    }
    modules
}

/// Every codeword in the symbol, data and error correction combined.
pub(crate) fn total_codewords(version: QRVersion) -> usize {
    raw_data_modules(version) / 8
}

pub(crate) fn ecc_codewords_per_block(version: QRVersion, level: ErrorCorrectionLevel) -> usize {
    ECC_CODEWORDS_PER_BLOCK[level as usize][version.version as usize - 1] as usize
}

pub(crate) fn error_correction_blocks(version: QRVersion, level: ErrorCorrectionLevel) -> usize {
    ERROR_CORRECTION_BLOCKS[level as usize][version.version as usize - 1] as usize
}

/// Codewords available for data (segments, terminator and padding) at this version and level.
pub(crate) fn data_codewords(version: QRVersion, level: ErrorCorrectionLevel) -> usize {
    total_codewords(version)
        - ecc_codewords_per_block(version, level) * error_correction_blocks(version, level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ErrorCorrectionLevel::*;

    fn v(version: u8) -> QRVersion {
        QRVersion { version }
    }

    #[test]
    fn total_codewords_match_the_smallest_and_largest_symbols() {
        assert_eq!(total_codewords(v(1)), 26);
        assert_eq!(total_codewords(v(7)), 196);
        assert_eq!(total_codewords(v(40)), 3706);
    }

    #[test]
    fn version_1_data_codewords() {
        assert_eq!([L, M, Q, H].map(|level| data_codewords(v(1), level)), [19, 16, 13, 9]);
    }

    #[test]
    fn version_40_data_codewords() {
        assert_eq!([L, M, Q, H].map(|level| data_codewords(v(40), level)), [2956, 2334, 1666, 1276]);
    }

    #[test]
    fn version_5_q_splits_unevenly_across_blocks() {
        // 2 blocks of 15 data codewords and 2 of 16.
        assert_eq!(data_codewords(v(5), Q), 62);
    }
}
