//! `/hmod`, the game's own command for mods: `/hmod list` says which mods
//! there are and how they are, `/hmod reload [id]` restarts the mods that are
//! on (or one) from their files, in the game, without leaving the world. The
//! window runs it for its mods and the server for its own; each answers in
//! chat.

/// What `/hmod` asks.
#[derive(Clone, Debug, PartialEq)]
pub enum HmodCommand {
    List,
    /// The expansion boards in `horizon-boards/`.
    Boards,
    /// Every mod that is on, or only the one with this id.
    Reload(Option<String>),
    /// The whole game restarts on this world, for what a running one can't change.
    FullReload,
    /// Anything else: the usage line.
    Help,
}

/// The command typed after the slash, when it is `/hmod ...`.
pub fn parse(line: &str) -> Option<HmodCommand> {
    let mut words = line.split_whitespace();
    if words.next()? != "hmod" {
        return None;
    }
    Some(match (words.next(), words.next(), words.next()) {
        (Some("list"), None, _) => HmodCommand::List,
        (Some("boards"), None, _) => HmodCommand::Boards,
        (Some("full-reload"), None, _) => HmodCommand::FullReload,
        (Some("reload"), id, None) => HmodCommand::Reload(id.map(str::to_owned)),
        _ => HmodCommand::Help,
    })
}

pub const USAGE: &str = "/hmod list | /hmod boards | /hmod reload [mod id] | /hmod full-reload";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hmod_lines() {
        assert_eq!(parse("hmod list"), Some(HmodCommand::List));
        assert_eq!(parse("hmod reload"), Some(HmodCommand::Reload(None)));
        assert_eq!(
            parse("hmod reload test_core"),
            Some(HmodCommand::Reload(Some("test_core".into())))
        );
        assert_eq!(parse("hmod full-reload"), Some(HmodCommand::FullReload));
        assert_eq!(parse("hmod"), Some(HmodCommand::Help));
        assert_eq!(parse("hmod reload a b"), Some(HmodCommand::Help));
        assert_eq!(parse("hmods list"), None);
        assert_eq!(parse("give @s stone"), None);
    }
}
