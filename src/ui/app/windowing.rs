//! The Iced subscription set: which timers and event streams are live.
//!
//! Moved verbatim out of the single 809-line `impl XionApp` block in `mod.rs`.

use crate::ui::UiMessage;
use crate::ui::theme::timing::WATCHER_POLL_INTERVAL;
use iced::{Subscription, time};
use std::time::Duration;

use super::XionApp;

impl XionApp {
    pub(in crate::ui::app) fn subscription(&self) -> Subscription<UiMessage> {
        let mut subscriptions = vec![iced::event::listen_with(super::map_event_to_message)];

        if self.media.animated.is_some() {
            subscriptions
                .push(time::every(Duration::from_millis(30)).map(UiMessage::AnimatedPreviewTick));
        }

        // Only poll while something is actually being watched. The tick used to
        // fire unconditionally, and in Iced every message rebuilds the widget
        // tree — so an idle window rebuilt itself 1.3 times a second for nothing.
        if self.watched_path.is_some() {
            subscriptions
                .push(time::every(WATCHER_POLL_INTERVAL).map(|_| UiMessage::FileWatchTick));
        }

        if self.operation_progress.is_some() {
            subscriptions.push(
                time::every(Duration::from_millis(100)).map(|_| UiMessage::OperationProgressTick),
            );
        }

        // Preview panel slide animation — only ticks while animating
        if (self.preview_anim_progress - self.preview_anim_target).abs() > 0.001 {
            subscriptions
                .push(time::every(Duration::from_millis(16)).map(|_| UiMessage::PreviewAnimTick));
        }

        // Terminal panel slide animation — only ticks while animating
        if (self.terminal_anim_progress - self.terminal_anim_target).abs() > 0.001 {
            subscriptions
                .push(time::every(Duration::from_millis(16)).map(|_| UiMessage::TerminalAnimTick));
        }

        // Any tab with a live process, not just the active one: a build running
        // in a background tab used to produce output nobody ever drained.
        if self
            .terminal
            .tabs
            .iter()
            .any(|tab| tab.process.as_ref().is_some_and(|p| p.is_running()))
        {
            subscriptions.push(
                time::every(Duration::from_millis(50)).map(|_| UiMessage::TerminalPollOutput),
            );
        }

        // Later launches hand their path to this process instead of starting a
        // second window. Polling rather than pushing, because the D-Bus handler
        // runs on its own thread and Iced only accepts messages from its own
        // loop; a quarter of a second is imperceptible when a window has just
        // been asked to appear, and costs one `try_recv` on an empty channel.
        if self.single_instance.is_some() {
            subscriptions
                .push(time::every(Duration::from_millis(250)).map(|_| UiMessage::OpenRequestTick));
        }

        Subscription::batch(subscriptions)
    }
}
