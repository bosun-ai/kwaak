use uuid::Uuid;

/// Commands are the main way to interact with the backend
///
/// By default all commands can be triggered from the ui like `/<command>`
#[derive(
    Debug,
    PartialEq,
    Eq,
    strum_macros::EnumString,
    strum_macros::Display,
    strum_macros::IntoStaticStr,
    strum_macros::EnumIs,
    Clone,
)]
#[strum(serialize_all = "snake_case")]
pub enum Command {
    Quit { uuid: Uuid },
    ShowConfig { uuid: Uuid },
    IndexRepository { uuid: Uuid },
    StopAgent { uuid: Uuid },
    Chat { uuid: Uuid, message: String },
}

impl Command {
    #[must_use]
    pub fn uuid(&self) -> Uuid {
        match self {
            Command::Quit { uuid }
            | Command::StopAgent { uuid }
            | Command::ShowConfig { uuid }
            | Command::IndexRepository { uuid }
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
            Command::Chat { message, .. } => Command::Chat { uuid, message },
        }
    }
}
