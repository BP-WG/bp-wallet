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

use std::env::VarError;
use std::path::{Path, PathBuf};
use std::{env, fs};

use amplify::hex::ToHex;
use amplify::{Display, IoError};
use bip39::Mnemonic;
use bpstd::signers::TestnetRefSigner;
use bpstd::{HardenedIndex, SighashCache, Tx, XprivAccount};
use clap::Subcommand;
use colored::Colorize;
use psbt::Psbt;

use crate::hot::{calculate_entropy, DataError, SecureIo, Seed, SeedType};
use crate::Bip43;

const SEED_PASSWORD_ENVVAR: &str = "SEED_PASSWORD";

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct HotArgs {
    /// Set verbosity level
    ///
    /// Can be used multiple times to increase verbosity
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Command to execute
    #[clap(subcommand)]
    pub command: HotCommand,
}

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
pub enum HotCommand {
    /// Generate new seed and saves it as an encoded file. The password can be provided via the
    /// `SEED_PASSWORD` environment variable (security warning: don't set it on the command line,
    /// use instead the shell's builtin `read` and then export it).
    #[display("seed")]
    Seed {
        /// File to save generated seed data and extended master key
        output_file: PathBuf,
    },

    /// Derive new extended private key from the seed and saves it into a separate file as a new
    /// signing account. The seed password can be provided via the `SEED_PASSWORD` environment
    /// variable (security warning: don't set it on the command line, use instead the shell's
    /// builtin `read` and then export it).
    #[display("derive")]
    Derive {
        /// Do not ask for a password and default to an empty-line password. For testing purposes
        /// only
        #[clap(short = 'N', long, conflicts_with = "mainnet")]
        no_password: bool,

        /// Seed file containing extended master key, created previously with `seed` command
        seed_file: PathBuf,

        /// Derivation scheme.
        #[clap(short, long, default_value = "bip86")]
        scheme: Bip43,

        /// Account derivation number (should be hardened, i.e. with `h` suffix)
        #[clap(short, long, default_value = "0h")]
        account: HardenedIndex,

        /// Use the seed for bitcoin mainnet
        #[clap(long)]
        mainnet: bool,

        /// Output file for storing account-based extended private key
        output_file: PathBuf,
    },

    /// Print information about seed or the signing account
    #[display("info")]
    Info {
        /// File containing either seed information or extended private key for the account,
        /// previously created with `seed` and `derive` commands
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

    /// Analyze PSBT and print debug signing information
    #[display("sighash")]
    Sighash {
        /// File containing PSBT
        psbt_file: PathBuf,
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
            HotCommand::Sign {
                no_password,
                psbt_file,
                signing_account,
            } => sign(&psbt_file, &signing_account, no_password)?,
            HotCommand::Sighash { psbt_file } => sighash(&psbt_file)?,
        };
        Ok(())
    }
}

fn get_password(
    password_envvar: Option<&str>,
    prompt: &str,
    accept_weak: bool,
) -> Result<String, std::io::Error> {
    let password = loop {
        let password = if let Some(varname) = password_envvar {
            match env::var(varname) {
                Ok(password) => return Ok(password),
                Err(VarError::NotUnicode(_)) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "password set by environment is not a valid unicode string",
                    ));
                }
                Err(VarError::NotPresent) => None,
            }
        } else {
            None
        };
        let password =
            if let Some(pass) = password { pass } else { rpassword::prompt_password(prompt)? };

        let entropy = calculate_entropy(&password);
        eprintln!("Password entropy: ~{entropy:.0} bits");
        if !accept_weak && (password.is_empty() || entropy < 64.0) {
            eprintln!("Entropy is too low, please try with a different password");
            if password_envvar.is_some() {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "low password entropy"));
            } else {
                continue;
            }
        }

        if password_envvar.is_none() {
            let repeat = rpassword::prompt_password("Repeat the password: ")?;
            if repeat != password {
                eprintln!("Passwords do not match, please try again");
                continue;
            }
        }
        break password;
    };
    Ok(password)
}

fn seed(output_file: &Path) -> Result<(), DataError> {
    let seed = Seed::random(SeedType::Bit128);
    let seed_password = get_password(Some(SEED_PASSWORD_ENVVAR), "Seed password:", false)?;

    seed.write(output_file, &seed_password)?;
    Seed::read(output_file, &seed_password).inspect_err(|_| {
        eprintln!("Unable to save seed file");
        let _ = fs::remove_file(output_file);
    })?;

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
    let seed_password = get_password(Some(SEED_PASSWORD_ENVVAR), "Seed password:", false)?;

    let account_password = if !mainnet && no_password {
        s!("")
    } else {
        get_password(None, "Account password:", !mainnet)?
    };

    let seed = Seed::read(seed_file, &seed_password)?;
    let account = seed.derive(scheme, !mainnet, account);

    account.write(output_file, &account_password)?;
    XprivAccount::read(output_file, &account_password).inspect_err(|_| {
        eprintln!("Unable to save account file");
        let _ = fs::remove_file(output_file);
    })?;

    info_account(account, false);

    Ok(())
}

fn sign(psbt_file: &Path, account_file: &Path, no_password: bool) -> Result<(), DataError> {
    eprintln!("Signing {} with {}", psbt_file.display(), account_file.display());
    let password = if no_password { s!("") } else { rpassword::prompt_password("Password: ")? };
    let account = XprivAccount::read(account_file, &password)?;

    eprintln!("Signing key: {}", account.to_xpub_account());
    eprintln!("Signing using testnet signer");

    let data = fs::read(psbt_file)?;
    let mut psbt = Psbt::deserialize(&data)?;

    eprintln!("PSBT version: {:#}", psbt.version);
    eprintln!("Transaction id: {}", psbt.txid());

    let signer = TestnetRefSigner::new(&account);
    let sig_count = psbt.sign(&signer)?;

    fs::write(psbt_file, psbt.serialize(psbt.version))?;
    eprintln!(
        "Done {} signatures, saved to {}\n",
        sig_count.to_string().bright_green(),
        psbt_file.display()
    );
    println!("\n{}\n", psbt);
    Ok(())
}

fn sighash(psbt_file: &Path) -> Result<(), DataError> {
    let data = fs::read(psbt_file)?;
    let psbt = Psbt::deserialize(&data)?;

    let tx = psbt.to_unsigned_tx();
    let txid = tx.txid();
    let prevouts = psbt.inputs().map(psbt::Input::prev_txout).cloned().collect::<Vec<_>>();
    let mut sig_hasher = SighashCache::new(Tx::from(tx), prevouts)
        .expect("inputs and prevouts match algorithmically");
    println!(
        "PSBT contains transaction with id {} and {} inputs",
        txid.to_string().bright_green(),
        psbt.inputs().count()
    );
    println!("Input #\tSig type\tSighash algo\tSighash type\tSighash\t\t\t\t\t\t\t\t\tScript code");
    for input in psbt.inputs() {
        let (ty, algo) = match (input.is_bip340(), input.is_segwit_v0()) {
            (true, _) => ("BIP340", "Taproot"),
            (false, true) => ("ECDSA", "SegWitV0"),
            (false, false) => ("ECDSA", "Legacy"),
        };
        let sighash_type = match input.sighash_type {
            None if input.is_bip340() => s!("DEFAULT"),
            None => s!("unspecified (assumed ALL)"),
            Some(sighash_type) => sighash_type.to_string(),
        };
        print!("{}\t{}\t\t{}\t\t{}\t\t", input.index() + 1, ty, algo, sighash_type);

        if input.is_bip340() {
            match sig_hasher.tap_sighash_key(input.index(), input.sighash_type) {
                Ok(sighash) => println!("{sighash}\tn/a"),
                Err(e) => println!("{e}"),
            }
        } else if input.is_segwit_v0() {
            let Some(script_code) = input.script_code() else {
                println!("no witness script is given, which is required to compute script code");
                continue;
            };
            match sig_hasher.segwit_sighash(
                input.index(),
                &script_code,
                input.value(),
                input.sighash_type.unwrap_or_default(),
            ) {
                Ok(sighash) => println!("{sighash}\t"),
                Err(e) => println!("{e}"),
            }
            println!("{}", script_code.to_hex());
        } else {
            match sig_hasher.legacy_sighash(
                input.index(),
                &input.prev_txout().script_pubkey,
                input.sighash_type.unwrap_or_default().to_consensus_u32(),
            ) {
                Ok(sighash) => println!("{sighash}\tn/a"),
                Err(e) => println!("{e}"),
            }
        }
    }

    Ok(())
}
