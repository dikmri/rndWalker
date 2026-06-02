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

    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .show_download_progress(false)
        .show_output(false)
        .no_confirm(true)
        .current_version(self_update::cargo_crate_version!())
        .build()
        .and_then(|updater| updater.update());

    match status {
        Ok(status) if status.updated() => UpdateMessage::Updated(status.version().to_owned()),
        Ok(status) => UpdateMessage::UpToDate(status.version().to_owned()),
        Err(error) => UpdateMessage::Failed(error.to_string()),
    }
}
