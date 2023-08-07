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

use bp::{Chain, DeriveSpk, DescriptorStd};

use crate::{Indexer, Wallet};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum LoadError {
    #[from]
    Io(io::Error),

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

    pub fn load(_path: PathBuf) -> Result<Self, LoadError> {
        Err(LoadError::Custom(s!("not implemented")))
        // TODO: implement wallet loading
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

    pub fn sync<I: Indexer>(&mut self, indexer: &I) -> Result<(), Vec<I::Error>> {
        self.wallet.update(indexer).into_result()
    }
}
