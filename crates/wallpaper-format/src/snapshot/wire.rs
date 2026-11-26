use super::{PackageSnapshot, RelativePath, file_limit};
use crate::{MAX_ENTRIES, MAX_PACKAGE_BYTES, path::MAX_PATH_BYTES};
use std::io::{self, Read, Write};

const SIGNATURE: &[u8; 4] = b"SPS\x01";

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

impl PackageSnapshot {
    /// Write one snapshot to a stream without parsing its source files.
    pub fn write_to(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(SIGNATURE)?;
        writer.write_all(&(self.entries.len() as u32).to_le_bytes())?;
        for (path, bytes) in &self.entries {
            writer.write_all(&(path.len() as u16).to_le_bytes())?;
            writer.write_all(path.as_bytes())?;
            writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
            writer.write_all(bytes)?;
        }
        Ok(())
    }

    /// Read one snapshot, leaving later messages in the stream. The caller sets
    /// the I/O deadline and must end the exchange on a decoding error.
    pub fn read_from(mut reader: impl Read) -> io::Result<Self> {
        let mut word = [0; 4];
        reader.read_exact(&mut word)?;
        if &word != SIGNATURE {
            return Err(invalid("Unknown package snapshot format"));
        }
        reader.read_exact(&mut word)?;
        let count = u32::from_le_bytes(word) as usize;
        if count > MAX_ENTRIES {
            return Err(invalid("Package snapshot entry limit exceeded"));
        }

        let mut entries = Vec::with_capacity(count);
        let mut remaining = MAX_PACKAGE_BYTES;
        let mut path_buffer = [0; MAX_PATH_BYTES];
        for _ in 0..count {
            let mut short = [0; 2];
            reader.read_exact(&mut short)?;
            let path_len = u16::from_le_bytes(short) as usize;
            if !(1..=MAX_PATH_BYTES).contains(&path_len) {
                return Err(invalid("Invalid package snapshot path length"));
            }
            let path = &mut path_buffer[..path_len];
            reader.read_exact(path)?;
            let relative = RelativePath::parse(
                std::str::from_utf8(path).map_err(|_| invalid("Invalid package snapshot path"))?,
            )
            .map_err(|_| invalid("Invalid package snapshot path"))?;
            reader.read_exact(&mut word)?;
            let len = u32::from_le_bytes(word) as usize;
            if len > file_limit(&relative, remaining) {
                return Err(invalid("Package snapshot byte limit exceeded"));
            }
            let mut bytes = vec![0; len];
            reader.read_exact(&mut bytes)?;
            remaining -= len;
            entries.push((relative.as_str().to_owned(), bytes));
        }
        Ok(Self { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, MAX_MANIFEST_BYTES};

    #[test]
    fn round_trip_defers_parsing_and_preserves_following_bytes() {
        let snapshot =
            PackageSnapshot::from_readers([Ok(("manifest.json".into(), &b"{"[..]))]).unwrap();
        let mut encoded = Vec::new();
        snapshot.write_to(&mut encoded).unwrap();
        for end in 0..encoded.len() {
            assert!(PackageSnapshot::read_from(&encoded[..end]).is_err());
        }
        encoded.extend_from_slice(b"next");
        let mut stream = encoded.as_slice();
        let decoded = PackageSnapshot::read_from(&mut stream).unwrap();
        assert_eq!(decoded.entries, snapshot.entries);
        assert_eq!(stream, b"next");
        assert_eq!(decoded.parse().unwrap_err().code, ErrorCode::ManifestSyntax);
    }

    #[test]
    fn rejects_declared_sizes_without_reading_payloads() {
        let mut encoded = SIGNATURE.to_vec();
        encoded.extend_from_slice(&((MAX_ENTRIES + 1) as u32).to_le_bytes());
        assert_eq!(
            PackageSnapshot::read_from(&encoded[..]).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        encoded.truncate(SIGNATURE.len());
        encoded.extend_from_slice(&1u32.to_le_bytes());
        encoded.extend_from_slice(&13u16.to_le_bytes());
        encoded.extend_from_slice(b"manifest.json");
        encoded.extend_from_slice(&((MAX_MANIFEST_BYTES + 1) as u32).to_le_bytes());
        assert_eq!(
            PackageSnapshot::read_from(&encoded[..]).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
