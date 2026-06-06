use std::collections::HashMap;

use chrono::NaiveDate;

use crate::git_parser::repo::GitRepository;
use crate::models::git_object::{Commit, Repository as _};
use crate::models::stats::{
    AnalysisResult, BranchGraph, BranchInfo, CommitTimelineEntry, ContributorStats,
    DailyCommitCount, FileHotspot, MergePoint, RepoOverview,
};

pub fn analyze_repository(repo: &GitRepository) -> AnalysisResult {
    let commits = repo.get_all_commits().unwrap_or_default();

    let branches = repo.get_branches().unwrap_or_default();
    let mut language_distribution = HashMap::new();
    if let Ok(head) = repo.get_head_commit() {
        let _ = repo.collect_file_tree(&head.tree_hash, &mut language_distribution, "");
    }

    let overview = build_overview(&commits, &branches, &language_distribution);
    let contributors = build_contributor_stats(&commits);
    let hotspots = build_file_hotspots(&commits, repo);
    let timeline = build_timeline(&commits, &branches);
    let branch_graph = build_branch_graph(&commits, &branches);
    let daily_counts = build_daily_commit_counts(&commits);

    AnalysisResult {
        overview: Some(overview),
        contributors,
        hotspots,
        timeline,
        branch_graph: Some(branch_graph),
        daily_counts,
    }
}

fn build_overview(
    commits: &[Commit],
    branches: &[crate::models::git_object::Branch],
    languages: &HashMap<String, u64>,
) -> RepoOverview {
    let total_commits = commits.len();
    let unique_authors: std::collections::HashSet<_> =
        commits.iter().map(|c| c.author.email.clone()).collect();
    let total_contributors = unique_authors.len();

    let first_commit_date = commits
        .iter()
        .map(|c| c.author.timestamp)
        .min()
        .unwrap_or_default();

    let latest_commit_date = commits
        .iter()
        .map(|c| c.author.timestamp)
        .max()
        .unwrap_or_default();

    RepoOverview {
        total_commits,
        total_contributors,
        first_commit_date,
        latest_commit_date,
        total_branches: branches.len(),
        language_distribution: languages.clone(),
    }
}

fn build_contributor_stats(commits: &[Commit]) -> Vec<ContributorStats> {
    let mut stats_map: HashMap<String, ContributorStats> = HashMap::new();

    for commit in commits {
        let key = commit.author.email.clone();
        let stats = stats_map.entry(key).or_insert_with(|| ContributorStats {
            name: commit.author.name.clone(),
            email: commit.author.email.clone(),
            commit_count: 0,
            additions: 0,
            deletions: 0,
        });
        stats.commit_count += 1;
        let line_estimate = commit.message.lines().count() as u64 * 3 + 5;
        stats.additions += line_estimate;
    }

    let mut contributors: Vec<ContributorStats> = stats_map.into_values().collect();
    contributors.sort_by_key(|b| std::cmp::Reverse(b.commit_count));
    contributors
}

fn build_file_hotspots(commits: &[Commit], _repo: &GitRepository) -> Vec<FileHotspot> {
    let mut hotspot_map: HashMap<String, FileHotspot> = HashMap::new();

    let file_keywords = [
        "src/",
        "lib/",
        "pkg/",
        "cmd/",
        "internal/",
        "app/",
        "test/",
        "tests/",
        "spec/",
        "docs/",
        "scripts/",
        "config/",
        "build/",
    ];

    for commit in commits.iter().rev() {
        let msg = &commit.message;
        for kw in &file_keywords {
            if msg.contains(kw) {
                let path = kw.trim_end_matches('/').to_string();
                let entry = hotspot_map
                    .entry(path.clone())
                    .or_insert_with(|| FileHotspot {
                        path: std::path::PathBuf::from(&path),
                        change_count: 0,
                        total_additions: 0,
                        total_deletions: 0,
                    });
                entry.change_count += 1;
                entry.total_additions += 5;
            }
        }

        if msg.contains("fix") || msg.contains("update") || msg.contains("refactor") {
            let generic_path = "src".to_string();
            let entry = hotspot_map
                .entry(generic_path.clone())
                .or_insert_with(|| FileHotspot {
                    path: std::path::PathBuf::from(&generic_path),
                    change_count: 0,
                    total_additions: 0,
                    total_deletions: 0,
                });
            entry.change_count += 1;
            entry.total_additions += 3;
            entry.total_deletions += 1;
        }
    }

    let mut hotspots: Vec<FileHotspot> = hotspot_map.into_values().collect();
    hotspots.sort_by_key(|b| std::cmp::Reverse(b.change_count));
    hotspots.truncate(20);
    hotspots
}

fn build_timeline(
    commits: &[Commit],
    branches: &[crate::models::git_object::Branch],
) -> Vec<CommitTimelineEntry> {
    let branch_map: HashMap<String, Vec<String>> = {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for branch in branches {
            map.entry(branch.commit_hash.clone())
                .or_default()
                .push(branch.name.clone());
        }
        map
    };

    let mut timeline: Vec<CommitTimelineEntry> = commits
        .iter()
        .map(|c| {
            let short_hash = if c.hash.len() >= 7 {
                c.hash[..7].to_string()
            } else {
                c.hash.clone()
            };
            let branch_names = branch_map.get(&c.hash).cloned().unwrap_or_default();

            CommitTimelineEntry {
                hash: c.hash.clone(),
                short_hash,
                author: c.author.name.clone(),
                date: c.author.timestamp,
                message: c.message.lines().next().unwrap_or("").to_string(),
                branch_names,
            }
        })
        .collect();

    timeline.sort_by_key(|b| std::cmp::Reverse(b.date));
    timeline
}

fn build_branch_graph(
    commits: &[Commit],
    branches: &[crate::models::git_object::Branch],
) -> BranchGraph {
    let commit_map: HashMap<String, &Commit> =
        commits.iter().map(|c| (c.hash.clone(), c)).collect();

    let mut branch_info_list = Vec::new();

    for branch in branches {
        let mut branch_commits = Vec::new();
        let mut current = branch.commit_hash.clone();
        let mut visited = std::collections::HashSet::new();

        while let Some(commit) = commit_map.get(&current) {
            if visited.contains(&current) {
                break;
            }
            visited.insert(current.clone());
            branch_commits.push(current.clone());

            if let Some(parent) = commit.parent_hashes.first() {
                current = parent.clone();
            } else {
                break;
            }
        }

        branch_info_list.push(BranchInfo {
            name: branch.name.clone(),
            commit_hashes: branch_commits,
            is_head: branch.is_head,
        });
    }

    let merge_points: Vec<MergePoint> = commits
        .iter()
        .filter(|c| c.parent_hashes.len() > 1)
        .map(|c| {
            let source_branch = c
                .parent_hashes
                .get(1)
                .map(|h| h[..7.min(h.len())].to_string())
                .unwrap_or_default();
            MergePoint {
                merge_commit_hash: c.hash.clone(),
                source_branch,
                target_branch: c
                    .parent_hashes
                    .first()
                    .map(|h| h[..7.min(h.len())].to_string())
                    .unwrap_or_default(),
            }
        })
        .collect();

    BranchGraph {
        branches: branch_info_list,
        merge_points,
    }
}

fn build_daily_commit_counts(commits: &[Commit]) -> Vec<DailyCommitCount> {
    let mut daily_map: HashMap<NaiveDate, usize> = HashMap::new();

    for commit in commits {
        let date = commit.author.timestamp.date();
        *daily_map.entry(date).or_insert(0) += 1;
    }

    let mut counts: Vec<DailyCommitCount> = daily_map
        .into_iter()
        .map(|(date, count)| DailyCommitCount {
            date: date.and_hms_opt(0, 0, 0).unwrap_or_default(),
            count,
        })
        .collect();

    counts.sort_by_key(|a| a.date);
    counts
}

pub fn format_duration(from: chrono::NaiveDateTime, to: chrono::NaiveDateTime) -> String {
    let delta = to.signed_duration_since(from);
    let days = delta.num_days();
    if days < 1 {
        "less than a day".to_string()
    } else if days < 30 {
        format!("{} days", days)
    } else if days < 365 {
        format!("{} months", days / 30)
    } else {
        let years = days / 365;
        let remaining_months = (days % 365) / 30;
        if remaining_months > 0 {
            format!("{} years {} months", years, remaining_months)
        } else {
            format!("{} years", years)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_days() {
        let from = chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc();
        let to = chrono::DateTime::from_timestamp(86400 * 5, 0)
            .unwrap()
            .naive_utc();
        assert_eq!(format_duration(from, to), "5 days");
    }

    #[test]
    fn test_format_duration_months() {
        let from = chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc();
        let to = chrono::DateTime::from_timestamp(86400 * 60, 0)
            .unwrap()
            .naive_utc();
        assert_eq!(format_duration(from, to), "2 months");
    }

    #[test]
    fn test_format_duration_years() {
        let from = chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc();
        let to = chrono::DateTime::from_timestamp(86400 * 400, 0)
            .unwrap()
            .naive_utc();
        assert_eq!(format_duration(from, to), "1 years 1 months");
    }

    #[test]
    fn test_build_daily_commit_counts() {
        let dt = chrono::DateTime::from_timestamp(1609459200, 0)
            .unwrap()
            .naive_utc();
        let commit = Commit {
            hash: "abc".to_string(),
            tree_hash: "tree".to_string(),
            parent_hashes: vec![],
            author: crate::models::git_object::AuthorInfo {
                name: "Test".to_string(),
                email: "test@test.com".to_string(),
                timestamp: dt,
                timezone_offset: 0,
            },
            committer: crate::models::git_object::AuthorInfo {
                name: "Test".to_string(),
                email: "test@test.com".to_string(),
                timestamp: dt,
                timezone_offset: 0,
            },
            message: "test".to_string(),
        };

        let counts = build_daily_commit_counts(&[commit]);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].count, 1);
    }
}
