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

use nonasync::persistence::{CloneNoPersistence, Persistence, Persisting};

pub trait Layer2: Debug + CloneNoPersistence + Persisting {
    type Descr: Layer2Descriptor<LoadError = Self::LoadError, StoreError = Self::StoreError>;
    type Data: Layer2Data<LoadError = Self::LoadError, StoreError = Self::StoreError>;
    type Cache: Layer2Cache<LoadError = Self::LoadError, StoreError = Self::StoreError>;
    type LoadError: error::Error;
    type StoreError: error::Error;
}

pub trait Layer2Descriptor: Debug + CloneNoPersistence {
    type LoadError: error::Error;
    type StoreError: error::Error;
}

pub trait Layer2Data: Debug + CloneNoPersistence + Default {
    type LoadError: error::Error;
    type StoreError: error::Error;
}

pub trait Layer2Cache: Debug + CloneNoPersistence + Default {
    type LoadError: error::Error;
    type StoreError: error::Error;

    type Tx: Layer2Tx;
    type Coin: Layer2Coin;
}

#[cfg(not(feature = "serde"))]
pub trait Layer2Tx: Debug + Default {}

#[cfg(feature = "serde")]
pub trait Layer2Tx: Debug + Default + serde::Serialize + for<'de> serde::Deserialize<'de> {}

#[cfg(not(feature = "serde"))]
pub trait Layer2Coin: Debug + Default {}

#[cfg(feature = "serde")]
pub trait Layer2Coin:
    Debug + Default + serde::Serialize + for<'de> serde::Deserialize<'de>
{
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate")
)]
pub struct Layer2Empty;

#[derive(Debug, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(crate = "serde_crate")
)]
pub struct NoLayer2 {
    #[cfg_attr(feature = "serde", serde(skip))]
    persistence: Option<Persistence<Self>>,
}

impl CloneNoPersistence for NoLayer2 {
    fn clone_no_persistence(&self) -> Self { none!() }
}

impl Persisting for NoLayer2 {
    #[inline]
    fn persistence(&self) -> Option<&Persistence<Self>> { self.persistence.as_ref() }
    #[inline]
    fn persistence_mut(&mut self) -> Option<&mut Persistence<Self>> { self.persistence.as_mut() }
    #[inline]
    fn as_mut_persistence(&mut self) -> &mut Option<Persistence<Self>> { &mut self.persistence }
}

impl Layer2 for NoLayer2 {
    type Descr = NoLayer2;
    type Data = NoLayer2;
    type Cache = NoLayer2;
    type LoadError = Infallible;
    type StoreError = Infallible;
}

impl Layer2Descriptor for NoLayer2 {
    type LoadError = Infallible;
    type StoreError = Infallible;
}

impl Layer2Data for NoLayer2 {
    type LoadError = Infallible;
    type StoreError = Infallible;
}

impl Layer2Cache for NoLayer2 {
    type Tx = Layer2Empty;
    type Coin = Layer2Empty;

    type LoadError = Infallible;
    type StoreError = Infallible;
}

impl Layer2Tx for Layer2Empty {}
impl Layer2Coin for Layer2Empty {}
