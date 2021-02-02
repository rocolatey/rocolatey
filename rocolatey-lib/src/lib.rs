pub mod roco;

pub static ROCO_VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_verbose_mode(verbose: bool) {
  ROCO_VERBOSE.store(verbose, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_verbose_mode() -> bool {
  ROCO_VERBOSE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn println_verbose(text: &str) {
  if is_verbose_mode() {
    println!("VERBOSE: {}", text);
  }
}
