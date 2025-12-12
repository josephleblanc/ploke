//! Structured, log-friendly pretty-print helpers.
//!
//! This is intentionally gated behind the `serde` feature to avoid pulling
//! serialization dependencies into consumers that don't need them.

#![cfg(feature = "serde")]

use serde::Serialize;

/// Provide a structured view of an error or diagnostic for logging/UI.
///
/// Implementors expose a lightweight set of serializable fields; callers can
/// choose between the raw fields, a `serde_json::Value`, or a pretty JSON
/// string for human consumption.
pub trait PrettyDebug {
    type Fields<'a>: Serialize + 'a
    where
        Self: 'a;

    /// Return the structured fields, if available.
    fn fields(&self) -> Option<Self::Fields<'_>>;

    /// Convert fields to a JSON value without pretty whitespace.
    fn to_value(&self) -> Option<serde_json::Value> {
        self.fields().and_then(|f| serde_json::to_value(&f).ok())
    }

    /// Convert fields to a pretty JSON string (for logs or UI).
    fn pretty_json(&self) -> Option<String> {
        self.fields()
            .and_then(|f| serde_json::to_string_pretty(&f).ok())
    }

    /// Convert fields to a pretty JSON string and panic if serialization fails.
    ///
    /// Useful in tracing calls where fallible plumbing is noisy and you prefer a hard
    /// failure over silently missing structured data.
    fn pretty_json_or_panic(&self) -> Option<String> {
        self.fields().map(|f| {
            serde_json::to_string_pretty(&f)
                .expect("PrettyDebug serialization should not fail; verify Fields implementation")
        })
    }

    /// Emit a tracing event with both the Display string and structured fields when available.
    #[cfg(feature = "tracing")]
    fn emit_tracing(&self, level: tracing::Level, message: &str)
    where
        Self: std::fmt::Display,
        for<'a> Self::Fields<'a>: std::fmt::Debug,
    {
        let emit = |level: tracing::Level, fields: Option<Self::Fields<'_>>| match (fields, level) {
            (Some(f), tracing::Level::ERROR) => {
                tracing::event!(tracing::Level::ERROR, error = %self, fields = ?f, "{message}")
            }
            (None, tracing::Level::ERROR) => {
                tracing::event!(tracing::Level::ERROR, error = %self, "{message}")
            }
            (Some(f), tracing::Level::WARN) => {
                tracing::event!(tracing::Level::WARN, error = %self, fields = ?f, "{message}")
            }
            (None, tracing::Level::WARN) => {
                tracing::event!(tracing::Level::WARN, error = %self, "{message}")
            }
            (Some(f), tracing::Level::INFO) => {
                tracing::event!(tracing::Level::INFO, error = %self, fields = ?f, "{message}")
            }
            (None, tracing::Level::INFO) => {
                tracing::event!(tracing::Level::INFO, error = %self, "{message}")
            }
            (Some(f), tracing::Level::DEBUG) => {
                tracing::event!(tracing::Level::DEBUG, error = %self, fields = ?f, "{message}")
            }
            (None, tracing::Level::DEBUG) => {
                tracing::event!(tracing::Level::DEBUG, error = %self, "{message}")
            }
            (Some(f), tracing::Level::TRACE) => {
                tracing::event!(tracing::Level::TRACE, error = %self, fields = ?f, "{message}")
            }
            (None, tracing::Level::TRACE) => {
                tracing::event!(tracing::Level::TRACE, error = %self, "{message}")
            }
        };

        emit(level, self.fields());
    }
}

#[cfg(test)]
mod tests {
    use super::PrettyDebug;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    struct DemoFields<'a> {
        code: u32,
        msg: &'a str,
    }

    #[derive(Debug)]
    struct DemoError {
        code: u32,
        msg: String,
    }

    impl std::fmt::Display for DemoError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} ({})", self.msg, self.code)
        }
    }

    impl PrettyDebug for DemoError {
        type Fields<'a> = DemoFields<'a>;

        fn fields(&self) -> Option<Self::Fields<'_>> {
            Some(DemoFields {
                code: self.code,
                msg: &self.msg,
            })
        }
    }

    #[test]
    fn pretty_debug_outputs_value_and_string() {
        let err = DemoError {
            code: 42,
            msg: "boom".to_string(),
        };

        let value = err.to_value().expect("value");
        assert_eq!(value["code"], 42);
        assert_eq!(value["msg"], "boom");

        let pretty = err.pretty_json().expect("pretty json");
        assert!(pretty.contains("\"code\": 42"));
        assert!(pretty.contains("\"msg\": \"boom\""));
    }

    #[test]
    fn pretty_debug_expect_variant() {
        let err = DemoError {
            code: 7,
            msg: "lucky".to_string(),
        };

        let pretty = err.pretty_json_or_panic().expect("pretty json");
        assert!(pretty.contains("\"code\": 7"));
        assert!(pretty.contains("\"msg\": \"lucky\""));
    }

    #[cfg(feature = "tracing")]
    #[test]
    fn emit_tracing_does_not_panic() {
        let _ = tracing_subscriber::fmt::try_init();
        let err = DemoError {
            code: 500,
            msg: "boom".to_string(),
        };
        err.emit_tracing(tracing::Level::ERROR, "demo emit");
    }
}
