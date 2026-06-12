use eframe::egui::Context;
use egui_video::{DecoderConfig, Player};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// Number of background worker threads used to construct [`Player`]s off the UI thread.
const LOADER_WORKER_COUNT: usize = 3;

/// What a finished load is for, so the app can dispatch the resulting [`Player`].
pub(crate) enum LoadPurpose {
    /// Replace the active single-view video. `request_id` lets newer presses supersede this one.
    Single {
        add_to_history: bool,
        request_id: u64,
    },
    /// Preload the upcoming single-view video for the given target folder.
    Queued { folder: Option<usize> },
    /// Fill a multiview tile at the given slot index.
    Tile { index: usize },
}

/// A request to build a [`Player`] on a worker thread.
pub(crate) struct LoadRequest {
    /// Generation token; results with a stale generation are dropped by the app.
    pub generation: u64,
    pub purpose: LoadPurpose,
    pub path: PathBuf,
    /// Target decode box in physical pixels, or `None` for native resolution.
    pub target_size: Option<(u32, u32)>,
    /// Tiles decode cheaply: low quality + a single decoder thread.
    pub economy: bool,
    /// How many times this request has already failed (used to bound retries).
    pub attempts: u8,
}

/// The outcome of a [`LoadRequest`]: the original request plus the constructed player (or error).
pub(crate) struct LoadResult {
    pub request: LoadRequest,
    pub player: Result<Player, String>,
}

/// Owns the background worker threads and the channels used to talk to them.
pub(crate) struct PlayerLoader {
    request_tx: Sender<LoadRequest>,
    result_rx: Receiver<LoadResult>,
    _workers: Vec<thread::JoinHandle<()>>,
}

impl PlayerLoader {
    /// Spawn the worker pool. Workers call [`Context::request_repaint`] after each result so the
    /// UI wakes up even when otherwise idle.
    pub(crate) fn new(ctx: Context) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<LoadRequest>();
        let (result_tx, result_rx) = mpsc::channel::<LoadResult>();
        let shared_rx = Arc::new(Mutex::new(request_rx));

        let mut workers = Vec::with_capacity(LOADER_WORKER_COUNT);
        for _ in 0..LOADER_WORKER_COUNT {
            let shared_rx = Arc::clone(&shared_rx);
            let result_tx = result_tx.clone();
            let ctx = ctx.clone();
            workers.push(thread::spawn(move || {
                worker_loop(&shared_rx, &result_tx, &ctx);
            }));
        }

        Self {
            request_tx,
            result_rx,
            _workers: workers,
        }
    }

    /// Enqueue a load. Silently dropped if the worker pool has gone away.
    pub(crate) fn enqueue(&self, request: LoadRequest) {
        let _ = self.request_tx.send(request);
    }

    /// Pop the next finished load, if any.
    pub(crate) fn try_recv(&self) -> Option<LoadResult> {
        self.result_rx.try_recv().ok()
    }
}

/// Build a single [`Player`] for the given request on the calling (worker) thread.
///
/// Audio devices are intentionally NOT created here: SDL audio must live on the main thread, so
/// `add_audio` is performed by the app after the player arrives.
fn build_player(ctx: &Context, request: &LoadRequest) -> Result<Player, String> {
    let config = if request.economy {
        DecoderConfig {
            max_threads: Some(1),
            low_quality_decode: true,
        }
    } else {
        DecoderConfig::default()
    };

    let input_path = request.path.to_string_lossy().to_string();
    let mut player = Player::new_with_decoder_config(ctx, &input_path, config)
        .map_err(|error| error.to_string())?;
    player.options.looping = false;
    player.options.set_audio_volume(0.0);

    if let Some((width, height)) = request.target_size {
        player.set_target_texture_size(width, height);
    }

    // Tiles start decoding immediately so the cell shows a frame as soon as it is swapped in.
    if matches!(request.purpose, LoadPurpose::Tile { .. }) {
        player.start();
    }

    Ok(player)
}

fn worker_loop(
    shared_rx: &Arc<Mutex<Receiver<LoadRequest>>>,
    result_tx: &Sender<LoadResult>,
    ctx: &Context,
) {
    loop {
        // Hold the lock only long enough to pull one request, so workers stay parallel.
        let request = {
            let guard = match shared_rx.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            match guard.recv() {
                Ok(request) => request,
                Err(_) => return,
            }
        };

        let player = build_player(ctx, &request);
        if result_tx.send(LoadResult { request, player }).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}
