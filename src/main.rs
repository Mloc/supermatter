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

extern crate zmq;
extern crate rustc_serialize;

extern crate libc;
extern crate kernel32;

use std::path::PathBuf;
use std::sync::Arc;

#[macro_use]
mod msg;

mod error;
mod comm;
mod byond;
mod server;
mod supervisor;
mod config;
mod updater;

use msg::{Message, ToMessagePart};

fn main() {
    let cfg = Arc::new(config::Config::load(PathBuf::from("supermatter.cfg")).unwrap());
    let ctx = Arc::new(comm::Context::new());

    let mut sock = ctx.socket(zmq::DEALER).unwrap();
    sock.connect(&cfg.internal_endpoint).unwrap();

    let mut suvi = supervisor::Supervisor::new(cfg, ctx.clone()).unwrap();

    std::thread::spawn(move || {
        suvi.start();
    });

    {
        sock.send_message(message!("START-SERVER", "test"), 0).unwrap();
    }

    loop {
        std::thread::sleep(std::time::Duration::new(1000, 0));
    }
}
