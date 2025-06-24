fn main() {
    let repo_path = "/tmp/test-repo";
    let file_path = "README.md";
    let new_content = "This is an emergency edit!";
    let message = "EMERGENCY: Fix typo in README";
    
    match git_actor::perform_commit(repo_path, file_path, new_content, message) {
        Ok(oid) => println!("Success! New commit hash: {}", oid),
        Err(e) => eprintln!("Error: {:?}", e),
    }
}