use ktlsp_parser::{MAX_ALLOCATION_BYTES, WORKER_ALLOCATION_LIMIT_EXIT_CODE};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub(crate) struct LimitedAllocator;
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static REFUSED: AtomicBool = AtomicBool::new(false);
pub(crate) fn charge(counter: &AtomicUsize, bytes: usize) -> bool {
    let mut used = counter.load(Ordering::Relaxed);
    loop {
        let Some(total) = used
            .checked_add(bytes)
            .filter(|&total| total <= MAX_ALLOCATION_BYTES)
        else {
            return false;
        };
        match counter.compare_exchange_weak(used, total, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => used = actual,
        }
    }
}
// Cumulative charging also bounds allocation churn during exponential backtracking.
// Safety: every successful allocation/deallocation is forwarded unchanged to System.
unsafe impl GlobalAlloc for LimitedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !charge(&ALLOCATED, layout.size()) {
            REFUSED.store(true, Ordering::Relaxed);
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !charge(&ALLOCATED, layout.size()) {
            REFUSED.store(true, Ordering::Relaxed);
            return std::ptr::null_mut();
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if !charge(&ALLOCATED, size) {
            REFUSED.store(true, Ordering::Relaxed);
            return std::ptr::null_mut();
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
pub(crate) fn worker_exit_code(run_succeeded: bool) -> i32 {
    if REFUSED.load(Ordering::Relaxed) {
        WORKER_ALLOCATION_LIMIT_EXIT_CODE
    } else if run_succeeded {
        0
    } else {
        1
    }
}
