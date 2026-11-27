//! Fine-grained reactive state for [`crate::widgets`] and [`crate::window`].
//!
//! Cadmium keeps a small dependency graph in **thread-local** storage. A
//! [`Signal`] is a cheap `Copy` handle to a slot in that graph: reading it
//! inside an effect (with [`Signal::get`], [`Signal::try_get`],
//! [`Signal::with`], or [`Signal::try_with`]) records a dependency, while
//! [`Signal::set`] and [`Signal::update`] mark dependent effects as dirty.
//! Effects created with [`create_effect`] run immediately once, then re-run on
//! the next [`flush_effects`] after one of their dependencies changes. They
//! re-track dependencies on every run, so reads behind a branch that is no
//! longer taken do not keep stale subscriptions. Because the runtime is
//! thread-local, signals and effects must be created, read, written, and
//! flushed on the thread that hosts the UI; [`UiDispatcher`] is the intended
//! bridge from worker threads.
//!
//! [`create_memo`] derives a cached [`Memo`] from other signals. It computes
//! its initial value immediately and, on later flushes, writes its backing
//! signal only when the new value differs under `PartialEq`, so downstream
//! subscribers are not woken for unchanged results. [`create_selector`]
//! builds a [`Selector`] that maintains one lazily created boolean signal per
//! key, which makes "which item is selected?" cheap to observe without
//! invalidating widgets interested in other keys.
//!
//! [`crate::window::App`] flushes effects before rendering each frame and
//! after draining asynchronous tasks, so application code normally does not
//! call [`flush_effects`] itself; it is useful in tests and headless code.
//! Widgets in [`crate::widgets`] subscribe by creating effects that read
//! signals and push dirty flags into the widget arena; those flags drive
//! layout invalidation through [`crate::layout`] and subsequent repainting.
//! Effect ids returned by [`create_effect`] can be retained and cancelled with
//! [`dispose_effect`]; [`Memo::dispose`] and [`Selector::dispose`] tear down
//! their internal effects and signals, and [`OwnedSignal`] ties a signal
//! slot's lifetime to a Rust value's scope.
//!
//! # Examples
//!
//! ```
//! use std::cell::Cell;
//! use std::rc::Rc;
//!
//! use cadmium::reactive::{create_effect, create_memo, create_signal, flush_effects};
//!
//! let count = create_signal(1);
//! let observed = Rc::new(Cell::new(0));
//! let seen = observed.clone();
//! create_effect(move || seen.set(count.get() * 10));
//! assert_eq!(observed.get(), 10); // effects run once when created
//!
//! let doubled = create_memo(move|| count.get() * 2);
//! assert_eq!(doubled.get(), 2); // memos compute their initial value eagerly
//!
//! count.set(4); // marks subscribers dirty; no callback runs yet
//! assert_eq!(observed.get(), 10);
//! flush_effects(); // settles the effect and the memo
//! assert_eq!(observed.get(), 40);
//! assert_eq!(doubled.get(), 8);
//! ```

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use slotmap::{SlotMap, new_key_type};

new_key_type! {
    /// Opaque identifier for a slot in the current thread's signal table.
    ///
    /// Handles of this type are embedded in [`Signal`] and are not normally
    /// constructed by application code. Like other slotmap keys it is `Copy`,
    /// `Eq`, `Hash`, and `Default`; the default value is a null key that
    /// addresses no signal. A key is only meaningful to the thread-local
    /// runtime that issued it.
    pub struct SignalId;
    /// Opaque identifier for a registered effect.
    ///
    /// Returned by [`create_effect`] and accepted by [`dispose_effect`]. The
    /// key is `Copy`, so it can be stored by a widget while other copies stay
    /// in use. Use it on the same thread where the effect was created; a
    /// different thread has a different runtime and will not find the effect.
    pub struct EffectId;
}

include!("runtime.rs");
include!("signal.rs");
include!("effects.rs");
include!("dispatcher.rs");
include!("derived.rs");
include!("tests.rs");
