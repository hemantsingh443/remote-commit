use git2::{Repository, Signature, Oid}; // Removed unused Commit import
use std::path::Path;
use std::fs;
use anyhow::{Result, Context};

/// Performs a full add-and-commit cycle for a single file.
pub fn perform_commit(
    repo_path_str: &str,
    file_to_commit_str: &str,
    new_content: &str,
    commit_message: &str,
) -> Result<Oid> { // Returns the Oid (hash) of the new commit on success

    // 1. Open the repository
    let repo = Repository::open(repo_path_str)
        .with_context(|| format!("Failed to open repository at {}", repo_path_str))?;
    
    // 2. Write the new content to the file inside the repository's working directory
    let repo_path = Path::new(repo_path_str);
    let file_path = repo_path.join(file_to_commit_str);
    fs::write(&file_path, new_content)
        .with_context(|| format!("Failed to write to file {:?}", file_path))?;

    // 3. Get the repository index (the staging area)
    let mut index = repo.index()
        .with_context(|| "Failed to get repository index")?;

    // 4. Stage the file
    let file_path_in_repo = Path::new(file_to_commit_str);
    index.add_path(file_path_in_repo)
        .with_context(|| format!("Failed to add file to index: {:?}", file_path_in_repo))?;
    index.write()
        .with_context(|| "Failed to write index to disk")?;
    let tree_id = index.write_tree()
        .with_context(|| "Failed to write index as tree")?;
    let tree = repo.find_tree(tree_id)
        .with_context(|| "Failed to find the new tree in repository")?;

    // 5. Find the parent commit (the current HEAD)
    let head = repo.head()
        .with_context(|| "Failed to get repository HEAD")?;
    let parent_commit = head.peel_to_commit()
        .with_context(|| "Failed to peel HEAD to a commit")?;

    // 6. Create the signature for the commit
    // In a real app, you'd get this from Git config. We'll hardcode it for now.
    let signature = Signature::now("Emergency Committer", "emergency@example.com")?;

    // 7. Create the commit
    let new_commit_oid = repo.commit(
        Some("HEAD"), // Update HEAD to point to this new commit
        &signature,   // Author
        &signature,   // Committer
        commit_message,
        &tree,
        &[&parent_commit], // Array of parent commits
    )?;

    println!("Successfully created commit: {}", new_commit_oid);
    Ok(new_commit_oid)
}