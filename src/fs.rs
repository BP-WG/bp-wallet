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

use std::fs;
use std::path::PathBuf;

use descriptors::Descriptor;
use nonasync::persistence::{PersistenceError, PersistenceProvider};

use super::*;
use crate::{
    Layer2Cache, Layer2Data, Layer2Descriptor, NoLayer2, WalletCache, WalletData, WalletDescr,
};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct FsTextStore {
    pub descr: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub l2: PathBuf,
}

impl FsTextStore {
    pub fn new(path: PathBuf) -> Self {
        let mut descr = path.clone();
        descr.push("descriptor.toml");
        let mut data = path.clone();
        data.push("data.toml");
        let mut cache = path.clone();
        cache.push("cache.yaml");
        let mut l2 = path;
        l2.push("layer2.yaml");

        Self {
            descr,
            data,
            cache,
            l2,
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2Descriptor> PersistenceProvider<WalletDescr<K, D, L2>>
    for FsTextStore
where
    for<'de> WalletDescr<K, D, L2>: serde::Serialize + serde::Deserialize<'de>,
    for<'de> D: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
{
    fn load(&self) -> Result<WalletDescr<K, D, L2>, PersistenceError> {
        let descr = fs::read_to_string(&self.descr).map_err(PersistenceError::with)?;
        toml::from_str(&descr).map_err(PersistenceError::with)
    }

    fn store(&self, object: &WalletDescr<K, D, L2>) -> Result<(), PersistenceError> {
        let s = toml::to_string_pretty(object).map_err(PersistenceError::with)?;
        fs::write(&self.descr, s).map_err(PersistenceError::with)?;
        Ok(())
    }
}

impl<L2: Layer2Cache> PersistenceProvider<WalletCache<L2>> for FsTextStore
where
    for<'de> WalletCache<L2>: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
{
    fn load(&self) -> Result<WalletCache<L2>, PersistenceError> {
        let file = fs::File::open(&self.cache).map_err(PersistenceError::with)?;
        serde_yaml::from_reader(file).map_err(PersistenceError::with)
    }

    fn store(&self, object: &WalletCache<L2>) -> Result<(), PersistenceError> {
        let file = fs::File::create(&self.cache).map_err(PersistenceError::with)?;
        serde_yaml::to_writer(file, object).map_err(PersistenceError::with)?;
        Ok(())
    }
}

impl<L2: Layer2Data> PersistenceProvider<WalletData<L2>> for FsTextStore
where
    for<'de> WalletData<L2>: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
{
    fn load(&self) -> Result<WalletData<L2>, PersistenceError> {
        let data = fs::read_to_string(&self.data).map_err(PersistenceError::with)?;
        toml::from_str(&data).map_err(PersistenceError::with)
    }

    fn store(&self, object: &WalletData<L2>) -> Result<(), PersistenceError> {
        let s = toml::to_string_pretty(object).map_err(PersistenceError::with)?;
        fs::write(&self.data, s).map_err(PersistenceError::with)?;
        Ok(())
    }
}

impl PersistenceProvider<NoLayer2> for FsTextStore {
    fn load(&self) -> Result<NoLayer2, PersistenceError> {
        // Nothing to do
        Ok(none!())
    }

    fn store(&self, _: &NoLayer2) -> Result<(), PersistenceError> {
        // Nothing to do
        Ok(())
    }
}
