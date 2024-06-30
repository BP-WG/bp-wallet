// Modern, minimalistic & standard-compliant hot wallet library.
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

use std::collections::HashSet;

use bpstd::secp256k1::ecdsa::Signature;
use bpstd::{
    Address, KeyOrigin, LegacyPk, Satisfy, Sats, Sighash, TapLeafHash, TapMerklePath, TapSighash,
    XOnlyPk, XkeyOrigin, Xpriv,
};
use descriptors::Descriptor;
use psbt::{Psbt, Rejected, Sign};

pub struct SignTxInfo {
    pub fee: Sats,
    pub inputs: Sats,
    pub beneficiaries: HashSet<Address, Sats>,
}

pub struct ConsoleSigner<'descr, 'me, D: Descriptor>
where Self: 'me
{
    descriptor: &'descr D,
    origin: XkeyOrigin,
    xpriv: Xpriv,
    satisfier: XprivSatisfier<'me>,
}

pub struct XprivSatisfier<'xpriv> {
    xpriv: &'xpriv Xpriv,
    // TODO: Support key- and script-path selection
}

impl<'descr, 'me, D: Descriptor> Sign for ConsoleSigner<'descr, 'me, D>
where Self: 'me
{
    type Satisfier<'s> = &'s XprivSatisfier<'s> where Self: 's + 'me;

    fn approve(&self, _psbt: &Psbt) -> Result<Self::Satisfier<'_>, Rejected> { Ok(&self.satisfier) }
}

impl<'a, 'xpriv> Satisfy for &'a XprivSatisfier<'xpriv> {
    fn signature_ecdsa(
        &self,
        message: Sighash,
        pk: LegacyPk,
        origin: Option<&KeyOrigin>,
    ) -> Option<Signature> {
        todo!()
    }

    fn signature_bip340(
        &self,
        message: TapSighash,
        pk: XOnlyPk,
        origin: Option<&KeyOrigin>,
    ) -> Option<bpstd::secp256k1::schnorr::Signature> {
        todo!()
    }

    fn should_satisfy_script_path(
        &self,
        index: usize,
        merkle_path: &TapMerklePath,
        leaf: TapLeafHash,
    ) -> bool {
        todo!()
    }

    fn should_satisfy_key_path(&self, index: usize) -> bool { todo!() }
}
