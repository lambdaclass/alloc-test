# Memory Allocator Wrapper with Tracing Capabilities

``` rust
use tracing_allocator::{TracingAllocator, default_tracing_allocator, trace_allocs};

#[global_allocator]
static ALLOCATOR: TracingAllocator = default_tracing_allocator();

fn main() {
    let (_, stats) = trace_allocs(|| {
        let r: Vec<u8> = vec![1, 2, 3];
        r
    });
    assert_eq!(stats.peak, 3);
    assert_eq!(stats.current, 3);
}
```
