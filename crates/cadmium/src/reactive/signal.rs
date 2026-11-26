/// A `Copy` handle to a reactive value stored in the thread-local runtime.
///
/// A `Signal<T>` does not own its value: it holds a [`SignalId`] for a slot
/// containing a type-erased `Rc<RefCell<Box<dyn Any>>>`. Copying or cloning a
/// handle yields another reference to the **same** slot. Use [`OwnedSignal`]
/// when the slot's lifetime should be tied to a Rust value's scope.
///
/// Reads made while a [`create_effect`], [`create_memo`], or [`create_selector`]
/// computation is running register a dependency. Writes do not run effects
/// immediately; they mark subscribers dirty until [`flush_effects`] runs,
/// which [`crate::window::App`] does before rendering each frame.
///
/// `T` must be `'static` because the runtime stores values with type erasure.
/// A handle is only valid on the thread where it was created: calls on another
/// thread address that thread's separate runtime.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::create_signal;
///
/// let signal = create_signal(String::from("ready"));
/// let same_slot = signal; // Signal<T> is Copy
/// same_slot.set(String::from("running"));
/// assert_eq!(signal.get(), "running"); // both handles see the same value
///
/// signal.dispose();
/// assert!(!signal.is_alive());
/// ```
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
    /// Returns `true` while the signal's slot still exists in this thread's
    /// runtime.
    ///
    /// A signal reports `false` after [`Signal::dispose`], after its
    /// [`OwnedSignal`] owner is dropped, and when queried from a thread other
    /// than the one that created it. Prefer [`Signal::try_get`] when disposal
    /// is expected.
    pub fn is_alive(self) -> bool {
        RUNTIME.with(|rt| rt.borrow().signals.contains_key(self.id))
    }

    /// Returns a clone of the current value, or `None` if the signal has been
    /// disposed.
    ///
    /// Like other reads, this registers a dependency when called inside an
    /// effect. It is the non-panicking counterpart of [`Signal::get`].
    ///
    /// # Panics
    ///
    /// Internally the stored value is downcast to `T`; a mismatch would panic.
    /// The public API cannot construct such a mismatch because only
    /// [`create_signal`] creates slots and it fixes `T` on both sides.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::create_signal;
    ///
    /// let signal = create_signal(7);
    /// assert_eq!(signal.try_get(), Some(7));
    /// signal.dispose();
    /// assert_eq!(signal.try_get(), None);
    /// ```
    pub fn try_get(self) -> Option<T>
    where
        T: Clone,
    {
        self.try_with(Clone::clone)
    }

    /// Returns a clone of the current value.
    ///
    /// Reading inside an effect registers a dependency on this signal. Use
    /// [`Signal::try_get`] if the signal may already have been disposed.
    ///
    /// # Panics
    ///
    /// Panics with `"Signal accessed after disposal"` if the slot no longer
    /// exists, and panics on an internal type mismatch (which the public API
    /// cannot produce).
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::create_signal;
    ///
    /// let name = create_signal("Ada".to_string());
    /// assert_eq!(name.get(), "Ada");
    /// name.set("Grace".to_string());
    /// assert_eq!(name.get(), "Grace");
    /// name.dispose();
    /// ```
    pub fn get(self) -> T
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    /// Borrows the current value, passes `&T` to `f`, and returns
    /// `Some(result)`, or `None` if the signal has been disposed.
    ///
    /// This avoids cloning `T` and works for non-`Clone` types. The read is
    /// dependency-tracked when it occurs inside an effect. The borrow lasts
    /// for the duration of `f`: reading other signals is safe, but writing
    /// this same signal inside `f` would panic on the `RefCell` borrow.
    ///
    /// # Panics
    ///
    /// Panics on an internal type mismatch, which the public API cannot
    /// produce.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::create_signal;
    ///
    /// let name = create_signal(String::from("Ada"));
    /// assert_eq!(name.try_with(|s| s.len()), Some(3));
    /// name.dispose();
    /// assert_eq!(name.try_with(|s| s.len()), None);
    /// ```
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

    /// Borrows the current value, passes `&T` to `f`, and returns its result.
    ///
    /// This is the panicking counterpart of [`Signal::try_with`]; it avoids
    /// cloning `T` and registers a dependency when called inside an effect.
    /// Do not write to this same signal from within `f`, as the value is
    /// already borrowed.
    ///
    /// # Panics
    ///
    /// Panics with `"Signal accessed after disposal"` if the slot no longer
    /// exists, and panics on an internal type mismatch (which the public API
    /// cannot produce).
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::create_signal;
    ///
    /// let values = create_signal(vec![1, 2, 3]);
    /// let sum: i32 = values.with(|v| v.iter().sum());
    /// assert_eq!(sum, 6);
    /// values.dispose();
    /// ```
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        self.try_with(f).expect("Signal accessed after disposal")
    }

    /// Replaces the value, dropping the previous one, and notifies subscribers.
    ///
    /// Dependent effects are only marked dirty; they re-run on the next
    /// [`flush_effects`], which [`crate::window::App`] performs automatically
    /// as part of its frame loop. Calling `set` on a disposed signal is a
    /// no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::create_signal;
    ///
    /// let enabled = create_signal(false);
    /// enabled.set(true);
    /// assert!(enabled.get());
    /// enabled.dispose();
    /// ```
    pub fn set(self, value: T) {
        self.update(|slot| *slot = value);
    }

    /// Mutates the value in place with `f`, then notifies subscribers.
    ///
    /// The closure receives `&mut T`, so a new value need not be constructed
    /// by the caller. Reading other signals inside `f` is supported; writing
    /// this same signal from within `f` would panic because it is already
    /// mutably borrowed. Calling `update` on a disposed signal is a no-op and
    /// does not invoke `f`. Effects run later, during [`flush_effects`].
    ///
    /// # Panics
    ///
    /// Panics on an internal type mismatch (which the public API cannot
    /// produce) or if `f` tries to borrow this signal again. A panic from `f`
    /// propagates to the caller.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::create_signal;
    ///
    /// let count = create_signal(1);
    /// let step = create_signal(10);
    /// count.update(|v| *v += step.get()); // reading another signal is safe
    /// assert_eq!(count.get(), 11);
    /// count.dispose();
    /// step.dispose();
    /// ```
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

    /// Disposes the signal slot and removes its subscriber list.
    ///
    /// After this call [`Signal::try_get`] and [`Signal::try_with`] return
    /// `None`, while [`Signal::get`] and [`Signal::with`] panic. Copies of the
    /// handle become inert. Disposing is idempotent and does not cancel
    /// effects that were already queued by an earlier write; those effects
    /// will see the signal as disposed on their next run.
    ///
    /// # Examples
    ///
    /// ```
    /// use cadmium::reactive::{create_signal, flush_effects};
    ///
    /// let source = create_signal(1);
    /// let copy = source;
    /// source.dispose();
    /// source.dispose(); // safe to repeat
    /// assert!(!copy.is_alive());
    /// assert_eq!(copy.try_get(), None);
    /// assert_eq!(flush_effects(), 0);
    /// ```
    pub fn dispose(self) {
        RUNTIME.with(|rt| {
            let mut rt = rt.borrow_mut();
            rt.signals.remove(self.id);
            rt.subscribers.remove(&self.id);
        });
    }
}

/// RAII owner for a [`Signal`] that automatically disposes its slot on drop.
///
/// The owner wraps a signal created by [`create_signal`]. When it is dropped,
/// the slot and its subscriptions are removed even if copies of the
/// [`Signal`] handle are still alive; those copies then report
/// [`Signal::is_alive`] as `false` and [`Signal::try_get`] returns `None`.
/// Cleanup is best-effort during thread-local runtime teardown: if the
/// runtime is already gone, dropping the owner does nothing.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::OwnedSignal;
///
/// let escaped;
/// {
///     let owned = OwnedSignal::new(42);
///     escaped = owned.signal();
///     assert_eq!(escaped.try_get(), Some(42));
///     owned.update(|n| *n += 1);
///     assert_eq!(owned.get(), 43);
/// }
/// assert_eq!(escaped.try_get(), None); // slot was freed on drop
/// ```
pub struct OwnedSignal<T: 'static> {
    inner: Signal<T>,
}

impl<T: 'static> OwnedSignal<T> {
    /// Creates an owned signal initialized to `initial`.
    ///
    /// This is equivalent to calling [`create_signal`] and storing the
    /// returned handle, with disposal delegated to `Drop`.
    pub fn new(initial: T) -> Self {
        Self {
            inner: create_signal(initial),
        }
    }

    /// Returns a copy of the underlying [`Signal`] handle.
    ///
    /// The handle shares the same slot and is only valid until this owner is
    /// dropped (or the signal is disposed explicitly through a copy).
    pub fn signal(&self) -> Signal<T> {
        self.inner
    }

    /// Returns a clone of the current value.
    ///
    /// Behaves like [`Signal::get`], including its panic on a disposed slot;
    /// the owner normally keeps the slot alive.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.inner.get()
    }

    /// Replaces the value and notifies subscribers, like [`Signal::set`].
    ///
    /// If the slot was disposed through a copied handle, this is a no-op.
    pub fn set(&self, value: T) {
        self.inner.set(value);
    }

    /// Mutates the value in place and notifies subscribers, like
    /// [`Signal::update`].
    ///
    /// If the slot was disposed through a copied handle, `f` is not called.
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

/// Creates a signal containing `initial` in the current thread's runtime.
///
/// The returned handle is `Copy` but not an owner: call [`Signal::dispose`]
/// when finished, or use [`OwnedSignal::new`] to tie cleanup to a scope. All
/// copies of the handle read and write the same slot. `T` must be `'static`
/// because values are stored with type erasure.
///
/// # Examples
///
/// ```
/// use cadmium::reactive::create_signal;
///
/// let signal = create_signal(10);
/// let alias = signal;
/// alias.set(20);
/// assert_eq!(signal.get(), 20);
/// signal.dispose();
/// ```
pub fn create_signal<T: 'static>(initial: T) -> Signal<T> {
    let cell: SignalCell = Rc::new(RefCell::new(Box::new(initial)));
    let id = RUNTIME.with(|rt| rt.borrow_mut().signals.insert(cell));
    Signal {
        id,
        _marker: PhantomData,
    }
}

