use std::{path::PathBuf, time::Instant};

use super::{
    App, Buffer, Focus, Viewport, form::Cursor, messages::Messages, saved_config::SavedConfig,
};
use crate::{
    cli::Launch,
    config::{Config, Look, Theme},
    history::History,
};

/**
 * Settings given on the command line. They apply to this run only and are
 * never saved, unless the user sets the same setting in the app.
 */
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    pub theme: Option<Theme>,
    /// Already normalised by `network::server_url`.
    pub server: Option<String>,
}

impl Overrides {
    fn apply_to(&self, config: &mut Config) {
        if let Some(theme) = self.theme {
            config.theme = theme;
        }
        if let Some(server) = &self.server {
            config.multiplayer.server.clone_from(server);
        }
    }
}

impl App {
    /**
     * An application started with the `saved` settings, saved back to
     * `config_path` when there is one, and `overrides` for this run. The
     * `warnings` of loading the files are shown first, then `launch` opens.
     */
    pub fn new(
        saved: Config,
        overrides: &Overrides,
        config_path: Option<PathBuf>,
        history: History,
        warnings: Vec<String>,
        launch: Launch,
    ) -> Self {
        let mut config = saved;
        overrides.apply_to(&mut config);
        let mut app = Self {
            saved: SavedConfig::new(config_path, &config),
            config,
            history,
            focus: Focus::Explorer,
            buffer: Buffer::Practice,
            practice_cursor: Cursor::default(),
            race_cursor: Cursor::default(),
            room_code: String::new(),
            settings_cursor: Cursor::default(),
            history_scroll: 0,
            help_scroll: 0,
            editing: None,
            prompt: None,
            palette: None,
            messages: Messages::default(),
            activity: None,
            sidebar: true,
            sidebar_choice: None,
            viewport: Viewport::default(),
            pending: None,
            quiet_until: None,
            leave_armed: None,
            shuffled: Look::Notes,
            born: Instant::now(),
            last_input: None,
            workspace: workspace_name(),
            quit: false,
        };
        for warning in warnings {
            app.messages.next_event(false);
            app.error(warning);
        }
        app.messages.next_event(false);
        app.launch(launch);
        app
    }
}

fn workspace_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|directory| {
            directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "workspace".to_owned())
}
