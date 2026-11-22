use std::process::Command;

#[test]
fn workspace_dependencies_point_inward_and_adapters_are_independent() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = match Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
    {
        | Ok(output) => output,
        | Err(error) => panic!("failed to run cargo metadata: {error}"),
    };
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let metadata: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        | Ok(metadata) => metadata,
        | Err(error) => panic!("invalid cargo metadata JSON: {error}"),
    };
    let members = match metadata["workspace_members"].as_array() {
        | Some(workspace_members) => workspace_members,
        | None => panic!("expected workspace_members array in cargo metadata"),
    };
    let packages = match metadata["packages"].as_array() {
        | Some(packages) => packages,
        | None => panic!("expected packages array in cargo metadata"),
    };
    let rules: [(&str, &[&str]); 5] = [
        ("gm-core", &[]),
        ("gm-application", &["gm-core"]),
        ("gm-store", &["gm-core", "gm-application"]),
        ("gm-runner", &["gm-core", "gm-application"]),
        ("gm-cli", &["gm-core", "gm-application", "gm-store", "gm-runner"]),
    ];
    assert_eq!(members.len(), rules.len());
    let workspace = match root.canonicalize() {
        | Ok(path) => path,
        | Err(error) => panic!("failed to resolve workspace path {root:?}: {error}"),
    };
    for (name, allowed) in rules {
        let package = match packages.iter().find(|package| package["name"] == name) {
            | Some(package) => package,
            | None => panic!("missing workspace package {name}"),
        };
        let manifest_path = match package["manifest_path"].as_str() {
            | Some(path) => path,
            | None => panic!("expected manifest path for {name}"),
        };
        let manifest = std::path::Path::new(manifest_path);
        let directory = if name == "gm-cli" { "apps" } else { "packages" };
        let relative = match manifest.strip_prefix(&workspace) {
            | Ok(path) => path,
            | Err(error) => panic!("manifest for {name} lies outside workspace {workspace:?}: {error}"),
        };
        assert_eq!(relative, std::path::Path::new(directory).join(name).join("Cargo.toml"));
        let dependencies = match package["dependencies"].as_array() {
            | Some(dependencies) => dependencies,
            | None => panic!("expected dependencies array for {name}"),
        };
        if name == "gm-core" {
            assert!(dependencies.is_empty());
        }
        for dependency in dependencies {
            let target = match dependency["name"].as_str() {
                | Some(target) => target,
                | None => panic!("expected dependency name for {name}"),
            };
            if target.starts_with("gm-") {
                assert!(allowed.contains(&target), "{name} must not depend on {target}");
            }
        }
    }
}
