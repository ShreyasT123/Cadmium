use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use slotmap::{new_key_type, SlotMap};

new_key_type! {
    pub struct SignalId;
    pub struct EffectId;
}

struct EffectEntry {
    func: Option<Box<dyn FnMut()>>,
    deps: Vec<SignalId>,
}

struct ReactiveRuntime {
    signals: SlotMap<SignalId, Box<dyn Any>>,
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
            self.subscribers.entry(sig).or_default().insert(eff);
            if let Some(entry) = self.effects.get_mut(eff) {
                if !entry.deps.contains(&sig) {
                    entry.deps.push(sig);
                }
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
    pub fn get(self) -> T
    where
        T: Clone,
    {
        self.with(|v| v.clone())
    }

    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.record_read(self.id);
            let boxed = rt.signals.get(self.id).expect("Signal accessed after disposal");
            let val = boxed.downcast_ref::<T>().expect("Signal type mismatch");
            f(val)
        })
    }

    pub fn set(self, value: T) {
        self.update(|slot| *slot = value);
    }

    pub fn update(self, f: impl FnOnce(&mut T)) {
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            if let Some(boxed) = rt.signals.get_mut(self.id) {
                let val = boxed.downcast_mut::<T>().expect("Signal type mismatch");
                f(val);
                rt.notify_write(self.id);
            }
        });
    }

    pub fn dispose(self) {
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.signals.remove(self.id);
            rt.subscribers.remove(&self.id);
        });
    }
}

pub fn create_signal<T: 'static>(initial: T) -> Signal<T> {
    let id = RUNTIME.with(|rt| rt.borrow_mut().signals.insert(Box::new(initial)));
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
    });
}

fn run_effect(id: EffectId) {
    let mut func = RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        rt.detach_deps(id);
        let func = rt.effects.get_mut(id).and_then(|e| e.func.take());
        if func.is_some() {
            rt.tracking_stack.push(id);
        }
        func
    });

    if let Some(mut f) = func.take() {
        f();
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.tracking_stack.pop();
            if let Some(entry) = rt.effects.get_mut(id) {
                entry.func = Some(f);
            }
        });
    }
}

pub fn flush_effects() -> usize {
    let mut executed = 0;
    for _ in 0..64 {
        let batch: Vec<EffectId> = RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.dirty_set.clear();
            std::mem::take(&mut rt.dirty_effects)
        });
        if batch.is_empty() {
            break;
        }
        for id in batch {
            executed += 1;
            run_effect(id);
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
    use std::rc::Rc;

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
}
