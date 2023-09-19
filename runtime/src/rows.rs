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

use std::collections::HashMap;

use bp::{Address, Sats, ScriptPubkey, Txid};

use crate::{BlockHeight, Layer2Cache, Layer2Tx, WalletCache};

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
pub enum OpType {
    #[display("+")]
    Credit,
    #[display("-")]
    Debit,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
#[display(inner)]
pub enum Counterparty {
    Address(Address),
    #[display(LowerHex)]
    Unknown(ScriptPubkey),
}

#[cfg_attr(
    feature = "serde",
    serde_as,
    cfg_eval,
    derive(serde::Serialize, serde::Deserialize),
    serde(
        crate = "serde_crate",
        rename_all = "camelCase",
        bound(
            serialize = "L2: serde::Serialize",
            deserialize = "for<'d> L2: serde::Deserialize<'d>"
        )
    )
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxRow<L2: Layer2Tx> {
    pub height: BlockHeight,
    // TODO: Add date/time
    pub operation: OpType,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub counterparties: HashMap<Counterparty, Sats>,
    pub txid: Txid,
    pub fee: Sats,
    pub weight: u32,
    pub vbytes: u32,
    pub total: Sats,
    pub amount: Sats,
    pub balance: Sats,
    pub layer2: L2,
}

impl<L2: Layer2Cache> WalletCache<L2> {
    pub fn history(&self) -> Vec<TxRow<L2::Tx>> { for out in &self.outputs {} }
}
