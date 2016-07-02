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

use std::collections::VecDeque;
use error::Error;

pub struct Message {
    parts: VecDeque<Vec<u8>>,
}

impl Message {
    pub fn new() -> Self {
        Message {
            parts: VecDeque::<Vec<u8>>::new(),
        }
    }

    pub fn push(&mut self, part: Vec<u8>) {
        self.parts.push_back(part)
    }

    pub fn check_len(&self, len: usize) -> Result<(), Error> {
        if len != self.parts.len() {
            Err(Error::MalformedMessage)
        } else {
            Ok(())
        }
    }

    pub fn pop_bytes(&mut self) -> Result<Vec<u8>, Error> {
        match self.parts.pop_front() {
            Some(v) => Ok(v),
            None => Err(Error::MalformedMessage),
        }
    }

    pub fn pop_string(&mut self) -> Result<String, Error> {
        let bytes = try!(self.pop_bytes());

        match String::from_utf8(bytes) {
            Ok(s) => Ok(s),
            Err(e) => Err(Error::UnicodeDecodeError(e)),
        }
    }

    pub fn to_vec(self) -> VecDeque<Vec<u8>> {
        self.parts
    }
}

macro_rules! message {
    ( $( $x:expr ),* ) => {
        {
            let mut msg = Message::new();
            $(
                msg.push($x.to_message_part());
            )*
            msg
        }
    };
}

pub trait ToMessagePart {
    fn to_message_part(&self) -> Vec<u8>;
}

impl ToMessagePart for Vec<u8> {
    fn to_message_part(&self) -> Vec<u8> {
        self.clone()
    }
}

impl<'a> ToMessagePart for &'a [u8] {
    fn to_message_part(&self) -> Vec<u8> {
        Vec::<u8>::from(*self)
    }
}

impl<'a> ToMessagePart for &'a str {
    fn to_message_part(&self) -> Vec<u8> {
        self.as_bytes().to_message_part()
    }
}

impl ToMessagePart for String {
    fn to_message_part(&self) -> Vec<u8> {
        (&self[..]).to_message_part()
    }
}
