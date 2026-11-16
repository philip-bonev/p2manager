use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct WindowGeometry {
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) position: Option<(i32, i32)>,
}

pub(crate) struct AppState {
    pub(crate) geometry: Mutex<WindowGeometry>,
    pub(crate) cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub(crate) progress: Mutex<HashMap<String, serde_json::Value>>,
}
