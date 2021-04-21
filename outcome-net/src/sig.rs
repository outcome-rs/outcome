use crate::socket::Encoding;
use crate::{Result, TaskId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal(u32, outcome::distr::Signal);

impl Signal {
    pub fn from(id: TaskId, sig: outcome::distr::Signal) -> Self {
        Self(id, sig)
    }

    pub fn from_bytes(bytes: &[u8], encoding: &Encoding) -> Result<Self> {
        let sig = match encoding {
            Encoding::Bincode => bincode::deserialize(bytes)?,
            // #[cfg(feature = "msgpack_encoding")]
            // Encoding::MsgPack => {
            //     let mut de = rmp_serde::Deserializer::new(bytes);
            //     Deserialize::deserialize(&mut de)?
            // }
            _ => unimplemented!(),
        };
        Ok(sig)
    }

    pub fn to_bytes(&self, encoding: &Encoding) -> Result<Vec<u8>> {
        let bytes: Vec<u8> = match encoding {
            Encoding::Bincode => bincode::serialize(self)?,
            // #[cfg(feature = "msgpack_encoding")]
            // Encoding::MsgPack => {
            //     let mut buf = Vec::new();
            //     self.serialize(&mut rmp_serde::Serializer::new(&mut buf))?;
            //     buf
            // }
            _ => unimplemented!(),
        };
        Ok(bytes)
    }

    pub fn into_inner(self) -> (u32, outcome::distr::Signal) {
        (self.0, self.1)
    }
}
