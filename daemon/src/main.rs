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
    kad::{self, store::MemoryStore},
    identify,
    relay,
    Multiaddr,
};
use futures::StreamExt; // Required for select_next_some()
use tokio::select;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use pico_args;
use serde_json;
use tokio::time::{sleep, Duration};

mod protocol;
use protocol::{CommitRequest, CommitResponse, NetworkMessage};

// --- NEW: A struct to manage our trusted peers ---
struct PeerManager {
    trusted_peers_path: PathBuf,
    trusted_peers: HashSet<PeerId>,
}

impl PeerManager {
    fn new() -> anyhow::Result<Self> {
        let path = PathBuf::from("trusted_peers.json");
        let peers = if path.exists() {
            let file_content = fs::read_to_string(&path)?;
            serde_json::from_str(&file_content)?
        } else {
            HashSet::new()
        };
        println!("Loaded {} trusted peers.", peers.len());
        Ok(Self { trusted_peers_path: path, trusted_peers: peers })
    }

    fn is_trusted(&self, peer_id: &PeerId) -> bool {
        self.trusted_peers.contains(peer_id)
    }

    fn add_trusted_peer(&mut self, peer_id: PeerId) -> anyhow::Result<()> {
        self.trusted_peers.insert(peer_id);
        let json = serde_json::to_string_pretty(&self.trusted_peers)?;
        fs::write(&self.trusted_peers_path, json)?;
        println!("Added new trusted peer: {}. Total: {}", peer_id, self.trusted_peers.len());
        Ok(())
    }
}

// This derive macro will now work correctly with proper imports
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "DaemonBehaviourEvent")]
struct DaemonBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    identify: identify::Behaviour,
    relay: relay::Behaviour,
    kademlia: kad::Kademlia<MemoryStore>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // --- NEW: Parse command-line arguments ---
    let mut args = pico_args::Arguments::from_env();
    let is_pairing_mode = args.contains("--pair");

    let mut peer_manager = PeerManager::new()?;
    let id_keys = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(id_keys.public());
    println!("------------------------------------------------------");
    println!("Daemon Peer ID: {}", local_peer_id);
    if is_pairing_mode {
        println!("DAEMON IS IN PAIRING MODE.");
        println!("Client can now send a pair request.");
    } else {
        println!("DAEMON IS IN STANDARD MODE.");
        println!("Run with --pair to allow new clients to connect.");
    }
    println!("------------------------------------------------------");

    // Build transport manually since development_transport might not be available
    // with your current feature set
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&id_keys)?)
        .multiplex(yamux::Config::default())
        .boxed();

    let topic = gossipsub::IdentTopic::new("emergency-git-commits");

    // Use the modern SwarmBuilder API
    let mut swarm = {
        // --- Kademlia Setup ---
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
        // --- End Kademlia Setup ---
        let behaviour = DaemonBehaviour {
            gossipsub: {
                let gossipsub_config = gossipsub::Config::default();
                gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(id_keys.clone()),
                    gossipsub_config,
                )
                .map_err(|e| anyhow!(e))?
            },
            mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?,
            identify: identify::Behaviour::new(identify::Config::new(
                "/emergency-git/1.0".into(),
                id_keys.public(),
            )),
            relay: relay::Behaviour::new(local_peer_id, Default::default()),
            kademlia,
        };
        SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
    };

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
                    propagation_source: source_peer,
                    message,
                    ..
                })) => {
                    let source_peer = match message.source {
                        Some(peer_id) => peer_id,
                        None => continue, // Ignore anonymous messages
                    };
                    match serde_json::from_slice::<NetworkMessage>(&message.data) {
                        Ok(NetworkMessage::PairRequest) => {
                            if is_pairing_mode {
                                handle_pair_request(source_peer, &mut peer_manager, topic.clone(), &mut swarm.behaviour_mut().gossipsub).await;
                            } else {
                                println!("Ignoring pair request from {}. Daemon not in --pair mode.", source_peer);
                            }
                        }
                        Ok(NetworkMessage::Request(request)) => {
                            if peer_manager.is_trusted(&source_peer) {
                                println!("Received trusted commit request from {}", source_peer);
                                handle_commit_request(request, topic.clone(), &mut swarm.behaviour_mut().gossipsub);
                            } else {
                                println!("IGNORING untrusted commit request from {}", source_peer);
                            }
                        }
                        _ => {}
                    }
                }
                SwarmEvent::Behaviour(DaemonBehaviourEvent::Identify(identify::Event::Received {
                    peer_id,
                    info,
                })) => {
                    println!("[Identify] Received info from peer: {}", peer_id);
                    println!("[Identify] Their observed address: {}", info.observed_addr);
                    println!("[Identify] Their listen addresses: {:?}", info.listen_addrs);

                    // Add their listen addresses to Kademlia so we can find them later.
                    for addr in info.listen_addrs {
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                    }
                },
                SwarmEvent::Behaviour(DaemonBehaviourEvent::Identify(identify::Event::Pushed { peer_id, .. })) => {
                    println!("[Identify] Pushed our info to peer: {}", peer_id);
                    // Let's log our current known external addresses
                    println!("\n✅✅✅ DAEMON'S POTENTIAL PUBLIC ADDRESSES ✅✅✅");
                    println!("Copy one of these full addresses for the client:");
                    for addr_record in swarm.external_addresses() {
                        println!(
                            "  -> {}",
                            addr_record.addr.clone().with(libp2p::multiaddr::Protocol::P2p(local_peer_id.into()))
                        );
                    }
                    println!("✅✅✅ --- END OF ADDRESSES --- ✅✅✅\n");
                },
                _ => {}
            }
        }
    }
}

// --- NEW: Handler for pairing ---
async fn handle_pair_request(
    peer_id: PeerId,
    peer_manager: &mut PeerManager,
    topic: gossipsub::IdentTopic,
    gossipsub: &mut gossipsub::Behaviour,
) {
    println!("Pairing request received from {}. Approve? (y/n): ", peer_id);
    io::stdout().flush().unwrap();

    let approved = tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        io::stdin().read_line(&mut line).is_ok() && line.trim().eq_ignore_ascii_case("y")
    }).await.unwrap_or(false);

    if approved {
        if let Err(e) = peer_manager.add_trusted_peer(peer_id) {
            eprintln!("[ERROR] Failed to save trusted peer: {}", e);
            return;
        }

        let response = NetworkMessage::PairSuccess;
        if let Ok(json) = serde_json::to_string(&response) {
            let max_retries = 5;
            for i in 0..max_retries {
                match gossipsub.publish(topic.clone(), json.as_bytes()) {
                    Ok(_) => {
                        println!("[INFO] Published PairSuccess response.");
                        return;
                    }
                    Err(e) if i < max_retries - 1 => {
                        eprintln!("[WARN] Failed to publish reply (attempt {}): {}. Retrying...", i + 1, e);
                        sleep(Duration::from_millis(500)).await;
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to publish pairing success message after all retries: {:?}", e);
                        return;
                    }
                }
            }
        }
    } else {
        println!("[INFO] Pairing for {} denied.", peer_id);
    }
}

// --- MODIFIED: Handler for commits ---
fn handle_commit_request(request: CommitRequest, topic: gossipsub::IdentTopic, gossipsub: &mut gossipsub::Behaviour) {
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
        if let Err(e) = gossipsub.publish(topic, json.as_bytes()) {
            eprintln!("Failed to publish response: {:?}", e);
        } else {
            println!("Published commit response.");
        }
    }
}