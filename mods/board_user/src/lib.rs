//! A mod calling an expansion board: `/board_demo` tries every function of
//! `mods/example_board`, then sets a gold block above the player from native code.
use horizon_mod_sdk::server::{Args, Player, Registration, ServerMod, boards};

const BOARD: &str = "example_board";

struct BoardUser;

fn demo(player: &Player) -> Result<String, String> {
    let version = boards::version(BOARD).ok_or("no board")?;
    let echo = boards::call(BOARD, "echo", b"hello")?;
    if echo != b"hello" {
        return Err("echo answered wrong".into());
    }
    let numbers: Vec<u8> = [40u32, 2].iter().flat_map(|n| n.to_le_bytes()).collect();
    let sum: [u8; 8] = boards::call(BOARD, "sum", &numbers)?
        .try_into()
        .map_err(|_| "sum isn't 8 bytes")?;
    let sum = u64::from_le_bytes(sum);
    let (x, y, z) = (
        player.x.floor() as i32,
        player.y.floor() as i32 + 3,
        player.z.floor() as i32,
    );
    let at = format!("{} {x} {y} {z}", player.dimension);
    let before =
        String::from_utf8_lossy(&boards::call(BOARD, "block", at.as_bytes())?).into_owned();
    let place = format!("{at} minecraft:gold_block");
    boards::call(BOARD, "place", place.as_bytes())?;
    let panic = boards::call(BOARD, "panics", b"").err().unwrap_or_default();
    let unknown = boards::call(BOARD, "nothing", b"").is_err();
    let other = boards::call("other_board", "echo", b"").is_err();
    Ok(format!(
        "board {version}: echo ok, 40 + 2 = {sum}, above you was {before}; panic said \"{panic}\"; unknown refused: {unknown}, undeclared board refused: {other}"
    ))
}

impl ServerMod for BoardUser {
    fn init() -> Result<(Self, Registration), String> {
        Ok((BoardUser, Registration::new().command("board_demo")))
    }
    fn command(
        &mut self,
        player: &Player,
        _name: &str,
        _args: &Args,
    ) -> Result<Option<String>, String> {
        demo(player).map(Some)
    }
}
horizon_mod_sdk::export_server_mod!(BoardUser);
