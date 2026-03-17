//! Inclusion List types for FOCIL (EIP-7805).
//!
//! This module implements the consensus types for Fork-choice enforced
//! Inclusion Lists, which ensure censorship resistance by requiring
//! block builders to include transactions specified by a committee
//! of validators.

mod inclusion_list;

pub use inclusion_list::{
    InclusionList, IlTransaction, IlTransactions, SignedInclusionList,
    MAX_BYTES_PER_INCLUSION_LIST, MAX_TRANSACTIONS_PER_INCLUSION_LIST,
};