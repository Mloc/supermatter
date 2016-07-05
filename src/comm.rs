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

use zmq;
use std::sync::Mutex;

use error::Error;
//use msg::Message;

pub struct Context {
    ctx: Mutex<zmq::Context>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            ctx: Mutex::new(zmq::Context::new()),
        }
    }
    pub fn socket(&self, sock_type: zmq::SocketType) -> Result<Socket, Error> {
        let mut ctx = self.ctx.lock().unwrap();
        match ctx.socket(sock_type) {
            Ok(s) => Ok(Socket::new(s)),
            Err(e) => Err(Error::SocketError(e)),
        }
    }
}

pub struct Socket {
    zmq_sock: zmq::Socket,
}

impl Socket {
    pub fn new(sock: zmq::Socket) -> Self {
        Socket {
            zmq_sock: sock
        }
    }

    pub fn connect(&mut self, endpoint: &str) -> Result<(), Error> {
        match self.zmq_sock.connect(endpoint) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn bind(&mut self, endpoint: &str) -> Result<(), Error> {
        match self.zmq_sock.bind(endpoint) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn recv(&mut self, flags: i32) -> Result<Vec<u8>, Error> {
        match self.zmq_sock.recv_bytes(flags) {
            Ok(r) => Ok(r),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn recv_string(&mut self, flags: i32) -> Result<String, Error> {
        match self.recv(flags) {
            Ok(b) => match String::from_utf8(b) {
                Ok(s) => Ok(s),
                Err(e) => Err(Error::UnicodeDecodeError(e)),
            },
            Err(e) => Err(e),
        }
    }

    pub fn send(&mut self, msg: &[u8], flags: i32) -> Result<(), Error> {
        match self.zmq_sock.send(msg, flags) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn send_str(&mut self, msg: &str, flags: i32) -> Result<(), Error> {
        self.send(msg.as_bytes(), flags)
    }

    pub fn subscribe(&mut self, filter: &str) -> Result<(), Error> {
        match self.zmq_sock.set_subscribe(filter.as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn unsubscribe(&mut self, filter: &str) -> Result<(), Error> {
        match self.zmq_sock.set_unsubscribe(filter.as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn recv_more(&mut self, flags: i32) -> Result<Option<Vec<u8>>, Error> {
        match self.zmq_sock.get_rcvmore() {
            Ok(b) => match b {
                true => match self.recv(flags) {
                    Ok(s) => Ok(Some(s)),
                    Err(e) => Err(e),
                },
                false => Ok(None),
            },
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn recv_more_string(&mut self, flags: i32) -> Result<Option<String>, Error> {
        match self.zmq_sock.get_rcvmore() {
            Ok(b) => match b {
                true => match self.recv_string(flags) {
                    Ok(s) => Ok(Some(s)),
                    Err(e) => Err(e),
                },
                false => Ok(None),
            },
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn set_identity(&mut self, id: &str) -> Result<(), Error> {
        match self.zmq_sock.set_identity(id.as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn get_identity(&mut self) -> Result<Vec<u8>, Error> {
        match self.zmq_sock.get_identity() {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

    pub fn as_poll_item<'a>(&'a self, events: i16) -> zmq::PollItem<'a> {
        self.zmq_sock.as_poll_item(events)
    }

    pub fn get_rcvmore(&self) -> Result<bool, Error> {
        match self.zmq_sock.get_rcvmore() {
            Ok(b) => Ok(b),
            Err(e) => Err(Error::SocketError(e)),
        }
    }

/*    pub fn recv_message(&mut self, flags: i32) -> Result<Message, Error> {
        let mut msg = Message::new();
        msg.push(try!(self.recv(flags)));

        while try!(self.get_rcvmore()) {
            msg.push(try!(self.recv(flags)));
        }

        Ok(msg)
    }

    pub fn send_message(&mut self, msg: Message, flags: i32) -> Result<(), Error> {
        let mut vec = msg.to_vec();
        let last = match vec.pop_back() {
            Some(t) => t,
            None => return Err(Error::MalformedMessage),
        };

        for v in vec.iter() {
            try!(self.send(v, zmq::SNDMORE | flags));
        }
        try!(self.send(&last[..], flags));
        Ok(())
    }*/
}
