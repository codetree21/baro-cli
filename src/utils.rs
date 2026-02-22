use std::path::Path;

pub(crate) fn detect_metadata(dir: &Path) -> (Option<String>, Option<String>) {
    // Try Cargo.toml
    if let Ok(content) = std::fs::read_to_string(dir.join("Cargo.toml")) {
        let name = extract_toml_value(&content, "name");
        let desc = extract_toml_value(&content, "description");
        if name.is_some() || desc.is_some() {
            return (name, desc);
        }
    }
    // Try package.json
    if let Ok(content) = std::fs::read_to_string(dir.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            let name = v["name"].as_str().map(String::from);
            let desc = v["description"].as_str().map(String::from);
            return (name, desc);
        }
    }
    (None, None)
}

pub(crate) fn extract_toml_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{} ", key)) || trimmed.starts_with(&format!("{}=", key)) {
            if let Some(val) = trimmed.split('=').nth(1) {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

pub(crate) fn dir_to_slug(dir: &Path) -> String {
    dir.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
        .replace(' ', "-")
}

pub(crate) fn read_changelog(dir: &Path, _version: &str) -> Option<String> {
    let path = dir.join("CHANGELOG.md");
    std::fs::read_to_string(path).ok().map(|content| {
        // Take first non-empty section (simplistic)
        content
            .lines()
            .skip_while(|l| l.trim().is_empty() || l.starts_with('#'))
            .take_while(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    })
}

pub(crate) fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max_chars {
        let truncated: String = chars[..max_chars - 3].iter().collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

pub(crate) fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    // -- extract_toml_value --

    #[test]
    fn extract_toml_value_with_space() {
        let content = "[package]\nname = \"my-app\"\nversion = \"1.0.0\"";
        assert_eq!(extract_toml_value(content, "name"), Some("my-app".to_string()));
    }

    #[test]
    fn extract_toml_value_no_space() {
        let content = "[package]\nname=\"my-app\"";
        assert_eq!(extract_toml_value(content, "name"), Some("my-app".to_string()));
    }

    #[test]
    fn extract_toml_value_missing_key() {
        let content = "[package]\nname = \"my-app\"";
        assert_eq!(extract_toml_value(content, "description"), None);
    }

    #[test]
    fn extract_toml_value_single_quotes() {
        let content = "name = 'my-app'";
        assert_eq!(extract_toml_value(content, "name"), Some("my-app".to_string()));
    }

    // -- detect_metadata --

    #[test]
    fn detect_metadata_from_cargo_toml() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test-proj\"\ndescription = \"A test project\"",
        ).unwrap();
        let (name, desc) = detect_metadata(dir.path());
        assert_eq!(name, Some("test-proj".to_string()));
        assert_eq!(desc, Some("A test project".to_string()));
    }

    #[test]
    fn detect_metadata_from_package_json() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-node-app", "description": "A Node.js app"}"#,
        ).unwrap();
        let (name, desc) = detect_metadata(dir.path());
        assert_eq!(name, Some("my-node-app".to_string()));
        assert_eq!(desc, Some("A Node.js app".to_string()));
    }

    #[test]
    fn detect_metadata_no_build_file() {
        let dir = tempdir().unwrap();
        let (name, desc) = detect_metadata(dir.path());
        assert_eq!(name, None);
        assert_eq!(desc, None);
    }

    #[test]
    fn detect_metadata_prefers_cargo_toml() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"rust-app\"",
        ).unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name": "node-app"}"#,
        ).unwrap();
        let (name, _) = detect_metadata(dir.path());
        assert_eq!(name, Some("rust-app".to_string()));
    }

    // -- dir_to_slug --

    #[test]
    fn dir_to_slug_lowercases_and_replaces_spaces() {
        let path = Path::new("/home/user/My Project");
        assert_eq!(dir_to_slug(path), "my-project");
    }

    #[test]
    fn dir_to_slug_simple_name() {
        let path = Path::new("/home/user/myapp");
        assert_eq!(dir_to_slug(path), "myapp");
    }

    // -- truncate_str --

    #[test]
    fn truncate_str_short_unchanged() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_long_truncated() {
        assert_eq!(truncate_str("hello world this is long", 10), "hello w...");
    }

    #[test]
    fn truncate_str_korean_unicode_safe() {
        let korean = "안녕하세요 세계입니다";
        let result = truncate_str(korean, 6);
        assert_eq!(result, "안녕하...");
        // Ensure no panic from splitting multi-byte chars
        assert!(result.is_char_boundary(result.len()));
    }

    // -- format_bytes --

    #[test]
    fn format_bytes_thresholds() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(2621440), "2.5 MB");
    }

    // -- read_changelog --

    #[test]
    fn read_changelog_extracts_first_section() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("CHANGELOG.md"),
            "# Changelog\n\n## 1.0.0\n\nFirst release with core features.\nBug fixes included.\n\n## 0.9.0\n\nBeta release.\n",
        ).unwrap();
        let result = read_changelog(dir.path(), "1.0.0");
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("First release"));
        assert!(!text.contains("Beta release"));
    }

    #[test]
    fn read_changelog_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        assert_eq!(read_changelog(dir.path(), "1.0.0"), None);
    }
}
