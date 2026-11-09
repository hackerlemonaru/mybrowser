//! Extension host binary - runs as a separate process per extension.
//! Communicates with the main browser process via stdio IPC.

use extensions::message_bus::MESSAGE_BUS;
use extensions::sandbox::SandboxPolicy;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: extension-host <extension-id> <manifest-path>");
        std::process::exit(1);
    }

    let extension_id = &args[1];
    let manifest_path = &args[2];

    eprintln!("[Extension Host] Starting host for extension {}", extension_id);

    // Set sandbox policy.
    MESSAGE_BUS.set_policy(extension_id, SandboxPolicy::default());

    // Load and run the service worker.
    // This would parse the manifest, load the service worker JS,
    // and run it in Falco's TJS VM with extension API bindings.

    eprintln!("[Extension Host] Host ready for extension {}", extension_id);

    // Main event loop - wait for messages from browser process.
    loop {
        // Read IPC message from stdin.
        // Process it.
        // Send response to stdout.
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
