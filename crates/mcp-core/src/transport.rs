use serde::{Deserialize, Serialize};

use crate::protocol::{JsonRpcNotification, JsonRpcRequest};

/// This trait represents messages that can be sent over the transport.
/// By using this trait, we can use the type system to ensure we don't initiate communication with
/// a response type.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SendableMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

impl From<JsonRpcRequest> for SendableMessage {
    fn from(request: JsonRpcRequest) -> Self {
        SendableMessage::Request(request)
    }
}

impl From<JsonRpcNotification> for SendableMessage {
    fn from(notification: JsonRpcNotification) -> Self {
        SendableMessage::Notification(notification)
    }
}
