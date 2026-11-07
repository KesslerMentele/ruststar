use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ruststar::services::qr_code::{segmenter::segment, ErrorCorrectionLevel, QRVersion};

// Input lengths in chars, from a short URL up to near version 40-L's limit for Kanji (the
// tightest-packing mode per char that still fits at 1500).
const LENGTHS: [usize; 4] = [16, 128, 512, 1500];

// Each pattern is cycled out to the requested length. "mixed" switches between Byte,
// Alphanumeric, Numeric and Kanji runs, so it exercises the switch transitions the single-mode
// inputs never take.
const INPUTS: [(&str, &str); 5] = [
    ("numeric", "0123456789"),
    ("alphanumeric", "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:"),
    ("byte", "the quick brown fox jumps over the lazy dog"),
    ("kanji", "漢字日本語点"),
    ("mixed", "user42/ORDER-2026-000183920471/日本語?q="),
];

fn input(pattern: &str, length: usize) -> String {
    pattern.chars().cycle().take(length).collect()
}

// Benches one DP pass (a fixed version-40 symbol) against Auto (up to one pass per version
// group, plus the capacity lookups) as two series of the same group, so criterion plots them side
// by side and the gap is Auto's overhead.
fn bench_segment(c: &mut Criterion) {
    let version_40 = QRVersion::Exact(40);

    for (name, pattern) in INPUTS {
        let mut group = c.benchmark_group(format!("segment/{name}"));
        for length in LENGTHS {
            let data = input(pattern, length);
            // A bench input that doesn't fit would only measure the error path.
            segment(&data, QRVersion::Auto, ErrorCorrectionLevel::L)
                .expect("bench inputs should fit version 40-L");

            group.bench_with_input(BenchmarkId::new("fixed_v40", length), &data, |b, data| {
                b.iter(|| {
                    black_box(segment(
                        black_box(data),
                        version_40,
                        ErrorCorrectionLevel::L,
                    ))
                });
            });
            group.bench_with_input(BenchmarkId::new("auto", length), &data, |b, data| {
                b.iter(|| {
                    black_box(segment(black_box(data), QRVersion::Auto, ErrorCorrectionLevel::L))
                });
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_segment);
criterion_main!(benches);
