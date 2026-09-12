//! Small native UI policies, independent of the event loop and account provider.
use std::sync::atomic::{AtomicBool, Ordering};

pub struct WindowLifecycle {
    background: bool,
    main_ready: AtomicBool,
}

impl WindowLifecycle {
    pub fn new(background: bool) -> Self {
        Self {
            background,
            main_ready: AtomicBool::new(false),
        }
    }

    /// The React loading/error/data view is committed before its first show.
    /// A compact handshake must not consume the main window's startup decision.
    pub fn ready_should_show(&self, label: &str) -> bool {
        label == "main" && !self.main_ready.swap(true, Ordering::AcqRel) && !self.background
    }
}

#[derive(Default)]
pub struct ChangedText(Option<String>);

impl ChangedText {
    /// Record only a successful native update; failures are retried next time.
    pub fn deliver<E>(
        &mut self,
        text: String,
        send: impl FnOnce(&str) -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.0.as_deref() == Some(&text) {
            return Ok(false);
        }
        send(&text)?;
        self.0 = Some(text);
        Ok(true)
    }
}

pub fn needs_local_notification(previous_status: &str, status: &str, index_changed: bool) -> bool {
    previous_status != status || index_changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreground_start_waits_for_main_commit_and_repeated_commits_do_not_steal_focus() {
        let state = WindowLifecycle::new(false);
        assert!(!state.ready_should_show("compact"));
        assert!(!state.ready_should_show("unrecognized"));
        assert!(state.ready_should_show("main"));
        assert!(!state.ready_should_show("main"));
    }

    #[test]
    fn background_start_never_shows_a_window_from_frontend_readiness() {
        let state = WindowLifecycle::new(true);
        assert!(!state.ready_should_show("compact"));
        assert!(!state.ready_should_show("main"));
        assert!(!state.ready_should_show("main"));
    }

    #[test]
    fn tray_delivery_deduplicates_exact_text_but_keeps_value_language_and_status_changes() {
        let mut state = ChangedText::default();
        let mut delivered = vec![];
        for text in [
            "C 80%",
            "C 80%",
            "C 79%",
            "Weekly · 79% · LIVE",
            "每周 · 79% · 实时",
            "每周 · 79% · 过期",
        ] {
            let _: Result<_, ()> = state.deliver(text.into(), |value| {
                delivered.push(value.to_owned());
                Ok(())
            });
        }
        assert_eq!(delivered.len(), 5);
        assert_eq!(delivered[1], "C 79%");
        assert_eq!(delivered.last().unwrap(), "每周 · 79% · 过期");
    }

    #[test]
    fn failed_tray_delivery_is_retried_instead_of_silently_cached() {
        let mut state = ChangedText::default();
        assert_eq!(state.deliver("LIVE".into(), |_| Err("busy")), Err("busy"));
        assert_eq!(
            state.deliver("LIVE".into(), |_| Ok::<_, &str>(())),
            Ok(true)
        );
        assert_eq!(
            state.deliver("LIVE".into(), |_| Err("must not run")),
            Ok(false)
        );
    }

    #[test]
    fn metadata_only_checks_stay_quiet_but_index_and_health_changes_notify() {
        assert!(!needs_local_notification("watching", "watching", false));
        assert!(needs_local_notification("watching", "watching", true));
        assert!(needs_local_notification("scanning", "watching", false));
        assert!(needs_local_notification("watching", "ingest error", false));
        assert!(needs_local_notification("ingest error", "watching", false));
    }
}
