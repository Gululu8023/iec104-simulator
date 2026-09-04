use serde::Serialize;
use tauri::Emitter;

#[derive(Clone)]
pub struct RuntimeEventSink {
    app_handle: tauri::AppHandle,
}

impl RuntimeEventSink {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn emit<T>(&self, event_name: &str, payload: T) -> tauri::Result<()>
    where T: Serialize + Clone {
        self.app_handle.emit(event_name, payload)
    }
}
