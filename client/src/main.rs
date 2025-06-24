// This is now just a test runner for our library
use pico_args;
use mobile_core::{emergency_commit_async, pair_async};

#[tokio::main]
async fn main() {
    let mut args = pico_args::Arguments::from_env();
    if args.contains("--pair") {
        println!("--- Running Client in Pairing Mode ---");
        match pair_async().await {
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