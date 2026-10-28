use std::hint::black_box;

use bitvec::prelude::*;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::prelude::*;
use ruststar::services::qr_code::penalty_score::{
    calculate_finder_pattern_score, calculate_horizontal_score, calculate_ratio_score,
    calculate_squares_score, calculate_vertical_score, penalty_score, penalty_score_adaptive,
    penalty_score_mt,
};

// QR module grid sizes (size = 21 + 4 * (version - 1)) for versions 1, 10, 20, 30, 40 —
// spans the whole range from the smallest symbol up to the largest.
const SIZES: [usize; 5] = [21, 57, 97, 137, 177];

fn random_grid(size: usize) -> BitVec<u8, Lsb0> {
    let mut rng = rand::rng();
    let mut bits = BitVec::with_capacity(size * size);
    for _ in 0..(size * size) {
        bits.push(rng.random_bool(0.5));
    }
    bits
}

fn bench_horizontal(c: &mut Criterion) {
    let mut group = c.benchmark_group("calculate_horizontal_score");
    for size in SIZES {
        let grid = random_grid(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| black_box(calculate_horizontal_score(black_box(&grid), size, size)));
        });
    }
    group.finish();
}

fn bench_vertical(c: &mut Criterion) {
    let mut group = c.benchmark_group("calculate_vertical_score");
    for size in SIZES {
        let grid = random_grid(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| black_box(calculate_vertical_score(black_box(&grid), size, size)));
        });
    }
    group.finish();
}

fn bench_squares(c: &mut Criterion) {
    let mut group = c.benchmark_group("calculate_squares_score");
    for size in SIZES {
        let grid = random_grid(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| black_box(calculate_squares_score(black_box(&grid), size, size)));
        });
    }
    group.finish();
}

fn bench_finder_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("calculate_finder_pattern_score");
    for size in SIZES {
        let grid = random_grid(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| black_box(calculate_finder_pattern_score(black_box(&grid), size, size)));
        });
    }
    group.finish();
}

fn bench_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("calculate_ratio_score");
    for size in SIZES {
        let grid = random_grid(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| black_box(calculate_ratio_score(black_box(&grid))));
        });
    }
    group.finish();
}

// Benches the whole evaluation suite (all five sub-scores) in one call, the way a real mask
// evaluation loop actually uses it. Named as a "single_threaded" series within the "penalty_score"
// group so a future multithreaded `penalty_score` can be added as a "multi_threaded" series in
// the same group — criterion then plots the two side by side at every size instead of needing a
// separate report to compare them.
fn bench_full_suite(c: &mut Criterion) {
    let mut group = c.benchmark_group("penalty_score");
    for size in SIZES {
        let grid = random_grid(size);
        group.bench_with_input(BenchmarkId::new("single_threaded", size), &size, |b, &size| {
            b.iter(|| black_box(penalty_score(black_box(&grid), size, size)));
        });
        group.bench_with_input(BenchmarkId::new("multi_threaded", size), &size, |b, &size| {
            b.iter(|| black_box(penalty_score_mt(black_box(&grid), size, size)));
        });
        group.bench_with_input(BenchmarkId::new("adaptive", size), &size, |b, &size| {
            b.iter(|| black_box(penalty_score_adaptive(black_box(&grid), size, size)));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_horizontal,
    bench_vertical,
    bench_squares,
    bench_finder_pattern,
    bench_ratio,
    bench_full_suite
);
criterion_main!(benches);
