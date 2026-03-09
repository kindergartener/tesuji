use std::path::PathBuf;

use iced::Task;

use crate::gui::Message;

pub fn open_file_task() -> Task<Message> {
    Task::perform(
        tokio::task::spawn_blocking(|| -> Option<Result<(PathBuf, String), String>> {
            let path = rfd::FileDialog::new()
                .add_filter("SGF", &["sgf"])
                .pick_file()?;

            const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
            match std::fs::metadata(&path) {
                Ok(meta) if meta.len() > MAX_FILE_SIZE => Some(Err(format!(
                    "File too large ({:.1} MB, max {} MB)",
                    meta.len() as f64 / (1024.0 * 1024.0),
                    MAX_FILE_SIZE / (1024 * 1024),
                ))),
                Err(e) => Some(Err(e.to_string())),
                Ok(_) => Some(
                    std::fs::read_to_string(&path)
                        .map(|text| (path, text))
                        .map_err(|e| e.to_string()),
                ),
            }
        }),
        |join_result| match join_result.ok().flatten() {
            Some(r) => Message::FileOpened(r),
            None => Message::DismissStatus,
        },
    )
}

pub fn save_file_task(path: PathBuf, content: String) -> Task<Message> {
    Task::perform(
        tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
            let mut path = path;
            if path.extension().map_or(true, |ext| ext != "sgf") {
                path.set_extension("sgf");
            }
            std::fs::write(&path, &content)
                .map(|_| path)
                .map_err(|e| e.to_string())
        }),
        |join_result| Message::FileSaved(join_result.unwrap_or_else(|e| Err(e.to_string()))),
    )
}

pub fn save_as_file_task(content: String) -> Task<Message> {
    Task::perform(
        tokio::task::spawn_blocking(move || -> Option<Result<PathBuf, String>> {
            let mut path = rfd::FileDialog::new()
                .add_filter("SGF", &["sgf"])
                .save_file()?;
            if path.extension().map_or(true, |ext| ext != "sgf") {
                path.set_extension("sgf");
            }
            Some(
                std::fs::write(&path, &content)
                    .map(|_| path)
                    .map_err(|e| e.to_string()),
            )
        }),
        |join_result| match join_result.ok().flatten() {
            Some(r) => Message::FileSaved(r),
            None => Message::DismissStatus,
        },
    )
}
