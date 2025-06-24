use anyhow::{anyhow, Result};
use libp2p::{
    gossipsub, mdns,
    swarm::{SwarmEvent, SwarmBuilder},
    identity,
    PeerId,
    Swarm,
    // Import NetworkBehaviour from the correct location
    swarm::NetworkBehaviour,
    // Import transport building utilities
    noise,
    tcp,
    yamux,
    Transport,
    gossipsub::Message,
};
use futures::StreamExt; // Required for select_next_some()
use tokio::select;

mod protocol;
use protocol::{CommitRequest, CommitResponse, NetworkMessage};

// This derive macro will now work correctly with proper imports
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "DaemonBehaviourEvent")]
struct DaemonBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[tokio::main]
async fn main() -> Result<()> {
    let id_keys = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(id_keys.public());
    println!("Daemon Peer ID: {}", local_peer_id);

    // Build transport manually since development_transport might not be available
    // with your current feature set
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&id_keys)?)
        .multiplex(yamux::Config::default())
        .boxed();

    let topic = gossipsub::IdentTopic::new("emergency-git-commits");

    // Use the modern SwarmBuilder API
    let mut swarm = SwarmBuilder::with_tokio_executor(
        transport,
        DaemonBehaviour {
            gossipsub: {
                let gossipsub_config = gossipsub::Config::default();
                gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(id_keys),
                    gossipsub_config,
                )
                .map_err(|e| anyhow!(e))?
            },
            mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?,
        },
        local_peer_id,
    )
    .build();

    // Subscribe to the topic AFTER the swarm is created
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    println!("Starting P2P daemon event loop...");
    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Daemon listening on {}/p2p/{}", address, local_peer_id);
                }

                SwarmEvent::Behaviour(DaemonBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered a new peer: {}", peer_id);
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                
                SwarmEvent::Behaviour(DaemonBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message,
                    ..
                })) => {
                    handle_network_message(message, &mut swarm.behaviour_mut().gossipsub);
                }
                _ => {}
            }
        }
    }
}

fn handle_network_message(message: Message, gossipsub: &mut gossipsub::Behaviour) {
    if let Ok(NetworkMessage::Request(request)) = serde_json::from_slice(&message.data) {
        println!("Received commit request from peer: {}", message.source.unwrap());
        let response = match git_actor::perform_commit(
            &request.repo_path,
            &request.file_path,
            &request.new_content,
            &request.commit_message,
        ) {
            Ok(oid) => {
                println!("Successfully created commit: {}", oid);
                CommitResponse {
                    success: true,
                    commit_hash: Some(oid.to_string()),
                    error_message: None,
                }
            }
            Err(e) => {
                eprintln!("Failed to perform commit: {:?}", e);
                CommitResponse {
                    success: false,
                    commit_hash: None,
                    error_message: Some(e.to_string()),
                }
            }
        };
        let response_message = NetworkMessage::Response(response);
        if let Ok(json) = serde_json::to_string(&response_message) {
            if let Err(e) = gossipsub.publish(message.topic, json.as_bytes()) {
                eprintln!("Failed to publish response: {:?}", e);
            } else {
                println!("Published commit response.");
            }
        }
    }
}