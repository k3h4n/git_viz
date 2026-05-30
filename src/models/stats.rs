use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RepoOverview {
    pub total_commits: usize,
    pub total_contributors: usize,
    pub first_commit_date: NaiveDateTime,
    pub latest_commit_date: NaiveDateTime,
    pub total_branches: usize,
    pub language_distribution: HashMap<String, u64>,
}

#[derive(Debug, Clone)]
pub struct ContributorStats {
    pub name: String,
    pub email: String,
    pub commit_count: usize,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Debug, Clone)]
pub struct FileHotspot {
    pub path: PathBuf,
    pub change_count: usize,
    pub total_additions: u64,
    pub total_deletions: u64,
}

#[derive(Debug, Clone)]
pub struct CommitTimelineEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: NaiveDateTime,
    pub message: String,
    pub branch_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BranchGraph {
    pub branches: Vec<BranchInfo>,
    pub merge_points: Vec<MergePoint>,
}

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub commit_hashes: Vec<String>,
    pub is_head: bool,
}

#[derive(Debug, Clone)]
pub struct MergePoint {
    pub merge_commit_hash: String,
    pub source_branch: String,
    pub target_branch: String,
}

#[derive(Debug, Clone)]
pub struct DailyCommitCount {
    pub date: NaiveDateTime,
    pub count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisResult {
    pub overview: Option<RepoOverview>,
    pub contributors: Vec<ContributorStats>,
    pub hotspots: Vec<FileHotspot>,
    pub timeline: Vec<CommitTimelineEntry>,
    pub branch_graph: Option<BranchGraph>,
    pub daily_counts: Vec<DailyCommitCount>,
}
