//! Expansion boards for Horizon (`docs/boards.md`): a native library the game
//! loads once the player turns it on, offering functions to sandboxed mods.
//! A board runs unsandboxed, with the player's rights.
//!
//! ```ignore
//! fn echo(input: &[u8]) -> Result<Vec<u8>, String> {
//!     Ok(input.to_vec())
//! }
//! horizon_board_sdk::export_board!("echo" => echo);
//! ```
#![allow(unsafe_code)]

use std::sync::atomic::{AtomicPtr, Ordering};

/// The binary interface between the game and a board. Raised only when a
/// change would break a board already built; functions are added at the end of
/// [`abi::Host`], and `size` tells a board what its game has.
pub const ABI: u32 = 1;

/// What crosses between the game and a board: plain C types only, because a
/// board and the game may be built by different compilers.
pub mod abi {
    /// Bytes lent for the length of a call.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Bytes {
        pub ptr: *const u8,
        pub len: usize,
    }
    impl Bytes {
        pub fn of(bytes: &[u8]) -> Self {
            Self {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            }
        }
        /// # Safety
        /// `ptr` and `len` must describe live bytes, or `len` be 0.
        pub unsafe fn get<'a>(self) -> &'a [u8] {
            if self.len == 0 {
                &[]
            } else {
                // SAFETY: the caller's promise above.
                unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
            }
        }
    }
    /// Bytes handed over: whoever made them frees them.
    #[repr(C)]
    pub struct Owned {
        pub ptr: *mut u8,
        pub len: usize,
    }
    impl Owned {
        pub const EMPTY: Self = Self {
            ptr: std::ptr::null_mut(),
            len: 0,
        };
        pub fn new(bytes: Vec<u8>) -> Self {
            let boxed = bytes.into_boxed_slice();
            let len = boxed.len();
            Self {
                ptr: Box::into_raw(boxed).cast(),
                len,
            }
        }
        /// # Safety
        /// `self` must come from [`Owned::new`] in this same library.
        pub unsafe fn free(self) {
            if !self.ptr.is_null() {
                let slice = std::ptr::slice_from_raw_parts_mut(self.ptr, self.len);
                // SAFETY: the caller's promise: it was a box of this length.
                drop(unsafe { Box::from_raw(slice) });
            }
        }
    }

    /// `0`: done. Anything else: `out`, when there is one, holds the reason.
    pub const OK: i32 = 0;
    pub const FAILED: i32 = 1;

    /// What the game offers a board. Functions other than `log` work only
    /// during a call from a mod: they act for that mod's call.
    #[repr(C)]
    pub struct Host {
        pub abi: u32,
        /// The size of this table, for functions added later.
        pub size: usize,
        /// 0 error, 1 warning, 2 info.
        pub log: extern "C" fn(level: u8, line: Bytes),
        /// A command as the server's console, at the tick's end.
        pub run_command: extern "C" fn(line: Bytes, out: *mut Owned) -> i32,
        /// A block's state: `FAILED` with an empty `out` when it isn't loaded.
        pub get_block: extern "C" fn(dim: Bytes, x: i32, y: i32, z: i32, out: *mut Owned) -> i32,
        /// Sets a block at the tick's end.
        pub set_block:
            extern "C" fn(dim: Bytes, x: i32, y: i32, z: i32, state: Bytes, out: *mut Owned) -> i32,
        /// Frees what the three above put in `out`.
        pub free: extern "C" fn(bytes: Owned),
    }
}

static HOST: AtomicPtr<abi::Host> = AtomicPtr::new(std::ptr::null_mut());

/// Calls [`export_board!`] makes. Not for boards to use.
#[doc(hidden)]
pub mod __private {
    use super::*;
    pub fn set_host(host: *const abi::Host) {
        HOST.store(host.cast_mut(), Ordering::Release);
    }
    /// Runs `f`, keeping a panic from crossing into the game.
    pub fn answer(
        f: impl FnOnce() -> Result<Vec<u8>, String> + std::panic::UnwindSafe,
        out: *mut abi::Owned,
    ) -> i32 {
        let (code, bytes) = match std::panic::catch_unwind(f) {
            Ok(Ok(bytes)) => (abi::OK, bytes),
            Ok(Err(reason)) => (abi::FAILED, reason.into_bytes()),
            Err(_) => (abi::FAILED, b"the board panicked".to_vec()),
        };
        // SAFETY: the game passes a place to write, and frees it with this library.
        unsafe { out.write(abi::Owned::new(bytes)) };
        code
    }
}

/// Calling back into the game.
pub mod host {
    use super::{HOST, Ordering, abi};

    fn table() -> Option<&'static abi::Host> {
        // SAFETY: the game's table lives as long as the program.
        unsafe { HOST.load(Ordering::Acquire).as_ref() }
    }
    /// Takes what the game answered, as text or as the reason it failed.
    fn take(h: &abi::Host, code: i32, out: abi::Owned) -> Result<String, String> {
        // SAFETY: the game wrote `out`, or left it empty.
        let text = String::from_utf8_lossy(unsafe {
            abi::Bytes {
                ptr: out.ptr,
                len: out.len,
            }
            .get()
        })
        .into_owned();
        (h.free)(out);
        if code == abi::OK { Ok(text) } else { Err(text) }
    }
    /// A line in the game's log, under this board.
    pub fn log(line: &str) {
        if let Some(h) = table() {
            (h.log)(2, abi::Bytes::of(line.as_bytes()));
        }
    }
    /// A warning in the game's log.
    pub fn warn(line: &str) {
        if let Some(h) = table() {
            (h.log)(1, abi::Bytes::of(line.as_bytes()));
        }
    }
    /// A command as the server's console, at the tick's end. Works during a
    /// mod's call only.
    pub fn run_command(line: &str) -> Result<(), String> {
        let h = table().ok_or("no game")?;
        let mut out = abi::Owned::EMPTY;
        let code = (h.run_command)(abi::Bytes::of(line.as_bytes()), &raw mut out);
        take(h, code, out).map(drop)
    }
    /// A block's state, `minecraft:oak_log[axis=y]`; None when it isn't
    /// loaded. Works during a mod's call only.
    pub fn block(dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let h = table()?;
        let mut out = abi::Owned::EMPTY;
        let code = (h.get_block)(abi::Bytes::of(dimension.as_bytes()), x, y, z, &raw mut out);
        take(h, code, out).ok()
    }
    /// Sets a block at the tick's end. Works during a mod's call only.
    pub fn set_block(dimension: &str, x: i32, y: i32, z: i32, state: &str) -> Result<(), String> {
        let h = table().ok_or("no game")?;
        let mut out = abi::Owned::EMPTY;
        let code = (h.set_block)(
            abi::Bytes::of(dimension.as_bytes()),
            x,
            y,
            z,
            abi::Bytes::of(state.as_bytes()),
            &raw mut out,
        );
        take(h, code, out).map(drop)
    }
}

/// Makes this crate a board: `"name" => function` for each function in its
/// `horizon-board.toml`. A function takes bytes and gives bytes or a reason:
/// `fn(&[u8]) -> Result<Vec<u8>, String>`. The crate must be a `cdylib`.
#[macro_export]
macro_rules! export_board {
    ($($name:literal => $function:path),* $(,)?) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn horizon_board_abi() -> u32 {
            $crate::ABI
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn horizon_board_init(host: *const $crate::abi::Host) {
            $crate::__private::set_host(host);
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn horizon_board_call(
            name: $crate::abi::Bytes,
            input: $crate::abi::Bytes,
            out: *mut $crate::abi::Owned,
        ) -> i32 {
            $crate::__private::answer(
                move || {
                    // SAFETY: the game lends both for this call.
                    let (name, input) = unsafe { (name.get(), input.get()) };
                    $(if name == $name.as_bytes() {
                        let run: fn(&[u8]) -> Result<Vec<u8>, String> = $function;
                        return run(input);
                    })*
                    Err(format!("no function {}", String::from_utf8_lossy(name)))
                },
                out,
            )
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn horizon_board_free(bytes: $crate::abi::Owned) {
            // SAFETY: `horizon_board_call` made it, in this library.
            unsafe { bytes.free() }
        }
    };
}
