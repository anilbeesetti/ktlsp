use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

// Per-thread counters keep test-harness allocations out of this regression.
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
    static REALLOCS: Cell<usize> = const { Cell::new(0) };
}

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && TRACK.try_with(Cell::get).unwrap_or(false) {
            ALLOCS.with(|count| count.set(count.get() + 1));
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() && TRACK.try_with(Cell::get).unwrap_or(false) {
            REALLOCS.with(|count| count.set(count.get() + 1));
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn token_growth_is_geometric_without_realloc_or_source_byte_reservation() {
    use ktlsp_syntax::{MAX_SOURCE_BYTES, MAX_TOKENS, lex};
    let dense = "@ ".repeat(131072);
    for _ in 0..5 {
        ALLOCS.with(|count| count.set(0));
        REALLOCS.with(|count| count.set(0));
        TRACK.with(|track| track.set(true));
        let result = lex(&dense).unwrap();
        TRACK.with(|track| track.set(false));
        assert_eq!(result.tokens.len(), 262144);
        assert_eq!(result.tokens.capacity(), 262144);
        assert_eq!(ALLOCS.with(Cell::get), 17);
        assert_eq!(REALLOCS.with(Cell::get), 0);
    }
    for count in [0, 1, 3, 4, 5, 4095, 4096, 4097] {
        let prefix = "@ ".repeat(count);
        let source = format!("{prefix}{}", " ".repeat(MAX_SOURCE_BYTES - prefix.len()));
        let result = lex(&source).unwrap();
        assert_eq!(result.tokens.len(), (count * 2).max(1));
        assert!(result.tokens.capacity() <= result.tokens.len().max(4) * 2);
    }
    let result = lex(&"@ ".repeat(MAX_TOKENS / 2)).unwrap();
    assert_eq!(result.tokens.capacity(), MAX_TOKENS);
}
