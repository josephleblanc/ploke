/// Severity levels for error events
#[derive(Debug, Clone, Copy)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Fatal,
}

/// Extension trait for ergonomic error emission
pub trait ResultExt<T> {
    /// Emit an error event through the global event bus
    fn emit_event(self, severity: ErrorSeverity) -> Self;

    /// Emit a warning event
    fn emit_warning(self) -> Self;

    /// Emit an error event
    fn emit_error(self) -> Self;

    /// Emit a fatal event
    fn emit_fatal(self) -> Self;
}

/// Extension trait for direct error emission
pub trait ErrorExt {
    fn emit_event(&self, severity: ErrorSeverity);
    fn emit_warning(&self) {
        self.emit_event(ErrorSeverity::Warning)
    }
    fn emit_error(&self) {
        self.emit_event(ErrorSeverity::Error)
    }
    fn emit_fatal(&self) {
        self.emit_event(ErrorSeverity::Fatal)
    }
}

use std::{marker::PhantomData, ops::Deref, sync::Arc};

use syn_parser::discovery::DiscoveryError;
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    AppEvent, EventBus,
    app::commands::parser::Command,
    app_state::{AppState, events::SystemEvent, handlers::chat::add_msg_immediate},
    chat_history::MessageKind,
};

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn emit_event(self, severity: ErrorSeverity) -> Self {
        if let Err(err) = self.as_ref() {
            match severity {
                ErrorSeverity::Warning => warn!(target: "ploke_tui::error", "Warning: {:?}", err),
                ErrorSeverity::Error => error!(target: "ploke_tui::error", "Error: {:?}", err),
                ErrorSeverity::Fatal => error!(target: "ploke_tui::error", "Fatal: {:?}", err),
            }
        }
        self
    }

    fn emit_warning(self) -> Self {
        self.emit_event(ErrorSeverity::Warning)
    }

    fn emit_error(self) -> Self {
        self.emit_event(ErrorSeverity::Error)
    }

    fn emit_fatal(self) -> Self {
        self.emit_event(ErrorSeverity::Fatal)
    }
}

impl<E> ErrorExt for E
where
    E: std::fmt::Debug,
{
    fn emit_event(&self, severity: ErrorSeverity) {
        match severity {
            ErrorSeverity::Warning => warn!(target: "ploke_tui::error", "Warning: {:?}", self),
            ErrorSeverity::Error => error!(target: "ploke_tui::error", "Error: {:?}", self),
            ErrorSeverity::Fatal => error!(target: "ploke_tui::error", "Fatal: {:?}", self),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub struct UiError<A, R = Absent>
where
    A: AudienceMarker,
{
    message: Option<String>,
    // Might want to make this app::Command instead of String, or a struct that
    // includes app::Command and optionally other info like a PathBuf for
    // suggestions like: "Try the `/index crate <path>`", where path is filled in.
    recovery_command: Option<String>,
    formatted_message: Option<String>,
    audience: PhantomData<A>,
    _has_recovery: PhantomData<R>,
}

pub struct Absent;
pub struct Present;
pub struct Formatted;

pub struct Message<A: AudienceMarker> {
    message: String,
    _audience: PhantomData<A>,
}

impl Message<UserAudience> {
    fn user_new(message: String) -> Self {
        Message {
            message,
            _audience: PhantomData,
        }
    }
}

// works for any audience, e.g. UserAudience, LlmAudience, etc that implements AudienceMarker
// should inherit audience type A from Message
impl<A: AudienceMarker> UiError<A, Absent> {
    fn new_from_message(m: Message<A>) -> UiError<A, Absent> {
        UiError {
            message: Some(m.message),
            recovery_command: None,
            formatted_message: None,
            audience: PhantomData,
            _has_recovery: PhantomData,
        }
    }

    // once recovery is present we transform into UiError<A, P: PresentMarker>
    // could also make `recovery` typed but that might be too spooky
    fn with_recovery(self, recovery: String) -> UiError<A, Present> {
        UiError {
            message: self.message,
            recovery_command: Some(recovery),
            formatted_message: None,
            audience: PhantomData,
            _has_recovery: PhantomData,
        }
    }
}

// works for any audience type, uses formatting via a closure
// same audience in as out
impl<A: AudienceMarker> UiError<A, Present> {
    pub fn format_recovery(
        self,
        format: impl FnOnce(String, String) -> String,
    ) -> UiError<A, Formatted> {
        let formatted = match (self.recovery_command, self.message) {
            (Some(r), Some(m)) => format(r, m),
            _ => unreachable!(),
        };
        UiError {
            message: None,
            recovery_command: None,
            formatted_message: Some(formatted),
            audience: PhantomData,
            _has_recovery: PhantomData,
        }
    }
}

// now that it's formatted we are ready to send it to UI
impl UiError<UserAudience, Formatted> {
    pub fn send_ui(self, event_ctx: EventCtx<UserAudience>) -> impl Future<Output = ()> {
        let content = match self.formatted_message {
            Some(c) => c,
            None => unreachable!(),
        };
        event_ctx.send_sysinfo(content.into())
    }
}

pub struct EventCtx<'s, A>
where
    A: AudienceMarker,
{
    event_bus: &'s Arc<EventBus>,
    state: &'s Arc<AppState>,
    audience: A,
}
impl<'s> EventCtx<'s, UserAudience> {
    fn send_sysinfo(self, content: String) -> impl Future<Output = ()> {
        add_msg_immediate(
            self.state,
            self.event_bus,
            Uuid::new_v4(),
            content,
            MessageKind::SysInfo,
        )
    }
}

pub trait AudienceMarker {}
// later maybe add
// LlmAudience
// SystemAudience
pub struct UserAudience;
impl AudienceMarker for UserAudience {}
