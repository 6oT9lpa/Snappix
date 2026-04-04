pub mod callbacks;
pub mod clipboard;
pub mod helpers;
pub mod history;
pub mod hotkeys;
pub mod settings;
pub mod state;
pub mod sync;

pub use callbacks::register_callbacks;
pub use hotkeys::configure_hotkeys;
pub use settings::configure_app_settings;
pub use state::EditorState;
pub use sync::load_recent_projects;
