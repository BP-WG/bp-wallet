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

use bp::{Address, DeriveSpk, Idx, LockTime, NormalIndex, Outpoint, SeqNo, Witness};
use esplora::{BlockingClient, Error};

use super::BATCH_SIZE;
use crate::{
    Indexer, Layer2, MayError, MiningInfo, Party, TxCredit, TxDebit, TxStatus, WalletCache,
    WalletDescr, WalletTx,
};

impl From<esplora::TxStatus> for TxStatus {
    fn from(status: esplora::TxStatus) -> Self {
        if let esplora::TxStatus {
            confirmed: true,
            block_height: Some(height),
            block_hash: Some(hash),
            block_time: Some(ts),
        } = status
        {
            TxStatus::Mined(MiningInfo {
                height: NonZeroU32::try_from(height).unwrap_or(NonZeroU32::MIN),
                time: ts,
                block_hash: hash,
            })
        } else {
            TxStatus::Mempool
        }
    }
}

impl From<esplora::PrevOut> for Party {
    fn from(prevout: esplora::PrevOut) -> Self { Party::Unknown(prevout.scriptpubkey) }
}

impl From<esplora::Vin> for TxCredit {
    fn from(vin: esplora::Vin) -> Self {
        TxCredit {
            outpoint: Outpoint::new(vin.txid, vin.vout),
            sequence: SeqNo::from_consensus_u32(vin.sequence),
            coinbase: vin.is_coinbase,
            script_sig: vin.scriptsig,
            witness: Witness::from_consensus_stack(vin.witness),
            value: vin.prevout.as_ref().map(|prevout| prevout.value).unwrap_or_default().into(),
            payer: vin.prevout.map(Party::from).unwrap_or(Party::Subsidy),
        }
    }
}

impl From<esplora::Tx> for WalletTx {
    fn from(tx: esplora::Tx) -> Self {
        WalletTx {
            txid: tx.txid,
            status: tx.status.into(),
            inputs: tx.vin.into_iter().map(TxCredit::from).collect(),
            outputs: tx
                .vout
                .into_iter()
                .enumerate()
                .map(|(n, vout)| TxDebit {
                    outpoint: Outpoint::new(tx.txid, n as u32),
                    beneficiary: Party::from(vout.scriptpubkey),
                    value: vout.value.into(),
                    derivation: None,
                    spent: None,
                })
                .collect(),
            fee: tx.fee.into(),
            size: tx.size,
            weight: tx.weight,
            version: tx.version,
            locktime: LockTime::from_consensus_u32(tx.locktime),
        }
    }
}

impl Indexer for BlockingClient {
    type Error = Error;

    fn create<D: DeriveSpk, L2: Layer2>(
        &self,
        descriptor: &WalletDescr<D, L2::Descr>,
    ) -> MayError<WalletCache<L2::Cache>, Vec<Self::Error>> {
        let mut cache = WalletCache::new();
        let mut errors = vec![];

        for keychain in descriptor.keychains() {
            let mut index = NormalIndex::ZERO;
            let mut empty_count = 0usize;
            loop {
                let script = descriptor.derive(keychain, index);

                let address =
                    Address::with(&script, descriptor.chain).expect("descriptor guarantees");
                eprint!(".");
                match self.scripthash_txs(&script, None) {
                    Err(err) => errors.push(err),
                    Ok(txes) if txes.is_empty() => {
                        empty_count += 1;
                        if empty_count >= BATCH_SIZE as usize {
                            break;
                        }
                    }
                    Ok(txes) => {
                        empty_count = 0;
                        cache
                            .tx
                            .extend(txes.into_iter().map(WalletTx::from).map(|tx| (tx.txid, tx)));
                    }
                }

                if index.checked_inc_assign().is_none() {
                    break;
                }
            }
        }

        // TODO: Update headers & tip
        // TODO: Construct addr information

        if errors.is_empty() {
            MayError::ok(cache)
        } else {
            MayError::err(cache, errors)
        }
    }

    fn update<D: DeriveSpk, L2: Layer2>(
        &self,
        descr: &WalletDescr<D, L2::Descr>,
        cache: &mut WalletCache<L2::Cache>,
    ) -> (usize, Vec<Self::Error>) {
        todo!()
    }
}
