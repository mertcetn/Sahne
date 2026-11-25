//! Host-owned transactions for the flat scene prototype.
use crate::{Color, Content, MAX_TEXT_BYTES, ValidatedScene};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_EDITS: usize = 64;
pub const MAX_BATCH_TEXT_BYTES: usize = 48 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Edit {
    Text { node: u32, value: String },
    Color { node: u32, value: Color },
}

#[derive(Debug, PartialEq, Eq)]
pub struct Batch {
    pub session: u64,
    pub sequence: u64,
    pub base_revision: u64,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack {
    pub session: u64,
    pub sequence: u64,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    Closed,
    Context,
    Budget,
    Target,
    Property,
    Duplicate,
    Scene,
    Exhausted,
}

pub struct HostScene {
    scene: ValidatedScene,
    writable: BTreeMap<u32, usize>,
    session: u64,
    sequence: u64,
    revision: u64,
    closed: bool,
}

impl HostScene {
    /// The host selects writable node IDs before worker messages are accepted.
    pub fn new(
        scene: ValidatedScene,
        session: u64,
        writable: &BTreeSet<u32>,
    ) -> Result<Self, Reject> {
        if session == 0 {
            return Err(Reject::Context);
        }
        let indices: BTreeMap<_, _> = scene
            .scene()
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| writable.contains(&node.id))
            .map(|(index, node)| (node.id, index))
            .collect();
        if indices.len() != writable.len() {
            return Err(Reject::Target);
        }
        Ok(Self {
            scene,
            writable: indices,
            session,
            sequence: 1,
            revision: 0,
            closed: false,
        })
    }

    pub fn scene(&self) -> &ValidatedScene {
        &self.scene
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Transport EOF or acknowledgement delivery failure ends the session while
    /// preserving the host's last accepted scene.
    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn apply(&mut self, batch: Batch) -> Result<Ack, Reject> {
        if self.closed {
            return Err(Reject::Closed);
        }
        let result = self.apply_open(batch);
        if result.is_err() {
            self.close();
        }
        result
    }

    fn apply_open(&mut self, batch: Batch) -> Result<Ack, Reject> {
        if (batch.session, batch.sequence, batch.base_revision)
            != (self.session, self.sequence, self.revision)
        {
            return Err(Reject::Context);
        }
        if batch.edits.is_empty() || batch.edits.len() > MAX_EDITS {
            return Err(Reject::Budget);
        }
        let mut seen = BTreeSet::new();
        let mut text_bytes = 0;
        for edit in &batch.edits {
            let (id, is_text) = match edit {
                Edit::Text { node, value } => {
                    if value.len() > MAX_TEXT_BYTES {
                        return Err(Reject::Budget);
                    }
                    text_bytes += value.len();
                    if text_bytes > MAX_BATCH_TEXT_BYTES {
                        return Err(Reject::Budget);
                    }
                    (*node, true)
                }
                Edit::Color { node, .. } => (*node, false),
            };
            let index = *self.writable.get(&id).ok_or(Reject::Target)?;
            if !seen.insert((id, is_text)) {
                return Err(Reject::Duplicate);
            }
            match (&self.scene.scene().nodes[index].content, is_text) {
                (Content::Text { .. }, _) | (Content::Rectangle { .. }, false) => {}
                _ => return Err(Reject::Property),
            }
        }
        let revision = self.revision.checked_add(1).ok_or(Reject::Exhausted)?;
        let sequence = self.sequence.checked_add(1).ok_or(Reject::Exhausted)?;
        let mut candidate = self.scene.scene().clone();
        for edit in batch.edits {
            match edit {
                Edit::Text { node, value } => {
                    let Content::Text { text, .. } =
                        &mut candidate.nodes[self.writable[&node]].content
                    else {
                        unreachable!("validated text target")
                    };
                    *text = value;
                }
                Edit::Color { node, value } => match &mut candidate.nodes[self.writable[&node]]
                    .content
                {
                    Content::Text { color, .. } | Content::Rectangle { color } => *color = value,
                    Content::Image { .. } => unreachable!("validated color target"),
                },
            }
        }
        let scene = candidate.validate().map_err(|_| Reject::Scene)?;
        self.scene = scene;
        self.revision = revision;
        self.sequence = sequence;
        Ok(Ack {
            session: self.session,
            sequence: batch.sequence,
            revision,
        })
    }
}

#[cfg(test)]
mod tests;
