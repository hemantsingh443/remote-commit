use anyhow::{anyhow, Result};
use futures::StreamExt;
use libp2p::{
    gossipsub, mdns, noise, tcp, yamux,
    swarm::{SwarmEvent, SwarmBuilder, NetworkBehaviour},
    identity, PeerId, Transport,
};
use std::time::Duration;
use tokio::select;

mod protocol;
use protocol::{CommitRequest, NetworkMessage};

// The behaviour is now internal to our library
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ClientBehaviourEvent")]
struct ClientBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

/// This is the public function our mobile app will eventually call.
/// It connects to the P2P network, sends a commit request, and waits for a response.
/// Returns Ok(commit_hash) on success, or Err(error_message) on failure.
pub async fn emergency_commit(
    repo_path: String,
    file_path: String,
    new_content: String,
    commit_message: String,
) -> Result<String, String> { // Note the return type: Result<Success, Error>

    let id_keys = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(id_keys.public());
    println!("Mobile Core Peer ID: {}", local_peer_id);

    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&id_keys).unwrap()) // .unwrap() is ok here
        .multiplex(yamux::Config::default())
        .boxed();

    let topic = gossipsub::IdentTopic::new("emergency-git-commits");
    
    let commit_request = CommitRequest {
        repo_path,
        file_path,
        new_content,
        commit_message,
    };
    
    let mut swarm = {
        // ... (swarm setup is identical to the old client)
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

    // The main loop, adapted to return a Result instead of printing and exiting.
    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(ClientBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _) in list {
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed { peer_id, .. })) => {
                    if !published_request {
                        let request_message = NetworkMessage::Request(commit_request.clone());
                        let request_json = serde_json::to_string(&request_message).unwrap();
                        
                        if swarm.behaviour_mut().gossipsub.publish(topic.clone(), request_json.as_bytes()).is_ok() {
                           published_request = true;
                           println!("Request published. Waiting for response...");
                        }
                    }
                },
                SwarmEvent::Behaviour(ClientBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                    if let Ok(NetworkMessage::Response(response)) = serde_json::from_slice(&message.data) {
                        return if response.success {
                            Ok(response.commit_hash.unwrap_or_default())
                        } else {
                            Err(response.error_message.unwrap_or_default())
                        }
                    }
                }
                _ => {}
            },
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                return Err("Timeout: Did not receive a response from the daemon.".to_string());
            }
        }
    }
} 

uniffi::include_scaffolding!("mobile_core");