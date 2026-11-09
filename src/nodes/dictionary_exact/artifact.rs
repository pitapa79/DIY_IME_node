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

struct Row<'a> {
    hash: u64,
    reading: &'a str,
    surface: &'a str,
    score: f64,
    line: usize,
}

pub fn prepare(config: &serde_json::Value, original: &[u8]) -> Result<Vec<u8>, CoreError> {
    let path = super::path_config(config)?;
    let mut rows = Vec::with_capacity((original.len() / 16).min(MAX_ROWS));
    let parsed = scan_tsv(original, |reading, surface, score, line| {
        rows.push(Row {
            hash: hash(reading),
            reading,
            surface,
            score,
            line,
        });
        Ok(())
    });
    let mut order: Vec<_> = rows.iter().collect();
    if let Some(first) = order.first().copied()
        && !order.is_sorted_by_key(|row| row.hash)
    {
        // Stable byte passes move only row references. Skip invariant bytes.
        let varying = order
            .iter()
            .fold(0, |bits, row| bits | (row.hash ^ first.hash));
        let mut scratch = order.clone();
        for shift in (0..64).step_by(8) {
            if varying & (0xff << shift) == 0 {
                continue;
            }
            let byte = |row: &Row<'_>| ((row.hash >> shift) & 0xff) as usize;
            let mut slots = [0; 256];
            for row in &order {
                slots[byte(row)] += 1;
            }
            let mut next = 0;
            for slot in &mut slots {
                let count = *slot;
                *slot = next;
                next += count;
            }
            for row in &order {
                let slot = &mut slots[byte(row)];
                scratch[*slot] = row;
                *slot += 1;
            }
            std::mem::swap(&mut order, &mut scratch);
        }
    }
    for ties in order.chunk_by_mut(|a, b| a.hash == b.hash) {
        ties.sort_unstable_by(|a, b| a.reading.cmp(b.reading).then_with(|| a.line.cmp(&b.line)));
    }
    let groups = || order.chunk_by(|a, b| a.hash == b.hash && a.reading == b.reading);
    let mut count = 0;
    let base = HEADER_BYTES + path.len();
    let mut length = base;
    let mut first_error: Option<(usize, CoreError)> = None;
    for entries in groups() {
        count += 1;
        length += ENTRY_BYTES + entries[0].reading.len() + 4;
        for (index, entry) in entries.iter().enumerate() {
            let duplicate = entries[..index]
                .iter()
                .find(|other| other.surface == entry.surface);
            if duplicate.is_some() || index == MAX_CANDIDATES {
                if first_error
                    .as_ref()
                    .is_none_or(|(line, _)| entry.line < *line)
                {
                    let error = match duplicate {
                        Some(first) => format_error(
                            entry.line,
                            "reading/surface",
                            &format!("duplicate pair, first occurrence at line {}", first.line),
                        ),
                        None => limit_error(entry.line, "candidates per reading", MAX_CANDIDATES),
                    };
                    first_error = Some((entry.line, error));
                }
                break;
            }
            length += 12 + entry.surface.len();
        }
    }
    // A structural parse error stops the scan before adding that row.
    // A conflict in the scanned prefix therefore takes precedence.
    if let Some((_, error)) = first_error {
        return Err(error);
    }
    parsed?;
    if length > MAX_ARTIFACT_BYTES {
        return Err(limit_error(
            0,
            "prepared artifact bytes",
            MAX_ARTIFACT_BYTES,
        ));
    }
    let data_start = base + count * ENTRY_BYTES;
    let mut output = Vec::with_capacity(length);
    output.resize(data_start, 0);
    output[..8].copy_from_slice(MAGIC);
    output[8..12].copy_from_slice(&VERSION.to_le_bytes());
    output[12..16].copy_from_slice(&(rows.len() as u32).to_le_bytes());
    output[16..20].copy_from_slice(&(count as u32).to_le_bytes());
    output[20..24].copy_from_slice(&(path.len() as u32).to_le_bytes());
    output[24..32].copy_from_slice(&(length as u64).to_le_bytes());
    output[HEADER_BYTES..base].copy_from_slice(path.as_bytes());
    for (index, entries) in groups().enumerate() {
        let first = &entries[0];
        let reading_offset = output.len() as u64;
        output.extend_from_slice(first.reading.as_bytes());
        let candidates_offset = output.len() as u64;
        output.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for entry in entries {
            output.extend_from_slice(&(entry.surface.len() as u32).to_le_bytes());
            output.extend_from_slice(entry.surface.as_bytes());
            output.extend_from_slice(&entry.score.to_le_bytes());
        }
        let candidate_length = (output.len() as u64 - candidates_offset) as u32;
        let slot = base + index * ENTRY_BYTES;
        output[slot..slot + 8].copy_from_slice(&first.hash.to_le_bytes());
        output[slot + 8..slot + 16].copy_from_slice(&reading_offset.to_le_bytes());
        output[slot + 16..slot + 20].copy_from_slice(&(first.reading.len() as u32).to_le_bytes());
        output[slot + 20..slot + 28].copy_from_slice(&candidates_offset.to_le_bytes());
        output[slot + 28..slot + 32].copy_from_slice(&candidate_length.to_le_bytes());
    }
    debug_assert_eq!(output.len(), length);
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
