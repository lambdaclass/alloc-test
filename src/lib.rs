use std::alloc::{GlobalAlloc, Layout, System};

#[derive(Debug, Default)]
pub struct TracingAllocator<H: 'static = MemoryTracingHooks, A = System>(pub A, H)
where
    A: GlobalAlloc;

pub const fn default_tracing_allocator() -> TracingAllocator<MemoryTracingHooks, System> {
    TracingAllocator(System, MemoryTracingHooks)
}

unsafe impl<H, A> GlobalAlloc for TracingAllocator<H, A>
where
    A: GlobalAlloc,
    H: AllocHooks,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        let pointer = self.0.alloc(layout);
        self.1.on_alloc(pointer, size, align);
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let size = layout.size();
        let align = layout.align();
        self.0.dealloc(pointer, layout);
        self.1.on_dealloc(pointer, size, align);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        let pointer = self.0.alloc_zeroed(layout);
        self.1.on_alloc_zeroed(pointer, size, align);
        pointer
    }

    unsafe fn realloc(&self, old_pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        let align = layout.align();
        let new_pointer = self.0.realloc(old_pointer, layout, new_size);
        self.1
            .on_realloc(old_pointer, new_pointer, old_size, new_size, align);
        new_pointer
    }
}

unsafe trait AllocHooks {
    fn on_alloc(&self, pointer: *mut u8, size: usize, align: usize);
    fn on_dealloc(&self, pointer: *mut u8, size: usize, align: usize);
    fn on_alloc_zeroed(&self, pointer: *mut u8, size: usize, align: usize);
    fn on_realloc(
        &self,
        old_pointer: *mut u8,
        new_pointer: *mut u8,
        old_size: usize,
        new_size: usize,
        align: usize,
    );
}

use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::{
    mem,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default, Clone, Display, Serialize, Deserialize)]
#[display(fmt = r#"Currently allocated (B): {current}
Maximum allocated (B): {peak}
Total amount of claimed memory (B): {total_size}
Total number of allocations: (N): {total_num}
Reallocations (N): {reallocs}
"#)]
pub struct MemoryStats {
    pub current: usize,
    pub peak: usize,
    pub total_size: usize,
    pub total_num: usize,
    pub reallocs: usize,
}

static mut TRACE_ALLOCS: AtomicBool = AtomicBool::new(false);

static mut ALLOC_STATS: MemoryStats = MemoryStats {
    current: 0,
    peak: 0,
    total_size: 0,
    total_num: 0,
    reallocs: 0,
};

/// Traces allocations performed while executing the `f`.
///
/// Beware that allocations made by nother threads will be also recorded.
///
/// ```
/// use tracing_allocator::{TracingAllocator, default_tracing_allocator, trace_allocs};
///
/// #[global_allocator]
/// static ALLOCATOR: TracingAllocator = default_tracing_allocator();
///
/// fn main() {
///     let (_, stats) = trace_allocs(|| {
///         let r: Vec<u8> = vec![1, 2, 3];
///         r
///     });
///     assert_eq!(stats.peak, 3);
///     assert_eq!(stats.current, 3);
/// }
/// ```
pub fn trace_allocs<F: FnOnce() -> O, O>(f: F) -> (O, MemoryStats) {
    unsafe {
        while TRACE_ALLOCS
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
            .is_err()
        {}
        let o = f();
        let stats = mem::replace(&mut ALLOC_STATS, Default::default());
        TRACE_ALLOCS.store(false, Ordering::Release);
        (o, stats)
    }
}

pub struct MemoryTracingHooks;

unsafe impl AllocHooks for MemoryTracingHooks {
    fn on_alloc(&self, _pointer: *mut u8, size: usize, _align: usize) {
        unsafe {
            if TRACE_ALLOCS.load(Ordering::Acquire) {
                // println!("allocating {size}");
                ALLOC_STATS.current += size;
                ALLOC_STATS.total_size += size;
                ALLOC_STATS.total_num += 1;
                if ALLOC_STATS.current > ALLOC_STATS.peak {
                    ALLOC_STATS.peak = ALLOC_STATS.current;
                }
            }
        }
    }

    fn on_dealloc(&self, _pointer: *mut u8, size: usize, _align: usize) {
        unsafe {
            if TRACE_ALLOCS.load(Ordering::Acquire) {
                ALLOC_STATS.current = ALLOC_STATS.current.saturating_sub(size);
            }
        }
    }

    fn on_alloc_zeroed(&self, pointer: *mut u8, size: usize, align: usize) {
        self.on_alloc(pointer, size, align);
    }

    fn on_realloc(
        &self,
        old_pointer: *mut u8,
        new_pointer: *mut u8,
        old_size: usize,
        new_size: usize,
        align: usize,
    ) {
        unsafe {
            if TRACE_ALLOCS.load(Ordering::Acquire) {
                // println!("reallocating {old_size} -> {new_size}");
                ALLOC_STATS.reallocs += 1;
            }
        }
        self.on_dealloc(old_pointer, old_size, align);
        self.on_alloc(new_pointer, new_size, align);
    }
}

pub mod cmp {

    use std::fmt::Display;

    use derive_builder::Builder;
    use num::{rational::Ratio, Integer};
    use thiserror::Error;

    use crate::MemoryStats;

    #[derive(Debug, Clone, Copy, Default, derive_more::Display)]
    pub enum Limit<T: Display + Integer + Clone> {
        #[default]
        None,
        #[display(fmt = "{_0}")]
        Cap(T),
        #[display(fmt = "{_0}")]
        Ratio(Ratio<T>),
    }

    impl<T> Limit<T>
    where
        T: Clone + Integer + Display,
    {
        pub fn cap(cap: T) -> Self {
            Limit::Cap(cap)
        }

        pub fn ratio(numer: T, denom: T) -> Self {
            Limit::Ratio(Ratio::new(numer, denom))
        }
    }

    #[derive(Debug, Error)]
    #[error("{value} exceeds {ref_value} by {limit}")]
    pub struct LimitError<T: Display + Integer + Clone> {
        limit: Limit<T>,
        value: T,
        ref_value: T,
    }

    impl<T> Limit<T>
    where
        T: Clone + Integer + Display,
    {
        fn check_cap(cap: &T, value: &T, ref_value: &T) -> bool {
            value.clone() <= ref_value.clone() + cap.clone()
        }

        fn check_ratio(ratio: &Ratio<T>, value: &T, ref_value: &T) -> bool {
            value.clone() <= ref_value.clone()
                || Ratio::new(value.clone() - ref_value.clone(), ref_value.clone()) <= *ratio
        }

        pub fn check(&self, value: &T, ref_value: &T) -> Result<(), LimitError<T>> {
            match self {
                Limit::Cap(cap) if !Self::check_cap(cap, value, ref_value) => Err(LimitError {
                    limit: self.clone(),
                    value: value.clone(),
                    ref_value: ref_value.clone(),
                }),
                Limit::Ratio(ratio) if !Self::check_ratio(ratio, value, ref_value) => {
                    Err(LimitError {
                        limit: self.clone(),
                        value: value.clone(),
                        ref_value: ref_value.clone(),
                    })
                }
                _ => Ok(()),
            }
        }
    }

    /// Limits for each allocation statistics parameter.
    #[derive(Debug, Builder)]
    pub struct AllocLimits {
        #[builder(default)]
        pub current: Limit<usize>,
        #[builder(default)]
        pub peak: Limit<usize>,
        #[builder(default)]
        pub total_size: Limit<usize>,
        #[builder(default)]
        pub total_num: Limit<usize>,
        #[builder(default)]
        pub reallocs: Limit<usize>,
    }

    #[derive(Debug, Error)]
    #[error("Allocation parameter `{param}`: {error}")]
    pub struct AllocLimitsError {
        error: LimitError<usize>,
        param: &'static str,
    }

    macro_rules! check {
        ($f:ident, $l:expr, $v:expr, $r:expr) => {
            $l.$f.check(&$v.$f, &$r.$f).map_err(|e| AllocLimitsError {
                error: e,
                param: stringify!($f),
            })
        };
    }

    impl AllocLimits {
        pub fn check(
            &self,
            value: &MemoryStats,
            ref_value: &MemoryStats,
        ) -> Result<(), AllocLimitsError> {
            check!(current, self, value, ref_value)?;
            check!(peak, self, value, ref_value)?;
            check!(total_size, self, value, ref_value)?;
            check!(total_num, self, value, ref_value)?;
            check!(reallocs, self, value, ref_value)?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn limit_cap() {
            let l = Limit::cap(10_u32);
            let r = 100_u32;
            assert!(l.check(&0, &r).is_ok());
            assert!(l.check(&100, &r).is_ok());
            assert!(l.check(&110, &r).is_ok());
            assert!(l.check(&111, &r).is_err());

            println!("{}", l.check(&111, &r).unwrap_err());
        }

        #[test]
        fn limit_ratio() {
            let l = Limit::ratio(1, 10);
            let r = 100_u32;
            assert!(l.check(&0, &r).is_ok());
            assert!(l.check(&100, &r).is_ok());
            assert!(l.check(&110, &r).is_ok());
            assert!(l.check(&111, &r).is_err());

            println!("{}", l.check(&111, &r).unwrap_err());
        }

        #[test]
        fn limits() {
            let rs = MemoryStats {
                current: 100,
                peak: 1000,
                total_size: 2000,
                total_num: 100,
                reallocs: 0,
            };
            let vs = MemoryStats {
                current: 110,
                peak: 1100,
                total_size: 2200,
                total_num: 110,
                reallocs: 1,
            };

            let ls = AllocLimitsBuilder::default().build().unwrap();
            assert!(ls.check(&rs, &rs).is_ok());
            assert!(ls.check(&vs, &rs).is_ok());

            let ls = AllocLimitsBuilder::default()
                .current(Limit::Cap(1))
                .build()
                .unwrap();
            let r = ls.check(&vs, &rs);
            assert!(r.unwrap_err().param == "current");

            let ls = AllocLimitsBuilder::default()
                .reallocs(Limit::Cap(0))
                .build()
                .unwrap();
            let r = ls.check(&vs, &rs);
            assert!(r.unwrap_err().param == "reallocs");
        }
    }
}

#[cfg(feature = "benchmark")]
mod benchmark;

#[cfg(feature = "benchmark")]
pub use benchmark::*;
