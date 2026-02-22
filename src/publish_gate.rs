use std::path::Path;

use crate::types::Category;

pub struct GateResult {
    pub passed: bool,
    pub failures: Vec<CheckFailure>,
    pub warnings: Vec<CheckWarning>,
}

pub struct CheckFailure {
    pub message: String,
    pub ai_fix_prompt: String,
}

pub struct CheckWarning {
    pub message: String,
}

const BUILD_FILES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "Makefile",
    "CMakeLists.txt",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "build.gradle",
    "pom.xml",
];

const SECRET_FILES: &[&str] = &[
    "credentials.json",
    "service-account.json",
    "id_rsa",
    "id_ed25519",
];

const SECRET_EXTENSIONS: &[&str] = &[".pem", ".key", ".p12", ".pfx"];

pub fn run(
    dir: &Path,
    version: &str,
    description: Option<&str>,
    category_slug: &str,
    categories: &[Category],
) -> GateResult {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    // Required: build file
    if !BUILD_FILES.iter().any(|f| dir.join(f).exists()) {
        failures.push(CheckFailure {
            message: "No build file found (Cargo.toml, package.json, Makefile, etc.)".to_string(),
            ai_fix_prompt: "Create a build file (e.g., Cargo.toml for Rust, package.json for Node.js) that describes how to build this project.".to_string(),
        });
    }

    // Required: README
    let has_readme = dir.join("README.md").exists()
        || dir.join("readme.md").exists()
        || dir.join("README").exists();
    if !has_readme {
        failures.push(CheckFailure {
            message: "README.md not found".to_string(),
            ai_fix_prompt: "Create a README.md with: project description (what it does, who it's for), setup instructions, and usage examples. Minimum 200 words.".to_string(),
        });
    }

    // Required: no secrets
    check_secrets(dir, &mut failures);

    // Required: valid version
    let version_re = regex_lite(r"^\d+(\.\d+)*$");
    if version.is_empty() || !version_re(version) {
        failures.push(CheckFailure {
            message: format!("Invalid version: '{}'", version),
            ai_fix_prompt: "Provide a valid version with --version (e.g., --version 1.0.0). Must match pattern: digits separated by dots.".to_string(),
        });
    }

    // Required: description length (only checked when provided)
    if let Some(desc) = description {
        if desc.len() < 50 {
            failures.push(CheckFailure {
                message: format!(
                    "Description too short ({} chars, need 50+)",
                    desc.len()
                ),
                ai_fix_prompt: "Add a description of at least 50 characters. Use --description or update your Cargo.toml/package.json description field.".to_string(),
            });
        }
    }

    // Required: valid category
    if !categories.iter().any(|c| c.slug == category_slug) {
        let available: Vec<&str> = categories.iter().map(|c| c.slug.as_str()).collect();
        failures.push(CheckFailure {
            message: format!("Invalid category: '{}'", category_slug),
            ai_fix_prompt: format!(
                "Use --category with a valid slug. Available: {}",
                available.join(", ")
            ),
        });
    }

    // Recommended: AI context files
    let ai_files = ["CLAUDE.md", ".cursorrules", "AGENTS.md"];
    let has_ai = ai_files.iter().any(|f| dir.join(f).exists());
    if !has_ai {
        warnings.push(CheckWarning {
            message: "No AI context files found (CLAUDE.md, .cursorrules, AGENTS.md). These help AI tools understand your project.".to_string(),
        });
    }

    // Recommended: LICENSE
    let has_license = dir.join("LICENSE").exists()
        || dir.join("LICENSE.md").exists()
        || dir.join("LICENSE.txt").exists();
    if !has_license {
        warnings.push(CheckWarning {
            message: "No LICENSE file found. Consider adding one (MIT recommended for remix-friendly products).".to_string(),
        });
    }

    GateResult {
        passed: failures.is_empty(),
        failures,
        warnings,
    }
}

fn check_secrets(dir: &Path, failures: &mut Vec<CheckFailure>) {
    let mut found_secrets: Vec<String> = Vec::new();

    // Check .env* files
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let mut is_secret = false;
            if name.starts_with(".env") && name != ".env.example" {
                is_secret = true;
            }
            if SECRET_FILES.contains(&name.as_str()) {
                is_secret = true;
            }
            if SECRET_EXTENSIONS.iter().any(|ext| name.ends_with(ext)) {
                is_secret = true;
            }
            if is_secret {
                found_secrets.push(name);
            }
        }
    }

    if !found_secrets.is_empty() {
        failures.push(CheckFailure {
            message: format!("Potential secrets found: {}", found_secrets.join(", ")),
            ai_fix_prompt: format!(
                "Remove or .gitignore these files before publishing: {}. Use environment variables instead.",
                found_secrets.join(", ")
            ),
        });
    }
}

/// Simple regex matcher for version validation (avoids regex crate dependency).
fn regex_lite(pattern: &str) -> impl Fn(&str) -> bool {
    // Only support the specific pattern: ^\d+(\.\d+)*$
    let _ = pattern;
    |s: &str| {
        if s.is_empty() {
            return false;
        }
        let mut expect_digit = true;
        for c in s.chars() {
            if expect_digit {
                if !c.is_ascii_digit() {
                    return false;
                }
                expect_digit = false;
            } else if c == '.' {
                expect_digit = true;
            } else if !c.is_ascii_digit() {
                return false;
            }
        }
        !expect_digit // must not end with '.'
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    fn sample_categories() -> Vec<Category> {
        vec![
            Category {
                id: 1,
                slug: "developer-tools".to_string(),
                name: "Developer Tools".to_string(),
                description: None,
            },
            Category {
                id: 2,
                slug: "productivity".to_string(),
                name: "Productivity".to_string(),
                description: None,
            },
        ]
    }

    fn valid_description() -> String {
        "A comprehensive toolkit for developers that helps automate common tasks.".to_string()
    }

    fn setup_valid_dir() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(dir.path().join("README.md"), "# Test Project").unwrap();
        dir
    }

    #[test]
    fn all_checks_pass() {
        let dir = setup_valid_dir();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(result.passed, "Expected pass, got failures: {:?}", result.failures.iter().map(|f| &f.message).collect::<Vec<_>>());
        assert!(result.failures.is_empty());
    }

    #[test]
    fn missing_build_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "# Hello").unwrap();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("build file")));
    }

    #[test]
    fn missing_readme() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("README")));
    }

    #[test]
    fn detects_env_file_as_secret() {
        let dir = setup_valid_dir();
        fs::write(dir.path().join(".env"), "SECRET=abc").unwrap();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains(".env")));
    }

    #[test]
    fn env_example_allowed_env_local_blocked() {
        let dir = setup_valid_dir();
        fs::write(dir.path().join(".env.example"), "KEY=").unwrap();
        fs::write(dir.path().join(".env.local"), "SECRET=abc").unwrap();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        let secret_msg = result.failures.iter().find(|f| f.message.contains("secret")).unwrap();
        assert!(secret_msg.message.contains(".env.local"));
        assert!(!secret_msg.message.contains(".env.example"));
    }

    #[test]
    fn detects_credentials_json() {
        let dir = setup_valid_dir();
        fs::write(dir.path().join("credentials.json"), "{}").unwrap();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("credentials.json")));
    }

    #[test]
    fn detects_pem_extension() {
        let dir = setup_valid_dir();
        fs::write(dir.path().join("cert.pem"), "-----BEGIN").unwrap();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("cert.pem")));
    }

    #[test]
    fn empty_version_fails() {
        let dir = setup_valid_dir();
        let result = run(dir.path(), "", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("version")));
    }

    #[test]
    fn invalid_version_format() {
        let dir = setup_valid_dir();
        let result = run(dir.path(), "1.0.a", Some(&valid_description()), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("version")));
    }

    #[test]
    fn valid_version_formats() {
        let dir = setup_valid_dir();
        for v in &["1", "1.0", "1.0.0", "2.3.4.5"] {
            let result = run(dir.path(), v, Some(&valid_description()), "developer-tools", &sample_categories());
            assert!(
                !result.failures.iter().any(|f| f.message.contains("version")),
                "Version '{}' should be valid", v
            );
        }
    }

    #[test]
    fn short_description_fails() {
        let dir = setup_valid_dir();
        let result = run(dir.path(), "1.0.0", Some("Too short"), "developer-tools", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("Description too short")));
    }

    #[test]
    fn none_description_passes() {
        let dir = setup_valid_dir();
        let result = run(dir.path(), "1.0.0", None, "developer-tools", &sample_categories());
        assert!(!result.failures.iter().any(|f| f.message.contains("Description")),
            "None description should skip check for existing products");
    }

    #[test]
    fn invalid_category_fails() {
        let dir = setup_valid_dir();
        let result = run(dir.path(), "1.0.0", Some(&valid_description()), "nonexistent", &sample_categories());
        assert!(!result.passed);
        assert!(result.failures.iter().any(|f| f.message.contains("Invalid category")));
    }
}
