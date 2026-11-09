//! Extension Host Process - separate OS process per extension.
//! Runs the background service worker in a sandboxed environment.

use crate::manifest::Manifest;
use crate::message_bus::MessageBus;
use crate::sandbox::SandboxPolicy;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

/// Handle to a running extension host process.
#[derive(Debug)]
pub struct HostProcessHandle {
    pub extension_id: String,
    pub pid: u32,
}

impl HostProcessHandle {
    pub fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        // In a real implementation, send SIGTERM to the child process.
        eprintln!("[Extension Host] Stopping host for extension {}", self.extension_id);
        Ok(())
    }
}

/// The extension host - manages the service worker process.
pub struct HostProcess {
    _child: Option<Child>,
    pub handle: HostProcessHandle,
}

impl HostProcess {
    /// Spawn a new extension host process for the given extension.
    pub fn spawn(extension: &crate::Extension) -> Result<Self, Box<dyn std::error::Error>> {
        let extension_id = extension.id.clone();
        let manifest = &extension.manifest;

        // The host process runs the service worker script.
        // For now, we spawn a subprocess that runs the JS in a TJS VM.
        // In production, this would be a separate sandboxed process with seccomp/landlock.
        let service_worker_path = if let Some(bg) = &manifest.background {
            if let Some(sw) = &bg.service_worker {
                extension.path.join(sw)
            } else {
                extension.path.join("background.js")
            }
        } else {
            extension.path.join("background.js")
        };

        eprintln!("[Extension Host] Spawning host for extension {}", extension_id);
        eprintln!("[Extension Host] Service worker: {:?}", service_worker_path);

        // Create sandbox policy.
        let policy = SandboxPolicy::default();

        // Register with message bus.
        // (In production, the host process connects to the main process via IPC.)

        let handle = HostProcessHandle {
            extension_id: extension_id.clone(),
            pid: 0, // Would be the actual child PID.
        };

        Ok(Self {
            _child: None, // No actual child spawned yet - Falco's TJS VM runs in-process for now.
            handle,
        })
    }
}

/// Global message bus instance (shared between browser and extension hosts).
pub static MESSAGE_BUS: once_cell::sync::Lazy<MessageBus> = once_cell::sync::Lazy::new(MessageBus::new);
