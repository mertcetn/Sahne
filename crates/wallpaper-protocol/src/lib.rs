//! Bounded transport-neutral control frames. This is not an OS sandbox or a scene validator.
use std::{
    fmt,
    io::{self, Read, Write},
};
mod scene;
pub use scene::{SCENE_REJECT_CODE, accept_scene_batch};
use wallpaper_scene::transaction::Edit;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_DIAGNOSTIC_BYTES: usize = 256;
const HEADER_BYTES: usize = 27;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "IPC I/O: {error}"),
            Self::Invalid(reason) => write!(f, "Invalid IPC frame: {reason}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub session: u64,
    pub sequence: u64,
    pub base_revision: u64,
    pub body: Body,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Hello { api_version: u16 },
    ProbeResult { denied: u32 },
    Ping { nonce: u64 },
    Pong { nonce: u64 },
    Reject { code: u16, message: String },
    SceneBatch { edits: Vec<Edit> },
    SceneAck { revision: u64 },
}
impl Message {
    /// Check expected envelope state without advancing any host state.
    /// The caller must still validate direction, handshake and body semantics before commit.
    pub fn check_context(&self, session: u64, sequence: u64, revision: u64) -> Result<()> {
        if session == 0
            || sequence == 0
            || self.session != session
            || self.sequence != sequence
            || self.base_revision != revision
        {
            return Err(Error::Invalid("unexpected session, sequence or revision"));
        }
        Ok(())
    }
}

/// Encode completely before emitting bytes. An I/O error may leave a partial stream;
/// the caller must close that transport instead of retrying the whole frame.
pub fn write_frame(writer: &mut impl Write, message: &Message) -> Result<()> {
    if message.session == 0 || message.sequence == 0 {
        return Err(Error::Invalid("zero session or sequence"));
    }
    let mut data = Vec::with_capacity(HEADER_BYTES + MAX_DIAGNOSTIC_BYTES + 4);
    data.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    data.extend_from_slice(&message.session.to_le_bytes());
    data.extend_from_slice(&message.sequence.to_le_bytes());
    data.extend_from_slice(&message.base_revision.to_le_bytes());
    match &message.body {
        Body::ProbeResult { denied } => {
            data.push(5);
            data.extend_from_slice(&denied.to_le_bytes());
        }
        Body::Hello { api_version } => {
            if *api_version != 1 {
                return Err(Error::Invalid("unsupported API version"));
            }
            data.push(1);
            data.extend_from_slice(&api_version.to_le_bytes());
        }
        Body::Ping { nonce } => {
            data.push(2);
            data.extend_from_slice(&nonce.to_le_bytes());
        }
        Body::Pong { nonce } => {
            data.push(3);
            data.extend_from_slice(&nonce.to_le_bytes());
        }
        Body::Reject { code, message } => {
            if message.len() > MAX_DIAGNOSTIC_BYTES {
                return Err(Error::Invalid("diagnostic too long"));
            }
            data.push(4);
            data.extend_from_slice(&code.to_le_bytes());
            data.extend_from_slice(&(message.len() as u16).to_le_bytes());
            data.extend_from_slice(message.as_bytes());
        }
        Body::SceneBatch { edits } => {
            data.push(6);
            scene::encode_edits(&mut data, edits)?;
        }
        Body::SceneAck { revision } => {
            scene::check_ack(message.base_revision, *revision)?;
            data.push(7);
            data.extend_from_slice(&revision.to_le_bytes());
        }
    }
    if data.len() > MAX_FRAME_BYTES {
        return Err(Error::Invalid("frame too long"));
    }
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(&data)?;
    Ok(())
}

/// Clean EOF is allowed only before any prefix byte. All partial frames fail.
/// Length is checked before payload allocation/read. This synchronous codec has no timeout:
/// the future sandbox host must enforce deadlines and bounded transport queues.
pub fn read_frame(reader: &mut impl Read) -> Result<Option<Message>> {
    let mut prefix = [0u8; 4];
    loop {
        match reader.read(&mut prefix[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    reader.read_exact(&mut prefix[1..])?;
    let len = u32::from_le_bytes(prefix) as usize;
    if !(HEADER_BYTES..=MAX_FRAME_BYTES).contains(&len) {
        return Err(Error::Invalid("frame length outside bounds"));
    }
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    let mut data = Decoder {
        bytes: &bytes,
        pos: 0,
    };
    if data.u16()? != PROTOCOL_VERSION {
        return Err(Error::Invalid("unsupported protocol version"));
    }
    let session = data.u64()?;
    let sequence = data.u64()?;
    let base_revision = data.u64()?;
    if session == 0 || sequence == 0 {
        return Err(Error::Invalid("zero session or sequence"));
    }
    let body = match data.array::<1>()?[0] {
        1 => {
            let api_version = data.u16()?;
            if api_version != 1 {
                return Err(Error::Invalid("unsupported API version"));
            }
            Body::Hello { api_version }
        }
        2 => Body::Ping { nonce: data.u64()? },
        3 => Body::Pong { nonce: data.u64()? },
        4 => {
            let code = data.u16()?;
            let size = data.u16()? as usize;
            if size > MAX_DIAGNOSTIC_BYTES {
                return Err(Error::Invalid("diagnostic too long"));
            }
            let text = std::str::from_utf8(data.take(size)?)
                .map_err(|_| Error::Invalid("diagnostic is not UTF-8"))?;
            Body::Reject {
                code,
                message: text.to_owned(),
            }
        }
        5 => Body::ProbeResult {
            denied: u32::from_le_bytes(data.array()?),
        },
        6 => Body::SceneBatch {
            edits: scene::decode_edits(&mut data)?,
        },
        7 => {
            let revision = data.u64()?;
            scene::check_ack(base_revision, revision)?;
            Body::SceneAck { revision }
        }
        _ => return Err(Error::Invalid("unknown message kind")),
    };
    if data.pos != bytes.len() {
        return Err(Error::Invalid("trailing bytes"));
    }
    Ok(Some(Message {
        session,
        sequence,
        base_revision,
        body,
    }))
}
struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Decoder<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(Error::Invalid("length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or(Error::Invalid("truncated body"))?;
        self.pos = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?
            .try_into()
            .map_err(|_| Error::Invalid("truncated integer"))
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn sample(body: Body) -> Message {
        Message {
            session: 42,
            sequence: 1,
            base_revision: 0,
            body,
        }
    }
    fn encoded(message: &Message) -> Vec<u8> {
        let mut out = Vec::new();
        write_frame(&mut out, message).unwrap();
        out
    }
    #[test]
    fn ping_matches_documented_wire_bytes() {
        let bytes = vec![
            35, 0, 0, 0, 1, 0, 42, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 2, 123, 0, 0, 0, 0, 0, 0, 0,
        ];
        let message = sample(Body::Ping { nonce: 123 });
        assert_eq!(encoded(&message), bytes);
        assert_eq!(read_frame(&mut Cursor::new(bytes)).unwrap(), Some(message));
    }
    #[test]
    fn concatenated_frames_and_clean_eof_preserve_boundaries() {
        let messages = [
            sample(Body::Hello { api_version: 1 }),
            sample(Body::Ping { nonce: u64::MAX }),
            sample(Body::Pong { nonce: u64::MAX }),
            sample(Body::ProbeResult { denied: u32::MAX }),
            sample(Body::Reject {
                code: 2,
                message: "unsupported capability".into(),
            }),
        ];
        let mut stream = Cursor::new(messages.iter().flat_map(encoded).collect::<Vec<_>>());
        for expected in messages {
            assert_eq!(read_frame(&mut stream).unwrap(), Some(expected));
        }
        assert!(read_frame(&mut stream).unwrap().is_none());
    }
    #[test]
    fn probe_result_has_exact_bounded_body() {
        let message = sample(Body::ProbeResult { denied: 23 });
        let bytes = encoded(&message);
        assert_eq!(&bytes[..4], &31u32.to_le_bytes());
        assert_eq!(&bytes[30..], &[5, 23, 0, 0, 0]);
        assert_eq!(read_frame(&mut Cursor::new(&bytes)).unwrap(), Some(message));
        let mut short = bytes.clone();
        short.pop();
        short[..4].copy_from_slice(&30u32.to_le_bytes());
        assert!(read_frame(&mut Cursor::new(short)).is_err());
        let mut extra = bytes;
        extra.push(0);
        extra[..4].copy_from_slice(&32u32.to_le_bytes());
        assert!(read_frame(&mut Cursor::new(extra)).is_err());
    }
    #[test]
    fn every_truncation_is_an_error_not_clean_eof() {
        let frame = encoded(&sample(Body::Ping { nonce: 55 }));
        for end in 1..frame.len() {
            assert!(
                read_frame(&mut Cursor::new(&frame[..end])).is_err(),
                "accepted prefix of length {end}"
            );
        }
    }
    #[test]
    fn invalid_lengths_are_rejected_before_reading_payload() {
        for length in [0u32, 26, MAX_FRAME_BYTES as u32 + 1, u32::MAX] {
            let mut input = Cursor::new(length.to_le_bytes());
            assert!(matches!(read_frame(&mut input), Err(Error::Invalid(_))));
            assert_eq!(input.position(), 4);
        }
    }
    #[test]
    fn rejects_versions_unknown_tags_zero_ids_and_trailing_data() {
        let valid = encoded(&sample(Body::Ping { nonce: 55 }));
        for (offset, value) in [(4, 2), (30, 255), (6, 0), (14, 0)] {
            let mut bad = valid.clone();
            bad[offset] = value;
            assert!(read_frame(&mut Cursor::new(bad)).is_err());
        }
        let mut extra = valid;
        extra.push(0);
        let size = (extra.len() - 4) as u32;
        extra[..4].copy_from_slice(&size.to_le_bytes());
        assert!(read_frame(&mut Cursor::new(extra)).is_err());
    }
    #[test]
    fn diagnostic_budget_counts_utf8_bytes_and_validates_encoding() {
        let mut message = sample(Body::Reject {
            code: 1,
            message: "é".repeat(128),
        });
        assert_eq!(
            read_frame(&mut Cursor::new(encoded(&message))).unwrap(),
            Some(message.clone())
        );
        message.body = Body::Reject {
            code: 1,
            message: "é".repeat(129),
        };
        let mut output = Vec::new();
        assert!(write_frame(&mut output, &message).is_err());
        assert!(output.is_empty());
        let mut invalid = encoded(&sample(Body::Reject {
            code: 1,
            message: "a".into(),
        }));
        *invalid.last_mut().unwrap() = 255;
        assert!(read_frame(&mut Cursor::new(invalid)).is_err());
    }
    #[test]
    fn envelope_replay_or_wrong_context_never_matches() {
        let message = sample(Body::Ping { nonce: 1 });
        assert!(message.check_context(42, 1, 0).is_ok());
        for context in [(43, 1, 0), (42, 2, 0), (42, 1, 1), (0, 1, 0)] {
            assert!(
                message
                    .check_context(context.0, context.1, context.2)
                    .is_err()
            );
        }
    }
    #[test]
    fn fragmented_io_and_interrupted_reads_are_supported() {
        struct Fragmented {
            bytes: Cursor<Vec<u8>>,
            interrupted: bool,
        }
        impl Read for Fragmented {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let n = out.len().min(1);
                self.bytes.read(&mut out[..n])
            }
        }
        let message = sample(Body::Ping { nonce: 123 });
        let mut stream = Fragmented {
            bytes: Cursor::new(encoded(&message)),
            interrupted: false,
        };
        assert_eq!(read_frame(&mut stream).unwrap(), Some(message));
    }
}
