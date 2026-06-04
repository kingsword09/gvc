use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

pub fn init(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

#[macro_export]
macro_rules! outln {
    ($($arg:tt)*) => {
        if !$crate::utils::output::is_quiet() {
            println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! out {
    ($($arg:tt)*) => {
        if !$crate::utils::output::is_quiet() {
            print!($($arg)*);
        }
    };
}
