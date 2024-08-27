use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use bpstd::{BlockHash, DerivedAddr, Keychain, Tx, Txid};
use electrum::GetHistoryRes;
use lru::LruCache;

use super::electrum::ElectrumError;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TxDetail {
    pub(crate) inner: Tx,
    pub(crate) blockhash: Option<BlockHash>,
    pub(crate) blocktime: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct IndexerCache {
    // The cache is designed as Arc to ensure that
    // all Indexers globally use the same cache
    // (to maintain data consistency).
    //
    // The Mutex is used to ensure that the cache is thread-safe
    // Make sure to get the updated cache from immutable references
    // In the create/update processing logic of &Indexer
    // addr_transactions: for esplora
    pub(crate) addr_transactions: Arc<Mutex<LruCache<DerivedAddr, Vec<esplora::Tx>>>>,
    // TODO: WalletDescr::unique_id
    #[allow(dead_code)]
    pub(crate) wallet_addresses: Arc<Mutex<LruCache<String, HashMap<Keychain, Vec<DerivedAddr>>>>>,
    // script_history: for electrum
    pub(crate) script_history: Arc<Mutex<LruCache<DerivedAddr, Vec<GetHistoryRes>>>>,
    // tx_details: for electrum
    pub(crate) tx_details: Arc<Mutex<LruCache<Txid, TxDetail>>>,
}

impl IndexerCache {
    pub fn new(size: NonZeroUsize) -> Self {
        Self {
            addr_transactions: Arc::new(Mutex::new(LruCache::new(size))),
            wallet_addresses: Arc::new(Mutex::new(LruCache::new(size))),
            script_history: Arc::new(Mutex::new(LruCache::new(size))),
            // size of tx_details is 20 times the size of script_history for electrum
            tx_details: Arc::new(Mutex::new(LruCache::new(
                size.saturating_mul(NonZeroUsize::new(20).expect("20 is not zero")),
            ))),
        }
    }

    #[allow(dead_code, unused_variables)]
    fn get_cached_addresses(&self, id: String) -> HashMap<Keychain, Vec<DerivedAddr>> {
        // From IndexerCache get cached addresses
        todo!()
    }

    // #[allow(dead_code)]
    // fn derive_new_addresses(&self, id:String, keychain: &Keychain, new_addresses: &mut
    // Vec<DerivedAddr>) -> impl Iterator<Item = DerivedAddr> { Derive new addresses
    // todo!()
    // }

    #[allow(dead_code, unused_variables)]
    fn get_cached_history(&self, derived_addr: &DerivedAddr) -> Vec<GetHistoryRes> {
        // Get cached transaction history from IndexerCache
        todo!()
    }

    #[allow(dead_code, unused_variables)]
    // , cache: &mut WalletCache<_>
    fn update_transaction_cache(
        &self,
        derived_addr: &DerivedAddr,
        new_history: Vec<GetHistoryRes>,
        updated_count: &mut usize,
        errors: &mut Vec<ElectrumError>,
    ) {
        // Update transaction cache
        todo!()
    }

    #[allow(dead_code, unused_variables)]
    fn derive_additional_addresses(
        &self,
        id: String,
        keychain: &Keychain,
        new_addresses: &mut Vec<DerivedAddr>,
    ) {
        // Derive additional addresses until 10 consecutive empty addresses are encountered
        todo!()
    }

    #[allow(dead_code, unused_variables)]
    fn update_cached_addresses(&self, id: String, new_addresses: Vec<DerivedAddr>) {
        // Update the address cache in IndexerCache
        todo!()
    }
}
