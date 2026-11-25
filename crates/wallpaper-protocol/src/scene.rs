use super::{Body, Decoder, Error, Message, Result};
use wallpaper_scene::{
    Color, MAX_TEXT_BYTES,
    transaction::{Batch, Edit, HostScene, MAX_BATCH_TEXT_BYTES, MAX_EDITS},
};

pub const SCENE_REJECT_CODE: u16 = 100;

pub(super) fn check_ack(base: u64, revision: u64) -> Result<()> {
    if base.checked_add(1) != Some(revision) {
        return Err(Error::Invalid("invalid acknowledged revision"));
    }
    Ok(())
}

fn check_count(count: usize) -> Result<()> {
    if !(1..=MAX_EDITS).contains(&count) {
        return Err(Error::Invalid("edit count outside bounds"));
    }
    Ok(())
}

fn check_text(length: usize, total: &mut usize) -> Result<()> {
    if length > MAX_TEXT_BYTES || length > MAX_BATCH_TEXT_BYTES - *total {
        return Err(Error::Invalid("scene batch text budget exceeded"));
    }
    *total += length;
    Ok(())
}

pub(super) fn encode_edits(output: &mut Vec<u8>, edits: &[Edit]) -> Result<()> {
    check_count(edits.len())?;
    let mut total = 0;
    output.extend_from_slice(&(edits.len() as u16).to_le_bytes());
    for edit in edits {
        match edit {
            Edit::Text { node, value } => {
                check_text(value.len(), &mut total)?;
                output.push(1);
                output.extend_from_slice(&node.to_le_bytes());
                output.extend_from_slice(&(value.len() as u16).to_le_bytes());
                output.extend_from_slice(value.as_bytes());
            }
            Edit::Color { node, value } => {
                output.push(2);
                output.extend_from_slice(&node.to_le_bytes());
                output.extend_from_slice(&value.0);
            }
        }
    }
    Ok(())
}

pub(super) fn decode_edits(input: &mut Decoder<'_>) -> Result<Vec<Edit>> {
    let count = usize::from(input.u16()?);
    check_count(count)?;
    let mut edits = Vec::with_capacity(count);
    let mut total = 0;
    for _ in 0..count {
        let [tag] = input.array()?;
        let node = u32::from_le_bytes(input.array()?);
        let edit = match tag {
            1 => {
                let length = usize::from(input.u16()?);
                check_text(length, &mut total)?;
                let value = std::str::from_utf8(input.take(length)?)
                    .map_err(|_| Error::Invalid("scene edit text is not UTF-8"))?;
                Edit::Text {
                    node,
                    value: value.to_owned(),
                }
            }
            2 => Edit::Color {
                node,
                value: Color(input.array()?),
            },
            _ => return Err(Error::Invalid("unknown scene edit kind")),
        };
        edits.push(edit);
    }
    Ok(edits)
}

/// Handle one decoded update after transport handshake. On any codec or I/O
/// failure the channel owner must close the host session and stop the worker.
/// A rejection reply is terminal and may be discarded if the channel is broken.
pub fn accept_scene_batch(host: &mut HostScene, message: Message) -> Message {
    let Message {
        session,
        sequence,
        base_revision,
        body,
    } = message;
    let body = match body {
        Body::SceneBatch { edits } => {
            match host.apply(Batch {
                session,
                sequence,
                base_revision,
                edits,
            }) {
                Ok(ack) => Body::SceneAck {
                    revision: ack.revision,
                },
                Err(_) => rejection(),
            }
        }
        _ => {
            host.close();
            rejection()
        }
    };
    Message {
        session,
        sequence,
        base_revision,
        body,
    }
}

fn rejection() -> Body {
    Body::Reject {
        code: SCENE_REJECT_CODE,
        message: "Scene batch rejected".into(),
    }
}

#[cfg(test)]
mod tests;
