use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use tracing::warn;

/// File name for persisted MCP registry state.
const MCP_REGISTRY_FILE: &str = "mcp_registry.json";

/// Environment variable that overrides the directory used to persist MCP state.
const CODEX_STATE_HOME_ENV: &str = "CODEX_STATE_HOME";

/// Registry tracking user-managed MCP enablement state.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct McpRegistry {
    enabled: BTreeSet<String>,
}

impl McpRegistry {
    /// Load the MCP registry from disk. When the file is missing or cannot be
    /// parsed, return an empty registry.
    pub fn load(codex_home: &Path) -> io::Result<Self> {
        let path = registry_path(codex_home)?;
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(err) => {
                return Err(err);
            }
        };

        match serde_json::from_str::<Self>(&contents) {
            Ok(registry) => Ok(registry),
            Err(err) => {
                warn!("Failed to parse MCP registry at {}: {err}", path.display());
                Ok(Self::default())
            }
        }
    }

    /// Persist the registry to disk, atomically replacing any existing file.
    pub fn save(&self, codex_home: &Path) -> io::Result<()> {
        let path = registry_path(codex_home)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("missing parent dir"))?;
        fs::create_dir_all(parent)?;

        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(tmp.as_file_mut(), self)?;
        tmp.persist(path).map_err(|err| err.error).map(|_| ())
    }

    /// Return the names of all enabled servers.
    pub fn enabled(&self) -> &BTreeSet<String> {
        &self.enabled
    }

    /// Enable or disable the supplied server. Returns `true` when the registry
    /// was changed.
    pub fn set_enabled(&mut self, name: &str, enable: bool) -> bool {
        if enable {
            self.enabled.insert(name.to_string())
        } else {
            self.enabled.remove(name)
        }
    }

    /// Returns `true` when the supplied server name is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.contains(name)
    }
}

fn registry_path(codex_home: &Path) -> io::Result<PathBuf> {
    let base = if let Ok(path) = env::var(CODEX_STATE_HOME_ENV) {
        PathBuf::from(path)
    } else if let Some(dir) = dirs::state_dir() {
        dir.join("codex")
    } else {
        codex_home.join("state")
    };

    Ok(base.join(MCP_REGISTRY_FILE))
}
