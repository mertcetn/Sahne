//! Fixed original native-animation playback, owned across GPU generations.
use super::Result;
use std::time::{Duration, Instant};
use wallpaper_scene::animation::{self, Clock, Sample, ValidatedAnimation};
pub const MANUAL: u8 = 1;
pub const HIDDEN: u8 = 2;
pub const RECOVERY: u8 = 4;
pub const SUSPENDED: u8 = 8;
pub const ZERO_SIZE: u8 = 16;

pub fn program() -> Result<ValidatedAnimation> {
    let package = wallpaper_compiler::fixtures::animated_shapes_package()?;
    let compiled = wallpaper_compiler::compile_animated(
        &package,
        wallpaper_compiler::Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([12, 17, 22, 255]),
        },
        wallpaper_compiler::fixtures::animated_shapes_spec(),
    )?;
    let decoded = animation::decode(&compiled.animation().encode())?;
    wallpaper_media::validate_hierarchy_assets(decoded.base(), compiled.images())?;
    Ok(decoded)
}
pub struct Playback {
    program: ValidatedAnimation,
    clock: Clock,
    reasons: u8,
    active: bool,
    next: Option<Instant>,
    period: Duration,
    reference_pose: bool,
}
impl Playback {
    pub fn new(now: Instant) -> Result<Self> {
        let package = wallpaper_compiler::fixtures::animated_shapes_package()?;
        Self::with_program(program()?, now, package.manifest().preferred_fps())
    }
    fn with_program(program: ValidatedAnimation, now: Instant, fps: u16) -> Result<Self> {
        if !(1..=60).contains(&fps) {
            return Err("Animation FPS must be 1..60".into());
        }
        let mut clock = Clock::new(now);
        clock.set_paused(now, true)?;
        Ok(Self {
            program,
            clock,
            reasons: RECOVERY,
            active: true,
            next: None,
            period: Duration::from_nanos(1_000_000_000u64.div_ceil(u64::from(fps))),
            reference_pose: false,
        })
    }
    pub fn freeze_at_reference_pose(&mut self) {
        self.reference_pose = true;
        self.next = None;
    }
    pub fn reason(&mut self, now: Instant, reason: u8, enabled: bool) -> Result<()> {
        let reasons = if enabled {
            self.reasons | reason
        } else {
            self.reasons & !reason
        };
        self.clock.set_paused(now, reasons != 0)?;
        if reasons != self.reasons {
            eprintln!(
                "Animation pause state: reasons={reasons}, active elapsed ns={}",
                self.clock.elapsed().as_nanos()
            );
            self.next = if reasons == 0 && self.active && !self.reference_pose {
                Some(now)
            } else {
                None
            };
        }
        self.reasons = reasons;
        Ok(())
    }
    pub fn toggle(&mut self, now: Instant) -> Result<()> {
        if self.reference_pose {
            return Ok(());
        }
        self.reason(now, MANUAL, self.reasons & MANUAL == 0)
    }
    pub fn wake(&self) -> Option<Instant> {
        self.next
    }
    pub fn queued(&mut self) {
        self.next = None;
    }
    pub fn sample(&mut self, now: Instant) -> Result<Sample> {
        let elapsed = if self.reference_pose {
            Duration::from_secs(1)
        } else {
            self.clock.advance(now)?
        };
        let sample = self.program.sample(elapsed)?;
        self.active = sample.active;
        Ok(sample)
    }
    pub fn rendered(&mut self, now: Instant) -> Result<()> {
        self.next = if self.reasons == 0 && self.active && !self.reference_pose {
            Some(
                now.checked_add(self.period)
                    .ok_or("Animation deadline overflow")?,
            )
        } else {
            None
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_pose_stays_at_exact_midpoint_without_queued_animation() {
        let now = Instant::now();
        let mut playback = Playback::new(now).unwrap();
        let expected = playback.program.sample(Duration::from_secs(1)).unwrap();
        playback.freeze_at_reference_pose();
        playback.reason(now, RECOVERY, false).unwrap();
        playback.toggle(now + Duration::from_secs(2)).unwrap();
        let later = now + Duration::from_secs(5);
        assert_eq!(playback.sample(later).unwrap().scene, expected.scene);
        playback.rendered(later).unwrap();
        assert!(playback.wake().is_none());
    }
    #[test]
    fn independent_pause_reasons_preserve_pose_and_do_not_schedule_backlogs() {
        let start = Instant::now();
        let mut motion = Playback::new(start).unwrap();
        assert!(motion.wake().is_none());
        motion.reason(start, RECOVERY, false).unwrap();
        assert_eq!(motion.wake(), Some(start));
        motion.queued();
        assert!(motion.wake().is_none());
        let first = motion.sample(start + Duration::from_millis(500)).unwrap();
        motion
            .reason(start + Duration::from_millis(500), MANUAL, true)
            .unwrap();
        motion
            .reason(start + Duration::from_secs(1), HIDDEN, true)
            .unwrap();
        motion
            .reason(start + Duration::from_secs(2), MANUAL, false)
            .unwrap();
        motion.rendered(start + Duration::from_secs(2)).unwrap();
        assert!(motion.wake().is_none());
        assert_eq!(
            motion
                .sample(start + Duration::from_secs(10))
                .unwrap()
                .scene,
            first.scene
        );
        motion
            .reason(start + Duration::from_secs(20), HIDDEN, false)
            .unwrap();
        let resumed = motion
            .sample(start + Duration::from_millis(20_100))
            .unwrap();
        assert_ne!(resumed.scene, first.scene);
        motion.rendered(start + Duration::from_secs(100)).unwrap();
        assert_eq!(
            motion.wake(),
            Some(start + Duration::from_secs(100) + Duration::from_nanos(33_333_334))
        );
    }
    #[test]
    fn fps_bounds_completion_and_recovery_clock_are_explicit() {
        let start = Instant::now();
        for fps in [0, 61] {
            assert!(Playback::with_program(program().unwrap(), start, fps).is_err());
        }
        let mut source = program().unwrap().animation().clone();
        source.iterations = animation::Iterations::Count(1);
        let base = wallpaper_scene::hierarchy::decode(&program().unwrap().base().encode()).unwrap();
        let mut motion = Playback::with_program(source.validate(base).unwrap(), start, 60).unwrap();
        motion.reason(start, RECOVERY, false).unwrap();
        motion.sample(start + Duration::from_millis(300)).unwrap();
        motion
            .reason(start + Duration::from_millis(300), RECOVERY, true)
            .unwrap();
        motion
            .reason(start + Duration::from_secs(10), RECOVERY, false)
            .unwrap();
        assert_eq!(motion.clock.elapsed(), Duration::from_millis(300));
        assert!(
            !motion
                .sample(start + Duration::from_secs(12))
                .unwrap()
                .active
        );
        motion.rendered(start + Duration::from_secs(12)).unwrap();
        assert!(motion.wake().is_none());
    }
}
