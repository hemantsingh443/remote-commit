// This is now just a test runner for our library
#[tokio::main]
async fn main() {
    println!("--- Running Client as Test Harness for Mobile Core ---");

    // The data we want to send
    let repo_path = "/tmp/test-repo".to_string();
    let file_path = "README.md".to_string();
    let new_content = "This commit came from the new MOBILE CORE library!".to_string();
    let message = "refactor: Logic moved to mobile-core library".to_string();

    // Call the library function
    let result = mobile_core::emergency_commit(
        repo_path,
        file_path,
        new_content,
        message,
    ).await;

    // Print the result
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