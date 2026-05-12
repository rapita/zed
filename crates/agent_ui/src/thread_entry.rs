use gpui::{App, Entity, FocusHandle, Focusable, SharedString, Window};

use crate::{cli_conversation_view::CliConversationView, conversation_view::ConversationView};

#[derive(Clone)]
pub enum ThreadEntry {
    Acp(Entity<ConversationView>),
    Cli(Entity<CliConversationView>),
}

impl ThreadEntry {
    pub fn display_title(&self, cx: &App) -> SharedString {
        match self {
            Self::Acp(cv) => cv.read(cx).title(cx),
            Self::Cli(ccv) => ccv.read(cx).title(cx),
        }
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Acp(cv) => cv.read(cx).focus_handle(cx),
            Self::Cli(ccv) => ccv.read(cx).focus_handle(cx),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus_handle(cx).focus(window, cx);
    }

    pub fn is_cli(&self) -> bool {
        matches!(self, Self::Cli(_))
    }

    pub fn is_acp(&self) -> bool {
        matches!(self, Self::Acp(_))
    }

    pub fn as_acp(&self) -> Option<&Entity<ConversationView>> {
        match self {
            Self::Acp(cv) => Some(cv),
            _ => None,
        }
    }

    pub fn as_cli(&self) -> Option<&Entity<CliConversationView>> {
        match self {
            Self::Cli(ccv) => Some(ccv),
            _ => None,
        }
    }
}
