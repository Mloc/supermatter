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

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Runtime {
    pub byond_system: PathBuf,
    pub bin_dir: PathBuf,
}

impl Runtime {
    pub fn system() -> Self {
        Runtime {
            byond_system: PathBuf::from("/usr/share/byond/"),
            bin_dir: PathBuf::from("/usr/share/byond/bin/"),
        }
    }

    pub fn local<P: AsRef<Path>>(path: P) -> Self {
        Runtime {
            byond_system: path.as_ref().to_owned(),
            bin_dir: path.as_ref().join("bin/"),
        }
    }
}
