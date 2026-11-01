//! `boards` for server mods, and the table a board calls back through
//! ([`crate::boards`]). A board's call back acts for the mod whose call it is
//! in, found through [`CALLER`]: it reaches what that mod's call would.
use super::server_bindings::horizon::mods::{boards as boards_wit, world as mod_world};
use super::{State, quoted};
use crate::boards;
use horizon_board_sdk::{ABI, abi};
use std::cell::Cell;

thread_local! {
    /// The mod in a board call on this thread.
    static CALLER: Cell<*mut State> = const { Cell::new(std::ptr::null_mut()) };
}

/// Clears [`CALLER`], also if the call unwinds.
struct Calling;
impl Drop for Calling {
    fn drop(&mut self) {
        CALLER.with(|c| c.set(std::ptr::null_mut()));
    }
}

/// Answers in `out`, as the board expects.
fn answer(result: Result<String, String>, out: *mut abi::Owned) -> i32 {
    let (code, text) = match result {
        Ok(text) => (abi::OK, text),
        Err(reason) => (abi::FAILED, reason),
    };
    if !out.is_null() {
        // SAFETY: the board passes a place to write.
        unsafe { out.write(abi::Owned::new(text.into_bytes())) };
    }
    code
}
/// Runs `f` for the mod in the call; Err outside one.
fn in_call(f: impl FnOnce(&mut State) -> Result<String, String>) -> Result<String, String> {
    let state = CALLER.with(Cell::get);
    if state.is_null() {
        return Err("only during a mod's call".into());
    }
    // SAFETY: `boards::Host::call` set it from its `&mut self` and doesn't touch
    // `self` until the board returns; this is that board's own call, on its thread.
    f(unsafe { &mut *state })
}
fn text(bytes: abi::Bytes) -> String {
    // SAFETY: the board lends live bytes for this call.
    String::from_utf8_lossy(unsafe { bytes.get() }).into_owned()
}

extern "C" fn host_log(level: u8, line: abi::Bytes) {
    let line = text(line);
    match level {
        0 => log::error!("[board] {line}"),
        1 => log::warn!("[board] {line}"),
        _ => log::info!("[board] {line}"),
    }
}
extern "C" fn host_run_command(line: abi::Bytes, out: *mut abi::Owned) -> i32 {
    let line = text(line);
    answer(in_call(|s| s.console(line).map(|()| String::new())), out)
}
extern "C" fn host_get_block(dim: abi::Bytes, x: i32, y: i32, z: i32, out: *mut abi::Owned) -> i32 {
    let dim = text(dim);
    answer(
        in_call(|s| mod_world::Host::get_block(s, dim, x, y, z).ok_or_else(String::new)),
        out,
    )
}
extern "C" fn host_set_block(
    dim: abi::Bytes,
    x: i32,
    y: i32,
    z: i32,
    state: abi::Bytes,
    out: *mut abi::Owned,
) -> i32 {
    let (dim, state) = (text(dim), text(state));
    answer(
        in_call(|s| mod_world::Host::set_block(s, dim, x, y, z, state).map(|()| String::new())),
        out,
    )
}
extern "C" fn host_free(bytes: abi::Owned) {
    // SAFETY: `answer` made it, here.
    unsafe { bytes.free() }
}

/// What every board is given. `boards::start` takes it from here.
pub static HOST: abi::Host = abi::Host {
    abi: ABI,
    size: std::mem::size_of::<abi::Host>(),
    log: host_log,
    run_command: host_run_command,
    get_block: host_get_block,
    set_block: host_set_block,
    free: host_free,
};

impl State {
    fn declared(&self, board: &str) -> bool {
        self.package
            .1
            .boards
            .iter()
            .any(|n| n.split('@').next() == Some(board))
    }
}

impl boards_wit::Host for State {
    fn call(&mut self, board: String, function: String, input: Vec<u8>) -> Result<Vec<u8>, String> {
        if !self.declared(&board) {
            return Err(format!(
                "board {board} isn't in this mod's manifest: boards = [{}]",
                quoted(&board)
            ));
        }
        let this: *mut State = self;
        CALLER.with(|c| c.set(this));
        let _clear = Calling;
        boards::call(&board, &function, &input)
    }
    fn available(&mut self, board: String) -> bool {
        self.declared(&board) && boards::version(&board).is_some()
    }
    fn version(&mut self, board: String) -> Option<String> {
        self.declared(&board)
            .then(|| boards::version(&board))
            .flatten()
    }
}
