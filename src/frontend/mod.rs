mod app;
mod splash;
mod ui_event;
mod ui_input_command;

mod actions;
mod app_command_responder;
/// Different frontend ui modes
mod chat_mode;
mod logs_mode;

/// Let's be very strict about what to export
/// to avoid coupling frontend and the rest
pub use app::App;

pub use app_command_responder::AppCommandResponder;
pub use chat_mode::ui;
pub use ui_event::UIEvent;
pub use ui_input_command::{DiffVariant, UserInputCommand};
