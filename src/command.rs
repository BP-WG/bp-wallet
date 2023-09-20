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

use bp_rt::{BlockHeight, OpType, WalletAddr};
use strict_encoding::Ident;

use crate::opts::DescriptorOpts;
use crate::{Args, Config, Exec, RuntimeError};

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
        /// Use change keychain
        #[clap(short, long)]
        change: bool,

        /// Number of addresses to generate.
        #[clap(short = 'N', long = "no", default_value = "20")]
        count: u16,
    },

    /// List available coins (unspent transaction outputs).
    Coins,

    /// Display history of wallet operations.
    History {
        /// Print full transaction ids
        #[clap(long)]
        txid: bool,

        /// Print operation details
        #[clap(long)]
        details: bool,
    },
}

impl<O: DescriptorOpts> Exec for Args<Command, O> {
    type Error = RuntimeError;
    const CONF_FILE_NAME: &'static str = "bp.toml";

    fn exec(self, mut config: Config, name: &'static str) -> Result<(), Self::Error> {
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
                    config.store(&self.conf_path(name));
                } else {
                    println!("Default wallet is '{}'", config.default_wallet);
                }
            }
            Command::Create { name } => {
                let mut runtime = self.bp_runtime::<O::Descr>(&config)?;
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
            Command::Addresses { change, count } => {
                /*
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!("Addresses (outer):");
                println!("Term.\tAddress\t\t\t\t\t\t\t\t# used\tVolume\tBalance");
                for info in runtime.address_all(*change as u8).take(*count as usize) {
                    let WalletAddr {
                        addr,
                        terminal,
                        used,
                        volume,
                        balance,
                    } = info;
                    println!("{terminal}\t{addr}\t{used}\t{volume}\t{balance}");
                }
                 */
            }
            Command::Coins => {
                /*
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!("Coins (UTXOs):");
                println!("Address\t{:>12}\tOutpoint", "Value");
                for (addr, coins) in runtime.address_coins() {
                    println!("{addr}:");
                    for utxo in coins {
                        let TxoInfo {
                            outpoint, value, ..
                        } = utxo;
                        println!("\t{:>12} ṩ\t{outpoint}", value.0);
                        //₿
                    }
                    println!();
                }
                 */
            }
            Command::History { txid, details } => {
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!(
                    "\nHeight\t{: <1$}\t    Amount, ṩ\tFee rate, ṩ/vbyte",
                    "Txid",
                    if *txid { 64 } else { 18 }
                );
                for row in runtime.history() {
                    println!(
                        "{}\t{}\t{}{: >12}\t{: >8.2}",
                        row.height
                            .as_ref()
                            .map(BlockHeight::to_string)
                            .unwrap_or_else(|| s!("mempool")),
                        if *txid { row.txid.to_string() } else { format!("{:#}", row.txid) },
                        row.operation,
                        row.amount,
                        row.fee.sats() as f64 * 4.0 / row.weight as f64
                    );
                    if *details {
                        for (cp, value) in &row.own {
                            println!(
                                "\t* {value: >-12}ṩ\t{}\t{cp}",
                                if *value < 0 {
                                    "debit from"
                                } else if row.operation == OpType::Credit {
                                    "credit to "
                                } else {
                                    "change to "
                                }
                            );
                        }
                        for (cp, value) in &row.counterparties {
                            println!(
                                "\t* {value: >-12}ṩ\t{}\t{cp}",
                                if *value > 0 {
                                    "paid from "
                                } else if row.operation == OpType::Credit {
                                    "change to "
                                } else {
                                    "sent to   "
                                }
                            );
                        }
                        println!("\t* {: >-12}ṩ\tminer fee", -row.fee.sats_i64());
                        println!();
                    }
                }
            }
        };

        println!();

        Ok(())
    }
}
