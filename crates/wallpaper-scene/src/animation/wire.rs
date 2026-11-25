use super::*;
const MAGIC: &[u8; 8] = b"SAHNANM\0";
pub const MAX_BYTES: usize = crate::MAX_SCENE_BYTES + 29 + MAX_TRACKS * 7 + MAX_KEYS * 8;

pub(super) fn encode(value: &ValidatedAnimation) -> Vec<u8> {
    let scene = value.base.encode();
    let data = &value.data;
    let mut bytes = Vec::with_capacity(scene.len() + 29 + data.tracks.len() * 7 + MAX_KEYS * 8);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    put(&mut bytes, scene.len() as u32);
    bytes.extend_from_slice(&scene);
    put(&mut bytes, data.duration_ms);
    put(&mut bytes, data.delay_ms);
    put(
        &mut bytes,
        match data.iterations {
            Iterations::Forever => 0,
            Iterations::Count(n) => n,
        },
    );
    bytes.push(u8::from(data.alternate));
    bytes.extend_from_slice(&(data.tracks.len() as u16).to_le_bytes());
    for track in &data.tracks {
        put(&mut bytes, track.target);
        bytes.push(match track.property {
            Property::TranslateX => 1,
            Property::TranslateY => 2,
            Property::ScaleX => 3,
            Property::ScaleY => 4,
            Property::Opacity => 5,
        });
        bytes.extend_from_slice(&(track.keys.len() as u16).to_le_bytes());
        for key in &track.keys {
            put(&mut bytes, key.time_ms);
            put(&mut bytes, key.value.to_bits());
        }
    }
    bytes
}
fn put(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(error(Code::Budget, None))?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or(error(Code::Encoding, None))?;
        self.pos = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}
/// Self-contained base scene and program, independently revalidated; no I/O/assets.
pub fn decode(bytes: &[u8]) -> Result<ValidatedAnimation> {
    if bytes.len() > MAX_BYTES {
        return Err(error(Code::Budget, None));
    }
    let mut reader = Reader { bytes, pos: 0 };
    if reader.take(8)? != MAGIC {
        return Err(error(Code::Encoding, None));
    }
    let version = reader.u16()?;
    if version != VERSION {
        return Err(error(Code::Version, None));
    }
    let scene_len = reader.u32()? as usize;
    if scene_len > crate::MAX_SCENE_BYTES {
        return Err(error(Code::Budget, None));
    }
    let base =
        hierarchy::decode(reader.take(scene_len)?).map_err(|_| error(Code::Encoding, None))?;
    let duration_ms = reader.u32()?;
    let delay_ms = reader.u32()?;
    let iterations = match reader.u32()? {
        0 => Iterations::Forever,
        n => Iterations::Count(n),
    };
    let alternate = match reader.byte()? {
        0 => false,
        1 => true,
        _ => return Err(error(Code::Encoding, None)),
    };
    let count = reader.u16()? as usize;
    if count == 0 || count > MAX_TRACKS {
        return Err(error(Code::Budget, None));
    }
    let mut tracks = Vec::with_capacity(count);
    let mut total = 0usize;
    for index in 0..count {
        let target = reader.u32()?;
        let property = match reader.byte()? {
            1 => Property::TranslateX,
            2 => Property::TranslateY,
            3 => Property::ScaleX,
            4 => Property::ScaleY,
            5 => Property::Opacity,
            _ => return Err(error(Code::Encoding, Some(index))),
        };
        let count = reader.u16()? as usize;
        total = total
            .checked_add(count)
            .ok_or(error(Code::Budget, Some(index)))?;
        if !(2..=MAX_KEYS_PER_TRACK).contains(&count) || total > MAX_KEYS {
            return Err(error(Code::Budget, Some(index)));
        }
        // Check the complete payload before allocating this key vector.
        let payload = reader.take(count * 8)?;
        let keys = payload
            .as_chunks::<8>()
            .0
            .iter()
            .map(|chunk| Key {
                time_ms: u32::from_le_bytes(chunk[..4].try_into().unwrap()),
                value: f32::from_bits(u32::from_le_bytes(chunk[4..].try_into().unwrap())),
            })
            .collect();
        tracks.push(Track {
            target,
            property,
            keys,
        });
    }
    if reader.pos != bytes.len() {
        return Err(error(Code::Encoding, None));
    }
    Animation {
        version,
        duration_ms,
        delay_ms,
        iterations,
        alternate,
        tracks,
    }
    .validate(base)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{base, program, track};
    use super::*;
    #[test]
    fn animation_wire_roundtrip_and_every_truncation() {
        let value = program(vec![
            track(1, Property::TranslateX, -10.0, 30.0),
            track(1, Property::Opacity, 0.0, 1.0),
        ])
        .validate(base())
        .unwrap();
        let bytes = value.encode();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.base(), value.base());
        assert_eq!(decoded.animation(), value.animation());
        assert_eq!(decoded.encode(), bytes);
        for end in 0..bytes.len() {
            assert!(decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err());
        assert_eq!(
            decoded.sample(Duration::from_millis(500)).unwrap().scene,
            value.sample(Duration::from_millis(500)).unwrap().scene
        );
    }
    #[test]
    fn wire_lengths_tags_flags_and_versions_fail_closed() {
        let bytes = program(vec![track(1, Property::Opacity, 0.0, 1.0)])
            .validate(base())
            .unwrap()
            .encode();
        let scene_len = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        for (offset, value) in [
            (0, 0),
            (8, 2),
            (14 + scene_len + 12, 2),
            (14 + scene_len + 15 + 4, 255),
        ] {
            let mut bad = bytes.clone();
            bad[offset] = value;
            assert!(decode(&bad).is_err());
        }
        let mut bad = bytes.clone();
        bad[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode(&bad).unwrap_err().code, Code::Budget);
        let mut bad = bytes;
        bad[14 + scene_len + 13..14 + scene_len + 15].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(decode(&bad).unwrap_err().code, Code::Budget);
        assert_eq!(
            decode(&vec![0; MAX_BYTES + 1]).unwrap_err().code,
            Code::Budget
        );
    }
    #[test]
    fn deterministic_wire_mutations_never_skip_revalidation() {
        let bytes = program(vec![track(1, Property::ScaleX, 0.0, 2.0)])
            .validate(base())
            .unwrap()
            .encode();
        for offset in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[offset] ^= 0xA5;
            if let Ok(value) = decode(&changed) {
                value
                    .animation()
                    .clone()
                    .validate(hierarchy::decode(&value.base().encode()).unwrap())
                    .unwrap();
                for ms in [0, 250, 500, 1000] {
                    value.sample(Duration::from_millis(ms)).unwrap();
                }
            }
        }
    }
}
