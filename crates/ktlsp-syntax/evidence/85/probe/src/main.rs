use std::{hint::black_box, time::Instant};
#[cfg(feature = "counting")]
mod counts {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicUsize, Ordering::Relaxed},
    };
    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);
    pub static TOTAL: AtomicUsize = AtomicUsize::new(0);
    pub static CALLS: AtomicUsize = AtomicUsize::new(0);
    pub struct Counting;
    fn add(n: usize) {
        let live = LIVE.fetch_add(n, Relaxed) + n;
        PEAK.fetch_max(live, Relaxed);
        TOTAL.fetch_add(n, Relaxed);
        CALLS.fetch_add(1, Relaxed);
    }
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(l) };
            if !p.is_null() {
                add(l.size());
            }
            p
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(l) };
            if !p.is_null() {
                add(l.size());
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) };
            LIVE.fetch_sub(l.size(), Relaxed);
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let q = unsafe { System.realloc(p, l, n) };
            if !q.is_null() {
                LIVE.fetch_sub(l.size(), Relaxed);
                add(n);
            }
            q
        }
    }
    #[global_allocator]
    static ALLOC: Counting = Counting;
}
fn work(source: &str, mode: &str, count: usize) -> ktlsp_syntax::Lexed {
    if mode == "lex" {
        return ktlsp_syntax::lex(black_box(source)).unwrap();
    }
    let mut tokens = if mode == "exact" {
        Vec::with_capacity(count)
    } else {
        Vec::new()
    };
    for i in 0..count {
        if mode == "veccopy" && tokens.len() == tokens.capacity() {
            let cap = (tokens.capacity().max(2) * 2).min(ktlsp_syntax::MAX_TOKENS);
            let mut grown = Vec::with_capacity(cap);
            grown.append(&mut tokens);
            tokens = grown;
        }
        if mode == "vec4" && tokens.len() == tokens.capacity() {
            let cap = (tokens.capacity().max(1) * 4).min(ktlsp_syntax::MAX_TOKENS);
            tokens.reserve_exact(cap - tokens.len());
        }
        tokens.push(ktlsp_syntax::Token {
            kind: ktlsp_syntax::TokenKind::Whitespace,
            start: i,
            end: i + 1,
        });
    }
    ktlsp_syntax::Lexed {
        tokens,
        diagnostics: Vec::new(),
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let name = &args[1];
    let passes: usize = args[2].parse().unwrap();
    let mode = &args[3];
    let (source, count) = match name.as_str() {
        "large" => ("val 名 = \"😀 café\" // comment\r\n".repeat(32768), 327680),
        "short_tokens" => ("@ ".repeat(131072), 262144),
        "empty" => (String::new(), 0),
        "short" => ("val x = 1".into(), 7),
        "whitespace" => (" ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES), 1),
        "comment" => (
            format!("/*{}*/", " ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES - 4)),
            1,
        ),
        "string" => (
            format!("\"{}\"", " ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES - 2)),
            1,
        ),
        "dense_sparse" => (
            format!(
                "{}{}",
                "@ ".repeat(2048),
                " ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES - 4096)
            ),
            4096,
        ),
        "sparse_dense" => (
            format!(
                "{}{}",
                " ".repeat(ktlsp_syntax::MAX_SOURCE_BYTES - 262144),
                "@ ".repeat(131072)
            ),
            262145,
        ),
        _ => panic!("unknown corpus"),
    };
    #[cfg(feature = "counting")]
    let baseline = {
        use counts::*;
        use std::sync::atomic::Ordering::Relaxed;
        let baseline = LIVE.load(Relaxed);
        PEAK.store(baseline, Relaxed);
        TOTAL.store(0, Relaxed);
        CALLS.store(0, Relaxed);
        baseline
    };
    let first = work(&source, mode, count);
    let tokens = first.tokens.len();
    let capacity = first.tokens.capacity();
    assert_eq!(tokens, count);
    if mode == "lex" {
        let mut end = 0;
        for token in &first.tokens {
            assert_eq!(token.start, end);
            assert!(token.end > end);
            end = token.end;
        }
        assert_eq!(end, source.len());
    }
    drop(first);
    #[cfg(feature = "counting")]
    assert_eq!(
        counts::LIVE.load(std::sync::atomic::Ordering::Relaxed),
        baseline
    );
    let start = Instant::now();
    for _ in 0..passes {
        black_box(work(&source, mode, count));
        #[cfg(feature = "counting")]
        assert_eq!(
            counts::LIVE.load(std::sync::atomic::Ordering::Relaxed),
            baseline
        );
    }
    let nanos = start.elapsed().as_nanos();
    #[cfg(feature = "counting")]
    {
        use counts::*;
        use std::sync::atomic::Ordering::Relaxed;
        let (end, peak, churn, calls) = (
            LIVE.load(Relaxed),
            PEAK.load(Relaxed),
            TOTAL.load(Relaxed),
            CALLS.load(Relaxed),
        );
        let ps = std::process::Command::new("/bin/ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        assert!(ps.status.success());
        let rss: usize = std::str::from_utf8(&ps.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        println!(
            "{{\"corpus\":\"{name}\",\"mode\":\"{mode}\",\"total_calls\":{},\"bytes\":{},\"tokens\":{tokens},\"capacity\":{capacity},\"baseline_live\":{baseline},\"end_live\":{end},\"peak_live\":{peak},\"churn\":{churn},\"allocation_calls\":{calls},\"post_drop_ps_rss_kib\":{rss}}}",
            passes + 1,
            source.len()
        );
    }
    #[cfg(not(feature = "counting"))]
    println!(
        "{{\"corpus\":\"{name}\",\"mode\":\"{mode}\",\"total_calls\":{},\"bytes\":{},\"tokens\":{tokens},\"capacity\":{capacity},\"ns_per_call\":{}}}",
        passes + 1,
        source.len(),
        nanos / passes.max(1) as u128
    );
    #[cfg(feature = "counting")]
    black_box(nanos);
}
