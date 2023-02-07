use base58::ToBase58;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct PeerId([u8; 32]);

impl PeerId {
    pub fn get_key(&self) -> [u8; 32] {
        self.0
    }
}

impl<'a> TryFrom<&'a [u8]> for PeerId {
    type Error = <[u8; 32] as TryFrom<&'a [u8]>>::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let a: [u8; 32] = TryFrom::try_from(value)?;
        Ok(Self::from(a))
    }
}
impl From<[u8; 32]> for PeerId {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.as_ref().to_base58())?;
        Ok(())
    }
}
impl std::ops::DerefMut for PeerId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Deref for PeerId {
    type Target = [u8; 32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait Authenticated {
    fn peer_id(&self) -> PeerId;
}
