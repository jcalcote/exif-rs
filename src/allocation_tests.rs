//! Per-thread allocator observation around actual extraction of reference fixtures.
use crate::{Extractor, MAX_OUTPUT_BYTES, Prefix, SCRATCH_BYTES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::AtomicBool;

struct ObservedAllocator;
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
fn allocation() {
    if TRACK.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|count| count.set(count.get() + 1));
    }
}
// SAFETY: observation uses constant-initialized thread-local integer cells; every
// allocation/deallocation delegates unchanged to the platform System allocator.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocation();
        // SAFETY: caller provides the GlobalAlloc layout contract unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocation();
        // SAFETY: caller provides the GlobalAlloc layout contract unchanged.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: caller provides the matching pointer/layout unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocation();
        // SAFETY: caller provides the GlobalAlloc reallocation contract unchanged.
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

struct Tracking;
impl Drop for Tracking {
    fn drop(&mut self) {
        TRACK.with(|tracking| tracking.set(false));
    }
}

#[test]
fn reference_extraction_and_iteration_allocate_no_heap() {
    let mut scratch = vec![0; SCRATCH_BYTES];
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let cancel = AtomicBool::new(false);
    let extractor = Extractor::default();
    for (name, bytes) in crate::tests::UPSTREAM_REFERENCE
        .iter()
        .map(|(name, bytes, _)| (*name, *bytes))
        .chain(crate::tests::CARRIER_INPUTS.iter().copied())
        .chain(crate::tests::STRUCTURED_INPUTS.iter().copied())
    {
        let parts = [bytes];
        let prefix = Prefix::new(&parts, bytes.len()).unwrap();
        COUNT.with(|count| count.set(0));
        TRACK.with(|tracking| tracking.set(true));
        let tracking = Tracking;
        let result = extractor
            .extract(prefix, &mut scratch, &mut output, &cancel)
            .unwrap();
        std::hint::black_box(result.fields().count());
        let descriptor = result.descriptor();
        std::hint::black_box(descriptor.fields(&output).unwrap().count());
        drop(tracking);
        assert_eq!(COUNT.with(Cell::get), 0, "{name}");
    }
}
