use serde::{Deserialize, Serialize};

// A wrapper for all messages sent on the network
#[derive(Serialize, Deserialize, Debug)]
pub enum NetworkMessage {
    Request(CommitRequest),
    Response(CommitResponse),
}

// The message the client sends to the daemon
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitRequest {
    pub repo_path: String,
    pub file_path: String,
    pub new_content: String,
    pub commit_message: String,
}

// The message the daemon sends back to the client
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitResponse {
    pub success: bool,
    pub commit_hash: Option<String>,
    pub error_message: Option<String>,
}