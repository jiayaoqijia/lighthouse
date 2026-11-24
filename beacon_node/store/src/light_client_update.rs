//! Durable schema identification. The wire SSZ remains unchanged. A committee
//! period can contain a fork, so its first epoch cannot identify every record.
use crate::Error;
use ssz::Encode;
use types::{ChainSpec, EthSpec, ForkName, LightClientUpdate};

const MAGIC: &[u8] = b"LCUP";
const VERSION: u8 = 1;
const FORKS: [ForkName; 6] = [
    ForkName::Altair,
    ForkName::Capella,
    ForkName::Deneb,
    ForkName::Electra,
    ForkName::Fulu,
    ForkName::Gloas,
];

pub(crate) fn encode<E: EthSpec>(update: &LightClientUpdate<E>) -> Vec<u8> {
    let tag = update.map_with_fork_name(|fork| {
        FORKS
            .iter()
            .position(|candidate| *candidate == fork)
            .unwrap() as u8
    });
    let mut bytes = Vec::with_capacity(6 + update.ssz_bytes_len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&[VERSION, tag]);
    update.ssz_append(&mut bytes);
    bytes
}

fn matches_schema<E: EthSpec>(update: &LightClientUpdate<E>, spec: &ChainSpec) -> bool {
    let fork = spec.fork_name_at_slot::<E>(update.attested_header_slot());
    update.map_with_fork_name(|schema| {
        schema == fork || (schema == ForkName::Altair && fork == ForkName::Bellatrix)
    })
}

pub(crate) fn decode<E: EthSpec>(
    bytes: &[u8],
    spec: &ChainSpec,
) -> Result<LightClientUpdate<E>, Error> {
    if bytes.starts_with(MAGIC) {
        if bytes.get(4) != Some(&VERSION) {
            return Err(Error::InvalidBytes);
        }
        let fork = bytes
            .get(5)
            .and_then(|tag| FORKS.get(usize::from(*tag)))
            .ok_or(Error::InvalidBytes)?;
        let update = LightClientUpdate::from_ssz_bytes(&bytes[6..], fork)?;
        return matches_schema(&update, spec)
            .then_some(update)
            .ok_or(Error::InvalidBytes);
    }
    // Existing databases contain untagged wire SSZ. Accept only a complete
    // decoding whose attested slot identifies the same schema. Identical
    // Electra/Fulu layouts are disambiguated by that slot, not trial order.
    for fork in FORKS {
        if let Ok(update) = LightClientUpdate::from_ssz_bytes(bytes, &fork)
            && matches_schema(&update, spec)
        {
            return Ok(update);
        }
    }
    Err(Error::InvalidBytes)
}
