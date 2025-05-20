// Convertor between bp-wallet and rust-bitcoin data types.
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

use amplify::ByteArray;
use bitcoin::hashes::Hash;

pub trait Convertible {
    type Target: Sized;
    fn convert(&self) -> Self::Target;
}

impl Convertible for bpstd::Txid {
    type Target = bitcoin::Txid;
    fn convert(&self) -> Self::Target { Self::Target::from_byte_array(self.to_byte_array()) }
}

impl Convertible for bitcoin::Txid {
    type Target = bpstd::Txid;
    fn convert(&self) -> Self::Target { Self::Target::from_byte_array(self.to_byte_array()) }
}
