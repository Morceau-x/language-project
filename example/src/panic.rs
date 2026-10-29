//! Internal compiler errors.
//!
//! `maxim_panic!` marks an invariant one phase guarantees for the next:
//! reaching one is a bug in the compiler, never in the user's program.
//! The CLI catches `MaximPanic` at its boundary and renders it as a
//! structured report (exit code 2) instead of a raw Rust panic.

#[derive(Clone, Debug)]
pub struct MaximPanic {
    pub message: String,
    /// The `file:line` of the panic site — the analogue of the Haskell
    /// `HasCallStack` frame.
    pub location: String,
}

/// Abort on a broken internal invariant. Use only for states earlier
/// phases have made impossible; user mistakes get a [`crate::diag::Diagnostic`].
#[macro_export]
macro_rules! maxim_panic {
    ($($arg:tt)*) => {
        $crate::panic::panic_with(format!($($arg)*), file!(), line!())
    };
}

pub fn panic_with(message: String, file: &'static str, line: u32) -> ! {
    std::panic::panic_any(MaximPanic {
        message,
        location: format!("{file}:{line}"),
    })
}

/// Run `f`, turning a `MaximPanic` unwind into a value. Non-Maxim panics
/// are wrapped as panics too, so the CLI's exit-2 path is total.
pub fn catch_panic<T>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> Result<T, MaximPanic> {
    match std::panic::catch_unwind(f) {
        Ok(v) => Ok(v),
        Err(payload) => Err(match payload.downcast::<MaximPanic>() {
            Ok(p) => *p,
            Err(payload) => {
                let message = if let Some(s) = payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                MaximPanic {
                    message,
                    location: "<unknown>".to_string(),
                }
            }
        }),
    }
}

/// Rendered in the spirit of the diagnostic header rule, minus the
/// source excerpt: a panic has no blameworthy span.
pub fn render_panic(p: &MaximPanic) -> String {
    let title = "-- INTERNAL COMPILER ERROR ";
    let pad = (80 - title.chars().count() as i64).max(1) as usize;
    [
        format!("{}{}", title, "-".repeat(pad)),
        String::new(),
        "This is a bug in the Maxim compiler, not in your program.".to_string(),
        "Please report it along with the source file that triggered it.".to_string(),
        String::new(),
        format!("    {}", p.message),
        String::new(),
        format!("    at {}", p.location),
    ]
    .join("\n")
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_throws_a_catchable_maxim_panic() {
        let r = catch_panic(|| {
            if true {
                maxim_panic!("the sky is falling");
            }
            0
        });
        match r {
            Err(p) => assert_eq!(p.message, "the sky is falling"),
            Ok(n) => panic!("panic returned {n}"),
        }
    }

    #[test]
    fn render_panic_blames_the_compiler_not_the_program() {
        let r = catch_panic(|| -> i32 { maxim_panic!("broken invariant") });
        let p = r.expect_err("must panic");
        let out = render_panic(&p);
        assert!(out.contains("INTERNAL COMPILER ERROR"), "has the header");
        assert!(out.contains("broken invariant"), "carries the message");
        assert!(out.contains("not in your program"), "blames the compiler");
    }
}
