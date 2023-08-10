// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2023 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2020-2023 LNP/BP Standards Association. All rights reserved.
// Copyright (C) 2020-2023 Dr Maxim Orlovsky. All rights reserved.
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

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod loglevel;
mod opts;
mod command;

use std::process::ExitCode;

use clap::Parser;

pub use crate::command::Command;
pub use crate::loglevel::LogLevel;
use crate::opts::BoostrapError;
pub use crate::opts::Opts;

pub const DATA_DIR_ENV: &str = "BP_DATA_DIR";
#[cfg(any(target_os = "linux"))]
pub const DATA_DIR: &str = "~/.bp";
#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
pub const DATA_DIR: &str = "~/.bp";
#[cfg(target_os = "macos")]
pub const DATA_DIR: &str = "~/Library/Application Support/BP Wallets";
#[cfg(target_os = "windows")]
pub const DATA_DIR: &str = "~\\AppData\\Local\\BP Wallets";
#[cfg(target_os = "ios")]
pub const DATA_DIR: &str = "~/Documents";
#[cfg(target_os = "android")]
pub const DATA_DIR: &str = ".";

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> Result<(), BoostrapError> {
    let mut opts = Opts::parse();
    opts.process();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    trace!("Command-line arguments: {:#?}", &opts);

    eprintln!("\nBP: command-line wallet for bitcoin protocol");
    eprintln!("    by LNP/BP Standards Association\n");
    let mut runtime = opts.runtime()?;
    debug!("Executing command: {}", opts.command);
    opts.command.exec(&mut runtime);
    Ok(())
}
