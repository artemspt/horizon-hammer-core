//! Capability `net`: messages between a client mod and the server's mods,
//! on channels the mods agree on (`client-net`, `server-net`).
use super::bindings::horizon::mods::client_net;
use super::server_api::ServerAction;
use super::server_bindings::horizon::mods::server_net;
use super::{FUEL_EVENT, Mods, Output, Running, ServerPlayer, State};

/// The custom payload channel a mod channel goes on: `horizon_net:<channel>`.
pub const NET_PREFIX: &str = "horizon_net:";
/// A message's bytes at most (`ServerboundCustomPayloadPacket` allows 32 KiB).
pub const NET_BYTES: usize = 32 * 1024;
const NET_PER_CALL: usize = 64;

fn check(channel: &str, data: &[u8]) -> Result<(), String> {
    let ok = (1..=64).contains(&channel.len())
        && channel
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_./-".contains(&b));
    if !ok {
        return Err(format!("channel {channel:?}: [a-z0-9_./-], 1 to 64 characters"));
    }
    if data.len() > NET_BYTES {
        return Err(format!("{} bytes: at most {NET_BYTES}", data.len()));
    }
    Ok(())
}

impl State {
    fn sent(&self) -> usize {
        self.out
            .iter()
            .filter(|o| matches!(o, Output::NetToServer { .. } | Output::Server(ServerAction::Net { .. })))
            .count()
    }
    fn may_send(&self, channel: &str, data: &[u8]) -> Result<(), String> {
        check(channel, data)?;
        if self.sent() >= NET_PER_CALL {
            return Err(format!("at most {NET_PER_CALL} messages per call"));
        }
        Ok(())
    }
}

impl client_net::Host for State {
    fn send(&mut self, channel: String, data: Vec<u8>) -> Result<(), String> {
        self.may_send(&channel, &data)?;
        self.out.push(Output::NetToServer { channel, data });
        Ok(())
    }
}

impl server_net::Host for State {
    fn send(&mut self, player: String, channel: String, data: Vec<u8>) -> Result<(), String> {
        self.may_send(&channel, &data)?;
        if !self.snapshot.players.iter().any(|p| p.id == player) {
            return Err(format!("no player {player} online"));
        }
        self.out.push(Output::Server(ServerAction::Net {
            player: Some(player),
            channel,
            data,
        }));
        Ok(())
    }
    fn broadcast(&mut self, channel: String, data: Vec<u8>) -> Result<(), String> {
        self.may_send(&channel, &data)?;
        self.out.push(Output::Server(ServerAction::Net {
            player: None,
            channel,
            data,
        }));
        Ok(())
    }
}

impl Mods {
    /// A message from the server's mods: every client mod that hears `net`.
    pub fn client_net_received(&mut self, channel: &str, data: &[u8]) {
        for i in 0..self.entries.len() {
            if self.entries[i].run.as_ref().is_some_and(|r| r.net_client.is_some()) {
                self.call(i, FUEL_EVENT, |run| {
                    let Running { net_client, store, .. } = run;
                    match net_client {
                        Some(n) => n.horizon_mods_client_net_events().call_received(store, channel, data),
                        None => Ok(()),
                    }
                });
            }
        }
    }
    /// A message from `player`'s client mods: every server mod that hears `net`.
    pub fn server_net_received(&mut self, player: &ServerPlayer, channel: &str, data: &[u8]) {
        for i in 0..self.entries.len() {
            if self.entries[i].run.as_ref().is_some_and(|r| r.net_server.is_some()) {
                self.call(i, FUEL_EVENT, |run| {
                    let Running { net_server, store, .. } = run;
                    match net_server {
                        Some(n) => n
                            .horizon_mods_server_net_events()
                            .call_received(store, player, channel, data),
                        None => Ok(()),
                    }
                });
            }
        }
    }
}
