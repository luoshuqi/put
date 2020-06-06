pub mod config;
mod css_dialog;
pub mod db;
mod dialog;
mod editor;
mod group_dialog;
mod request;
mod request_store;
mod window;
pub use config::Group;
pub use css_dialog::CssDialog;
pub use editor::Editor;
pub use group_dialog::GroupDialog;
pub use request::Request;
pub use request::Response;
pub use request_store::RequestStore;
pub use window::Window;

use std::fs;
use std::path::PathBuf;

pub fn app_dir() -> PathBuf {
    let mut dir = dirs::config_dir().expect("get config dir failed");
    dir.push("put");
    if !dir.exists() {
        fs::create_dir(dir.as_path()).expect("create dir failed");
    }
    dir
}
