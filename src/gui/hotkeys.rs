use iced::keyboard::{Event, Key, Modifiers, key::Named};
use iced::Subscription;

use super::Message;

pub enum Hotkey {
    NavigatePrev,
    NavigateNext,
    NavigateFirst,
    NavigateLast,
    Undo,
    Redo,
}

impl Hotkey {
    pub fn to_message(self) -> Message {
        match self {
            Hotkey::NavigatePrev => Message::NavigatePrev,
            Hotkey::NavigateNext => Message::NavigateNext,
            Hotkey::NavigateFirst => Message::NavigateFirst,
            Hotkey::NavigateLast => Message::NavigateLast,
            Hotkey::Undo => Message::UndoRequested,
            Hotkey::Redo => Message::RedoRequested,
        }
    }
}

fn handle_key_press(key: Key, modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Named(Named::ArrowLeft) if modifiers.shift() => {
            Some(Hotkey::NavigateFirst.to_message())
        }
        Key::Named(Named::ArrowRight) if modifiers.shift() => {
            Some(Hotkey::NavigateLast.to_message())
        }
        Key::Named(Named::ArrowLeft) => Some(Hotkey::NavigatePrev.to_message()),
        Key::Named(Named::ArrowRight) => Some(Hotkey::NavigateNext.to_message()),
        // Ctrl+Shift+Z → Redo (must come before Ctrl+Z)
        Key::Character(ref c) if c.as_str() == "z" && modifiers.control() && modifiers.shift() => {
            Some(Hotkey::Redo.to_message())
        }
        Key::Character(ref c) if c.as_str() == "z" && modifiers.control() => {
            Some(Hotkey::Undo.to_message())
        }
        _ => None,
    }
}

pub fn subscription() -> Subscription<Message> {
    iced::keyboard::listen().filter_map(|event| match event {
        Event::KeyPressed { key, modifiers, .. } => handle_key_press(key, modifiers),
        _ => None,
    })
}
