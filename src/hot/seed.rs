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

use std::path::Path;
use std::str::FromStr;
use std::{fs, io};

use bip39::Mnemonic;
use bpstd::{HardenedIndex, XkeyOrigin, Xpriv, XprivAccount};
use rand::RngCore;

use crate::bip43::DerivationStandard;
use crate::hot::{decrypt, encrypt, DataError, SecureIo};
use crate::Bip43;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[repr(u16)]
pub enum SeedType {
    Bit128 = 128,
    Bit160 = 160,
    Bit192 = 192,
    Bit224 = 224,
    Bit256 = 256,
}

impl SeedType {
    #[inline]
    pub fn bit_len(self) -> usize { self as usize }

    #[inline]
    pub fn byte_len(self) -> usize {
        match self {
            SeedType::Bit128 => 16,
            SeedType::Bit160 => 160 / 8,
            SeedType::Bit192 => 192 / 8,
            SeedType::Bit224 => 224 / 8,
            SeedType::Bit256 => 32,
        }
    }

    #[inline]
    pub fn word_len(self) -> usize {
        match self {
            SeedType::Bit128 => 12,
            SeedType::Bit160 => 15,
            SeedType::Bit192 => 18,
            SeedType::Bit224 => 21,
            SeedType::Bit256 => 24,
        }
    }
}

pub struct Seed(Box<[u8]>);

impl Seed {
    pub fn random(seed_type: SeedType) -> Seed {
        let mut entropy = vec![0u8; seed_type.byte_len()];
        rand::thread_rng().fill_bytes(&mut entropy);
        Seed(Box::from(entropy))
    }

    #[inline]
    pub fn as_entropy(&self) -> &[u8] { &self.0 }

    #[inline]
    pub fn master_xpriv(&self, testnet: bool) -> Xpriv {
        Xpriv::new_master(testnet, self.as_entropy())
    }

    pub fn derive(&self, scheme: Bip43, testnet: bool, account: HardenedIndex) -> XprivAccount {
        let master_xpriv = self.master_xpriv(testnet);
        let master_xpub = master_xpriv.to_xpub();
        let derivation = scheme.to_account_derivation(account, testnet);
        let account_xpriv = master_xpriv.derive_priv(&derivation);

        let origin = XkeyOrigin::new(master_xpub.fingerprint(), derivation);
        XprivAccount::new(account_xpriv, origin)
    }
}

impl SecureIo for Seed {
    fn read<P>(file: P, password: &str) -> Result<Self, DataError>
    where P: AsRef<Path> {
        let data = fs::read(file)?;
        let data = decrypt(&data, password)?;
        let s = String::from_utf8(data).map_err(|_| DataError::Password)?;
        let mnemonic = Mnemonic::from_str(&s).map_err(|_| DataError::Password)?;
        Ok(Seed(Box::from(mnemonic.to_entropy())))
    }

    fn write<P>(&self, file: P, password: &str) -> io::Result<()>
    where P: AsRef<Path> {
        fs::write(file, encrypt(self.0.to_vec(), password))
    }
}

impl SecureIo for XprivAccount {
    fn read<P>(file: P, password: &str) -> Result<Self, DataError>
    where P: AsRef<Path> {
        let data = fs::read(file)?;
        let data = decrypt(&data, password)?;
        let s = String::from_utf8(data).map_err(|_| DataError::Password)?;
        XprivAccount::from_str(&s).map_err(|_| DataError::Password)
    }

    fn write<P>(&self, file: P, password: &str) -> io::Result<()>
    where P: AsRef<Path> {
        fs::write(file, encrypt(self.to_string().into_bytes(), password))
    }
}
