use crate::{Reader, Result, bail, pssh::wrm_header::WrmHeader};

pub fn parse_key_ids(data: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::new_little_endian(data);
    let size = reader.read_u32()?;

    if size as usize != data.len() {
        bail!("Invalid length of pssh box playready object.");
    }

    let count = reader.read_u16()?;
    let mut kids = Vec::new();

    for _ in 0..count {
        let record_type = reader.read_u16()?;
        let record_len = reader.read_u16()?;
        let record_data = reader.read_bytes_u16(record_len as usize)?;

        match record_type {
            1 => {
                let xml = String::from_utf16(&record_data)?;
                let wrm_header = quick_xml::de::from_str::<WrmHeader>(&xml)?;
                kids.extend(wrm_header.key_ids()?);
            }
            2 | 3 => (),
            _ => {
                bail!("Invalid pssh box playready object record type {record_type}.");
            }
        }
    }

    if reader.has_more_data() {
        bail!("pssh box has extra data after playready object records.");
    }

    Ok(kids)
}
