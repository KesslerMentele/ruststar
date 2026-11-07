// The function patterns of a QR symbol (ISO/IEC 18004 section 6.3): every module that isn't
// data or error correction. They're drawn onto an empty matrix before data placement, and marked
// as function modules so that placement and masking skip them.
//
// Layout, for a symbol `size` = 4 * version + 17 modules wide:
//  - Finder patterns: 7x7 concentric squares in the top-left, top-right and bottom-left corners.
//  - Separators: a 1-module light border around each finder, on its inner sides.
//  - Timing patterns: alternating modules along row 6 and column 6, between the separators.
//  - Dark module: always dark, at (4 * version + 9, 8).
//  - Alignment patterns: 5x5 concentric squares (version 2+), centered on every pairing of the
//    version's alignment coordinates, except the three that would overlap a finder.
//  - Format information: 15 modules beside the top-left finder, mirrored across the other two.
//  - Version information: 6x3 blocks beside the top-right and bottom-left finders (version 7+).
//
// Format and version information depend on the error correction level, mask and version
// encodings, so for now their areas are only reserved (left light).

use std::ops::RangeInclusive;
use bitvec::prelude::*;
use crate::services::qr_code::{ErrorCorrectionLevel, MaskSetting};
use crate::services::qr_code::penalty_score::penalty_score;

/// Alignment pattern center coordinates for each version, indexed by `version - 1`. Every pairing
/// of two coordinates is a center, apart from the three that overlap a finder pattern.
const ALIGNMENT_PATTERN_COORDINATES: [&[u8]; 40] = [
    &[],
    &[6, 18],
    &[6, 22],
    &[6, 26],
    &[6, 30],
    &[6, 34],
    &[6, 22, 38],
    &[6, 24, 42],
    &[6, 26, 46],
    &[6, 28, 50],
    &[6, 30, 54],
    &[6, 32, 58],
    &[6, 34, 62],
    &[6, 26, 46, 66],
    &[6, 26, 48, 70],
    &[6, 26, 50, 74],
    &[6, 30, 54, 78],
    &[6, 30, 56, 82],
    &[6, 30, 58, 86],
    &[6, 34, 62, 90],
    &[6, 28, 50, 72, 94],
    &[6, 26, 50, 74, 98],
    &[6, 30, 54, 78, 102],
    &[6, 28, 54, 80, 106],
    &[6, 32, 58, 84, 110],
    &[6, 30, 58, 86, 114],
    &[6, 34, 62, 90, 118],
    &[6, 26, 50, 74, 98, 122],
    &[6, 30, 54, 78, 102, 126],
    &[6, 26, 52, 78, 104, 130],
    &[6, 30, 56, 82, 108, 134],
    &[6, 34, 60, 86, 112, 138],
    &[6, 30, 58, 86, 114, 142],
    &[6, 34, 62, 90, 118, 146],
    &[6, 30, 54, 78, 102, 126, 150],
    &[6, 24, 50, 76, 102, 128, 154],
    &[6, 28, 54, 80, 106, 132, 158],
    &[6, 32, 58, 84, 110, 136, 162],
    &[6, 26, 54, 82, 110, 138, 166],
    &[6, 30, 58, 86, 114, 142, 170],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaskError {
    /// `pattern` is outside the valid 0-7 range of mask patterns.
    InvalidMaskVersion(u8),
}

/// The data-masking predicate for mask pattern `pattern` (0-7): `true` means "invert this
/// module". ISO/IEC 18004 Table 10.
///
/// Returns `Err(MaskError::InvalidMaskVersion(pattern))` if `pattern` is greater than 7.
pub(crate) fn mask_predicate(pattern: u8) -> Result<fn(usize, usize) -> bool, MaskError> {
    match pattern {
        0 => Ok(|row: usize, col: usize| (row + col) % 2 == 0),
        1 => Ok(|row: usize, _col: usize| row % 2 == 0),
        2 => Ok(|_row: usize, col: usize| col % 3 == 0),
        3 => Ok(|row: usize, col: usize| (row + col) % 3 == 0),
        4 => Ok(|row: usize, col: usize| (row / 2 + col / 3) % 2 == 0),
        5 => Ok(|row: usize, col: usize| (row * col) % 2 + (row * col) % 3 == 0),
        6 => Ok(|row: usize, col: usize| ((row * col) % 2 + (row * col) % 3) % 2 == 0),
        7 => Ok(|row: usize, col: usize| ((row * col) % 3 + (row + col) % 2) % 2 == 0),
        _ => Err(MaskError::InvalidMaskVersion(pattern)),
    }
}

/// The number of bits needed to represent `value` (0 for zero), i.e. its index of its highest
/// set bit, plus one.
fn bit_length(mut value: u32) -> u32 {
    let mut length = 0;
    while value != 0 {
        length += 1;
        value >>= 1;
    }
    length
}

/// Divides `data` (shifted left by `generator`'s degree) by `generator` over GF(2), returning
/// the remainder: the error-correction bits for a BCH code. Used for both format and version
/// information, which each reserve a fixed number of bits for this remainder alongside `data`.
fn bch_remainder(data: u32, generator: u32) -> u32 {
    let generator_length = bit_length(generator);
    let mut remainder = data;
    while bit_length(remainder) >= generator_length {
        remainder ^= generator << (bit_length(remainder) - generator_length);
    }
    remainder
}

/// The 15-bit format information codeword for `data` (5 bits: a 2-bit error correction level
/// indicator, then a 3-bit mask pattern), BCH(15,5) encoded and XORed with the format
/// information mask. ISO/IEC 18004 Annex C.
pub(crate) fn bch_format_information(data: u32) -> u32 {
    const GENERATOR: u32 = 0x537;
    const MASK: u32 = 0x5412;
    let shifted = data << 10;
    (shifted | bch_remainder(shifted, GENERATOR)) ^ MASK
}

/// Where bit `i` (0-14) of the format information sits in each of its two copies, for a symbol
/// `size` modules wide, as `(row, col)`: first the copy in column 8, then the copy in row 8.
pub(crate) fn format_information_positions(size: usize, i: usize) -> [(usize, usize); 2] {
    let row = match i {
        0..=5 => i,
        6..=7 => i + 1,
        _ => size - 15 + i,
    };
    let col = match i {
        0..=7 => size - 1 - i,
        8 => 7,
        _ => 14 - i,
    };
    [(row, 8), (8, col)]
}

/// Every data module of a `version` symbol as `(row, col)`, in the order data bits are placed.
/// The layout depends only on the version, so this needs no actual symbol to read from.
pub(crate) fn data_module_positions(version: u8) -> Vec<(usize, usize)> {
    // The level only affects the contents of the format information, never which modules it owns.
    QRMatrix::new(version, ErrorCorrectionLevel::M).data_module_order().collect()
}

/// The 18-bit version information codeword for `version` (6 bits), BCH(18,6) encoded.
/// ISO/IEC 18004 Annex D.
fn bch_version_information(version: u32) -> u32 {
    const GENERATOR: u32 = 0x1F25;
    let shifted = version << 12;
    shifted | bch_remainder(shifted, GENERATOR)
}



/// A square grid of modules, stored row-major (the layout `penalty_score` expects), alongside
/// which of those modules belong to a function pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QRMatrix {
    version: u8,
    error_correction_level: ErrorCorrectionLevel,
    size: usize,
    /// `true` is dark.
    modules: BitVec<u8, Lsb0>,
    /// `true` where a function pattern owns the module, so data placement and masking skip it.
    function: BitVec<u8, Lsb0>,
    /// The mask pattern currently applied to the data modules, if any.
    mask: Option<u8>,
}

impl QRMatrix {
    /// An all-light matrix for `version`, with no function patterns drawn.
    pub(crate) fn new_empty(version: u8, error_correction_level: ErrorCorrectionLevel) -> Self {
        let size = version as usize * 4 + 17;
        QRMatrix {
            version,
            error_correction_level,
            size,
            modules: bitvec![u8, Lsb0; 0; size * size],
            function: bitvec![u8, Lsb0; 0; size * size],
            mask: None,
        }
    }

    /// A matrix for `version` with every function pattern drawn, the version information
    /// written (version 7+), and the format information area reserved (left light until
    /// `set_mask`/`with_mask` picks a mask and fills it in).
    pub(crate) fn new(version: u8, error_correction_level: ErrorCorrectionLevel) -> Self {
        let mut matrix = QRMatrix::new_empty(version, error_correction_level);
        matrix.draw_finder_patterns();
        matrix.draw_separators();
        matrix.draw_timing_patterns();
        matrix.draw_alignment_patterns();
        matrix.draw_dark_module();
        matrix.reserve_format_information();
        matrix.reserve_version_information();
        matrix.write_version_information();
        matrix
    }

    /// Width and height, in modules.
    pub(crate) fn size(&self) -> usize {
        self.size
    }

    /// The version this matrix was built for.
    pub(crate) fn version(&self) -> u8 {
        self.version
    }

    /// Every module, row-major, `true` for dark.
    pub(crate) fn modules(&self) -> &BitSlice<u8, Lsb0> {
        &self.modules
    }

    pub(crate) fn is_dark(&self, row: usize, col: usize) -> bool {
        self.modules[row * self.size + col]
    }

    pub(crate) fn is_function(&self, row: usize, col: usize) -> bool {
        self.function[row * self.size + col]
    }

    /// Sets a module and marks it as part of a function pattern.
    fn set_function(&mut self, row: usize, col: usize, dark: bool) {
        let index = row * self.size + col;
        self.modules.set(index, dark);
        self.function.set(index, true);
    }

    /// Sets every module whose Chebyshev distance from `(center_row, center_col)` is in `rings`
    /// to `dark(distance)`, skipping any that fall outside the matrix.
    fn draw_rings(&mut self, center_row: usize, center_col: usize, rings: RangeInclusive<usize>, dark: impl Fn(usize) -> bool) {
        let radius = *rings.end() as isize;
        for row_offset in -radius..=radius {
            for col_offset in -radius..=radius {
                let distance = row_offset.unsigned_abs().max(col_offset.unsigned_abs());
                let row = center_row as isize + row_offset;
                let col = center_col as isize + col_offset;
                let in_bounds = (0..self.size as isize).contains(&row) && (0..self.size as isize).contains(&col);
                if in_bounds && rings.contains(&distance) {
                    self.set_function(row as usize, col as usize, dark(distance));
                }
            }
        }
    }

    /// Centers of the three finder patterns: top-left, top-right, bottom-left.
    fn finder_centers(&self) -> [(usize, usize); 3] {
        let far = self.size - 4;
        [(3, 3), (3, far), (far, 3)]
    }

    /// A dark 7x7 outline, a light 5x5 outline, then a dark 3x3 center, in each of three corners.
    fn draw_finder_patterns(&mut self) {
        for (row, col) in self.finder_centers() {
            self.draw_rings(row, col, 0..=3, |distance| distance != 2);
        }
    }

    /// The light ring just outside each finder pattern. Only its inner sides land in the matrix.
    fn draw_separators(&mut self) {
        for (row, col) in self.finder_centers() {
            self.draw_rings(row, col, 4..=4, |_| false);
        }
    }

    /// Row 6 and column 6 between the separators, dark on even indices.
    fn draw_timing_patterns(&mut self) {
        for i in 8..self.size - 8 {
            let dark = i % 2 == 0;
            self.set_function(6, i, dark);
            self.set_function(i, 6, dark);
        }
    }

    /// A dark 5x5 outline, a light 3x3 outline, then a dark center module, at every alignment
    /// center that doesn't overlap a finder pattern. Those on row or column 6 overlap the timing
    /// patterns, but agree with them module for module, since every center is even.
    fn draw_alignment_patterns(&mut self) {
        let coordinates:&[u8] = ALIGNMENT_PATTERN_COORDINATES[self.version as usize - 1];
        let (Some(&first), Some(&last)) = (coordinates.first(), coordinates.last()) else {
            return;
        };
        for &row in coordinates {
            for &col in coordinates {
                let overlaps_finder = [(first, first), (first, last), (last, first)].contains(&(row, col));
                if !overlaps_finder {
                    self.draw_rings(row as usize, col as usize, 0..=2, |distance| distance != 1);
                }
            }
        }
    }

    /// The single module above the bottom-left finder's separator that is always dark.
    fn draw_dark_module(&mut self) {
        self.set_function(self.version as usize * 4 + 9, 8, true);
    }

    /// The two copies of the 15 format information modules: wrapped around the top-left finder
    /// (skipping the timing patterns), and split between the top-right and bottom-left finders.
    fn reserve_format_information(&mut self) {
        for i in (0..=8).filter(|&i| i != 6) {
            self.set_function(8, i, false);
            self.set_function(i, 8, false);
        }
        for i in 0..8 {
            self.set_function(8, self.size - 1 - i, false);
        }
        for i in 0..7 {
            self.set_function(self.size - 1 - i, 8, false);
        }
    }

    /// The two 6x3 copies of the version information, above the bottom-left finder and left of
    /// the top-right finder. Only version 7 and up carry it.
    fn reserve_version_information(&mut self) {
        if self.version < 7 {
            return;
        }
        for i in 0..6 {
            for j in self.size - 11..self.size - 8 {
                self.set_function(i, j, false);
                self.set_function(j, i, false);
            }
        }
    }

    /// Fills in the version information reserved by `reserve_version_information` (version 7+;
    /// a no-op below that). Unlike format information, this depends only on the version, so
    /// it's written once, at construction, rather than whenever the mask changes.
    fn write_version_information(&mut self) {
        let version: usize = self.version.into();
        if version < 7 {
            return;
        }
        let bits = bch_version_information(version as u32);
        for i in 0..18usize {
            let dark = (bits >> i) & 1 == 1;
            self.set_function(i / 3, i % 3 + self.size - 11, dark);
            self.set_function(i % 3 + self.size - 11, i / 3, dark);
        }
    }

    /// Fills in the format information reserved by `reserve_format_information`, for error
    /// correction level `self.error_correction_level` and mask pattern `pattern`.
    fn write_format_information(&mut self, pattern: u8) {
        let level_bits: u32 = match self.error_correction_level {
            ErrorCorrectionLevel::L => 0b01,
            ErrorCorrectionLevel::M => 0b00,
            ErrorCorrectionLevel::Q => 0b11,
            ErrorCorrectionLevel::H => 0b10,
        };
        let bits = bch_format_information((level_bits << 3) | pattern as u32);
        for i in 0..15usize {
            let dark = (bits >> i) & 1 == 1;
            for (row, col) in format_information_positions(self.size, i) {
                self.set_function(row, col, dark);
            }
        }
    }

    /// Every non-function module, in the order data bits are placed: two-column strips from the
    /// right edge leftwards, alternating upwards and downwards, right column before left within
    /// each row. Column 6 is skipped entirely, so strips left of it start one column over.
    pub(crate) fn data_module_order(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        // `size - 1` is always a multiple of 4, so the strips' right columns run size - 1, ..., 8,
        // then 5, 3, 1 once column 6 is skipped.
        let right_columns = (7..self.size).rev().step_by(2).chain([5, 3, 1]);
        right_columns.enumerate().flat_map(move |(strip, right)| {
            let upwards = strip % 2 == 0;
            (0..self.size).flat_map(move |step| {
                let row = if upwards { self.size - 1 - step } else { step };
                [(row, right), (row, right - 1)]
            })
        })
        .filter(|&(row, col)| !self.is_function(row, col))
    }

    /// Places `bits` into the data modules in `data_module_order`, MSB of the first codeword
    /// first. Modules left over after the last bit (the 0 to 7 remainder bits) are left light.
    ///
    /// Panics if `bits` holds more bits than the data modules do.
    pub(crate) fn place_data(&mut self, bits: &BitVec<u8, Lsb0>) {
        let positions: Vec<(usize, usize)> = self.data_module_order().collect();
        assert!(
            bits.len() <= positions.len(),
            "{} bits don't fit in {} data modules",
            bits.len(),
            positions.len()
        );
        for (i, (row, col)) in positions.into_iter().enumerate() {
            let dark = bits.get(i).is_some_and(|bit| *bit);
            self.modules.set(row * self.size + col, dark);
        }
    }

    /// Toggles every non-function module for which `mask_predicate(pattern)` holds. Applying
    /// the same pattern twice cancels out, since masking is its own inverse.
    fn apply_mask(&mut self, pattern: u8) -> Result<(), MaskError> {
        let predicate = mask_predicate(pattern)?;
        for row in 0..self.size {
            for col in 0..self.size {
                if !self.is_function(row, col) && predicate(row, col) {
                    let index = row * self.size + col;
                    let dark = self.modules[index];
                    self.modules.set(index, !dark);
                }
            }
        }
        Ok(())
    }

    /// The mask pattern currently applied to the data modules, if any.
    pub(crate) fn mask(&self) -> Option<u8> {
        self.mask
    }

    /// Selects mask pattern `pattern` (0-7): toggles the data modules it covers and writes the
    /// matching format information, replacing whichever mask (if any) was previously applied.
    ///
    /// Returns `Err(MaskError::InvalidMaskVersion(pattern))`, without changing anything, if
    /// `pattern` is greater than 7.
    pub(crate) fn set_mask(&mut self, pattern: MaskSetting) -> Result<(), MaskError> {
        match pattern {
            MaskSetting::Exact(val) => {
                mask_predicate(val)?;
                if let Some(previous) = self.mask {
                    self.apply_mask(previous)?;
                }
                self.apply_mask(val)?;
                self.mask = Some(val);
                self.write_format_information(val);
                Ok(())
            }
            MaskSetting::Auto => {
                // try all the masks and select the mask with the lowest penalty
                let value = (0..8).map( |mask| {
                    let proposal = &self.with_mask(mask).unwrap(); // This is only ever iterating on 0..8, guaranteed safe
                    return (penalty_score(proposal, self.size, self.size).total, mask) 
                }).min_by_key(|(k, _)| *k).map(|(_,mask)| mask).unwrap();
      
                self.set_mask(MaskSetting::Exact(value))
                
            }
        }

    }

    /// Every module, row-major, as it would read if mask pattern `pattern` (0-7) were selected
    /// — the data modules it covers toggled, and the matching format information written —
    /// without changing the mask actually applied to `self`. See `modules`.
    ///
    /// Returns `Err(MaskError::InvalidMaskVersion(pattern))` if `pattern` is greater than 7.
    pub(crate) fn with_mask(&self, pattern: u8) -> Result<BitVec<u8, Lsb0>, MaskError> {
        let mut candidate = self.clone();
        candidate.set_mask(MaskSetting::Exact(pattern))?;
        Ok(candidate.modules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::qr_code::capacity::raw_data_modules;

    /// Renders a region as rows of `#` (dark function), `.` (light function) or ` ` (not a
    /// function module).
    fn render(matrix: &QRMatrix, rows: RangeInclusive<usize>, cols: RangeInclusive<usize>) -> Vec<String> {
        rows.map(|row| {
            cols.clone()
                .map(|col| match (matrix.is_function(row, col), matrix.is_dark(row, col)) {
                    (false, _) => ' ',
                    (true, true) => '#',
                    (true, false) => '.',
                })
                .collect()
        })
        .collect()
    }

    #[test]
    fn size_grows_by_four_per_version() {
        assert_eq!(QRMatrix::new_empty(1, ErrorCorrectionLevel::M).size(), 21);
        assert_eq!(QRMatrix::new_empty(40, ErrorCorrectionLevel::M).size(), 177);
    }

    #[test]
    fn top_left_corner_of_version_1() {
        // Finder, separator, timing modules at (6, 8) and (8, 6), and the reserved format area.
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        assert_eq!(
            render(&matrix, 0..=8, 0..=8),
            [
                "#######..",
                "#.....#..",
                "#.###.#..",
                "#.###.#..",
                "#.###.#..",
                "#.....#..",
                "#######.#",
                ".........",
                "......#..",
            ]
        );
    }

    #[test]
    fn finder_patterns_sit_in_three_corners() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        let finder = ["#######", "#.....#", "#.###.#", "#.###.#", "#.###.#", "#.....#", "#######"];
        assert_eq!(render(&matrix, 0..=6, 14..=20), finder);
        assert_eq!(render(&matrix, 14..=20, 0..=6), finder);
        // The bottom-right corner has no finder, so nothing there is a function module.
        assert_eq!(render(&matrix, 14..=20, 14..=20), vec![" ".repeat(7); 7]);
    }

    #[test]
    fn separators_border_the_inner_sides_of_each_finder() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        for i in 0..8 {
            for (row, col) in [(7, i), (i, 7), (7, 20 - i), (i, 13), (13, i), (20 - i, 7)] {
                assert!(matrix.is_function(row, col) && !matrix.is_dark(row, col), "({row}, {col})");
            }
        }
    }

    #[test]
    fn timing_patterns_alternate_between_the_separators() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        assert_eq!(render(&matrix, 6..=6, 7..=13), [".#.#.#."]);
        assert_eq!(render(&matrix, 7..=13, 6..=6), [".", "#", ".", "#", ".", "#", "."]);
    }

    #[test]
    fn dark_module_sits_above_the_bottom_left_separator() {
        for version in [1, 7, 40] {
            let matrix = QRMatrix::new(version, ErrorCorrectionLevel::M);
            let row = version as usize * 4 + 9;
            assert!(matrix.is_function(row, 8) && matrix.is_dark(row, 8), "version {version}");
            assert_eq!(row, matrix.size() - 8);
        }
    }

    #[test]
    fn version_1_has_no_alignment_pattern() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        assert!(!matrix.is_function(14, 14));
    }

    #[test]
    fn version_2_has_one_alignment_pattern() {
        let matrix = QRMatrix::new(2, ErrorCorrectionLevel::M);
        assert!(is_alignment_center(&matrix, 18, 18));
    }

    /// Whether `(row, col)` is the center of an alignment pattern.
    fn is_alignment_center(matrix: &QRMatrix, row: usize, col: usize) -> bool {
        render(matrix, row - 2..=row + 2, col - 2..=col + 2) == ["#####", "#...#", "#.#.#", "#...#", "#####"]
    }

    #[test]
    fn alignment_patterns_skip_the_finder_corners() {
        // Version 7's coordinates are 6, 22, 38: nine pairings, less the three finder corners.
        let matrix = QRMatrix::new(7, ErrorCorrectionLevel::M);
        let centers: Vec<(usize, usize)> = [6, 22, 38]
            .into_iter()
            .flat_map(|row| [6, 22, 38].map(|col| (row, col)))
            .filter(|&(row, col)| is_alignment_center(&matrix, row, col))
            .collect();
        assert_eq!(centers, [(6, 22), (22, 6), (22, 22), (22, 38), (38, 22), (38, 38)]);

        let finder = ["#######", "#.....#", "#.###.#", "#.###.#", "#.###.#", "#.....#", "#######"];
        assert_eq!(render(&matrix, 0..=6, 0..=6), finder);
        assert_eq!(render(&matrix, 0..=6, 38..=44), finder);
        assert_eq!(render(&matrix, 38..=44, 0..=6), finder);
    }

    #[test]
    fn alignment_patterns_agree_with_the_timing_patterns() {
        // The (6, 22) alignment pattern in version 7 crosses row 6's timing pattern.
        let matrix = QRMatrix::new(7, ErrorCorrectionLevel::M);
        for col in 8..matrix.size() - 8 {
            assert_eq!(matrix.is_dark(6, col), col % 2 == 0, "column {col}");
        }
    }

    #[test]
    fn alignment_coordinates_span_the_symbol() {
        for version in 2..=40u8 {
            let coordinates = ALIGNMENT_PATTERN_COORDINATES[version as usize - 1];
            assert_eq!(coordinates.len(), version as usize / 7 + 2, "version {version}");
            assert_eq!(coordinates[0], 6);
            assert_eq!(*coordinates.last().unwrap() as usize, QRMatrix::new_empty(version, ErrorCorrectionLevel::M).size() - 7);
        }
    }

    #[test]
    fn version_information_is_reserved_from_version_7() {
        let below = QRMatrix::new(6, ErrorCorrectionLevel::M);
        assert!(!below.is_function(0, below.size() - 11));

        let matrix = QRMatrix::new(7, ErrorCorrectionLevel::M);
        let size = matrix.size();
        for i in 0..6 {
            for j in size - 11..size - 8 {
                assert!(matrix.is_function(i, j) && matrix.is_function(j, i), "({i}, {j})");
            }
        }
    }

    #[test]
    fn data_order_starts_upwards_from_the_bottom_right_corner() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        let order: Vec<_> = matrix.data_module_order().take(4).collect();
        assert_eq!(order, [(20, 20), (20, 19), (19, 20), (19, 19)]);
    }

    #[test]
    fn data_order_turns_downwards_below_the_top_right_format_area() {
        // The first strip runs rows 20 up to 9 (24 modules); row 8 is format information, so the
        // next strip starts back down at row 9, in columns 18 and 17.
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        let order: Vec<_> = matrix.data_module_order().skip(22).take(4).collect();
        assert_eq!(order, [(9, 20), (9, 19), (9, 18), (9, 17)]);
    }

    #[test]
    fn data_order_skips_the_vertical_timing_column() {
        // Version 1's last three strips are columns 5-4 (down), 3-2 (up) and 1-0 (down).
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        let order: Vec<_> = matrix.data_module_order().collect();
        assert!(order.iter().all(|&(_, col)| col != 6));
        let last_strip_start = order.iter().position(|&(_, col)| col <= 5).unwrap();
        assert_eq!(order[last_strip_start..last_strip_start + 2], [(9, 5), (9, 4)]);
        assert_eq!(*order.last().unwrap(), (12, 0));
    }

    #[test]
    fn data_order_visits_every_data_module_exactly_once() {
        for version in 1..=40u8 {
            let matrix = QRMatrix::new(version, ErrorCorrectionLevel::M);
            let order: Vec<_> = matrix.data_module_order().collect();
            let distinct: std::collections::HashSet<_> = order.iter().collect();
            assert_eq!(order.len(), raw_data_modules(version.try_into().unwrap()), "version {version}");
            assert_eq!(distinct.len(), order.len(), "version {version}");
        }
    }

    #[test]
    fn place_data_writes_bits_in_data_order() {
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        let bits = bitvec![u8, Lsb0; 1, 0, 1, 1, 0, 0, 1];
        matrix.place_data(&bits);
        let placed: Vec<bool> = matrix.data_module_order().take(bits.len()).map(|(row, col)| matrix.is_dark(row, col)).collect();
        assert_eq!(placed, bits.iter().map(|bit| *bit).collect::<Vec<_>>());
    }

    #[test]
    fn place_data_leaves_function_modules_alone() {
        let blank = QRMatrix::new(7, ErrorCorrectionLevel::M);
        let mut matrix = blank.clone();
        let capacity = raw_data_modules(7);
        matrix.place_data(&bitvec![u8, Lsb0; 1; capacity]);
        for row in 0..matrix.size() {
            for col in 0..matrix.size() {
                if matrix.is_function(row, col) {
                    assert_eq!(matrix.is_dark(row, col), blank.is_dark(row, col), "({row}, {col})");
                } else {
                    assert!(matrix.is_dark(row, col), "({row}, {col})");
                }
            }
        }
    }

    #[test]
    fn place_data_leaves_the_remainder_bits_light() {
        // Version 2 has 359 data modules: 44 codewords (352 bits) and 7 remainder bits.
        let mut matrix = QRMatrix::new(2, ErrorCorrectionLevel::M);
        matrix.place_data(&bitvec![u8, Lsb0; 1; 352]);
        let dark: Vec<bool> = matrix.data_module_order().map(|(row, col)| matrix.is_dark(row, col)).collect();
        assert!(dark[..352].iter().all(|&d| d));
        assert!(dark[352..].iter().all(|&d| !d));
        assert_eq!(dark.len() - 352, 7);
    }

    #[test]
    #[should_panic(expected = "209 bits don't fit in 208 data modules")]
    fn place_data_rejects_more_bits_than_data_modules() {
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        matrix.place_data(&bitvec![u8, Lsb0; 0; 209]);
    }

    #[test]
    fn place_data_accepts_create_qr_code_bits_output() {
        use crate::services::qr_code::{create_qr_code_bits, EncodingMode, ErrorCorrectionLevel, MaskSetting, QRData, QRVersion};
        let bits = create_qr_code_bits(&QRData::new("01234567", ErrorCorrectionLevel::M, EncodingMode::Segmented, QRVersion::Exact(1), MaskSetting::Auto)).unwrap().bits;
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        matrix.place_data(&bits);
        // Mode indicator 0001: the first three modules light, the fourth dark.
        let first: Vec<bool> = matrix.data_module_order().take(4).map(|(row, col)| matrix.is_dark(row, col)).collect();
        assert_eq!(first, [false, false, false, true]);
    }

    #[test]
    fn function_patterns_leave_exactly_the_data_modules_free() {
        // Cross-checks every pattern's footprint against the capacity tables' module count.
        for version in 1..=40u8 {
            let matrix = QRMatrix::new(version, ErrorCorrectionLevel::M);
            let free = matrix.size() * matrix.size() - matrix.function.count_ones();
            assert_eq!(free, raw_data_modules(version.try_into().unwrap()), "version {version}");
        }
    }

    #[test]
    fn bch_format_information_matches_reference_values() {
        // From Python's `qrcode` library (util.BCH_type_info), which implements ISO/IEC 18004
        // Annex C: (error correction level << 3 | mask pattern) -> the 15-bit codeword.
        let cases = [
            (0b01_000, 30660), // L, mask 0
            (0b00_000, 21522), // M, mask 0
            (0b00_101, 16590), // M, mask 5
            (0b11_011, 14854), // Q, mask 3
            (0b10_111, 2107),  // H, mask 7
        ];
        for (data, expected) in cases {
            assert_eq!(bch_format_information(data), expected, "data {data:#07b}");
        }
    }

    #[test]
    fn bch_version_information_matches_reference_values() {
        // From Python's `qrcode` library (util.BCH_type_number), ISO/IEC 18004 Annex D.
        let cases = [(7, 31892), (8, 34236), (25, 102881), (40, 167017)];
        for (version, expected) in cases {
            assert_eq!(bch_version_information(version), expected, "version {version}");
        }
    }

    #[test]
    fn version_information_matches_the_bch_codeword() {
        let matrix = QRMatrix::new(7, ErrorCorrectionLevel::M);
        let bits = bch_version_information(7);
        let size = matrix.size();
        for i in 0..18usize {
            let expected = (bits >> i) & 1 == 1;
            assert_eq!(matrix.is_dark(i / 3, i % 3 + size - 11), expected, "block bit {i}");
            assert_eq!(matrix.is_dark(i % 3 + size - 11, i / 3), expected, "mirrored bit {i}");
        }
    }

    #[test]
    fn set_mask_writes_format_information_at_the_spec_positions() {
        // Positions for bit i = 0..14, hand-derived from ISO/IEC 18004 Figure 25, at version 1
        // (size 21) — independent of `write_format_information`'s own formula.
        let col8_rows = [0, 1, 2, 3, 4, 5, 7, 8, 14, 15, 16, 17, 18, 19, 20];
        let row8_cols = [20, 19, 18, 17, 16, 15, 14, 13, 7, 5, 4, 3, 2, 1, 0];

        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        matrix.set_mask(MaskSetting::Exact(0)).unwrap();
        let expected = bch_format_information(0b00_000); // M, mask 0

        for i in 0..15 {
            let bit = (expected >> i) & 1 == 1;
            assert_eq!(matrix.is_dark(col8_rows[i], 8), bit, "col8 bit {i}");
            assert_eq!(matrix.is_dark(8, row8_cols[i]), bit, "row8 bit {i}");
        }
    }

    #[test]
    fn set_mask_toggles_data_modules_matching_the_predicate() {
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        matrix.set_mask(MaskSetting::Exact(0)).unwrap(); // (row + col) % 2 == 0
        // The first two data modules in placement order (see `data_order_starts_upwards...`).
        assert!(matrix.is_dark(20, 20), "20 + 20 is even");
        assert!(!matrix.is_dark(20, 19), "20 + 19 is odd");
    }

    #[test]
    fn set_mask_leaves_function_modules_alone() {
        // Every function module outside the format information band (row 8 / column 8, which
        // legitimately changes with the mask — see `set_mask_writes_format_information_at_the_
        // spec_positions`): finders, separators, timing and alignment patterns.
        let mut matrix = QRMatrix::new(7, ErrorCorrectionLevel::M);
        let is_outside_format_band = |row: usize, col: usize| row != 8 && col != 8;
        let before: Vec<bool> = (0..matrix.size())
            .flat_map(|row| (0..matrix.size()).map(move |col| (row, col)))
            .filter(|&(row, col)| matrix.is_function(row, col) && is_outside_format_band(row, col))
            .map(|(row, col)| matrix.is_dark(row, col))
            .collect();
        matrix.set_mask(MaskSetting::Exact(5)).unwrap();
        let after: Vec<bool> = (0..matrix.size())
            .flat_map(|row| (0..matrix.size()).map(move |col| (row, col)))
            .filter(|&(row, col)| matrix.is_function(row, col) && is_outside_format_band(row, col))
            .map(|(row, col)| matrix.is_dark(row, col))
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn set_mask_undoes_the_previous_mask_before_applying_a_new_one() {
        let bits = bitvec![u8, Lsb0; 1, 0, 1, 1, 0, 0, 1, 0, 1, 1];

        let mut switched = QRMatrix::new(1, ErrorCorrectionLevel::M);
        switched.place_data(&bits);
        switched.set_mask(MaskSetting::Exact(3)).unwrap();
        switched.set_mask(MaskSetting::Exact(5)).unwrap();

        let mut direct = QRMatrix::new(1, ErrorCorrectionLevel::M);
        direct.place_data(&bits);
        direct.set_mask(MaskSetting::Exact(5)).unwrap();

        assert_eq!(switched.modules, direct.modules);
    }

    #[test]
    fn mask_returns_the_currently_set_pattern() {
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        assert_eq!(matrix.mask(), None);
        matrix.set_mask(MaskSetting::Exact(2)).unwrap();
        assert_eq!(matrix.mask(), Some(2));
    }

    #[test]
    fn with_mask_returns_the_masked_bits_without_changing_the_matrix() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        let size = matrix.size();
        let modules = matrix.with_mask(0).unwrap();
        assert_eq!(modules.len(), size * size);
        assert!(modules[20 * size + 20]);
        assert_eq!(matrix.mask(), None, "with_mask must not set the matrix's own mask");
        assert!(!matrix.is_dark(20, 20), "with_mask must not toggle the matrix's own modules");
    }

    #[test]
    fn with_mask_matches_set_mask_on_a_clone() {
        let bits = bitvec![u8, Lsb0; 1, 0, 1, 1, 0, 0, 1, 0, 1, 1];
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        matrix.place_data(&bits);

        let via_with_mask = matrix.with_mask(4).unwrap();

        let mut via_set_mask = matrix.clone();
        via_set_mask.set_mask(MaskSetting::Exact(4)).unwrap();

        assert_eq!(via_with_mask, via_set_mask.modules);
    }

    #[test]
    fn set_mask_rejects_patterns_above_7() {
        let mut matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        assert_eq!(matrix.set_mask(MaskSetting::Exact(8)), Err(MaskError::InvalidMaskVersion(8)));
        assert_eq!(matrix.mask(), None, "a rejected pattern must not become the matrix's mask");
    }

    #[test]
    fn with_mask_rejects_patterns_above_7() {
        let matrix = QRMatrix::new(1, ErrorCorrectionLevel::M);
        assert_eq!(matrix.with_mask(9), Err(MaskError::InvalidMaskVersion(9)));
    }
}
