use anyhow::Result;
use std::time::Duration;
use strum::IntoEnumIterator as _;
use tui_logger::TuiWidgetState;
use tui_textarea::TextArea;
use uuid::Uuid;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, ListState, Padding, Paragraph, Tabs},
};

use crossterm::event::{self, KeyCode, KeyEvent};

use tokio::sync::mpsc;
use tokio::task;

use crate::{
    chat::{Chat, ChatState},
    chat_message::ChatMessage,
    commands::Command,
    frontend,
};

use super::{chat_mode, logs_mode, UIEvent, UserInputCommand};

const TICK_RATE: u64 = 250;
const HEADER: &str = include_str!("ascii_logo");

/// Handles user and TUI interaction
pub struct App<'a> {
    // /// The chat input
    // pub input: String,
    pub text_input: TextArea<'a>,

    /// All known chats
    pub chats: Vec<Chat>,

    /// UUID of the current chat
    pub current_chat_uuid: uuid::Uuid,

    /// Holds the sender of UI events for later cloning if needed
    pub ui_tx: mpsc::UnboundedSender<UIEvent>,

    /// Receives UI events (key presses, commands, etc)
    pub ui_rx: mpsc::UnboundedReceiver<UIEvent>,

    /// Sends commands to the backend
    pub command_tx: Option<mpsc::UnboundedSender<Command>>,

    /// Mode the app is in, manages the which layout is rendered and if it should quit
    pub mode: AppMode,

    /// Tracks the current selected state in the UI
    pub chats_state: ListState,

    /// Tab names
    pub tab_names: Vec<&'static str>,

    /// Index of selected tab
    pub selected_tab: usize,

    /// States when viewing logs
    pub log_state: TuiWidgetState,

    /// Commands that relate to boot, and not a chat
    pub boot_uuid: Uuid,

    /// Skip indexing on boot
    pub skip_indexing: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum AppMode {
    #[default]
    Chat,
    Logs,
    Quit,
}

impl AppMode {
    fn on_key(self, app: &mut App, key: KeyEvent) {
        match self {
            AppMode::Chat => chat_mode::on_key(app, key),
            AppMode::Logs => logs_mode::on_key(app, key),
            AppMode::Quit => (),
        }
    }

    fn ui(self, f: &mut ratatui::Frame, area: Rect, app: &mut App) {
        match self {
            AppMode::Chat => chat_mode::ui(f, area, app),
            AppMode::Logs => logs_mode::ui(f, area, app),
            AppMode::Quit => (),
        }
    }

    fn tab_index(self) -> Option<usize> {
        match self {
            AppMode::Chat => Some(0),
            AppMode::Logs => Some(1),
            AppMode::Quit => None,
        }
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(AppMode::Chat),
            1 => Some(AppMode::Logs),
            _ => None,
        }
    }
}

impl Default for App<'_> {
    fn default() -> Self {
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();

        let chat = Chat {
            name: "Chat #1".to_string(),
            ..Chat::default()
        };

        Self {
            skip_indexing: false,
            text_input: new_text_area(),
            current_chat_uuid: chat.uuid,
            chats: vec![chat],
            ui_tx,
            ui_rx,
            command_tx: None,
            mode: AppMode::default(),
            chats_state: ListState::default().with_selected(Some(0)),
            tab_names: vec!["[F1] Chats", "[F2] Logs"],
            log_state: TuiWidgetState::new()
                .set_default_display_level(log::LevelFilter::Off)
                .set_level_for_target("kwaak", log::LevelFilter::Info)
                .set_level_for_target("swiftide", log::LevelFilter::Info),
            selected_tab: 0,
            boot_uuid: Uuid::new_v4(),
        }
    }
}

fn new_text_area() -> TextArea<'static> {
    let mut text_area = TextArea::default();

    text_area.set_placeholder_text("Send a message to an agent ...");
    text_area.set_placeholder_style(Style::default().fg(Color::Gray));
    text_area.set_cursor_line_style(Style::reset());

    text_area
}

impl App<'_> {
    async fn recv_messages(&mut self) -> Option<UIEvent> {
        self.ui_rx.recv().await
    }

    #[allow(clippy::unused_self)]
    pub fn supported_commands(&self) -> Vec<UserInputCommand> {
        UserInputCommand::iter().collect()
    }

    pub fn send_ui_event(&self, msg: impl Into<UIEvent>) {
        let event = msg.into();
        tracing::debug!("Sending ui event {event}");
        if let Err(err) = self.ui_tx.send(event) {
            tracing::error!("Failed to send ui event {err}");
        }
    }

    pub fn reset_text_input(&mut self) {
        self.text_input = new_text_area();
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers == crossterm::event::KeyModifiers::CONTROL
            && key.code == KeyCode::Char('q')
        {
            tracing::warn!("Ctrl-Q pressed, quitting");
            return self.send_ui_event(UIEvent::Quit);
        }

        if let KeyCode::F(index) = key.code {
            let index = index - 1;
            if let Some(mode) = AppMode::from_index(index as usize) {
                return self.change_mode(mode);
            }
        }

        self.mode.on_key(self, key);
    }

    #[tracing::instrument(skip(self))]
    pub fn dispatch_command(&mut self, cmd: &Command) {
        if let Some(chat) = self.current_chat_mut() {
            chat.transition(ChatState::Loading);
        }

        self.command_tx
            .as_ref()
            .expect("Command tx not set")
            .send(cmd.clone())
            .expect("Failed to dispatch command");
    }

    fn add_chat_message(&mut self, message: ChatMessage) {
        if message.uuid() == Some(self.boot_uuid) {
            return;
        }
        if let Some(chat) = self.find_chat_mut(message.uuid().unwrap_or(self.current_chat_uuid)) {
            chat.add_message(message);
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        let handle = task::spawn(poll_ui_events(self.ui_tx.clone()));

        let mut has_indexed_on_boot = false;
        let mut splash = frontend::splash::Splash::default();

        if self.skip_indexing {
            has_indexed_on_boot = true;
        } else {
            self.dispatch_command(&Command::IndexRepository {
                uuid: self.boot_uuid,
            });
        }

        loop {
            // Draw the UI
            terminal.draw(|f| {
                if has_indexed_on_boot && splash.is_rendered() {
                    let base_area = self.draw_base_ui(f);

                    self.mode.ui(f, base_area, self);
                } else {
                    splash.render(f);
                }
            })?;
            if !splash.is_rendered() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            if self.mode == AppMode::Quit {
                break;
            }

            // Handle events
            if let Some(event) = self.recv_messages().await {
                if !matches!(event, UIEvent::Tick | UIEvent::Input(_)) {
                    tracing::debug!("Received ui event: {:?}", event);
                }
                match event {
                    UIEvent::Input(key) => {
                        self.on_key(key);
                    }
                    UIEvent::Tick => {
                        // Handle periodic tasks if necessary
                    }
                    UIEvent::CommandDone(uuid) => {
                        if uuid == self.boot_uuid {
                            has_indexed_on_boot = true;
                            self.current_chat_mut()
                                .expect("Boot uuid should always be present")
                                .transition(ChatState::Ready);
                        } else if let Some(chat) = self.find_chat_mut(uuid) {
                            chat.transition(ChatState::Ready);
                        }
                    }
                    UIEvent::ActivityUpdate(uuid, activity) => {
                        if uuid == self.boot_uuid {
                            splash.set_message(activity);
                        } else if let Some(chat) = self.find_chat_mut(uuid) {
                            chat.transition(ChatState::LoadingWithMessage(activity));
                        }
                    }
                    UIEvent::ChatMessage(message) => {
                        self.add_chat_message(message);
                    }
                    UIEvent::NewChat => {
                        self.add_chat(Chat::default());
                    }
                    UIEvent::RenameChat(uuid, name) => {
                        if let Some(chat) = self.find_chat_mut(uuid) {
                            chat.name = name;
                        };
                    }
                    UIEvent::NextChat => self.next_chat(),
                    UIEvent::ChangeMode(mode) => self.change_mode(mode),
                    UIEvent::Quit => {
                        tracing::warn!("UI received quit event, quitting");

                        self.dispatch_command(&Command::Quit {
                            uuid: self.current_chat_uuid,
                        });
                        self.change_mode(AppMode::Quit);
                    }
                    UIEvent::DeleteChat => {
                        let uuid = self.current_chat_uuid;
                        self.dispatch_command(&Command::StopAgent { uuid });
                        // Remove the chat with the given UUID
                        self.chats.retain(|chat| chat.uuid != uuid);

                        if self.chats.is_empty() {
                            self.add_chat(Chat::default());
                            self.chats_state.select(Some(0));
                            self.add_chat_message(
                                ChatMessage::new_system(
                                    "Nice, you managed to delete the last chat!",
                                )
                                .build(),
                            );
                        } else {
                            self.next_chat();
                        }
                    }
                }
            }
        }

        tracing::warn!("Quitting frontend");

        handle.abort();

        Ok(())
    }

    fn find_chat_mut(&mut self, uuid: Uuid) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|chat| chat.uuid == uuid)
    }

    fn find_chat(&self, uuid: Uuid) -> Option<&Chat> {
        self.chats.iter().find(|chat| chat.uuid == uuid)
    }

    pub(crate) fn current_chat(&self) -> Option<&Chat> {
        self.find_chat(self.current_chat_uuid)
    }

    pub(crate) fn current_chat_mut(&mut self) -> Option<&mut Chat> {
        self.find_chat_mut(self.current_chat_uuid)
    }

    fn add_chat(&mut self, mut new_chat: Chat) {
        new_chat.name = format!("Chat #{}", self.chats.len() + 1);

        self.current_chat_uuid = new_chat.uuid;
        self.chats.push(new_chat);
        self.chats_state.select_last();
    }

    fn next_chat(&mut self) {
        #[allow(clippy::skip_while_next)]
        let Some(next_idx) = self
            .chats
            .iter()
            .position(|chat| chat.uuid == self.current_chat_uuid)
            .map(|idx| idx + 1)
        else {
            assert!(
                !cfg!(debug_assertions),
                "Could not find current chat in chats"
            );

            return;
        };

        if let Some(chat) = self.chats.get(next_idx) {
            self.chats_state.select(Some(next_idx));
            self.current_chat_uuid = chat.uuid;
        } else {
            self.chats_state.select(Some(0));
            self.current_chat_uuid = self.chats[0].uuid;
        }
    }

    fn draw_base_ui(&self, f: &mut Frame) -> Rect {
        let [top_area, main_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).areas(f.area());

        // Hardcoded tabs length for now to right align
        let [header_area, tabs_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(24)]).areas(top_area);

        Tabs::new(self.tab_names.iter().copied())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .padding(Padding::top(top_area.height - 2)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .select(self.selected_tab)
            .render(tabs_area, f.buffer_mut());

        Paragraph::new(HEADER)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .padding(Padding::new(1, 0, 1, 0)),
            )
            .render(header_area, f.buffer_mut());

        main_area
    }

    fn change_mode(&mut self, mode: AppMode) {
        self.mode = mode;
        if let Some(tab_index) = mode.tab_index() {
            self.selected_tab = tab_index;
        }
    }
}

#[allow(clippy::unused_async)]
async fn poll_ui_events(ui_tx: mpsc::UnboundedSender<UIEvent>) -> Result<()> {
    loop {
        // Poll for input events
        if event::poll(Duration::from_millis(TICK_RATE))? {
            if let crossterm::event::Event::Key(key) = event::read()? {
                let _ = ui_tx.send(UIEvent::Input(key));
            }
        }
        // Send a tick event, ignore if the receiver is gone
        let _ = ui_tx.send(UIEvent::Tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_or_first_chat() {
        let mut app = App::default();
        let chat = Chat::default();
        let first_uuid = app.current_chat_uuid;
        let second_uuid = chat.uuid;

        // Starts with first
        assert_eq!(app.current_chat_uuid, first_uuid);

        app.add_chat(chat);
        assert_eq!(app.current_chat_uuid, second_uuid);

        app.next_chat();

        assert_eq!(app.current_chat_uuid, first_uuid);

        app.next_chat();
        assert_eq!(app.current_chat_uuid, second_uuid);
    }

    #[test]
    fn test_app_mode_from_index() {
        assert_eq!(AppMode::from_index(0), Some(AppMode::Chat));
        assert_eq!(AppMode::from_index(1), Some(AppMode::Logs));
        assert_eq!(AppMode::from_index(2), None);
    }

    #[test]
    fn test_app_mode_tab_index() {
        assert_eq!(AppMode::Chat.tab_index(), Some(0));
        assert_eq!(AppMode::Logs.tab_index(), Some(1));
        assert_eq!(AppMode::Quit.tab_index(), None);
    }

    #[test]
    fn test_change_mode() {
        let mut app = App::default();
        assert_eq!(app.mode, AppMode::Chat);
        assert_eq!(app.selected_tab, 0);

        app.change_mode(AppMode::Logs);
        assert_eq!(app.mode, AppMode::Logs);
        assert_eq!(app.selected_tab, 1);

        app.change_mode(AppMode::Quit);
        assert_eq!(app.mode, AppMode::Quit);
        assert_eq!(app.selected_tab, 1);
    }
}
