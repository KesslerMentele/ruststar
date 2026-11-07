// Codeword counts for every version and error correction level, from ISO/IEC 18004's error
// correction block characteristics. Every block of a given symbol has the same number of error
// correction codewords; data codewords are whatever is left, spread as evenly as possible across
// the blocks.

use super::ErrorCorrectionLevel;

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

/// Error correction block characteristics for one version and level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EcBlockInfo {
    pub(crate) total_data_codewords: u16,
    pub(crate) ec_codewords_per_block: u16,
    pub(crate) group_1_blocks: u16,
    pub(crate) group_1_data_codewords: u16,
    pub(crate) group_2_blocks: u16,
    pub(crate) group_2_data_codewords: u16,
}

/// Error correction block characteristics, derived from ISO/IEC 18004 Table 9.
/// Indexed by `[version - 1][level]`, `level` ordered L, M, Q, H. Columns per row, matching
/// `EcBlockInfo`'s field order:
///   0: Total Data Codewords
///   1: EC Codewords (per block)
///   2: Number of blocks in Group 1
///   3: Number of Data Codewords in Group 1's Blocks
///   4: Number of Blocks in Group 2
///   5: Number of Data Codewords in Group 2's Blocks
///
const EC_TABLE: [[(u16, u16, u16, u16, u16, u16); 4]; 40] = [
    [(19, 7, 1, 19, 0, 0), (16, 10, 1, 16, 0, 0), (13, 13, 1, 13, 0, 0), (9, 17, 1, 9, 0, 0)],
    [(34, 10, 1, 34, 0, 0), (28, 16, 1, 28, 0, 0), (22, 22, 1, 22, 0, 0), (16, 28, 1, 16, 0, 0)],
    [(55, 15, 1, 55, 0, 0), (44, 26, 1, 44, 0, 0), (34, 18, 2, 17, 0, 0), (26, 22, 2, 13, 0, 0)],
    [(80, 20, 1, 80, 0, 0), (64, 18, 2, 32, 0, 0), (48, 26, 2, 24, 0, 0), (36, 16, 4, 9, 0, 0)],
    [(108, 26, 1, 108, 0, 0), (86, 24, 2, 43, 0, 0), (62, 18, 2, 15, 2, 16), (46, 22, 2, 11, 2, 12)],
    [(136, 18, 2, 68, 0, 0), (108, 16, 4, 27, 0, 0), (76, 24, 4, 19, 0, 0), (60, 28, 4, 15, 0, 0)],
    [(156, 20, 2, 78, 0, 0), (124, 18, 4, 31, 0, 0), (88, 18, 2, 14, 4, 15), (66, 26, 4, 13, 1, 14)],
    [(194, 24, 2, 97, 0, 0), (154, 22, 2, 38, 2, 39), (110, 22, 4, 18, 2, 19), (86, 26, 4, 14, 2, 15)],
    [(232, 30, 2, 116, 0, 0), (182, 22, 3, 36, 2, 37), (132, 20, 4, 16, 4, 17), (100, 24, 4, 12, 4, 13)],
    [(274, 18, 2, 68, 2, 69), (216, 26, 4, 43, 1, 44), (154, 24, 6, 19, 2, 20), (122, 28, 6, 15, 2, 16)],
    [(324, 20, 4, 81, 0, 0), (254, 30, 1, 50, 4, 51), (180, 28, 4, 22, 4, 23), (140, 24, 3, 12, 8, 13)],
    [(370, 24, 2, 92, 2, 93), (290, 22, 6, 36, 2, 37), (206, 26, 4, 20, 6, 21), (158, 28, 7, 14, 4, 15)],
    [(428, 26, 4, 107, 0, 0), (334, 22, 8, 37, 1, 38), (244, 24, 8, 20, 4, 21), (180, 22, 12, 11, 4, 12)],
    [(461, 30, 3, 115, 1, 116), (365, 24, 4, 40, 5, 41), (261, 20, 11, 16, 5, 17), (197, 24, 11, 12, 5, 13)],
    [(523, 22, 5, 87, 1, 88), (415, 24, 5, 41, 5, 42), (295, 30, 5, 24, 7, 25), (223, 24, 11, 12, 7, 13)],
    [(589, 24, 5, 98, 1, 99), (453, 28, 7, 45, 3, 46), (325, 24, 15, 19, 2, 20), (253, 30, 3, 15, 13, 16)],
    [(647, 28, 1, 107, 5, 108), (507, 28, 10, 46, 1, 47), (367, 28, 1, 22, 15, 23), (283, 28, 2, 14, 17, 15)],
    [(721, 30, 5, 120, 1, 121), (563, 26, 9, 43, 4, 44), (397, 28, 17, 22, 1, 23), (313, 28, 2, 14, 19, 15)],
    [(795, 28, 3, 113, 4, 114), (627, 26, 3, 44, 11, 45), (445, 26, 17, 21, 4, 22), (341, 26, 9, 13, 16, 14)],
    [(861, 28, 3, 107, 5, 108), (669, 26, 3, 41, 13, 42), (485, 30, 15, 24, 5, 25), (385, 28, 15, 15, 10, 16)],
    [(932, 28, 4, 116, 4, 117), (714, 26, 17, 42, 0, 0), (512, 28, 17, 22, 6, 23), (406, 30, 19, 16, 6, 17)],
    [(1006, 28, 2, 111, 7, 112), (782, 28, 17, 46, 0, 0), (568, 30, 7, 24, 16, 25), (442, 24, 34, 13, 0, 0)],
    [(1094, 30, 4, 121, 5, 122), (860, 28, 4, 47, 14, 48), (614, 30, 11, 24, 14, 25), (464, 30, 16, 15, 14, 16)],
    [(1174, 30, 6, 117, 4, 118), (914, 28, 6, 45, 14, 46), (664, 30, 11, 24, 16, 25), (514, 30, 30, 16, 2, 17)],
    [(1276, 26, 8, 106, 4, 107), (1000, 28, 8, 47, 13, 48), (718, 30, 7, 24, 22, 25), (538, 30, 22, 15, 13, 16)],
    [(1370, 28, 10, 114, 2, 115), (1062, 28, 19, 46, 4, 47), (754, 28, 28, 22, 6, 23), (596, 30, 33, 16, 4, 17)],
    [(1468, 30, 8, 122, 4, 123), (1128, 28, 22, 45, 3, 46), (808, 30, 8, 23, 26, 24), (628, 30, 12, 15, 28, 16)],
    [(1531, 30, 3, 117, 10, 118), (1193, 28, 3, 45, 23, 46), (871, 30, 4, 24, 31, 25), (661, 30, 11, 15, 31, 16)],
    [(1631, 30, 7, 116, 7, 117), (1267, 28, 21, 45, 7, 46), (911, 30, 1, 23, 37, 24), (701, 30, 19, 15, 26, 16)],
    [(1735, 30, 5, 115, 10, 116), (1373, 28, 19, 47, 10, 48), (985, 30, 15, 24, 25, 25), (745, 30, 23, 15, 25, 16)],
    [(1843, 30, 13, 115, 3, 116), (1455, 28, 2, 46, 29, 47), (1033, 30, 42, 24, 1, 25), (793, 30, 23, 15, 28, 16)],
    [(1955, 30, 17, 115, 0, 0), (1541, 28, 10, 46, 23, 47), (1115, 30, 10, 24, 35, 25), (845, 30, 19, 15, 35, 16)],
    [(2071, 30, 17, 115, 1, 116), (1631, 28, 14, 46, 21, 47), (1171, 30, 29, 24, 19, 25), (901, 30, 11, 15, 46, 16)],
    [(2191, 30, 13, 115, 6, 116), (1725, 28, 14, 46, 23, 47), (1231, 30, 44, 24, 7, 25), (961, 30, 59, 16, 1, 17)],
    [(2306, 30, 12, 121, 7, 122), (1812, 28, 12, 47, 26, 48), (1286, 30, 39, 24, 14, 25), (986, 30, 22, 15, 41, 16)],
    [(2434, 30, 6, 121, 14, 122), (1914, 28, 6, 47, 34, 48), (1354, 30, 46, 24, 10, 25), (1054, 30, 2, 15, 64, 16)],
    [(2566, 30, 17, 122, 4, 123), (1992, 28, 29, 46, 14, 47), (1426, 30, 49, 24, 10, 25), (1096, 30, 24, 15, 46, 16)],
    [(2702, 30, 4, 122, 18, 123), (2102, 28, 13, 46, 32, 47), (1502, 30, 48, 24, 14, 25), (1142, 30, 42, 15, 32, 16)],
    [(2812, 30, 20, 117, 4, 118), (2216, 28, 40, 47, 7, 48), (1582, 30, 43, 24, 22, 25), (1222, 30, 10, 15, 67, 16)],
    [(2956, 30, 19, 118, 6, 119), (2334, 28, 18, 47, 31, 48), (1666, 30, 34, 24, 34, 25), (1276, 30, 20, 15, 61, 16)],
];

/// Modules left for data and error correction once the finder, separator, timing and alignment
/// patterns, the format information and (from version 7) the version information are removed.
/// Not always a multiple of 8: the 0, 3, 4 or 7 leftover remainder bits hold no codeword.
pub(crate) fn raw_data_modules(v: usize) -> usize {

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
pub(crate) fn total_codewords(version: u8) -> usize {
    raw_data_modules(version.into()) / 8
}

pub(crate) fn ecc_codewords_per_block(version: u8, level: ErrorCorrectionLevel) -> usize {
    let v:usize = version.into();
    ECC_CODEWORDS_PER_BLOCK[level as usize][v - 1] as usize
}

pub(crate) fn error_correction_blocks(version: u8, level: ErrorCorrectionLevel) -> usize {
    let v:usize = version.into();
    ERROR_CORRECTION_BLOCKS[level as usize][v - 1] as usize
}

/// Retrieves the error correction information for a given version and `ErrorCorrectionLevel`.
/// Returns an `EcBlockInfo`.
pub(crate) fn ec_block_info(version: u8, level: ErrorCorrectionLevel) -> EcBlockInfo {
    let v:usize = version.into();
    let (total_data_codewords, ec_codewords_per_block, group_1_blocks, group_1_data_codewords,
        group_2_blocks, group_2_data_codewords) = EC_TABLE[v - 1][level as usize];
    EcBlockInfo {
        total_data_codewords,
        ec_codewords_per_block,
        group_1_blocks,
        group_1_data_codewords,
        group_2_blocks,
        group_2_data_codewords,
    }
}

/// Codewords available for data (segments, terminator and padding) at this version and level.
pub(crate) fn data_codewords(version: u8, level: ErrorCorrectionLevel) -> usize {
    total_codewords(version)
        - ecc_codewords_per_block(version, level) * error_correction_blocks(version, level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ErrorCorrectionLevel::*;

    #[test]
    fn total_codewords_match_the_smallest_and_largest_symbols() {
        assert_eq!(total_codewords(1), 26);
        assert_eq!(total_codewords(7), 196);
        assert_eq!(total_codewords(40), 3706);
    }

    #[test]
    fn version_1_data_codewords() {
        assert_eq!([L, M, Q, H].map(|level| data_codewords(1, level)), [19, 16, 13, 9]);
    }

    #[test]
    fn version_40_data_codewords() {
        assert_eq!([L, M, Q, H].map(|level| data_codewords(40, level)), [2956, 2334, 1666, 1276]);
    }

    #[test]
    fn version_5_q_splits_unevenly_across_blocks() {
        // 2 blocks of 15 data codewords and 2 of 16.
        assert_eq!(data_codewords(5, Q), 62);
    }
}
