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
use bp::{Address, DerivedAddr, Outpoint, Sats, ScriptPubkey, Txid};
#[cfg(feature = "serde")]
use serde_with::DisplayFromStr;

use crate::{BlockHeight, Layer2Cache, Layer2Coin, Layer2Tx, Party, TxStatus, WalletCache};

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
    pub height: TxStatus<BlockHeight>,
    // TODO: Add date/time
    pub operation: OpType,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub counterparties: Vec<(Counterparty, i64)>,
    #[cfg_attr(feature = "serde", serde_as(as = "HashMap<DisplayFromStr, _>"))]
    pub own: Vec<(DerivedAddr, i64)>,
    pub txid: Txid,
    pub fee: Sats,
    pub weight: u32,
    pub size: u32,
    pub total: Sats,
    pub amount: Sats,
    pub balance: Sats,
    pub layer2: L2,
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
pub struct CoinRow<L2: Layer2Coin> {
    pub height: TxStatus<BlockHeight>,
    // TODO: Add date/time
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub address: DerivedAddr,
    pub outpoint: Outpoint,
    pub amount: Sats,
    pub layer2: Vec<L2>,
}

impl<L2: Layer2Cache> WalletCache<L2> {
    pub fn coins(&self) -> impl Iterator<Item = CoinRow<L2::Coin>> + '_ {
        self.utxo.iter().map(|utxo| {
            let Some(tx) = self.tx.get(&utxo.txid) else {
                panic!("cache data inconsistency");
            };
            let Some(out) = tx.outputs.get(utxo.vout_usize()) else {
                panic!("cache data inconsistency");
            };
            CoinRow {
                height: tx.status.map(|info| info.height),
                outpoint: *utxo,
                address: out.derived_addr().expect("cache data inconsistency"),
                amount: out.value,
                layer2: none!(), // TODO: Add support to WalletTx
            }
        })
    }

    pub fn history(&self) -> impl Iterator<Item = TxRow<L2::Tx>> + '_ {
        self.tx.values().map(|tx| {
            let (credit, debit) = tx.credited_debited();
            let mut row = TxRow {
                height: tx.status.map(|info| info.height),
                operation: OpType::Credit,
                counterparties: none!(),
                own: none!(),
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
            row.own = tx
                .inputs
                .iter()
                .filter_map(|i| i.derived_addr().map(|a| (a, -i.value.sats_i64())))
                .chain(
                    tx.outputs
                        .iter()
                        .filter_map(|o| o.derived_addr().map(|a| (a, o.value.sats_i64()))),
                )
                .collect();
            if credit.is_non_zero() {
                row.counterparties = tx.credits().fold(Vec::new(), |mut cp, inp| {
                    let party = Counterparty::from(inp.payer.clone());
                    cp.push((party, inp.value.sats_i64()));
                    cp
                });
                row.counterparties.extend(tx.debits().fold(Vec::new(), |mut cp, out| {
                    let party = Counterparty::from(out.beneficiary.clone());
                    cp.push((party, -out.value.sats_i64()));
                    cp
                }));
                row.operation = OpType::Credit;
                row.amount = credit - debit - tx.fee;
            } else if debit.is_non_zero() {
                row.counterparties = tx.debits().fold(Vec::new(), |mut cp, out| {
                    let party = Counterparty::from(out.beneficiary.clone());
                    cp.push((party, -out.value.sats_i64()));
                    cp
                });
                row.operation = OpType::Debit;
                row.amount = debit;
            }
            row
        })
    }
}
