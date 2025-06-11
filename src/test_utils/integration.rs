use std::sync::Arc;

use anyhow::Result;
use ratatui::{Terminal, backend::TestBackend};
use swiftide::traits::Persist as _;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

use crate::{
    commands::CommandHandler,
    frontend::{self, App},
    indexing::duckdb_index::{self, DuckdbIndex},
    repository::Repository,
};

use super::{TestGuard, test_repository};

pub struct IntegrationContext {
    pub app: App<'static>,
    pub uuid: Uuid,
    pub repository: Arc<Repository>,
    pub terminal: Terminal<TestBackend>,
    pub workdir: std::path::PathBuf,

    // Guards the command handler
    pub handler_guard: AbortOnDropHandle<()>,
    // Guards the repository
    pub repository_guard: TestGuard,
}

impl IntegrationContext {
    pub fn render_ui(&mut self) -> &TestBackend {
        self.terminal
            .draw(|f| frontend::ui(f, f.area(), &mut self.app))
            .unwrap();

        self.terminal.backend()
    }
}

/// Sets up an app
pub async fn setup_integration() -> Result<IntegrationContext> {
    let (repository, repository_guard) = test_repository();
    let workdir = repository.path().clone();
    let repository = Arc::new(repository);
    let mut app = App::default_from_repository(repository.clone()).with_workdir(repository.path());
    let duckdb = duckdb_index::get_duckdb(repository.config());
    duckdb.setup().await.unwrap();
    let terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();

    let index = DuckdbIndex::default();
    let mut handler = CommandHandler::from_index(index);
    handler.register_ui(&mut app);
    let handler_guard = handler.start();

    let uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8").unwrap();
    let current_chat = app.current_chat_mut();

    // Force to fixed uuid so that snapshots are stable
    current_chat.uuid = uuid;
    app.current_chat_uuid = uuid;

    Ok(IntegrationContext {
        app,
        uuid,
        repository,
        terminal,
        workdir,

        handler_guard,
        repository_guard,
    })
}
