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

use std::convert::Infallible;
use std::error;
use std::fmt::Debug;
use std::path::Path;

pub trait Layer2: Debug {
    type Descr: Layer2Descriptor<LoadError = Self::LoadError, StoreError = Self::StoreError>;
    type Data: Layer2Data<LoadError = Self::LoadError, StoreError = Self::StoreError>;
    type Cache: Layer2Cache<LoadError = Self::LoadError, StoreError = Self::StoreError>;
    type LoadError: error::Error;
    type StoreError: error::Error;

    fn load(path: &Path) -> Result<Self, Self::LoadError>
    where Self: Sized;
    fn store(&self, path: &Path) -> Result<(), Self::StoreError>;
}

pub trait Layer2Descriptor: Debug {
    type LoadError: error::Error;
    type StoreError: error::Error;

    fn load(path: &Path) -> Result<Self, Self::LoadError>
    where Self: Sized;
    fn store(&self, path: &Path) -> Result<(), Self::StoreError>;
}

pub trait Layer2Data: Debug + Default {
    type LoadError: error::Error;
    type StoreError: error::Error;

    fn load(path: &Path) -> Result<Self, Self::LoadError>
    where Self: Sized;
    fn store(&self, path: &Path) -> Result<(), Self::StoreError>;
}

pub trait Layer2Cache: Debug + Default {
    type LoadError: error::Error;
    type StoreError: error::Error;

    type Tx: Layer2Tx;
    type Coin: Layer2Coin;

    fn load(path: &Path) -> Result<Self, Self::LoadError>
    where Self: Sized;
    fn store(&self, path: &Path) -> Result<(), Self::StoreError>;
}

#[cfg(not(feature = "serde"))]
pub trait Layer2Tx: Debug + Default {}

#[cfg(feature = "serde")]
pub trait Layer2Tx:
    Clone + Debug + Default + serde::Serialize + for<'de> serde::Deserialize<'de>
{
}

#[cfg(not(feature = "serde"))]
pub trait Layer2Coin: Debug + Default {}

#[cfg(feature = "serde")]
pub trait Layer2Coin:
    Clone + Debug + Default + serde::Serialize + for<'de> serde::Deserialize<'de>
{
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate")
)]
pub enum ImpossibleLayer2 {}
pub type NoLayer2 = Option<ImpossibleLayer2>;

impl Layer2 for NoLayer2 {
    type Descr = NoLayer2;
    type Data = NoLayer2;
    type Cache = NoLayer2;
    type LoadError = Infallible;
    type StoreError = Infallible;

    fn load(_: &Path) -> Result<Self, Self::LoadError> { Ok(None) }
    fn store(&self, _: &Path) -> Result<(), Self::StoreError> { Ok(()) }
}

impl Layer2Descriptor for NoLayer2 {
    type LoadError = Infallible;
    type StoreError = Infallible;

    fn load(_: &Path) -> Result<Self, Self::LoadError> { Ok(None) }
    fn store(&self, _: &Path) -> Result<(), Self::StoreError> { Ok(()) }
}

impl Layer2Data for NoLayer2 {
    type LoadError = Infallible;
    type StoreError = Infallible;

    fn load(_: &Path) -> Result<Self, Self::LoadError> { Ok(None) }
    fn store(&self, _: &Path) -> Result<(), Self::StoreError> { Ok(()) }
}

impl Layer2Cache for NoLayer2 {
    type Tx = NoLayer2;
    type Coin = NoLayer2;

    type LoadError = Infallible;
    type StoreError = Infallible;

    fn load(_: &Path) -> Result<Self, Self::LoadError> { Ok(None) }
    fn store(&self, _: &Path) -> Result<(), Self::StoreError> { Ok(()) }
}

impl Layer2Tx for NoLayer2 {}
impl Layer2Coin for NoLayer2 {}
