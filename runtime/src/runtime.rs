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

use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use bp::{Bip32Keychain, Chain, DeriveSpk, DescriptorStd, Keychain};

use crate::{Indexer, Wallet, WalletDescr};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum RuntimeError {
    #[from]
    Load(LoadError),
    #[from]
    Explora(esplora::Error),
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum LoadError {
    #[from]
    Io(io::Error),

    #[from]
    Toml(toml::de::Error),

    #[from]
    Custom(String),
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum StoreError {
    #[from]
    Io(io::Error),

    #[from]
    Toml(toml::ser::Error),

    #[from]
    Custom(String),
}

#[derive(Getters, Debug)]
pub struct Runtime<D: DeriveSpk = DescriptorStd, C: Keychain = Bip32Keychain> {
    path: Option<PathBuf>,
    #[getter(as_mut)]
    wallet: Wallet<D, C>,
}

impl<D: DeriveSpk, C: Keychain> Deref for Runtime<D, C> {
    type Target = Wallet<D, C>;
    fn deref(&self) -> &Self::Target { &self.wallet }
}

impl<D: DeriveSpk, C: Keychain> DerefMut for Runtime<D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.wallet }
}

impl<D: DeriveSpk, C: Keychain> Runtime<D, C> {
    pub fn new(descr: D, network: Chain) -> Self {
        Runtime {
            path: None,
            wallet: Wallet::new_standard(descr, network),
        }
    }

    pub fn set_name(&mut self, name: String) { self.wallet.set_name(name) }

    pub fn sync<I: Indexer>(&mut self, indexer: &I) -> Result<(), Vec<I::Error>> {
        self.wallet.update(indexer).into_result()
    }

    #[inline]
    pub fn attach(wallet: Wallet<D, C>) -> Self { Self { path: None, wallet } }

    #[inline]
    pub fn detach(self) -> Wallet<D, C> { self.wallet }
}

impl<D: DeriveSpk, C: Keychain> Runtime<D, C>
where for<'de> WalletDescr<D, C>: serde::Serialize + serde::Deserialize<'de>
{
    pub fn load(path: PathBuf) -> Result<Self, LoadError> {
        Ok(Runtime {
            path: Some(path.clone()),
            wallet: Wallet::load(&path)?,
        })
    }

    pub fn load_or_init<E>(
        data_dir: PathBuf,
        chain: Chain,
        init: impl FnOnce(LoadError) -> Result<D, E>,
    ) -> Result<Self, LoadError>
    where
        LoadError: From<E>,
    {
        Self::load(data_dir).or_else(|err| {
            let descriptor = init(err)?;
            Ok(Self::new(descriptor, chain))
        })
    }

    pub fn try_store(&self) -> Result<bool, StoreError> {
        let Some(path) = &self.path else {
            return Ok(false);
        };

        self.wallet.store(path)?;

        Ok(true)
    }

    pub fn store_as(&mut self, path: PathBuf) -> Result<(), StoreError> {
        self.path = None;
        self.store_default_path(path)
    }

    pub fn store_default_path(&mut self, path: PathBuf) -> Result<(), StoreError> {
        self.path = Some(path);
        let res = self.try_store()?;
        debug_assert!(res);
        Ok(())
    }
}
