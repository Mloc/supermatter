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

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use config::Config;
use comm::Context;
use error::Error;
use msg::{Message, ToMessagePart};

use std;

use zmq;
use byond::Runtime;

#[derive(Debug)]
pub struct Description {
    pub runtime: Arc<Runtime>,

    pub work_dir: PathBuf,
    pub dmb: String,
    pub port: u16,

    pub update_commands: Vec<PathBuf>,

    pub id: String,
}

pub struct Server {
    desc: Arc<Description>,
    config: Arc<Config>,
    context: Arc<Context>,
}

impl Server {
    pub fn new(desc: Arc<Description>, config: Arc<Config>, ctx: Arc<Context>) -> Self {
        Server {
            desc: desc,
            config: config,
            context: ctx,
        }
    }

    pub fn start(mut self) -> Result<(), Error> {
        let mut child = Command::new(self.desc.runtime.bin_dir.join("DreamDaemon"))
                                .current_dir(&self.desc.work_dir)
                                .env("BYOND_SYSTEM", &self.desc.runtime.byond_system)
                                .env("LD_LIBRARY_PATH", &self.desc.runtime.bin_dir)
                                .env("LIBC_FATAL_STDERR_", "1")
                                .arg(&self.desc.dmb)
                                .arg(self.desc.port.to_string())
                                .arg("-trusted")
                                .arg("-core")
                                .arg("-logself")
                                .arg("-params").arg(format!("supermatter_endpoint={}&supermatter_id={}", self.config.byond_endpoint, &self.desc.id))
                                .stdin(Stdio::null())
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .spawn().unwrap();

        let pid = child.id();
        let mut kill_sock = try!(self.context.socket(zmq::DEALER));
        try!(kill_sock.set_identity(&format!("{}-killwatcher", self.desc.id)));
        try!(kill_sock.connect(&self.config.internal_endpoint));
        std::thread::spawn(move || {
//            sock.send_str("HELLO", 0).unwrap();
            loop {
                let cmd = match kill_sock.recv_string(0) {
                    Ok(c) => c,
                    Err(Error::SocketError(e)) => panic!(e),
                    Err(_) => continue,
                };
                match &cmd[..] {
                    "KILL-SERVER" => {
                        kill_process(pid);
                        return
                    },
                    "KILL-WATCHER" => return,
                    _ => continue
                };
            }
        });

        let mut sock = try!(self.context.socket(zmq::DEALER));
        try!(sock.connect(&self.config.internal_endpoint));

        try!(sock.send_message(message!("STARTED", self.desc.id, format!("{}-killwatcher", self.desc.id)), 0));
        child.wait().unwrap();
        try!(sock.send_message(message!("STOPPED", self.desc.id), 0));

        Ok(())
    }
}

use libc;
use kernel32;

// yeah these need error handling.
#[cfg(unix)]
fn kill_process(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

// untested, zmq crate doesn't compile on windows
#[cfg(windows)]
fn kill_process(pid: u32) {
    unsafe {
        let handle = kernel32::OpenProcess(kernel32::winapi::winnt::PROCESS_TERMINATE, false, pid as kernel32::winapi::minwindef::DWORD);
        kernel32::TerminateProcess(handle, 0);
    }
}
