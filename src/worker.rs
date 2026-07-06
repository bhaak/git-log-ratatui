use std::sync::mpsc;
use std::thread;

/// Generic background worker that processes commands on a dedicated thread.
///
/// Each worker has its own thread (per AGENTS.md), receives commands via an
/// mpsc channel, and sends results back via a separate channel.
pub struct BackgroundWorker<C, R> {
    cmd_tx: mpsc::Sender<C>,
    result_rx: mpsc::Receiver<R>,
}

impl<C, R> BackgroundWorker<C, R>
where
    C: Send + 'static,
    R: Send + 'static,
{
    /// Spawn a new worker thread.
    ///
    /// The closure `f` receives the command receiver and result sender.
    /// It runs on the spawned thread and should process commands in a loop.
    pub fn spawn<F>(f: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<C>, mpsc::Sender<R>) + Send + 'static,
    {
        let (cmd_tx, cmd_rx) = mpsc::channel::<C>();
        let (result_tx, result_rx) = mpsc::channel::<R>();

        thread::spawn(move || {
            f(cmd_rx, result_tx);
        });

        BackgroundWorker { cmd_tx, result_rx }
    }

    /// Send a command to the worker. Returns false if the channel is closed.
    pub fn send(&self, cmd: C) -> bool {
        self.cmd_tx.send(cmd).is_ok()
    }

    /// Try to receive a result (non-blocking).
    pub fn try_recv(&self) -> Option<R> {
        self.result_rx.try_recv().ok()
    }

    /// Receive a result (blocking).
    pub fn recv(&self) -> Option<R> {
        self.result_rx.recv().ok()
    }
}
