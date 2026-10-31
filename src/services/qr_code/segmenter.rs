// Segments are bitstrings, concatenated without any byte-alignment.

// PEDAC
// Problem:
//  We want to create the most efficient packing for a given sequence of data. We have a variety of
//  encoding modes available to us, each with its own set of characters it can encode. Every time we
//  swap modes, we incur a penalty, having to insert a header to identify which encoder to switch to.
//
//  The header's size (4-bit mode indicator + character count indicator) depends on the version
//  group (1-9, 10-26, 27-40), so the optimal segmentation is computed per version group.

// Examples:
// https://www.imperialviolet.org/2021/08/26/qrencoding.html
// https://github.com/Shane32/QRCoder/pull/676
// https://observablehq.com/@zavierhenry/encoding-qr-codes
// https://www.barcodefaq.com/2d/qr-code/qr-code-auto-encoding-mode/

// Data Structures / Algorithm
// Shortest-path over (position, mode):
//  - All costs are measured in sixths of a bit, so that grouped modes have integer per-char costs
//    (numeric 10 bits / 3 chars = 20, alphanumeric 11 bits / 2 chars = 33)
//  - table[i][m] is the cheapest cost of encoding the first i chars, given the segment covering
//    char i - 1 is in mode m (still open, so not yet rounded).
//  - For each char, each mode m can either continue its own open segment, or close the cheapest
//    segment of any mode (rounded up) and open a new one in m by paying header(m).
//  - Each cell remembers which mode it came from, so the segments are recovered by walking back
//    from the cheapest final cell.
//
// Version selection:
//  - Fixed: segment with that version's group, then check the result fits its capacity.
//  - Auto: segment once per version group (smallest first), and take the first version in that
//    group whose capacity holds the result.

// Code

use std::ops::Range;

use super::capacity;
use super::encoder::{alphanumeric_value, shift_jis_kanji};
use super::{ErrorCorrectionLevel, QRVersion};

/// How the caller wants the version chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionChoice {
    /// Use exactly this version, failing if the data doesn't fit.
    Fixed(QRVersion),
    /// Use the smallest version that fits the optimally segmented data.
    Auto,
}

/// The data modes that take part in segmentation. ECI, FNC1 and Structured Append are prefixes,
/// not alternatives for encoding a run of characters, so they're added by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    // ECI
}

impl SegmentMode {
    const ALL: [SegmentMode; 4] = [
        SegmentMode::Numeric,
        SegmentMode::Alphanumeric,
        SegmentMode::Byte,
        SegmentMode::Kanji,
        // SegmentMode::ECI,
    ];

    /// This mode's slot in a DP row.
    fn index(self) -> usize {
        self as usize
    }

    /// The cost of encoding `c` in this mode, in sixths of a bit, or `None` if this mode can't
    /// encode `c`.
    fn char_cost(self, c: char) -> Option<usize> {
        match self {
            SegmentMode::Numeric => c.is_ascii_digit().then_some(20),
            SegmentMode::Alphanumeric => alphanumeric_value(c).map(|_| 33),
            SegmentMode::Byte => Some(c.len_utf8() * 8 * 6),
            SegmentMode::Kanji => shift_jis_kanji(c).ok().map(|_| 13 * 6),

            // SegmentMode::ECI =>
        }
    }
}

/// Character count indicator widths change at versions 10 and 27; within a group, every version
/// shares the same header costs and so the same optimal segmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VersionGroup {
    Small,
    Medium,
    Large,
}

impl VersionGroup {
    const ALL: [VersionGroup; 3] = [VersionGroup::Small, VersionGroup::Medium, VersionGroup::Large];

    fn of(version: QRVersion) -> Self {
        match version.version {
            1..=9 => VersionGroup::Small,
            10..=26 => VersionGroup::Medium,
            _ => VersionGroup::Large,
        }
    }

    fn versions(self) -> impl Iterator<Item = QRVersion> {
        let range = match self {
            VersionGroup::Small => 1..=9,
            VersionGroup::Medium => 10..=26,
            VersionGroup::Large => 27..=40,
        };
        range.map(|version| QRVersion { version })
    }

    /// Width of the character count indicator for `mode` in this group, in bits.
    fn char_count_bits(self, mode: SegmentMode) -> usize {
        match (mode, self) {
            (SegmentMode::Numeric, VersionGroup::Small) => 10,
            (SegmentMode::Numeric, VersionGroup::Medium) => 12,
            (SegmentMode::Numeric, VersionGroup::Large) => 14,
            (SegmentMode::Alphanumeric, VersionGroup::Small) => 9,
            (SegmentMode::Alphanumeric, VersionGroup::Medium) => 11,
            (SegmentMode::Alphanumeric, VersionGroup::Large) => 13,
            (SegmentMode::Byte, VersionGroup::Small) => 8,
            (SegmentMode::Byte, _) => 16,
            (SegmentMode::Kanji, VersionGroup::Small) => 8,
            (SegmentMode::Kanji, VersionGroup::Medium) => 10,
            (SegmentMode::Kanji, VersionGroup::Large) => 12,
        }
    }

    /// The cost of opening a segment in `mode` (mode indicator + character count indicator), in
    /// sixths of a bit.
    fn header_cost(self, mode: SegmentMode) -> usize {
        (4 + self.char_count_bits(mode)) * 6
    }
}

/// One run of `data` encoded in a single mode. `range` is in bytes, always on char boundaries,
/// so `&data[range]` is the segment's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub(crate) mode: SegmentMode,
    pub(crate) range: Range<usize>,
}

/// The result of segmentation: the version the data fits in, and how to split it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPlan {
    pub(crate) version: QRVersion,
    pub(crate) segments: Vec<Segment>,
    /// Total length of every segment's header and data, in bits.
    pub(crate) data_bits: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentError {
    /// The data needs more bits than the requested version holds at this error correction level.
    ExceedsVersionCapacity { needed_bits: usize, capacity_bits: usize },
    /// The data needs more bits than even version 40 holds at this error correction level.
    TooLong { needed_bits: usize },
}

/// A DP cell: the cheapest cost to reach it, and the mode of the cell it was reached from.
/// `from` equal to this cell's own mode means the segment continued; anything else means a new
/// segment was opened here.
#[derive(Debug, Clone, Copy)]
struct Step {
    cost: usize,
    from: SegmentMode,
}

/// Rounds a cost in sixths of a bit up to a whole bit (still in sixths).
fn round_up_to_bit(cost: usize) -> usize {
    cost.div_ceil(6) * 6
}

/// Splits `data` into segments, choosing the version per `choice`.
pub fn segment(
    data: &str,
    choice: VersionChoice,
    error_correction_level: ErrorCorrectionLevel,
) -> Result<SegmentPlan, SegmentError> {
    // A segment's character count can't overflow its count indicator here: in every group, the
    // longest count the indicator holds needs more bits than the group's largest version has, so
    // any plan that passes the capacity check is representable.
    match choice {
        VersionChoice::Fixed(version) => {
            let (segments, needed_bits) = optimal_segments(data, VersionGroup::of(version));
            let capacity_bits = data_capacity_bits(version, error_correction_level);
            if needed_bits > capacity_bits {
                return Err(SegmentError::ExceedsVersionCapacity { needed_bits, capacity_bits });
            }
            Ok(SegmentPlan { version, segments, data_bits: needed_bits })
        }
        VersionChoice::Auto => {
            let mut needed_bits = 0;
            for group in VersionGroup::ALL {
                let (segments, bits) = optimal_segments(data, group);
                needed_bits = bits;
                if let Some(version) = group
                    .versions()
                    .find(|&version| bits <= data_capacity_bits(version, error_correction_level))
                {
                    return Ok(SegmentPlan { version, segments, data_bits: bits });
                }
            }
            Err(SegmentError::TooLong { needed_bits })
        }
    }
}

/// The optimal segmentation of `data` for a version group, and its total length in bits.
fn optimal_segments(data: &str, group: VersionGroup) -> (Vec<Segment>, usize) {
    // Nothing to encode, so no segment and no header to pay for.
    if data.is_empty() {
        return (Vec::new(), 0);
    }

    // Byte offset of every char, plus `data.len()`, so that DP position i maps to a byte range.
    let boundaries: Vec<usize> = data
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(data.len()))
        .collect();

    let table = fill_table(data, group);
    let (end_mode, cost) = cheapest_rounded(table.last().expect("the table always has row 0"));
    (backtrack(&table, &boundaries, end_mode), cost / 6)
}

/// Fills the DP table: `table.len() == char count + 1`, and `table[i][m.index()]` is `None` when
/// mode `m` can't encode the char at position i - 1.
fn fill_table(data: &str, group: VersionGroup) -> Vec<[Option<Step>; 4]> {
    let mut table = Vec::with_capacity(data.len() + 1);

    // Row 0: before any chars, every mode is "open" having paid only its header. `from` is
    // irrelevant here, since `backtrack` stops at row 0.
    table.push(SegmentMode::ALL.map(|mode| Some(Step { cost: group.header_cost(mode), from: mode })));

    for c in data.chars() {
        let previous = table.last().expect("the table always has row 0");
        // Switching from the cheapest closed segment is the cheapest way to open any new one.
        let (switch_from, switch_cost) = cheapest_rounded(previous);

        let row = SegmentMode::ALL.map(|mode| {
            let char_cost = mode.char_cost(c)?;
            let switching = Step {
                cost: switch_cost + group.header_cost(mode) + char_cost,
                from: switch_from,
            };
            let continuing = previous[mode.index()].map(|step| Step {
                cost: step.cost + char_cost,
                from: mode,
            });
            // Ties go to continuing, which keeps the segment count down.
            Some(match continuing {
                Some(continuing) if continuing.cost <= switching.cost => continuing,
                _ => switching,
            })
        });
        table.push(row);
    }
    table
}

/// The mode whose open segment in `row` is cheapest to close, and that cost rounded up to a whole
/// bit. Ties go to the earlier mode in `SegmentMode::ALL`.
fn cheapest_rounded(row: &[Option<Step>; 4]) -> (SegmentMode, usize) {
    SegmentMode::ALL
        .into_iter()
        .filter_map(|mode| row[mode.index()].map(|step| (mode, round_up_to_bit(step.cost))))
        .min_by_key(|&(_, cost)| cost)
        .expect("Byte mode encodes every char, so every row has a reachable cell")
}

/// Walks back from `end_mode` in the final row to recover the segments, front to back.
fn backtrack(
    table: &[[Option<Step>; 4]],
    boundaries: &[usize],
    end_mode: SegmentMode,
) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut mode = end_mode;

    for i in (1..table.len()).rev() {
        let char_range = boundaries[i - 1]..boundaries[i];
        // Walking backwards, so a char in the same mode extends the current segment's start.
        // Switching never lands on the mode it left (continuing is always cheaper), so equal
        // modes always mean the same segment.
        match segments.last_mut() {
            Some(segment) if segment.mode == mode => segment.range.start = char_range.start,
            _ => segments.push(Segment { mode, range: char_range }),
        }
        mode = table[i][mode.index()]
            .expect("backtracking only follows reachable cells")
            .from;
    }

    segments.reverse();
    segments
}

/// How many data bits `version` holds at `error_correction_level`, i.e. its data codewords * 8.
fn data_capacity_bits(version: QRVersion, error_correction_level: ErrorCorrectionLevel) -> usize {
    capacity::data_codewords(version, error_correction_level) * 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_groups_split_at_10_and_27() {
        assert_eq!(VersionGroup::of(QRVersion { version: 9 }), VersionGroup::Small);
        assert_eq!(VersionGroup::of(QRVersion { version: 10 }), VersionGroup::Medium);
        assert_eq!(VersionGroup::of(QRVersion { version: 26 }), VersionGroup::Medium);
        assert_eq!(VersionGroup::of(QRVersion { version: 27 }), VersionGroup::Large);
    }

    #[test]
    fn version_groups_cover_every_version_once() {
        let versions: Vec<u8> = VersionGroup::ALL
            .iter()
            .flat_map(|group| group.versions())
            .map(|v| v.version)
            .collect();
        assert_eq!(versions, (1..=40).collect::<Vec<_>>());
    }

    #[test]
    fn rounding_reproduces_short_trailing_groups() {
        // 1 digit = 4 bits, 2 digits = 7 bits, 3 digits = 10 bits.
        assert_eq!(round_up_to_bit(20) / 6, 4);
        assert_eq!(round_up_to_bit(40) / 6, 7);
        assert_eq!(round_up_to_bit(60) / 6, 10);
        // 1 alphanumeric char = 6 bits, 2 = 11 bits.
        assert_eq!(round_up_to_bit(33) / 6, 6);
        assert_eq!(round_up_to_bit(66) / 6, 11);
    }

    #[test]
    fn byte_mode_costs_every_utf8_byte() {
        assert_eq!(SegmentMode::Byte.char_cost('a'), Some(8 * 6));
        assert_eq!(SegmentMode::Byte.char_cost('é'), Some(16 * 6));
    }

    #[test]
    fn kanji_mode_only_takes_double_byte_shift_jis() {
        assert_eq!(SegmentMode::Kanji.char_cost('点'), Some(13 * 6));
        assert_eq!(SegmentMode::Kanji.char_cost('a'), None);
    }

    fn seg(mode: SegmentMode, range: Range<usize>) -> Segment {
        Segment { mode, range }
    }

    #[test]
    fn worked_example_switches_from_byte_to_numeric() {
        // The worked example in docs/qr_segmenter_dp.md.
        assert_eq!(
            optimal_segments("a1234567", VersionGroup::Small),
            (vec![seg(SegmentMode::Byte, 0..1), seg(SegmentMode::Numeric, 1..8)], 58)
        );
    }

    #[test]
    fn short_runs_are_not_worth_a_switch() {
        // A Numeric segment for "12" would cost 14 header bits to save 9 data bits.
        assert_eq!(
            optimal_segments("a12", VersionGroup::Small),
            (vec![seg(SegmentMode::Byte, 0..3)], 12 + 24)
        );
    }

    #[test]
    fn alphanumeric_prefix_then_numeric() {
        assert_eq!(
            optimal_segments("AB1234567", VersionGroup::Small),
            (vec![seg(SegmentMode::Alphanumeric, 0..2), seg(SegmentMode::Numeric, 2..9)], 62)
        );
    }

    #[test]
    fn single_mode_input_is_one_segment() {
        // ISO 18004's "01234567": 4 + 10 header bits, then 10 + 10 + 7 data bits.
        assert_eq!(
            optimal_segments("01234567", VersionGroup::Small),
            (vec![seg(SegmentMode::Numeric, 0..8)], 41)
        );
    }

    #[test]
    fn kanji_run_is_one_segment() {
        assert_eq!(
            optimal_segments("点点", VersionGroup::Small),
            (vec![seg(SegmentMode::Kanji, 0..6)], 4 + 8 + 26)
        );
    }

    #[test]
    fn segment_ranges_are_byte_offsets() {
        // é is 2 UTF-8 bytes, so the Numeric run starts at byte 2.
        assert_eq!(
            optimal_segments("é1234567", VersionGroup::Small),
            (vec![seg(SegmentMode::Byte, 0..2), seg(SegmentMode::Numeric, 2..9)], 28 + 38)
        );
    }

    #[test]
    fn empty_input_has_no_segments() {
        assert_eq!(optimal_segments("", VersionGroup::Small), (vec![], 0));
    }

    #[test]
    fn larger_groups_pay_for_wider_headers() {
        // Byte's count indicator grows from 8 to 16 bits from version 10.
        assert_eq!(optimal_segments("ab", VersionGroup::Small).1, 12 + 16);
        assert_eq!(optimal_segments("ab", VersionGroup::Medium).1, 20 + 16);
    }

    #[test]
    fn auto_picks_the_smallest_fitting_version() {
        // 41 bits fits version 1-M (128 bits).
        let plan = segment("01234567", VersionChoice::Auto, ErrorCorrectionLevel::M).unwrap();
        assert_eq!(plan.version, QRVersion { version: 1 });
        assert_eq!(plan.data_bits, 41);

        // 20 bytes is 172 bits: too many for 1-L (152), enough for 2-L (272).
        let plan = segment(&"a".repeat(20), VersionChoice::Auto, ErrorCorrectionLevel::L).unwrap();
        assert_eq!(plan.version, QRVersion { version: 2 });
    }

    #[test]
    fn auto_moves_to_the_next_version_group() {
        // 300 bytes overflows 9-L (1856 bits); at 16-bit counts it's 2420 bits, past 10-L (2192)
        // but within 11-L (2592).
        let plan = segment(&"a".repeat(300), VersionChoice::Auto, ErrorCorrectionLevel::L).unwrap();
        assert_eq!(plan.version, QRVersion { version: 11 });
        assert_eq!(plan.data_bits, 2420);
    }

    #[test]
    fn auto_rejects_data_too_long_for_version_40() {
        // 4 + 16 + 3000 * 8 = 24020 bits, against 40-L's 23648.
        assert_eq!(
            segment(&"a".repeat(3000), VersionChoice::Auto, ErrorCorrectionLevel::L),
            Err(SegmentError::TooLong { needed_bits: 24020 })
        );
    }

    #[test]
    fn fixed_uses_the_requested_version() {
        let version = QRVersion { version: 5 };
        let plan = segment("01234567", VersionChoice::Fixed(version), ErrorCorrectionLevel::H).unwrap();
        assert_eq!(plan.version, version);
        assert_eq!(plan.segments, vec![seg(SegmentMode::Numeric, 0..8)]);
    }

    #[test]
    fn fixed_rejects_data_that_does_not_fit() {
        assert_eq!(
            segment(&"a".repeat(20), VersionChoice::Fixed(QRVersion { version: 1 }), ErrorCorrectionLevel::L),
            Err(SegmentError::ExceedsVersionCapacity { needed_bits: 172, capacity_bits: 152 })
        );
    }

    #[test]
    fn lowercase_is_not_alphanumeric() {
        assert_eq!(SegmentMode::Alphanumeric.char_cost('a'), None);
        assert_eq!(SegmentMode::Alphanumeric.char_cost('A'), Some(33));
    }
}
