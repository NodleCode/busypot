use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct EncodedKey {
    pub content: Vec<String>,
    #[serde(rename = "type")]
    pub type_field: Vec<String>,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct AccountMeta {
    pub name: String,
    pub whenCreated: u64,
}

#[derive(Debug, Deserialize)]
pub struct AccountData {
    pub address: String,
    pub encoded: String,
    pub encoding: EncodedKey,
    pub meta: AccountMeta,
}
