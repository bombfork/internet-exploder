use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum IpcMessage {
    FetchRequest {
        id: u64,
        url: String,
    },
    FetchResponse {
        id: u64,
        status: u16,
        headers: HashMap<String, String>,
        #[serde(with = "base64_serde")]
        body: Vec<u8>,
        final_url: String,
    },
    FetchError {
        id: u64,
        error: String,
    },
    Shutdown,
    Ping,
    Pong,
}

mod base64_serde {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(data))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(&s)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_response_body_base64_round_trip() {
        let body = vec![0u8; 1_000_000]; // 1MB of zeros
        let msg = IpcMessage::FetchResponse {
            id: 1,
            status: 200,
            headers: HashMap::new(),
            body: body.clone(),
            final_url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        // Base64 encoding: ~1.33x body size, NOT ~4x (array of numbers)
        assert!(
            json.len() < body.len() * 2,
            "JSON too large: {} bytes for {} byte body",
            json.len(),
            body.len()
        );
        let decoded: IpcMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }
}
