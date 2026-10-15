use std::sync::{Mutex, MutexGuard};

static FUNCTIONAL: Mutex<()> = Mutex::new(());

pub fn serial() -> MutexGuard<'static, ()> {
    FUNCTIONAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
