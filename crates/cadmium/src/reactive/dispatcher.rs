/// A cloneable handle for scheduling closures to run on the UI thread.
///
/// Obtain one from [`crate::window::App::dispatcher`]. Internally it holds an
/// `mpsc::Sender` of tasks plus a clone of the event-loop wake function; both
/// are thread-safe, so the dispatcher can be moved or cloned into worker
/// threads. [`UiDispatcher::dispatch`] enqueues a `Send + 'static` closure and
/// wakes the event loop. `App` drains queued tasks on the UI thread and then
/// flushes reactive effects, so signals written by a dispatched task are
/// visible to widgets on the next frame. Signal handles are thread-local;
/// create them on the UI thread and copy them into the task rather than
/// creating new ones on the worker.
#[derive(Clone)]
pub struct UiDispatcher {
    tx: Sender<Box<dyn FnOnce() + Send>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl UiDispatcher {
    pub(crate) fn new(
        tx: Sender<Box<dyn FnOnce() + Send>>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self { tx, wake }
    }

    /// Enqueues `f` for execution on the UI thread and wakes the event loop.
    ///
    /// The closure must be `Send + 'static` because it crosses a thread
    /// boundary and may outlive the caller. Execution is asynchronous: the
    /// closure runs when [`crate::window::App`] next drains its task queue,
    /// not during this call. If the receiving end of the channel is gone (the
    /// app has stopped), the task is silently discarded and the wake function
    /// is not invoked.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// use cadmium::prelude::*;
    ///
    /// let app = App::new();
    /// let dispatcher = app.dispatcher();
    /// let count = create_signal(0usize);
    ///
    /// let completions = Arc::new(AtomicUsize::new(0));
    /// let flag = completions.clone();
    /// let d = dispatcher.clone();
    /// std::thread::spawn(move || {
    ///     d.dispatch(move || {
    ///         count.update(|n| *n += 1);
    ///         flag.store(1, Ordering::SeqCst);
    ///     });
    /// });
    ///
    /// // `App` drains the task on the UI thread and flushes effects before
    /// // drawing a frame; starting the event loop requires a real display.
    /// let _ = app.run();
    /// ```
    pub fn dispatch(&self, f: impl FnOnce() + Send + 'static) {
        if self.tx.send(Box::new(f)).is_ok() {
            (self.wake)();
        }
    }
}

