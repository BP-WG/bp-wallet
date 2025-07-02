// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2024 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2024 Dr Maxim Orlovsky. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;

use log::LevelFilter;

/// Represents desired logging verbosity level
#[derive(Copy, Clone, PartialEq, Eq, Debug, Display)]
pub enum LogLevel {
    #[display("none")]
    None = 0,

    /// Report only errors to `stderr` and normal program output to stdin
    /// (if it is not directed to a file). Corresponds to zero verbosity
    /// flags.
    #[display("error")]
    Error,

    /// Report warning messages and errors, plus standard program output.
    /// Corresponds to a single `-v` verbosity flag.
    #[display("warn")]
    Warn,

    /// Report genetic information messages, warnings and errors.
    /// Corresponds to a double `-vv` verbosity flag.
    #[display("info")]
    Info,

    /// Report debugging information and all non-trace messages, including
    /// general information, warnings and errors.
    /// Corresponds to triple `-vvv` verbosity flag.
    #[display("debug")]
    Debug,

    /// Print all possible messages including tracing information.
    /// Corresponds to quadruple `-vvvv` verbosity flag.
    #[display("trace")]
    Trace,
}

impl From<u8> for LogLevel {
    fn from(val: u8) -> Self { Self::from_verbosity_flag_count(val) }
}

impl From<LogLevel> for u8 {
    fn from(log_level: LogLevel) -> Self { log_level.verbosity_flag_count() }
}

impl LogLevel {
    /// Indicates number of required verbosity flags
    pub fn verbosity_flag_count(&self) -> u8 { *self as u8 }

    /// Constructs enum value from a given number of verbosity flags
    pub fn from_verbosity_flag_count(level: u8) -> Self {
        match level {
            0 => LogLevel::None,
            1 => LogLevel::Error,
            2 => LogLevel::Warn,
            3 => LogLevel::Info,
            4 => LogLevel::Debug,
            _ => LogLevel::Trace,
        }
    }

    /// Applies log level to the system
    pub fn apply(&self) {
        log::set_max_level(LevelFilter::Trace);
        if env::var("RUST_LOG").is_err() {
            env::set_var("RUST_LOG", self.to_string());
        }
        env_logger::init();
    }
}
