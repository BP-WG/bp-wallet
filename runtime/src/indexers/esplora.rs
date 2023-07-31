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

use bp::{DeriveSpk, Idx, NormalIndex};
use esplora::{BlockingClient, Error};

use super::BATCH_SIZE;
use crate::{Indexer, MayError, WalletCache, WalletDescr};

impl Indexer for BlockingClient {
    type Error = Error;

    fn create<D: DeriveSpk>(
        &self,
        descriptor: &WalletDescr<D>,
    ) -> MayError<WalletCache, Vec<Self::Error>> {
        let mut cache = WalletCache::new();
        let mut errors = vec![];

        for keychain in &descriptor.keychains {
            let mut index = NormalIndex::ZERO;
            let max_known = cache.max_known.entry(*keychain).or_default();
            loop {
                let scripts = descriptor.derive_batch(keychain, index, BATCH_SIZE);

                for script in scripts {
                    match self.scripthash_txs(&script, None) {
                        Err(err) => errors.push(err),
                        Ok(txes) => {
                            if txes.is_empty() {
                                break;
                            }
                            *max_known = max(*max_known, index);
                            for tx in txes {
                                for out in tx.vout {}
                            }
                        }
                    }
                }

                if !index.saturating_add_assign(BATCH_SIZE) {
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

    fn update<D: DeriveSpk>(
        &self,
        descr: &WalletDescr<D>,
        cache: &mut WalletCache,
    ) -> (usize, Vec<Self::Error>) {
        todo!()
    }
}
