//! Extension message bus - chrome.runtime.sendMessage implementation.
//! All messages between extension components go through here with validation.

use crate::sandbox::{validate_message, SandboxPolicy, ValidatedMessage, ComponentType};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// A message handler registered by an extension component.
type MessageHandler = Box<dyn Fn(&ValidatedMessage, Value) -> Result<Value, String> + Send + Sync>;

/// The message bus for all extensions.
pub struct MessageBus {
    /// Handlers per extension ID.
    handlers: Mutex<HashMap<String, Vec<MessageHandler>>>,
    /// Sandbox policy per extension.
    policies: Mutex<HashMap<String, SandboxPolicy>>,
}

impl MessageBus {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
            policies: Mutex::new(HashMap::new()),
        }
    }

    /// Register a handler for an extension.
    pub fn register_handler(
        &self,
        extension_id: &str,
        handler: MessageHandler,
    ) {
        self.handlers.lock().unwrap()
            .entry(extension_id.to_string())
            .or_default()
            .push(handler);
    }

    /// Set the sandbox policy for an extension.
    pub fn set_policy(&self, extension_id: &str, policy: SandboxPolicy) {
        self.policies.lock().unwrap()
            .insert(extension_id.to_string(), policy);
    }

    /// Send a message through the bus with validation.
    /// Returns the response from the handler, or an error if validation fails.
    pub fn send_message(
        &self,
        extension_id: &str,
        message: ValidatedMessage,
        payload: Value,
    ) -> Result<Value, String> {
        // Validate the message.
        let policy = self.policies.lock().unwrap()
            .get(extension_id)
            .cloned()
            .unwrap_or_default();

        validate_message(&policy, extension_id, &message)
            .map_err(|e| format!("Message validation failed: {}", e))?;

        // Create the full message with payload.
        let action = message.action.clone();
        let full_message = ValidatedMessage {
            payload,
            ..message
        };

        // Dispatch to handlers.
        let handlers = self.handlers.lock().unwrap();
        if let Some(handlers) = handlers.get(extension_id) {
            for handler in handlers {
                let result = handler(&full_message, full_message.payload.clone());
                if let Ok(response) = result {
                    return Ok(response);
                }
            }
        }

        Err(format!("No handler found for action '{}'", action))
    }

    /// Broadcast a message to all extensions (used for browser events like tab updated).
    pub fn broadcast(
        &self,
        action: &str,
        payload: Value,
    ) -> Vec<(String, Result<Value, String>)> {
        let handlers = self.handlers.lock().unwrap();
        let mut results = Vec::new();
        for (extension_id, handlers) in handlers.iter() {
            for handler in handlers {
                let msg = ValidatedMessage {
                    extension_id: extension_id.clone(),
                    sender_component: ComponentType::Background,
                    action: action.to_string(),
                    payload: payload.clone(),
                    sender_tab_id: None,
                    sender_origin: None,
                };
                let result = handler(&msg, payload.clone());
                results.push((extension_id.clone(), result));
            }
        }
        results
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}
