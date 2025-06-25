use anyhow::Result;
use futures::StreamExt;
use futures::TryFutureExt;
use libp2p::{
    gossipsub, mdns, noise, tcp, yamux,
    swarm::{SwarmEvent, SwarmBuilder, NetworkBehaviour},
    identity, PeerId, Transport,
    kad::{self, store::MemoryStore},
    identify,
    relay,
    Multiaddr,
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
    identify: identify::Behaviour,
    relay: relay::Behaviour,
    kademlia: kad::Kademlia<MemoryStore>,
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
    daemon_full_addr: String,
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
        let store = MemoryStore::new(local_peer_id);
        let mut kademlia = kad::Kademlia::new(local_peer_id, store);
        let bootstrap_nodes = [
            "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt"
        ];
        for addr in bootstrap_nodes {
            let multiaddr: Multiaddr = addr.parse().expect("Failed to parse bootstrap address");
            if let Some(libp2p::multiaddr::Protocol::P2p(hash)) = multiaddr.iter().last() {
                let peer_id = PeerId::from_multihash(hash).expect("Valid PeerId multihash");
                kademlia.add_address(&peer_id, multiaddr);
            } else {
                eprintln!("Could not extract PeerId from bootstrap address: {}", addr);
            }
        }
        kademlia.bootstrap().unwrap();
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(id_keys.clone()),
            gossipsub::Config::default(),
        ).unwrap();
        let behaviour = ClientBehaviour {
            gossipsub,
            mdns,
            identify: identify::Behaviour::new(identify::Config::new(
                "/emergency-git/1.0".into(),
                id_keys.public(),
            )),
            relay: relay::Behaviour::new(local_peer_id, Default::default()),
            kademlia,
        };
        SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
    };
    swarm.behaviour_mut().gossipsub.subscribe(&topic).unwrap();

    // --- NEW: Direct Dial Logic ---
    let daemon_addr: Multiaddr = daemon_full_addr.parse()
        .map_err(|e| CoreError::NetworkError { message: format!("Invalid daemon address: {}", e) })?;
    if let Err(e) = swarm.dial(daemon_addr) {
        return Err(CoreError::NetworkError { message: format!("Failed to dial daemon: {}", e) });
    }
    println!("Dialing daemon... waiting for connection.");
    let mut published_request = false;
    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("✅ Successfully connected to daemon: {}", peer_id);
                }
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, .. })) => {
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
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                return Err(CoreError::Timeout);
            }
        }
    }
}

// Synchronous wrapper for UniFFI
pub fn emergency_commit(
    daemon_full_addr: String,
    repo_path: String,
    file_path: String,
    new_content: String,
    commit_message: String,
) -> Result<String, CoreError> {
    // Create a new Tokio runtime or use the existing one
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CoreError::NetworkError { message: format!("Failed to create runtime: {}", e) })?;
    rt.block_on(emergency_commit_async(daemon_full_addr, repo_path, file_path, new_content, commit_message))
}

pub async fn pair_async(daemon_full_addr: String) -> Result<(), CoreError> {
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
        let store = MemoryStore::new(local_peer_id);
        let mut kademlia = kad::Kademlia::new(local_peer_id, store);
        let bootstrap_nodes = [
            "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt"
        ];
        for addr in bootstrap_nodes {
            let multiaddr: Multiaddr = addr.parse().expect("Failed to parse bootstrap address");
            if let Some(libp2p::multiaddr::Protocol::P2p(hash)) = multiaddr.iter().last() {
                let peer_id = PeerId::from_multihash(hash).expect("Valid PeerId multihash");
                kademlia.add_address(&peer_id, multiaddr);
            } else {
                eprintln!("Could not extract PeerId from bootstrap address: {}", addr);
            }
        }
        kademlia.bootstrap().unwrap();
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(id_keys.clone()),
            gossipsub::Config::default(),
        ).unwrap();
        let behaviour = ClientBehaviour {
            gossipsub,
            mdns,
            identify: identify::Behaviour::new(identify::Config::new(
                "/emergency-git/1.0".into(),
                id_keys.public(),
            )),
            relay: relay::Behaviour::new(local_peer_id, Default::default()),
            kademlia,
        };
        SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
    };
    swarm.behaviour_mut().gossipsub.subscribe(&topic).unwrap();
    // --- NEW: Direct Dial Logic ---
    let daemon_addr: Multiaddr = daemon_full_addr.parse()
        .map_err(|e| CoreError::NetworkError { message: format!("Invalid daemon address: {}", e) })?;
    if let Err(e) = swarm.dial(daemon_addr) {
        return Err(CoreError::NetworkError { message: format!("Failed to dial daemon: {}", e) });
    }
    println!("Dialing daemon... waiting for connection.");
    let mut published_request = false;
    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("✅ Successfully connected to daemon: {}", peer_id);
                }
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, .. })) => {
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
            _ = tokio::time::sleep(Duration::from_secs(20)) => {
                return Err(CoreError::Timeout);
            }
        }
    }
}

pub fn pair(daemon_full_addr: String) -> Result<(), CoreError> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CoreError::NetworkError { message: format!("Failed to create runtime: {}", e) })?;
    rt.block_on(pair_async(daemon_full_addr))
}

uniffi::include_scaffolding!("mobile_core");