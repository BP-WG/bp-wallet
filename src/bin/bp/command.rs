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

use std::fs;

use bp_rt::{AddrInfo, UtxoInfo};
use bpw::BoostrapError;
use strict_encoding::Ident;

use crate::args::Args;
use crate::Config;

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
pub enum Command {
    /// List known wallets.
    List,

    /// Get or set default wallet.
    #[display("default")]
    Default {
        /// Name of the wallet to make it default.
        default: Option<Ident>,
    },

    /// Create a wallet.
    Create {
        /// The name for the new wallet.
        name: Ident,
    },

    /// List addresses for the wallet descriptor.
    Addresses {
        /// Number of addresses to generate.
        #[clap(short, default_value = "20")]
        count: u16,
    },

    /// List available coins (unspent transaction outputs).
    Coins,
}

impl Args {
    pub fn exec(self, mut config: Config) -> Result<(), BoostrapError> {
        println!();

        match &self.command {
            Command::List => {
                let dir = self.general.base_dir();
                let Ok(dir) = fs::read_dir(dir).map_err(|err| {
                    error!("Error reading wallet directory: {err:?}");
                    eprintln!("System directory is not initialized");
                }) else {
                    return Ok(());
                };
                println!("Known wallets:");
                let mut count = 0usize;
                for wallet in dir {
                    let Ok(wallet) = wallet else {
                        continue;
                    };
                    let Ok(meta) = wallet.metadata() else {
                        continue;
                    };
                    if !meta.is_dir() {
                        continue;
                    }
                    let name = wallet.file_name().into_string().expect("invalid directory name");
                    println!(
                        "{name}{}",
                        if config.default_wallet == name { "\t[default]" } else { "" }
                    );
                    count += 1;
                }
                if count == 0 {
                    println!("no wallets found");
                }
            }
            Command::Default { default } => {
                if let Some(default) = default {
                    config.default_wallet = default.to_string();
                    config.store(&self.conf_path());
                } else {
                    println!("Default wallet is '{}'", config.default_wallet);
                }
            }
            Command::Create { name } => {
                let mut runtime = self.runtime(&config)?;
                let name = name.to_string();
                print!("Saving the wallet as '{name}' ... ");
                let dir = self.general.wallet_dir(&name);
                runtime.set_name(name);
                if let Err(err) = runtime.store(&dir) {
                    println!("error: {err}");
                } else {
                    println!("success");
                }
            }
            Command::Addresses { count } => {
                let runtime = self.runtime(&config)?;
                println!("Addresses (outer):");
                println!();
                println!("Term.\tAddress\t\t\t\t\t\t\t\t# used\tVolume\tBalance");
                for info in runtime.address_all().take(*count as usize) {
                    let AddrInfo {
                        addr,
                        terminal,
                        used,
                        volume,
                        balance,
                    } = info;
                    println!("{terminal}\t{addr}\t{used}\t{volume}\t{balance}");
                }
            }
            Command::Coins => {
                let runtime = self.runtime(&config)?;
                println!("Coins (UTXOs):");
                println!();
                println!("Address\t{:>12}\tOutpoint", "Value");
                for (addr, coins) in runtime.address_coins() {
                    println!("{addr}:");
                    for utxo in coins {
                        let UtxoInfo {
                            outpoint, value, ..
                        } = utxo;
                        println!("\t{:>12} ṩ\t{outpoint}", value.0);
                        //₿
                    }
                    println!();
                }
            }
        };

        println!();

        Ok(())
    }
}
