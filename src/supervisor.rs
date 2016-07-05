/*
  Copyright 2016 Colm Hickey <colmohici@gmail.com>

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
*/

use std::collections::HashMap;
use std::sync::Arc;
use std::time;
use std::thread;
use rustc_serialize::json;

use zmq;
use chan;

use server;
use comm;
use config;
use updater;
use comm::{Context, Socket};
use error::Error;
use msg;

use server::Description;

#[derive(Debug)]
struct State {
    server: ServerState,
    update: UpdateState,
}

#[derive(Debug)]
enum ServerState {
    Stopped,
    PreStart,
    Starting(time::Instant),
    Stopping(time::Instant),
    Serving(usize, Vec<u8>),
    UpdatePending,
}

#[derive(Debug)]
enum UpdateState {
    Idle,
    PreUpdate,
    Updating,
}

pub struct Listener {
    internal_recv: chan::Receiver<msg::Internal>,
    byond_recv: chan::Receiver<(msg::Byond, Vec<u8>)>,
    external_recv: chan::Receiver<(msg::External, Vec<u8>)>,
}

impl Listener {
    pub fn start(mut self, suvi: &mut Supervisor) {
        let ping = chan::tick_ms((suvi.config.ping_interval.as_secs() * 1000) as u32 + (suvi.config.ping_interval.subsec_nanos() / 1000000));

        let ref internal = self.internal_recv;
        let ref byond = self.byond_recv;
        let ref external = self.external_recv;

        loop {
            chan_select! {
                ping.recv() => {
                    suvi.ping_check()
                },
                internal.recv() -> msg => {
                    match msg {
                        Some(msg) => suvi.handle_internal_message(msg),
                        None => continue,
                    }
                },
                byond.recv() -> msg => {
                    match msg {
                        Some((msg, id)) => suvi.handle_byond_message(msg, id),
                        None => continue,
                    }
                },
    //            external.recv() -> (msg, id) => {
      //              self.handle_external_message(msg, id)
        //        },
            }
        }
    }
}

pub struct Supervisor {
    servers: HashMap<String, State>,
    kill_handler: HashMap<String, chan::Sender<server::WatcherMessage>>,

    internal_send: chan::Sender<msg::Internal>,
    byond_send: chan::Sender<(msg::Byond, Vec<u8>)>,
    external_send: chan::Sender<(msg::External, Vec<u8>)>,

    config: Arc<config::Config>,
    context: Arc<Context>,
}

impl Supervisor {
    pub fn new(config: Arc<config::Config>, ctx: Arc<Context>) -> Result<(Self, Listener), Error> {
        let (internal_send, internal_recv) = chan::async::<msg::Internal>();
        let (byond_send, byond_recv) = chan::async::<(msg::Byond, Vec<u8>)>();
        let (external_send, external_recv) = chan::async::<(msg::External, Vec<u8>)>();

        let mut server_states = HashMap::<String, State>::new();
        for id in config.servers.keys() {
            server_states.insert(id.clone(), State {server: ServerState::Stopped, update: UpdateState::Idle});
        }

        Ok((Supervisor {
            servers: server_states,
            kill_handler: HashMap::<String, chan::Sender<server::WatcherMessage>>::new(),

            internal_send: internal_send,
            byond_send: byond_send,
            external_send: external_send,

            config: config,
            context: ctx,
        },
        Listener {
            internal_recv: internal_recv,
            byond_recv: byond_recv,
            external_recv: external_recv,
        }))
    }

    fn ping_check(&mut self) -> Result<(), Error> {
        for (id, mut state) in self.servers.iter_mut() {
            match state.server {
                ServerState::Stopped | ServerState::PreStart | ServerState::UpdatePending => {},
                ServerState::Starting(ref startup_time) => {
                    if *startup_time + self.config.starting_timeout < time::Instant::now() {
                        self.internal_send.send(msg::Internal::KillServer(id.clone()));
                    }
                },
                ServerState::Stopping(ref shutdown_time) => {
                    if *shutdown_time + self.config.stopping_timeout < time::Instant::now() {
                        self.internal_send.send(msg::Internal::KillServer(id.clone()));
                    }
                },
                ServerState::Serving(ref mut ping_checks, ref peer_id) => {
                    if *ping_checks >= self.config.max_lost_pings {
                        self.internal_send.send(msg::Internal::KillServer(id.clone()));
                    } else {
                        self.byond_send.send((msg::Byond::Ping, peer_id.clone()));
                        *ping_checks += 1;
                    }
                },
            }
        }
        Ok(())
    }

    fn handle_internal_message(&mut self, msg: msg::Internal) -> Result<(), Error> {
        match msg {
            msg::Internal::StartServer(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let ServerState::Stopped = state.server {
                        if let UpdateState::Updating = state.update {
                            state.server = ServerState::UpdatePending;
                        } else {
                            let desc: Arc<Description> = self.config.servers[&server_id].clone();
                            let cfg = self.config.clone();
                            let chan = self.internal_send.clone();

                            thread::spawn(move || {
                                let serv = server::Server::new(desc, cfg, chan);
                                serv.start();
                            });
                            state.server = ServerState::PreStart;
                        }
                    }
                }
            },
            msg::Internal::KillServer(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    match state.server {
                        ServerState::Starting(_) | ServerState::Stopping(_) | ServerState::Serving(_, _) => {
                            self.kill_handler[&server_id].send(server::WatcherMessage::KillServer);
                        },
                        // potential race condition with PreStart, however very unlikely
                        ServerState::Stopped | ServerState::PreStart | ServerState::UpdatePending => {},
                    }
                }
            },
            msg::Internal::ServerStarted(server_id, kill_handler) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let ServerState::PreStart = state.server  {
                        self.kill_handler.insert(server_id.clone(), kill_handler);
                        state.server = ServerState::Starting(time::Instant::now());
//                        try!(self.internal_sock.send_message(message!(peer_id, "OK"), 0));
                    } else {
//                        try!(self.internal_sock.send_message(message!(peer_id, "ERR"), 0));
                    }
                } else {
//                    try!(self.internal_sock.send_message(message!(peer_id, "ERR"), 0));
                }
            },
            msg::Internal::ServerStopped(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    state.server = ServerState::Stopped;
                }
                self.kill_handler[&server_id].send(server::WatcherMessage::KillWatcher);
            },
            msg::Internal::RunUpdate(server_id, env) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    match state.update {
                        UpdateState::Idle => {
                            let desc: Arc<Description> = self.config.servers[&server_id].clone();
                            let cfg = self.config.clone();
                            let chan = self.internal_send.clone();

                            thread::spawn(move || {
                                let updater = updater::Updater::new(desc, cfg, chan, env);
                                updater.start();
                            });
                            state.update = UpdateState::PreUpdate;
                        },
                        UpdateState::PreUpdate | UpdateState::Updating => {
                            // inform of error, somehow?
                        }
                    }
                }
            },
            msg::Internal::UpdateStarted(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let UpdateState::PreUpdate = state.update {
                        state.update = UpdateState::Updating;
                        if let ServerState::Serving(_, ref peer_id) = state.server {
                            self.byond_send.send((msg::Byond::UpdateStarted, peer_id.clone()));
                        }
                    }
                }
            },
            msg::Internal::UpdateError(server_id, error) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    state.update = UpdateState::Idle;
                    if let ServerState::Serving(_, ref peer_id) = state.server {
                        self.byond_send.send((msg::Byond::UpdateError(error), peer_id.clone()));
                    }
                }
            },
            msg::Internal::UpdateComplete(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let UpdateState::Updating = state.update {
                        state.update = UpdateState::Idle;
                        if let ServerState::UpdatePending = state.server {
                            state.server = ServerState::Stopped;
                            self.internal_send.send(msg::Internal::StartServer(server_id));
                        } else if let ServerState::Serving(_, ref peer_id) = state.server {
                            self.byond_send.send((msg::Byond::UpdateComplete, peer_id.clone()));
                        }
                    }
                }
            },
        };

        Ok(())
    }

    fn handle_byond_message(&mut self, msg: msg::Byond, peer_id: Vec<u8>) -> Result<(), Error> {
        match msg {
            msg::Byond::ServerStarted(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    state.server = ServerState::Serving(0, peer_id);
                }
            },
            msg::Byond::ServerStopping(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {

                }
            },
            msg::Byond::Pong(server_id) => {
                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let ServerState::Serving(ref mut count, _) = state.server {
                        *count = 0;
                    }
                }
            },
            msg::Byond::RunUpdate(server_id, env) => {
                self.internal_send.send(msg::Internal::RunUpdate(server_id, env));
            },
            // BYOND-end exclusive messages
            // maybe we should have two types
            msg::Byond::Ping |
            msg::Byond::UpdateStarted |
            msg::Byond::UpdateError(_) |
            msg::Byond::UpdateComplete => {},
        };

        Ok(())
    }
}
