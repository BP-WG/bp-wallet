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

use amplify::Wrapper;
use bpstd::secp256k1::{ecdsa, schnorr as bip340};
use bpstd::{
    Address, InternalKeypair, InternalPk, KeyOrigin, LegacyPk, Sats, Sighash, Sign, TapLeafHash,
    TapMerklePath, TapNodeHash, TapSighash, XOnlyPk, Xpriv, XprivAccount,
};
use descriptors::Descriptor;
use psbt::{Psbt, Rejected, Signer};

pub struct SignTxInfo {
    pub fee: Sats,
    pub inputs: Sats,
    pub beneficiaries: HashSet<Address, Sats>,
}

pub struct ConsoleSigner<'descr, 'me, D: Descriptor>
where Self: 'me
{
    pub descriptor: &'descr D,
    pub account: XprivAccount,
    pub signer: XprivSigner<'me>,
}

pub struct XprivSigner<'xpriv> {
    account: &'xpriv XprivAccount,
    // TODO: Support key- and script-path selection
}

impl<'descr, 'me, D: Descriptor> Signer for ConsoleSigner<'descr, 'me, D>
where Self: 'me
{
    type Sign<'s> = &'s XprivSigner<'s> where Self: 's + 'me;

    fn approve(&self, _psbt: &Psbt) -> Result<Self::Sign<'_>, Rejected> { Ok(&self.signer) }
}

impl<'xpriv> XprivSigner<'xpriv> {
    fn derive_subkey(&self, origin: Option<&KeyOrigin>) -> Option<Xpriv> {
        let origin = origin?;
        if !self.account.origin().is_subset_of(origin) {
            return None;
        }
        Some(
            self.account
                .xpriv()
                .derive_priv(&origin.derivation()[self.account.origin().derivation().len()..]),
        )
    }
}

impl<'a, 'xpriv> Sign for &'a XprivSigner<'xpriv> {
    fn sign_ecdsa(
        &self,
        message: Sighash,
        pk: LegacyPk,
        origin: Option<&KeyOrigin>,
    ) -> Option<ecdsa::Signature> {
        let sk = self.derive_subkey(origin)?;
        if sk.to_compr_pk().to_inner() != pk.pubkey {
            return None;
        }
        Some(sk.to_private_ecdsa().sign_ecdsa(message.into()))
    }

    fn sign_bip340_key_only(
        &self,
        message: TapSighash,
        pk: InternalPk,
        origin: Option<&KeyOrigin>,
        merkle_root: Option<TapNodeHash>,
    ) -> Option<bip340::Signature> {
        let xpriv = self.derive_subkey(origin)?;
        if xpriv.to_xonly_pk() != pk.to_xonly_pk() {
            return None;
        }
        let output_pair =
            InternalKeypair::from(xpriv.to_keypair_bip340()).to_output_keypair(merkle_root).0;
        if output_pair.x_only_public_key().0.serialize()
            != pk.to_output_pk(merkle_root).0.to_byte_array()
        {
            return None;
        }
        Some(output_pair.sign_schnorr(message.into()))
    }

    fn sign_bip340_script_path(
        &self,
        message: TapSighash,
        pk: XOnlyPk,
        origin: Option<&KeyOrigin>,
    ) -> Option<bip340::Signature> {
        let sk = self.derive_subkey(origin)?;
        if sk.to_xonly_pk() != pk {
            return None;
        }
        Some(sk.to_keypair_bip340().sign_schnorr(message.into()))
    }

    fn should_sign_script_path(
        &self,
        _index: usize,
        _merkle_path: &TapMerklePath,
        _leaf: TapLeafHash,
    ) -> bool {
        true
    }

    fn should_sign_key_path(&self, _index: usize) -> bool { true }
}
