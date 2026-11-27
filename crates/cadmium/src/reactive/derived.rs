/// Reactive per-key selection state derived from one source [`Signal`].
///
/// Created by [`create_selector`], a selector lazily allocates a boolean
/// signal for each key passed to [`Selector::is_selected`]. An internal effect
/// watches the source and flips only those booleans whose selected state
/// actually changed. Consequently, a widget that subscribes to key `A` is not
/// invalidated when the selection moves from `B` to `C`.
///
/// `Selector<T>` is `Clone`, but clones share the same `Rc`-backed state and
/// are not thread-safe. Call [`Selector::dispose`] when finished: it cancels
/// the tracking effect and disposes every per-key signal. Do not use clones
/// afterwards; queries may create fresh signals from the current source value,
/// but nothing will keep them updated.
pub struct Selector<T: Eq + std::hash::Hash + Clone + 'static> {
    inner: Rc<SelectorInner<T>>,
}

struct SelectorInner<T: Eq + std::hash::Hash + Clone + 'static> {
    source: Signal<T>,
    subs: Rc<RefCell<HashMap<T, Signal<bool>>>>,
    _effect: EffectId,
}

impl<T: Eq + std::hash::Hash + Clone + 'static> Clone for Selector<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Eq + std::hash::Hash + Clone + 'static> Selector<T> {
    /// Returns `true` when `key` is equal to the selector's current source
    /// value.
    ///
    /// The first query for a key creates a boolean signal initialized from the
    /// source and caches it. Later queries read that cached signal. Calling
    /// this inside an effect subscribes the caller to changes in the selected
    /// state of that specific key. `key` is taken by value because it may be
    /// retained in the selector's internal map; `T: Clone` covers that
    /// insertion.
    pub fn is_selected(&self, key: T) -> bool {
        let sig = {
            let mut subs = self.inner.subs.borrow_mut();
            if let Some(&s) = subs.get(&key) {
                s
            } else {
                let selected = RUNTIME
                    .with(|rt| {
                        let cell = rt.borrow().signals.get(self.inner.source.id).cloned();
                        cell.and_then(|c| c.borrow().downcast_ref::<T>().map(|cur| *cur == key))
                    })
                    .unwrap_or(false);
                let s = create_signal(selected);
                subs.insert(key.clone(), s);
                s
            }
        };
        sig.get()
    }

    /// Cancels the selector's tracking effect and disposes all per-key signals.
    ///
    /// Consumes `self`; all clones share the same state, so they should not be
    /// used afterwards. Calling `dispose` again on a clone is harmless. This
    /// does not dispose the source signal, which remains owned by its creator.
    pub fn dispose(self) {
        dispose_effect(self.inner._effect);
        for (_, sig) in self.inner.subs.borrow_mut().drain() {
            sig.dispose();
        }
    }
}

/// Creates a [`Selector`] that tracks which value of `source` is selected.
///
/// The selector installs an internal effect that runs immediately, so it is
/// subscribed to `source` on return. Keys are discovered lazily through
/// [`Selector::is_selected`]. `T` must be `Eq + Hash + Clone + 'static` so
/// keys can be compared with the source value and stored in the selector's
/// map. Call [`Selector::dispose`] to release the effect and cached signals.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::{create_selector, create_signal, flush_effects};
///
/// let selected = create_signal(1u32);
/// let selector = create_selector(selected);
/// assert!(selector.is_selected(1));
/// assert!(!selector.is_selected(2));
///
/// selected.set(2);
/// assert_eq!(flush_effects(), 1); // only the selector's effect re-runs
/// assert!(!selector.is_selected(1));
/// assert!(selector.is_selected(2));
///
/// selector.dispose();
/// selected.dispose();
/// ```
pub fn create_selector<T: Eq + std::hash::Hash + Clone + 'static>(
    source: Signal<T>,
) -> Selector<T> {
    let subs: Rc<RefCell<HashMap<T, Signal<bool>>>> = Rc::new(RefCell::new(HashMap::new()));
    let subs_c = subs.clone();
    let eff = create_effect(move || {
        let cur = source.get();
        let subs = subs_c.borrow();
        for (k, &sig) in subs.iter() {
            let is_sel = *k == cur;
            let changed = RUNTIME
                .with(|rt| {
                    let cell = rt.borrow().signals.get(sig.id).cloned();
                    cell.and_then(|c| {
                        c.borrow()
                            .downcast_ref::<bool>()
                            .copied()
                            .map(|v| v != is_sel)
                    })
                })
                .unwrap_or(false);
            if changed {
                sig.set(is_sel);
            }
        }
    });
    Selector {
        inner: Rc::new(SelectorInner {
            source,
            subs,
            _effect: eff,
        }),
    }
}

/// A cached derived value backed by a [`Signal`].
///
/// Created by [`create_memo`]. Whenever a dependency changes, the memo's
/// internal effect recomputes the value and writes its backing signal only if
/// the result differs from the cached value under `PartialEq`, avoiding
/// unnecessary invalidation of downstream subscribers. `Memo<T>` is `Copy`:
/// copies read the same derived slot.
///
/// The memo's storage is not owned: call [`Memo::dispose`] to remove both the
/// internal effect and the backing signal, or keep the memo alive for as long
/// as its consumers need it.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::{create_memo, create_signal, flush_effects, Memo};
///
/// let count = create_signal(2);
/// let doubled: Memo<i32> = create_memo(move || count.get() * 2);
/// assert_eq!(doubled.get(), 4);
///
/// count.set(5);
/// assert_eq!(doubled.get(), 4); // cached until effects flush
/// flush_effects();
/// assert_eq!(doubled.get(), 10);
/// count.dispose();
/// doubled.dispose();
/// ```
pub struct Memo<T: 'static> {
    signal: Signal<T>,
    _effect: EffectId,
}

impl<T: 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Memo<T> {}

impl<T: 'static> Memo<T> {
    /// Returns the [`Signal`] holding the memoized value.
    ///
    /// Reading it inside an effect subscribes to the memo, just as with any
    /// other signal. The returned handle refers to the same slot as the memo
    /// and becomes invalid when the memo is disposed.
    pub fn signal(self) -> Signal<T> {
        self.signal
    }

    /// Returns a clone of the cached value.
    ///
    /// The value reflects the last flush, not unflushed dependency writes.
    /// Like [`Signal::get`], this registers a dependency when called inside an
    /// effect and panics if the memo has been disposed.
    pub fn get(self) -> T
    where
        T: Clone,
    {
        self.signal.get()
    }

    /// Returns a clone of the cached value, or `None` if the memo was disposed.
    ///
    /// This is the non-panicking counterpart of [`Memo::get`].
    pub fn try_get(self) -> Option<T>
    where
        T: Clone,
    {
        self.signal.try_get()
    }

    /// Borrows the cached value and passes `&T` to `f` without cloning it.
    ///
    /// Registers a dependency when called inside an effect; panics if the memo
    /// has been disposed. Do not write to the memo's signal from inside `f`.
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        self.signal.with(f)
    }

    /// Disposes the memo's internal effect and backing signal.
    ///
    /// After this call [`Memo::try_get`] returns `None`, and copies of the
    /// memo are invalid as well. The signals that the memo reads are not
    /// disposed; their owner remains responsible for them. Calling `dispose`
    /// on a copy is idempotent.
    pub fn dispose(self) {
        dispose_effect(self._effect);
        self.signal.dispose();
    }
}

/// Creates a memoized value that updates when the signals it reads change.
///
/// `f` is called once immediately to seed the memo, then again inside an
/// effect on creation (that second result is discarded) and on each later
/// flush after one of its dependencies changes. The new value is published
/// only when it differs from the cached value under `PartialEq`, so consumers
/// are spared redundant notifications. Signal reads inside `f` are tracked
/// as dependencies and re-tracked on every recomputation.
///
/// `T` must be `Clone + PartialEq + 'static`, and `f` must be
/// `FnMut + 'static` so it can be stored in the runtime. Call
/// [`Memo::dispose`] when the memo is no longer needed.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::{create_memo, create_signal, flush_effects};
///
/// let width = create_signal(3);
/// let height = create_signal(4);
/// let area = create_memo(move || width.get() * height.get());
/// assert_eq!(area.get(), 12);
///
/// width.set(5);
/// flush_effects();
/// assert_eq!(area.get(), 20);
///
/// width.dispose();
/// height.dispose();
/// area.dispose();
/// ```
pub fn create_memo<T: PartialEq + Clone + 'static>(mut f: impl FnMut() -> T + 'static) -> Memo<T> {
    let initial = f();
    let sig = create_signal(initial);
    let mut first = true;
    let eff = create_effect(move || {
        let next = f();
        if first {
            first = false;
            return;
        }
        let changed = sig.try_with(|cur| *cur != next).unwrap_or(false);
        if changed {
            sig.set(next);
        }
    });
    Memo {
        signal: sig,
        _effect: eff,
    }
}

