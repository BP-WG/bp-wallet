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

use std::num::NonZeroU32;

use bp::{
    Address, BlockHash, BlockHeader, DerivedAddr, Keychain, LockTime, Outpoint, Sats, SeqNo,
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
    pub header: BlockHeader,
    pub difficulty: u8,
    pub tx_count: u32,
    pub size: u32,
    pub weight: u32,
    pub mediantime: u32,
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

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase", bound = "")
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxInfo<C: Keychain> {
    pub txid: Txid,
    pub status: TxStatus,
    pub inputs: Vec<TxInInfo>,
    pub outputs: Vec<TxOutInfo<C>>,
    pub fee: Sats,
    pub size: u32,
    pub weight: u32,
    pub version: i32,
    pub locktime: LockTime,
}

#[cfg_attr(
    feature = "serde",
    serde_as,
    cfg_eval,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase")
)]
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxInInfo {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub outpoint: Outpoint,
    pub sequence: SeqNo,
    pub coinbase: bool,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub script_sig: SigScript,
    pub witness: Witness,
    pub value: Option<Sats>,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase", bound = "")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxOutInfo<C: Keychain> {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub outpoint: Outpoint,
    pub value: Sats,
    #[cfg_attr(feature = "serde", serde_as(as = "Option<DisplayFromStr>"))]
    pub derivation: Option<Terminal<C>>,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase", bound = "")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxoInfo<C: Keychain> {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub outpoint: Outpoint,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub terminal: Terminal<C>,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub address: Address,
    pub value: Sats,
}

#[cfg_attr(
    feature = "serde",
    cfg_eval,
    serde_as,
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate", rename_all = "camelCase", bound = "")
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct AddrInfo<C: Keychain> {
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub addr: Address,
    #[cfg_attr(feature = "serde", serde_as(as = "DisplayFromStr"))]
    pub terminal: Terminal<C>,
    pub used: u32,
    pub volume: Sats,
    pub balance: Sats,
}

impl<C: Keychain> From<DerivedAddr<C>> for AddrInfo<C> {
    fn from(derived: DerivedAddr<C>) -> Self {
        AddrInfo {
            addr: derived.addr,
            terminal: derived.terminal,
            used: 0,
            volume: Sats::ZERO,
            balance: Sats::ZERO,
        }
    }
}
