use crate::models::git_object::{AuthorInfo, Commit, GitVizError, Result};

pub fn parse_commit(content: &[u8], hash: &str) -> Result<Commit> {
    let text = String::from_utf8_lossy(content);
    let lines = text.lines();

    let mut tree_hash = String::new();
    let mut parent_hashes = Vec::new();
    let mut author = None;
    let mut committer = None;
    let mut message_lines = Vec::new();
    let mut in_message = false;

    for line in lines {
        if in_message {
            message_lines.push(line);
        } else if line.is_empty() {
            in_message = true;
        } else if let Some(rest) = line.strip_prefix("tree ") {
            tree_hash = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("parent ") {
            parent_hashes.push(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = Some(parse_author_info(rest)?);
        } else if let Some(rest) = line.strip_prefix("committer ") {
            committer = Some(parse_author_info(rest)?);
        }
    }

    let author = author.ok_or_else(|| GitVizError::InvalidFormat("missing author".into()))?;
    let committer =
        committer.ok_or_else(|| GitVizError::InvalidFormat("missing committer".into()))?;

    let message = message_lines
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    Ok(Commit {
        hash: hash.to_string(),
        tree_hash,
        parent_hashes,
        author,
        committer,
        message,
    })
}

pub fn compute_commit_hash(content: &[u8]) -> String {
    let header = format!("commit {}\0", content.len());
    let data: Vec<u8> = header.bytes().chain(content.iter().copied()).collect();
    compute_sha1(&data)
}

fn compute_sha1(data: &[u8]) -> String {
    let mut state: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    let mut padded = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        process_block(&mut state, chunk);
    }

    state
        .iter()
        .flat_map(|&w| w.to_be_bytes())
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn process_block(state: &mut [u32; 5], block: &[u8]) {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *state;

    for (i, w_val) in w.iter().enumerate().take(80) {
        let (f, k) = match i {
            0..=19 => ((b & c) | (!b & d), 0x5A827999u32),
            20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
            _ => (b ^ c ^ d, 0xCA62C1D6u32),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(*w_val);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
}

fn parse_author_info(s: &str) -> Result<AuthorInfo> {
    let s = s.trim();
    let timestamp_start = s
        .rfind(' ')
        .ok_or_else(|| GitVizError::InvalidFormat(format!("cannot parse author info: {}", s)))?;
    let timezone_str = &s[timestamp_start + 1..];
    let before_tz = &s[..timestamp_start];

    let ts_start = before_tz.rfind(' ').ok_or_else(|| {
        GitVizError::InvalidFormat(format!("cannot parse author timestamp: {}", s))
    })?;
    let timestamp_str = &before_tz[ts_start + 1..];
    let name_email = &before_tz[..ts_start];

    let email_start = name_email
        .rfind('<')
        .ok_or_else(|| GitVizError::InvalidFormat(format!("cannot parse email: {}", name_email)))?;
    let email_end = name_email
        .rfind('>')
        .ok_or_else(|| GitVizError::InvalidFormat(format!("cannot parse email: {}", name_email)))?;

    let name = name_email[..email_start].trim().to_string();
    let email = name_email[email_start + 1..email_end].to_string();

    let timestamp: i64 = timestamp_str
        .parse()
        .map_err(|_| GitVizError::InvalidFormat(format!("invalid timestamp: {}", timestamp_str)))?;
    let timezone_offset = parse_timezone(timezone_str)?;

    let naive_dt = chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|dt| dt.naive_utc())
        .ok_or_else(|| GitVizError::Parse(format!("invalid timestamp: {}", timestamp)))?;

    Ok(AuthorInfo {
        name,
        email,
        timestamp: naive_dt,
        timezone_offset,
    })
}

fn parse_timezone(s: &str) -> Result<i32> {
    if s.len() < 5 {
        return Err(GitVizError::InvalidFormat(format!(
            "invalid timezone: {}",
            s
        )));
    }
    let sign: i32 = if s.starts_with('-') { -1 } else { 1 };
    let hours: i32 = s[1..3]
        .parse()
        .map_err(|_| GitVizError::InvalidFormat(format!("invalid tz hours: {}", s)))?;
    let mins: i32 = s[3..5]
        .parse()
        .map_err(|_| GitVizError::InvalidFormat(format!("invalid tz minutes: {}", s)))?;
    Ok(sign * (hours * 60 + mins))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha1_known_value() {
        let result = compute_sha1(b"abc");
        assert_eq!(result, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn test_parse_timezone() {
        assert_eq!(parse_timezone("+0800").unwrap(), 480);
        assert_eq!(parse_timezone("-0500").unwrap(), -300);
        assert_eq!(parse_timezone("+0000").unwrap(), 0);
    }

    #[test]
    fn test_parse_commit_basic() {
        let content = b"tree abc123\nauthor John Doe <john@example.com> 1609459200 +0800\ncommitter John Doe <john@example.com> 1609459200 +0800\n\nInitial commit";
        let commit = parse_commit(content, "fake_hash").unwrap();
        assert_eq!(commit.hash, "fake_hash");
        assert_eq!(commit.author.name, "John Doe");
        assert_eq!(commit.author.email, "john@example.com");
        assert_eq!(commit.message, "Initial commit");
        assert!(commit.parent_hashes.is_empty());
    }

    #[test]
    fn test_parse_commit_with_parents() {
        let content = b"tree abc123\nparent def456\nparent ghi789\nauthor A <a@b.com> 1609459200 +0800\ncommitter A <a@b.com> 1609459200 +0800\n\nMerge commit";
        let commit = parse_commit(content, "merge_hash").unwrap();
        assert_eq!(commit.parent_hashes.len(), 2);
        assert_eq!(commit.message, "Merge commit");
    }

    #[test]
    fn test_compute_commit_hash() {
        let content =
            b"tree abc\nauthor A <a@a.com> 0 +0000\ncommitter A <a@a.com> 0 +0000\n\ntest";
        let hash = compute_commit_hash(content);
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
