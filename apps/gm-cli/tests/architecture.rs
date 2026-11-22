//! Enforce dependency direction using Cargo's resolved package metadata.
use std::process::Command;

#[test]
fn workspace_dependencies_point_inward_and_adapters_are_independent() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let members = metadata["workspace_members"].as_array().unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let rules: [(&str, &[&str]); 5] = [
        ("gm-core", &[]),
        ("gm-application", &["gm-core"]),
        ("gm-store", &["gm-core", "gm-application"]),
        ("gm-runner", &["gm-core", "gm-application"]),
        ("gm-cli", &["gm-core", "gm-application", "gm-store", "gm-runner"]),
    ];
    assert_eq!(members.len(), rules.len());
    for (name, allowed) in rules {
        let package = packages.iter().find(|package| package["name"] == name).unwrap();
        let manifest = std::path::Path::new(package["manifest_path"].as_str().unwrap());
        let directory = if name == "gm-cli" { "apps" } else { "packages" };
        assert_eq!(
            manifest.strip_prefix(root.canonicalize().unwrap()).unwrap(),
            std::path::Path::new(directory).join(name).join("Cargo.toml")
        );
        if name == "gm-core" {
            assert!(package["dependencies"].as_array().unwrap().is_empty());
        }
        for dependency in package["dependencies"].as_array().unwrap() {
            let target = dependency["name"].as_str().unwrap();
            if target.starts_with("gm-") {
                assert!(allowed.contains(&target), "{name} must not depend on {target}");
            }
        }
    }
}
