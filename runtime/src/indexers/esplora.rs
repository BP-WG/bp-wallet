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

use std::cmp::max;

use bp::{Address, DeriveSpk, Idx, Keychain, NormalIndex, Outpoint, Terminal};
use esplora::{BlockingClient, Error};

use super::BATCH_SIZE;
use crate::{Indexer, MayError, TxoInfo, WalletCache, WalletDescr};

impl Indexer for BlockingClient {
    type Error = Error;

    fn create<D: DeriveSpk, C: Keychain>(
        &self,
        descriptor: &WalletDescr<D, C>,
    ) -> MayError<WalletCache<C>, Vec<Self::Error>> {
        let mut cache = WalletCache::new();
        let mut errors = vec![];

        for keychain in &descriptor.keychains {
            let mut index = NormalIndex::ZERO;
            let max_known = cache.max_known.entry(keychain.derivation()).or_default();
            let mut empty_count = 0usize;
            loop {
                let script = descriptor.derive(*keychain, index);

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
                        *max_known = max(*max_known, index);
                        for tx in txes {
                            for (vout, out) in tx.vout.iter().enumerate() {
                                if out.scriptpubkey != script {
                                    continue;
                                }
                                let utxo = TxoInfo {
                                    outpoint: Outpoint::new(tx.txid, vout as u32),
                                    terminal: Terminal::new(*keychain, index),
                                    address,
                                    value: out.value.into(),
                                };
                                cache.utxo.entry(address).or_default().insert(utxo);
                            }
                        }
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

    fn update<D: DeriveSpk, C: Keychain>(
        &self,
        descr: &WalletDescr<D, C>,
        cache: &mut WalletCache<C>,
    ) -> (usize, Vec<Self::Error>) {
        todo!()
    }
}
