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
    common_dir: PathBuf,
    reader: ObjectReader,
}

impl GitRepository {
    pub fn open(path: &Path) -> Result<Self> {
        let dot_git = path.join(".git");
        let git_dir = if dot_git.is_dir() {
            dot_git
        } else if dot_git.is_file() {
            resolve_gitdir_file(path, &dot_git)?
        } else if path.file_name().map(|n| n == ".git").unwrap_or(false) {
            if path.is_file() {
                let parent = path.parent().ok_or_else(|| {
                    GitVizError::InvalidFormat(format!(
                        "cannot resolve parent for {}",
                        path.display()
                    ))
                })?;
                resolve_gitdir_file(parent, path)?
            } else {
                path.to_path_buf()
            }
        } else {
            return Err(GitVizError::NotAGitRepo(path.display().to_string()));
        };

        if !git_dir.exists() {
            return Err(GitVizError::NotAGitRepo(path.display().to_string()));
        }

        let common_dir = resolve_common_dir(&git_dir)?;
        let reader = ObjectReader::new(&git_dir, &common_dir);

        Ok(GitRepository {
            git_dir,
            common_dir,
            reader,
        })
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
        self.reader
            .read_ref(&self.git_dir, &self.common_dir, ref_path)
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
                let commit_hash = self.resolve_ref(&format!("refs/heads/{}", full_name))?;
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

    fn list_packed_head_refs(&self) -> Result<Vec<Branch>> {
        let packed_refs_path = self.common_dir.join("packed-refs");
        if !packed_refs_path.exists() {
            return Ok(Vec::new());
        }

        let head_hash = self.get_head_hash().ok();
        let mut branches = Vec::new();
        let packed_refs = std::fs::read_to_string(packed_refs_path)?;

        for line in packed_refs.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
                continue;
            }

            let mut parts = line.split_whitespace();
            let commit_hash = parts.next();
            let ref_name = parts.next();

            if let (Some(commit_hash), Some(ref_name)) = (commit_hash, ref_name) {
                if let Some(branch_name) = ref_name.strip_prefix("refs/heads/") {
                    branches.push(Branch {
                        name: branch_name.to_string(),
                        commit_hash: commit_hash.to_string(),
                        is_head: head_hash.as_deref() == Some(commit_hash),
                    });
                }
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
        let heads_dir = self.common_dir.join("refs").join("heads");
        let mut branches = self.list_refs(&heads_dir, "")?;

        let mut seen = std::collections::HashSet::new();
        for branch in &branches {
            seen.insert(branch.name.clone());
        }

        for branch in self.list_packed_head_refs()? {
            if seen.insert(branch.name.clone()) {
                branches.push(branch);
            }
        }

        let head_hash = self.get_head_hash().ok();
        let head_target = self.resolve_ref("HEAD").ok();

        if let Some(target) = &head_target {
            let is_detached = !branches.iter().any(|b| &b.commit_hash == target);

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

fn resolve_gitdir_file(repo_root: &Path, git_file: &Path) -> Result<PathBuf> {
    let content = std::fs::read_to_string(git_file)?;
    let line = content.lines().next().unwrap_or("").trim();
    let rel = line.strip_prefix("gitdir:").map(str::trim).ok_or_else(|| {
        GitVizError::InvalidFormat(format!("invalid .git file format: {}", git_file.display()))
    })?;

    let candidate = PathBuf::from(rel);
    if candidate.is_absolute() {
        Ok(candidate)
    } else {
        Ok(repo_root.join(candidate))
    }
}

fn resolve_common_dir(git_dir: &Path) -> Result<PathBuf> {
    let commondir_file = git_dir.join("commondir");
    if !commondir_file.exists() {
        return Ok(git_dir.to_path_buf());
    }

    let rel = std::fs::read_to_string(&commondir_file)?;
    let rel = rel.trim();
    if rel.is_empty() {
        return Ok(git_dir.to_path_buf());
    }

    let path = PathBuf::from(rel);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(git_dir.join(path))
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
