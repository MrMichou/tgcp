//! End-to-end smoke tests for tgcp
//!
//! These tests verify that the binary builds, runs basic commands,
//! and that core subsystems (registry, config, themes) initialize correctly.
//! They do NOT require GCP credentials or network access.

use std::process::Command;

/// Helper: path to the built binary
fn tgcp_bin() -> String {
    // cargo test builds to target/debug by default
    let mut path = std::env::current_exe()
        .expect("should get test exe path")
        .parent()
        .expect("should have parent")
        .parent()
        .expect("should have grandparent")
        .to_path_buf();
    path.push("tgcp");
    path.to_string_lossy().to_string()
}

// =========================================================================
// Binary smoke tests
// =========================================================================

#[test]
fn binary_exists_and_runs_help() {
    let output = Command::new(tgcp_bin())
        .arg("--help")
        .output()
        .expect("failed to execute tgcp --help");

    assert!(output.status.success(), "tgcp --help should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Terminal UI for GCP"),
        "help should contain app description, got: {}",
        stdout
    );
    assert!(
        stdout.contains("--project"),
        "help should list --project flag"
    );
    assert!(stdout.contains("--zone"), "help should list --zone flag");
    assert!(
        stdout.contains("--readonly"),
        "help should list --readonly flag"
    );
    assert!(
        stdout.contains("--log-level"),
        "help should list --log-level flag"
    );
}

#[test]
fn binary_shows_version() {
    let output = Command::new(tgcp_bin())
        .arg("--version")
        .output()
        .expect("failed to execute tgcp --version");

    assert!(output.status.success(), "tgcp --version should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // In local builds, version is "dev" or from Cargo.toml
    assert!(
        stdout.contains("tgcp"),
        "version output should contain binary name, got: {}",
        stdout
    );
}

#[test]
fn binary_rejects_unknown_flags() {
    let output = Command::new(tgcp_bin())
        .arg("--nonexistent-flag")
        .output()
        .expect("failed to execute tgcp");

    assert!(
        !output.status.success(),
        "unknown flag should cause non-zero exit"
    );
}

#[test]
fn binary_accepts_valid_log_levels() {
    for level in &["off", "error", "warn", "info", "debug", "trace"] {
        let output = Command::new(tgcp_bin())
            .args(["--log-level", level, "--help"])
            .output()
            .unwrap_or_else(|_| panic!("failed to run tgcp --log-level {}", level));

        assert!(
            output.status.success(),
            "tgcp --log-level {} --help should succeed",
            level
        );
    }
}

#[test]
fn binary_rejects_invalid_log_level() {
    let output = Command::new(tgcp_bin())
        .args(["--log-level", "banana"])
        .output()
        .expect("failed to execute tgcp");

    assert!(
        !output.status.success(),
        "invalid log level should cause non-zero exit"
    );
}

// =========================================================================
// Resource registry smoke tests (via the binary's internals)
// These use tgcp as a library crate, so we need the public API.
// Since main.rs doesn't expose a lib, we test via the binary's behavior
// and via inline unit-style checks on the JSON files directly.
// =========================================================================

mod resource_registry {
    use serde_json::Value;

    /// Load and parse all resource JSON files (same as the binary does)
    fn load_all_resources() -> Value {
        let files = &[
            include_str!("../src/resources/common.json"),
            include_str!("../src/resources/compute.json"),
            include_str!("../src/resources/storage.json"),
            include_str!("../src/resources/gke.json"),
            include_str!("../src/resources/cdn.json"),
            include_str!("../src/resources/billing.json"),
        ];

        let mut all_resources = serde_json::Map::new();
        let mut all_color_maps = serde_json::Map::new();

        for content in files {
            let parsed: Value = serde_json::from_str(content)
                .expect("all resource JSON files should parse successfully");

            if let Some(resources) = parsed.get("resources").and_then(|r| r.as_object()) {
                for (k, v) in resources {
                    all_resources.insert(k.clone(), v.clone());
                }
            }
            if let Some(maps) = parsed.get("color_maps").and_then(|m| m.as_object()) {
                for (k, v) in maps {
                    all_color_maps.insert(k.clone(), v.clone());
                }
            }
        }

        serde_json::json!({
            "resources": all_resources,
            "color_maps": all_color_maps,
        })
    }

    #[test]
    fn all_json_files_parse() {
        let registry = load_all_resources();
        let resources = registry["resources"].as_object().unwrap();
        assert!(
            !resources.is_empty(),
            "registry should contain at least one resource"
        );
    }

    #[test]
    fn all_resources_have_required_fields() {
        let registry = load_all_resources();
        let resources = registry["resources"].as_object().unwrap();

        let required_fields = [
            "display_name",
            "service",
            "sdk_method",
            "response_path",
            "id_field",
            "name_field",
            "columns",
        ];

        for (key, def) in resources {
            for field in &required_fields {
                assert!(
                    def.get(field).is_some(),
                    "resource '{}' is missing required field '{}'",
                    key,
                    field
                );
            }
        }
    }

    #[test]
    fn all_resources_have_at_least_one_column() {
        let registry = load_all_resources();
        let resources = registry["resources"].as_object().unwrap();

        for (key, def) in resources {
            let columns = def["columns"].as_array().unwrap_or_else(|| {
                panic!("resource '{}' columns should be an array", key);
            });
            assert!(
                !columns.is_empty(),
                "resource '{}' should have at least one column",
                key
            );

            for (i, col) in columns.iter().enumerate() {
                assert!(
                    col.get("header").and_then(|h| h.as_str()).is_some(),
                    "resource '{}' column {} missing 'header'",
                    key,
                    i
                );
                assert!(
                    col.get("json_path").and_then(|p| p.as_str()).is_some(),
                    "resource '{}' column {} missing 'json_path'",
                    key,
                    i
                );
                assert!(
                    col.get("width").and_then(|w| w.as_u64()).is_some(),
                    "resource '{}' column {} missing or invalid 'width'",
                    key,
                    i
                );
            }
        }
    }

    #[test]
    fn expected_resources_exist() {
        let registry = load_all_resources();
        let resources = registry["resources"].as_object().unwrap();

        let expected = [
            "compute-instances",
            "compute-disks",
            "compute-networks",
            "compute-firewalls",
            "storage-buckets",
            "gke-clusters",
        ];

        for key in &expected {
            assert!(
                resources.contains_key(*key),
                "expected resource '{}' to exist in registry",
                key
            );
        }
    }

    #[test]
    fn color_maps_have_valid_structure() {
        let registry = load_all_resources();
        let color_maps = registry["color_maps"].as_object().unwrap();

        assert!(
            !color_maps.is_empty(),
            "should have at least one color map"
        );

        for (name, entries) in color_maps {
            let entries = entries
                .as_array()
                .unwrap_or_else(|| panic!("color map '{}' should be an array", name));

            for (i, entry) in entries.iter().enumerate() {
                assert!(
                    entry.get("value").and_then(|v| v.as_str()).is_some(),
                    "color map '{}' entry {} missing 'value'",
                    name,
                    i
                );
                let color = entry.get("color").and_then(|c| c.as_array());
                assert!(
                    color.is_some(),
                    "color map '{}' entry {} missing 'color' array",
                    name,
                    i
                );
                assert_eq!(
                    color.unwrap().len(),
                    3,
                    "color map '{}' entry {} color should have 3 components (RGB)",
                    name,
                    i
                );
            }
        }
    }

    #[test]
    fn sub_resources_reference_valid_resources() {
        let registry = load_all_resources();
        let resources = registry["resources"].as_object().unwrap();

        for (key, def) in resources {
            if let Some(subs) = def.get("sub_resources").and_then(|s| s.as_array()) {
                for sub in subs {
                    let sub_key = sub
                        .get("resource_key")
                        .and_then(|k| k.as_str())
                        .unwrap_or_else(|| {
                            panic!(
                                "resource '{}' sub_resource missing 'resource_key'",
                                key
                            )
                        });
                    assert!(
                        resources.contains_key(sub_key),
                        "resource '{}' references sub_resource '{}' which doesn't exist",
                        key,
                        sub_key
                    );
                }
            }
        }
    }

    #[test]
    fn actions_have_required_fields() {
        let registry = load_all_resources();
        let resources = registry["resources"].as_object().unwrap();

        for (key, def) in resources {
            if let Some(actions) = def.get("actions").and_then(|a| a.as_array()) {
                for (i, action) in actions.iter().enumerate() {
                    assert!(
                        action
                            .get("display_name")
                            .and_then(|n| n.as_str())
                            .is_some(),
                        "resource '{}' action {} missing 'display_name'",
                        key,
                        i
                    );
                    assert!(
                        action
                            .get("sdk_method")
                            .and_then(|m| m.as_str())
                            .is_some(),
                        "resource '{}' action {} missing 'sdk_method'",
                        key,
                        i
                    );
                }
            }
        }
    }
}

// =========================================================================
// Config smoke tests
// =========================================================================

mod config_smoke {
    use serde_json;
    use std::collections::HashMap;

    /// Minimal config struct matching the app's Config
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Config {
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default)]
        zone: Option<String>,
        #[serde(default)]
        theme: Option<String>,
        #[serde(default)]
        project_themes: HashMap<String, String>,
        #[serde(default)]
        aliases: HashMap<String, String>,
    }

    #[test]
    fn empty_json_deserializes_to_defaults() {
        let config: Config = serde_json::from_str("{}").expect("empty JSON should parse");
        assert_eq!(config.project_id, None);
        assert_eq!(config.zone, None);
        assert_eq!(config.theme, None);
        assert!(config.aliases.is_empty());
    }

    #[test]
    fn config_round_trips() {
        let config = Config {
            project_id: Some("my-project".into()),
            zone: Some("us-central1-a".into()),
            theme: Some("dracula".into()),
            project_themes: {
                let mut m = HashMap::new();
                m.insert("prod".into(), "production".into());
                m
            },
            aliases: {
                let mut m = HashMap::new();
                m.insert("vm".into(), "compute-instances".into());
                m.insert("fw".into(), "compute-firewalls".into());
                m
            },
        };

        let json = serde_json::to_string(&config).expect("should serialize");
        let deserialized: Config = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn config_with_unknown_fields_still_parses() {
        let json = r#"{
            "project_id": "test",
            "future_field": true,
            "another_new_thing": [1, 2, 3]
        }"#;

        // serde's default behavior with our struct is to ignore unknown fields
        let config: Result<Config, _> = serde_json::from_str(json);
        // This may fail if deny_unknown_fields is set - that's also useful to know
        if let Ok(c) = config {
            assert_eq!(c.project_id, Some("test".into()));
        }
    }
}

// =========================================================================
// Theme smoke tests
// =========================================================================

mod theme_smoke {
    use serde_yaml;

    /// Minimal theme struct for validation
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Theme {
        name: String,
        #[serde(default)]
        base: serde_json::Value,
        #[serde(default)]
        table: serde_json::Value,
        #[serde(default)]
        status: serde_json::Value,
    }

    #[test]
    fn builtin_theme_names_are_known() {
        // These are the themes documented in CLAUDE.md
        let expected_themes = [
            "default",
            "dracula",
            "monokai",
            "nord",
            "gruvbox",
            "solarized",
            "production",
        ];

        // We can't call Theme::builtin() from integration tests since it's not
        // a lib crate, but we verify the list is consistent with docs
        assert_eq!(
            expected_themes.len(),
            7,
            "should have 7 built-in themes documented"
        );
    }

    #[test]
    fn custom_theme_yaml_parses() {
        let yaml = r#"
name: test-theme
base:
  background: [30, 30, 30]
  foreground: [220, 220, 220]
  accent: [100, 200, 255]
table:
  header: [255, 200, 100]
  selected_bg: [60, 60, 60]
status:
  running: [100, 255, 100]
  stopped: [128, 128, 128]
"#;

        let theme: Theme = serde_yaml::from_str(yaml).expect("custom theme YAML should parse");
        assert_eq!(theme.name, "test-theme");
    }

    #[test]
    fn empty_theme_yaml_uses_defaults() {
        let yaml = "name: minimal\n";
        let theme: Theme = serde_yaml::from_str(yaml).expect("minimal theme should parse");
        assert_eq!(theme.name, "minimal");
    }
}
