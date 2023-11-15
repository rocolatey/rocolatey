use std::sync::atomic::{AtomicBool, Ordering};

pub mod roco;

pub static ROCO_VERBOSE: AtomicBool = AtomicBool::new(false);
pub static ROCO_REQUIRE_SSL: AtomicBool = AtomicBool::new(false);

pub fn set_ssl_enabled(enable_ssl: bool) {
    ROCO_REQUIRE_SSL.store(enable_ssl, Ordering::Relaxed);
}

pub fn is_ssl_required() -> bool {
    ROCO_REQUIRE_SSL.load(Ordering::Relaxed)
}

pub fn set_verbose_mode(verbose: bool) {
    ROCO_VERBOSE.store(verbose, Ordering::Relaxed);
}

pub fn is_verbose_mode() -> bool {
    ROCO_VERBOSE.load(Ordering::Relaxed)
}

pub fn println_verbose(text: &str) {
    if is_verbose_mode() {
        println!("VERBOSE: {}", text);
    }
}
