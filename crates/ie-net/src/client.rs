use anyhow::Result;
use url::Url;

pub struct Client {}

impl Client {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub async fn get(&self, _url: &Url) -> Result<Vec<u8>> {
        todo!("HTTP GET")
    }
}
