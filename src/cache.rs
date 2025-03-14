use alloy_rpc_types_eth::Block as EthBlock;
use sp_core::H256;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

/// Maximum number of blocks to store in cache
pub const DEFAULT_MAX_BLOCKS: usize = 100;

/// Thread-safe in-memory block cache layer for storing recent block data
#[derive(Clone, Debug)]
pub struct BlockCache {
    inner: Arc<RwLock<BlockCacheInner>>,
}

/// Inner implementation of the block cache
#[derive(Debug)]
struct BlockCacheInner {
    /// FIFO queue to track block insertion order
    order: VecDeque<H256>,
    /// Maps block number to block hash
    number_to_hash: HashMap<u64, H256>,
    /// Maps block hash to full block data
    hash_to_block: HashMap<H256, EthBlock>,
    /// Maximum number of blocks to store
    max_blocks: usize,
}

impl BlockCache {
    /// Create a new block cache with default capacity
    pub fn new(capacity: Option<usize>) -> Self {
        Self::with_capacity(capacity.unwrap_or(DEFAULT_MAX_BLOCKS))
    }

    /// Create a new block cache with specified capacity
    pub fn with_capacity(max_blocks: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(BlockCacheInner {
                order: VecDeque::with_capacity(max_blocks),
                number_to_hash: HashMap::new(),
                hash_to_block: HashMap::new(),
                max_blocks,
            })),
        }
    }

    /// Insert block number to hash mapping
    pub fn insert_number_to_hash(&self, number: u64, hash: H256) {
        if let Ok(mut inner) = self.inner.write() {
            inner.number_to_hash.insert(number, hash);
        }
    }

    /// Insert a block into the cache
    pub fn insert_block(&self, block: EthBlock) {
        if let Ok(mut inner) = self.inner.write() {
            let hash = H256::from(block.header.hash.0);
            let number = block.header.inner.number;

            // If we're at capacity, remove the oldest block
            if inner.order.len() >= inner.max_blocks {
                if let Some(old_hash) = inner.order.pop_front() {
                    inner.hash_to_block.remove(&old_hash);

                    // Remove from number_to_hash mapping if it points to this hash
                    inner.number_to_hash.retain(|_, h| *h != old_hash);
                }
            }

            // Insert block hash to number mapping
            inner.number_to_hash.insert(number.into(), hash.0.into());

            // Insert the full block
            inner.hash_to_block.insert(hash.0.into(), block.clone());

            // Add to ordered list
            inner.order.push_back(hash.0.into());
            log::info!("CACHE: inserted block: {:?}", block);
        }
    }

    /// Get a block by number
    pub fn get_by_number(&self, number: u64) -> Option<EthBlock> {
        log::info!("CACHE: get_by_number: {}", number);
        if let Ok(inner) = self.inner.read() {
            inner
                .number_to_hash
                .get(&number)
                .and_then(|hash| inner.hash_to_block.get(hash).cloned())
        } else {
            None
        }
    }

    /// Get a block by hash
    pub fn get_by_hash(&self, hash: &H256) -> Option<EthBlock> {
        log::info!("CACHE: get_by_hash: {}", hash);
        if let Ok(inner) = self.inner.read() {
            inner.hash_to_block.get(hash).cloned()
        } else {
            None
        }
    }

    /// Get the hash of the block with the given number
    pub fn get_hash_by_number(&self, number: u64) -> Option<H256> {
        log::info!("CACHE: get_hash_by_number: {}", number);
        if let Ok(inner) = self.inner.read() {
            inner.number_to_hash.get(&number).cloned()
        } else {
            None
        }
    }

    /// Clear the cache
    pub fn clear(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.order.clear();
            inner.number_to_hash.clear();
            inner.hash_to_block.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Header as ConsensusHeader;
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::{BlockTransactions, Header};

    fn create_mock_block(number: u64, hash_suffix: u8) -> EthBlock {
        let mut hash = [0u8; 32];
        hash[31] = hash_suffix;

        EthBlock {
            header: Header {
                hash: B256::from(hash),
                inner: ConsensusHeader {
                    number: number.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            transactions: BlockTransactions::Full(vec![]),
            ..Default::default()
        }
    }

    #[test]
    fn test_insert_and_retrieve() {
        let cache = BlockCache::with_capacity(2);
        let block1 = create_mock_block(1, 1);
        let block2 = create_mock_block(2, 2);

        cache.insert_block(block1.clone());
        cache.insert_block(block2.clone());

        assert_eq!(cache.get_by_number(1), Some(block1.clone()));
        assert_eq!(cache.get_by_number(2), Some(block2.clone()));
        assert_eq!(
            cache.get_by_hash(&H256::from(block1.header.hash.0)),
            Some(block1)
        );
        assert_eq!(
            cache.get_by_hash(&H256::from(block2.header.hash.0)),
            Some(block2)
        );
    }

    #[test]
    fn test_capacity_limit() {
        let cache = BlockCache::with_capacity(2);
        let block1 = create_mock_block(1, 1);
        let block2 = create_mock_block(2, 2);
        let block3 = create_mock_block(3, 3);

        cache.insert_block(block1.clone());
        cache.insert_block(block2.clone());
        cache.insert_block(block3.clone());

        assert_eq!(cache.get_by_number(1), None); // Should be evicted
        assert_eq!(cache.get_by_number(2), Some(block2));
        assert_eq!(cache.get_by_number(3), Some(block3));
    }

    #[test]
    fn test_latest_block() {
        let cache = BlockCache::with_capacity(3);

        let block1 = create_mock_block(1, 1);
        let block2 = create_mock_block(2, 2);

        cache.insert_block(block1);
        cache.insert_block(block2.clone());
    }
}
