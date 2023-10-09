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

#[cfg(feature = "esplora")]
mod esplora;

use descriptors::Descriptor;

use crate::{Layer2, MayError, WalletCache, WalletDescr};

pub(self) const BATCH_SIZE: u8 = 10;

pub trait Indexer {
    type Error;

    fn create<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descr: &WalletDescr<K, D, L2::Descr>,
    ) -> MayError<WalletCache<L2::Cache>, Vec<Self::Error>>;

    fn update<K, D: Descriptor<K>, L2: Layer2>(
        &self,
        descr: &WalletDescr<K, D, L2::Descr>,
        cache: &mut WalletCache<L2::Cache>,
    ) -> (usize, Vec<Self::Error>);
}
