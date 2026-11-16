use alloy_consensus::TxEnvelope;
use alloy_primitives::{Address, U256};
use alloy_rlp::{Decodable, Header};
use typenum::Unsigned;
use types::{EthSpec, ExecutionPayloadRef, ForkName, Hash256, VersionedHash};

const FRAME_TX_TYPE: u8 = 0x06;

#[derive(Debug)]
pub enum Error {
    DecodingTransaction(String),
    LengthMismatch { expected: usize, found: usize },
    VersionHashMismatch { expected: Hash256, found: Hash256 },
}

pub fn verify_versioned_hashes<E: EthSpec>(
    execution_payload: ExecutionPayloadRef<E>,
    expected_versioned_hashes: &[VersionedHash],
    fork: ForkName,
) -> Result<(), Error> {
    let versioned_hashes =
        extract_versioned_hashes_from_transactions::<E>(execution_payload.transactions(), fork)?;
    if versioned_hashes.len() != expected_versioned_hashes.len() {
        return Err(Error::LengthMismatch {
            expected: expected_versioned_hashes.len(),
            found: versioned_hashes.len(),
        });
    }
    for (found, expected) in versioned_hashes
        .iter()
        .zip(expected_versioned_hashes.iter())
    {
        if found != expected {
            return Err(Error::VersionHashMismatch {
                expected: *expected,
                found: *found,
            });
        }
    }

    Ok(())
}

pub fn extract_versioned_hashes_from_transactions<E: EthSpec>(
    transactions: types::TransactionsRef<E>,
    fork: ForkName,
) -> Result<Vec<VersionedHash>, Error> {
    let mut versioned_hashes = Vec::new();

    for tx in transactions.iter() {
        if fork.heze_enabled()
            && let Some(frame) = tx.strip_prefix(&[FRAME_TX_TYPE])
        {
            extract_frame_blob_hashes(frame, &mut versioned_hashes)
                .map_err(|error| Error::DecodingTransaction(error.to_string()))?;
            continue;
        }
        // TxEnvelope is non-exhaustive so unforunately we can (no longer) write an exhaustive
        // match here.
        if let TxEnvelope::Eip4844(signed_tx_eip4844) = tx_bytes_to_tx_envelope(tx)? {
            versioned_hashes.extend(
                signed_tx_eip4844
                    .tx()
                    .tx()
                    .blob_versioned_hashes
                    .iter()
                    .map(|fb| Hash256::from(fb.0)),
            );
        }
    }

    Ok(versioned_hashes)
}

/// Read EIP-8141's hash list without changing the bytes used for the transaction trie.
/// Frame execution and authorization remain the execution client's responsibility.
fn extract_frame_blob_hashes(
    mut transaction: &[u8],
    hashes: &mut Vec<VersionedHash>,
) -> alloy_rlp::Result<()> {
    let mut fields = Header::decode_bytes(&mut transaction, true)?;
    U256::decode(&mut fields)?; // chain_id
    u64::decode(&mut fields)?; // nonce
    Address::decode(&mut fields)?; // sender
    Header::decode_bytes(&mut fields, true)?; // frames
    Header::decode_bytes(&mut fields, true)?; // signatures
    let mut fees = Header::decode_bytes(&mut fields, true)?;
    U256::decode(&mut fees)?;
    U256::decode(&mut fees)?;
    let mut versioned_hashes = Header::decode_bytes(&mut fields, true)?;
    if !(transaction.is_empty() && fields.is_empty() && fees.is_empty()) {
        return Err(alloy_rlp::Error::Custom("extra frame transaction fields"));
    }
    while !versioned_hashes.is_empty() {
        let versioned_hash = <[u8; 32]>::decode(&mut versioned_hashes)?;
        if versioned_hash.first() != Some(&0x01) {
            return Err(alloy_rlp::Error::Custom("invalid frame blob hash version"));
        }
        hashes.push(Hash256::from(versioned_hash));
    }
    Ok(())
}

pub fn beacon_tx_to_tx_envelope<N: Unsigned>(
    tx: &types::Transaction<N>,
) -> Result<TxEnvelope, Error> {
    tx_bytes_to_tx_envelope(tx)
}

pub fn tx_bytes_to_tx_envelope(mut tx_bytes: &[u8]) -> Result<TxEnvelope, Error> {
    TxEnvelope::decode(&mut tx_bytes).map_err(|e| Error::DecodingTransaction(e.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::static_valid_tx;
    use alloy_consensus::TxLegacy;
    use alloy_primitives::TxKind;
    use types::Spec;

    type E = Spec;

    // EIP-8141 typed RLP fixtures from the pinned seven-field draft.
    const FRAME_WITHOUT_BLOBS: &str = "06eb0180941111111111111111111111111111111111111111cbca010380c4822710808080c0c5830f424001c0";
    const FRAME_WITH_BLOBS: &str = "06f86e0180941111111111111111111111111111111111111111cbca010380c4822710808080c0c5830f424001f842a00102020202020202020202020202020202020202020202020202020202020202a00103030303030303030303030303030303030303030303030303030303030303";

    #[test]
    fn test_decode_static_transaction() {
        let valid_tx = static_valid_tx::<E>().expect("should give me known valid transaction");
        let tx_envelope = beacon_tx_to_tx_envelope(&valid_tx).expect("should decode tx");
        let TxEnvelope::Legacy(signed_tx) = tx_envelope else {
            panic!("should decode to legacy transaction");
        };

        assert!(matches!(
            signed_tx.tx(),
            TxLegacy {
                chain_id: Some(0x01),
                nonce: 0x15,
                gas_price: 0x4a817c800,
                to: TxKind::Call(..),
                ..
            }
        ));
    }

    #[test]
    fn test_extract_versioned_hashes() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[serde(transparent)]
        struct TestTransactions<E: EthSpec>(
            #[serde(with = "ssz_types::serde_utils::list_of_hex_var_list")] types::Transactions<E>,
        );

        let TestTransactions(raw_transactions): TestTransactions<E> = serde_json::from_str(r#"[
            "0x03f901388501a1f0ff430f843b9aca00843b9aca0082520894e7249813d8ccf6fa95a2203f46a64166073d58878080c002f8c6a0012e98362c814f1724262c0d211a1463418a5f6382a8d457b37a2698afbe7b5ea00100ef985761395dfa8ed5ce91f3f2180b612401909e4cb8f33b90c8a454d9baa0013d45411623b90d90f916e4025ada74b453dd4ca093c017c838367c9de0f801a001753e2af0b1e70e7ef80541355b2a035cc9b2c177418bb2a4402a9b346cf84da0011789b520a8068094a92aa0b04db8d8ef1c6c9818947c5210821732b8744049a0011c4c4f95597305daa5f62bf5f690e37fa11f5de05a95d05cac4e2119e394db80a0ccd86a742af0e042d08cbb35d910ddc24bbc6538f9e53be6620d4b6e1bb77662a01a8bacbc614940ac2f5c23ffc00a122c9f085046883de65c88ab0edb859acb99",
            "0x02f9017a8501a1f0ff4382363485012a05f2008512a05f2000830249f094c1b0bc605e2c808aa0867bfc98e51a1fe3e9867f80b901040cc7326300000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000009445a285baa43e00000000000000000000000000c500931f24edb821cef6e28f7adb33b38578c82000000000000000000000000fc7360b3b28cf4204268a8354dbec60720d155d2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000020000000000000000000000009a054a063f0fe7b9c68de8df91aaa5e96c15ab540000000000000000000000000c8d41b8fcc066cdabaf074d78e5153e8ce018a9c080a008e14475c1173cd9f5740c24c08b793f9e16c36c08fa73769db95050e31e3396a019767dcdda26c4a774ca28c9df15d0c20e43bd07bd33ee0f84d6096cb5a1ebed"
        ]"#).expect("should get raw transactions");
        let expected_versioned_hashes = vec![
            "0x012e98362c814f1724262c0d211a1463418a5f6382a8d457b37a2698afbe7b5e",
            "0x0100ef985761395dfa8ed5ce91f3f2180b612401909e4cb8f33b90c8a454d9ba",
            "0x013d45411623b90d90f916e4025ada74b453dd4ca093c017c838367c9de0f801",
            "0x01753e2af0b1e70e7ef80541355b2a035cc9b2c177418bb2a4402a9b346cf84d",
            "0x011789b520a8068094a92aa0b04db8d8ef1c6c9818947c5210821732b8744049",
            "0x011c4c4f95597305daa5f62bf5f690e37fa11f5de05a95d05cac4e2119e394db",
        ]
        .into_iter()
        .map(|tx| Hash256::from_slice(&hex::decode(&tx[2..]).expect("should decode hex")))
        .collect::<Vec<_>>();

        let versioned_hashes = extract_versioned_hashes_from_transactions::<E>(
            (&raw_transactions).into(),
            ForkName::Deneb,
        )
        .expect("should get versioned hashes");
        assert_eq!(versioned_hashes, expected_versioned_hashes);

        let mixed = types::ProgressiveTransactions::new(
            [FRAME_WITHOUT_BLOBS, FRAME_WITH_BLOBS]
                .into_iter()
                .map(|raw| hex::decode(raw).unwrap())
                .chain(raw_transactions.iter().map(|tx| tx.to_vec()))
                .map(|tx| tx.try_into().unwrap())
                .collect(),
        )
        .unwrap();
        let expected = [2, 3]
            .map(|fill| {
                let mut hash = [fill; 32];
                hash[0] = 1;
                Hash256::from(hash)
            })
            .into_iter()
            .chain(expected_versioned_hashes)
            .collect::<Vec<_>>();
        assert_eq!(
            extract_versioned_hashes_from_transactions::<E>((&mixed).into(), ForkName::Heze)
                .unwrap(),
            expected
        );
        assert!(matches!(
            extract_versioned_hashes_from_transactions::<E>((&mixed).into(), ForkName::Gloas),
            Err(Error::DecodingTransaction(_))
        ));
    }

    #[test]
    fn frame_hash_extraction_rejects_malformed_wire_bytes() {
        let valid = hex::decode(FRAME_WITH_BLOBS).unwrap();
        let mut extra = valid.clone();
        extra.push(0);
        let mut wrong_version = valid.clone();
        let final_hash_start = wrong_version.len() - 32;
        wrong_version[final_hash_start] = 0;
        for raw in [&valid[..valid.len() - 1], &extra, &wrong_version] {
            let txs = types::ProgressiveTransactions::new(vec![raw.to_vec().try_into().unwrap()])
                .unwrap();
            assert!(matches!(
                extract_versioned_hashes_from_transactions::<E>((&txs).into(), ForkName::Heze),
                Err(Error::DecodingTransaction(_))
            ));
        }
    }
}
