use crate::Result;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal(outcome::distr::Signal);

impl Signal {
    pub fn from(sig: outcome::distr::Signal) -> Self {
        Self(sig)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut de = Deserializer::new(bytes);
        let mut sig: Signal = Deserialize::deserialize(&mut de)?;
        Ok(sig)
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf))?;
        Ok(buf)
    }
    pub fn inner(self) -> outcome::distr::Signal {
        self.0
    }
}
