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

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use amplify::confinement::Confined;
use amplify::num::u4;
use bpstd::{Network, XpubDerivable};
use clap::error::ErrorKind;
use clap::ValueHint;
use descriptors::{
    Descriptor, Pkh, ShMulti, ShSortedMulti, StdDescr, TrKey, TrMulti, TrSortedMulti, Wpkh,
    WshMulti, WshSortedMulti,
};
use strict_encoding::Ident;

pub const BP_DATA_DIR_ENV: &str = "BP_DATA_DIR";
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub const BP_DATA_DIR: &str = "~/.local/share/bp";
#[cfg(target_os = "macos")]
pub const BP_DATA_DIR: &str = "~/Library/Application Support/BP Wallet";
#[cfg(target_os = "windows")]
pub const BP_DATA_DIR: &str = "~\\AppData\\Local\\BP Wallet";
#[cfg(target_os = "ios")]
pub const BP_DATA_DIR: &str = "~/Documents";
#[cfg(target_os = "android")]
pub const BP_DATA_DIR: &str = ".";

// Uses XDG_DATA_HOME if set, otherwise falls back to RGB_DATA_DIR
fn default_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("bp")
    } else {
        PathBuf::from(BP_DATA_DIR)
    }
}

pub const DEFAULT_ELECTRUM: &str = "example.com:50001";
pub const DEFAULT_ESPLORA: &str = "https://blockstream.info/{network}/api";
pub const DEFAULT_MEMPOOL: &str = "https://mempool.space/{network}/api";

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group(args = ["electrum", "esplora", "mempool"])]
pub struct ResolverOpt {
    /// Electrum server to use
    #[arg(
        long,
        global = true,
        default_missing_value = DEFAULT_ELECTRUM,
        num_args = 0..=1,
        require_equals = true,
        env = "ELECRTUM_SERVER",
        value_hint = ValueHint::Url,
        value_name = "URL"
    )]
    pub electrum: Option<String>,

    /// Esplora server to use
    #[arg(
        long,
        global = true,
        default_missing_value = DEFAULT_ESPLORA,
        num_args = 0..=1,
        require_equals = true,
        env = "ESPLORA_SERVER",
        value_hint = ValueHint::Url,
        value_name = "URL"
    )]
    pub esplora: Option<String>,

    /// Mempool server to use
    #[arg(
        long,
        global = true,
        default_missing_value = DEFAULT_MEMPOOL,
        num_args = 0..=1,
        require_equals = true,
        env = "MEMPOOL_SERVER",
        value_hint = ValueHint::Url,
        value_name = "URL"
    )]
    pub mempool: Option<String>,
}

pub trait DescriptorOpts: clap::Args + Clone + Eq + Debug {
    type Descr: Descriptor + serde::Serialize + for<'de> serde::Deserialize<'de>;
    fn is_some(&self) -> bool;
    fn descriptor(
        &self,
        keys: &[XpubDerivable],
        internal_key: &Option<XpubDerivable>,
        command: clap::Command,
    ) -> Option<Self::Descr>;
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group(id = "descr", multiple = false, requires = "key")]
pub struct DescrStdOpts {
    /// Use pkh(key) descriptor as wallet
    #[arg(long, global = true)]
    pub pkh: bool,

    /// Use sh(multi(threshold, key, ...)) descriptor as wallet
    #[arg(long, global = true)]
    pub sh_multi: Option<u4>,

    /// Use sh(sortedmulti(threshold, key, ...)) descriptor as wallet
    #[arg(long, global = true)]
    pub sh_sorted_multi: Option<u4>,

    /// Use wpkh(key) descriptor as wallet
    #[arg(long, global = true)]
    pub wpkh: bool,

    /// Use wsh(multi(threshold, key, ...)) descriptor as wallet
    #[arg(long, global = true)]
    pub wsh_multi: Option<u4>,

    /// Use wsh(sortedmulti(threshold, key, ...)) descriptor as wallet
    #[arg(long, global = true)]
    pub wsh_sorted_multi: Option<u4>,

    /// Use tr(key) descriptor as wallet
    #[arg(long, global = true)]
    pub tr_key_only: bool,

    /// Use tr(unspendable, multi_a(threshold, key, ...)) descriptor as wallet
    #[arg(long, global = true, requires = "internal_key")]
    pub tr_multi: Option<u16>,

    /// Use tr(unspendable, sortedmulti_a(threshold, key, ...)) descriptor as wallet
    #[arg(long, global = true, requires = "internal_key")]
    pub tr_sorted_multi: Option<u16>,
}

impl DescrStdOpts {
    pub fn required_key_count(&self) -> usize {
        if self.pkh || self.wpkh || self.tr_key_only {
            1
        } else if let Some(threshold) = self.sh_multi {
            threshold.into_u8() as usize
        } else if let Some(threshold) = self.sh_sorted_multi {
            threshold.into_u8() as usize
        } else if let Some(threshold) = self.wsh_multi {
            threshold.into_u8() as usize
        } else if let Some(threshold) = self.wsh_sorted_multi {
            threshold.into_u8() as usize
        } else if let Some(threshold) = self.tr_multi {
            threshold as usize
        } else if let Some(threshold) = self.tr_sorted_multi {
            threshold as usize
        } else {
            0
        }
    }
}

impl DescriptorOpts for DescrStdOpts {
    type Descr = StdDescr;

    fn is_some(&self) -> bool { self.required_key_count() > 0 }
    fn descriptor(
        &self,
        keys: &[XpubDerivable],
        internal_key: &Option<XpubDerivable>,
        mut command: clap::Command,
    ) -> Option<Self::Descr> {
        let required_key_count = self.required_key_count();
        if required_key_count > keys.len() {
            command
                .error(
                    ErrorKind::MissingRequiredArgument,
                    format!(
                        "the selected wallet descriptor require at least {required_key_count} \
                         keys, which must be provided using `--key` argument",
                    ),
                )
                .exit();
        }

        let mut confine_keys = || {
            Confined::try_from(keys.to_vec()).unwrap_or_else(|_| {
                command
                    .error(ErrorKind::MissingRequiredArgument, "too many key key arguments")
                    .exit();
            })
        };

        if self.pkh {
            Some(Pkh::from(keys.first().cloned().expect("at least one key is required")).into())
        } else if self.wpkh {
            Some(Wpkh::from(keys.first().cloned().expect("at least one key is required")).into())
        } else if self.tr_key_only {
            Some(TrKey::from(keys.first().cloned().expect("at least one key is required")).into())
        } else if let Some(threshold) = self.sh_multi {
            Some(
                ShMulti {
                    threshold,
                    keys: confine_keys(),
                }
                .into(),
            )
        } else if let Some(threshold) = self.sh_sorted_multi {
            Some(
                ShSortedMulti {
                    threshold,
                    keys: confine_keys(),
                }
                .into(),
            )
        } else if let Some(threshold) = self.wsh_multi {
            Some(
                WshMulti {
                    threshold,
                    keys: confine_keys(),
                }
                .into(),
            )
        } else if let Some(threshold) = self.wsh_sorted_multi {
            Some(
                WshSortedMulti {
                    threshold,
                    keys: confine_keys(),
                }
                .into(),
            )
        } else {
            // We need this because of the rust borrower checker
            let mut tr_script_keys = || {
                Confined::try_from(keys.to_vec()).unwrap_or_else(|_| {
                    command
                        .error(ErrorKind::MissingRequiredArgument, "too many key key arguments")
                        .exit();
                })
            };
            let internal_key = internal_key.clone().expect("internal ey is required by clap");
            if let Some(threshold) = self.tr_multi {
                Some(
                    TrMulti {
                        threshold,
                        script_keys: tr_script_keys(),
                        internal_key,
                    }
                    .into(),
                )
            } else if let Some(threshold) = self.tr_sorted_multi {
                Some(
                    TrSortedMulti {
                        threshold,
                        script_keys: tr_script_keys(),
                        internal_key,
                    }
                    .into(),
                )
            } else {
                None
            }
        }
    }
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group(multiple = false)]
pub struct WalletOpts<O: DescriptorOpts = DescrStdOpts> {
    /// Use specific named wallet
    #[arg(short = 'w', long = "wallet", global = true)]
    pub name: Option<Ident>,

    /// Use wallet from a given path
    #[arg(
        short = 'W',
        long,
        global = true,
        value_hint = ValueHint::DirPath,
    )]
    pub wallet_path: Option<PathBuf>,

    #[command(flatten)]
    pub descriptor_opts: O,

    /// A xpub with full derivation path, which should be added to the descriptor
    #[arg(long, global = true)]
    pub key: Vec<XpubDerivable>,

    /// A xpub with a full derivation path for taproot-based descriptors
    #[arg(long, global = true)]
    pub internal_key: Option<XpubDerivable>,
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct GeneralOpts {
    /// Data directory path
    ///
    /// Path to the directory that contains RGB stored data.
    #[arg(
        short,
        long,
        global = true,
        default_value_os_t = default_data_dir(),
        env = BP_DATA_DIR_ENV,
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// Network to use
    #[arg(short, long, global = true, default_value = "testnet3", env = "LNPBP_NETWORK")]
    pub network: Network,

    /// Do not add network prefix to the `--data-dir`
    #[arg(long = "no-network-prefix", global = true)]
    pub no_prefix: bool,
}

impl GeneralOpts {
    pub fn process(&mut self) {
        self.data_dir =
            PathBuf::from(shellexpand::tilde(&self.data_dir.display().to_string()).to_string());
    }

    pub fn base_dir(&self) -> PathBuf {
        let mut dir = self.data_dir.clone();
        if !self.no_prefix {
            dir.push(self.network.to_string());
        }
        dir
    }

    pub fn wallet_dir(&self, wallet_name: impl AsRef<Path>) -> PathBuf {
        let mut dir = self.base_dir();
        dir.push(wallet_name);
        dir
    }
}
