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

use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::{error, io};

use amplify::IoError;
use bpstd::{Network, XpubDerivable};
use descriptors::{Descriptor, StdDescr};

use crate::wallet::fs::Warning;
use crate::{ConstructionError, Indexer, Layer2, NoLayer2, Wallet};

#[derive(Debug, Display, Error, From)]
#[non_exhaustive]
#[display(inner)]
pub enum RuntimeError<L2: error::Error = Infallible> {
    #[from]
    Load(LoadError<L2>),

    #[from]
    Store(StoreError<L2>),

    #[from]
    ConstructPsbt(ConstructionError),

    #[cfg(feature = "electrum")]
    /// error querying electrum server.
    ///
    /// {0}
    #[from]
    #[display(doc_comments)]
    Electrum(electrum::Error),

    #[cfg(feature = "esplora")]
    /// error querying esplora server.
    ///
    /// {0}
    #[from]
    #[display(doc_comments)]
    Esplora(esplora::Error),
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum LoadError<L2: error::Error = Infallible> {
    /// I/O error loading wallet - {0}
    #[from]
    #[from(io::Error)]
    Io(IoError),

    /// unable to parse TOML file - {0}
    #[from]
    Toml(toml::de::Error),

    #[display(inner)]
    Layer2(L2),

    #[display(inner)]
    #[from]
    Custom(String),
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum StoreError<L2: error::Error = Infallible> {
    /// I/O error storing wallet - {0}
    #[from]
    #[from(io::Error)]
    Io(IoError),

    /// unable to serialize wallet data as TOML file - {0}
    #[from]
    Toml(toml::ser::Error),

    /// unable to serialize wallet cache as YAML file - {0}
    #[from]
    Yaml(serde_yaml::Error),

    #[display(inner)]
    Layer2(L2),

    #[display(inner)]
    #[from]
    Custom(String),
}

#[derive(Getters, Debug)]
pub struct Runtime<D: Descriptor<K> = StdDescr, K = XpubDerivable, L2: Layer2 = NoLayer2> {
    path: Option<PathBuf>,
    #[getter(as_mut)]
    wallet: Wallet<K, D, L2>,
    warnings: Vec<Warning>,
}

impl<K, D: Descriptor<K>, L2: Layer2> Deref for Runtime<D, K, L2> {
    type Target = Wallet<K, D, L2>;
    fn deref(&self) -> &Self::Target { &self.wallet }
}

impl<K, D: Descriptor<K>, L2: Layer2> DerefMut for Runtime<D, K, L2> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.wallet }
}

impl<K, D: Descriptor<K>> Runtime<D, K> {
    pub fn new_standard(descr: D, network: Network) -> Self {
        Runtime {
            path: None,
            wallet: Wallet::new_standard(descr, network),
            warnings: none!(),
        }
    }
}

impl<K, D: Descriptor<K>, L2: Layer2> Runtime<D, K, L2> {
    pub fn new_layer2(descr: D, l2_descr: L2::Descr, layer2: L2, network: Network) -> Self {
        Runtime {
            path: None,
            wallet: Wallet::new_layer2(descr, l2_descr, layer2, network),
            warnings: none!(),
        }
    }
    pub fn set_name(&mut self, name: String) { self.wallet.set_name(name) }

    pub fn sync<I: Indexer>(&mut self, indexer: &I) -> Result<usize, Vec<I::Error>> {
        self.wallet.update(indexer).into_result()
    }

    #[inline]
    pub fn attach(wallet: Wallet<K, D, L2>) -> Self {
        Self {
            path: None,
            wallet,
            warnings: none!(),
        }
    }

    #[inline]
    pub fn detach(self) -> Wallet<K, D, L2> { self.wallet }

    pub fn reset_warnings(&mut self) { self.warnings.clear() }
}

impl<K, D: Descriptor<K>> Runtime<D, K>
where for<'de> D: serde::Serialize + serde::Deserialize<'de>
{
    pub fn load_standard(path: PathBuf) -> Result<Self, LoadError> {
        let (wallet, warnings) = Wallet::load(&path)?;
        Ok(Runtime {
            path: Some(path.clone()),
            wallet,
            warnings,
        })
    }

    pub fn load_standard_or_init<E>(
        data_dir: PathBuf,
        network: Network,
        init: impl FnOnce(LoadError) -> Result<D, E>,
    ) -> Result<Self, LoadError>
    where
        LoadError: From<E>,
    {
        Self::load_standard(data_dir).or_else(|err| {
            let descriptor = init(err)?;
            Ok(Self::new_standard(descriptor, network))
        })
    }
}

impl<K, D: Descriptor<K>, L2: Layer2> Runtime<D, K, L2>
where
    for<'de> D: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2::Descr: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2::Data: serde::Serialize + serde::Deserialize<'de>,
    for<'de> L2::Cache: serde::Serialize + serde::Deserialize<'de>,
{
    pub fn load_layer2(path: PathBuf) -> Result<Self, LoadError<L2::LoadError>> {
        let (wallet, warnings) = Wallet::load(&path)?;
        Ok(Runtime {
            path: Some(path.clone()),
            wallet,
            warnings,
        })
    }

    pub fn load_layer2_or_init<E1, E2>(
        data_dir: PathBuf,
        network: Network,
        init: impl FnOnce(LoadError<L2::LoadError>) -> Result<D, E1>,
        init_l2: impl FnOnce() -> Result<(L2, L2::Descr), E2>,
    ) -> Result<Self, LoadError<L2::LoadError>>
    where
        LoadError<L2::LoadError>: From<E1>,
        LoadError<L2::LoadError>: From<E2>,
    {
        Self::load_layer2(data_dir).or_else(|err| {
            let descriptor = init(err)?;
            let (layer2, l2_descr) = init_l2()?;
            Ok(Self::new_layer2(descriptor, l2_descr, layer2, network))
        })
    }

    pub fn try_store(&self) -> Result<bool, StoreError<L2::StoreError>> {
        let Some(path) = &self.path else {
            return Ok(false);
        };

        self.wallet.store(path)?;

        Ok(true)
    }

    pub fn store_as(&mut self, path: PathBuf) -> Result<(), StoreError<L2::StoreError>> {
        self.path = None;
        self.store_default_path(path)
    }

    pub fn store_default_path(&mut self, path: PathBuf) -> Result<(), StoreError<L2::StoreError>> {
        self.path = Some(path);
        let res = self.try_store()?;
        debug_assert!(res);
        Ok(())
    }
}
