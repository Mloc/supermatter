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

// Struct to handle the transition between internal channels and external JSON 0MQ sockets
// this was meant to be the nicest, most elegant part of this project
// but it's not, mostly because you can't poll 0MQ sockets and channels at the same time

use zmq;
use std::sync::Arc;
use std::thread;
use chan;
use serde_json;
use serde::{Serialize, Deserialize};
use snowflake::ProcessUniqueId;

use comm::{Context, Socket};
use error::Error;


pub struct Liason<S, D> {
    // just to keep the Arc alive for the lifetime of the sockets
    _zmq_context: Arc<Context>,

    sock_external: Socket,
    sock_internal_send: Socket,
    sock_internal_recv: Socket,

    chan_send: chan::Sender<(D, Vec<u8>)>,
    chan_recv: chan::Receiver<(S, Vec<u8>)>,
}

use std::marker;
impl<S: Serialize + marker::Send + 'static, D: Deserialize + marker::Send + 'static> Liason<S, D> {
    pub fn new(chan_send: chan::Sender<(D, Vec<u8>)>, chan_recv: chan::Receiver<(S, Vec<u8>)>, context: Arc<Context>, bind_endpoint: String) -> Result<Self, Error> {
        let mut sock_external = try!(context.socket(zmq::ROUTER));
        try!(sock_external.bind(&bind_endpoint));

        let id = ProcessUniqueId::new();
        let mut sock_internal_send = try!(context.socket(zmq::PAIR));
        let mut sock_internal_recv = try!(context.socket(zmq::PAIR));

        try!(sock_internal_recv.bind(&format!("inproc://liason{}", id)));
        try!(sock_internal_send.connect(&format!("inproc://liason{}", id)));

        Ok(Liason {
            _zmq_context: context,

            sock_external: sock_external,
            sock_internal_send: sock_internal_send,
            sock_internal_recv: sock_internal_recv,

            chan_send: chan_send,
            chan_recv: chan_recv,
        })
    }

    pub fn run(self) {
        let chan_send = self.chan_send;
        let chan_recv = self.chan_recv;

        let sock_external = self.sock_external;
        let sock_internal_recv = self.sock_internal_recv;
        let sock_internal_send = self.sock_internal_send;

        thread::spawn(move || {
            Self::zmq_proc(sock_internal_recv, sock_external, chan_send);
        });

        thread::spawn(move || {
            Self::chan_proc(chan_recv, sock_internal_send);
        });
    }

    fn zmq_proc(mut sock_internal: Socket, mut sock_external: Socket, chan_send: chan::Sender<(D, Vec<u8>)>) {
        loop {
            // worst line of this whole project
            // can't put the pollset into a var because of the borrow checker
            match zmq::poll(&mut [sock_internal.as_poll_item(zmq::POLLIN), sock_external.as_poll_item(zmq::POLLIN)], -1) {
                Ok(_) => {
                    match Self::zmq_proc_recv(&mut sock_external, &chan_send) {
                        Ok(()) => {},
                        Err(Error::SocketError(zmq::Error::EAGAIN)) => {},
                        Err(e) => panic!("{}", e),
                    }
                    match Self::zmq_proc_send(&mut sock_internal, &mut sock_external) {
                        Ok(()) => {},
                        Err(Error::SocketError(zmq::Error::EAGAIN)) => {},
                        Err(e) => panic!("{}", e),
                    }
                },
                Err(e) => panic!("{}", e),
            }
        }
    }

    fn zmq_proc_recv(sock_external: &mut Socket, chan_send: &chan::Sender<(D, Vec<u8>)>) -> Result<(), Error> {
        let id = try!(sock_external.recv(zmq::DONTWAIT));

        match try!(sock_external.recv_more(0)) {
            Some(v) => {
                if &v[..] != b"" {
                    return Ok(())
                }
            },
            _ => return Ok(())
        }

        let data = match try!(sock_external.recv_more_string(0)) {
            Some(s) => s,
            _ => return Ok(())
        };

        let msg: D = match serde_json::from_str(&data) {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };

        chan_send.send((msg, id));

        Ok(())
    }

    fn zmq_proc_send(sock_internal: &mut Socket, sock_external: &mut Socket) -> Result<(), Error> {
        let id = try!(sock_internal.recv(zmq::DONTWAIT));

        match try!(sock_internal.recv_more(0)) {
            Some(v) => {
                if &v[..] != b"" {
                    return Ok(())
                }
            },
            _ => return Ok(())
        }

        let data = match try!(sock_internal.recv_more_string(0)) {
            Some(s) => s,
            _ => return Ok(())
        };

        try!(sock_external.send(&id, zmq::SNDMORE));
        try!(sock_external.send(b"", zmq::SNDMORE));
        try!(sock_external.send_str(&data, 0));

        Ok(())
    }

    fn chan_proc(chan: chan::Receiver<(S, Vec<u8>)>, mut sock_internal: Socket) {
        loop {
            let (msg, id) = match chan.recv() {
                Some((msg, id)) => (msg, id),
                None => return,
            };

            let s = serde_json::to_string(&msg).unwrap();

            sock_internal.send(&id, zmq::SNDMORE).unwrap();
            sock_internal.send(b"", zmq::SNDMORE).unwrap();
            sock_internal.send_str(&s, 0).unwrap();
        }
    }
}
