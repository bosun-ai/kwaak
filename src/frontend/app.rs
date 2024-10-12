use anyhow::Result;
use std::io;
use std::time::Duration;
use strum::IntoEnumIterator as _;
use uuid::Uuid;

use ratatui::{
    widgets::{ListState, ScrollbarState},
    Terminal,
};

use crossterm::event::{self, KeyCode, KeyEvent};

use tokio::sync::mpsc;
use tokio::task;

use crate::{
    chat::Chat,
    chat_message::{ChatMessage, ChatMessageBuilder},
    commands::Command,
};

use super::{chat_mode, ui, UIEvent, UserInputCommand};

const TICK_RATE: u64 = 250;

/// Handles user and TUI interaction
#[derive(Debug)]
pub struct App {
    /// The chat input
    pub input: String,

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

    // Scroll chat
    pub vertical_scroll_state: ScrollbarState,
    pub vertical_scroll: u16,

    // Tracks the current selected state in the UI
    pub chats_state: ListState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum AppMode {
    #[default]
    Chat,
    Logs,
    Quit,
}

impl Default for App {
    fn default() -> Self {
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();

        let mut chat = Chat::default();
        chat.name = "Chat #1".to_string();

        Self {
            input: String::new(),
            current_chat: chat.uuid,
            chats: vec![chat],
            ui_tx,
            ui_rx,
            command_tx: None,
            mode: AppMode::default(),
            vertical_scroll_state: ScrollbarState::default(),
            vertical_scroll: 0,
            chats_state: ListState::default().with_selected(Some(0)),
        }
    }
}

impl App {
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
            self.mode = AppMode::Quit;
            return;
        }

        match self.mode {
            AppMode::Chat => chat_mode::on_key(self, key),
            _ => {}
        }
    }

    pub fn dispatch_command(&self, cmd: &Command) {
        self.command_tx
            .as_ref()
            .expect("Command tx not set")
            .send(cmd.clone())
            .expect("Failed to dispatch command");
    }

    fn add_chat_message(&mut self, message: ChatMessage) {
        let chat = self.find_chat_mut(message.uuid().unwrap_or_else(|| self.current_chat));
        chat.add_message(message);
    }

    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> io::Result<()> {
        let handle = task::spawn(poll_ui_events(self.ui_tx.clone()));

        loop {
            // Draw the UI
            terminal.draw(|f| ui::ui(f, self))?;

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
                    UIEvent::Command(cmd) => match cmd {
                        Command::Quit { .. } => {
                            // If the backends tells us to quit we also do it
                            self.mode = AppMode::Quit;
                        }

                        _ => {
                            tracing::warn!("Unhandled command: {:?}", cmd);
                        }
                    },
                    UIEvent::ChatMessage(message) => {
                        self.add_chat_message(message);
                    }
                    UIEvent::NewChat => {
                        self.add_chat(Chat::default());
                    }
                    UIEvent::NextChat => self.set_next_chat(),
                }
            }

            if self.mode == AppMode::Quit {
                break;
            }
        }

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

    fn add_chat(&mut self, mut new_chat: Chat) {
        new_chat.name = format!("Chat #{}", self.chats.len() + 1);

        self.current_chat = new_chat.uuid;
        self.chats.push(new_chat);
        self.chats_state.select_last();
    }

    fn set_next_chat(&mut self) {
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

        app.set_next_chat();
        dbg!(app.current_chat);
        dbg!(app.chats.iter().map(|chat| chat.uuid).collect::<Vec<_>>());

        assert_eq!(app.current_chat, first_uuid);

        app.set_next_chat();
        assert_eq!(app.current_chat, second_uuid);
    }
}
