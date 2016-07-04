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

use server;
use comm;
use config;
use updater;
use comm::{Context, Socket};
use error::Error;
use msg::{Message, ToMessagePart};

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

pub struct Supervisor {
    servers: HashMap<String, State>,

    kill_handler: HashMap<String, Vec<u8>>,

    context: Arc<Context>,
    internal_sock: Socket,
    internal_loopback: Socket,
    byond_sock: Socket,

    next_ping_check: time::Instant,
    config: Arc<config::Config>,
}

impl Supervisor {
    pub fn new(config: Arc<config::Config>, ctx: Arc<Context>) -> Result<Self, Error> {
        let mut internal_sock = try!(ctx.socket(zmq::ROUTER));
        try!(internal_sock.bind(&config.internal_endpoint));

        let mut internal_loopback = try!(ctx.socket(zmq::DEALER));
        try!(internal_loopback.connect(&config.internal_endpoint));

        let mut byond_sock = try!(ctx.socket(zmq::ROUTER));
        try!(byond_sock.bind(&config.byond_endpoint));

        let mut server_states = HashMap::<String, State>::new();
        for id in config.servers.keys() {
            server_states.insert(id.clone(), State {server: ServerState::Stopped, update: UpdateState::Idle});
        }

        Ok(Supervisor {
            servers: server_states,

            kill_handler: HashMap::<String, Vec<u8>>::new(),

            context: ctx,
            internal_sock: internal_sock,
            internal_loopback: internal_loopback,
            byond_sock: byond_sock,

            next_ping_check: time::Instant::now(),
            config: config,
        })
    }

    pub fn start(mut self) {
        loop {
            match zmq::poll(&mut [self.internal_sock.as_poll_item(zmq::POLLIN), self.byond_sock.as_poll_item(zmq::POLLIN)], self.get_timeout()) {
                Ok(0) => {
                    self.next_ping_check = time::Instant::now() + self.config.ping_interval;
                    self.ping_check().unwrap();
                },
                Ok(_) => {
                    match self.handle_internal_message() {
                        Ok(_) => {},
                        Err(Error::SocketError(zmq::Error::EAGAIN)) => {},
                        Err(e) => panic!("{}", e),
                    };
                    match self.handle_byond_message() {
                        Ok(_) => {},
                        Err(Error::SocketError(zmq::Error::EAGAIN)) => {},
                        Err(e) => panic!("{}", e),
                    };
                },
                Err(e) => panic!("{}", e),
            }
        }
    }

    fn get_timeout(&self) -> i64 {
        let cur_time = time::Instant::now();
        if cur_time >= self.next_ping_check {
            0
        } else {
            let delta = self.next_ping_check - cur_time;
            ((delta.as_secs() * 1000) + (delta.subsec_nanos() / 1000000) as u64) as i64
        }
    }

    fn ping_check(&mut self) -> Result<(), Error> {
        for (id, mut state) in self.servers.iter_mut() {
            match state.server {
                ServerState::Stopped | ServerState::PreStart | ServerState::UpdatePending => {},
                ServerState::Starting(ref startup_time) => {
                    if *startup_time + self.config.starting_timeout < time::Instant::now() {
                        try!(self.internal_loopback.send_message(message!("KILL-SERVER", id), 0));
                    }
                },
                ServerState::Stopping(ref shutdown_time) => {
                    if *shutdown_time + self.config.stopping_timeout < time::Instant::now() {
                        try!(self.internal_loopback.send_message(message!("KILL-SERVER", id), 0));
                    }
                },
                ServerState::Serving(ref mut ping_checks, ref peer_id) => {
                    if *ping_checks >= self.config.max_lost_pings {
                        try!(self.internal_loopback.send_message(message!("KILL-SERVER", id), 0));
                    } else {
                        try!(self.byond_sock.send_message(message!(peer_id, "PING"), 0));
                        *ping_checks += 1;
                    }
                },
            }
        }
        Ok(())
    }

    fn handle_internal_message(&mut self) -> Result<(), Error> {
        let mut msg = try!(self.internal_sock.recv_message(zmq::DONTWAIT));
        let peer_id = try!(msg.pop_bytes());
        let payload = try!(msg.pop_string());

        match &payload[..] {
            "START-SERVER" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let ServerState::Stopped = state.server {
                        if let UpdateState::Updating = state.update {
                            state.server = ServerState::UpdatePending;
                        } else {
                            let desc: Arc<Description> = self.config.servers[&server_id].clone();
                            let cfg = self.config.clone();
                            let ctx = self.context.clone();
                            let id = server_id.clone();
                            thread::spawn(move || {
                                let serv = server::Server::new(desc, cfg, ctx);
                                serv.start();
                            });
                            state.server = ServerState::PreStart;
                        }
                    }
                }
            },
            "KILL-SERVER" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    match state.server {
                        ServerState::Starting(_) | ServerState::Stopping(_) | ServerState::Serving(_, _) => {
                            try!(self.internal_sock.send_message(message!(self.kill_handler[&server_id], "KILL-SERVER"), 0));
                        },
                        // potential race condition with PreStart, however very unlikely
                        ServerState::Stopped | ServerState::PreStart | ServerState::UpdatePending => {},
                    }
                }
            },
            "STARTED" => {
                try!(msg.check_len(2));
                let server_id = try!(msg.pop_string());
                let kill_handler = try!(msg.pop_bytes());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let ServerState::PreStart = state.server  {
                        self.kill_handler.insert(server_id.clone(), kill_handler);
                        state.server  = ServerState::Starting(time::Instant::now());
                        try!(self.internal_sock.send_message(message!(peer_id, "OK"), 0));
                    } else {
                        try!(self.internal_sock.send_message(message!(peer_id, "ERR"), 0));
                    }
                } else {
                    try!(self.internal_sock.send_message(message!(peer_id, "ERR"), 0));
                }
            },
            "STOPPED" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    state.server = ServerState::Stopped;
                }
                try!(self.internal_sock.send_message(message!(self.kill_handler[&server_id], "KILL-WATCHER"), 0));
            },
            "RUN-UPDATE" => {
                try!(msg.check_len(2));
                let server_id = try!(msg.pop_string());
                let env_str = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    match state.update {
                        UpdateState::Idle => {
                            let desc: Arc<Description> = self.config.servers[&server_id].clone();
                            let cfg = self.config.clone();
                            let ctx = self.context.clone();

                            let env: HashMap<String, String> = match json::decode(&env_str) {
                                Ok(e) => e,
                                Err(_) => return Err(Error::MalformedMessage),
                            };
                            thread::spawn(move || {
                                let updater = updater::Updater::new(desc, cfg, ctx, env);
                                updater.start();
                            });
                            state.update = UpdateState::PreUpdate;
                        },
                        UpdateState::PreUpdate | UpdateState::Updating => {
                            try!(self.internal_sock.send_message(message!(peer_id, "ERR", "Update in progress"), 0));
                        }
                    }
                }
            },
            "UPDATE-STARTED" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let UpdateState::PreUpdate = state.update {
                        state.update = UpdateState::Updating;
                        if let ServerState::Serving(_, ref peer_id) = state.server {
                            try!(self.byond_sock.send_message(message!(peer_id, "UPDATE-STARTED"), 0));
                        }
                    }
                }
            },
            "UPDATE-ERR" => {
                try!(msg.check_len(2));
                let server_id = try!(msg.pop_string());
                let error_str = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    state.update = UpdateState::Idle;
                    if let ServerState::Serving(_, ref peer_id) = state.server {
                        try!(self.byond_sock.send_message(message!(peer_id, "UPDATE-ERR", error_str), 0));
                    }
                }
            },
            "UPDATE-COMPLETE" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let UpdateState::Updating = state.update {
                        state.update = UpdateState::Idle;
                        if let ServerState::UpdatePending = state.server {
                            state.server = ServerState::Stopped;
                            try!(self.internal_loopback.send_message(message!("START-SERVER", server_id), 0));
                        } else if let ServerState::Serving(_, ref peer_id) = state.server {
                            try!(self.byond_sock.send_message(message!(peer_id, "UPDATE-COMPLETE"), 0));
                        }
                    }
                }
            }
            _ => {},
        };

        Ok(())
    }

    fn handle_byond_message(&mut self) -> Result<(), Error> {
        let mut msg = try!(self.byond_sock.recv_message(zmq::DONTWAIT));
        let peer_id = try!(msg.pop_bytes());
        let payload = try!(msg.pop_string());

        match &payload[..] {
            "STARTED" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    state.server = ServerState::Serving(0, peer_id);
                }
            },
            "STOPPING" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {

                }
            },
            "PONG" => {
                try!(msg.check_len(1));
                let server_id = try!(msg.pop_string());

                if let Some(state) = self.servers.get_mut(&server_id) {
                    if let ServerState::Serving(ref mut count, _) = state.server {
                        *count = 0;
                    }
                }
            },
            "RUN-UPDATE" => {
                try!(msg.check_len(2));
                let server_id = try!(msg.pop_string());
                let env_str = try!(msg.pop_string());
                self.internal_loopback.send_message(message!("RUN-UPDATE", server_id, env_str), 0);
            },
            _ => {},
        };

        Ok(())
    }
}

// make sure to update SVConfigSerialize in config alongside this
// serde will save us from this
struct Config {
    ping_interval: time::Duration,
    max_lost_pings: usize,

    starting_timeout: time::Duration,
    stopping_timeout: time::Duration,
}

impl Config {
    fn default() -> Self {
        Config {
            ping_interval: time::Duration::new(2, 0),
            max_lost_pings: 5,

            starting_timeout: time::Duration::new(10, 0),
            stopping_timeout: time::Duration::new(10, 0),
        }
    }
}
