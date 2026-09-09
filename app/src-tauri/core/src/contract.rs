//! Typed views of `paddock ls --json` and `paddock info PATH --json`.
//!
//! `docs/cli-json.md` is the contract; this file is the other half of it. The
//! app must never parse the human-readable table, so every field the UI shows
//! comes from here.
//!
//! **Strictness.** Unknown fields are accepted, missing ones are not. The CLI
//! may add a field without breaking the app — that is an additive change and
//! the app simply does not show it — but a field the app expects and does not
//! find is a contract break, and it fails loudly at the seam rather than
//! rendering a half-empty row that reads to the user as a broken sandbox.
//! Fields documented as nullable are `Option`, and nothing else is.

use serde::{Deserialize, Serialize};

/// A published port pair. `host` is bound on `127.0.0.1`, never `0.0.0.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub host: u16,
    pub container: u16,
}

/// The egress policy paddock last tried to apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// Persisted set names. The `claude` set is always added on top and is not
    /// listed here. `open` anywhere in this list means there is no firewall.
    pub sets: Vec<String>,
    /// Extra domains from the profile and `--allow`.
    pub extra: Vec<String>,
    /// paddock's own summary, e.g. `github,npm +1`, `strict`, `open`.
    pub describe: String,
}

impl Policy {
    /// No firewall at all. Worth its own method because it is the one policy
    /// state the UI must never render as just another label.
    pub fn is_open(&self) -> bool {
        self.sets.iter().any(|s| s == "open")
    }
}

/// One row of `paddock ls --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sandbox {
    pub container: String,
    /// Docker's state string: `running`, `exited`, `created`, …
    pub state: String,
    pub repo: String,
    pub profile: String,
    pub workspace: String,
    /// The last policy application did not fully succeed. A fact about the
    /// sandbox, not about the policy, which is why it survives `policy` being
    /// `null`.
    pub closed: bool,
    /// `null` when paddock has no persisted policy for this container.
    pub policy: Option<Policy>,
    pub ports: Vec<Port>,
}

impl Sandbox {
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

/// `paddock info PATH --json` — what `up` *would* do, with no side effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// The directory that would be sandboxed.
    pub root: String,
    pub repo: String,
    /// Basename of `root`; differs from `repo` for a linked worktree.
    pub name: String,
    pub container: String,
    /// Set when `root` is a linked worktree.
    pub common_git: Option<String>,
    pub profile: String,
    /// `null` is the cue to offer `paddock init`.
    pub profile_path: Option<String>,
    pub env_file: Option<String>,
    /// `null` when the container does not exist.
    pub state: Option<String>,
    /// Only knowable for a running container.
    pub provisioned: Option<bool>,
    pub ports: Vec<Port>,
    pub policy: Option<Policy>,
}

impl Info {
    /// The container does not exist yet, so `up` would create it.
    pub fn is_new(&self) -> bool {
        self.state.is_none()
    }

    /// Only `profiles/default.json` applies — the cue to offer `paddock init`.
    pub fn has_no_profile_of_its_own(&self) -> bool {
        self.profile_path.is_none()
    }
}

/// Failure to read the CLI's output as the contract describes it.
#[derive(Debug)]
pub struct ContractError {
    pub command: String,
    pub source: serde_json::Error,
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`paddock {}` returned output this version of the app does not understand ({}). \
             The CLI and the app are probably different versions.",
            self.command, self.source
        )
    }
}

impl std::error::Error for ContractError {}

pub fn parse_sandboxes(json: &str) -> Result<Vec<Sandbox>, ContractError> {
    serde_json::from_str(json).map_err(|source| ContractError {
        command: "ls --json".to_string(),
        source,
    })
}

pub fn parse_info(json: &str) -> Result<Info, ContractError> {
    serde_json::from_str(json).map_err(|source| ContractError {
        command: "info --json".to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verbatim from docs/cli-json.md. If the doc changes, this fails.
    const LS_EXAMPLE: &str = r#"[
      {
        "container": "paddock-openmatch-openmatch-web",
        "state": "running",
        "repo": "openmatch",
        "profile": "openmatch",
        "workspace": "/Users/you/Documents/openmatch-web",
        "closed": false,
        "policy": {
          "sets": ["github", "npm"],
          "extra": ["fonts.googleapis.com"],
          "describe": "github,npm +1"
        },
        "ports": [{"host": 3010, "container": 3000}]
      }
    ]"#;

    const INFO_EXAMPLE: &str = r#"{
      "root": "/Users/you/Documents/openmatch-web",
      "repo": "openmatch",
      "name": "openmatch-web",
      "container": "paddock-openmatch-openmatch-web",
      "common_git": "/Users/you/Documents/openmatch/.git",
      "profile": "openmatch",
      "profile_path": "/Users/you/.config/paddock/profiles/openmatch.json",
      "env_file": "/Users/you/.config/paddock/env/openmatch.env",
      "state": "exited",
      "provisioned": null,
      "ports": [{"host": 3010, "container": 3000}],
      "policy": {"sets": ["github", "npm"], "extra": [], "describe": "github,npm"}
    }"#;

    #[test]
    fn the_documented_ls_example_parses() {
        let got = parse_sandboxes(LS_EXAMPLE).unwrap();
        assert_eq!(got.len(), 1);
        let s = &got[0];
        assert_eq!(s.container, "paddock-openmatch-openmatch-web");
        assert!(s.is_running());
        assert!(!s.closed);
        assert_eq!(s.ports, vec![Port { host: 3010, container: 3000 }]);
        assert_eq!(s.policy.as_ref().unwrap().describe, "github,npm +1");
    }

    #[test]
    fn the_documented_info_example_parses() {
        let got = parse_info(INFO_EXAMPLE).unwrap();
        assert_eq!(got.name, "openmatch-web");
        assert_eq!(got.common_git.as_deref(), Some("/Users/you/Documents/openmatch/.git"));
        assert_eq!(got.provisioned, None);
        assert!(!got.is_new());
        assert!(!got.has_no_profile_of_its_own());
    }

    #[test]
    fn no_sandboxes_is_an_empty_list_not_an_error() {
        assert_eq!(parse_sandboxes("[]").unwrap(), vec![]);
    }

    #[test]
    fn a_null_policy_parses_and_closed_survives_it() {
        // The documented reason `closed` sits at the top level: it must still
        // be readable when paddock has no persisted policy for the container.
        let json = r#"[{"container":"c","state":"exited","repo":"r","profile":"p",
            "workspace":"/w","closed":true,"policy":null,"ports":[]}]"#;
        let got = parse_sandboxes(json).unwrap();
        assert!(got[0].closed);
        assert!(got[0].policy.is_none());
    }

    #[test]
    fn a_new_container_has_no_state() {
        let json = r#"{"root":"/w","repo":"r","name":"r","container":"c","common_git":null,
            "profile":"default","profile_path":null,"env_file":null,"state":null,
            "provisioned":null,"ports":[],"policy":null}"#;
        let got = parse_info(json).unwrap();
        assert!(got.is_new());
        assert!(got.has_no_profile_of_its_own());
    }

    #[test]
    fn an_open_policy_is_recognised() {
        let p = Policy { sets: vec!["open".into()], extra: vec![], describe: "open".into() };
        assert!(p.is_open());
        let p = Policy { sets: vec!["github".into()], extra: vec![], describe: "github".into() };
        assert!(!p.is_open());
    }

    #[test]
    fn a_missing_field_fails_loudly() {
        // `closed` absent: the app must refuse the row rather than default it
        // to false and tell the user a CLOSED sandbox is fine.
        let json = r#"[{"container":"c","state":"exited","repo":"r","profile":"p",
            "workspace":"/w","policy":null,"ports":[]}]"#;
        assert!(parse_sandboxes(json).is_err());
    }

    #[test]
    fn an_added_field_is_ignored_not_fatal() {
        // Additive CLI changes must not break an older app.
        let json = r#"[{"container":"c","state":"exited","repo":"r","profile":"p",
            "workspace":"/w","closed":false,"policy":null,"ports":[],"invented_later":42}]"#;
        assert_eq!(parse_sandboxes(json).unwrap().len(), 1);
    }

    #[test]
    fn the_error_says_the_versions_probably_disagree() {
        let err = parse_sandboxes("{}").unwrap_err();
        assert!(err.to_string().contains("different versions"), "{err}");
    }
}
