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

mod seed;
#[cfg(feature = "cli")]
mod command;
#[cfg(feature = "cli")]
pub mod signer;
mod password;

#[cfg(feature = "cli")]
pub use command::{HotArgs, HotCommand};
pub use io::{decrypt, encrypt, DataError, SecureIo};
pub use password::calculate_entropy;
pub use seed::{Seed, SeedType};

mod io {
    use std::io;
    use std::path::Path;

    use aes_gcm::aead::{Aead, Nonce, OsRng};
    use aes_gcm::{AeadCore, Aes256Gcm, KeyInit};
    use amplify::IoError;
    use psbt::{PsbtError, SignError};
    use sha2::{Digest, Sha256};

    pub fn encrypt(source: Vec<u8>, key: impl AsRef<[u8]>) -> Vec<u8> {
        let key = Sha256::digest(key.as_ref());
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(key.as_slice());

        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let cipher = Aes256Gcm::new(key);

        let ciphered_data = cipher.encrypt(&nonce, source.as_ref()).expect("failed to encrypt");
        debug_assert_eq!(Aes256Gcm::new(key).decrypt(&nonce, &ciphered_data[..]), Ok(source));

        let mut data = nonce.to_vec();
        data.extend(ciphered_data);
        data
    }

    pub fn decrypt(encrypted: &[u8], key: impl AsRef<[u8]>) -> Result<Vec<u8>, aes_gcm::Error> {
        let key = Sha256::digest(key.as_ref());
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(key.as_slice());
        let nonce = Nonce::<Aes256Gcm>::from_slice(&encrypted[..12]);
        Aes256Gcm::new(key).decrypt(nonce, &encrypted[12..])
    }

    #[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
    #[display(inner)]
    pub enum DataError {
        #[from]
        #[from(io::Error)]
        Io(IoError),

        #[display("invalid seed password.")]
        SeedPassword,

        #[display("invalid account key password.")]
        AccountPassword,

        #[from]
        Psbt(PsbtError),

        #[from]
        Sign(SignError),
    }

    pub trait SecureIo {
        fn read<P>(file: P, password: &str) -> Result<Self, DataError>
        where
            P: AsRef<Path>,
            Self: Sized;

        fn write<P>(&self, file: P, password: &str) -> io::Result<()>
        where P: AsRef<Path>;
    }
}
