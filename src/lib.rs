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
#[cfg(feature = "serde")]
#[macro_use]
extern crate cfg_eval;
#[cfg(feature = "serde")]
extern crate serde_crate as serde;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde_with;

mod indexers;
#[cfg(feature = "fs")]
mod runtime;
mod util;
mod data;
mod rows;
mod wallet;
mod layer2;
mod payments;
pub mod coinselect;

pub use data::{
    BlockHeight, BlockInfo, MiningInfo, Party, TxCredit, TxDebit, TxStatus, WalletAddr, WalletTx,
    WalletUtxo,
};
#[cfg(any(feature = "electrum", feature = "esplora"))]
pub use indexers::AnyIndexer;
pub use indexers::Indexer;
pub use layer2::{
    Layer2, Layer2Cache, Layer2Coin, Layer2Data, Layer2Descriptor, Layer2Tx, NoLayer2,
};
pub use payments::{Amount, Beneficiary, ConstructionError, PsbtMeta, TxParams};
pub use rows::{CoinRow, Counterparty, OpType, TxRow};
#[cfg(feature = "fs")]
pub use runtime::{LoadError, Runtime, RuntimeError, StoreError};
pub use util::MayError;
pub use wallet::{Wallet, WalletCache, WalletData, WalletDescr};
