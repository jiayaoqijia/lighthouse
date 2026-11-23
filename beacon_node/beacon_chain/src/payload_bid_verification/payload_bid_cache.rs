use crate::payload_bid_verification::{
    PayloadBidError, gossip_verified_bid::GossipVerifiedPayloadBid,
};
use educe::Educe;
use parking_lot::RwLock;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use types::{
    BuilderIndex, EthSpec, ExecutionBlockHash, ExecutionPayloadBidRef, Hash256,
    SignedExecutionPayloadBid, Slot,
};

/// The parent a bid builds on: which beacon block, and which payload state of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BidParent {
    pub parent_block_hash: ExecutionBlockHash,
    pub parent_block_root: Hash256,
}

impl BidParent {
    pub fn from_bid<E: EthSpec>(bid: ExecutionPayloadBidRef<'_, E>) -> Self {
        Self {
            parent_block_hash: bid.parent_block_hash(),
            parent_block_root: bid.parent_block_root(),
        }
    }
}

/// The highest-value bid seen per `(slot, BidParent)`.
///
/// Keyed first by `Slot` (in a `BTreeMap` so that stale slots can be pruned cheaply via
/// `split_off`), then by the [`BidParent`] the bid builds on.
type HighestBidMap<E> = BTreeMap<Slot, HashMap<BidParent, GossipVerifiedPayloadBid<E>>>;

/// The mutable state guarded by the cache's lock.
#[derive(Educe)]
#[educe(Default(bound = "E: EthSpec"))]
pub struct GossipBidCacheInner<E: EthSpec> {
    /// The current best bid for each `(slot, BidParent)`.
    highest_bid: HighestBidMap<E>,
    /// Accepted gossip bid counts, separately for each slot, parent and builder.
    bid_counts: BTreeMap<Slot, HashMap<(BidParent, BuilderIndex), u64>>,
}

/// A cache of gossip-verified payload bids.
///
/// Tracks, per slot, the highest-value bid observed for each parent a bid may build on and the
/// count of accepted bids per `(parent, builder)`, so that duplicate and lower-value
/// gossip bids can be rejected. Stale entries are removed via [`prune`](Self::prune) as the chain
/// advances.
#[derive(Educe)]
#[educe(Default(bound = "E: EthSpec"))]
pub struct GossipVerifiedPayloadBidCache<E: EthSpec> {
    inner: RwLock<GossipBidCacheInner<E>>,
}

impl<E: EthSpec> GossipVerifiedPayloadBidCache<E> {
    /// Create a new, empty cache.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(GossipBidCacheInner::default()),
        }
    }

    /// Get the highest-value cached bid for `(slot, bid_parent)`, if one exists.
    pub fn get_highest_bid(
        &self,
        slot: Slot,
        bid_parent: BidParent,
    ) -> Option<Arc<SignedExecutionPayloadBid<E>>> {
        self.inner
            .read()
            .highest_bid
            .get(&slot)
            .and_then(|map| map.get(&bid_parent).map(|b| b.signed_bid.clone()))
    }

    /// Atomically enforce best-value and balance-backed count limits after signature
    /// verification. Parallel verifiers cannot oversubscribe the same allowance.
    pub fn observe_bid_with_limit(
        &self,
        bid: GossipVerifiedPayloadBid<E>,
        bid_limit: u64,
    ) -> Result<(), PayloadBidError> {
        let slot = bid.signed_bid.message().slot();
        let key = BidParent::from_bid(bid.signed_bid.message());
        let builder_index = bid.signed_bid.message().builder_index();
        let value = bid.signed_bid.message().value();
        let mut inner = self.inner.write();
        if let Some(cached) = inner.highest_bid.get(&slot).and_then(|map| map.get(&key)) {
            let cached_value = cached.signed_bid.message().value();
            if value <= cached_value {
                return Err(PayloadBidError::BidValueBelowCached {
                    cached_value,
                    incoming_value: value,
                });
            }
        }
        let count = inner
            .bid_counts
            .get(&slot)
            .and_then(|counts| counts.get(&(key, builder_index)))
            .copied()
            .unwrap_or(0);
        if count >= bid_limit {
            return Err(PayloadBidError::BuilderAlreadySeen {
                builder_index,
                slot,
            });
        }
        inner
            .bid_counts
            .entry(slot)
            .or_default()
            .insert((key, builder_index), count.saturating_add(1));
        inner.highest_bid.entry(slot).or_default().insert(key, bid);
        Ok(())
    }

    /// Test fixtures can seed already-verified candidates without a state lookup.
    pub fn observe_bid(&self, bid: GossipVerifiedPayloadBid<E>) -> bool {
        self.observe_bid_with_limit(bid, u64::MAX).is_ok()
    }

    pub fn builder_bid_count(
        &self,
        slot: &Slot,
        bid_parent: BidParent,
        builder_index: BuilderIndex,
    ) -> u64 {
        self.inner
            .read()
            .bid_counts
            .get(slot)
            .and_then(|counts| counts.get(&(bid_parent, builder_index)))
            .copied()
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub fn seen_builder_bid_for_parent(
        &self,
        slot: &Slot,
        bid_parent: BidParent,
        builder_index: BuilderIndex,
    ) -> bool {
        self.builder_bid_count(slot, bid_parent, builder_index) != 0
    }

    /// Removes all cached bids and seen-builder records for slots older than `current_slot`.
    ///
    /// Entries for `current_slot` and later are retained.
    pub fn prune(&self, current_slot: Slot) {
        let mut inner = self.inner.write();
        inner.highest_bid = inner.highest_bid.split_off(&current_slot);
        inner.bid_counts = inner.bid_counts.split_off(&current_slot);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bls::Signature;
    use types::{
        ExecutionBlockHash, ExecutionPayloadBidGloas, Hash256, SignedExecutionPayloadBid,
        SignedExecutionPayloadBidGloas, Slot, Spec,
    };

    use super::{BidParent, GossipVerifiedPayloadBidCache};
    use crate::payload_bid_verification::gossip_verified_bid::GossipVerifiedPayloadBid;

    type E = Spec;

    fn make_gossip_verified(
        slot: Slot,
        builder_index: u64,
        parent_block_hash: ExecutionBlockHash,
        parent_block_root: Hash256,
        value: u64,
    ) -> GossipVerifiedPayloadBid<E> {
        GossipVerifiedPayloadBid {
            signed_bid: Arc::new(SignedExecutionPayloadBid::Gloas(
                SignedExecutionPayloadBidGloas {
                    message: ExecutionPayloadBidGloas {
                        slot,
                        builder_index,
                        parent_block_hash,
                        parent_block_root,
                        value,
                        ..ExecutionPayloadBidGloas::default()
                    },
                    signature: Signature::empty(),
                },
            )),
        }
    }

    #[test]
    fn seen_builder_for_parent() {
        let cache = GossipVerifiedPayloadBidCache::<E>::default();
        let slot = Slot::new(1);
        let parent_a = BidParent {
            parent_block_hash: ExecutionBlockHash::zero(),
            parent_block_root: Hash256::ZERO,
        };
        let parent_b = BidParent {
            parent_block_hash: ExecutionBlockHash::from_root(Hash256::repeat_byte(0x01)),
            parent_block_root: Hash256::repeat_byte(0x02),
        };

        let verified = make_gossip_verified(
            slot,
            0,
            parent_a.parent_block_hash,
            parent_a.parent_block_root,
            100,
        );
        cache.observe_bid(verified);

        // Seen only for the exact (slot, parent tuple, builder) combination.
        assert!(cache.seen_builder_bid_for_parent(&slot, parent_a, 0));
        assert!(!cache.seen_builder_bid_for_parent(&slot, parent_b, 0));
        assert!(!cache.seen_builder_bid_for_parent(&slot, parent_a, 1));
        assert!(!cache.seen_builder_bid_for_parent(&Slot::new(2), parent_a, 0));
    }

    #[test]
    fn highest_bid_for_parent() {
        let cache = GossipVerifiedPayloadBidCache::<E>::default();
        let slot = Slot::new(1);
        let hash_a = ExecutionBlockHash::zero();
        let root_a = Hash256::ZERO;
        let hash_b = ExecutionBlockHash::from_root(Hash256::repeat_byte(0x01));
        let root_b = Hash256::repeat_byte(0x02);
        let parent_a = BidParent {
            parent_block_hash: hash_a,
            parent_block_root: root_a,
        };
        let parent_b = BidParent {
            parent_block_hash: hash_b,
            parent_block_root: root_b,
        };

        assert!(cache.observe_bid(make_gossip_verified(slot, 0, hash_a, root_a, 100)));
        assert!(cache.observe_bid(make_gossip_verified(slot, 1, hash_b, root_b, 50)));

        // Each parent tuple keeps its own highest bid.
        assert_eq!(
            cache
                .get_highest_bid(slot, parent_a)
                .unwrap()
                .message()
                .value(),
            100
        );
        assert_eq!(
            cache
                .get_highest_bid(slot, parent_b)
                .unwrap()
                .message()
                .value(),
            50
        );

        // A lower bid does not replace the cached bid for its tuple, and does
        // not touch the other tuple.
        assert!(!cache.observe_bid(make_gossip_verified(slot, 2, hash_a, root_a, 60)));
        assert_eq!(
            cache
                .get_highest_bid(slot, parent_a)
                .unwrap()
                .message()
                .value(),
            100
        );
        assert_eq!(
            cache
                .get_highest_bid(slot, parent_b)
                .unwrap()
                .message()
                .value(),
            50
        );

        // A higher bid replaces the cached bid for its tuple.
        assert!(cache.observe_bid(make_gossip_verified(slot, 3, hash_b, root_b, 70)));
        let highest_b = cache.get_highest_bid(slot, parent_b).unwrap();
        assert_eq!(highest_b.message().value(), 70);
        assert_eq!(highest_b.message().builder_index(), 3);
    }

    #[test]
    fn prune_removes_old_retains_current() {
        let cache = GossipVerifiedPayloadBidCache::<E>::default();
        let hash = ExecutionBlockHash::zero();
        let root = Hash256::ZERO;
        let bid_parent = BidParent {
            parent_block_hash: hash,
            parent_block_root: root,
        };

        for slot in [1, 2, 3, 7, 8, 9, 10] {
            let verified = make_gossip_verified(Slot::new(slot), slot, hash, root, slot * 100);
            cache.observe_bid(verified);
        }

        cache.prune(Slot::new(8));

        // Slots 1-7 pruned from both maps.
        for slot in [1, 2, 3, 7] {
            assert!(cache.get_highest_bid(Slot::new(slot), bid_parent).is_none());
            assert!(!cache.seen_builder_bid_for_parent(&Slot::new(slot), bid_parent, slot));
        }
        // Slots 8-10 retained in both maps.
        for slot in [8, 9, 10] {
            assert!(cache.get_highest_bid(Slot::new(slot), bid_parent).is_some());
            assert!(cache.seen_builder_bid_for_parent(&Slot::new(slot), bid_parent, slot));
        }
    }
    #[test]
    fn concurrent_admission_respects_budget_and_rejections_do_not_count() {
        let cache = Arc::new(GossipVerifiedPayloadBidCache::<E>::default());
        let slot = Slot::new(1);
        let hash = ExecutionBlockHash::zero();
        let root = Hash256::ZERO;
        let parent = BidParent {
            parent_block_hash: hash,
            parent_block_root: root,
        };
        assert!(
            cache
                .observe_bid_with_limit(make_gossip_verified(slot, 0, hash, root, 0), 2)
                .is_ok()
        );
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (1..=8)
            .map(|value| {
                let cache = cache.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    cache
                        .observe_bid_with_limit(make_gossip_verified(slot, 0, hash, root, value), 2)
                        .is_ok()
                })
            })
            .collect();
        let accepted = threads
            .into_iter()
            .map(|thread| usize::from(thread.join().unwrap()))
            .sum::<usize>();
        assert_eq!(accepted, 1);
        assert_eq!(cache.builder_bid_count(&slot, parent, 0), 2);
        assert_eq!(cache.builder_bid_count(&slot, parent, 1), 0);
        assert_eq!(cache.builder_bid_count(&Slot::new(2), parent, 0), 0);
        let fresh = GossipVerifiedPayloadBidCache::<E>::default();
        assert!(
            fresh
                .observe_bid_with_limit(make_gossip_verified(slot, 0, hash, root, 10), 0)
                .is_err()
        );
        assert_eq!(fresh.builder_bid_count(&slot, parent, 0), 0);
        assert!(fresh.get_highest_bid(slot, parent).is_none());
    }
}
