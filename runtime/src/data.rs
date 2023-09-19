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

use std::cmp::Ordering;
use std::fmt::{self, Display, Formatter, LowerHex};
use std::num::{NonZeroU32, ParseIntError};
use std::str::FromStr;

use amplify::hex;
use amplify::hex::FromHex;
use bp::{
    Address, BlockHash, BlockHeader, DerivedAddr, LockTime, Outpoint, Sats, ScriptPubkey, SeqNo,
    SigScript, Terminal, Txid, Witness,
};
#[cfg(feature = "serde")]
use serde_with::DisplayFromStr;

pub type BlockHeight = NonZeroU32;

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct BlockInfo {
    pub mined: MiningInfo,
    pub header: BlockHeader,
    pub difficulty: u8,
    pub tx_count: u32,
    pub size: u32,
    pub weight: u32,
    pub mediantime: u32,
}

impl Ord for BlockInfo {
    fn cmp(&self, other: &Self) -> Ordering { self.mined.cmp(&other.mined) }
}

impl PartialOrd for BlockInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct MiningInfo {
    pub height: BlockHeight,
    pub time: u64,
    pub block_hash: BlockHash,
}

impl Ord for MiningInfo {
    fn cmp(&self, other: &Self) -> Ordering { self.height.cmp(&other.height) }
}

impl PartialOrd for MiningInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl MiningInfo {
    pub fn genesis() -> Self {
        MiningInfo {
            height: BlockHeight::MIN,
            time: 1231006505,
            block_hash: BlockHash::from_hex(
                "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
            )
            .unwrap(),
        }
    }
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum TxStatus {
    Mined(MiningInfo),
    Mempool,
    Channel,
    Unknown,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Display)]
#[display("{txid}.{vin}")]
pub struct Inpoint {
    pub txid: Txid,
    pub vin: u32,
}

impl Inpoint {
    #[inline]
    pub fn new(txid: Txid, vin: u32) -> Self { Inpoint { txid, vin } }
}

#[derive(Clone, Eq, PartialEq, Debug, Display, From, Error)]
#[display(doc_comments)]
pub enum InpointParseError {
    /// malformed string representation of transaction input '{0}' lacking txid and vin
    /// separator '.'
    MalformedSeparator(String),

    /// malformed transaction input number. Details: {0}
    #[from]
    InvalidVout(ParseIntError),

    /// malformed transaction input txid value. Details: {0}
    #[from]
    InvalidTxid(hex::Error),
}

impl FromStr for Inpoint {
    type Err = InpointParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (txid, vin) =
            s.split_once('.').ok_or_else(|| InpointParseError::MalformedSeparator(s.to_owned()))?;
        Ok(Inpoint::new(txid.parse()?, u32::from_str(vin)?))
    }
}

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct WalletTx {
    pub txid: Txid,
    pub status: TxStatus,
    pub inputs: Vec<TxCredit>,
    pub outputs: Vec<TxDebit>,
    pub fee: Sats,
    pub size: u32,
    pub weight: u32,
    pub version: i32,
    pub locktime: LockTime,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, From)]
pub enum Party {
    Subsidy,

    #[from]
    Counterparty(Address),

    #[from]
    Unknown(ScriptPubkey),

    #[from]
    Wallet(Terminal),
}

impl Display for Party {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Party::Subsidy => f.write_str("coinbase"),
            Party::Counterparty(addr) => Display::fmt(addr, f),
            Party::Unknown(script) => LowerHex::fmt(script, f),
            Party::Wallet(term) => Display::fmt(term, f),
        }
    }
}

impl FromStr for Party {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "coinbase" {
            return Ok(Party::Subsidy);
        }
        Address::from_str(s)
            .map(Self::from)
            .or_else(|_| Terminal::from_str(s).map(Self::from))
            .or_else(|_| ScriptPubkey::from_hex(s).map(Self::from))
            .map_err(|_| s.to_owned())
    }
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase",)
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxCredit {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub outpoint: Outpoint,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub payer: Party,
    pub sequence: SeqNo,
    pub coinbase: bool,
    pub script_sig: SigScript,
    pub witness: Witness,
    pub value: Sats,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxDebit {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub outpoint: Outpoint,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub beneficiary: Party,
    pub value: Sats,
    #[cfg_attr(feature = "serde", serde_as(as = "Option<DisplayFromStr>"))]
    pub derivation: Option<Terminal>,
    #[cfg_attr(feature = "serde", serde_as(as = "Option<DisplayFromStr>"))]
    pub spent: Option<Inpoint>,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct WalletAddr {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub terminal: Terminal,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub addr: Address,
    pub used: u32,
    pub volume: Sats,
    pub balance: Sats,
}

impl Ord for WalletAddr {
    fn cmp(&self, other: &Self) -> Ordering { self.terminal.cmp(&other.terminal) }
}

impl PartialOrd for WalletAddr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl From<DerivedAddr> for WalletAddr {
    fn from(derived: DerivedAddr) -> Self {
        WalletAddr {
            addr: derived.addr,
            terminal: derived.terminal,
            used: 0,
            volume: Sats::ZERO,
            balance: Sats::ZERO,
        }
    }
}
