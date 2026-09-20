//! The JSON-RPC 2.0 envelope.
//!
//! Only the envelope: what a method name means, and what a call does, belongs to the dispatcher.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a caller sends.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// Must be exactly "2.0". Checked rather than trusted, so a client speaking another dialect
    /// is told so instead of being half-understood.
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    /// Absent for a notification, which expects no answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
}

/// A request's identity, echoed back so a client can match answers to calls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Number(i64),
    String(String),
}

/// One request, or several.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Single(Request),
    Batch(Vec<Request>),
}

/// What comes back.
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// Null rather than absent when the request could not be read far enough to carry one, which
    /// is what the specification asks for.
    pub id: Option<Id>,
}

impl Response {
    pub fn ok(result: Value, id: Option<Id>) -> Self {
        Self {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn failed(error: RpcError, id: Option<Id>) -> Self {
        Self {
            jsonrpc: "2.0",
            result: None,
            error: Some(error),
            id,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// The message a caller is given.
    ///
    /// Deliberately the error's own text and nothing else: no backtrace, no SQL, no internal
    /// path. What went wrong on the inside is for the log, not for whoever asked.
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn parse_error(detail: impl Into<String>) -> Self {
        Self::new(-32700, detail)
    }

    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::new(-32600, detail)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method \"{method}\" not found"))
    }

    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::new(-32602, detail)
    }

    /// Anything the call itself raised.
    ///
    /// Kept out of the reserved range, which the specification sets aside for protocol faults;
    /// this one means the protocol worked and the operation did not.
    pub fn call_failed(detail: impl Into<String>) -> Self {
        Self::new(-32000, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(-32603, detail)
    }
}
