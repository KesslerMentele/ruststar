use bitvec::prelude::*;

pub struct MaskEvaluation {
    pub horizontal: usize,
    pub vertical: usize,
    pub squares: usize,
    pub finder_pattern: usize,
    pub ratio: usize,
    pub total: usize,
}

// Flattened to a single pass over the whole (row-major) bit buffer: `row_end` is computed from
// `i` on the fly, so a run never scans past its own row, without an outer per-row loop.
//
// `pub` (rather than private/`pub(crate)`) solely so the criterion benchmarks in benches/
// can call these sub-scores directly instead of only the combined `penalty_score` entry point.
pub fn calculate_horizontal_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
    let mut total_score = 0;
    let mut i = 0;
    while i < width * height {
        let row_end = (i / width + 1) * width;
        if i >= row_end.saturating_sub(4) {
            i = row_end;
            continue;
        }
        let mut next_i = i + 1;
        let current_val = bits[i];
        while next_i < row_end && bits[next_i] == current_val {
            let diff = next_i - i;
            if diff == 4 {
                total_score += 3;
            } else if diff > 4 {
                total_score += 1;
            }
            next_i += 1;
        }
        i = next_i;
    }
    total_score
}

// Same flattening as calculate_horizontal_score, but walked in column-major order: `k` maps to
// row-major index `idx(k)`, and `col_end` bounds a run to its own column. This only removes the
// nested-loop shape, not the strided memory access — columns are `width` elements apart in
// row-major storage regardless of how the loop is written.
pub fn calculate_vertical_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
    let mut total_score = 0;
    let idx = |k: usize| (k % height) * width + (k / height);

    let mut k = 0;
    while k < width * height {
        let col_end = (k / height + 1) * height;
        if k >= col_end.saturating_sub(4) {
            k = col_end;
            continue;
        }
        let mut next_k = k + 1;
        let current_val = bits[idx(k)];
        while next_k < col_end && bits[idx(next_k)] == current_val {
            let diff = next_k - k;
            if diff == 4 {
                total_score += 3;
            } else if diff > 4 {
                total_score += 1;
            }
            next_k += 1;
        }
        k = next_k;
    }
    total_score
}

// Flattened to a single pass over the top-left corners of every 2x2 square: restricting the
// range to `(height - 1) * width` keeps every corner's row below the last row, and skipping the
// last column keeps every corner's column before the last column, so all four flat offsets
// (`bit_idx`, `+1`, `+width`, `+width+1`) are always in bounds without a bounds-checked `get`.
pub fn calculate_squares_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
    if height < 2 || width < 2 {
        return 0;
    }

    let mut score = 0;
    for bit_idx in 0..((height - 1) * width) {
        if bit_idx % width == width - 1 {
            continue;
        }
        let top_left = bits[bit_idx];
        let top_right = bits[bit_idx + 1];
        let bot_left = bits[bit_idx + width];
        let bot_right = bits[bit_idx + width + 1];

        if top_left == top_right && top_left == bot_left && top_left == bot_right {
            score += 3;
        }
    }
    score
}

pub fn calculate_finder_pattern_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
    let pattern = bits![1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0];
    let mut score = 0;

    if width < 11 || height < 11 {
        return 0;
    }

    for row in 0..height {
        let row_start = row * width;
        let row_end = row_start + width;
        let row_bits = &bits[row_start..row_end];

        for window in row_bits.windows(11) {
            if window == pattern || window.iter().rev().eq(pattern.iter().by_vals()) {
                score += 40;
            }
        }
    }

    for x in 0..width {
        for y in 0..(height - 10) {
            let mut match_pattern = true;
            let mut match_rev_pattern = true;

            for i in 0..11 {
                let bit_val = bits[(y + i) * width + x];
                if bit_val != pattern[i] { match_pattern = false; }
                if bit_val != pattern[10 - i] { match_rev_pattern = false; }

                if !match_pattern && !match_rev_pattern {
                    break;
                }
            }

            if match_pattern || match_rev_pattern {
                score += 40;
            }
        }
    }

    score
}

pub fn calculate_ratio_score(bits: &BitSlice<u8, Lsb0>) -> usize {
    let ones = bits.count_ones();
    let percent = (ones / bits.len()) * 100;
    ((percent.div_ceil(5) * 5).abs_diff(50) / 5).min(((percent / 5) * 5).abs_diff(50) / 5)
}

pub fn penalty_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> MaskEvaluation {
    let horizontal_score = calculate_horizontal_score(bits, width, height);
    let vertical_score = calculate_vertical_score(bits, width, height);
    let squares_score = calculate_squares_score(bits, width, height);
    let finder_pattern_score = calculate_finder_pattern_score(bits, width, height);
    let ratio_score = calculate_ratio_score(bits);

    let total = horizontal_score + vertical_score + squares_score + finder_pattern_score + ratio_score;

    MaskEvaluation {
        horizontal: horizontal_score,
        vertical: vertical_score,
        squares: squares_score,
        finder_pattern: finder_pattern_score,
        ratio: ratio_score,
        total,
    }
}

/// Same result as `penalty_score`, but runs each of the five sub-scores on its own thread.
/// `thread::scope` lets every thread borrow `bits` directly — no `Arc`/`'static` needed — since
/// the scope guarantees all five finish before this function returns.
pub fn penalty_score_mt(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> MaskEvaluation {
    let (horizontal_score, vertical_score, squares_score, finder_pattern_score, ratio_score) =
        std::thread::scope(|scope| {
            let horizontal = scope.spawn(|| calculate_horizontal_score(bits, width, height));
            let vertical = scope.spawn(|| calculate_vertical_score(bits, width, height));
            let squares = scope.spawn(|| calculate_squares_score(bits, width, height));
            let finder_pattern = scope.spawn(|| calculate_finder_pattern_score(bits, width, height));
            let ratio = scope.spawn(|| calculate_ratio_score(bits));

            (
                horizontal.join().unwrap(),
                vertical.join().unwrap(),
                squares.join().unwrap(),
                finder_pattern.join().unwrap(),
                ratio.join().unwrap(),
            )
        });

    let total = horizontal_score + vertical_score + squares_score + finder_pattern_score + ratio_score;

    MaskEvaluation {
        horizontal: horizontal_score,
        vertical: vertical_score,
        squares: squares_score,
        finder_pattern: finder_pattern_score,
        ratio: ratio_score,
        total,
    }
}

// The grid-area crossover point measured by this crate's `penalty_score` benchmark: below it,
// `thread::scope`'s spawn overhead outweighs the parallel work, and above it the parallel work
// wins. This is specific to the machine it was measured on, not a universal constant — re-check
// it with `cargo bench -- penalty_score` if the sub-scores change or this runs somewhere else.
const MT_THRESHOLD_AREA: usize = 97 * 97;

/// Picks `penalty_score` or `penalty_score_mt`, whichever benchmarked faster for a grid this
/// size, instead of committing to one strategy for every grid.
pub fn penalty_score_adaptive(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> MaskEvaluation {
    if width * height >= MT_THRESHOLD_AREA {
        penalty_score_mt(bits, width, height)
    } else {
        penalty_score(bits, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[&str], width: usize) -> BitVec<u8, Lsb0> {
        let mut bits = BitVec::new();
        for row in rows {
            for c in row.chars() {
                bits.push(c == '1');
            }
        }
        assert_eq!(bits.len() % width, 0, "row strings must each be `width` chars long");
        bits
    }

    // Unflattened reference implementations, kept only for the cross-check tests below.
    fn get(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize, row: usize, col: usize) -> Option<bool> {
        if row >= height || col >= width {
            return None;
        }
        bits.get(row * width + col).as_deref().copied()
    }
    fn reference_horizontal_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
        let mut total_score = 0;
        for r in 0..height {
            let mut i = 0;
            while i < width.saturating_sub(4) {
                let mut next_i = i + 1;
                let current_val = get(bits, width, height, r, i);
                while next_i < width && get(bits, width, height, r, next_i) == current_val {
                    let diff = next_i - i;
                    if diff == 4 {
                        total_score += 3;
                    } else if diff > 4 {
                        total_score += 1;
                    }
                    next_i += 1;
                }
                i = next_i;
            }
        }
        total_score
    }
    fn reference_vertical_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
        let mut total_score = 0;
        for col in 0..width {
            let mut i = 0;
            while i < height.saturating_sub(4) {
                let mut next_i = i + 1;
                let current_val = get(bits, width, height, i, col);
                while next_i < height && get(bits, width, height, next_i, col) == current_val {
                    let diff = next_i - i;
                    if diff == 4 {
                        total_score += 3;
                    } else if diff > 4 {
                        total_score += 1;
                    }
                    next_i += 1;
                }
                i = next_i;
            }
        }
        total_score
    }
    fn reference_squares_score(bits: &BitSlice<u8, Lsb0>, width: usize, height: usize) -> usize {
        let mut score = 0;
        if height < 2 || width < 2 {
            return 0;
        }
        for row_idx in 0..(height - 1) {
            for col_idx in 0..(width - 1) {
                if let (Some(top_left), Some(top_right), Some(bot_left), Some(bot_right)) = (
                    get(bits, width, height, row_idx, col_idx),
                    get(bits, width, height, row_idx, col_idx + 1),
                    get(bits, width, height, row_idx + 1, col_idx),
                    get(bits, width, height, row_idx + 1, col_idx + 1),
                ) {
                    if top_left == top_right && top_left == bot_left && top_left == bot_right {
                        score += 3;
                    }
                }
            }
        }
        score
    }

    // A tiny xorshift so the cross-check grids are deterministic without a rand dependency.
    fn random_bits(len: usize, mut seed: u32) -> BitVec<u8, Lsb0> {
        let mut bits = BitVec::new();
        for _ in 0..len {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            bits.push(seed % 2 == 0);
        }
        bits
    }

    #[test]
    fn horizontal_score_scores_a_run_of_five_as_three() {
        let bits = grid(&["11111"], 5);
        assert_eq!(calculate_horizontal_score(&bits, 5, 1), 3);
    }

    #[test]
    fn horizontal_score_does_not_cross_a_row_boundary() {
        // Row 0 ends in two 1s, row 1 starts with three 1s: a flattened scan must not treat
        // that as a single run of five.
        let bits = grid(&["00011", "11000"], 5);
        assert_eq!(calculate_horizontal_score(&bits, 5, 2), 0);
    }

    #[test]
    fn horizontal_score_scores_each_full_row_independently() {
        let bits = grid(&["11111", "11111"], 5);
        assert_eq!(calculate_horizontal_score(&bits, 5, 2), 6);
    }

    #[test]
    fn vertical_score_scores_a_run_of_five_as_three() {
        let bits = grid(&["1", "1", "1", "1", "1"], 1);
        assert_eq!(calculate_vertical_score(&bits, 1, 5), 3);
    }

    #[test]
    fn vertical_score_does_not_cross_a_column_boundary() {
        let bits = grid(&["10", "10", "11", "01", "01"], 2);
        assert_eq!(calculate_vertical_score(&bits, 2, 5), 0);
    }

    #[test]
    fn squares_score_scores_a_single_uniform_square() {
        let bits = grid(&["11", "11"], 2);
        assert_eq!(calculate_squares_score(&bits, 2, 2), 3);
    }

    #[test]
    fn squares_score_does_not_wrap_a_square_across_a_row_boundary() {
        let bits = grid(&["01", "10"], 2);
        assert_eq!(calculate_squares_score(&bits, 2, 2), 0);
    }

    #[test]
    fn squares_score_ignores_a_grid_too_small_to_hold_a_square() {
        let bits = grid(&["1"], 1);
        assert_eq!(calculate_squares_score(&bits, 1, 1), 0);
    }

    #[test]
    fn flattened_scores_match_the_unflattened_reference_on_random_grids() {
        for (width, height, seed) in [(21, 21, 1), (25, 25, 42), (1, 40, 7), (40, 1, 7), (6, 6, 99)] {
            let bits = random_bits(width * height, seed);
            assert_eq!(
                calculate_horizontal_score(&bits, width, height),
                reference_horizontal_score(&bits, width, height),
                "horizontal mismatch for {width}x{height} seed {seed}"
            );
            assert_eq!(
                calculate_vertical_score(&bits, width, height),
                reference_vertical_score(&bits, width, height),
                "vertical mismatch for {width}x{height} seed {seed}"
            );
            assert_eq!(
                calculate_squares_score(&bits, width, height),
                reference_squares_score(&bits, width, height),
                "squares mismatch for {width}x{height} seed {seed}"
            );
        }
    }

    #[test]
    fn penalty_score_mt_matches_penalty_score() {
        for (width, height, seed) in [(21, 21, 1), (25, 25, 42), (1, 40, 7), (40, 1, 7), (6, 6, 99)] {
            let bits = random_bits(width * height, seed);
            let single = penalty_score(&bits, width, height);
            let multi = penalty_score_mt(&bits, width, height);
            assert_eq!(single.horizontal, multi.horizontal, "horizontal mismatch for {width}x{height}");
            assert_eq!(single.vertical, multi.vertical, "vertical mismatch for {width}x{height}");
            assert_eq!(single.squares, multi.squares, "squares mismatch for {width}x{height}");
            assert_eq!(single.finder_pattern, multi.finder_pattern, "finder_pattern mismatch for {width}x{height}");
            assert_eq!(single.ratio, multi.ratio, "ratio mismatch for {width}x{height}");
            assert_eq!(single.total, multi.total, "total mismatch for {width}x{height}");
        }
    }

    #[test]
    fn penalty_score_adaptive_matches_both_naive_implementations() {
        // 21 and 57 fall below MT_THRESHOLD_AREA (single-threaded path), 97 sits exactly at
        // it (multithreaded path), and 177 sits well above it (multithreaded path too).
        for (width, height, seed) in [(21, 21, 1), (57, 57, 2), (97, 97, 3), (177, 177, 4)] {
            let bits = random_bits(width * height, seed);
            let adaptive = penalty_score_adaptive(&bits, width, height);
            let single = penalty_score(&bits, width, height);
            let multi = penalty_score_mt(&bits, width, height);

            assert_eq!(adaptive.total, single.total, "adaptive vs single-threaded mismatch for {width}x{height}");
            assert_eq!(adaptive.total, multi.total, "adaptive vs multithreaded mismatch for {width}x{height}");
        }
    }
}
