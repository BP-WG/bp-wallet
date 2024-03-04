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

use std::fmt::Debug;
use std::path::PathBuf;

use bpwallet::{AnyIndexer, Runtime};
use clap::Subcommand;
use descriptors::Descriptor;
use strict_encoding::Ident;

use crate::opts::{DescrStdOpts, DescriptorOpts, DEFAULT_ELECTRUM};
use crate::{Config, GeneralOpts, ResolverOpt, RuntimeError, WalletOpts};

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Args<C: Clone + Eq + Debug + Subcommand, O: DescriptorOpts = DescrStdOpts> {
    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity.
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(flatten)]
    pub wallet: WalletOpts<O>,

    #[command(flatten)]
    pub resolver: ResolverOpt,

    #[command(flatten)]
    pub general: GeneralOpts,

    /// Command to execute.
    #[clap(subcommand)]
    pub command: C,
}

impl<C: Clone + Eq + Debug + Subcommand, O: DescriptorOpts> Args<C, O> {
    pub fn translate<C1: Clone + Eq + Debug + Subcommand>(&self, cmd: &C1) -> Args<C1, O> {
        Args {
            verbose: self.verbose.clone(),
            wallet: self.wallet.clone(),
            resolver: self.resolver.clone(),
            general: self.general.clone(),
            command: cmd.clone(),
        }
    }
}

pub trait Exec {
    type Error: std::error::Error;
    const CONF_FILE_NAME: &'static str;

    fn exec(self, config: Config, name: &'static str) -> Result<(), Self::Error>;
}

impl<C: Clone + Eq + Debug + Subcommand, O: DescriptorOpts> Args<C, O> {
    pub fn process(&mut self) { self.general.process(); }

    pub fn conf_path(&self, name: &'static str) -> PathBuf {
        let mut conf_path = self.general.base_dir();
        conf_path.push(name);
        conf_path.set_extension("toml");
        conf_path
    }

    pub fn bp_runtime<D: Descriptor>(&self, conf: &Config) -> Result<Runtime<D>, RuntimeError>
    where for<'de> D: From<O::Descr> + serde::Serialize + serde::Deserialize<'de> {
        eprint!("Loading descriptor");
        let mut runtime: Runtime<D> = if let Some(d) = self.wallet.descriptor_opts.descriptor() {
            eprint!(" from command-line argument ... ");
            Runtime::new_standard(d.into(), self.general.network)
        } else if let Some(wallet_path) = self.wallet.wallet_path.clone() {
            eprint!(" from specified wallet directory ... ");
            Runtime::load_standard(wallet_path)?
        } else {
            let wallet_name = self
                .wallet
                .name
                .as_ref()
                .map(Ident::to_string)
                .unwrap_or(conf.default_wallet.clone());
            eprint!(" from wallet {wallet_name} ... ");
            Runtime::load_standard(self.general.wallet_dir(wallet_name))?
        };
        let mut sync = self.resolver.sync;
        if runtime.warnings().is_empty() {
            eprintln!("success");
        } else {
            eprintln!("complete with warnings:");
            for warning in runtime.warnings() {
                eprintln!("- {warning}");
            }
            sync = true;
            runtime.reset_warnings();
        }

        if sync || self.wallet.descriptor_opts.is_some() {
            eprint!("Syncing");
            let indexer = if self.resolver.electrum != DEFAULT_ELECTRUM {
                AnyIndexer::Electrum(Box::new(electrum::Client::new(&self.resolver.electrum)?))
            } else {
                AnyIndexer::Esplora(Box::new(
                    esplora::Builder::new(&self.resolver.esplora).build_blocking()?,
                ))
            };
            if let Err(errors) = runtime.sync(&indexer) {
                eprintln!(" partial, some requests has failed:");
                for err in errors {
                    eprintln!("- {err}");
                }
            } else {
                eprintln!(" success");
            }
            runtime.try_store()?;
        }

        Ok(runtime)
    }
}
