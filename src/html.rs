//! Escaping text for the server-rendered pages, so untrusted text (a
//! patient's name, a physician's words) stays text.

/// `text` with `&`, `<`, `>` and double quotes escaped for HTML: safe in
/// element content and in double-quoted attributes.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
