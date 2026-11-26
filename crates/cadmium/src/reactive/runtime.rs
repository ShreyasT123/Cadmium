type SignalCell = Rc<RefCell<Box<dyn Any>>>;

struct EffectEntry {
    func: Option<Box<dyn FnMut()>>,
    deps: Vec<SignalId>,
}

struct ReactiveRuntime {
    signals: SlotMap<SignalId, SignalCell>,
    subscribers: HashMap<SignalId, HashSet<EffectId>>,
    effects: SlotMap<EffectId, EffectEntry>,
    tracking_stack: Vec<EffectId>,
    dirty_effects: Vec<EffectId>,
    dirty_set: HashSet<EffectId>,
    wake_cb: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ReactiveRuntime {
    fn new() -> Self {
        Self {
            signals: SlotMap::with_key(),
            subscribers: HashMap::new(),
            effects: SlotMap::with_key(),
            tracking_stack: Vec::new(),
            dirty_effects: Vec::new(),
            dirty_set: HashSet::new(),
            wake_cb: None,
        }
    }

    fn record_read(&mut self, sig: SignalId) {
        if let Some(&eff) = self.tracking_stack.last() {
            if !self.effects.contains_key(eff) {
                return;
            }
            self.subscribers.entry(sig).or_default().insert(eff);
            if let Some(entry) = self.effects.get_mut(eff)
                && !entry.deps.contains(&sig)
            {
                entry.deps.push(sig);
            }
        }
    }

    fn notify_write(&mut self, sig: SignalId) {
        if let Some(subs) = self.subscribers.get(&sig) {
            for &eff in subs {
                if self.effects.contains_key(eff) && self.dirty_set.insert(eff) {
                    self.dirty_effects.push(eff);
                }
            }
        }
    }

    fn detach_deps(&mut self, eff: EffectId) {
        if let Some(entry) = self.effects.get_mut(eff) {
            let old_deps = std::mem::take(&mut entry.deps);
            for sig in old_deps {
                if let Some(subs) = self.subscribers.get_mut(&sig) {
                    subs.remove(&eff);
                }
            }
        }
    }
}

thread_local! {
    static RUNTIME: RefCell<ReactiveRuntime> = RefCell::new(ReactiveRuntime::new());
}

/// Stores a thread-local callback that can wake the UI event loop.
///
/// [`crate::window::App::new`] installs a wrapper around the window's event-loop
/// proxy here, allowing other threads to request attention from the UI thread.
/// Calling this function again replaces the callback for the calling thread
/// only. The callback must be `Send + Sync` because it may be shared with
/// worker threads.
///
/// The reactive runtime does **not** invoke this callback when a signal is
/// written: [`crate::window::App`] handles scheduling redraws as it drains
/// asynchronous tasks and renders frames. [`UiDispatcher::dispatch`] invokes
/// its own clone of the wake function after enqueueing a task.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// use cadmium::reactive::set_wake_callback;
///
/// let wakes = Arc::new(AtomicUsize::new(0));
/// let counter = wakes.clone();
/// set_wake_callback(Arc::new(move || {
///     counter.fetch_add(1, Ordering::Relaxed);
/// }));
/// ```
pub fn set_wake_callback(cb: Arc<dyn Fn() + Send + Sync>) {
    RUNTIME.with(|rt| {
        rt.borrow_mut().wake_cb = Some(cb);
    });
}

