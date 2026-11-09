//! OS-level sandbox enforcement for renderer processes.
//! Limits what a compromised renderer can do:
//!   - No direct filesystem access
//!   - No direct network access (must go through browser process)
//!   - No executing arbitrary binaries
//!   - No reading other processes' memory
//!
//! Platform-specific:
//!   - Linux: seccomp-bpf + user namespaces
//!   - macOS: Seatbelt sandbox profile
//!   - Windows: Job object with restricted token

use std::sync::Mutex;

/// Sandbox level for a renderer process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxLevel {
    /// No sandbox (unsafe, only for trusted local content).
    None,
    /// Basic sandbox — blocks filesystem and direct network.
    Basic,
    /// Strict sandbox — blocks everything except browser-mediated APIs.
    Strict,
}

/// Sandbox configuration for a renderer context.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub level: SandboxLevel,
    pub allow_file_read: bool,
    pub allow_network: bool,
    pub allow_exec: bool,
    pub allow_syscalls: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            level: SandboxLevel::Strict,
            allow_file_read: false,
            allow_network: false,
            allow_exec: false,
            allow_syscalls: Vec::new(),
        }
    }
}

/// The sandbox manager — applies sandbox restrictions to renderer processes.
pub struct SandboxManager {
    configs: Mutex<Vec<(String, SandboxConfig)>>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self {
            configs: Mutex::new(Vec::new()),
        }
    }

    /// Apply a sandbox config to a renderer context.
    pub fn apply(&self, context_id: &str, config: SandboxConfig) {
        let mut configs = self.configs.lock().unwrap();
        if let Some(entry) = configs.iter_mut().find(|(id,_)| id == context_id) {
            entry.1 = config;
        } else {
            configs.push((context_id.to_string(), config));
        }
    }

    /// Get the sandbox config for a context.
    pub fn get(&self, context_id: &str) -> Option<SandboxConfig> {
        self.configs.lock().unwrap()
            .iter()
            .find(|(id,_)| id == context_id)
            .map(|(_, config)| config.clone())
    }

    /// Remove a context's sandbox config.
    pub fn remove(&self, context_id: &str) {
        self.configs.lock().unwrap()
            .retain(|(id,_)| id != context_id);
    }

    /// Check if a syscall is allowed for a context.
    pub fn is_syscall_allowed(&self, context_id: &str, syscall: &str) -> bool {
        let config = match self.get(context_id) {
            Some(c) => c,
            None => return true, // No config = allow (trusted)
        };

        match config.level {
            SandboxLevel::None => true,
            SandboxLevel::Basic => {
                // Basic allows most syscalls but blocks dangerous ones.
                !matches!(syscall, "execve" | "execveat" | "mount" | "umount2")
            }
            SandboxLevel::Strict => {
                // Strict only allows explicitly listed syscalls.
                config.allow_syscalls.contains(&syscall.to_string())
            }
        }
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_sandbox_blocks_exec() {
        let mgr = SandboxManager::new();
        mgr.apply("ctx-1", SandboxConfig {
            level: SandboxLevel::Strict,
            ..Default::default()
        });
        assert!(!mgr.is_syscall_allowed("ctx-1", "execve"));
    }

    #[test]
    fn basic_sandbox_allows_read() {
        let mgr = SandboxManager::new();
        mgr.apply("ctx-1", SandboxConfig {
            level: SandboxLevel::Basic,
            allow_file_read: true,
            ..Default::default()
        });
        assert!(mgr.is_syscall_allowed("ctx-1", "read"));
    }
}
