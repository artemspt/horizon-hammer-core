//! An example expansion board (`docs/boards.md`). `mods/board_user` calls it.
use horizon_board_sdk::{export_board, host};

fn echo(input: &[u8]) -> Result<Vec<u8>, String> {
    Ok(input.to_vec())
}

/// Little-endian `u32`s in, their sum out: the work a board is for.
fn sum(input: &[u8]) -> Result<Vec<u8>, String> {
    let total = input
        .chunks_exact(4)
        .map(|c| u64::from(u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
        .sum::<u64>();
    Ok(total.to_le_bytes().to_vec())
}

/// Splits `dimension x y z rest` as the two functions below take it.
fn place_args(input: &[u8]) -> Result<(String, i32, i32, i32, String), String> {
    let text = String::from_utf8_lossy(input);
    let mut words = text.split(' ');
    let mut word = || words.next().ok_or("too few words");
    let dimension = word()?.to_owned();
    let number = |w: &str| w.parse::<i32>().map_err(|e| e.to_string());
    let (x, y, z) = (number(word()?)?, number(word()?)?, number(word()?)?);
    Ok((dimension, x, y, z, words.next().unwrap_or("").to_owned()))
}

fn block(input: &[u8]) -> Result<Vec<u8>, String> {
    let (dimension, x, y, z, _) = place_args(input)?;
    host::block(&dimension, x, y, z)
        .map(String::into_bytes)
        .ok_or_else(|| "not loaded".to_owned())
}

fn place(input: &[u8]) -> Result<Vec<u8>, String> {
    let (dimension, x, y, z, state) = place_args(input)?;
    host::log(&format!("placing {state} at {x} {y} {z}"));
    host::set_block(&dimension, x, y, z, &state)?;
    Ok(Vec::new())
}

/// A panic stays in the board: the caller gets an error.
fn panics(_input: &[u8]) -> Result<Vec<u8>, String> {
    panic!("on purpose")
}

export_board!(
    "echo" => echo,
    "sum" => sum,
    "block" => block,
    "place" => place,
    "panics" => panics,
);
