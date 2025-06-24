// In protocol.rs
use serde::{Deserialize, Serialize};
use libp2p::PeerId; // We need to serialize PeerId

#[derive(Serialize, Deserialize, Debug)]
pub enum NetworkMessage {
    // Client -> Daemon: "I'd like to pair with you."
    PairRequest,
    // Daemon -> Client: "Okay, I've saved you as a trusted peer."
    PairSuccess,
    
    Request(CommitRequest),
    Response(CommitResponse),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitRequest {
    pub repo_path: String,
    pub file_path: String,
    pub new_content: String,
    pub commit_message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommitResponse {
    pub success: bool,
    pub commit_hash: Option<String>,
    pub error_message: Option<String>,
}