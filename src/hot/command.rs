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

use std::path::{Path, PathBuf};

use amplify::{Display, IoError};
use bip39::Mnemonic;
use bpstd::{HardenedIndex, XprivAccount};
use clap::Subcommand;
use colored::Colorize;

use crate::hot::{calculate_entropy, DataError, SecureIo, Seed, SeedType};
use crate::Bip43;

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct HotArgs {
    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity.
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Command to execute.
    #[clap(subcommand)]
    pub command: HotCommand,
}

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
pub enum HotCommand {
    /// Generate new seed and saves it as an encoded file
    #[display("seed")]
    Seed {
        /// File to save generated seed data and extended master key
        output_file: PathBuf,
    },

    /// Derive new extended private key from the seed and saves it into a separate file as a new
    /// signing account
    #[display("derive")]
    Derive {
        /// Do not ask for a password and default to an empty-line password. For testing purposes
        /// only.
        #[clap(short = 'N', long, conflicts_with = "mainnet")]
        no_password: bool,

        /// Seed file containing extended master key, created previously with `seed` command.
        seed_file: PathBuf,

        /// Derivation scheme.
        #[clap(
            short,
            long,
            long_help = "Possible values are:
- bip44: used for P2PKH (not recommended)
- bip84: used for P2WPKH
- bip49: used for P2WPKH-in-P2SH
- bip86: used for P2TR with single key (no MuSig, no multisig)
- bip45: used for legacy multisigs (P2SH, not recommended)
- bip48//1h: used for P2WSH-in-P2SH multisigs (deterministic order)
- bip48//2h: used for P2WSH multisigs (deterministic order)
- bip87: used for modern multisigs with descriptors (pre-MuSig)
- bip43/<purpose>h: any other non-standard purpose field",
            default_value = "bip86"
        )]
        scheme: Bip43,

        /// Account derivation number (should be hardened, i.e. with `h` or `'` suffix).
        #[clap(short, long, default_value = "0'")]
        account: HardenedIndex,

        /// Use the seed for bitcoin mainnet
        #[clap(long)]
        mainnet: bool,

        /// Output file for storing account-based extended private key
        output_file: PathBuf,
    },

    /// Print information about seed or the signing account.
    #[display("info")]
    Info {
        /// File containing either seed information or extended private key for the account,
        /// previously created with `seed` and `derive` commands.
        file: PathBuf,

        /// Print private information, including mnemonic, extended private keys and
        /// signatures
        #[clap(short = 'P', long)]
        print_private: bool,
    },

    /// Sign PSBT with the provided account keys
    #[display("sign")]
    Sign {
        /// Do not ask for a password and default to an empty-line password. For testing purposes
        /// only.
        #[clap(short = 'N', long)]
        no_password: bool,

        /// File containing PSBT
        psbt_file: PathBuf,

        /// Signing account file used to (partially co-)sign PSBT
        signing_account: PathBuf,
    },
}

impl HotArgs {
    pub fn exec(self) -> Result<(), DataError> {
        match self.command {
            HotCommand::Seed { output_file } => seed(&output_file)?,
            HotCommand::Derive {
                no_password,
                seed_file,
                scheme,
                account,
                mainnet,
                output_file,
            } => derive(&seed_file, scheme, account, mainnet, &output_file, no_password)?,
            HotCommand::Info {
                file,
                print_private,
            } => info(&file, print_private)?,
            HotCommand::Sign { .. } => {
                todo!()
            }
        };
        Ok(())
    }
}

fn seed(output_file: &Path) -> Result<(), IoError> {
    let seed = Seed::random(SeedType::Bit128);
    let seed_password = loop {
        let seed_password = rpassword::prompt_password("Seed password: ")?;
        let entropy = calculate_entropy(&seed_password);
        eprintln!("Password entropy: ~{entropy:.0} bits");
        if !seed_password.is_empty() && entropy >= 64.0 {
            break seed_password;
        }
        eprintln!("Entropy is too low, please try with a different password")
    };

    seed.write(output_file, &seed_password)?;

    info_seed(seed, false);

    Ok(())
}

fn info(file: &Path, print_private: bool) -> Result<(), IoError> {
    let password = rpassword::prompt_password("File password: ")?;
    if let Ok(seed) = Seed::read(file, &password) {
        info_seed(seed, print_private)
    } else if let Ok(account) = XprivAccount::read(file, &password) {
        info_account(account, print_private)
    } else {
        eprintln!("{} can't detect file format for `{}`", "Error:".bright_red(), file.display());
    }
    Ok(())
}

fn info_seed(seed: Seed, print_private: bool) {
    if print_private {
        let mnemonic = Mnemonic::from_entropy(seed.as_entropy()).expect("invalid seed");
        println!("\n{:-18} {}", "Mnemonic:".bright_white(), mnemonic.to_string().black().dimmed());
    }

    let xpriv = seed.master_xpriv(false);
    let xpub = xpriv.to_xpub();

    println!("{}", "Master key:".bright_white());
    println!(
        "{:-18} {}",
        "  - fingerprint:".bright_white(),
        xpub.fingerprint().to_string().bright_green()
    );
    println!("{:-18} {}", "  - mainnet:", if xpub.is_testnet() { "no" } else { "yes" });
    println!("{:-18} {}", "  - id:".bright_white(), xpub.identifier());
    if print_private {
        println!("{:-18} {}", "  - xprv:".bright_white(), xpriv.to_string().black().dimmed());
    }
    println!("{:-18} {}", "  - xpub:".bright_white(), xpub.to_string().bright_green());
}

fn info_account(account: XprivAccount, print_private: bool) {
    let xpub = account.to_xpub_account();
    println!("\n{} {}", "Account:".bright_white(), xpub);
    println!(
        "{:-18} {}",
        "  - fingerprint:".bright_white(),
        xpub.account_fp().to_string().bright_green()
    );
    println!("{:-18} {}", "  - id:".bright_white(), xpub.account_id());
    println!("{:-18} [{}]", "  - key origin:".bright_white(), xpub.origin(),);
    if print_private {
        let account_xpriv = account.xpriv();
        println!(
            "{:-18} {}",
            "  - xpriv:".bright_white(),
            account_xpriv.to_string().black().dimmed()
        );
        // TODO: Add Zpriv etc
    }
    println!("{:-18} {}", "  - xpub:".bright_white(), xpub.to_string().bright_green());
    // TODO: Add Zpub etc
}

fn derive(
    seed_file: &Path,
    scheme: Bip43,
    account: HardenedIndex,
    mainnet: bool,
    output_file: &Path,
    no_password: bool,
) -> Result<(), DataError> {
    let seed_password = rpassword::prompt_password("Seed password: ")?;

    let account_password = if !mainnet && no_password {
        s!("")
    } else {
        loop {
            let account_password = rpassword::prompt_password("Account password: ")?;
            let entropy = calculate_entropy(&seed_password);
            eprintln!("Password entropy: ~{entropy:.0} bits");
            if !account_password.is_empty() && entropy >= 64.0 {
                break account_password;
            }
            if !mainnet {
                eprintln!("Entropy is too low, but since we are on testnet we accept that");
                break account_password;
            }
            eprintln!("Entropy is too low, please try with a different password")
        }
    };

    let seed = Seed::read(seed_file, &seed_password)?;
    let account = seed.derive(scheme, !mainnet, account);

    account.write(output_file, &account_password)?;

    info_account(account, false);

    Ok(())
}
