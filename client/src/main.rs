// This is now just a test runner for our library
use pico_args;
use mobile_core::{emergency_commit_async, pair_async};
use libp2p::Multiaddr;

#[tokio::main]
async fn main() {
    let mut args = pico_args::Arguments::from_env();
    // 1. Run the daemon first and get its full address
    let daemon_full_addr_str = "/ip4/172.20.128.55/tcp/35809/p2p/12D3KooWMzd2tGd9pWxQDQz6C9cy9QHaGLndPMLNjC7avBGJrp4F";
    if args.contains("--pair") {
        println!("--- Running Client in Pairing Mode ---");
        match pair_async(daemon_full_addr_str.to_string()).await {
            Ok(_) => println!("✅ Pairing successful! Daemon has approved this client."),
            Err(e) => eprintln!("❌ Pairing failed: {}", e),
        }
    } else {
        println!("--- Running Client in Commit Mode ---");
        let repo_path = "/tmp/test-repo".to_string();
        let file_path = "README.md".to_string();
        let new_content = "This commit came from the new MOBILE CORE library!".to_string();
        let message = "refactor: Logic moved to mobile-core library".to_string();
        let result = emergency_commit_async(
            daemon_full_addr_str.to_string(),
            repo_path,
            file_path,
            new_content,
            message,
        ).await;
        match result {
            Ok(commit_hash) => {
                println!("\n✅ SUCCESS!");
                println!("New commit hash: {}", commit_hash);
            }
            Err(e) => {
                eprintln!("\n❌ FAILURE!");
                eprintln!("Error: {}", e);
            }
        }
    }
}