//! Request-scoped, Rust-only accounting for guarded frontend work and recursion.
//!
//! This does not grow the stack or prove a sufficient fixed stack size. The embedding worker must
//! supply thread-stack capacity and process containment for recursion not covered by these funnels.

use std::cell::Cell;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

/// Shared semantic expression-nesting contract; parser entries may consume two levels per expression.
pub(crate) const MAX_SEMANTIC_EXPR_DEPTH: u32 = 500;

/// Request-wide limits on guarded entries, not bytes or a bound on every compiler operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub work: usize,
    pub depth: usize,
}

/// Terminal resource failures: no partially checked output from this request may be published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitExceeded {
    Work,
    Depth,
    SemanticExpressionDepth,
    TypeAliasExpansionDepth,
    SourceHierarchyNodes,
}

#[derive(Clone, Copy)]
struct State {
    remaining: usize,
    depth: usize,
    depth_limit: usize,
    failure: Option<LimitExceeded>,
}

thread_local! {
    static STATE: Cell<Option<State>> = const { Cell::new(None) };
}

// Only this private payload is caught as a resource failure, never an arbitrary caller panic.
#[derive(Debug)]
struct ResourcePanic(LimitExceeded);

struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        STATE.with(|state| state.set(None));
    }
}

/// Run one whole request on the caller's worker-owned thread.
///
/// Nested calls inherit the outermost limits and cumulative accounting; they do not establish a
/// new recovery boundary. A resource failure propagates to the outermost call and remains terminal
/// even if intermediate code catches its unwind. Unrelated panics are resumed unchanged.
/// All compiler-owned state must be discarded after a resource failure.
pub fn with_limits<T>(limits: Limits, f: impl FnOnce() -> T) -> Result<T, LimitExceeded> {
    if STATE.with(|state| state.get().is_some()) {
        return Ok(f());
    }
    STATE.with(|state| {
        state.set(Some(State {
            remaining: limits.work,
            depth: 0,
            depth_limit: limits.depth,
            failure: None,
        }));
    });
    let _scope = Scope;
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => match STATE.with(|state| state.get().unwrap().failure) {
            Some(error) => Err(error),
            None => Ok(value),
        },
        Err(payload) => match payload.downcast::<ResourcePanic>() {
            Ok(error) => Err(error.0),
            Err(payload) => resume_unwind(payload),
        },
    }
}

/// Abort the active request rather than substituting incomplete semantic facts.
pub(crate) fn exceed(error: LimitExceeded) -> ! {
    let error = STATE.with(|slot| {
        let mut state = slot.get().expect("frontend work requires with_limits");
        let error = *state.failure.get_or_insert(error);
        slot.set(Some(state));
        error
    });
    resume_unwind(Box::new(ResourcePanic(error)))
}

pub(crate) struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        STATE.with(|slot| {
            let mut state = slot.get().expect("resource guard outlived its request");
            state.depth -= 1;
            slot.set(Some(state));
        });
    }
}

/// Count a recursive funnel entry, retaining its depth until the returned guard is dropped.
pub(crate) fn enter() -> Guard {
    let error = STATE.with(|slot| {
        let mut state = slot.get().expect("frontend work requires with_limits");
        if let Some(error) = state.failure {
            return Some(error);
        }
        if state.remaining == 0 {
            return Some(LimitExceeded::Work);
        }
        if state.depth >= state.depth_limit {
            return Some(LimitExceeded::Depth);
        }
        state.remaining -= 1;
        state.depth += 1;
        slot.set(Some(state));
        None
    });
    if let Some(error) = error {
        exceed(error);
    }
    Guard
}

/// Run guarded frontend work on the same thread; stack capacity belongs to the worker.
pub(crate) fn on_wide_stack<T>(f: impl FnOnce() -> T) -> T {
    let _guard = enter();
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_are_cumulative_and_scopes_restore_after_all_exits() {
        let limits = Limits { work: 2, depth: 2 };
        assert_eq!(
            with_limits(limits, || {
                on_wide_stack(|| ());
                with_limits(
                    Limits {
                        work: usize::MAX,
                        depth: usize::MAX,
                    },
                    || {
                        on_wide_stack(|| ());
                        on_wide_stack(|| ());
                    },
                )
            }),
            Err(LimitExceeded::Work),
        );
        assert_eq!(with_limits(limits, || on_wide_stack(|| 7)), Ok(7));
        assert_eq!(
            with_limits(limits, || on_wide_stack(|| on_wide_stack(
                || on_wide_stack(|| ())
            ))),
            Err(LimitExceeded::Work),
        );
        assert_eq!(
            with_limits(Limits { work: 10, depth: 1 }, || {
                on_wide_stack(|| on_wide_stack(|| ()))
            }),
            Err(LimitExceeded::Depth),
        );
        let panic = catch_unwind(|| with_limits(limits, || panic!("unrelated")));
        assert_eq!(*panic.unwrap_err().downcast::<&str>().unwrap(), "unrelated");
        assert_eq!(with_limits(limits, || on_wide_stack(|| 8)), Ok(8));
        assert!(catch_unwind(|| on_wide_stack(|| ())).is_err());
    }

    #[test]
    fn caught_resource_panics_still_reject_the_whole_request() {
        assert_eq!(
            with_limits(Limits { work: 0, depth: 1 }, || {
                let _ = catch_unwind(|| on_wide_stack(|| ()));
                "incomplete"
            }),
            Err(LimitExceeded::Work),
        );
        assert_eq!(
            with_limits(Limits { work: 10, depth: 1 }, || {
                let _ = catch_unwind(|| on_wide_stack(|| panic!("unrelated")));
                on_wide_stack(|| 9)
            }),
            Ok(9),
        );
        let panic = catch_unwind(|| {
            with_limits(Limits { work: 1, depth: 1 }, || {
                std::panic::panic_any(LimitExceeded::Work)
            })
        });
        assert_eq!(
            *panic.unwrap_err().downcast::<LimitExceeded>().unwrap(),
            LimitExceeded::Work
        );
    }

    #[test]
    fn semantic_lexing_counts_flat_tokens_and_nested_templates() {
        let mut diagnostics = crate::diag::DiagSink::new();
        assert!(matches!(
            with_limits(Limits { work: 2, depth: 10 }, || {
                crate::lexer::lex("a + b", &mut diagnostics)
            }),
            Err(LimitExceeded::Work),
        ));
        let mut source = "value".to_owned();
        for _ in 0..4 {
            source = format!("\"${{{source}}}\"");
        }
        let limits = Limits {
            work: 1_000,
            depth: 3,
        };
        assert!(matches!(
            with_limits(limits, || crate::lexer::lex(&source, &mut diagnostics)),
            Err(LimitExceeded::Depth),
        ));
        assert!(matches!(
            with_limits(limits, || crate::lexer::lex_name_tokens(
                &source,
                &mut diagnostics
            )),
            Err(LimitExceeded::Depth),
        ));
        assert!(with_limits(limits, || {
            crate::lexer::lex("\"hello $value\"", &mut diagnostics)
        })
        .is_ok());
    }
}
