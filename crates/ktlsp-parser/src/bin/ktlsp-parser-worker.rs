//! Private one-request worker. Never run ktrs inside the server process.
use ktlsp_parser::{
    Diagnostic, Element, MAX_ALLOCATION_BYTES, MAX_DIAGNOSTICS, MAX_ELEMENTS, MAX_NESTING,
    MAX_OUTPUT_BYTES, MAX_SOURCE_BYTES, Reply,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    io::{Read, Write},
    sync::atomic::{AtomicUsize, Ordering},
};

struct LimitedAllocator;
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
fn charge(counter: &AtomicUsize, bytes: usize) -> bool {
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
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if !charge(&ALLOCATED, layout.size()) {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if !charge(&ALLOCATED, size) {
            return std::ptr::null_mut();
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: LimitedAllocator = LimitedAllocator;

fn main() {
    let result = run();
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let kind = match args.next().as_deref() {
        Some("--source") => ktrs_parser::FileKind::Source,
        Some("--script") => ktrs_parser::FileKind::Script,
        _ => return Err("invalid worker mode".into()),
    };
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let mut source = String::new();
    std::io::stdin()
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_string(&mut source)?;
    if source.len() > MAX_SOURCE_BYTES || source.contains('\r') {
        return Err("invalid worker input".into());
    }
    ktlsp_parser::preflight(&source)?;
    let parse = ktrs_parser::parse_file(&source, kind);
    let tree = &parse.tree;
    let reply = if !parse.missed_tokens.is_empty() || tree.text() != source {
        Reply::Coverage
    } else if tree.len() > MAX_ELEMENTS || parse.error_messages.len() > MAX_DIAGNOSTICS {
        Reply::OutputLimit
    } else {
        let mut elements = Vec::with_capacity(tree.len());
        let mut diagnostics = Vec::new();
        let mut messages = parse.error_messages.iter();
        let mut depths = Vec::with_capacity(tree.len());
        for id in 0..tree.len() as u32 {
            let parent = tree.parent(id).map(|p| p as usize);
            let depth = parent.map_or(1, |p| depths[p] + 1);
            if depth > MAX_NESTING {
                return write_reply(&Reply::NestingLimit);
            }
            depths.push(depth);
            let range = tree.text_range(id);
            let start = u32::from(range.start()) as usize;
            let end = u32::from(range.end()) as usize;
            let kind = tree.kind(id);
            if kind == ktrs_syntax::SyntaxKind::ERROR_ELEMENT {
                let message = messages.next().ok_or("missing error message")?;
                if message.len() > 4096 {
                    return write_reply(&Reply::OutputLimit);
                }
                diagnostics.push(Diagnostic {
                    start,
                    end,
                    message: message.clone(),
                });
            }
            elements.push(Element {
                kind: format!("{kind:?}"),
                start,
                end,
                parent,
                subtree_end: tree.subtree_end(id) as usize,
                token: tree.is_token(id),
            });
        }
        if messages.next().is_some() {
            return Err("extra error message".into());
        }
        Reply::Tree {
            elements,
            diagnostics,
        }
    };
    write_reply(&reply)
}
fn write_reply(reply: &Reply) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(reply)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        serde_json::to_writer(std::io::stdout().lock(), &Reply::OutputLimit)?;
    } else {
        std::io::stdout().lock().write_all(&bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_accounting_is_checked_and_cumulative() {
        let counter = AtomicUsize::new(0);
        assert!(charge(&counter, MAX_ALLOCATION_BYTES - 1));
        assert!(!charge(&counter, usize::MAX));
        assert!(charge(&counter, 1));
        assert!(!charge(&counter, 1));
        assert_eq!(counter.load(Ordering::Relaxed), MAX_ALLOCATION_BYTES);
    }
    #[test]
    fn allocation_denial_aborts_only_child() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::allocation_denial_child", "--ignored"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success());
        let value = vec![0u8; 1024];
        assert_eq!(value.len(), 1024);
    }
    #[test]
    #[ignore = "intentionally aborts; launched by allocation_denial_aborts_only_child"]
    fn allocation_denial_child() {
        std::hint::black_box(vec![1u8; MAX_ALLOCATION_BYTES + 1]);
    }
}
