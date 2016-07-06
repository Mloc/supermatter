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

#[derive(Debug)]
pub enum Internal {
    // Server ID
    StartServer(String),
    // Server ID
    KillServer(String),

    // Server ID; control channel for server watcher
    ServerStarted(String, chan::Sender<server::WatcherMessage>),
    // Server ID
    ServerStopped(String),

    // Server ID, String->String map of env vars to set for update
    RunUpdate(String, HashMap<String, String>),
    // Server ID
    UpdateStarted(String),
    // Server ID, error message
    UpdateError(String, String),
    // Server ID
    UpdateComplete(String),
}

#[derive(Debug, Deserialize)]
pub enum ByondIn {
    // Server ID
    ServerStarted(String),
    // Server ID
    ServerStopping(String),

    // Server ID
    Pong(String),

    // Server ID, String->String map of env vars to set for update
    RunUpdate(String, HashMap<String, String>),
}

#[derive(Debug, Serialize)]
pub enum ByondOut {
    Ping,
    UpdateStarted,
    // Error message
    UpdateError(String),
    UpdateComplete,
}

#[derive(Debug, Deserialize)]
pub enum ExternalIn {
}

#[derive(Debug, Serialize)]
pub enum ExternalOut {
}
