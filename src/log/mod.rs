pub mod mmkv_log {
    use std::sync::atomic::{AtomicBool, Ordering};

    #[cfg(not(target_os = "android"))]
    struct Logger;

    #[cfg(not(target_os = "android"))]
    impl log::Log for Logger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                println!("{} - {}", record.level(), record.args());
            }
        }

        fn flush(&self) {}
    }

    #[cfg(not(target_os = "android"))]
    static LOGGER: Logger = Logger;
    static LOG_SET: AtomicBool = AtomicBool::new(false);

    pub fn init_log() {
        if LOG_SET.load(Ordering::Acquire) {
            return;
        }
        if cfg!(target_os = "android") {
            #[cfg(target_os = "android")]
            android_log::init("MMKV").unwrap();
        } else {
            #[cfg(not(target_os = "android"))]
            {
                log::set_logger(&LOGGER).unwrap();
                log::set_max_level(log::LevelFilter::Info);
            }
        }
        LOG_SET.swap(true, Ordering::Release);
        log::set_max_level(log::LevelFilter::Info);
    }
}
