use anyhow::Result;
use futures::StreamExt;
use futures::TryFutureExt;
use libp2p::{
    gossipsub, mdns, noise, tcp, yamux,
    swarm::{SwarmEvent, SwarmBuilder, NetworkBehaviour},
    identity, PeerId, Transport,
};
use std::time::Duration;
use std::fs;
use std::path::Path;
use tokio::select;
use thiserror::Error;

mod protocol;
use protocol::{CommitRequest, NetworkMessage};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("A networking error occurred: {message}")]
    NetworkError { message: String },

    #[error("JSON serialization failed: {message}")]
    JsonError { message: String },

    #[error("The operation timed out.")]
    Timeout,
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ClientBehaviourEvent")]
struct ClientBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

/// Loads a keypair from a file or creates a new one if it doesn't exist.
fn get_or_create_identity() -> Result<identity::Keypair, CoreError> {
    let keypair_path = Path::new("client_identity.key");

    if keypair_path.exists() {
        println!("Loading existing client identity...");
        let key_bytes = fs::read(keypair_path)
            .map_err(|e| CoreError::NetworkError { message: format!("Failed to read key file: {}", e) })?;
        identity::Keypair::from_protobuf_encoding(&key_bytes)
            .map_err(|e| CoreError::NetworkError { message: format!("Failed to decode key file: {}", e) })
    } else {
        println!("No client identity found. Generating a new one...");
        let keypair = identity::Keypair::generate_ed25519();
        let encoded_key = keypair.to_protobuf_encoding()
            .map_err(|e| CoreError::NetworkError { message: format!("Failed to encode key file: {}", e) })?;
        fs::write(keypair_path, encoded_key)
            .map_err(|e| CoreError::NetworkError { message: format!("Failed to write key file: {}", e) })?;
        Ok(keypair)
    }
}

// Async implementation
pub async fn emergency_commit_async(
    repo_path: String,
    file_path: String,
    new_content: String,
    commit_message: String,
) -> Result<String, CoreError> {
    let id_keys = get_or_create_identity()?;
    let local_peer_id = PeerId::from(id_keys.public());
    println!("Client Peer ID: {}", local_peer_id);

    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&id_keys).unwrap())
        .multiplex(yamux::Config::default())
        .boxed();

    let topic = gossipsub::IdentTopic::new("emergency-git-commits");
    
    let commit_request = CommitRequest { repo_path, file_path, new_content, commit_message };
    
    let mut swarm = {
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(id_keys),
            gossipsub::Config::default(),
        ).unwrap();
        let behaviour = ClientBehaviour { gossipsub, mdns };
        SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
    };
    
    swarm.behaviour_mut().gossipsub.subscribe(&topic).unwrap();
    
    let mut published_request = false;

    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(ClientBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _) in list {
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { .. })) => {
                    if !published_request {
                        let request_message = NetworkMessage::Request(commit_request.clone());
                        let request_json = serde_json::to_string(&request_message)
                            .map_err(|e| CoreError::JsonError { message: e.to_string() })?;
                        
                        if swarm.behaviour_mut().gossipsub.publish(topic.clone(), request_json.as_bytes()).is_ok() {
                           published_request = true;
                        }
                    }
                },
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                    if let Ok(NetworkMessage::Response(response)) = serde_json::from_slice(&message.data) {
                        return if response.success {
                            Ok(response.commit_hash.unwrap_or_default())
                        } else {
                            Err(CoreError::NetworkError { message: response.error_message.unwrap_or_default() })
                        }
                    }
                }
                _ => {}
            },
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                return Err(CoreError::Timeout);
            }
        }
    }
}

// Synchronous wrapper for UniFFI
pub fn emergency_commit(
    repo_path: String,
    file_path: String,
    new_content: String,
    commit_message: String,
) -> Result<String, CoreError> {
    // Create a new Tokio runtime or use the existing one
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CoreError::NetworkError { message: format!("Failed to create runtime: {}", e) })?;
    
    rt.block_on(emergency_commit_async(repo_path, file_path, new_content, commit_message))
}

pub async fn pair_async() -> Result<(), CoreError> {
    let id_keys = get_or_create_identity()?;
    let local_peer_id = PeerId::from(id_keys.public());
    println!("Client Peer ID: {}", local_peer_id);
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&id_keys).unwrap())
        .multiplex(yamux::Config::default())
        .boxed();
    let topic = gossipsub::IdentTopic::new("emergency-git-commits");
    let mut swarm = {
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(id_keys),
            gossipsub::Config::default(),
        ).unwrap();
        let behaviour = ClientBehaviour { gossipsub, mdns };
        SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
    };
    swarm.behaviour_mut().gossipsub.subscribe(&topic).unwrap();
    let mut published_request = false;
    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(ClientBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _) in list {
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { .. })) => {
                    if !published_request {
                        let request_message = NetworkMessage::PairRequest;
                        let request_json = serde_json::to_string(&request_message).unwrap();
                        if swarm.behaviour_mut().gossipsub.publish(topic.clone(), request_json.as_bytes()).is_ok() {
                           published_request = true;
                           println!("Pairing request sent. Waiting for approval on daemon...");
                        }
                    }
                },
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                    if let Ok(NetworkMessage::PairSuccess) = serde_json::from_slice(&message.data) {
                        return Ok(());
                    }
                }
                _ => {}
            },
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                return Err(CoreError::Timeout);
            }
        }
    }
}

uniffi::include_scaffolding!("mobile_core");