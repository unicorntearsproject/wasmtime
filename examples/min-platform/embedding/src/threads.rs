//! A small test of shared memories and `memory.atomic.wait`/`notify` without
//! `std`, which rely on the `wasmtime_now_ns` and `wasmtime_thread_*` platform
//! symbols.
//!
//! This uses the host API on `SharedMemory` rather than a WebAssembly module,
//! because modules that use shared memories can't be loaded when virtual memory
//! is unavailable (Pulley doesn't support them). Without virtual memory the
//! memory is allocated from the heap, and this checks that it works as well.

use alloc::format;
use core::time::Duration;
use wasmtime::{Engine, MemoryType, Result, SharedMemory, WaitResult, ensure};

unsafe extern "C" {
    fn wasmtime_now_ns() -> u64;
}

/// Runs the test.
///
/// Like `run`, this returns the length of an error message written to
/// `error_buf`, or 0 on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn run_threads(error_buf: *mut u8, error_size: usize) -> usize {
    unsafe {
        let buf = core::slice::from_raw_parts_mut(error_buf, error_size);
        match run_result() {
            Ok(()) => 0,
            Err(e) => {
                let msg = format!("{e:?}");
                let len = buf.len().min(msg.len());
                buf[..len].copy_from_slice(&msg.as_bytes()[..len]);
                len
            }
        }
    }
}

fn run_result() -> Result<()> {
    let mut config = super::config();
    config.shared_memory(true);
    let engine = Engine::new(&config)?;
    let memory = SharedMemory::new(&engine, MemoryType::shared(1, 2))?;

    // The memory holds zeros, so waiting for another value returns right away.
    ensure!(matches!(
        memory.atomic_wait32(0, 1, None)?,
        WaitResult::Mismatch
    ));

    // Waiting for the current value blocks until the timeout passes.
    let timeout = Duration::from_millis(20);
    let start = unsafe { wasmtime_now_ns() };
    ensure!(matches!(
        memory.atomic_wait32(0, 0, Some(timeout))?,
        WaitResult::TimedOut
    ));
    let elapsed = unsafe { wasmtime_now_ns() } - start;
    ensure!(
        elapsed >= timeout.as_nanos() as u64,
        "wait returned after {elapsed}ns, before its timeout"
    );

    // There are no waiters to wake.
    ensure!(memory.atomic_notify(0, 1)? == 0);

    // A shared memory can't move when it grows, because other threads may be
    // using it at the same time.
    let base = memory.data().as_ptr();
    ensure!(memory.grow(1)? == 1);
    ensure!(memory.data_size() == 2 * 65536);
    ensure!(memory.data().as_ptr() == base);
    Ok(())
}
