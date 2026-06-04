use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::git_object::{Commit, FileChange};

pub fn compute_diff_stat(
    current_tree_files: &HashMap<PathBuf, String>,
    parent_tree_files: &HashMap<PathBuf, String>,
) -> DiffStatResult {
    let mut changes = Vec::new();
    let mut total_additions = 0u64;
    let mut total_deletions = 0u64;

    for (path, content) in current_tree_files {
        match parent_tree_files.get(path) {
            Some(old_content) => {
                if content != old_content {
                    let (add, del) = count_line_diff(old_content, content);
                    total_additions += add;
                    total_deletions += del;
                    changes.push(FileChange {
                        path: path.clone(),
                        additions: add,
                        deletions: del,
                    });
                }
            }
            None => {
                let add = content.lines().count() as u64;
                total_additions += add;
                changes.push(FileChange {
                    path: path.clone(),
                    additions: add,
                    deletions: 0,
                });
            }
        }
    }

    for (path, content) in parent_tree_files {
        if !current_tree_files.contains_key(path) {
            let del = content.lines().count() as u64;
            total_deletions += del;
            changes.push(FileChange {
                path: path.clone(),
                additions: 0,
                deletions: del,
            });
        }
    }

    DiffStatResult {
        changes,
        total_additions,
        total_deletions,
    }
}

fn count_line_diff(old: &str, new: &str) -> (u64, u64) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let lcs_len = longest_common_subsequence_len(&old_lines, &new_lines);
    let additions = new_lines.len().saturating_sub(lcs_len) as u64;
    let deletions = old_lines.len().saturating_sub(lcs_len) as u64;

    (additions, deletions)
}

fn longest_common_subsequence_len<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let m = a.len();
    let n = b.len();

    if m == 0 || n == 0 {
        return 0;
    }

    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = curr[j - 1].max(prev[j]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

pub struct DiffStatResult {
    pub changes: Vec<FileChange>,
    pub total_additions: u64,
    pub total_deletions: u64,
}

pub fn estimate_commit_changes(
    commit: &Commit,
    parent_tree_files: &[HashMap<PathBuf, String>],
) -> (u64, u64) {
    let mut total_add = 0u64;
    let mut total_del = 0u64;

    for parent_files in parent_tree_files {
        if parent_files.is_empty() {
            total_add += 1;
        } else {
            total_add += parent_files.len() as u64 / 10 + 1;
            total_del += parent_files.len() as u64 / 20;
        }
    }

    if commit.parent_hashes.is_empty() {
        total_add = total_add.max(1);
        total_del = 0;
    }

    (total_add, total_del)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lcs_len() {
        assert_eq!(longest_common_subsequence_len(&[1, 2, 3], &[1, 2, 3]), 3);
        assert_eq!(longest_common_subsequence_len(&[1, 2, 3], &[4, 5, 6]), 0);
        assert_eq!(longest_common_subsequence_len(&[1, 2, 3], &[2, 3, 4]), 2);
        assert_eq!(longest_common_subsequence_len::<i32>(&[], &[]), 0);
    }

    #[test]
    fn test_count_line_diff_identical() {
        let (add, del) = count_line_diff("hello\nworld", "hello\nworld");
        assert_eq!(add, 0);
        assert_eq!(del, 0);
    }

    #[test]
    fn test_count_line_diff_additions() {
        let (add, del) = count_line_diff("hello", "hello\nworld");
        assert!(add > 0);
        assert_eq!(del, 0);
    }

    #[test]
    fn test_compute_diff_stat_new_file() {
        let current: HashMap<PathBuf, String> = {
            let mut m = HashMap::new();
            m.insert(PathBuf::from("new.txt"), "content".to_string());
            m
        };
        let parent = HashMap::new();

        let result = compute_diff_stat(&current, &parent);
        assert_eq!(result.changes.len(), 1);
        assert!(result.total_additions > 0);
    }
}
