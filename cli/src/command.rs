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
use std::fs::File;
use std::path::PathBuf;
use std::process::exit;

use bpstd::{Idx, NormalIndex, Sats, SeqNo};
use bpwallet::{coinselect, Amount, Invoice, OpType, TxParams, WalletUtxo};
use psbt::PsbtVer;
use strict_encoding::Ident;

use crate::opts::DescriptorOpts;
use crate::{Args, Config, Exec, RuntimeError, WalletAddr};

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
pub enum Command {
    /// List known wallets
    #[display("list")]
    List,

    /// Get or set default wallet
    #[display("default")]
    Default {
        /// Name of the wallet to make it default
        default: Option<Ident>,
    },

    /// Create a wallet
    #[display("create")]
    Create {
        /// The name for the new wallet
        name: Ident,
    },

    /// List wallet balance with additional optional details
    #[display("balance")]
    Balance {
        /// Print balance for each individual address
        #[clap(short, long)]
        addr: bool,

        /// Print information about individual UTXOs
        #[clap(short, long)]
        utxo: bool,
    },

    /// Generate a new wallet address(es)
    #[display("addr")]
    Addr {
        /// Use change keychain
        #[clap(short = '1', long)]
        change: bool,

        /// Use custom address index
        #[clap(short, long)]
        index: Option<NormalIndex>,

        /// Do not shift the last used index
        #[clap(short = 'N', long, conflicts_with_all = ["change", "index"])]
        no_shift: bool,

        /// Number of addresses to generate
        #[clap(short, long, default_value = "1")]
        no: u8,
    },

    /// Display history of wallet operations
    #[display("history")]
    History {
        /// Print full transaction ids
        #[clap(long)]
        txid: bool,

        /// Print operation details
        #[clap(long)]
        details: bool,
    },

    /// Compose a new PSBT to pay invoice
    #[display("construct")]
    Construct {
        /// Encode PSBT as V2
        #[clap(short = '2')]
        v2: bool,

        /// Bitcoin invoice in form of `<sats>@<address>`. To spend full wallet balance use
        /// `MAX` for the amount.
        invoice: Invoice,

        /// Fee
        fee: Sats,

        /// Name of PSBT file to save. If not given, prints PSBT to STDOUT
        psbt: Option<PathBuf>,
    },
}

impl<O: DescriptorOpts> Exec for Args<Command, O> {
    type Error = RuntimeError;
    const CONF_FILE_NAME: &'static str = "bp.toml";

    fn exec(mut self, mut config: Config, name: &'static str) -> Result<(), Self::Error> {
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
                if !self.wallet.descriptor_opts.is_some() {
                    eprintln!("Error: you must provide an argument specifying wallet descriptor");
                    exit(1);
                }
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
            Command::Balance {
                addr: false,
                utxo: false,
            } => {
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!("\nWallet total balance: {} ṩ", runtime.balance());
            }
            Command::Balance {
                addr: true,
                utxo: false,
            } => {
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!("\nTerm.\t{:62}\t# used\tVol., ṩ\tBalance, ṩ", "Address");
                for info in runtime.address_balance() {
                    let WalletAddr {
                        addr,
                        terminal,
                        used,
                        volume,
                        balance,
                    } = info;
                    println!("{terminal}\t{:62}\t{used}\t{volume}\t{balance}", addr.to_string());
                }
                self.command = Command::Balance {
                    addr: false,
                    utxo: false,
                };
                self.resolver.sync = false;
                self.exec(config, name)?;
            }
            Command::Balance {
                addr: false,
                utxo: true,
            } => {
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!("\nHeight\t{:>12}\t{:68}\tAddress", "Amount, ṩ", "Outpoint");
                for row in runtime.coins() {
                    println!(
                        "{}\t{: >12}\t{:68}\t{}",
                        row.height, row.amount, row.outpoint, row.address
                    );
                }
                self.command = Command::Balance {
                    addr: false,
                    utxo: false,
                };
                self.resolver.sync = false;
                self.exec(config, name)?;
            }
            Command::Balance {
                addr: true,
                utxo: true,
            } => {
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!("\nHeight\t{:>12}\t{:68}", "Amount, ṩ", "Outpoint");
                for (derived_addr, utxos) in runtime.address_coins() {
                    println!("{}\t{}", derived_addr.addr, derived_addr.terminal);
                    for row in utxos {
                        println!("{}\t{: >12}\t{:68}", row.height, row.amount, row.outpoint);
                    }
                    println!()
                }
                self.command = Command::Balance {
                    addr: false,
                    utxo: false,
                };
                self.resolver.sync = false;
                self.exec(config, name)?;
            }
            Command::Addr {
                change,
                index,
                no_shift,
                no,
            } => {
                let mut runtime = self.bp_runtime::<O::Descr>(&config)?;
                let keychain = *change as u8;
                let index = index.unwrap_or_else(|| runtime.next_index(keychain, !*no_shift));
                println!("\nTerm.\tAddress");
                for derived_addr in
                    runtime.addresses(keychain).skip(index.index() as usize).take(*no as usize)
                {
                    println!("{}\t{}", derived_addr.terminal, derived_addr.addr);
                }
                runtime.try_store()?;
            }
            Command::History { txid, details } => {
                let runtime = self.bp_runtime::<O::Descr>(&config)?;
                println!(
                    "\nHeight\t{:<1$}\t    Amount, ṩ\tFee rate, ṩ/vbyte",
                    "Txid",
                    if *txid { 64 } else { 18 }
                );
                for row in runtime.history() {
                    println!(
                        "{}\t{}\t{}{: >12}\t{: >8.2}",
                        row.height,
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
            Command::Construct {
                v2,
                invoice,
                fee,
                psbt: psbt_file,
            } => {
                let mut runtime = self.bp_runtime::<O::Descr>(&config)?;
                let params = TxParams {
                    fee: *fee,
                    lock_time: None,
                    // TODO: Support lock time and RBFs
                    seq_no: SeqNo::from_consensus_u32(0),
                };
                // Do coin selection
                let coins: Vec<_> = match invoice.amount {
                    Amount::Fixed(sats) => {
                        runtime.wallet().coinselect(sats, coinselect::all).collect()
                    }
                    Amount::Max => {
                        runtime.wallet().all_utxos().map(WalletUtxo::into_outpoint).collect()
                    }
                };
                let psbt = runtime.wallet_mut().construct_psbt(&coins, *invoice, params)?;
                let ver = if *v2 { PsbtVer::V2 } else { PsbtVer::V0 };
                eprintln!("{}", serde_yaml::to_string(&psbt).unwrap());
                match psbt_file {
                    Some(file_name) => {
                        let mut psbt_file = File::create(file_name)?;
                        psbt.encode(ver, &mut psbt_file)?;
                    }
                    None => match ver {
                        PsbtVer::V0 => println!("{psbt}"),
                        PsbtVer::V2 => println!("{psbt:#}"),
                    },
                }
            }
        };

        println!();

        Ok(())
    }
}
