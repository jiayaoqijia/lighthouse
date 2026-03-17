//! Inclusion List types for FOCIL (EIP-7805).
//!
//! This module implements the consensus types for Fork-choice enforced
//! Inclusion Lists, which ensure censorship resistance by requiring
//! block builders to include transactions specified by a committee
//! of validators.

mod helpers;
mod inclusion_list;

pub use helpers::{
    get_inclusion_list_committee, get_inclusion_list_committee_root,
    is_inclusion_list_committee_member, is_valid_inclusion_list_signature,
};
pub use inclusion_list::{
    InclusionList, IlTransaction, IlTransactions, SignedInclusionList,
    MAX_BYTES_PER_INCLUSION_LIST, MAX_TRANSACTIONS_PER_INCLUSION_LIST,
};

/// Size of the inclusion list committee as per EIP-7805.
pub const INCLUSION_LIST_COMMITTEE_SIZE: usize = 16;