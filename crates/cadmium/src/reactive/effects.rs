/// Registers an effect and runs it once immediately, returning its [`EffectId`].
///
/// The closure may be `FnMut`, so it can keep mutable state across runs. On
/// every run, signal reads made inside `f` are recorded as dependencies and
/// previously recorded dependencies are discarded first; conditional reads
/// therefore track only the signals actually read in the latest run. Writes
/// to a dependency mark the effect dirty, but do not call `f` until the next
/// [`flush_effects`], which [`crate::window::App`] performs automatically
/// before each frame.
///
/// Keep the returned id to stop the effect later with [`dispose_effect`].
/// Dropping the id alone does not unregister the effect. Widgets typically
/// store effect ids and dispose them when the widget is unmounted.
///
/// # Panics
///
/// If `f` panics, the panic propagates to the caller (or to `flush_effects`
/// for a later run), but the runtime restores its tracking stack and puts the
/// closure back, so subsequently created or scheduled effects still work.
///
/// # Examples
///
/// ```
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// use cadmium::reactive::{create_effect, create_signal, dispose_effect, flush_effects};
///
/// let value = create_signal(2);
/// let seen = Rc::new(Cell::new(0));
/// let observed = seen.clone();
/// let id = create_effect(move || observed.set(value.get() * 3));
/// assert_eq!(seen.get(), 6); // initial run
///
/// value.set(5);
/// assert_eq!(seen.get(), 6); // writes only schedule the effect
/// assert_eq!(flush_effects(), 1);
/// assert_eq!(seen.get(), 15);
///
/// dispose_effect(id);
/// value.set(8);
/// assert_eq!(flush_effects(), 0);
/// value.dispose();
/// ```
pub fn create_effect(f: impl FnMut() + 'static) -> EffectId {
    let id = RUNTIME.with(|rt| {
        rt.borrow_mut().effects.insert(EffectEntry {
            func: Some(Box::new(f)),
            deps: Vec::new(),
        })
    });
    run_effect(id);
    id
}

/// Unregisters an effect and drops all of its signal subscriptions.
///
/// Accepts an [`EffectId`] returned by [`create_effect`]. Unknown, already
/// disposed, or default (null) ids are no-ops, so this function is safe to
/// call defensively. Pending dirty entries for the effect are removed too.
/// Use [`Memo::dispose`] and [`Selector::dispose`] to clean up the effects
/// owned by those wrappers.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::{create_effect, create_signal, dispose_effect, flush_effects};
///
/// let source = create_signal(0);
/// let id = create_effect(move || {
///     let _ = source.get();
/// });
///
/// dispose_effect(id);
/// source.set(1);
/// assert_eq!(flush_effects(), 0); // the effect no longer subscribes
/// source.dispose();
/// ```
pub fn dispose_effect(id: EffectId) {
    RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        rt.detach_deps(id);
        rt.effects.remove(id);
        rt.dirty_set.remove(&id);
        rt.dirty_effects.retain(|&e| e != id);
    });
}

struct EffectRunGuard {
    id: EffectId,
    func: Option<Box<dyn FnMut()>>,
}

impl Drop for EffectRunGuard {
    fn drop(&mut self) {
        let func = self.func.take();
        let id = self.id;
        let _ = RUNTIME.try_with(|rt| {
            let mut rt = rt.borrow_mut();
            if rt.tracking_stack.last() == Some(&id) {
                rt.tracking_stack.pop();
            } else {
                rt.tracking_stack.retain(|&e| e != id);
            }
            if let Some(entry) = rt.effects.get_mut(id) {
                entry.func = func;
            }
        });
    }
}

fn run_effect(id: EffectId) {
    let func = RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        if !rt.effects.contains_key(id) {
            return None;
        }
        rt.detach_deps(id);
        let func = rt.effects.get_mut(id).and_then(|e| e.func.take());
        if func.is_some() {
            rt.tracking_stack.push(id);
        }
        func
    });

    if let Some(f) = func {
        let mut guard = EffectRunGuard { id, func: Some(f) };
        if let Some(cb) = guard.func.as_mut() {
            cb();
        }
    }
}

/// Runs all dirty effects and returns how many effect executions occurred.
///
/// Signal writes only enqueue effects; this function drains that queue in
/// waves so an effect that writes a signal read by another effect settles in
/// the same call. At most 64 waves are processed: if effects are still dirty
/// at that point (for example because of a cycle), an error is logged through
/// `tracing` and the remaining queue is discarded. Returns `0` when nothing
/// was dirty.
///
/// [`crate::window::App`] calls this before rendering each frame and after
/// draining tasks posted via [`UiDispatcher::dispatch`], so applications
/// normally do not need to call it. It acts on the calling thread's runtime,
/// which must be the one that owns the signals.
///
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// use cadmium::reactive::{create_effect, create_signal, flush_effects};
///
/// let source = create_signal(0);
/// let events = Rc::new(RefCell::new(Vec::new()));
/// let log = events.clone();
/// let _id = create_effect(move || log.borrow_mut().push(source.get()));
/// assert_eq!(*events.borrow(), vec![0]); // initial run
///
/// assert_eq!(flush_effects(), 0); // nothing dirty yet
/// source.set(1);
/// assert_eq!(flush_effects(), 1);
/// assert_eq!(*events.borrow(), vec![0, 1]);
/// source.dispose();
/// ```
pub fn flush_effects() -> usize {
    let mut executed = 0;
    for wave in 0..64 {
        let batch: Vec<EffectId> = RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.dirty_set.clear();
            std::mem::take(&mut rt.dirty_effects)
        });
        if batch.is_empty() {
            return executed;
        }
        for id in batch {
            executed += 1;
            run_effect(id);
        }
        if wave == 63 {
            let remaining = RUNTIME.with(|rt| {
                let mut rt = rt.borrow_mut();
                let count = rt.dirty_effects.len();
                rt.dirty_effects.clear();
                rt.dirty_set.clear();
                count
            });
            if remaining > 0 {
                tracing::error!(
                    "Reactive effect cycle detected after 64 waves; dropped {remaining} pending effects"
                );
            }
        }
    }
    executed
}

