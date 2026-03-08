use std::path::PathBuf;

use iced::Task;

use crate::gui::Message;

pub fn open_file_task() -> Task<Message> {
    Task::perform(
        async {
            let handle = rfd::AsyncFileDialog::new()
                .add_filter("SGF", &["sgf"])
                .pick_file()
                .await;

            match handle {
                None => None,
                Some(h) => {
                    let path = h.path().to_path_buf();
                    // Check file size before reading to prevent OOM on huge files
                    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
                    match tokio::fs::metadata(&path).await {
                        Ok(meta) if meta.len() > MAX_FILE_SIZE => Some(Err(format!(
                            "File too large ({:.1} MB, max {} MB)",
                            meta.len() as f64 / (1024.0 * 1024.0),
                            MAX_FILE_SIZE / (1024 * 1024),
                        ))),
                        Err(e) => Some(Err(e.to_string())),
                        Ok(_) => match tokio::fs::read_to_string(&path).await {
                            Ok(text) => Some(Ok((path, text))),
                            Err(e) => Some(Err(e.to_string())),
                        },
                    }
                }
            }
        },
        |result| match result {
            Some(r) => Message::FileOpened(r),
            None => Message::DismissStatus,
        },
    )
}

pub fn save_file_task(path: PathBuf, content: String) -> Task<Message> {
    Task::perform(
        async move {
            let mut path = path;
            if path.extension().map_or(true, |ext| ext != "sgf") {
                path.set_extension("sgf");
            }
            tokio::fs::write(&path, &content)
                .await
                .map(|_| path)
                .map_err(|e| e.to_string())
        },
        Message::FileSaved,
    )
}

pub fn save_as_file_task(content: String) -> Task<Message> {
    Task::perform(
        async move {
            let handle = rfd::AsyncFileDialog::new()
                .add_filter("SGF", &["sgf"])
                .save_file()
                .await;

            match handle {
                None => None,
                Some(h) => {
                    let mut path = h.path().to_path_buf();
                    if path.extension().map_or(true, |ext| ext != "sgf") {
                        path.set_extension("sgf");
                    }
                    match tokio::fs::write(&path, &content).await {
                        Ok(_) => Some(Ok(path)),
                        Err(e) => Some(Err(e.to_string())),
                    }
                }
            }
        },
        |result| match result {
            Some(r) => Message::FileSaved(r),
            None => Message::DismissStatus,
        },
    )
}
