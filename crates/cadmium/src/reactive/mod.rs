use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use slotmap::{SlotMap, new_key_type};

new_key_type! {
    pub struct SignalId;
    pub struct EffectId;
}

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
        if let Some(cb) = &self.wake_cb {
            cb();
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

pub fn set_wake_callback(cb: Arc<dyn Fn() + Send + Sync>) {
    RUNTIME.with(|rt| {
        rt.borrow_mut().wake_cb = Some(cb);
    });
}

pub struct Signal<T: 'static> {
    id: SignalId,
    _marker: PhantomData<T>,
}

impl<T: 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Signal<T> {}

impl<T: 'static> Signal<T> {
    pub fn is_alive(self) -> bool {
        RUNTIME.with(|rt| rt.borrow().signals.contains_key(self.id))
    }

    pub fn try_get(self) -> Option<T>
    where
        T: Clone,
    {
        self.try_with(Clone::clone)
    }

    pub fn get(self) -> T
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn try_with<R>(self, f: impl FnOnce(&T) -> R) -> Option<R> {
        let cell = RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            if !rt.signals.contains_key(self.id) {
                return None;
            }
            rt.record_read(self.id);
            rt.signals.get(self.id).cloned()
        })?;
        let borrowed = cell.borrow();
        let val = borrowed.downcast_ref::<T>().expect("Signal type mismatch");
        Some(f(val))
    }

    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("Signal accessed after disposal")
    }

    pub fn set(self, value: T) {
        self.update(|slot| *slot = value);
    }

    pub fn update(self, f: impl FnOnce(&mut T)) {
        let cell = RUNTIME.with(|rt| rt.borrow().signals.get(self.id).cloned());
        if let Some(cell) = cell {
            {
                let mut borrowed = cell.borrow_mut();
                let val = borrowed.downcast_mut::<T>().expect("Signal type mismatch");
                f(val);
            }
            RUNTIME.with(|rt| {
                rt.borrow_mut().notify_write(self.id);
            });
        }
    }

    pub fn dispose(self) {
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.signals.remove(self.id);
            rt.subscribers.remove(&self.id);
        });
    }
}

/// RAII wrapper around a `Signal<T>` that automatically frees its slot on drop.
pub struct OwnedSignal<T: 'static> {
    inner: Signal<T>,
}

impl<T: 'static> OwnedSignal<T> {
    pub fn new(initial: T) -> Self {
        Self {
            inner: create_signal(initial),
        }
    }

    pub fn signal(&self) -> Signal<T> {
        self.inner
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner.get()
    }

    pub fn set(&self, value: T) {
        self.inner.set(value);
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.inner.update(f);
    }
}

impl<T: 'static> Drop for OwnedSignal<T> {
    fn drop(&mut self) {
        let _ = RUNTIME.try_with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.signals.remove(self.inner.id);
            rt.subscribers.remove(&self.inner.id);
        });
    }
}

pub fn create_signal<T: 'static>(initial: T) -> Signal<T> {
    let cell: SignalCell = Rc::new(RefCell::new(Box::new(initial)));
    let id = RUNTIME.with(|rt| rt.borrow_mut().signals.insert(cell));
    Signal {
        id,
        _marker: PhantomData,
    }
}

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

    pub fn dispatch(&self, f: impl FnOnce() + Send + 'static) {
        if self.tx.send(Box::new(f)).is_ok() {
            (self.wake)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn signal_tracks_and_reruns_effects() {
        let count = create_signal(1);
        let observed = Rc::new(Cell::new(0));
        let obs_clone = observed.clone();

        let eff = create_effect(move || {
            obs_clone.set(count.get() * 10);
        });
        assert_eq!(observed.get(), 10);

        count.set(4);
        assert_eq!(observed.get(), 10);
        assert_eq!(flush_effects(), 1);
        assert_eq!(observed.get(), 40);

        dispose_effect(eff);
        count.set(9);
        assert_eq!(flush_effects(), 0);
        assert_eq!(observed.get(), 40);
        count.dispose();
    }

    #[test]
    fn owned_signal_cleans_up_on_drop_and_try_get_is_safe() {
        let handle = {
            let owned = OwnedSignal::new(42);
            let sig = owned.signal();
            assert_eq!(sig.try_get(), Some(42));
            assert!(sig.is_alive());
            sig
        };
        assert!(!handle.is_alive());
        assert_eq!(handle.try_get(), None);
    }

    #[test]
    fn nested_reads_inside_update_and_with_do_not_panic() {
        let a = create_signal(5);
        let b = create_signal(10);
        a.update(|v| *v += b.get());
        assert_eq!(a.get(), 15);

        let sum = a.with(|av| *av + b.get());
        assert_eq!(sum, 25);
        a.dispose();
        b.dispose();
    }

    #[test]
    fn effect_panic_restores_tracking_stack() {
        let trigger = create_signal(0);
        let clean_sig = create_signal(1);
        let observed = Rc::new(Cell::new(0));

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            create_effect(move || {
                let _ = trigger.get();
                panic!("boom inside effect");
            });
        }));
        assert!(res.is_err());

        let obs = observed.clone();
        let eff = create_effect(move || {
            obs.set(clean_sig.get());
        });
        assert_eq!(observed.get(), 1);

        clean_sig.set(7);
        flush_effects();
        assert_eq!(observed.get(), 7);

        dispose_effect(eff);
        trigger.dispose();
        clean_sig.dispose();
    }
}
