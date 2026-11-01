//! `/hmod full-reload` (`docs/boards.md`): a fresh process of the same game on
//! the same world, for what cannot change in a running one, like a board. This
//! game saves and closes the world, starts the new one and waits, hidden. The
//! new one says it is ready over localhost, and only then does this one exit;
//! if it dies or stays silent, it is killed and this one opens the world again.
use super::Game;
use horizon_client::text::{self, Text};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use winit::event_loop::{ActiveEventLoop, ControlFlow};

/// Tells a new game where to say it is ready: `address token`.
const PARENT_ENV: &str = "HORIZON_RELOAD_PARENT";
/// Long enough for an old computer to load a world. An autosave comes later
/// than this, so a game killed here has not written the world.
const TIMEOUT: Duration = Duration::from_secs(180);
/// Options that say what to open, or that were for this start only.
const NOT_REPEATED: [&str; 5] = [
    "--world",
    "--quickPlaySingleplayer",
    "--server",
    "--quickPlayMultiplayer",
    "--session-file",
];

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Asked(String),
    Closing(String),
    Waiting {
        folder: String,
        child: Child,
        listener: TcpListener,
        token: String,
        session: PathBuf,
        until: Instant,
    },
    /// Back in the old process, opening the world again: why, for the chat.
    RolledBack(String),
}

pub(super) struct FullReload {
    phase: Phase,
    /// The world's folder under `saves/`: with a server thread only the
    /// server knows its path.
    pub(super) world: String,
    /// In a game a full reload started: who waits to hear it is ready.
    parent: Option<String>,
}

impl Default for FullReload {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            world: String::new(),
            parent: std::env::var(PARENT_ENV).ok(),
        }
    }
}

/// This run's command line without what `NOT_REPEATED` names, opening `folder`.
fn child_args(args: impl Iterator<Item = String>, folder: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut args = args;
    while let Some(arg) = args.next() {
        if NOT_REPEATED.contains(&arg.as_str()) {
            args.next();
        } else {
            out.push(arg);
        }
    }
    out.extend(["--world".to_owned(), folder.to_owned()]);
    out
}

impl Game<'_> {
    /// `/hmod full-reload`: what the chat answers. The work starts next turn
    /// of the event loop, out of the chat's handling.
    pub(super) fn start_full_reload(&mut self) -> Vec<String> {
        if !matches!(self.full_reload.phase, Phase::Idle) {
            return vec!["A full reload is already running".to_owned()];
        }
        if self.remote.is_some() && self.integrated.is_none() {
            return vec!["A full reload is for a world of this computer".to_owned()];
        }
        if self.player.is_none() || self.full_reload.world.is_empty() {
            return vec!["A full reload needs a world open".to_owned()];
        }
        self.full_reload.phase = Phase::Asked(self.full_reload.world.clone());
        vec!["Saving and restarting the game; other players will have to join again".to_owned()]
    }

    /// One turn of the event loop. True while it waits for the new game and
    /// the event loop has nothing else to do.
    pub(super) fn poll_full_reload(&mut self, event_loop: &ActiveEventLoop) -> bool {
        self.tell_reload_parent();
        match std::mem::take(&mut self.full_reload.phase) {
            Phase::Idle => false,
            Phase::Asked(folder) => {
                self.full_reload.phase = Phase::Closing(folder);
                if let Err(error) = self.close_world() {
                    self.error = Some(error);
                    event_loop.exit();
                }
                true
            }
            Phase::Closing(folder) => {
                if self.title.is_none() {
                    self.full_reload.phase = Phase::Closing(folder);
                    return false;
                }
                match self.spawn_reloaded(&folder) {
                    Ok(waiting) => {
                        self.full_reload.phase = waiting;
                        true
                    }
                    Err(why) => {
                        self.roll_back(folder, why, None, event_loop);
                        false
                    }
                }
            }
            Phase::Waiting {
                folder,
                mut child,
                listener,
                token,
                session,
                until,
            } => {
                if heard_ready(&listener, &token) {
                    // Nothing of this game is left to keep, and a normal exit
                    // would write its settings over the new game's.
                    std::process::exit(0);
                }
                let why = match child.try_wait() {
                    Ok(Some(status)) => Some(format!("the new game stopped: {status}")),
                    Err(error) => Some(format!("the new game can't be watched: {error}")),
                    Ok(None) if Instant::now() >= until => {
                        Some("the new game was not ready in time".to_owned())
                    }
                    Ok(None) => None,
                };
                if let Some(why) = why {
                    self.roll_back(folder, why, Some((child, session)), event_loop);
                    return false;
                }
                self.full_reload.phase = Phase::Waiting {
                    folder,
                    child,
                    listener,
                    token,
                    session,
                    until,
                };
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(50),
                ));
                true
            }
            Phase::RolledBack(why) => {
                if self.player.is_none() {
                    self.full_reload.phase = Phase::RolledBack(why);
                } else {
                    self.system_message(
                        Text::literal(format!("Full reload failed, back in the old game: {why}"))
                            .with_color(text::RED),
                    );
                }
                false
            }
        }
    }

    /// Starts the new game, once this one has let go of the world.
    fn spawn_reloaded(&mut self, folder: &str) -> Result<Phase, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .and_then(|l| l.set_nonblocking(true).map(|()| l))
            .map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let token = horizon_client::control::random_token();
        let mut command = Command::new(exe);
        command
            .args(child_args(std::env::args().skip(1), folder))
            .env(PARENT_ENV, format!("{address} {token}"))
            .env_remove("HORIZON_SESSION_FILE");
        // The launcher's session file was deleted when it was read.
        let session = std::env::temp_dir().join(format!("horizon-reload-{}", std::process::id()));
        if let Some(account) = horizon_client::auth::current() {
            account.write_file(&session).map_err(|e| e.to_string())?;
            command.env("HORIZON_SESSION_FILE", &session);
        }
        // The new game listens on the same control socket.
        self.control = None;
        let child = command.spawn().map_err(|e| {
            let _ = std::fs::remove_file(&session);
            e.to_string()
        })?;
        if let Some(window) = &self.window {
            window.set_visible(false);
        }
        Ok(Phase::Waiting {
            folder: folder.to_owned(),
            child,
            listener,
            token,
            session,
            until: Instant::now() + TIMEOUT,
        })
    }

    /// The new game is out of the running: gone for good, reaped so no zombie
    /// stays, and this game opens the world it closed.
    fn roll_back(
        &mut self,
        folder: String,
        why: String,
        child: Option<(Child, PathBuf)>,
        event_loop: &ActiveEventLoop,
    ) {
        log::warn!("Horizon: full reload failed: {why}");
        if let Some((mut child, session)) = child {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(session);
        }
        if let Some(window) = &self.window {
            window.set_visible(true);
        }
        self.full_reload.phase = Phase::RolledBack(why);
        let opened = self
            .start_agent_control()
            .and_then(|()| self.open_world(&folder, None, None, false, None));
        if let Err(error) = opened {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    /// In the new game: when the world is in place (what `horizon-ctl
    /// wait-ready` waits for), the old game may go.
    fn tell_reload_parent(&mut self) {
        if self.full_reload.parent.is_none()
            || self.player.is_none()
            || self.streaming.pending() + self.pending_uploads.len() + self.remote_backlog() != 0
            || self
                .remote_status()
                .is_some_and(|server| server["loaded"] == false)
        {
            return;
        }
        let Some(parent) = self.full_reload.parent.take() else {
            return;
        };
        let Some((address, token)) = parent.split_once(' ') else {
            return;
        };
        let told = address.parse().and_then(|address: SocketAddr| {
            Ok(TcpStream::connect_timeout(&address, Duration::from_secs(1))
                .and_then(|mut stream| stream.write_all(token.as_bytes())))
        });
        if !matches!(told, Ok(Ok(()))) {
            // It has gone already; this game goes on regardless.
            log::warn!("Horizon: the old game didn't hear the full reload was ready");
        }
    }
}

/// A connection presenting the token. One second at most, and only if something
/// else on this computer connected and stayed silent.
fn heard_ready(listener: &TcpListener, token: &str) -> bool {
    let Ok((stream, _)) = listener.accept() else {
        return false;
    };
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut said = String::new();
    let _ = stream.take(128).read_to_string(&mut said);
    said == token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_args_keep_settings_and_open_the_world() {
        let args = [
            "--resources",
            "r",
            "--world",
            "Old",
            "--creative",
            "--server",
            "h:1",
        ];
        assert_eq!(
            child_args(args.iter().map(|a| a.to_string()), "New"),
            ["--resources", "r", "--creative", "--world", "New"]
        );
    }
}
