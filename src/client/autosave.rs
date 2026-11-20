//! Ordered, bounded autosave writes. Serialization happens in the turn worker.
use std::{path::PathBuf, sync::mpsc, thread};

struct SaveRequest {
    path: PathBuf,
    payload: Vec<u8>,
}

pub(crate) struct AutosaveWriter {
    sender: Option<mpsc::SyncSender<SaveRequest>>,
    results: mpsc::Receiver<Result<(), String>>,
    pending: Option<SaveRequest>,
    worker: Option<thread::JoinHandle<()>>,
}
impl AutosaveWriter {
    pub(crate) fn new() -> Result<Self, String> {
        let (sender, requests) = mpsc::sync_channel::<SaveRequest>(1);
        let (completed, results) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("civcraft-autosave".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    let result = crate::sim::Simulation::write_campaign_payload(
                        &request.path,
                        &request.payload,
                    )
                    .map_err(|error| error.to_string());
                    if completed.send(result).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            sender: Some(sender),
            results,
            pending: None,
            worker: Some(worker),
        })
    }
    pub(crate) fn enqueue(&mut self, path: PathBuf, payload: Vec<u8>) -> Result<(), String> {
        // An autosave has one destination. If storage falls behind, retain only
        // the latest waiting snapshot; active/queued writes still finish in order.
        self.pending = Some(SaveRequest { path, payload });
        self.pump()
    }
    fn pump(&mut self) -> Result<(), String> {
        if let Some(request) = self.pending.take() {
            match self.sender.as_ref().unwrap().try_send(request) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("autosave worker disconnected".into());
                }
            }
        }
        Ok(())
    }
    pub(crate) fn poll_errors(&mut self) -> Vec<String> {
        let mut errors: Vec<_> = self.results.try_iter().filter_map(Result::err).collect();
        if let Err(error) = self.pump() {
            errors.push(error);
        }
        errors
    }
}
impl Drop for AutosaveWriter {
    fn drop(&mut self) {
        // Only shutdown waits: flush the final snapshot before exiting the app.
        if let Some(sender) = self.sender.take() {
            if let Some(request) = self.pending.take() {
                let _ = sender.send(request);
            }
            drop(sender);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shutdown_flushes_the_latest_snapshot_without_out_of_order_overwrites() {
        let path =
            std::env::temp_dir().join(format!("civcraft-save-worker-{}.save", std::process::id()));
        let mut writer = AutosaveWriter::new().unwrap();
        for i in 0..20 {
            writer.enqueue(path.clone(), vec![i]).unwrap();
        }
        drop(writer);
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"CIVCRAFT\0"));
        assert_eq!(bytes.last(), Some(&19));
        std::fs::remove_file(path).unwrap();
    }
}
