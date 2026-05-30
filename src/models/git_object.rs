use chrono::NaiveDateTime;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
}

impl std::fmt::Display for GitObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitObjectType::Commit => write!(f, "commit"),
            GitObjectType::Tree => write!(f, "tree"),
            GitObjectType::Blob => write!(f, "blob"),
            GitObjectType::Tag => write!(f, "tag"),
        }
    }
}

impl std::str::FromStr for GitObjectType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "commit" => Ok(GitObjectType::Commit),
            "tree" => Ok(GitObjectType::Tree),
            "blob" => Ok(GitObjectType::Blob),
            "tag" => Ok(GitObjectType::Tag),
            _ => Err(format!("unknown git object type: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitObject {
    pub obj_type: GitObjectType,
    pub hash: String,
    pub size: usize,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub tree_hash: String,
    pub parent_hashes: Vec<String>,
    pub author: AuthorInfo,
    pub committer: AuthorInfo,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AuthorInfo {
    pub name: String,
    pub email: String,
    pub timestamp: NaiveDateTime,
    pub timezone_offset: i32,
}

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: String,
    pub entry_type: TreeEntryType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEntryType {
    Blob,
    Tree,
    Commit,
}

#[derive(Debug, Clone)]
pub struct Tree {
    pub hash: String,
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub name: String,
    pub commit_hash: String,
    pub is_head: bool,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Debug, Clone)]
pub struct DiffStat {
    pub commit_hash: String,
    pub changes: Vec<FileChange>,
    pub total_additions: u64,
    pub total_deletions: u64,
}

pub trait Repository: Send + Sync {
    fn get_commit(&self, hash: &str) -> Result<Commit>;
    fn get_tree(&self, hash: &str) -> Result<Tree>;
    fn get_branches(&self) -> Result<Vec<Branch>>;
    fn get_head_commit(&self) -> Result<Commit>;
    fn get_all_commits(&self) -> Result<Vec<Commit>>;
}

#[derive(Debug, thiserror::Error)]
pub enum GitVizError {
    #[error("object not found: {0}")]
    ObjectNotFound(String),

    #[error("invalid object format: {0}")]
    InvalidFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("decompression error: {0}")]
    Decompression(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("not a git repository: {0}")]
    NotAGitRepo(String),

    #[error("terminal error: {0}")]
    Terminal(String),
}

pub type Result<T> = std::result::Result<T, GitVizError>;
