use anyhow::Result;
use std::io;
use std::time::Duration;
use strum::IntoEnumIterator as _;

use ratatui::{widgets::ScrollbarState, Terminal};

use crossterm::event::{self, KeyCode, KeyEvent};

use tokio::sync::mpsc;
use tokio::task;

use crate::{chat_message::ChatMessage, commands::Command};

use super::{ui, UIEvent};

const TICK_RATE: u64 = 250;

/// Handles user and TUI interaction
pub struct App {
    pub input: String,
    pub messages: Vec<ChatMessage>,
    pub ui_tx: mpsc::UnboundedSender<UIEvent>,
    pub ui_rx: mpsc::UnboundedReceiver<UIEvent>,
    pub command_tx: Option<mpsc::UnboundedSender<Command>>,
    pub should_quit: bool,

    // Scroll chat
    pub vertical_scroll_state: ScrollbarState,
    pub vertical_scroll: u16,
}

impl Default for App {
    fn default() -> Self {
        let (ui_tx, ui_rx) = mpsc::unbounded_channel();

        Self {
            input: String::new(),
            messages: Vec::new(),
            ui_tx,
            ui_rx,
            command_tx: None,
            should_quit: false,
            vertical_scroll_state: ScrollbarState::default(),
            vertical_scroll: 0,
        }
    }
}

impl App {
    async fn recv_messages(&mut self) -> Option<UIEvent> {
        self.ui_rx.recv().await
    }

    pub fn supported_commands(&self) -> Vec<Command> {
        Command::iter().collect()
    }

    fn send_ui_event(&self, msg: impl Into<UIEvent>) -> Result<()> {
        self.ui_tx.send(msg.into())?;

        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) {
        // Always quit on ctrl c
        if key.modifiers == crossterm::event::KeyModifiers::CONTROL
            && key.code == KeyCode::Char('c')
        {
            self.should_quit = true;
            return;
        }

        match key.code {
            KeyCode::Down => {
                self.vertical_scroll = self.vertical_scroll.saturating_add(1);
                self.vertical_scroll_state = self
                    .vertical_scroll_state
                    .position(self.vertical_scroll as usize);
            }
            KeyCode::Up => {
                self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
                self.vertical_scroll_state = self
                    .vertical_scroll_state
                    .position(self.vertical_scroll as usize);
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let message = if self.input.starts_with("/") {
                        if let Ok(cmd) = Command::parse(&self.input) {
                            // Send it to the handler
                            self.dispatch_command(&cmd);

                            // Display the command as a message
                            ChatMessage::new_command(cmd)
                        } else {
                            ChatMessage::new_system("Unknown command")
                        }
                    } else {
                        // Currently just dispatch a user message command and answer the query
                        // Later, perhaps maint a 'chat', add message to that chat, and then send
                        // the whole thing
                        self.dispatch_command(&Command::Chat(self.input.clone()));

                        ChatMessage::new_user(&self.input)
                    };

                    let _ = self.send_ui_event(message);

                    self.input.clear();
                }
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn dispatch_command(&self, cmd: &Command) {
        self.command_tx
            .as_ref()
            .expect("Command tx not set")
            .send(cmd.clone())
            .expect("Failed to dispatch command");
    }

    fn add_chat_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    pub async fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> io::Result<()> {
        // Spawn a blocking task to read input events

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
                        Command::Quit => {
                            self.should_quit = true;
                        }

                        _ => {
                            tracing::warn!("Unhandled command: {:?}", cmd);
                        }
                    },
                    UIEvent::ChatMessage(message) => {
                        self.add_chat_message(message);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        handle.abort();

        Ok(())
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
        // let _ = ui_tx.send(UIEvent::Tick);
    }
}