use super::measure::Stats;

pub fn perf_benchmark<F: Fn() -> O, O>(id: &str, f: F) -> Stats {
    let stats = super::measure::bench(f);
    log!("\nperformance stats for `{id}`:\n{stats}\n");
    stats
}

#[macro_export]
macro_rules! perf_bench {
    ($test:ident, $thresh:expr) => {
        crate::threshold::check_threshold_with_args(
            || crate::perf::benchmark::perf_benchmark(stringify!($test), $test),
            "perf_bench",
            stringify!($test),
            $thresh,
        )
    };
}

#[macro_export]
macro_rules! perf_bench_cmp_with_toml {
    ($test:ident $(,)?) => {
        crate::perf::benchmark::perf_benchmark(stringify!($test), $test)
    };
    ($test:ident, $toml:expr, $limits:expr, $toml_log:expr $(,)?) => {
        let value = crate::perf::measure::trace_perf($test);
        crate::threshold::check_threshold_with_str(
            || crate::perf::benchmark::perf_benchmark(stringify!($test), $test),
            $toml,
            $limits,
        )
    };
}
