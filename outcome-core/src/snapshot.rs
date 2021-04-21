use std::convert::TryFrom;
use std::path::PathBuf;
use std::time::Instant;

use chrono::{DateTime, Utc};
use fnv::FnvHashMap;

use crate::distr::SimNode;
use crate::entity::Entity;
use crate::error::Error;
use crate::{EntityId, EventName, Result, Sim, SimModel};
use id_pool::IdPool;

/// Representation of the simulation state at a certain point in time.
///
/// This representation is not fully self-sufficient, and will require the
/// project file structure for proper initialization.
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub metadata: SnapshotMetadata,
    pub clock: usize,
    pub event_queue: Vec<EventName>,
    pub entity_pool: IdPool,
    pub model: SimModel,
    pub entities: FnvHashMap<EntityId, Entity>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub created: DateTime<Utc>,
}

/// Partial snapshot, used when partitioning large snapshots.
// TODO support snapshot partitioning
#[derive(Serialize, Deserialize)]
pub struct SnapshotPart {
    pub entities: FnvHashMap<EntityId, Entity>,
}

impl From<Sim> for Snapshot {
    fn from(sim: Sim) -> Self {
        Self {
            metadata: SnapshotMetadata {
                created: Utc::now(),
            },
            clock: sim.clock,
            event_queue: sim.event_queue,
            entity_pool: sim.entity_pool,
            model: sim.model,
            entities: sim.entities,
        }
    }
}

impl TryFrom<&Vec<u8>> for Snapshot {
    type Error = Error;
    fn try_from(bytes: &Vec<u8>) -> Result<Self> {
        #[cfg(feature = "lz4")]
        {
            match lz4::block::decompress(&bytes, None) {
                Ok(data) => {
                    let snapshot: Snapshot = bincode::deserialize(&data)
                        .map_err(|e| Error::FailedReadingSnapshot(e.to_string()))?;
                    Ok(snapshot)
                }
                Err(e) => Err(Error::SnapshotDecompressionError(e.to_string())),
            }
        }
        #[cfg(not(feature = "lz4"))]
        {
            let snapshot: Snapshot = bincode::deserialize(&bytes)?;
            Ok(snapshot)
        }
    }
}

impl Snapshot {
    pub fn to_bytes(&self, compress: bool) -> Result<Vec<u8>> {
        let mut data: Vec<u8> =
            bincode::serialize(&self).map_err(|e| Error::FailedCreatingSnapshot(e.to_string()))?;
        #[cfg(feature = "lz4")]
        {
            if compress {
                data = lz4::block::compress(&data, None, true)?;
            }
        }
        Ok(data)
    }
}

/// Self-sufficient representation of a simulation state that includes
/// serialized project files.
pub struct Package {
    pub project: FnvHashMap<PathBuf, Vec<u8>>,
    pub snapshot: Snapshot,
}
