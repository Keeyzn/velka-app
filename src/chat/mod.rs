//! Chat Twitch intégré : connexion IRC, analyse des messages, emotes.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))] // données reçues du réseau, aucune indexation qui panique
pub mod client;
pub mod emotes;
pub mod extra;
pub mod parse;

use client::{AuthState, ConnState};
use parse::ChatMessage;
use std::collections::VecDeque;

/// Nombre maximal de messages gardés en mémoire.
pub const MAX_MESSAGES: usize = 500;

pub struct ChatState {
    pub messages: VecDeque<ChatMessage>,
    pub conn: ConnState,
    pub auth: AuthState,
    /// La chaîne est en mode « followers-only ».
    pub followers_only: bool,
    /// Notre compte est modérateur, VIP, diffuseur ou abonné (dispensé du mode followers-only).
    pub exempt: bool,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            conn: ConnState::Connecting,
            auth: AuthState::Anonymous,
            followers_only: false,
            exempt: false,
        }
    }

    pub fn push(&mut self, m: ChatMessage) {
        if self.messages.len() >= MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.messages.push_back(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limite_de_messages() {
        let mut s = ChatState::new();
        for i in 0..(MAX_MESSAGES + 50) {
            s.push(ChatMessage {
                user: i.to_string(),
                color: None,
                segments: vec![],
                is_action: false,
                is_reply: false,
                is_mention: false,
                highlight: false,
                time: String::new(),
                badges: Vec::new(),
            });
        }
        assert_eq!(s.messages.len(), MAX_MESSAGES);
        assert_eq!(s.messages.front().map(|m| m.user.as_str()), Some("50"));
    }
}
