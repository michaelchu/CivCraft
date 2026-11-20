//! Small owned jobs: results cross channels, never shared mutable game state.
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread,
};

pub(crate) struct Task<T> {
    receiver: mpsc::Receiver<Result<T, String>>,
}
impl<T: Send + 'static> Task<T> {
    pub(crate) fn spawn(
        name: &str,
        run: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|panic| {
                    let message = panic
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| panic.downcast_ref::<&str>().copied())
                        .unwrap_or("worker panicked");
                    Err(message.to_owned())
                });
                let _ = sender.send(result);
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { receiver })
    }
    #[cfg(test)]
    pub(crate) fn wait(self) -> Result<T, String> {
        self.receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|error| error.to_string())?
    }

    pub(crate) fn poll(&self) -> Option<Result<T, String>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("background worker disconnected".to_owned()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polling_does_not_wait_for_a_blocked_job_and_reports_panics() {
        let (send, receive) = mpsc::channel();
        let task = Task::spawn("test-background", move || {
            receive.recv().unwrap();
            Ok(42)
        })
        .unwrap();
        assert!(task.poll().is_none());
        send.send(()).unwrap();
        assert_eq!(task.receiver.recv().unwrap().unwrap(), 42);
        let task = Task::<()>::spawn("test-panic", || panic!("expected failure")).unwrap();
        assert_eq!(
            task.receiver.recv().unwrap().unwrap_err(),
            "expected failure"
        );
    }
}
