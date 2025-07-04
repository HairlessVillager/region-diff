use std::{fs, hint::black_box, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};

use region_diff::diff::chunk::RegionChunkDiff;
use region_diff::{
    config::{Config, init_config},
    diff::{Diff, file::MCADiff},
};

fn criterion_benchmark(c: &mut Criterion) {
    init_config(Config {
        log_config: region_diff::config::LogConfig::NoLog,
        threads: 16,
    });
    let old =
        fs::read("resources/test-payload/region/mca/hairlessvillager-0/20250511.mca").unwrap();
    let new =
        fs::read("resources/test-payload/region/mca/hairlessvillager-0/20250512.mca").unwrap();

    c.bench_function("mca_diff", |b| {
        b.iter(|| {
            black_box::<MCADiff<RegionChunkDiff>>(MCADiff::from_compare(
                black_box(&old),
                black_box(&new),
            ));
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(60))
        .sample_size(30)
        .warm_up_time(Duration::from_secs(20))
        .noise_threshold(0.1);
    targets = criterion_benchmark
}
criterion_main!(benches);
