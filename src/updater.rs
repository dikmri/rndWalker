use std::sync::mpsc::{self, Receiver};

const REPO_OWNER: &str = "dikmri";
const REPO_NAME: &str = "rndWalker";
const BIN_NAME: &str = "rndWalker";

#[derive(Debug, Clone)]
pub enum UpdateMessage {
    Checking,
    Skipped(String),
    UpToDate(String),
    Updated(String),
    Failed(String),
}

pub fn spawn_update_check() -> Receiver<UpdateMessage> {
    let (tx, rx) = mpsc::channel();
    let _ = tx.send(UpdateMessage::Checking);

    std::thread::spawn(move || {
        let message = check_for_update();
        let _ = tx.send(message);
    });

    rx
}

fn check_for_update() -> UpdateMessage {
    if cfg!(debug_assertions) {
        return UpdateMessage::Skipped("debug build: update check skipped".to_owned());
    }

    match try_update() {
        Ok(message) => message,
        Err(error) => UpdateMessage::Failed(error.to_string()),
    }
}

fn configure_updater() -> self_update::backends::github::UpdateBuilder {
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .current_version(self_update::cargo_crate_version!());
    builder
}

fn try_update() -> Result<UpdateMessage, self_update::errors::Error> {
    let current_version = self_update::cargo_crate_version!();

    // Resolve the newest release first and update straight to its tag. The default
    // `update()` prefers semver-"compatible" releases, which for 0.x versions means the
    // same minor only — an old install would crawl one minor version per restart instead
    // of jumping directly to the latest release.
    let latest = configure_updater().build()?.get_latest_release()?;
    if !self_update::version::bump_is_greater(current_version, &latest.version)? {
        return Ok(UpdateMessage::UpToDate(current_version.to_owned()));
    }

    let status = configure_updater()
        .target_version_tag(&format!("v{}", latest.version))
        .build()?
        .update()?;

    if status.updated() {
        Ok(UpdateMessage::Updated(status.version().to_owned()))
    } else {
        Ok(UpdateMessage::UpToDate(status.version().to_owned()))
    }
}
