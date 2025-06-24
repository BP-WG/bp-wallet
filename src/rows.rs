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

use std::fmt::{self, Display, Formatter, LowerHex};
use std::str::FromStr;

use amplify::hex::FromHex;
use bpstd::{Address, DerivedAddr, Outpoint, Sats, ScriptPubkey, Txid};

use crate::{BlockHeight, Layer2Coin, Layer2Empty, Layer2Tx, Party, TxStatus};

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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
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
pub struct TxRow<L2: Layer2Tx = Layer2Empty> {
    pub height: TxStatus<BlockHeight>,
    // TODO: Add date/time
    pub operation: OpType,
    pub our_inputs: Vec<u32>,
    pub counterparties: Vec<(Counterparty, i64)>,
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
pub struct CoinRow<L2: Layer2Coin> {
    pub height: TxStatus<BlockHeight>,
    // TODO: Add date/time
    pub address: DerivedAddr,
    pub outpoint: Outpoint,
    pub amount: Sats,
    pub layer2: Vec<L2>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counterparty_str_round_trip() {
        fn assert_from_str_to_str(counterparty: Counterparty) {
            let str = counterparty.to_string();
            let from_str = Counterparty::from_str(&str).unwrap();

            assert_eq!(counterparty, from_str);
        }

        assert_from_str_to_str(Counterparty::Miner);
        assert_from_str_to_str(Counterparty::Address(
            Address::from_str("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq").unwrap(),
        ));
        assert_from_str_to_str(Counterparty::Unknown(
            ScriptPubkey::from_hex("0014a3f8e1f1e1c7e8b4b2f4f3a1b4f7f0a1b4f7f0a1").unwrap(),
        ));
    }
}
