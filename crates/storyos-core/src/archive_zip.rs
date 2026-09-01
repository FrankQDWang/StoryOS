//! Deterministic ZIP STORE bytes for a verified Project Export Archive.

use crate::ProjectArchiveBuildRefusal;

pub(crate) fn store_zip(files: &[(&str, &[u8])]) -> Result<Vec<u8>, ProjectArchiveBuildRefusal> {
    let mut locals = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let name_bytes = name.as_bytes();
        let local_offset = u32_len(locals.len())?;
        let crc = crc32(data);
        let size = u32_len(data.len())?;
        let name_len = u16_len(name_bytes.len())?;
        locals.extend_from_slice(b"PK\x03\x04");
        locals.extend_from_slice(&20u16.to_le_bytes());
        locals.extend_from_slice(&0u16.to_le_bytes());
        locals.extend_from_slice(&0u16.to_le_bytes());
        locals.extend_from_slice(&0u16.to_le_bytes());
        locals.extend_from_slice(&0x0021u16.to_le_bytes());
        locals.extend_from_slice(&crc.to_le_bytes());
        locals.extend_from_slice(&size.to_le_bytes());
        locals.extend_from_slice(&size.to_le_bytes());
        locals.extend_from_slice(&name_len.to_le_bytes());
        locals.extend_from_slice(&0u16.to_le_bytes());
        locals.extend_from_slice(name_bytes);
        locals.extend_from_slice(data);
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0x0021u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&local_offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let central_offset = u32_len(locals.len())?;
    let central_size = u32_len(central.len())?;
    let entry_count = u16_len(files.len())?;
    locals.extend_from_slice(&central);
    locals.extend_from_slice(b"PK\x05\x06");
    locals.extend_from_slice(&0u16.to_le_bytes());
    locals.extend_from_slice(&0u16.to_le_bytes());
    locals.extend_from_slice(&entry_count.to_le_bytes());
    locals.extend_from_slice(&entry_count.to_le_bytes());
    locals.extend_from_slice(&central_size.to_le_bytes());
    locals.extend_from_slice(&central_offset.to_le_bytes());
    locals.extend_from_slice(&0u16.to_le_bytes());
    Ok(locals)
}

fn u16_len(len: usize) -> Result<u16, ProjectArchiveBuildRefusal> {
    u16::try_from(len).map_err(|_| ProjectArchiveBuildRefusal::InvalidProvenance)
}

fn u32_len(len: usize) -> Result<u32, ProjectArchiveBuildRefusal> {
    u32::try_from(len).map_err(|_| ProjectArchiveBuildRefusal::InvalidProvenance)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let lowest = crc & 1;
            crc >>= 1;
            if lowest == 1 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}
