use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use bpstd::{BlockHash, DerivedAddr, Tx, Txid};
use electrum::GetHistoryRes;
use lru::LruCache;

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
    // script_history: for electrum
    pub(crate) script_history: Arc<Mutex<LruCache<DerivedAddr, HashMap<Txid, GetHistoryRes>>>>,
    // tx_details: for electrum
    pub(crate) tx_details: Arc<Mutex<LruCache<Txid, TxDetail>>>,
}

impl IndexerCache {
    pub fn new(size: NonZeroUsize) -> Self {
        Self {
            addr_transactions: Arc::new(Mutex::new(LruCache::new(size))),
            script_history: Arc::new(Mutex::new(LruCache::new(size))),
            // size of tx_details is 20 times the size of script_history for electrum
            tx_details: Arc::new(Mutex::new(LruCache::new(
                size.saturating_mul(NonZeroUsize::new(20).expect("20 is not zero")),
            ))),
        }
    }
}
