//! Node-owned opaque format. Only already validated TSV dictionaries can build it.
//! Hashes guide a bounded binary search; full readings decide equality.
use super::*;
use std::cmp::Ordering;

pub const FORMAT: &str = "dictionary-exact-index";
pub const VERSION: u32 = 1;
pub const MAX_ARTIFACT_BYTES: usize = 4 * MAX_FILE_BYTES;
const MAGIC: &[u8; 8] = b"DIYDIC01";
const HEADER_BYTES: usize = 32;
const ENTRY_BYTES: usize = 32;

pub trait ReadArtifact {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn read_exact(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, CoreError>;
}
impl ReadArtifact for diy_ime2_wasm_resource_sdk::Resource {
    fn len(&self) -> u64 {
        self.len()
    }
    fn read_exact(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, CoreError> {
        diy_ime2_wasm_resource_sdk::Resource::read_exact(self, offset, length)
            .map_err(|_| CoreError::new("dictionary-read", "prepared resource read failed"))
    }
}
fn invalid() -> CoreError {
    CoreError::new("dictionary-not-prepared", "invalid prepared dictionary")
}
fn hash(reading: &str) -> u64 {
    reading.chars().fold(0u64, |h, c| {
        h.wrapping_mul(31).wrapping_add(u64::from(c as u32))
    })
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, CoreError> {
    bytes
        .get(offset..offset.checked_add(4).ok_or_else(invalid)?)
        .ok_or_else(invalid)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}
fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, CoreError> {
    bytes
        .get(offset..offset.checked_add(8).ok_or_else(invalid)?)
        .ok_or_else(invalid)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
}
fn append(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), CoreError> {
    if output
        .len()
        .checked_add(bytes.len())
        .is_none_or(|end| end > MAX_ARTIFACT_BYTES)
    {
        return Err(limit_error(
            0,
            "prepared artifact bytes",
            MAX_ARTIFACT_BYTES,
        ));
    }
    output.extend_from_slice(bytes);
    Ok(())
}

/// Existing parser and every original limit run before any artifact is returned.
pub fn prepare(config: &serde_json::Value, original: &[u8]) -> Result<Vec<u8>, CoreError> {
    let path = super::path_config(config)?;
    let dictionary = ExactDictionary::from_tsv_bytes(original)?;
    let mut groups = dictionary
        .entries
        .iter()
        .map(|(reading, entries)| (hash(reading), reading, entries))
        .collect::<Vec<_>>();
    groups.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    let rows = groups
        .iter()
        .map(|(_, _, entries)| entries.len())
        .sum::<usize>();
    let base = HEADER_BYTES.checked_add(path.len()).ok_or_else(invalid)?;
    let data_start = base
        .checked_add(groups.len().checked_mul(ENTRY_BYTES).ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    if data_start > MAX_ARTIFACT_BYTES {
        return Err(invalid());
    }
    let mut output = vec![0; data_start];
    output[..8].copy_from_slice(MAGIC);
    output[8..12].copy_from_slice(&VERSION.to_le_bytes());
    output[12..16].copy_from_slice(&(rows as u32).to_le_bytes());
    output[16..20].copy_from_slice(&(groups.len() as u32).to_le_bytes());
    output[20..24].copy_from_slice(&(path.len() as u32).to_le_bytes());
    output[HEADER_BYTES..base].copy_from_slice(path.as_bytes());
    for (index, (key, reading, entries)) in groups.into_iter().enumerate() {
        let reading_offset = output.len() as u64;
        append(&mut output, reading.as_bytes())?;
        let candidates_offset = output.len() as u64;
        append(&mut output, &(entries.len() as u32).to_le_bytes())?;
        for entry in entries {
            append(&mut output, &(entry.surface.len() as u32).to_le_bytes())?;
            append(&mut output, entry.surface.as_bytes())?;
            append(&mut output, &entry.score.to_le_bytes())?;
        }
        let candidate_length =
            u32::try_from(output.len() as u64 - candidates_offset).map_err(|_| invalid())?;
        let slot = base + index * ENTRY_BYTES;
        output[slot..slot + 8].copy_from_slice(&key.to_le_bytes());
        output[slot + 8..slot + 16].copy_from_slice(&reading_offset.to_le_bytes());
        output[slot + 16..slot + 20].copy_from_slice(&(reading.len() as u32).to_le_bytes());
        output[slot + 20..slot + 28].copy_from_slice(&candidates_offset.to_le_bytes());
        output[slot + 28..slot + 32].copy_from_slice(&candidate_length.to_le_bytes());
    }
    let length = output.len() as u64;
    output[24..32].copy_from_slice(&length.to_le_bytes());
    Ok(output)
}

fn span(reader: &mut impl ReadArtifact, offset: u64, length: usize) -> Result<Vec<u8>, CoreError> {
    if length > MAX_ARTIFACT_BYTES
        || offset
            .checked_add(length as u64)
            .is_none_or(|end| end > reader.len())
    {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        let count = (length - bytes.len()).min(diy_ime2_wasm_resource_sdk::MAX_CHUNK_BYTES);
        let part = reader.read_exact(offset + bytes.len() as u64, count)?;
        if part.len() != count {
            return Err(invalid());
        }
        bytes.extend(part);
    }
    Ok(bytes)
}
struct Header {
    rows: usize,
    groups: usize,
    base: u64,
    data: u64,
}
fn header(reader: &mut impl ReadArtifact, path: &str) -> Result<Header, CoreError> {
    if reader.len() < HEADER_BYTES as u64 || reader.len() > MAX_ARTIFACT_BYTES as u64 {
        return Err(invalid());
    }
    let bytes = span(reader, 0, HEADER_BYTES)?;
    if &bytes[..8] != MAGIC || u32_at(&bytes, 8)? != VERSION || u64_at(&bytes, 24)? != reader.len()
    {
        return Err(invalid());
    }
    let rows = u32_at(&bytes, 12)? as usize;
    let groups = u32_at(&bytes, 16)? as usize;
    let path_len = u32_at(&bytes, 20)? as usize;
    if rows > MAX_ROWS
        || groups > rows
        || (groups == 0) != (rows == 0)
        || path_len == 0
        || path_len > 4096
    {
        return Err(invalid());
    }
    let actual = span(reader, HEADER_BYTES as u64, path_len)?;
    if actual != path.as_bytes() {
        return Err(CoreError::new(
            "dictionary-not-prepared",
            "prepared dictionary belongs to another config",
        ));
    }
    let base = (HEADER_BYTES + path_len) as u64;
    let data = base
        .checked_add(
            (groups as u64)
                .checked_mul(ENTRY_BYTES as u64)
                .ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)?;
    if data > reader.len() {
        return Err(invalid());
    }
    Ok(Header {
        rows,
        groups,
        base,
        data,
    })
}
fn text(bytes: &[u8]) -> Result<&str, CoreError> {
    let value = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    if value.is_empty()
        || value.chars().count() > MAX_CODE_POINTS
        || value
            .chars()
            .any(|c| matches!(c, '\0' | '\t' | '\r' | '\n'))
    {
        return Err(invalid());
    }
    Ok(value)
}
fn data_range(
    reader: &impl ReadArtifact,
    h: &Header,
    offset: u64,
    length: usize,
) -> Result<(), CoreError> {
    if offset < h.data
        || length == 0
        || offset
            .checked_add(length as u64)
            .is_none_or(|end| end > reader.len())
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn candidates(bytes: &[u8], rows: usize) -> Result<Packet, CoreError> {
    let count = u32_at(bytes, 0)? as usize;
    if count == 0 || count > MAX_CANDIDATES || count > rows {
        return Err(invalid());
    }
    let mut options = Vec::with_capacity(count);
    let mut position = 4usize;
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..count {
        let length = u32_at(bytes, position)? as usize;
        position = position.checked_add(4).ok_or_else(invalid)?;
        let end = position.checked_add(length).ok_or_else(invalid)?;
        let surface = text(bytes.get(position..end).ok_or_else(invalid)?)?;
        if !seen.insert(surface) {
            return Err(invalid());
        }
        position = end;
        let score = f64::from_bits(u64_at(bytes, position)?);
        position = position.checked_add(8).ok_or_else(invalid)?;
        if !score.is_finite() {
            return Err(invalid());
        }
        options.push(ChoiceOption {
            value: Candidate::Text {
                text: surface.into(),
                meta: None,
            },
            score: Some(score),
        });
    }
    if position != bytes.len() {
        return Err(invalid());
    }
    Ok(Packet::new(Candidate::Choice { options }))
}

/// Lookup stays O(log readings), including intentionally colliding hashes.
/// No input failure, resource failure or partial candidate block becomes a miss.
pub fn lookup(
    reader: &mut impl ReadArtifact,
    config: &serde_json::Value,
    input: &OutputValue,
) -> Result<OutputValue, CoreError> {
    let path = super::path_config(config)?;
    let h = header(reader, path)?;
    // Reuse the canonical validation and unknown/empty/NoOutput behavior.
    let fallback = ExactDictionary {
        entries: BTreeMap::new(),
    }
    .lookup_output(input)?;
    let OutputValue::Data(packet) = input else {
        return Ok(fallback);
    };
    let Candidate::Text { text: query, .. } = &packet.root else {
        return Err(invalid());
    };
    let key = hash(query);
    let (mut low, mut high) = (0usize, h.groups);
    while low < high {
        let mid = low + (high - low) / 2;
        let entry = span(
            reader,
            h.base + mid as u64 * ENTRY_BYTES as u64,
            ENTRY_BYTES,
        )?;
        let stored = u64_at(&entry, 0)?;
        let ordering = match stored.cmp(&key) {
            Ordering::Equal => {
                let offset = u64_at(&entry, 8)?;
                let length = u32_at(&entry, 16)? as usize;
                if length > MAX_CODE_POINTS * 4 {
                    return Err(invalid());
                }
                data_range(reader, &h, offset, length)?;
                let bytes = span(reader, offset, length)?;
                let reading = text(&bytes)?;
                if hash(reading) != stored {
                    return Err(invalid());
                }
                reading.cmp(query)
            }
            other => other,
        };
        match ordering {
            Ordering::Less => low = mid + 1,
            Ordering::Greater => high = mid,
            Ordering::Equal => {
                let offset = u64_at(&entry, 20)?;
                let length = u32_at(&entry, 28)? as usize;
                data_range(reader, &h, offset, length)?;
                let bytes = span(reader, offset, length)?;
                return candidates(&bytes, h.rows)
                    .map(|packet| OutputValue::Data(Arc::new(packet)));
            }
        }
    }
    Ok(fallback)
}

#[cfg(test)]
mod tests;
