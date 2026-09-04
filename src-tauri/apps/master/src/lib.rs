use iec104_simulator_backend::{AppRole, api, setup_app};
use tokio::runtime::Runtime;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime = Runtime::new().expect("无法创建Tokio运行时");
    let _guard = runtime.enter();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| setup_app(app, AppRole::Master))
        .invoke_handler(api::master_invoke_handler())
        .run(tauri::generate_context!())
        .expect("Tauri主站应用运行出错");
}
