use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Spaces requests across workspaces and callers that share a provider limit.
pub(super) struct RateGate(Mutex<Option<Instant>>);

impl RateGate {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub(super) fn wait(&self, spacing: Duration) {
        let mut previous = crate::lock::lock(&self.0);
        if let Some(previous) = *previous {
            std::thread::sleep(spacing.saturating_sub(previous.elapsed()));
        }
        *previous = Some(Instant::now());
    }
}

/// URI component encoding: exchange names and index symbols may contain spaces, `&` and `:`.
pub(crate) fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    use std::fmt::Write;
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}
