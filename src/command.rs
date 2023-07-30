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

use bp::{DeriveSpk, DerivedAddr};
use bp_rt::{Runtime, RuntimeError};

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
pub enum Command {
    /// List addresses for the wallet descriptor.
    Addresses {
        #[clap(short, default_value = "20")]
        count: u16,
    },
}

impl Command {
    pub fn exec<D: DeriveSpk, L2: Default>(
        self,
        runtime: &mut Runtime<D, L2>,
    ) -> Result<(), RuntimeError> {
        match self {
            Command::Addresses { count } => {
                println!();
                println!("Addresses (outer):");
                for derived in runtime.addresses().take(count as usize) {
                    let DerivedAddr {
                        addr,
                        keychain,
                        index,
                    } = derived;
                    println!("/{keychain}/{index}\t{addr}");
                }
            }
        };

        println!();
        Ok(())
    }
}
