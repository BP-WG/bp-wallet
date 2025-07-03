// Modern, minimalistic & standard-compliant hot wallet library.
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

use std::path::Path;
use std::{fs, io};

impl SecureIo for Seed {
    fn read<P>(file: P, password: &str) -> Result<Self, DataError>
    where P: AsRef<Path> {
        let data = fs::read(file)?;
        let data = decrypt(&data, password).map_err(|_| DataError::SeedPassword)?;
        let s = String::from_utf8(data).map_err(|_| DataError::SeedPassword)?;
        let mnemonic = Mnemonic::from_str(&s).map_err(|_| DataError::SeedPassword)?;
        Ok(Seed(Box::from(mnemonic.to_entropy())))
    }

    fn write<P>(&self, file: P, password: &str) -> io::Result<()>
    where P: AsRef<Path> {
        let mnemonic = Mnemonic::from_entropy(&self.0).expect("mnemonic generator is broken");
        fs::write(file, encrypt(mnemonic.to_string().into_bytes(), password))
    }
}

impl SecureIo for XprivAccount {
    fn read<P>(file: P, password: &str) -> Result<Self, DataError>
    where P: AsRef<Path> {
        let data = fs::read(file)?;
        let data = decrypt(&data, password).map_err(|_| DataError::AccountPassword)?;
        let s = String::from_utf8(data).map_err(|_| DataError::AccountPassword)?;
        XprivAccount::from_str(&s).map_err(|_| DataError::AccountPassword)
    }

    fn write<P>(&self, file: P, password: &str) -> io::Result<()>
    where P: AsRef<Path> {
        fs::write(file, encrypt(self.to_string().into_bytes(), password))
    }
}
