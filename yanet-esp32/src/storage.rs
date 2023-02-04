use crate::wifi::WifiService;
use anyhow::Result;
use embedded_svc::storage::RawStorage;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use serde::{de::DeserializeOwned, Serialize};
use std::cell::RefCell;
use yanet_core::authenticate::PeerId;
pub struct StorageService {
    default_nvs: EspDefaultNvsPartition,
    storage: RefCell<EspNvs<NvsDefault>>,
}

impl StorageService {
    pub fn new() -> Result<Self> {
        let default_nvs = EspDefaultNvsPartition::take()?;
        let storage = RefCell::new(EspNvs::new(default_nvs.clone(), "storage", true)?);

        Ok(Self {
            default_nvs,
            storage,
        })
    }
    pub fn default_nvs(&self) -> EspDefaultNvsPartition {
        self.default_nvs.clone()
    }

    pub fn private_key(&self, _: &WifiService) -> [u8; 32] {
        let mut key = [0u8; 32];
        let mut storage = self.storage.borrow_mut();

        if let Some(buf) = storage.get_raw("key", &mut key).unwrap() {
            buf.try_into().unwrap()
        } else {
            let key = rand::random::<[u8; 32]>();
            storage.set_raw("key", &key).unwrap();
            key
        }
    }

    pub fn public_key(&self, wifi: &WifiService) -> [u8; 32] {
        x25519_dalek::x25519(self.private_key(wifi), x25519_dalek::X25519_BASEPOINT_BYTES)
    }
    pub fn peer_id(&self, wifi: &WifiService) -> PeerId {
        self.public_key(wifi).into()
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut buf = [0; 1024];
        if let Some(buf) = self.storage.borrow_mut().get_raw(key, &mut buf)? {
            Ok(Some(postcard::from_bytes(buf)?))
        } else {
            Ok(None)
        }
    }
    pub fn get_str(&self, key: &str) -> Result<Option<String>> {
        self.get(key)
    }
    pub fn set(&self, key: &str, value: &impl Serialize) -> Result<()> {
        let buf = postcard::to_allocvec(value)?;
        self.storage.borrow_mut().set_raw(key, &buf)?;
        Ok(())
    }
    pub fn set_str(&self, key: &str, value: &str) -> Result<()> {
        self.set(key, &value)
    }
}
