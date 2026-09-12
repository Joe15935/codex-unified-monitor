//! Cooperative shutdown must keep the event loop alive until its worker returns.
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread::JoinHandle;

#[derive(Default)]
pub struct Shutdown {
    // 0 = running, 1 = waiting for the worker, 2 = safe to exit.
    phase: AtomicU8,
}

impl Shutdown {
    /// Only the first exit request starts cleanup. Repeated Quit clicks wait.
    pub fn begin(&self) -> bool {
        self.phase
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn is_finished(&self) -> bool {
        self.phase.load(Ordering::Acquire) == 2
    }

    pub fn finish(&self) {
        self.phase.store(2, Ordering::Release);
    }
}

/// Never call JoinHandle::join on the UI thread: tray setters may be waiting for
/// that same thread. Completion also waits for the worker's owned child Drop.
pub fn after_worker(
    worker: Option<JoinHandle<()>>,
    complete: impl FnOnce() + Send + 'static,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        complete();
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn exit_waits_for_main_thread_rendezvous_and_worker_cleanup_without_blocking_ui() {
        let state = Arc::new(Shutdown::default());
        let (ui_request_tx, ui_request_rx) = mpsc::channel();
        let (ui_reply_tx, ui_reply_rx) = mpsc::channel();
        let (cleaned_tx, cleaned_rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        struct OwnedResource(mpsc::Sender<()>);
        impl Drop for OwnedResource {
            fn drop(&mut self) {
                self.0.send(()).unwrap();
            }
        }
        let worker = std::thread::spawn(move || {
            let _owned_child = OwnedResource(cleaned_tx);
            // This models Tauri's tray setter dispatch + blocking response wait.
            ui_request_tx.send(()).unwrap();
            ui_reply_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        ui_request_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(state.begin());
        assert!(!state.begin());
        let done = state.clone();
        let coordinator = after_worker(Some(worker), move || {
            done.finish();
            exit_tx.send(()).unwrap();
        });
        assert!(!state.is_finished());
        assert!(exit_rx.try_recv().is_err());
        // An active event loop can still answer the worker's pending UI request.
        ui_reply_tx.send(()).unwrap();
        exit_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(state.is_finished());
        assert!(cleaned_rx.try_recv().is_ok());
        assert!(!state.begin());
        coordinator.join().unwrap();
    }

    #[test]
    fn an_already_failed_or_absent_worker_does_not_leave_exit_waiting_forever() {
        for worker in [
            None,
            Some(std::thread::spawn(|| panic!("synthetic worker failure"))),
        ] {
            let (tx, rx) = mpsc::channel();
            let coordinator = after_worker(worker, move || tx.send(()).unwrap());
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
            coordinator.join().unwrap();
        }
    }
}
