//! Private one-request worker. Never run ktrs inside the server process.
use ktlsp_parser::{
    Diagnostic, Element, MAX_DIAGNOSTICS, MAX_ELEMENTS, MAX_NESTING, MAX_OUTPUT_BYTES,
    MAX_SOURCE_BYTES, Reply,
};
use std::io::{Read, Write};

#[path = "../worker_allocator.rs"]
mod worker_allocator;
use worker_allocator::LimitedAllocator;

#[cfg(target_os = "macos")]
#[path = "../macos_phase_worker.rs"]
mod macos_phase_worker;

macro_rules! phase {
    ($observer:expr, $stage:expr, $detail:expr) => {
        #[cfg(target_os = "macos")]
        macos_phase_worker::emit($observer, $stage, $detail);
    };
}

#[global_allocator]
static ALLOCATOR: LimitedAllocator = LimitedAllocator;

fn main() {
    #[cfg(target_os = "macos")]
    let mut phase = macos_phase_worker::Observer::start();
    let result = run(
        #[cfg(target_os = "macos")]
        &mut phase,
    );
    phase!(&mut phase, 16, u64::from(result.is_ok()));
    phase!(&mut phase, 17, u64::from(result.is_ok()));
    std::process::exit(worker_allocator::worker_exit_code(result.is_ok()));
}
fn run(
    #[cfg(target_os = "macos")] phase: &mut Option<macos_phase_worker::Observer>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let kind = match args.next().as_deref() {
        Some("--source") => ktrs_parser::FileKind::Source,
        Some("--script") => ktrs_parser::FileKind::Script,
        _ => {
            phase!(phase, 15, 0);
            return Err("invalid worker mode".into());
        }
    };
    let observing = match args.next().as_deref() {
        None => 0,
        #[cfg(unix)]
        Some("--observe-deadline-eof-v1") => 1,
        #[cfg(unix)]
        Some("--observe-generic-copy-v1") => 2,
        _ => {
            phase!(phase, 15, 0);
            return Err("unexpected argument".into());
        }
    };
    if args.next().is_some() {
        phase!(phase, 15, 0);
        return Err("unexpected argument".into());
    }
    phase!(phase, 2, 0);
    let mut source = String::new();
    phase!(phase, 3, 0);
    std::io::stdin()
        .take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_string(&mut source)
        .inspect_err(|_| {
            phase!(phase, 15, 0);
        })?;
    phase!(phase, 4, source.len() as u64);
    if source.len() > MAX_SOURCE_BYTES || source.contains('\r') {
        phase!(phase, 15, 0);
        return Err("invalid worker input".into());
    }
    phase!(phase, 5, 0);
    ktlsp_parser::preflight(&source).inspect_err(|_| {
        phase!(phase, 15, 0);
    })?;
    phase!(phase, 6, 0);
    phase!(phase, 7, 0);
    #[cfg(unix)]
    let parse = if observing == 1 {
        parse_observing_deadline(&source, kind)
    } else if observing == 2 {
        parse_observing_generic_copy(&source, kind)
    } else {
        ktrs_parser::parse_file(&source, kind)
    };
    #[cfg(not(unix))]
    let parse = {
        let _ = observing;
        ktrs_parser::parse_file(&source, kind)
    };
    phase!(phase, 8, parse.tree.len() as u64);
    let tree = &parse.tree;
    phase!(phase, 9, 0);
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
                let result = write_reply(
                    &Reply::NestingLimit,
                    #[cfg(target_os = "macos")]
                    phase,
                );
                phase!(phase, 15, 0);
                return result;
            }
            depths.push(depth);
            let range = tree.text_range(id);
            let start = u32::from(range.start()) as usize;
            let end = u32::from(range.end()) as usize;
            let kind = tree.kind(id);
            if kind == ktrs_syntax::SyntaxKind::ERROR_ELEMENT {
                let message = messages
                    .next()
                    .ok_or("missing error message")
                    .inspect_err(|_| {
                        phase!(phase, 15, 0);
                    })?;
                if message.len() > 4096 {
                    let result = write_reply(
                        &Reply::OutputLimit,
                        #[cfg(target_os = "macos")]
                        phase,
                    );
                    phase!(phase, 15, 0);
                    return result;
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
            phase!(phase, 15, 0);
            return Err("extra error message".into());
        }
        Reply::Tree {
            elements,
            diagnostics,
        }
    };
    phase!(phase, 10, 0);
    let result = write_reply(
        &reply,
        #[cfg(target_os = "macos")]
        phase,
    );
    phase!(phase, 15, 0);
    result
}
#[cfg(unix)]
std::thread_local! {
    static DEADLINE_OBSERVER: (std::cell::Cell<bool>, std::cell::RefCell<Option<(std::os::unix::net::UnixDatagram, u16)>>) =
        const { (std::cell::Cell::new(false), std::cell::RefCell::new(None)) };
}

#[cfg(unix)]
fn deadline_socket() -> std::io::Result<std::os::unix::net::UnixDatagram> {
    use std::os::{fd::AsFd, unix::net::UnixDatagram};
    let socket = UnixDatagram::from(std::io::stderr().as_fd().try_clone_to_owned()?);
    // Address checks precede shared-OFD mutation; the trusted parent establishes SO_TYPE.
    if !socket.local_addr()?.is_unnamed() || !socket.peer_addr()?.is_unnamed() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    socket.set_nonblocking(true)?;
    Ok(socket)
}

#[cfg(unix)]
fn parse_observing_deadline(source: &str, kind: ktrs_parser::FileKind) -> ktrs_syntax::Parse {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            let _ = DEADLINE_OBSERVER.try_with(|(disabled, state)| {
                disabled.set(true);
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.take();
                }
            });
        }
    }
    let installed = deadline_socket().ok().is_some_and(|socket| {
        DEADLINE_OBSERVER
            .try_with(|(disabled, state)| {
                let Ok(mut state) = state.try_borrow_mut() else {
                    return false;
                };
                *state = Some((socket, 0));
                disabled.set(false);
                true
            })
            .unwrap_or(false)
    });
    let _clear = Clear;
    if installed {
        ktrs_parser::parse_file_observing_eof(source, kind, deadline_checkpoint)
    } else {
        ktrs_parser::parse_file(source, kind)
    }
}

#[cfg(unix)]
fn deadline_checkpoint() {
    let _ = DEADLINE_OBSERVER.try_with(|(disabled, state)| {
        if disabled.get() {
            return;
        }
        let Ok(mut state) = state.try_borrow_mut() else {
            disabled.set(true);
            return;
        };
        let Some((socket, count)) = state.as_mut() else {
            return;
        };
        *count = count.wrapping_add(1);
        if *count & 1023 != 0 {
            return;
        }
        let mut bytes = [0; 17];
        match socket.recv(&mut bytes) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            outcome => {
                disabled.set(true);
                let Some((socket, _)) = state.take() else {
                    return;
                };
                if matches!(outcome, Ok(16)) && bytes[..16] == *b"KTLSP-DUE-v1!!!!" {
                    let _ = socket.send(b"KTLSP-EOF-v1!!!!");
                }
            }
        }
    });
}

#[cfg(unix)]
struct GenericCopyState {
    socket: std::os::unix::net::UnixDatagram,
    source: String,
    record: ktlsp_parser::GenericCopyRecord,
}
#[cfg(unix)]
std::thread_local! {
    static GENERIC_COPY_STATE: (std::cell::Cell<bool>, std::cell::RefCell<Option<GenericCopyState>>) =
        const { (std::cell::Cell::new(false), std::cell::RefCell::new(None)) };
}

#[cfg(unix)]
fn parse_observing_generic_copy(source: &str, kind: ktrs_parser::FileKind) -> ktrs_syntax::Parse {
    use ktlsp_parser::GenericCopyRecord;
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            let _ = GENERIC_COPY_STATE.try_with(|(invalid, state)| {
                invalid.set(true);
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.take();
                }
            });
        }
    }
    // The same trusted-parent socket assumptions and address-before-mutation checks as B3.
    let installed = deadline_socket().ok().is_some_and(|socket| {
        GENERIC_COPY_STATE
            .try_with(|(invalid, state)| {
                let Ok(mut state) = state.try_borrow_mut() else {
                    invalid.set(true);
                    return false;
                };
                if state.is_some() {
                    invalid.set(true);
                    return false;
                }
                let Ok(source_len) = u32::try_from(source.len()) else {
                    invalid.set(true);
                    return false;
                };
                *state = Some(GenericCopyState {
                    socket,
                    source: source.to_owned(),
                    record: GenericCopyRecord {
                        source_len,
                        ..GenericCopyRecord::default()
                    },
                });
                invalid.set(false);
                true
            })
            .unwrap_or(false)
    });
    if !installed {
        return ktrs_parser::parse_file(source, kind);
    }
    let _clear = Clear;
    let (parse, completed) =
        ktrs_parser::parse_file_observing_generic_copy(source, kind, generic_copy_checkpoint);
    let _ = GENERIC_COPY_STATE.try_with(|(invalid, state)| {
        let Ok(mut state) = state.try_borrow_mut() else {
            invalid.set(true);
            return;
        };
        let Some(mut state) = state.take() else {
            invalid.set(true);
            return;
        };
        if invalid.get() || !completed {
            state.record = GenericCopyRecord {
                status: 2,
                source_len: state.record.source_len,
                active_events: state.record.active_events,
                ..GenericCopyRecord::default()
            };
        }
        let _ = state.socket.send(&state.record.to_bytes());
    });
    parse
}

#[cfg(unix)]
fn generic_copy_checkpoint(source: &str, boundary: ktrs_parser::GenericCopyBoundary) {
    use ktlsp_parser::GenericCopyRecord;
    let _ = GENERIC_COPY_STATE.try_with(|(invalid, state)| {
        if invalid.get() {
            return;
        }
        let Ok(mut state) = state.try_borrow_mut() else {
            invalid.set(true);
            return;
        };
        let Some(state) = state.as_mut() else {
            invalid.set(true);
            return;
        };
        if source != state.source || boundary.active_layers == 0 {
            return;
        }
        state.record.active_events = (state.record.active_events + 1).min(2);
        if state.record.active_events == 2 {
            invalid.set(true);
            return;
        }
        let (Ok(active_layers), Some(actual_copy_end), Some(independent_min)) = (
            u32::try_from(boundary.active_layers),
            boundary.actual_copy_end,
            boundary.independent_min,
        ) else {
            invalid.set(true);
            return;
        };
        if independent_min < 0 || boundary.production_bound.is_some_and(|value| value < 0) {
            invalid.set(true);
            return;
        }
        let (next_start, next_end) = boundary.next_token.unwrap_or((0, 0));
        state.record = GenericCopyRecord {
            status: 1,
            source_len: state.record.source_len,
            active_events: 1,
            active_layers,
            production_bound: boundary.production_bound.unwrap_or(-1),
            independent_min,
            window_start: boundary.window_start,
            actual_copy_end,
            next_start,
            next_end,
            next_fits_other_caps: u32::from(boundary.next_fits_other_caps),
            reserved: 0,
        };
    });
}

fn write_reply(
    reply: &Reply,
    #[cfg(target_os = "macos")] phase: &mut Option<macos_phase_worker::Observer>,
) -> Result<(), Box<dyn std::error::Error>> {
    phase!(phase, 11, 0);
    let bytes = serde_json::to_vec(reply)?;
    phase!(phase, 12, bytes.len() as u64);
    phase!(phase, 13, 0);
    if bytes.len() > MAX_OUTPUT_BYTES {
        serde_json::to_writer(std::io::stdout().lock(), &Reply::OutputLimit)?;
    } else {
        std::io::stdout().lock().write_all(&bytes)?;
    }
    phase!(phase, 14, 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ktlsp_parser::MAX_ALLOCATION_BYTES;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use worker_allocator::charge;
    #[cfg(unix)]
    #[test]
    fn generic_copy_aggregation_does_not_filter_bad_endpoints_or_overwrite_duplicates() {
        use ktlsp_parser::GenericCopyRecord;
        use std::os::unix::net::UnixDatagram;
        let (_parent, socket) = UnixDatagram::pair().unwrap();
        GENERIC_COPY_STATE.with(|(invalid, state)| {
            invalid.set(false);
            *state.borrow_mut() = Some(GenericCopyState {
                socket,
                source: "source".into(),
                record: GenericCopyRecord {
                    source_len: 6,
                    ..GenericCopyRecord::default()
                },
            });
        });
        let boundary = ktrs_parser::GenericCopyBoundary {
            active_layers: 1,
            independent_min: Some(3),
            production_bound: None,
            window_start: 1,
            actual_copy_end: Some(6),
            next_token: None,
            next_fits_other_caps: false,
        };
        generic_copy_checkpoint("other", boundary);
        generic_copy_checkpoint(
            "source",
            ktrs_parser::GenericCopyBoundary {
                active_layers: 0,
                ..boundary
            },
        );
        GENERIC_COPY_STATE.with(|(_, state)| {
            assert_eq!(state.borrow().as_ref().unwrap().record.active_events, 0)
        });
        generic_copy_checkpoint("source", boundary);
        GENERIC_COPY_STATE.with(|(invalid, state)| {
            assert!(!invalid.get());
            let record = state.borrow().as_ref().unwrap().record;
            assert_eq!(
                (
                    record.active_events,
                    record.actual_copy_end,
                    record.production_bound
                ),
                (1, 6, -1)
            );
        });
        generic_copy_checkpoint(
            "source",
            ktrs_parser::GenericCopyBoundary {
                actual_copy_end: Some(3),
                ..boundary
            },
        );
        GENERIC_COPY_STATE.with(|(invalid, state)| {
            assert!(invalid.get());
            assert_eq!(
                (
                    state.borrow().as_ref().unwrap().record.active_events,
                    state.borrow().as_ref().unwrap().record.actual_copy_end
                ),
                (2, 6)
            );
            invalid.set(false);
            let borrowed = state.borrow_mut();
            generic_copy_checkpoint("source", boundary);
            assert!(invalid.get());
            drop(borrowed);
            state.borrow_mut().take();
        });
    }
    #[cfg(unix)]
    #[test]
    fn generic_copy_callback_restores_nested_and_unwinding_scopes() {
        std::thread_local! { static CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
        fn count(_: &str, _: ktrs_parser::GenericCopyBoundary) {
            CALLS.with(|count| count.set(count.get() + 1));
        }
        fn nested(source: &str, boundary: ktrs_parser::GenericCopyBoundary) {
            count(source, boundary);
            let (_, installed) = ktrs_parser::parse_file_observing_generic_copy(
                "class N<T fun nested()=0",
                ktrs_parser::FileKind::Source,
                count,
            );
            assert!(!installed);
        }
        fn panic_observer(_: &str, _: ktrs_parser::GenericCopyBoundary) {
            panic!("machinery unwind");
        }
        let source = "class C<T fun f()=1";
        let (_, installed) = ktrs_parser::parse_file_observing_generic_copy(
            source,
            ktrs_parser::FileKind::Source,
            nested,
        );
        assert!(installed);
        let calls = CALLS.with(std::cell::Cell::get);
        assert!(calls > 0);
        ktrs_parser::parse_file(source, ktrs_parser::FileKind::Source);
        assert_eq!(CALLS.with(std::cell::Cell::get), calls);
        assert!(
            std::panic::catch_unwind(|| ktrs_parser::parse_file_observing_generic_copy(
                source,
                ktrs_parser::FileKind::Source,
                panic_observer
            ))
            .is_err()
        );
        ktrs_parser::parse_file(source, ktrs_parser::FileKind::Source);
        let (_, installed) = ktrs_parser::parse_file_observing_generic_copy(
            source,
            ktrs_parser::FileKind::Source,
            count,
        );
        assert!(installed);
        assert!(CALLS.with(std::cell::Cell::get) > calls);
    }
    #[cfg(unix)]
    fn is_datagram_backpressure(error: &std::io::Error) -> bool {
        error.kind() == std::io::ErrorKind::WouldBlock
            // Darwin reports a full nonblocking Unix datagram queue as ENOBUFS,
            // not EAGAIN. This classification belongs only to the fill fixture.
            || (cfg!(target_vendor = "apple") && error.raw_os_error() == Some(libc::ENOBUFS))
    }

    #[cfg(unix)]
    #[test]
    fn datagram_backpressure_classification_keeps_other_errors_fatal() {
        assert!(is_datagram_backpressure(
            &std::io::ErrorKind::WouldBlock.into()
        ));
        assert_eq!(
            is_datagram_backpressure(&std::io::Error::from_raw_os_error(libc::ENOBUFS)),
            cfg!(target_vendor = "apple")
        );
        for code in [libc::EINTR, libc::EBADF, libc::EMSGSIZE, libc::ECONNRESET] {
            assert!(!is_datagram_backpressure(
                &std::io::Error::from_raw_os_error(code)
            ));
        }
        assert!(!is_datagram_backpressure(&std::io::Error::other(
            "not a queue error"
        )));
    }

    #[cfg(unix)]
    #[test]
    fn checkpoint_is_sampled_and_disarms_before_one_ack_attempt() {
        use std::os::unix::net::UnixDatagram;
        for packet in [
            b"KTLSP-DUE-v1!!!!".as_slice(),
            b"KTLSP-DUE-v1!!!!!".as_slice(),
            b"bad".as_slice(),
            b"".as_slice(),
        ] {
            let (parent, socket) = UnixDatagram::pair().unwrap();
            parent.set_nonblocking(true).unwrap();
            socket.set_nonblocking(true).unwrap();
            // Keep the channel live after disarm drops its observer-owned alias.
            // Production likewise retains the original stderr fd until exit.
            // An empty live queue, not peer-close errno, proves no extra ACK.
            let _worker_endpoint = socket.try_clone().unwrap();
            DEADLINE_OBSERVER.with(|(disabled, state)| {
                disabled.set(false);
                *state.borrow_mut() = Some((socket, 0));
            });
            // A pre-decision sampled WouldBlock must keep the hook armed.
            for _ in 0..1024 {
                deadline_checkpoint();
            }
            DEADLINE_OBSERVER.with(|(disabled, state)| {
                assert!(!disabled.get());
                assert!(state.borrow().is_some());
            });
            parent.send(packet).unwrap();
            for _ in 0..1023 {
                deadline_checkpoint();
            }
            let mut bytes = [0; 17];
            assert_eq!(
                parent.recv(&mut bytes).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            deadline_checkpoint();
            DEADLINE_OBSERVER.with(|(disabled, state)| {
                assert!(disabled.get());
                assert!(state.borrow().is_none());
            });
            if packet == b"KTLSP-DUE-v1!!!!" {
                assert_eq!(parent.recv(&mut bytes).unwrap(), 16);
                assert_eq!(&bytes[..16], b"KTLSP-EOF-v1!!!!");
            }
            for _ in 0..2048 {
                deadline_checkpoint();
            }
            assert_eq!(
                parent.recv(&mut bytes).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn reentrant_checkpoint_disables_without_panicking() {
        use std::os::unix::net::UnixDatagram;
        let (_parent, socket) = UnixDatagram::pair().unwrap();
        DEADLINE_OBSERVER.with(|(disabled, state)| {
            disabled.set(false);
            *state.borrow_mut() = Some((socket, 0));
            let mut borrowed = state.borrow_mut();
            deadline_checkpoint();
            assert!(disabled.get());
            borrowed.take();
        });
    }
    #[cfg(unix)]
    #[test]
    fn acknowledgement_backpressure_and_peer_loss_do_not_retry() {
        use std::os::unix::net::UnixDatagram;
        for peer_lost in [false, true] {
            let (parent, socket) = UnixDatagram::pair().unwrap();
            parent.set_nonblocking(true).unwrap();
            socket.set_nonblocking(true).unwrap();
            let _worker_endpoint = socket.try_clone().unwrap();
            let mut queued = 0;
            if !peer_lost {
                let mut full = false;
                for _ in 0..4096 {
                    match socket.send(b"synthetic-noise") {
                        Ok(count) => {
                            assert_eq!(count, b"synthetic-noise".len());
                            queued += 1;
                        }
                        Err(e) if is_datagram_backpressure(&e) => {
                            full = true;
                            break;
                        }
                        Err(e) => panic!("unexpected queue error: {e}"),
                    }
                }
                assert!(full, "finite synthetic queue did not backpressure");
                assert!(queued > 0, "fixture must queue noise before backpressure");
            }
            parent.send(b"KTLSP-DUE-v1!!!!").unwrap();
            let parent = if peer_lost {
                drop(parent);
                None
            } else {
                Some(parent)
            };
            DEADLINE_OBSERVER.with(|(disabled, state)| {
                disabled.set(false);
                *state.borrow_mut() = Some((socket, 0));
            });
            for _ in 0..1024 {
                deadline_checkpoint();
            }
            DEADLINE_OBSERVER.with(|(disabled, state)| {
                assert!(disabled.get());
                assert!(state.borrow().is_none());
            });
            if let Some(parent) = &parent {
                let mut bytes = [0; 17];
                for _ in 0..queued {
                    let count = parent.recv(&mut bytes).unwrap();
                    assert_eq!(&bytes[..count], b"synthetic-noise");
                }
                assert_eq!(
                    parent.recv(&mut bytes).unwrap_err().kind(),
                    std::io::ErrorKind::WouldBlock
                );
            }
            // Once capacity returns, an accidentally armed retry would now
            // become observable. Disarm must prevent a late ACK as well.
            for _ in 0..2048 {
                deadline_checkpoint();
            }
            DEADLINE_OBSERVER.with(|(disabled, state)| {
                assert!(disabled.get());
                assert!(state.borrow().is_none());
            });
            if let Some(parent) = &parent {
                assert_eq!(
                    parent.recv(&mut [0; 17]).unwrap_err().kind(),
                    std::io::ErrorKind::WouldBlock
                );
            }
        }
    }
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
