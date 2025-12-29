use std::fmt;

#[derive(Debug, Clone)]
pub struct ClipboardError {
    message: String,
}

impl ClipboardError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ClipboardError {}

impl From<arboard::Error> for ClipboardError {
    fn from(err: arboard::Error) -> Self {
        ClipboardError::new(err.to_string())
    }
}

pub struct SystemClipboard {
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    pub fn new() -> Result<Self, ClipboardError> {
        Ok(Self {
            inner: arboard::Clipboard::new()?,
        })
    }

    pub fn set_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.inner.set_text(text.to_string())?;
        Ok(())
    }
}
