use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn init(cli_verbose: bool) {
    VERBOSE.store(
        cli_verbose || std::env::var("GVC_VERBOSE").is_ok(),
        Ordering::Relaxed,
    );
}

pub fn is_enabled() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn log(message: impl AsRef<str>) {
    if is_enabled() {
        eprintln!("[VERBOSE] {}", message.as_ref());
    }
}
