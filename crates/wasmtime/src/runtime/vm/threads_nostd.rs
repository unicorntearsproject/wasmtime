//! The few `std` items that shared memories and `memory.atomic.wait`/`notify`
//! need, rebuilt on top of host-provided hooks for `no_std` builds. See
//! `sys/custom/capi.rs` for what the embedder must implement:
//! `wasmtime_now_ns`, `wasmtime_thread_id`, `wasmtime_thread_park`,
//! `wasmtime_thread_unpark`. Locks come from `crate::sync`, which blocks
//! through the `custom-sync-primitives` hooks.

use crate::runtime::vm::sys;
use core::convert::Infallible;
use core::fmt;
use core::ops::{Deref, DerefMut, Sub};
pub use core::time::Duration;

/// A monotonic point in time, in nanoseconds from the host's clock.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Instant(u64);

impl Instant {
    pub fn now() -> Instant {
        Instant(sys::now_ns())
    }

    pub fn checked_add(self, duration: Duration) -> Option<Instant> {
        let nanos = u64::try_from(duration.as_nanos()).ok()?;
        self.0.checked_add(nanos).map(Instant)
    }
}

impl Sub for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(other.0))
    }
}

/// A handle that can wake one thread.
#[derive(Clone, Copy, Debug)]
pub struct Thread(usize);

impl Thread {
    pub fn unpark(&self) {
        sys::thread_unpark(self.0);
    }
}

/// Mirrors the parts of `std::thread` that the parking spot uses.
pub mod thread {
    use super::*;

    pub fn current() -> Thread {
        Thread(sys::thread_id())
    }

    /// Blocks the current thread until unparked or `timeout` passes. Like
    /// `std::thread::park_timeout`, an unpark that arrived earlier makes this
    /// return immediately, and it may return spuriously.
    pub fn park_timeout(timeout: Duration) {
        let deadline = Instant::now()
            .checked_add(timeout)
            .map_or(0, |i| i.0.max(1));
        sys::thread_park(deadline);
    }
}

/// `std::sync::Mutex` without poisoning, on top of `crate::sync::RwLock`
/// (whose write lock is exclusive and blocks via the host hooks).
#[derive(Default)]
pub struct Mutex<T>(crate::sync::RwLock<T>);

impl<T> Mutex<T> {
    pub fn lock(&self) -> Result<impl DerefMut<Target = T> + '_, Infallible> {
        Ok(self.0.write())
    }
}

impl<T> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Mutex")
    }
}

/// `std::sync::RwLock` with the `Result`-returning API the callers expect.
pub struct RwLock<T>(crate::sync::RwLock<T>);

impl<T> RwLock<T> {
    pub const fn new(value: T) -> RwLock<T> {
        RwLock(crate::sync::RwLock::new(value))
    }

    pub fn read(&self) -> Result<impl Deref<Target = T> + '_, Infallible> {
        Ok(self.0.read())
    }

    pub fn write(&self) -> Result<impl DerefMut<Target = T> + '_, Infallible> {
        Ok(self.0.write())
    }
}
