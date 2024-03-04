// Modern, minimalistic & standard-compliant cold wallet library.
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2024 by
//     Nicola Busanello <nicola.busanello@gmail.com>
//
// Copyright (C) 2024 LNP/BP Standards Association. All rights reserved.
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

use descriptors::Descriptor;

use crate::{Indexer, Layer2, MayError, WalletCache, WalletDescr};

/// Type that contains any of the client types implementing the Indexer trait
#[derive(From)]
#[non_exhaustive]
pub enum AnyIndexer {
    #[cfg(feature = "electrum")]
    #[from]
    /// Electrum indexer
    Electrum(Box<electrum::client::Client>),
    #[cfg(feature = "esplora")]
    #[from]
    /// Esplora indexer
    Esplora(Box<esplora::blocking::BlockingClient>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error)]
#[display(doc_comments)]
pub enum AnyIndexerError {
    #[cfg(feature = "electrum")]
    #[display(inner)]
    Electrum(electrum::Error),
    #[cfg(feature = "esplora")]
    #[display(inner)]
    Esplora(esplora::Error),
}

impl Indexer for AnyIndexer {
    type Error = AnyIndexerError;

    fn create<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descr: &WalletDescr<K, D, L2::Descr>,
    ) -> MayError<WalletCache<L2::Cache>, Vec<Self::Error>> {
        match self {
            #[cfg(feature = "electrum")]
            AnyIndexer::Electrum(inner) => {
                let result = inner.create::<K, D, L2>(descr);
                MayError {
                    ok: result.ok,
                    err: result.err.map(|v| v.into_iter().map(|e| e.into()).collect()),
                }
            }
            #[cfg(feature = "esplora")]
            AnyIndexer::Esplora(inner) => {
                let result = inner.create::<K, D, L2>(descr);
                MayError {
                    ok: result.ok,
                    err: result.err.map(|v| v.into_iter().map(|e| e.into()).collect()),
                }
            }
        }
    }

    fn update<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descr: &WalletDescr<K, D, L2::Descr>,
        cache: &mut WalletCache<L2::Cache>,
    ) -> MayError<usize, Vec<Self::Error>> {
        match self {
            #[cfg(feature = "electrum")]
            AnyIndexer::Electrum(inner) => {
                let result = inner.update::<K, D, L2>(descr, cache);
                MayError {
                    ok: result.ok,
                    err: result.err.map(|v| v.into_iter().map(|e| e.into()).collect()),
                }
            }
            #[cfg(feature = "esplora")]
            AnyIndexer::Esplora(inner) => {
                let result = inner.update::<K, D, L2>(descr, cache);
                MayError {
                    ok: result.ok,
                    err: result.err.map(|v| v.into_iter().map(|e| e.into()).collect()),
                }
            }
        }
    }
}

#[cfg(feature = "electrum")]
impl From<electrum::Error> for AnyIndexerError {
    fn from(e: electrum::Error) -> Self { AnyIndexerError::Electrum(e) }
}

#[cfg(feature = "esplora")]
impl From<esplora::Error> for AnyIndexerError {
    fn from(e: esplora::Error) -> Self { AnyIndexerError::Esplora(e) }
}
