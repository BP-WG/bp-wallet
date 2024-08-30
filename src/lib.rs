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

#[macro_use]
extern crate amplify;
#[cfg(feature = "serde")]
extern crate serde_crate as serde;
#[macro_use]
#[cfg(feature = "clap")]
extern crate clap;
#[macro_use]
#[cfg(feature = "log")]
extern crate log;

pub mod indexers;
mod util;
mod data;
mod rows;
mod wallet;
mod layer2;
pub mod coinselect;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "hot")]
pub mod hot;
mod bip43;

pub use bip43::{Bip43, DerivationStandard, ParseBip43Error};
pub use bpstd::*;
pub use data::{
    BlockHeight, BlockInfo, MiningInfo, Party, TxCredit, TxDebit, TxStatus, WalletAddr, WalletTx,
    WalletUtxo,
};
#[cfg(all(feature = "cli", feature = "hot"))]
pub use hot::{HotArgs, HotCommand};
#[cfg(feature = "hot")]
pub use hot::{Seed, SeedType};
pub use indexers::Indexer;
#[cfg(any(feature = "electrum", feature = "esplora", feature = "mempool"))]
pub use indexers::{AnyIndexer, AnyIndexerError};
pub use layer2::{
    Layer2, Layer2Cache, Layer2Coin, Layer2Data, Layer2Descriptor, Layer2Tx, NoLayer2,
};
pub use rows::{CoinRow, Counterparty, OpType, TxRow};
pub use util::MayError;
#[cfg(feature = "fs")]
pub use wallet::{fs, FsConfig};
pub use wallet::{Save, Wallet, WalletCache, WalletData, WalletDescr};
