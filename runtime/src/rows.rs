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
use std::fmt::{self, Display, Formatter, LowerHex};
use std::str::FromStr;

use amplify::hex::FromHex;
use bp::{Address, Sats, ScriptPubkey, Txid};
#[cfg(feature = "serde")]
use serde_with::DisplayFromStr;

use crate::{BlockHeight, Layer2Cache, Layer2Tx, Party, WalletCache};

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

#[derive(Clone, Eq, PartialEq, Hash, Debug, From)]
pub enum Counterparty {
    Miner,
    #[from]
    Address(Address),
    #[from]
    Unknown(ScriptPubkey),
}

impl From<Party> for Counterparty {
    fn from(party: Party) -> Self {
        match party {
            Party::Subsidy => Counterparty::Miner,
            Party::Counterparty(addr) => Counterparty::Address(addr),
            Party::Unknown(script) => Counterparty::Unknown(script),
            Party::Wallet(_) => {
                panic!("counterparty must be constructed only for external parties")
            }
        }
    }
}

impl Display for Counterparty {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Counterparty::Miner => f.write_str("miner"),
            Counterparty::Address(addr) => Display::fmt(addr, f),
            Counterparty::Unknown(script) => LowerHex::fmt(script, f),
        }
    }
}

impl FromStr for Counterparty {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "miner" {
            return Ok(Counterparty::Miner);
        }
        Address::from_str(s)
            .map(Self::from)
            .or_else(|_| ScriptPubkey::from_hex(s).map(Self::from))
            .map_err(|_| s.to_owned())
    }
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
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
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TxRow<L2: Layer2Tx> {
    pub height: Option<BlockHeight>,
    // TODO: Add date/time
    pub operation: OpType,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub counterparties: HashMap<Counterparty, Sats>,
    pub txid: Txid,
    pub fee: Sats,
    pub weight: u32,
    pub size: u32,
    pub total: Sats,
    pub amount: Sats,
    pub balance: Sats,
    pub layer2: L2,
}

impl<L2: Layer2Cache> WalletCache<L2> {
    pub fn history(&self) -> impl Iterator<Item = TxRow<L2::Tx>> + '_ {
        self.tx.values().flat_map(|tx| {
            let mut rows = Vec::with_capacity(2);
            let (credit, debit) = tx.credited_debited();
            let mut row = TxRow {
                height: tx.status.map(|info| info.height),
                operation: OpType::Credit,
                counterparties: none!(),
                txid: tx.txid,
                fee: tx.fee,
                weight: tx.weight,
                size: tx.size,
                total: tx.total_moved(),
                amount: Sats::ZERO,
                balance: Sats::ZERO,
                layer2: none!(), // TODO: Add support to WalletTx
            };
            // TODO: Add balance calculation
            if credit.is_non_zero() {
                row.counterparties = tx.credits().fold(HashMap::new(), |mut cp, inp| {
                    let party = Counterparty::from(inp.payer.clone());
                    cp.entry(party).or_default().saturating_add_assign(inp.value);
                    cp
                });
                row.operation = OpType::Credit;
                row.amount = credit;
                rows.push(row.clone());
            }
            if debit.is_non_zero() {
                row.counterparties = tx.debits().fold(HashMap::new(), |mut cp, out| {
                    let party = Counterparty::from(out.beneficiary.clone());
                    cp.entry(party).or_default().saturating_add_assign(out.value);
                    cp
                });
                row.operation = OpType::Debit;
                row.amount = debit;
                rows.push(row);
            }
            rows
        })
    }
}
