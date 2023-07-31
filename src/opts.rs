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

use std::path::PathBuf;

use bp::{Chain, DescriptorStd, TrKey, XpubDescriptor};
use bp_rt::{LoadError, Runtime};
use clap::ValueHint;

use crate::{Command, BP_DATA_DIR, DEFAULT_ESPLORA};

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Opts {
    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity.
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Data directory path.
    ///
    /// Path to the directory that contains RGB stored data.
    #[clap(
        short,
        long,
        global = true,
        default_value = BP_DATA_DIR,
        env = "BP_DATA_DIR",
        value_hint = ValueHint::DirPath,
        conflicts_with_all = ["wallet_path", "tr_key_only"],
    )]
    pub data_dir: Option<PathBuf>,

    /// Blockchain to use.
    #[clap(
        short = 'n',
        long,
        global = true,
        alias = "network",
        default_value = "testnet",
        env = "BP_NETWORK",
        conflicts_with = "wallet_path"
    )]
    pub chain: Option<Chain>,

    /// Path to wallet directory.
    #[clap(
        short,
        long,
        global = true,
        value_hint = ValueHint::DirPath,
        conflicts_with = "tr_key_only",
    )]
    pub wallet_path: Option<PathBuf>,

    /// Use tr(KEY) descriptor as wallet.
    #[clap(long, global = true)]
    pub tr_key_only: Option<XpubDescriptor>,

    /// Esplora server to use.
    #[clap(
        short,
        long,
        global = true,
        default_value = DEFAULT_ESPLORA,
        env = "BP_ELECTRUM_SERVER"
    )]
    pub esplora: String,

    #[clap(long, global = true)]
    pub sync: bool,

    /// Command to execute.
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum BoostrapError {
    #[from]
    Load(LoadError),
    #[from]
    Explora(esplora::Error),
}

impl Opts {
    pub fn process(&mut self) {
        self.data_dir.as_mut().map(|data_dir| {
            *data_dir =
                PathBuf::from(shellexpand::tilde(&data_dir.display().to_string()).to_string())
        });
    }

    pub fn runtime(&self) -> Result<Runtime, BoostrapError> {
        eprint!("Loading descriptor");
        let mut runtime: Runtime<DescriptorStd, ()> = if let Some(d) = self.tr_key_only.clone() {
            eprint!(" from command-line argument ...");
            let network = self.chain.expect("chain must be present in data director is given");
            Ok(Runtime::new(TrKey::from(d).into(), network))
        } else if let Some(wallet_path) = self.wallet_path.clone() {
            eprint!(" from specified wallet directory ...");
            Runtime::load(wallet_path)
        } else if let Some(mut data_dir) = self.data_dir.clone() {
            eprint!(" from wallet ...");
            let network = self.chain.expect("chain must be present in data director is given");
            data_dir.push(network.to_string());
            Runtime::load(data_dir)
        } else {
            unreachable!()
        }?;
        eprintln!(" success");

        if self.sync || self.tr_key_only.is_some() {
            eprint!("Syncing ...");
            let indexer = esplora::Builder::new(&self.esplora).build_blocking()?;
            if let Err(errors) = runtime.sync(&indexer) {
                eprintln!(" partial, some requests has failed:");
                for err in errors {
                    eprintln!("- {err}");
                }
            } else {
                eprintln!(" success");
            }
        }

        Ok(runtime)
    }
}
