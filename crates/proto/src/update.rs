// The app updating itself: check the release feed, then download, verify and install a newer
// version. The engine does the work (engine/src/update.rs); the UI asks and shows progress.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum UpdateCommand {
    /// Answered with `Checked`, `Unmanaged` or `Failed`.
    Check,
    /// Checks again so an app behind by several releases lands on the newest in one hop, then
    /// downloads, verifies and starts the install. Progress comes as `Step`, the end as `Quit`
    /// or `Failed`.
    Install,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum UpdateEvent {
    /// The feed's answer: a newer release, or None when this is the newest.
    Checked {
        release: Option<Release>,
    },
    /// This copy runs from a build folder, not an install, so it never replaces itself.
    Unmanaged,
    Step {
        step: Step,
    },
    /// The installer is running, or the new app is in place and will open once this one exits.
    /// The app quits now so the install can finish.
    Quit,
    /// `message` is user-facing. `install` says whether an install failed or only a check.
    Failed {
        message: String,
        install: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    /// The release notes, as the release's body has them.
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "camelCase")]
pub enum Step {
    /// Checking the feed again before the download.
    Preparing,
    /// None when the server didn't say how big the file is.
    Downloading {
        percent: Option<u8>,
    },
    Installing,
}
