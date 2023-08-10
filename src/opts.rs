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
use strict_encoding::Ident;

pub const DEFAULT_ESPLORA: &str = "https://blockstream.info/testnet/api";
use crate::{Command, DATA_DIR, DATA_DIR_ENV};

#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct ResolverOpt {
    /// Esplora server to use.
    #[arg(
        short,
        long,
        global = true,
        default_value = DEFAULT_ESPLORA,
        env = "ESPLORA_SERVER",
        value_hint = ValueHint::Url,
        value_name = "URL"
    )]
    pub esplora: String,

    #[clap(long, global = true)]
    pub sync: bool,
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group(multiple = false)]
pub struct WalletOpts {
    #[arg(short = 'w', long = "wallet", global = true)]
    pub name: Option<Ident>,

    /// Path to wallet directory.
    #[arg(
        short = 'W',
        long,
        global = true,
        value_hint = ValueHint::DirPath,
    )]
    pub wallet_path: Option<PathBuf>,

    /// Use tr(KEY) descriptor as wallet.
    #[arg(long, global = true)]
    pub tr_key_only: Option<XpubDescriptor>,
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct Config {
    /// Data directory path.
    ///
    /// Path to the directory that contains RGB stored data.
    #[arg(
        short,
        long,
        global = true,
        default_value = DATA_DIR,
        env = DATA_DIR_ENV,
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// Blockchain to use.
    #[arg(
        short = 'n',
        long,
        global = true,
        alias = "network",
        default_value = "testnet",
        env = "RGB_NETWORK"
    )]
    pub chain: Chain,
}

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

    #[command(flatten)]
    pub wallet: WalletOpts,

    #[command(flatten)]
    pub resolver: ResolverOpt,

    #[command(flatten)]
    pub config: Config,

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
        self.config.data_dir = PathBuf::from(
            shellexpand::tilde(&self.config.data_dir.display().to_string()).to_string(),
        );
    }

    pub fn runtime(&self) -> Result<Runtime<DescriptorStd>, BoostrapError> {
        eprint!("Loading descriptor");
        let mut runtime: Runtime<DescriptorStd> = if let Some(d) = self.wallet.tr_key_only.clone() {
            eprint!(" from command-line argument ...");
            let network = self.config.chain;
            Runtime::new(TrKey::from(d).into(), network)
        } else if let Some(wallet_path) = self.wallet.wallet_path.clone() {
            eprint!(" from specified wallet directory ...");
            Runtime::load(wallet_path)?
        } else {
            eprint!(" from wallet ...");
            let network = self.config.chain;
            let mut data_dir = self.config.data_dir.clone();
            data_dir.push(network.to_string());
            Runtime::load(data_dir)?
        };
        eprintln!(" success");

        if self.resolver.sync || self.wallet.tr_key_only.is_some() {
            eprint!("Syncing ...");
            let indexer = esplora::Builder::new(&self.resolver.esplora).build_blocking()?;
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
