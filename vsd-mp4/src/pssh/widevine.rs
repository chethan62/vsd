use crate::Result;
use prost::Message;

include!(concat!(env!("OUT_DIR"), "/widevine.rs"));

pub fn parse_key_ids(data: &[u8]) -> Result<Vec<String>> {
    let wv = WidevinePsshData::decode(data)?;
    Ok(wv.key_ids.into_iter().map(hex::encode).collect())
}
