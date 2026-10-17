//! The browser's own dialogs and address bar (web only).

use wasm_bindgen::JsValue;

/// A message box.
pub fn alert(msg: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.alert_with_message(msg);
    }
}

/// Ask for a line of text (`None`: cancelled).
pub fn prompt(msg: &str) -> Option<String> {
    web_sys::window()?.prompt_with_message(msg).ok().flatten()
}

/// Drop the address's `#...` (a share link's level) without reloading.
pub fn clear_hash() {
    let Some(w) = web_sys::window() else { return };
    let l = w.location();
    let url = format!("{}{}", l.pathname().unwrap_or_default(), l.search().unwrap_or_default());
    if let Ok(h) = w.history() {
        let _ = h.replace_state_with_url(&JsValue::NULL, "", Some(&url));
    }
}
