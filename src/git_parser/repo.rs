use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::git_parser::commit_parser;
use crate::git_parser::object_reader::ObjectReader;
use crate::git_parser::tree_parser;
use crate::models::git_object::{
    Branch, Commit, GitVizError, Repository, Result, Tree, TreeEntryType,
};

pub struct GitRepository {
    git_dir: PathBuf,
    reader: ObjectReader,
}

impl GitRepository {
    pub fn open(path: &Path) -> Result<Self> {
        let git_dir = if path.join(".git").exists() {
            path.join(".git")
        } else if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            path.to_path_buf()
        } else {
            return Err(GitVizError::NotAGitRepo(path.display().to_string()));
        };

        if !git_dir.exists() {
            return Err(GitVizError::NotAGitRepo(path.display().to_string()));
        }

        let reader = ObjectReader::new(&git_dir);
        Ok(GitRepository { git_dir, reader })
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn read_object_content(
        &self,
        hash: &str,
    ) -> Result<(crate::models::git_object::GitObjectType, Vec<u8>)> {
        let obj = self.reader.read_object(hash)?;
        Ok((obj.obj_type, obj.content))
    }

    fn resolve_ref(&self, ref_path: &str) -> Result<String> {
        self.reader.read_ref(&self.git_dir, ref_path)
    }

    pub fn get_head_hash(&self) -> Result<String> {
        self.resolve_ref("HEAD")
    }

    fn list_refs(&self, ref_dir: &Path, prefix: &str) -> Result<Vec<Branch>> {
        let mut branches = Vec::new();
        if !ref_dir.exists() {
            return Ok(branches);
        }

        for entry in std::fs::read_dir(ref_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let full_name = format!("{}{}", prefix, name);

            if entry.file_type()?.is_dir() {
                let sub_prefix = format!("{}/", full_name);
                branches.extend(self.list_refs(&entry.path(), &sub_prefix)?);
            } else {
                let commit_hash = self.resolve_ref(&format!("refs/{}", full_name))?;
                let head_hash = self.get_head_hash().ok();
                let is_head = head_hash.as_deref() == Some(commit_hash.as_str());

                branches.push(Branch {
                    name: full_name,
                    commit_hash,
                    is_head,
                });
            }
        }

        Ok(branches)
    }

    pub fn collect_file_tree(
        &self,
        tree_hash: &str,
        languages: &mut HashMap<String, u64>,
        prefix: &str,
    ) -> Result<()> {
        let tree = self.get_tree(tree_hash)?;

        for entry in &tree.entries {
            let full_path = if prefix.is_empty() {
                entry.name.clone()
            } else {
                format!("{}/{}", prefix, entry.name)
            };

            match entry.entry_type {
                TreeEntryType::Tree => {
                    self.collect_file_tree(&entry.hash, languages, &full_path)?;
                }
                TreeEntryType::Blob => {
                    if let Some(lang) = tree_parser::detect_language(&entry.name) {
                        *languages.entry(lang.to_string()).or_insert(0) += 1;
                    }
                }
                TreeEntryType::Commit => {}
            }
        }

        Ok(())
    }
}

impl Repository for GitRepository {
    fn get_commit(&self, hash: &str) -> Result<Commit> {
        let obj = self.reader.read_object(hash)?;
        if obj.obj_type != crate::models::git_object::GitObjectType::Commit {
            return Err(GitVizError::InvalidFormat(format!(
                "expected commit, got {}",
                obj.obj_type
            )));
        }
        commit_parser::parse_commit(&obj.content, hash)
    }

    fn get_tree(&self, hash: &str) -> Result<Tree> {
        let obj = self.reader.read_object(hash)?;
        if obj.obj_type != crate::models::git_object::GitObjectType::Tree {
            return Err(GitVizError::InvalidFormat(format!(
                "expected tree, got {}",
                obj.obj_type
            )));
        }
        tree_parser::parse_tree(&obj.content, hash)
    }

    fn get_branches(&self) -> Result<Vec<Branch>> {
        let heads_dir = self.git_dir.join("refs").join("heads");
        let mut branches = self.list_refs(&heads_dir, "")?;

        let head_hash = self.get_head_hash().ok();
        let head_target = self.resolve_ref("HEAD").ok();

        if let Some(target) = &head_target {
            let is_detached = !self.git_dir.join("refs").join("heads").exists()
                || !branches.iter().any(|b| &b.commit_hash == target);

            if is_detached && !branches.iter().any(|b| b.name == "HEAD (detached)") {
                branches.push(Branch {
                    name: "HEAD (detached)".to_string(),
                    commit_hash: target.clone(),
                    is_head: true,
                });
            }
        }

        for branch in &mut branches {
            if let Some(ref head) = head_hash {
                branch.is_head = &branch.commit_hash == head;
            }
        }

        Ok(branches)
    }

    fn get_head_commit(&self) -> Result<Commit> {
        let head_hash = self.get_head_hash()?;
        self.get_commit(&head_hash)
    }

    fn get_all_commits(&self) -> Result<Vec<Commit>> {
        let mut commits = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        let branches = self.get_branches()?;
        for branch in &branches {
            queue.push_back(branch.commit_hash.clone());
        }

        while let Some(hash) = queue.pop_front() {
            if visited.contains(&hash) {
                continue;
            }
            visited.insert(hash.clone());

            match self.get_commit(&hash) {
                Ok(commit) => {
                    for parent in &commit.parent_hashes {
                        if !visited.contains(parent) {
                            queue.push_back(parent.clone());
                        }
                    }
                    commits.push(commit);
                }
                Err(_) => continue,
            }
        }

        Ok(commits)
    }
}

pub fn collect_all_commits_parallel(repo: &GitRepository) -> Result<Vec<Commit>> {
    let all_hashes = {
        let mut hashes = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        let branches = repo.get_branches()?;
        for branch in &branches {
            queue.push_back(branch.commit_hash.clone());
        }

        let mut visited = std::collections::HashSet::new();
        while let Some(hash) = queue.pop_front() {
            if visited.contains(&hash) {
                continue;
            }
            visited.insert(hash.clone());

            match repo.get_commit(&hash) {
                Ok(commit) => {
                    for parent in &commit.parent_hashes {
                        if !visited.contains(parent) {
                            queue.push_back(parent.clone());
                        }
                    }
                    hashes.insert(hash);
                }
                Err(_) => continue,
            }
        }
        hashes.into_iter().collect::<Vec<_>>()
    };

    let commits: Vec<Commit> = all_hashes
        .par_iter()
        .filter_map(|hash| repo.get_commit(hash).ok())
        .collect();

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_open_nonexistent_repo() {
        let result = GitRepository::open(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
