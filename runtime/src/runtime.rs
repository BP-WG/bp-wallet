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

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::{fs, io};

use bp::{Chain, DeriveSpk, DescriptorStd};

use crate::{Indexer, Wallet, WalletDescr};

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

#[derive(Getters, Debug)]
pub struct Runtime<D: DeriveSpk = DescriptorStd> {
    path: Option<PathBuf>,
    #[getter(as_mut)]
    wallet: Wallet<D>,
}

impl<D: DeriveSpk> Deref for Runtime<D> {
    type Target = Wallet<D>;
    fn deref(&self) -> &Self::Target { &self.wallet }
}

impl<D: DeriveSpk> DerefMut for Runtime<D> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.wallet }
}

impl<D: DeriveSpk> Runtime<D> {
    pub fn new(descr: D, network: Chain) -> Self {
        Runtime {
            path: None,
            wallet: Wallet::new(descr, network),
        }
    }

    pub fn sync<I: Indexer>(&mut self, indexer: &I) -> Result<(), Vec<I::Error>> {
        self.wallet.update(indexer).into_result()
    }
}

impl<D: DeriveSpk> Runtime<D>
where for<'de> WalletDescr<D>: serde::Deserialize<'de>
{
    pub fn load(path: PathBuf) -> Result<Self, LoadError> {
        let mut descr_file = path.clone();
        descr_file.push("descriptor.toml");
        let descr = fs::read_to_string(descr_file)?;
        let descr = toml::from_str(&descr)?;

        // TODO: Load data and cache

        Ok(Runtime {
            path: Some(path),
            wallet: Wallet {
                descr,
                data: default!(),
                cache: none!(),
            },
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
}
