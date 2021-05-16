use criterion::{black_box, criterion_group, criterion_main, Criterion};
use performance_timing::CriterionCycleCounter;
use performance_timing::{const_cycle_loop, cycle_accurate_config};

pub fn criterion_benchmark(c: &mut Criterion<CriterionCycleCounter>) {
    c.bench_function("cycle_10K", |b| b.iter(|| const_cycle_loop(black_box(10_000))));
    c.bench_function("cycle_20K", |b| b.iter(|| const_cycle_loop(black_box(20_000))));
    c.bench_function("cycle_100K", |b| b.iter(|| const_cycle_loop(black_box(100_000))));
    c.bench_function("cycle_200K", |b| b.iter(|| const_cycle_loop(black_box(200_000))));
}

criterion_group! {
    name = benches;
    config = cycle_accurate_config();
    targets = criterion_benchmark
}

criterion_main!(benches);
