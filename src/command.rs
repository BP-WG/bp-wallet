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

use bp::DeriveSpk;
use bp_rt::{AddrInfo, Runtime, UtxoInfo};

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
pub enum Command {
    /// List addresses for the wallet descriptor.
    Addresses {
        #[clap(short, default_value = "20")]
        count: u16,
    },

    /// List available coins (unspent transaction outputs).
    Coins,
}

impl Command {
    pub fn exec<D: DeriveSpk, L2: Default>(self, runtime: &mut Runtime<D, L2>) {
        match self {
            Command::Addresses { count } => {
                println!();
                println!("Addresses (outer):");
                println!();
                println!("Term.\tAddress\t\t\t\t\t\t\t\t# used\tVolume\tBalance");
                for info in runtime.address_all().take(count as usize) {
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
                println!();
                println!("Coins (UTXOs):");
                println!();
                println!("Term.\tAddress\t\t\t\t\t\t\t\t# used\tVolume\tBalance");
                for (addr, coins) in runtime.address_coins() {
                    println!("{addr}");
                    for utxo in coins {
                        let UtxoInfo {
                            outpoint, value, ..
                        } = utxo;
                        println!("{value}ṩ\t{outpoint}");
                        //₿
                    }
                }
            }
        };

        println!();
    }
}
