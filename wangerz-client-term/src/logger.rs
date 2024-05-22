use std::{
    fs::{File, OpenOptions},
    io::Write,
    sync::Mutex,
};

pub(crate) struct Logger {
    file: Mutex<File>,
}

impl Logger {
    pub(crate) fn new(file: &str) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .expect("Unable to open log file");

        Logger {
            file: Mutex::new(file),
        }
    }

    pub(crate) fn log(&self, message: &str) {
        if let Ok(mut file) = self.file.lock() {
            if writeln!(file, "{}", message).is_err() {
                eprintln!("Failed to write to log file");
            }
        } else {
            eprintln!("Log file lock is poisoned")
        }
    }
}
