//! NFR-13 robustness: no panic crosses into the app.
//!
//! Every Tauri command runs inside [`catch`]: a panic becomes a rejected
//! promise with a plain message instead of unwinding through the webview
//! (which would abort the app). Locks use [`lock`], which recovers a mutex
//! that an earlier panic left poisoned, so one bad command does not break
//! every later one.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, MutexGuard};

/// Readable text of a panic payload.
pub fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    let detail = p
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown error".into());
    format!("Internt fel / internal error: {detail}")
}

/// Run `f`; if it panics, hand the message to `on_panic` and return `ret`.
pub fn catch<T>(f: impl FnOnce() -> T, ret: T, on_panic: impl FnOnce(String)) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(p) => {
            let msg = panic_message(p.as_ref());
            log::error!("command panicked: {msg}");
            on_panic(msg);
            ret
        }
    }
}

/// Run a command body and turn a panic into `Err(message)`.
#[cfg(test)]
pub fn command<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let mut err = None;
    let r = catch(|| Some(f()), None, |m| err = Some(m));
    r.unwrap_or_else(|| Err(err.unwrap_or_default()))
}

/// Lock, recovering a mutex poisoned by an earlier panic.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfr_13_panicking_command_returns_plain_error() {
        let r: Result<u8, String> = command(|| panic!("index out of bounds"));
        let e = r.unwrap_err();
        assert!(e.contains("internal error"), "{e}");
        assert!(e.contains("index out of bounds"), "{e}");
        let fmt: Result<u8, String> = command(|| panic!("bad {}", 7));
        assert!(fmt.unwrap_err().contains("bad 7"));
        assert_eq!(command(|| Ok::<_, String>(3)), Ok(3));
    }

    #[test]
    fn nfr_13_poisoned_lock_recovers() {
        let m = std::sync::Arc::new(Mutex::new(1));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert!(m.is_poisoned());
        *lock(&m) += 1;
        assert_eq!(*lock(&m), 2);
    }

    #[test]
    fn nfr_13_invoke_guard_reports_and_survives() {
        let mut got = String::new();
        let handled = catch(|| -> bool { panic!("boom") }, true, |m| got = m);
        assert!(handled);
        assert!(got.contains("boom"));
    }
}
