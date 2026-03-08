use iced::Subscription;
use iced::keyboard::{Event, Key, Modifiers, key::Named};

use super::Message;

pub enum Hotkey {
    NavigatePrev,
    NavigateNext,
    NavigatePrevVariation,
    NavigateNextVariation,
    NavigateFirstVariation,
    NavigateLastVariation,
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
            Hotkey::NavigatePrevVariation => Message::NavigatePrevVariation,
            Hotkey::NavigateNextVariation => Message::NavigateNextVariation,
            Hotkey::NavigateFirstVariation => Message::NavigateFirstVariation,
            Hotkey::NavigateLastVariation => Message::NavigateLastVariation,
            Hotkey::NavigateFirst => Message::NavigateFirst,
            Hotkey::NavigateLast => Message::NavigateLast,
            Hotkey::Undo => Message::UndoRequested,
            Hotkey::Redo => Message::RedoRequested,
        }
    }
}

fn handle_key_press(key: Key, modifiers: Modifiers) -> Option<Message> {
    match key {
        Key::Named(Named::ArrowUp) if modifiers.shift() => Some(Hotkey::NavigateFirst.to_message()),
        Key::Named(Named::ArrowDown) if modifiers.shift() => {
            Some(Hotkey::NavigateLast.to_message())
        }
        Key::Named(Named::ArrowLeft) if modifiers.shift() => {
            Some(Hotkey::NavigateFirstVariation.to_message())
        }
        Key::Named(Named::ArrowRight) if modifiers.shift() => {
            Some(Hotkey::NavigateLastVariation.to_message())
        }
        Key::Named(Named::ArrowUp) => Some(Hotkey::NavigatePrev.to_message()),
        Key::Named(Named::ArrowDown) => Some(Hotkey::NavigateNext.to_message()),
        Key::Named(Named::ArrowLeft) => Some(Hotkey::NavigatePrevVariation.to_message()),
        Key::Named(Named::ArrowRight) => Some(Hotkey::NavigateNextVariation.to_message()),
        // Ctrl+Shift+Z → Redo (must come before Ctrl+Z)
        Key::Character(ref c) if c.as_str() == "z" && modifiers.control() && modifiers.shift() => {
            Some(Hotkey::Redo.to_message())
        }
        Key::Character(ref c) if c.as_str() == "z" && modifiers.control() => {
            Some(Hotkey::Undo.to_message())
        }
        Key::Named(Named::F3) => Some(Message::ToggleFps),
        _ => None,
    }
}

pub fn subscription() -> Subscription<Message> {
    iced::keyboard::listen().filter_map(|event| match event {
        Event::KeyPressed { key, modifiers, .. } => handle_key_press(key, modifiers),
        _ => None,
    })
}
