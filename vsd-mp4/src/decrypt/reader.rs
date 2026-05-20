use std::io::{ErrorKind, Read, Result};

pub struct BoxHeader {
    pub box_type: [u8; 4],
    pub header_bytes: Vec<u8>,
    pub total_size: u64,
}

impl BoxHeader {
    pub fn read_header<R: Read>(reader: &mut R) -> Result<Option<Self>> {
        let mut buf = [0u8; 8];
        match reader.read_exact(&mut buf) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let size = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as u64;
        let box_type = [buf[4], buf[5], buf[6], buf[7]];

        if size == 1 {
            let mut ext = [0u8; 8];
            reader.read_exact(&mut ext)?;
            let total_size = u64::from_be_bytes(ext);
            let mut header = Vec::with_capacity(16);
            header.extend_from_slice(&buf);
            header.extend_from_slice(&ext);
            Ok(Some(BoxHeader {
                box_type,
                total_size,
                header_bytes: header,
            }))
        } else {
            Ok(Some(BoxHeader {
                box_type,
                header_bytes: buf.to_vec(),
                total_size: size,
            }))
        }
    }

    pub fn read_data<R: Read>(&self, reader: &mut R) -> Result<Vec<u8>> {
        if self.total_size == 0 {
            let mut data = Vec::from(self.header_bytes.as_slice());
            reader.read_to_end(&mut data)?;
            return Ok(data);
        }

        let body_size = self.total_size as usize - self.header_bytes.len();
        let mut data = vec![0u8; self.total_size as usize];
        data[..self.header_bytes.len()].copy_from_slice(&self.header_bytes);
        if body_size > 0 {
            reader.read_exact(&mut data[self.header_bytes.len()..])?;
        }
        Ok(data)
    }

    pub fn read_init<R: Read>(reader: &mut R) -> Result<(Vec<u8>, Option<BoxHeader>)> {
        let mut init_data = Vec::new();
    
        loop {
            let Some(header) = BoxHeader::read_header(reader)? else {
                break;
            };
    
            if &header.box_type == b"moof" {
                return Ok((init_data, Some(header)));
            }
    
            let box_data = header.read_data(reader)?;
            init_data.extend_from_slice(&box_data);
        }
    
        Ok((init_data, None))
    }
}
