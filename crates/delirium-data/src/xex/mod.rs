mod aes;
pub(crate) mod lzx;

const FILE_FORMAT_INFO: u32 = 0x0000_03ff;
const IMAGE_BASE_ADDRESS: u32 = 0x0001_0201;
const DELTA_PATCH_DESCRIPTOR: u32 = 0x0000_05ff;

#[derive(Debug)]
pub struct XexImage {
    pub base_address: u32,
    pub image: Vec<u8>,
}

fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| format!("Truncated XEX at {at}"))
}
fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("Truncated XEX at {at}"))
}
fn span(d: &[u8], at: usize, n: usize) -> Result<&[u8], String> {
    d.get(at..at.checked_add(n).ok_or("XEX offset overflow")?)
        .ok_or_else(|| format!("Truncated XEX at {at} (need {n})"))
}

impl XexImage {

    pub fn parse_with_key(xex: &[u8], retail_key: &[u8; 16]) -> Result<Self, String> {
        if span(xex, 0, 4)? != b"XEX2" {
            return Err("Not an XEX2 executable".into());
        }
        let payload_offset = be32(xex, 8)? as usize;
        let security = be32(xex, 16)? as usize;
        let header_count = be32(xex, 20)? as usize;
        let mut format = None;
        let mut base_address = None;
        for i in 0..header_count {
            let key = be32(xex, 24 + i * 8)?;
            let value = be32(xex, 28 + i * 8)?;
            match key {
                FILE_FORMAT_INFO => format = Some(value as usize),
                IMAGE_BASE_ADDRESS => base_address = Some(value),
                _ => {}
            }
        }
        let format = format.ok_or("XEX has no file format header")?;
        let encryption = be16(xex, format + 4)?;
        let compression = be16(xex, format + 6)?;
        let image_size = be32(xex, security + 4)? as usize;
        let base_address = match base_address {
            Some(address) => address,
            None => be32(xex, security + 0x110)?,
        };
        let mut payload = span(xex, payload_offset, xex.len().saturating_sub(payload_offset))?.to_vec();
        match encryption {
            0 => {}
            1 => {
                let mut key: [u8; 16] = span(xex, security + 0x150, 16)?.try_into().unwrap();
                aes::Aes128::new(*retail_key).decrypt_block(&mut key);
                aes::Aes128::new(key).decrypt_cbc_zero_iv(&mut payload);
            }
            other => return Err(format!("Unsupported XEX encryption type {other}")),
        }
        let image = match compression {
            1 => basic(xex, format, &payload, image_size)?,
            2 => normal(xex, format, &payload, image_size)?,
            other => return Err(format!("Unsupported XEX compression type {other}")),
        };
        if image.get(..2) != Some(b"MZ") {
            return Err("Unpacked XEX image does not start with a PE header (wrong key?)".into());
        }
        Ok(Self { base_address, image })
    }

    pub fn parse_patched_with_key(xex: &[u8], xexp: &[u8], retail_key: &[u8; 16]) -> Result<Self, String> {
        let mut this = Self::parse_with_key(xex, retail_key)?;
        if span(xexp, 0, 4)? != b"XEX2" {
            return Err("Not a XEX2 patch".into());
        }
        let (mut format, mut descriptor) = (None, None);
        for i in 0..be32(xexp, 20)? as usize {
            match be32(xexp, 24 + i * 8)? {
                FILE_FORMAT_INFO => format = Some(be32(xexp, 28 + i * 8)? as usize),
                DELTA_PATCH_DESCRIPTOR => descriptor = Some(be32(xexp, 28 + i * 8)? as usize),
                _ => {}
            }
        }
        let format = format.ok_or("XEX patch has no file format header")?;
        let d = descriptor.ok_or("XEX patch has no delta patch descriptor")?;
        if be16(xexp, format + 6)? != 3 {
            return Err("XEX patch is not delta-compressed".into());
        }
        let window_size = be32(xexp, format + 8)? as usize;

        let mut headers = span(xex, 0, be32(xex, 8)? as usize)?.to_vec();
        let target = be32(xexp, d + 0x30)? as usize;
        let (source_offset, source_size, target_offset) =
            (be32(xexp, d + 0x34)? as usize, be32(xexp, d + 0x38)? as usize, be32(xexp, d + 0x3C)? as usize);
        let target = if target == 0 { target_offset + source_size } else { target };
        if headers.len() < target {
            headers.resize(target, 0);
        }
        if source_offset != 0 {
            headers.copy_within(source_offset..source_offset + source_size, target_offset);
        }
        headers[target..].fill(0);
        let records = span(xexp, d + 0x4C, (be32(xexp, d)? as usize).min(xexp.len() - d - 0x4C))?;
        apply_records(records, window_size, &mut headers)?;
        headers.truncate(target);
        let security = be32(&headers, 16)? as usize;
        let image_size = be32(&headers, security + 4)? as usize;

        let payload_offset = be32(xexp, 8)? as usize;
        let mut payload = span(xexp, payload_offset, xexp.len().saturating_sub(payload_offset))?.to_vec();
        if be16(xexp, format + 4)? == 1 {
            let mut base_key: [u8; 16] = span(&headers, security + 0x150, 16)?.try_into().unwrap();
            aes::Aes128::new(*retail_key).decrypt_block(&mut base_key);
            let mut key: [u8; 16] = span(xexp, be32(xexp, 16)? as usize + 0x150, 16)?.try_into().unwrap();
            aes::Aes128::new(base_key).decrypt_block(&mut key);
            aes::Aes128::new(key).decrypt_cbc_zero_iv(&mut payload);
        }

        let image = &mut this.image;
        let old_size = image.len();
        if image.len() < image_size {
            image.resize(image_size, 0);
        }
        let (source_offset, source_size, target_offset) =
            (be32(xexp, d + 0x40)? as usize, be32(xexp, d + 0x44)? as usize, be32(xexp, d + 0x48)? as usize);
        if source_offset != 0 {
            image.copy_within(source_offset..source_offset + source_size, target_offset);
        }
        if target_offset + source_size < old_size {
            image[target_offset + source_size..old_size].fill(0);
        }
        let mut block_size = be32(xexp, format + 12)? as usize;
        let mut at = 0;
        while block_size != 0 {
            let block = span(&payload, at, block_size)?;
            let next = be32(block, 0)? as usize;
            if next > payload.len() {
                return Err("XEX patch payload did not decrypt (wrong base file for this patch?)".into());
            }
            apply_records(&block[24..], window_size, image)?;
            at += block_size;
            block_size = next;
        }
        image.truncate(image_size.max(1));
        if let Some(base) = (0..be32(&headers, 20)? as usize).find(|i| be32(&headers, 24 + i * 8) == Ok(IMAGE_BASE_ADDRESS)) {
            this.base_address = be32(&headers, 28 + base * 8)?;
        }
        Ok(this)
    }

    pub fn at(&self, address: u32, len: usize) -> Option<&[u8]> {
        let offset = address.checked_sub(self.base_address)? as usize;
        self.image.get(offset..offset.checked_add(len)?)
    }
}

fn apply_records(records: &[u8], window_size: usize, dest: &mut Vec<u8>) -> Result<(), String> {
    let mut p = 0;
    while p + 12 <= records.len() {
        let (old, new) = (be32(records, p)? as usize, be32(records, p + 4)? as usize);
        let (len, packed) = (be16(records, p + 8)? as usize, be16(records, p + 10)? as usize);
        if old == 0 && new == 0 && len == 0 && packed == 0 {
            break;
        }
        p += 12;
        if dest.len() < new + len {
            dest.resize(new + len, 0);
        }
        match packed {
            0 => dest[new..new + len].fill(0),
            1 => {
                if old + len > dest.len() {
                    return Err("XEX patch copies from outside the image".into());
                }
                dest.copy_within(old..old + len, new);
            }
            _ => {
                let reference = dest.get(old..old + len).ok_or("XEX patch reads outside the image")?.to_vec();
                let unpacked = lzx::decompress_after(span(records, p, packed)?, window_size, len, true, &reference)?;
                dest[new..new + len].copy_from_slice(&unpacked);
                p += packed;
            }
        }
    }
    Ok(())
}

fn basic(xex: &[u8], format: usize, payload: &[u8], image_size: usize) -> Result<Vec<u8>, String> {
    let info_size = be32(xex, format)? as usize;
    let blocks = info_size.saturating_sub(8) / 8;
    let mut image = Vec::with_capacity(image_size);
    let mut at = 0;
    for i in 0..blocks {
        let data = be32(xex, format + 8 + i * 8)? as usize;
        let zeros = be32(xex, format + 12 + i * 8)? as usize;
        image.extend_from_slice(span(payload, at, data)?);
        at += data;
        image.resize(image.len() + zeros, 0);
    }
    image.resize(image_size, 0);
    Ok(image)
}

fn normal(xex: &[u8], format: usize, payload: &[u8], image_size: usize) -> Result<Vec<u8>, String> {
    let window_size = be32(xex, format + 8)? as usize;
    let mut block_size = be32(xex, format + 12)? as usize;
    let mut at = 0;
    let mut stream = Vec::with_capacity(payload.len());
    while block_size != 0 {
        let block = span(payload, at, block_size)?;
        let next = be32(block, 0)? as usize;
        let mut p = 24;
        loop {
            let chunk = be16(block, p)? as usize;
            p += 2;
            if chunk == 0 {
                break;
            }
            stream.extend_from_slice(span(block, p, chunk)?);
            p += chunk;
        }
        at += block_size;
        block_size = next;
    }
    lzx::decompress(&stream, window_size, image_size, true)
}
