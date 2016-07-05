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

use chan;
use server;
use std::collections::HashMap;

pub enum Internal {
    StartServer(String),
    KillServer(String),

    ServerStarted(String, chan::Sender<server::WatcherMessage>),
    ServerStopped(String),

    RunUpdate(String, HashMap<String, String>),
    UpdateStarted(String),
    UpdateError(String, String),
    UpdateComplete(String),
}

pub enum Byond {
    ServerStarted(String),
    ServerStopping(String),

    Ping,
    Pong(String),

    RunUpdate(String, HashMap<String, String>),
    UpdateStarted,
    UpdateError(String),
    UpdateComplete,
}

pub enum External {
}
