//! The Iced subscription set: which timers and event streams are live, and
//! how the panel slides settle when motion is reduced.

use crate::ui::UiMessage;
use crate::ui::theme::timing::WATCHER_POLL_INTERVAL;
use iced::{Subscription, time};
use std::time::Duration;

use super::XionApp;

impl XionApp {
    pub(in crate::ui::app) fn subscription(&self) -> Subscription<UiMessage> {
        let mut subscriptions = vec![iced::event::listen_with(super::map_event_to_message)];

        // Reduced motion, or Appearance > Effects off: no animation tick at
        // all. An animated GIF then stays on its first frame, and the panel
        // slides are settled by `update` instead.
        let animate = colony_ui::effects_enabled();

        if animate && self.media.animated.is_some() {
            subscriptions
                .push(time::every(Duration::from_millis(30)).map(UiMessage::AnimatedPreviewTick));
        }

        // Only poll while something is actually being watched. The tick used to
        // fire unconditionally, and in Iced every message rebuilds the widget
        // tree, so an idle window rebuilt itself 1.3 times a second for nothing.
        if self.watched_path.is_some() {
            subscriptions
                .push(time::every(WATCHER_POLL_INTERVAL).map(|_| UiMessage::FileWatchTick));
        }

        if self.operation_progress.is_some() {
            subscriptions.push(
                time::every(Duration::from_millis(100)).map(|_| UiMessage::OperationProgressTick),
            );
        }

        // Preview panel slide animation: only ticks while animating
        if sliding(
            self.preview_anim_progress,
            self.preview_anim_target,
            animate,
        ) {
            subscriptions
                .push(time::every(Duration::from_millis(16)).map(|_| UiMessage::PreviewAnimTick));
        }

        // Terminal panel slide animation: only ticks while animating
        if sliding(
            self.terminal_anim_progress,
            self.terminal_anim_target,
            animate,
        ) {
            subscriptions
                .push(time::every(Duration::from_millis(16)).map(|_| UiMessage::TerminalAnimTick));
        }

        // One stream per live emulator, background tabs included: a build
        // started in one of them produces output nobody would drain otherwise.
        //
        // Replaces a 50 ms poll that ran as long as a shell lived, whether it
        // had written or not, and in iced every message rebuilds the widget
        // tree. Here the pseudo-terminal wakes the interface, and only when it
        // has something to say.
        for tab in &self.terminal.tabs {
            if let Some(terminal) = &tab.terminal {
                subscriptions.push(terminal.subscription().map(UiMessage::TerminalEvent));
            }
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

    /// Puts both panels where their slide was heading when nothing may move.
    pub(in crate::ui::app) fn settle_slides(&mut self, animate: bool) {
        if !animate {
            self.preview_anim_progress = self.preview_anim_target;
            self.terminal_anim_progress = self.terminal_anim_target;
        }
    }
}

/// Whether a panel slide still needs its 16 ms tick.
fn sliding(progress: f32, target: f32, animate: bool) -> bool {
    animate && (progress - target).abs() > 0.001
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduced_motion_never_ticks_a_slide() {
        assert!(sliding(0.0, 1.0, true));
        assert!(!sliding(0.0, 1.0, false));
        assert!(!sliding(1.0, 1.0, true), "a settled panel does not tick");
    }

    #[test]
    fn reduced_motion_jumps_both_panels_to_their_final_state() {
        let mut app = XionApp::new_for_test();
        app.preview_anim_target = 1.0;
        app.terminal_anim_target = 1.0;

        app.settle_slides(true);
        assert_eq!(app.preview_anim_progress, 0.0, "animations on: it slides");

        app.settle_slides(false);
        assert_eq!(app.preview_anim_progress, 1.0);
        assert_eq!(app.terminal_anim_progress, 1.0);
    }
}
