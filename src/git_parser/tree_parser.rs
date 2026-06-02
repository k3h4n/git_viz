use crate::models::git_object::{GitVizError, Result, Tree, TreeEntry, TreeEntryType};

pub fn parse_tree(content: &[u8], hash: &str) -> Result<Tree> {
    let mut entries = Vec::new();
    let mut pos = 0;

    while pos < content.len() {
        let space_pos = content[pos..]
            .iter()
            .position(|&b| b == b' ')
            .ok_or_else(|| GitVizError::InvalidFormat("missing space in tree entry".into()))?
            + pos;

        let mode = String::from_utf8_lossy(&content[pos..space_pos]).to_string();

        let null_pos = content[space_pos + 1..]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| GitVizError::InvalidFormat("missing null in tree entry".into()))?
            + space_pos
            + 1;

        let name = String::from_utf8_lossy(&content[space_pos + 1..null_pos]).to_string();

        if null_pos + 20 > content.len() {
            return Err(GitVizError::InvalidFormat(
                "truncated hash in tree entry".into(),
            ));
        }

        let hash_bytes = &content[null_pos + 1..null_pos + 21];
        let entry_hash: String = hash_bytes.iter().map(|b| format!("{:02x}", b)).collect();

        let entry_type = match mode.as_str() {
            "40000" | "040000" => TreeEntryType::Tree,
            "160000" => TreeEntryType::Commit,
            _ => TreeEntryType::Blob,
        };

        entries.push(TreeEntry {
            mode,
            name,
            hash: entry_hash,
            entry_type,
        });

        pos = null_pos + 21;
    }

    Ok(Tree {
        hash: hash.to_string(),
        entries,
    })
}

pub fn detect_language(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next()?;
    match ext {
        "rs" => Some("Rust"),
        "py" => Some("Python"),
        "js" | "mjs" => Some("JavaScript"),
        "ts" | "tsx" => Some("TypeScript"),
        "jsx" => Some("JSX"),
        "java" => Some("Java"),
        "c" | "h" => Some("C"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("C++"),
        "go" => Some("Go"),
        "rb" => Some("Ruby"),
        "php" => Some("PHP"),
        "swift" => Some("Swift"),
        "kt" => Some("Kotlin"),
        "scala" => Some("Scala"),
        "sh" | "bash" => Some("Shell"),
        "sql" => Some("SQL"),
        "html" | "htm" => Some("HTML"),
        "css" | "scss" | "sass" | "less" => Some("CSS"),
        "md" | "markdown" => Some("Markdown"),
        "json" => Some("JSON"),
        "yaml" | "yml" => Some("YAML"),
        "toml" => Some("TOML"),
        "xml" => Some("XML"),
        "dockerfile" => Some("Dockerfile"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tree_single_entry() {
        let mut content = Vec::new();
        content.extend_from_slice(b"100644 README.md\0");
        let hash: [u8; 20] = [
            0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
            0x67, 0x89, 0xab, 0xcd, 0xef, 0x01,
        ];
        content.extend_from_slice(&hash);

        let tree = parse_tree(&content, "fake_hash").unwrap();
        assert_eq!(tree.entries.len(), 1);
        assert_eq!(tree.entries[0].name, "README.md");
        assert_eq!(tree.entries[0].mode, "100644");
        assert_eq!(tree.entries[0].entry_type, TreeEntryType::Blob);
    }

    #[test]
    fn test_parse_tree_with_directory() {
        let mut content = Vec::new();
        content.extend_from_slice(b"40000 src\0");
        let hash: [u8; 20] = [0u8; 20];
        content.extend_from_slice(&hash);

        let tree = parse_tree(&content, "fake_hash").unwrap();
        assert_eq!(tree.entries[0].entry_type, TreeEntryType::Tree);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("Rust"));
        assert_eq!(detect_language("app.py"), Some("Python"));
        assert_eq!(detect_language("index.js"), Some("JavaScript"));
        assert_eq!(detect_language("Makefile"), None);
    }
}
