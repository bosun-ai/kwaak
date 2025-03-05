use anyhow::Result;
use std::{path::PathBuf, time::Duration};
use strum::IntoEnumIterator as _;
use tui_logger::TuiWidgetState;
use tui_textarea::TextArea;
use update_informer::{registry, Check};
use uuid::Uuid;

use ratatui::{
    prelude::*,
    widgets::{Block, ListState, Padding, Paragraph, Tabs},
};

use crossterm::event::{self, KeyCode, KeyEvent};

use tokio::sync::mpsc;
use tokio::task;

use crate::{
    chat::{Chat, ChatState},
    chat_message::ChatMessage,
    commands::{Command, CommandEvent},
    config::UIConfig,
    frontend::actions,
};

use super::{
    app_command_responder::AppCommandResponder, chat_mode, logs_mode, splash, ui_event::UIEvent,
    ui_input_command::UserInputCommand,
};

const TICK_RATE: u64 = 250;
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Handles user and TUI interaction
pub struct App<'a> {
    pub splash: splash::Splash<'a>,
    pub has_indexed_on_boot: bool,
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
    pub command_tx: Option<mpsc::UnboundedSender<CommandEvent>>,

    /// Responds to commands from the backend
    /// And maps them to ui events
    pub command_responder: AppCommandResponder,

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

    /// Override the working directory if it is not "."
    pub workdir: PathBuf,

    /// Hack to get line wrapping on input into the textarea
    pub input_width: Option<u16>,

    /// Max lines we can render in the chat messages
    pub chat_messages_max_lines: u16,

    /// User configuration for the UI
    pub ui_config: UIConfig,

    /// Informs the user if there is an update available
    pub update_available: Option<update_informer::Version>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum AppMode {
    #[default]
    Chat,
    Logs,
    Quit,
}

impl AppMode {
    fn on_key(self, app: &mut App, key: &KeyEvent) {
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

        let command_responder = AppCommandResponder::spawn_for(ui_tx.clone());

        // Actual api checks are done only once every 24 hours, with the first happening after 24
        let update_available = update_informer::new(registry::Crates, "kwaak", VERSION)
            // .interval(Duration::ZERO) // Uncomment me to test the update informer
            .check_version()
            .ok()
            .flatten();

        Self {
            workdir: ".".into(),
            splash: splash::Splash::default(),
            has_indexed_on_boot: false,
            skip_indexing: false,
            text_input: new_text_area(),
            current_chat_uuid: chat.uuid,
            chats: vec![chat],
            command_responder,
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
            input_width: None,
            chat_messages_max_lines: 0,
            ui_config: UIConfig::default(),
            update_available,
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
    pub async fn recv_messages(&mut self) -> Option<UIEvent> {
        self.ui_rx.recv().await
    }

    #[allow(clippy::unused_self)]
    pub fn supported_commands(&self) -> Vec<UserInputCommand> {
        UserInputCommand::iter().collect()
    }

    pub fn send_ui_event(&self, msg: impl Into<UIEvent>) {
        let event = msg.into();
        if let Err(err) = self.ui_tx.send(event) {
            tracing::error!("Failed to send ui event {err}");
        }
    }

    pub fn reset_text_input(&mut self) {
        self.text_input = new_text_area();
    }

    fn on_key(&mut self, key: &KeyEvent) {
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
    pub fn dispatch_command(&mut self, uuid: Uuid, cmd: Command) {
        if let Some(chat) = self.current_chat_mut() {
            chat.transition(ChatState::Loading);
        }

        let event = CommandEvent::builder()
            .command(cmd)
            .uuid(uuid)
            .responder(self.command_responder.for_chat_id(uuid))
            .build()
            .expect("Infallible; Failed to build command event");

        self.dispatch_command_event(event);
    }

    /// Dispatch a command event to the backend
    ///
    /// # Panics
    ///
    /// If the command dispatcher is not set or the handler is disconnected
    pub fn dispatch_command_event(&mut self, event: CommandEvent) {
        self.command_tx
            .as_ref()
            .expect("Command tx not set")
            .send(event)
            .expect("Failed to dispatch command");
    }

    pub fn add_chat_message(&mut self, chat_id: Uuid, message: impl Into<ChatMessage>) {
        let message = message.into();
        if chat_id == self.boot_uuid {
            return;
        }
        if let Some(chat) = self.find_chat_mut(chat_id) {
            chat.add_message(message);
        } else {
            tracing::error!("Could not find chat with id {chat_id}");
        }
    }

    #[must_use]
    /// Overrides the working directory
    ///
    /// Any actions that use ie system commands use this directory
    pub fn with_workdir(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.workdir = workdir.into();
        self
    }

    #[tracing::instrument(skip_all)]
    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        let handle = task::spawn(poll_ui_events(self.ui_tx.clone()));

        if self.skip_indexing {
            self.has_indexed_on_boot = true;
        } else {
            self.dispatch_command(self.boot_uuid, Command::IndexRepository);
        }

        loop {
            // Draw the UI
            terminal.draw(|f| {
                if self.has_indexed_on_boot {
                    self.render_tui(f);
                } else {
                    self.splash.render(f);
                }
            })?;

            if let Some(event) = self.recv_messages().await {
                self.handle_single_event(&event).await;
            }
            if !self.splash.is_rendered() && !self.has_indexed_on_boot {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            if self.mode == AppMode::Quit {
                break;
            }

            // Handle events
        }

        tracing::warn!("Quitting frontend");

        handle.abort();

        Ok(())
    }

    pub fn render_tui(&mut self, f: &mut Frame) {
        let base_area = self.draw_base_ui(f);

        self.mode.ui(f, base_area, self);
    }

    /// Wait for and handle a single ui event
    ///
    /// # Panics
    ///
    /// Panics if after boot completed, it cannot find the initial chat
    #[allow(clippy::too_many_lines)]
    pub async fn handle_single_event(&mut self, event: &UIEvent) {
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
                if *uuid == self.boot_uuid {
                    self.has_indexed_on_boot = true;
                    self.current_chat_mut()
                        .expect("Boot uuid should always be present")
                        .transition(ChatState::Ready);
                } else if let Some(chat) = self.find_chat_mut(*uuid) {
                    chat.transition(ChatState::Ready);
                }
            }
            UIEvent::ActivityUpdate(uuid, activity) => {
                if *uuid == self.boot_uuid {
                    self.splash.set_message(activity.to_string());
                } else if let Some(chat) = self.find_chat_mut(*uuid) {
                    chat.transition(ChatState::LoadingWithMessage(activity.to_string()));
                }
            }
            UIEvent::ChatMessage(uuid, message) => {
                self.add_chat_message(*uuid, message.clone());

                if let Some(chat) = self.find_chat_mut(*uuid) {
                    if chat.auto_tail {
                        self.send_ui_event(UIEvent::ScrollEnd);
                    }
                }
            }
            UIEvent::NewChat => {
                let chat = Chat {
                    // add the repo from the current chat to the new chat
                    // TODO eventually this should be updated for more complex multi-repo setups
                    repository: self
                        .current_chat()
                        .map(|c| c.repository.clone())
                        .unwrap_or_default(),
                    ..Default::default()
                };
                self.add_chat(chat);
            }
            UIEvent::RenameChat(uuid, name) => {
                if let Some(chat) = self.find_chat_mut(*uuid) {
                    chat.name = name.to_string();
                };
            }
            UIEvent::RenameBranch(uuid, branch_name) => {
                if let Some(chat) = self.find_chat_mut(*uuid) {
                    chat.branch_name = Some(branch_name.to_string());
                };
            }
            UIEvent::NextChat => self.next_chat(),
            UIEvent::ChangeMode(mode) => self.change_mode(*mode),
            UIEvent::Quit => {
                tracing::warn!("UI received quit event, quitting");

                self.dispatch_command(self.current_chat_uuid, Command::Quit);
                self.change_mode(AppMode::Quit);
            }
            UIEvent::DeleteChat => actions::delete_chat(self),
            UIEvent::CopyLastMessage => actions::copy_last_message(self),
            UIEvent::DiffPull => actions::diff_pull(self).await,
            UIEvent::DiffShow => actions::diff_show(self).await,
            UIEvent::UserInputCommand(uuid, cmd) => {
                if let Some(cmd) = cmd.to_command() {
                    self.dispatch_command(*uuid, cmd);
                } else if let Some(event) = cmd.to_ui_event(*uuid) {
                    self.send_ui_event(event);
                } else {
                    tracing::error!(
                        "Could not convert ui command to backend command nor ui event {cmd}"
                    );
                    self.add_chat_message(
                        self.current_chat_uuid,
                        ChatMessage::new_system("Unknown command"),
                    );
                }
            }
            UIEvent::ScrollUp => actions::scroll_up(self),
            UIEvent::ScrollDown => actions::scroll_down(self),
            UIEvent::ScrollEnd => actions::scroll_end(self),
            UIEvent::Help => actions::help(self),
            UIEvent::GithubFixIssue(uuid, number) => actions::github_issue(self, *number, *uuid).await,
        }
    }

    #[cfg(debug_assertions)]
    /// Used for testing so we can do something and wait for it to complete
    ///
    /// *will* hang until event is encountered
    pub async fn handle_events_until(
        &mut self,
        stop_fn: impl Fn(&UIEvent) -> bool,
    ) -> Option<UIEvent> {
        while let Some(event) = self.recv_messages().await {
            self.handle_single_event(&event).await;
            if stop_fn(&event) {
                return Some(event);
            }
            if self.mode == AppMode::Quit {
                return Some(event);
            }
        }
        None
    }

    pub fn find_chat_mut(&mut self, uuid: Uuid) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|chat| chat.uuid == uuid)
    }

    pub fn find_chat(&self, uuid: Uuid) -> Option<&Chat> {
        self.chats.iter().find(|chat| chat.uuid == uuid)
    }

    pub fn current_chat(&self) -> Option<&Chat> {
        self.find_chat(self.current_chat_uuid)
    }

    pub fn current_chat_mut(&mut self) -> Option<&mut Chat> {
        self.find_chat_mut(self.current_chat_uuid)
    }

    pub fn add_chat(&mut self, mut new_chat: Chat) {
        new_chat.name = format!("Chat #{}", self.chats.len() + 1);

        self.current_chat_uuid = new_chat.uuid;
        self.chats.push(new_chat);
        self.chats_state.select_last();
    }

    pub fn next_chat(&mut self) {
        #[allow(clippy::skip_while_next)]
        let Some(next_idx) = self
            .chats
            .iter()
            .position(|chat| chat.uuid == self.current_chat_uuid)
            .map(|idx| idx + 1)
        else {
            let Some(chat) = self.chats.first() else {
                debug_assert!(
                    false,
                    "No chats in app found when selecting next app, this should never happen"
                );
                tracing::error!(
                    "No chats in app found when selecting next app, this should never happen"
                );

                self.add_chat(Chat::default());
                return;
            };

            let uuid = chat.uuid;
            self.current_chat_uuid = uuid;
            self.chats_state.select(Some(0));
            return;
        };

        let chat = if let Some(chat) = self.chats.get(next_idx) {
            self.chats_state.select(Some(next_idx));
            chat
        } else {
            self.chats_state.select(Some(0));
            &self.chats[0]
        };
        self.current_chat_uuid = chat.uuid;
    }

    fn draw_base_ui(&self, f: &mut Frame) -> Rect {
        if self.ui_config.hide_header {
            return f.area();
        }

        let [top_area, main_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(f.area());

        // Hardcoded tabs length for now to right align
        let [header_area, tabs_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(24)]).areas(top_area);

        Tabs::new(self.tab_names.iter().copied())
            .block(Block::default().padding(Padding::new(0, 1, 1, 0)))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .select(self.selected_tab)
            .render(tabs_area, f.buffer_mut());

        let duck = Span::styled("󰇥", Style::default().fg(Color::Yellow));
        let header = Span::styled("  kwaak  ", Style::default().bold());

        // If the version is outdated, show it in red. The version is checked every 24 hours.
        // update_informer caches the version in the cache folder.
        let mut version = Span::styled(VERSION, Style::default().dim());
        if let Some(new_version) = &self.update_available {
            version = Span::styled(
                format!("new version available: {new_version}"),
                Style::default().fg(Color::Red).dim(),
            );
        }

        Paragraph::new(duck + header + version)
            .block(Block::default().padding(Padding::new(1, 0, 1, 0)))
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
        if ui_tx.send(UIEvent::Tick).is_err() {
            break Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_last_or_first_chat() {
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

    #[tokio::test]
    async fn test_app_mode_from_index() {
        assert_eq!(AppMode::from_index(0), Some(AppMode::Chat));
        assert_eq!(AppMode::from_index(1), Some(AppMode::Logs));
        assert_eq!(AppMode::from_index(2), None);
    }

    #[tokio::test]
    async fn test_app_mode_tab_index() {
        assert_eq!(AppMode::Chat.tab_index(), Some(0));
        assert_eq!(AppMode::Logs.tab_index(), Some(1));
        assert_eq!(AppMode::Quit.tab_index(), None);
    }

    #[tokio::test]
    async fn test_change_mode() {
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

    #[tokio::test]
    async fn test_add_chat() {
        let mut app = App::default();
        let initial_chat_count = app.chats.len();

        app.add_chat(Chat::default());

        assert_eq!(app.chats.len(), initial_chat_count + 1);
        assert_eq!(
            app.chats.last().unwrap().name,
            format!("Chat #{}", initial_chat_count + 1)
        );
    }

    #[tokio::test]
    async fn test_next_chat() {
        let mut app = App::default();
        let first_uuid = app.current_chat_uuid;

        app.add_chat(Chat::default());
        let second_uuid = app.current_chat_uuid;

        app.next_chat();
        assert_eq!(app.current_chat_uuid, first_uuid);

        app.next_chat();
        assert_eq!(app.current_chat_uuid, second_uuid);
    }

    #[tokio::test]
    async fn test_find_chat() {
        let mut app = App::default();
        let chat = Chat::default();
        let uuid = chat.uuid;

        app.add_chat(chat);

        assert!(app.find_chat(uuid).is_some());
        assert!(app.find_chat(Uuid::new_v4()).is_none());
    }

    #[tokio::test]
    async fn test_find_chat_mut() {
        let mut app = App::default();
        let chat = Chat::default();
        let uuid = chat.uuid;

        app.add_chat(chat);

        assert!(app.find_chat_mut(uuid).is_some());
        assert!(app.find_chat_mut(Uuid::new_v4()).is_none());
    }

    #[tokio::test]
    async fn test_current_chat() {
        let app = App::default();
        assert!(app.current_chat().is_some());
    }

    #[tokio::test]
    async fn test_current_chat_mut() {
        let mut app = App::default();
        assert!(app.current_chat_mut().is_some());
    }

    #[tokio::test]
    async fn test_add_chat_message() {
        let mut app = App::default();
        let message = ChatMessage::new_system("Test message");

        app.add_chat_message(app.current_chat_uuid, message.clone());

        let chat = app.current_chat().unwrap();
        assert!(chat.messages.contains(&message));
    }
}
