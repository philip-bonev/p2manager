// Copyright 2026 Philip Bonev
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://apache.org
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
