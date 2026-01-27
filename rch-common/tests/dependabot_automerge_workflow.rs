//! GitHub Workflow Validation Tests
//!
//! Ensures critical workflows remain syntactically valid and keep their expected
//! structure (prevents silent automerge failures due to YAML drift).
//!
//! Bead: bd-zxiv

use serde_yaml_ng::{Mapping, Value};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .expect("CARGO_MANIFEST_DIR should have a parent (repo root)")
}

fn workflow_path() -> PathBuf {
    repo_root().join(".github/workflows/dependabot-automerge.yml")
}

fn map_get<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    map.get(Value::String(key.to_string()))
}

fn as_mapping<'a>(value: &'a Value, context: &str) -> &'a Mapping {
    value
        .as_mapping()
        .unwrap_or_else(|| panic!("{context} must be a YAML mapping"))
}

fn as_sequence<'a>(value: &'a Value, context: &str) -> &'a Vec<Value> {
    value
        .as_sequence()
        .unwrap_or_else(|| panic!("{context} must be a YAML sequence"))
}

fn as_str<'a>(value: &'a Value, context: &str) -> &'a str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("{context} must be a YAML string"))
}

fn mapping_keys_as_strings(map: &Mapping) -> Vec<String> {
    map.keys()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

#[test]
fn dependabot_automerge_workflow_is_valid_and_has_expected_steps() {
    let path = workflow_path();
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

    let root: Value = serde_yaml_ng::from_str(&content)
        .unwrap_or_else(|e| panic!("Invalid YAML in {}: {e}", path.display()));

    let root_map = as_mapping(&root, "workflow root");

    // name
    let name = map_get(root_map, "name").expect("workflow must have 'name'");
    assert!(!as_str(name, "workflow.name").trim().is_empty());

    // on
    let on_value = map_get(root_map, "on").expect("workflow must have 'on'");
    let triggers: Vec<String> = match on_value {
        Value::String(single) => vec![single.clone()],
        Value::Sequence(seq) => seq
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Value::Mapping(map) => mapping_keys_as_strings(map),
        other => panic!("workflow.on must be string/sequence/mapping; got {other:?}"),
    };
    assert!(
        triggers.contains(&"pull_request".to_string())
            || triggers.contains(&"pull_request_target".to_string()),
        "workflow.on must include pull_request or pull_request_target; got {triggers:?}"
    );

    // permissions
    let permissions = map_get(root_map, "permissions").expect("workflow must have 'permissions'");
    let permissions_map = as_mapping(permissions, "workflow.permissions");

    assert_eq!(
        map_get(permissions_map, "contents")
            .map(|v| as_str(v, "workflow.permissions.contents"))
            .unwrap_or(""),
        "write",
        "workflow.permissions.contents must be 'write'"
    );
    assert_eq!(
        map_get(permissions_map, "pull-requests")
            .map(|v| as_str(v, "workflow.permissions.pull-requests"))
            .unwrap_or(""),
        "write",
        "workflow.permissions.pull-requests must be 'write'"
    );

    // jobs.automerge
    let jobs = map_get(root_map, "jobs").expect("workflow must have 'jobs'");
    let jobs_map = as_mapping(jobs, "workflow.jobs");
    let automerge = map_get(jobs_map, "automerge").expect("workflow.jobs must have 'automerge'");
    let automerge_map = as_mapping(automerge, "workflow.jobs.automerge");

    let job_if = map_get(automerge_map, "if").expect("automerge job must have 'if'");
    let job_if_str = as_str(job_if, "workflow.jobs.automerge.if");
    assert!(
        job_if_str.contains("dependabot[bot]"),
        "automerge job 'if' must restrict to dependabot[bot]; got: {job_if_str:?}"
    );

    let steps_value = map_get(automerge_map, "steps").expect("automerge job must have 'steps'");
    let steps = as_sequence(steps_value, "workflow.jobs.automerge.steps");

    // Step: dependabot/fetch-metadata
    let has_fetch_metadata = steps.iter().any(|step: &Value| {
        step.as_mapping()
            .and_then(|step_map| map_get(step_map, "uses"))
            .and_then(Value::as_str)
            .is_some_and(|uses: &str| uses.starts_with("dependabot/fetch-metadata@"))
    });
    assert!(
        has_fetch_metadata,
        "automerge workflow must include dependabot/fetch-metadata step"
    );

    // Step(s): gh pr merge --auto --squash "$PR_URL"
    let merge_steps: Vec<&Mapping> = steps
        .iter()
        .filter_map(|step: &Value| step.as_mapping())
        .filter(|step_map| {
            map_get(step_map, "run")
                .and_then(Value::as_str)
                .is_some_and(|run| run.contains("gh pr merge"))
        })
        .collect();

    assert!(
        !merge_steps.is_empty(),
        "automerge workflow must include at least one 'gh pr merge' step"
    );

    for (idx, step_map) in merge_steps.iter().enumerate() {
        let run = map_get(step_map, "run")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            run.contains("--auto") && run.contains("--squash"),
            "merge step {idx} must include --auto and --squash; got: {run:?}"
        );

        let env = map_get(step_map, "env").expect("merge step must define env");
        let env_map = as_mapping(env, "merge_step.env");

        let pr_url = map_get(env_map, "PR_URL").expect("merge step env must set PR_URL");
        let gh_token = map_get(env_map, "GH_TOKEN").expect("merge step env must set GH_TOKEN");

        let pr_url_str = as_str(pr_url, "merge_step.env.PR_URL");
        let gh_token_str = as_str(gh_token, "merge_step.env.GH_TOKEN");

        assert!(
            pr_url_str.contains("github.event.pull_request.html_url"),
            "PR_URL must reference github.event.pull_request.html_url; got: {pr_url_str:?}"
        );
        assert!(
            gh_token_str.contains("secrets.GITHUB_TOKEN"),
            "GH_TOKEN must reference secrets.GITHUB_TOKEN; got: {gh_token_str:?}"
        );
    }
}
