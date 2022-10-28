use super::measure::MemoryStats;

pub fn alloc_benchmark<F: FnOnce() -> O, O>(id: &str, f: F) -> MemoryStats {
    let (_, stats) = crate::alloc::measure::trace_allocs(f);
    log!("\nmemory allocation stats for `{id}`:\n{stats}");
    stats
}

#[macro_export]
macro_rules! alloc_bench {
    ($test:ident, $thresh:expr) => {
        $crate::threshold::check_threshold_with_args(
            || $crate::alloc::benchmark::alloc_benchmark(stringify!($test), $test),
            "alloc_bench",
            stringify!($test),
            $thresh,
        )
    };
}

#[macro_export]
macro_rules! alloc_bench_cmp_with_toml {
    ($test:ident $(,)?) => {
        $crate::alloc::benchmark::alloc_benchmark(stringify!($test), $test)
    };
    ($test:ident, $toml:expr, $limits:expr $(,)?) => {{
        $crate::threshold::check_threshold_with_str(
            || $crate::alloc::benchmark::alloc_benchmark(stringify!($test), $test),
            $toml,
            $limits,
        )
    }};
}
