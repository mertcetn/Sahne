//! Fixed host-driven fixture for the clock renderer path.
use super::*;
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};
use wallpaper_scene::{
    Color, Content, Node, Rect, SCENE_VERSION, Scene, ValidatedScene,
    transaction::{Batch, Edit, HostScene, Snapshot, SnapshotKey},
};

pub const CLOCK_ID: u32 = 1;
pub const ACCENT_ID: u32 = 2;
const SESSION: u64 = 1;

pub fn initial_scene() -> Result<ValidatedScene> {
    Ok(Scene {
        version: SCENE_VERSION,
        width: 1920,
        height: 1080,
        background: Color([12, 17, 22, 255]),
        resources: vec![],
        nodes: vec![
            Node {
                id: 3,
                bounds: Rect {
                    x: 400.0,
                    y: 300.0,
                    width: 1120.0,
                    height: 480.0,
                },
                content: Content::Rectangle {
                    color: Color([31, 42, 51, 255]),
                },
            },
            Node {
                id: CLOCK_ID,
                bounds: Rect {
                    x: 520.0,
                    y: 390.0,
                    width: 880.0,
                    height: 230.0,
                },
                content: Content::Text {
                    text: "00:00".into(),
                    font_size: 176.0,
                    color: Color([232, 244, 240, 255]),
                },
            },
            Node {
                id: ACCENT_ID,
                bounds: Rect {
                    x: 528.0,
                    y: 650.0,
                    width: 400.0,
                    height: 64.0,
                },
                content: Content::Text {
                    text: "UTC".into(),
                    font_size: 36.0,
                    color: Color([155, 220, 202, 255]),
                },
            },
        ],
    }
    .validate()?)
}

pub struct Clock {
    host: HostScene,
    wake: Option<Instant>,
    rollover: Option<u16>,
    worker: Option<Snapshot>,
}

impl Clock {
    pub fn new(now: Instant, test_rollover: bool) -> Result<Self> {
        let host = HostScene::new(initial_scene()?, SESSION, &BTreeSet::from([CLOCK_ID]))
            .map_err(|_| "Could not create fixed clock scene")?;
        Ok(Self {
            host,
            wake: Some(now),
            rollover: test_rollover.then_some(1439),
            worker: None,
        })
    }

    pub fn from_worker(now: Instant, folder: &str) -> Result<Self> {
        let mut clock = Self::new(now, false)?;
        let text = worker_text(folder)?;
        clock.apply_text(text)?;
        clock.wake = None;
        Ok(clock)
    }

    pub fn wake(&self) -> Option<Instant> {
        self.wake
    }

    pub fn live_fallback(now: Instant) -> Result<Self> {
        let mut clock = Self::new(now, false)?;
        clock.wake = None;
        Ok(clock)
    }

    pub fn accept_worker(&mut self, snapshot: Snapshot) {
        self.worker = Some(snapshot);
    }

    pub fn text(&self) -> &str {
        let scene = self
            .worker
            .as_ref()
            .map_or(self.host.scene(), |snapshot| snapshot.scene.as_ref());
        let Content::Text { text, .. } = &scene.scene().nodes[1].content else {
            unreachable!("fixed clock text node")
        };
        text
    }

    pub fn snapshot_after(&self, key: Option<SnapshotKey>) -> Option<Snapshot> {
        if let Some(snapshot) = &self.worker {
            (Some(snapshot.key) != key).then(|| Snapshot {
                key: snapshot.key,
                scene: Arc::clone(&snapshot.scene),
            })
        } else {
            self.host.snapshot_after(key)
        }
    }

    pub fn tick(&mut self, now: Instant) -> Result<bool> {
        if self.wake.is_none_or(|wake| now < wake) {
            return Ok(false);
        }
        let (minute, delay) = if let Some(minute) = self.rollover {
            (minute, Duration::from_secs(2))
        } else {
            utc_sample(SystemTime::now().duration_since(UNIX_EPOCH)?)
        };
        let next = now.checked_add(delay).ok_or("Clock deadline overflow")?;
        let changed = self.apply_text(format!("{:02}:{:02}", minute / 60, minute % 60))?;
        self.wake = Some(next);
        if let Some(minute) = self.rollover.as_mut() {
            *minute = 0;
        }
        Ok(changed)
    }

    fn apply_text(&mut self, value: String) -> Result<bool> {
        let before = self
            .host
            .snapshot_after(None)
            .ok_or("Missing clock snapshot")?
            .key;
        let revision = self.host.revision();
        let sequence = revision.checked_add(1).ok_or("Clock revision overflow")?;
        self.host
            .apply(Batch {
                session: SESSION,
                sequence,
                base_revision: revision,
                edits: vec![Edit::Text {
                    node: CLOCK_ID,
                    value,
                }],
            })
            .map_err(|_| "Fixed clock edit was rejected")?;
        Ok(self.host.snapshot_after(Some(before)).is_some())
    }
}

fn worker_text(folder: &str) -> Result<String> {
    use std::io::Read;
    use wallpaper_platform::{
        package_directory::open_folder,
        package_open::{Kind, open_child},
    };

    let parent = open_folder(folder)?;
    let mut file = open_child(&parent, "clock-snapshot.bin", Kind::RegularFile)?;
    let mut bytes = [0; 5];
    file.read_exact(&mut bytes)?;
    if file.read(&mut [0])? != 0 {
        return Err("Clock snapshot has trailing bytes".into());
    }
    let [h1, h2, colon, m1, m2] = bytes;
    if colon != b':'
        || ![h1, h2, m1, m2].iter().all(u8::is_ascii_digit)
        || (h1 - b'0') * 10 + (h2 - b'0') >= 24
        || m1 >= b'6'
    {
        return Err("Clock snapshot must contain a five-byte UTC HH:MM value".into());
    }
    Ok(String::from_utf8(bytes.to_vec())?)
}

pub fn utc_sample(elapsed: Duration) -> (u16, Duration) {
    let seconds = elapsed.as_secs();
    let minute = ((seconds / 60) % 1440) as u16;
    let until_next = Duration::from_secs(60 - seconds % 60)
        - Duration::from_nanos(u64::from(elapsed.subsec_nanos()));
    (minute, until_next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_rollover_publishes_two_distinct_poses_then_stays_unchanged() {
        let now = Instant::now();
        let mut clock = Clock::new(now, true).unwrap();
        assert!(clock.tick(now).unwrap());
        let before = clock.snapshot_after(None).unwrap();
        assert!(
            matches!(&before.scene.scene().nodes[1].content, Content::Text { text, .. } if text == "23:59")
        );
        assert!(!clock.tick(now + Duration::from_secs(1)).unwrap());
        assert!(clock.tick(now + Duration::from_secs(2)).unwrap());
        let after = clock.snapshot_after(Some(before.key)).unwrap();
        assert!(
            matches!(&after.scene.scene().nodes[1].content, Content::Text { text, .. } if text == "00:00")
        );
        assert!(!clock.tick(now + Duration::from_secs(4)).unwrap());
        assert!(clock.snapshot_after(Some(after.key)).is_none());
    }

    #[test]
    fn utc_minute_boundary_and_next_deadline_include_subseconds() {
        assert_eq!(
            utc_sample(Duration::from_millis(86_399_750)),
            (1439, Duration::from_millis(250))
        );
        assert_eq!(
            utc_sample(Duration::from_secs(86_400)),
            (0, Duration::from_secs(60))
        );
    }
}
