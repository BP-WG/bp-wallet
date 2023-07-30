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

use bp::{Chain, DeriveSpk, DescriptorStd, Wallet};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum RuntimeError {
    #[from]
    Io(io::Error),

    #[from]
    Custom(String),
}

#[derive(Getters, Debug)]
pub struct Runtime<D: DeriveSpk = DescriptorStd, L2: Default = ()> {
    path: Option<PathBuf>,
    wallet: Wallet<D, L2>,
}

impl<D: DeriveSpk, L2: Default> Deref for Runtime<D, L2> {
    type Target = Wallet<D, L2>;
    fn deref(&self) -> &Self::Target { &self.wallet }
}

impl<D: DeriveSpk, L2: Default> DerefMut for Runtime<D, L2> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.wallet }
}

impl<D: DeriveSpk, L2: Default> Runtime<D, L2> {
    pub fn new(descr: D, network: Chain) -> Self {
        Runtime {
            path: None,
            wallet: Wallet::new(descr, network),
        }
    }

    pub fn load(path: PathBuf) -> Result<Self, RuntimeError> { todo!() }
}
