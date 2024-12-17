use anyhow::Result;
use std::io;
use std::time::Duration;
use strum::IntoEnumIterator as _;
use tui_logger::TuiWidgetState;
use tui_textarea::TextArea;
use uuid::Uuid;

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, ListState, Padding, Tabs},
};

use crossterm::event::{self, KeyCode, KeyEvent};

use tokio::sync::mpsc;
use tokio::task;

use crate::{
    chat::{Chat, ChatState},
    chat_message::ChatMessage,
    commands::Command,
};

use super::{chat_mode, logs_mode, UIEvent, UserInputCommand};

const TICK_RATE: u64 = 250;

/// Handles user and TUI interaction
pub struct App<'a> {
    // /// The chat input
    // pub input: String,
    pub text_input: TextArea<'a>,

    /// All known chats
    pub chats: Vec<Chat>,

    /// UUID of the current chat
    pub current_chat: uuid::Uuid,

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
            text_input: TextArea::default(),
            current_chat: chat.uuid,
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
        }
    }
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

    fn on_key(&mut self, key: KeyEvent) {
        // Always quit on ctrl c
        if key.modifiers == crossterm::event::KeyModifiers::CONTROL
            && key.code == KeyCode::Char('c')
        {
            tracing::warn!("Ctrl-C pressed, quitting");
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
        self.current_chat_mut().transition(ChatState::Loading);

        self.command_tx
            .as_ref()
            .expect("Command tx not set")
            .send(cmd.clone())
            .expect("Failed to dispatch command");
    }

    fn add_chat_message(&mut self, message: ChatMessage) {
        let chat = self.find_chat_mut(message.uuid().unwrap_or(self.current_chat));
        chat.add_message(message);
    }

    #[tracing::instrument(skip_all)]
    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> io::Result<()> {
        let handle = task::spawn(poll_ui_events(self.ui_tx.clone()));

        loop {
            // Draw the UI
            terminal.draw(|f| {
                let base_area = self.draw_base_ui(f);

                self.mode.ui(f, base_area, self);
            })?;

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
                        self.find_chat_mut(uuid).transition(ChatState::Ready);
                    }
                    UIEvent::AgentActivity(uuid, activity) => {
                        self.find_chat_mut(uuid)
                            .transition(ChatState::LoadingWithMessage(activity));
                    }
                    UIEvent::ChatMessage(message) => {
                        self.add_chat_message(message);
                    }
                    UIEvent::NewChat => {
                        self.add_chat(Chat::default());
                    }
                    UIEvent::NextChat => self.next_chat(),
                    UIEvent::ChangeMode(mode) => self.change_mode(mode),
                    UIEvent::Quit => {
                        tracing::warn!("UI received quit event, quitting");

                        self.dispatch_command(&Command::Quit {
                            uuid: self.current_chat,
                        });
                        self.change_mode(AppMode::Quit);
                    }
                }
            }
        }

        tracing::warn!("Quitting frontend");

        handle.abort();

        Ok(())
    }

    fn find_chat_mut(&mut self, uuid: Uuid) -> &mut Chat {
        self.chats
            .iter_mut()
            .find(|chat| chat.uuid == uuid)
            .unwrap_or_else(|| panic!("Could not find chat for {uuid}"))
    }

    fn find_chat(&self, uuid: Uuid) -> &Chat {
        self.chats
            .iter()
            .find(|chat| chat.uuid == uuid)
            .unwrap_or_else(|| panic!("Could not find chat for {uuid}"))
    }

    pub(crate) fn current_chat(&self) -> &Chat {
        self.find_chat(self.current_chat)
    }

    pub(crate) fn current_chat_mut(&mut self) -> &mut Chat {
        self.find_chat_mut(self.current_chat)
    }

    fn add_chat(&mut self, mut new_chat: Chat) {
        new_chat.name = format!("Chat #{}", self.chats.len() + 1);

        self.current_chat = new_chat.uuid;
        self.chats.push(new_chat);
        self.chats_state.select_last();
    }

    fn next_chat(&mut self) {
        #[allow(clippy::skip_while_next)]
        let Some(next_idx) = self
            .chats
            .iter()
            .position(|chat| chat.uuid == self.current_chat)
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
            self.current_chat = chat.uuid;
        } else {
            self.chats_state.select(Some(0));
            self.current_chat = self.chats[0].uuid;
        }
    }

    fn draw_base_ui(&self, f: &mut Frame) -> Rect {
        let [tabs_area, main_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(f.area());

        Tabs::new(self.tab_names.iter().copied())
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .padding(Padding::top(1)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .select(self.selected_tab)
            .render(tabs_area, f.buffer_mut());

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
        let first_uuid = app.current_chat;
        let second_uuid = chat.uuid;

        // Starts with first
        assert_eq!(app.current_chat, first_uuid);

        app.add_chat(chat);
        assert_eq!(app.current_chat, second_uuid);

        app.next_chat();

        assert_eq!(app.current_chat, first_uuid);

        app.next_chat();
        assert_eq!(app.current_chat, second_uuid);
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
