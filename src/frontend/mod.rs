#[cfg(test)]
mod tests;

mod app;
mod splash;
mod ui_event;
mod ui_input_command;

mod app_command_responder;
/// Different frontend ui modes
mod chat_mode;
mod logs_mode;

/// Let's be very strict about what to export
/// to avoid coupling frontend and the rest
pub use app::App;
// pub use ui_event::UIEvent;
// pub use ui_input_command::UserInputCommand;
