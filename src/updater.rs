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

use server;
use error::Error;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use comm::Context;
use config::Config;
use std::collections::HashMap;
use zmq;
use chan;
use msg;

pub struct Updater {
    server: Arc<server::Description>,
    config: Arc<Config>,
    channel: chan::Sender<msg::Internal>,
    env: HashMap<String, String>,
}

impl Updater {
    pub fn new(server: Arc<server::Description>, config: Arc<Config>, chan: chan::Sender<msg::Internal>, env: HashMap<String, String>) -> Self {
        Updater {
            server: server,
            config: config,
            channel: chan,
            env: env,
        }
    }

    pub fn start(mut self) -> Result<(), Error> {
        if self.server.update_commands.is_empty() {
            self.channel.send(msg::Internal::UpdateError(self.server.id.clone(), "No update scripts defined".to_string()));
            return Ok(())
        }
            self.channel.send(msg::Internal::UpdateStarted(self.server.id.clone()));

        for (i, path) in self.server.update_commands.iter().enumerate() {
            let mut cmd = Command::new(path.clone());
            let mut cmd = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            let mut cmd = cmd.current_dir(&self.server.work_dir);

            for (k, v) in self.env.iter() {
                let mut cmd = cmd.env(k, v);
            }

            let status = match cmd.status() {
                Ok(out) => {
                    if out.success() {
                        Ok(())
                    } else {
                        match out.code() {
                            Some(x) => Err(format!("Update command #{} failed with exit code {}", i + 1, out)),
                            None => Err(format!("Update command #{} failed", i + 1)),
                        }
                    }
                },
                Err(e) => {
                    Err(format!("Failed to execute update command #{}: '{}'", i + 1, e))
                }
            };

            match status {
                Ok(()) => {},
                Err(error_str) => {
                    self.channel.send(msg::Internal::UpdateError(self.server.id.clone(), error_str));
                    return Ok(())
                }
            }
        }

        self.channel.send(msg::Internal::UpdateComplete(self.server.id.clone()));

        Ok(())
    }
}
