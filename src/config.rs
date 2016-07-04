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

use std::time::Duration;
use rustc_serialize::json;
use std::path::PathBuf;
use std::fs::File;
use std::io;
use std::io::Read;
use std::collections::HashMap;
use std::sync::Arc;

use server;
use byond;

#[derive(Debug)]
pub enum LoadError {
    DecodeError(json::DecoderError),
    IOError(io::Error),
    AssemblyError,
}

#[derive(Debug)]
pub struct Config {
    pub internal_endpoint: String,
    pub byond_endpoint: String,
    pub external_endpoint: String,

    pub ping_interval: Duration,
    pub max_lost_pings: usize,

    pub starting_timeout: Duration,
    pub stopping_timeout: Duration,

    pub servers: HashMap<String, Arc<server::Description>>,
}

impl Config {
    pub fn load(path: PathBuf) -> Result<Self, LoadError> {
        let mut json = String::new();

        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => return Err(LoadError::IOError(e)),
        };

        match file.read_to_string(&mut json) {
            Ok(_) => {},
            Err(e) => return Err(LoadError::IOError(e)),
        };

        let mut config_ser: ConfigSerialize = match json::decode(&json) {
            Ok(c) => c,
            Err(e) => return Err(LoadError::DecodeError(e)),
        };

        let mut runtimes = HashMap::<String, Arc<byond::Runtime>>::new();
        for (id, runtime) in config_ser.runtimes.drain() {
            runtimes.insert(id, Arc::new(byond::Runtime {
                byond_system: PathBuf::from(runtime.byond_system),
                bin_dir: PathBuf::from(runtime.bin_dir),
            }));
        }
        runtimes.insert(String::from("system"), Arc::new(byond::Runtime::system()));

        let mut servers = HashMap::<String, Arc<server::Description>>::new();
        for (id, sdesc) in config_ser.servers.drain() {
            servers.insert(id.clone(), Arc::new(server::Description {
                runtime: match runtimes.get(&sdesc.runtime) {
                    Some(v) => v.clone(),
                    None => return Err(LoadError::AssemblyError),
                },

                work_dir: PathBuf::from(sdesc.work_dir),
                dmb: sdesc.dmb,
                port: sdesc.port,

                update_commands: match sdesc.update_commands {
                    Some(scripts) => scripts.iter().map(PathBuf::from).collect(),
                    None => Vec::<PathBuf>::new(),
                },

                id: id,
            }));
        }

        Ok(Config {
            internal_endpoint: String::from("ipc://internal_endpoint"),
            byond_endpoint: config_ser.byond_endpoint,
            external_endpoint: config_ser.external_endpoint,

            ping_interval: duration_from_f64(config_ser.ping_interval),
            max_lost_pings: config_ser.max_lost_pings,

            starting_timeout: duration_from_f64(config_ser.starting_timeout),
            stopping_timeout: duration_from_f64(config_ser.stopping_timeout),

            servers: servers,
        })
    }
}

#[derive(RustcDecodable, Debug)]
struct ConfigSerialize {
    byond_endpoint: String,
    external_endpoint: String,

    ping_interval: f64,
    max_lost_pings: usize,

    starting_timeout: f64,
    stopping_timeout: f64,

    servers: HashMap<String, DescriptionSerialize>,
    runtimes: HashMap<String, RuntimeSerialize>,
}

#[derive(RustcDecodable, Debug)]
pub struct DescriptionSerialize {
    pub runtime: String,

    pub work_dir: String,
    pub dmb: String,
    pub port: u16,

    pub update_commands: Option<Vec<String>>,
}

#[derive(RustcDecodable, Debug)]
pub struct RuntimeSerialize {
    pub byond_system: String,
    pub bin_dir: String,
}

fn duration_from_f64(float: f64) -> Duration {
    Duration::new(float.trunc() as u64, float.fract() as u32)
}
