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
                    match tokio::fs::read_to_string(&path).await {
                        Ok(text) => Some(Ok((path, text))),
                        Err(e) => Some(Err(e.to_string())),
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
                    let path = h.path().to_path_buf();
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
