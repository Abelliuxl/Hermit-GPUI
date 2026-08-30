use crate::logger::dirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendKind {
    Hermes,
    OpenCode,
    Codex,
    ClaudeCode,
    Pi,
    OpenClaw,
    MiMoCode,
}

impl BackendKind {
    pub const ALL: [BackendKind; 7] = [
        BackendKind::Hermes,
        BackendKind::OpenCode,
        BackendKind::MiMoCode,
        BackendKind::Codex,
        BackendKind::ClaudeCode,
        BackendKind::Pi,
        BackendKind::OpenClaw,
    ];

    pub fn id(&self) -> &'static str {
        match self {
            BackendKind::Hermes => "hermes",
            BackendKind::OpenCode => "opencode",
            BackendKind::Codex => "codex",
            BackendKind::ClaudeCode => "claudecode",
            BackendKind::Pi => "pi",
            BackendKind::OpenClaw => "openclaw",
            BackendKind::MiMoCode => "mimocode",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            BackendKind::Hermes => "Hermes",
            BackendKind::OpenCode => "OpenCode",
            BackendKind::Codex => "Codex CLI",
            BackendKind::ClaudeCode => "Claude Code",
            BackendKind::Pi => "Pi",
            BackendKind::OpenClaw => "OpenClaw",
            BackendKind::MiMoCode => "MiMoCode",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            BackendKind::Hermes => 9119,
            BackendKind::OpenCode | BackendKind::MiMoCode => 4096,
            BackendKind::OpenClaw => 18789,
            BackendKind::Codex | BackendKind::ClaudeCode | BackendKind::Pi => 0,
        }
    }

    pub fn uses_network_server(&self) -> bool {
        matches!(
            self,
            BackendKind::Hermes
                | BackendKind::OpenCode
                | BackendKind::OpenClaw
                | BackendKind::MiMoCode
        )
    }

    pub fn parse(value: &str) -> Option<Self> {
        BackendKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.id() == value)
    }

    pub fn description(&self) -> &'static str {
        match self {
            BackendKind::Hermes => {
                "Connects to hermes serve; loopback instances are managed automatically."
            }
            BackendKind::OpenCode => "Connects to the structured HTTP/SSE API from opencode serve.",
            BackendKind::MiMoCode => "Connects to mimo serve using its OpenCode-compatible API.",
            BackendKind::OpenClaw => {
                "Connects directly to the OpenClaw Gateway WebSocket protocol."
            }
            BackendKind::Codex => "Runs codex app-server locally using its JSON-RPC protocol.",
            BackendKind::ClaudeCode => "Runs Claude Code locally using bidirectional stream-json.",
            BackendKind::Pi => "Runs Pi locally in RPC mode over JSONL.",
        }
    }

    pub fn credential_label(&self) -> &'static str {
        match self {
            BackendKind::Hermes => "Session token",
            BackendKind::OpenClaw => "Gateway token (optional)",
            _ => "Server password (optional)",
        }
    }
}

/// Persistent app settings. Stored as JSON in
/// `~/Library/Application Support/HermitGPUI/settings.json`
/// (UserDefaults equivalent for a non-bundled GPUI app).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub backend_kind: BackendKind,
    #[serde(default)]
    pub session_token: String,
    #[serde(default)]
    pub remember_session_token: bool,
    #[serde(default = "default_true")]
    pub auto_connect: bool,
    #[serde(default)]
    pub selected_profile: String,
    #[serde(default)]
    pub workspace_path: String,
    #[serde(default)]
    pub backend_host: String,
    #[serde(default)]
    pub backend_port: u16,
    #[serde(default)]
    pub backend_use_tls: bool,
    #[serde(default)]
    pub debug_logging_enabled: bool,
    #[serde(default)]
    pub per_backend: std::collections::HashMap<String, PerBackendConnection>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PerBackendConnection {
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub use_tls: bool,
}

fn default_true() -> bool {
    true
}

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 9119;

impl Default for Settings {
    fn default() -> Self {
        Self {
            backend_kind: BackendKind::Hermes,
            session_token: String::new(),
            remember_session_token: false,
            auto_connect: true,
            selected_profile: String::new(),
            workspace_path: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            backend_host: DEFAULT_HOST.to_string(),
            backend_port: DEFAULT_PORT,
            backend_use_tls: false,
            debug_logging_enabled: false,
            per_backend: std::collections::HashMap::new(),
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        dirs::app_support().join("HermitGPUI/settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read(&path) {
            Ok(data) => match serde_json::from_slice::<Settings>(&data) {
                Ok(mut settings) => {
                    let scoped = settings
                        .per_backend
                        .get(settings.backend_kind.id())
                        .cloned()
                        .unwrap_or_default();
                    if settings.backend_host.is_empty() {
                        settings.backend_host = if scoped.host.is_empty() {
                            DEFAULT_HOST.to_string()
                        } else {
                            scoped.host
                        };
                    }
                    if settings.backend_port == 0 {
                        settings.backend_port = if scoped.port > 0 {
                            scoped.port
                        } else {
                            settings.backend_kind.default_port()
                        };
                    }
                    settings
                }
                Err(_) => Settings::default(),
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }

    /// Persist the outgoing backend's connection under its scoped key and load
    /// the incoming one, mirroring the didSet logic in the SwiftUI store.
    pub fn switch_backend(&mut self, next: BackendKind) {
        let outgoing = self.backend_kind;
        self.per_backend.insert(
            outgoing.id().to_string(),
            PerBackendConnection {
                host: self.backend_host.clone(),
                port: self.backend_port,
                use_tls: self.backend_use_tls,
            },
        );
        self.backend_kind = next;
        let scoped = self.per_backend.get(next.id()).cloned().unwrap_or_default();
        self.backend_host = if scoped.host.is_empty() {
            DEFAULT_HOST.to_string()
        } else {
            scoped.host
        };
        self.backend_port = if scoped.port > 0 {
            scoped.port
        } else {
            next.default_port()
        };
        self.backend_use_tls = scoped.use_tls;
        self.save();
    }

    pub fn resolved_host(&self) -> String {
        let value = self.backend_host.trim();
        if value.is_empty() {
            DEFAULT_HOST.to_string()
        } else {
            value.to_string()
        }
    }

    pub fn resolved_port(&self) -> u16 {
        if self.backend_port > 0 {
            self.backend_port
        } else {
            self.backend_kind.default_port()
        }
    }

    pub fn active_backend_url(&self) -> String {
        let scheme = if self.backend_use_tls {
            "https"
        } else {
            "http"
        };
        format!(
            "{}://{}:{}",
            scheme,
            self.resolved_host(),
            self.resolved_port()
        )
    }

    /// Only loopback Hermes addresses are managed (auto-started) by Hermit.
    pub fn is_managed_local_backend(&self) -> bool {
        if self.backend_kind != BackendKind::Hermes || self.backend_use_tls {
            return false;
        }
        matches!(
            self.resolved_host().to_lowercase().as_str(),
            "127.0.0.1" | "localhost" | "::1"
        )
    }

    pub fn normalized_profile(&self) -> Option<String> {
        let value = self.selected_profile.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    pub fn workspace_trimmed(&self) -> Option<String> {
        let value = self.workspace_path.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
}
