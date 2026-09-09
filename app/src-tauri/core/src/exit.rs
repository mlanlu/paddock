//! paddock's exit codes, as documented in `docs/cli-json.md`.
//!
//! The whole point of the distinct codes is that a caller can react
//! differently, so the app carries them as a type rather than checking
//! `success()`. "It failed" is the one message this module exists to avoid.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Exit {
    Ok,
    /// `1` — anything else. The message on stderr is the only detail there is.
    Failure,
    /// `2` — usage error. A bug in this app, not something the user can fix.
    Usage,
    /// `3` — Docker missing or not running.
    NoDocker,
    /// `4` — configuration missing or wrong.
    Config,
    /// `5` — provisioning failed; the sandbox is running wider than its profile.
    Provisioning,
    /// `6` — policy not fully applied; the sandbox is CLOSED.
    Closed,
    /// `7` — the command needs a running sandbox and there is none.
    NoSandbox,
    /// Anything paddock has not documented, or the exit code of a command run
    /// inside the sandbox by `run`/`shell`/`exec`.
    Other(i32),
}

impl Exit {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Exit::Ok,
            1 => Exit::Failure,
            2 => Exit::Usage,
            3 => Exit::NoDocker,
            4 => Exit::Config,
            5 => Exit::Provisioning,
            6 => Exit::Closed,
            7 => Exit::NoSandbox,
            other => Exit::Other(other),
        }
    }

    pub fn is_ok(self) -> bool {
        self == Exit::Ok
    }

    /// What went wrong, in the user's terms.
    pub fn meaning(self) -> &'static str {
        match self {
            Exit::Ok => "succeeded",
            Exit::Failure => "failed",
            Exit::Usage => "the app called paddock incorrectly",
            Exit::NoDocker => "Docker is not running",
            Exit::Config => "the profile or policy configuration is wrong",
            Exit::Provisioning => "provisioning failed",
            Exit::Closed => "the egress policy could not be fully applied",
            Exit::NoSandbox => "there is no running sandbox",
            Exit::Other(_) => "failed",
        }
    }

    /// The single next action worth offering. `None` when there is nothing the
    /// app can usefully suggest and the stderr message is all there is.
    pub fn remedy(self) -> Option<&'static str> {
        match self {
            Exit::NoDocker => Some("Start Docker Desktop, then try again."),
            Exit::Config => Some("Run `paddock init` for this folder, or fix the profile."),
            Exit::Provisioning => {
                Some("The sandbox is running wider than its profile until this succeeds. Retry provisioning once the network is fixed.")
            }
            Exit::Closed => {
                Some("Egress is not what the profile asked for. Re-apply the policy once the network is back.")
            }
            Exit::NoSandbox => Some("Start the sandbox first."),
            _ => None,
        }
    }

    /// Whether the sandbox is currently *wider* or *narrower* than its profile
    /// promises. The UI must not present these as ordinary errors: `5` and `6`
    /// both leave a sandbox running in a state the user did not ask for, and
    /// that is the one thing paddock exists to be honest about.
    pub fn misstates_policy(self) -> bool {
        matches!(self, Exit::Provisioning | Exit::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_codes_map_to_their_meaning() {
        assert_eq!(Exit::from_code(0), Exit::Ok);
        assert_eq!(Exit::from_code(3), Exit::NoDocker);
        assert_eq!(Exit::from_code(6), Exit::Closed);
        assert_eq!(Exit::from_code(7), Exit::NoSandbox);
    }

    #[test]
    fn undocumented_codes_are_preserved_not_flattened() {
        // `run`/`shell`/`exec` exit with the inner command's code; collapsing
        // 130 into "failed" would lose that Ctrl-C is not a paddock failure.
        assert_eq!(Exit::from_code(130), Exit::Other(130));
    }

    #[test]
    fn only_the_policy_codes_misstate_policy() {
        for code in [0, 1, 2, 3, 4, 7, 130] {
            assert!(!Exit::from_code(code).misstates_policy(), "code {code}");
        }
        assert!(Exit::from_code(5).misstates_policy());
        assert!(Exit::from_code(6).misstates_policy());
    }

    #[test]
    fn every_actionable_code_offers_a_remedy() {
        for code in [3, 4, 5, 6, 7] {
            assert!(Exit::from_code(code).remedy().is_some(), "code {code}");
        }
    }
}
