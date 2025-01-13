use uuid::Uuid;

/// Commands are the main way to interact with the backend
///
/// By default all commands can be triggered from the ui like `/<command>`
#[derive(
    Debug,
    // PartialEq,
    // Eq,
    // strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::IntoStaticStr,
    strum_macros::EnumIs,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum Command {
    /// Cleanly stop the backend
    Quit { uuid: Uuid },

    /// Print the config the backend is using
    ShowConfig { uuid: Uuid },
    /// Re-index a repository
    IndexRepository { uuid: Uuid },

    /// Stop an agent
    StopAgent { uuid: Uuid },

    /// Chat with an agent
    Chat { uuid: Uuid, message: String },

    /// Execute a tool executor compatible command in a running tool executor
    Exec {
        uuid: Uuid,
        command: swiftide::traits::Command,
    },
}

impl Command {
    #[must_use]
    pub fn uuid(&self) -> Uuid {
        match self {
            Command::Quit { uuid }
            | Command::StopAgent { uuid }
            | Command::ShowConfig { uuid }
            | Command::IndexRepository { uuid }
            | Command::Exec { uuid, .. }
            | Command::Chat { uuid, .. } => *uuid,
        }
    }

    #[must_use]
    pub fn with_uuid(self, uuid: Uuid) -> Self {
        match self {
            Command::StopAgent { .. } => Command::StopAgent { uuid },
            Command::Quit { .. } => Command::Quit { uuid },
            Command::ShowConfig { .. } => Command::ShowConfig { uuid },
            Command::IndexRepository { .. } => Command::IndexRepository { uuid },
            Command::Exec { command, .. } => Command::Exec { uuid, command },
            Command::Chat { message, .. } => Command::Chat { uuid, message },
        }
    }
}
