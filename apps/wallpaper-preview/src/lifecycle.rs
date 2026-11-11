//! Run-wide recovery policy shared by real faults and the lifecycle stress harness.
#[derive(Default)]
pub struct RecoveryBudget {
    used: u32,
}
impl RecoveryBudget {
    pub fn consume(&mut self) -> Result<u32, &'static str> {
        if self.used >= 10 {
            return Err("Run-wide recovery budget exhausted (10)");
        }
        self.used += 1;
        Ok(self.used)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_failures_cannot_restart_forever() {
        let mut budget = RecoveryBudget::default();
        for expected in 1..=10 {
            assert_eq!(budget.consume(), Ok(expected));
        }
        for _ in 0..100 {
            assert!(budget.consume().is_err());
        }
    }
}

/// A test only passes after the actual callback and a later-generation frame.
#[derive(Debug, Default)]
pub struct DeviceLossTest {
    enabled: bool,
    requested: Option<u64>,
    callback_seen: bool,
    restored: bool,
}
impl DeviceLossTest {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }
    pub fn needs_request(&self) -> bool {
        self.enabled && self.requested.is_none()
    }
    pub fn request(&mut self, generation: u64) {
        if self.needs_request() {
            self.requested = Some(generation);
        }
    }
    pub fn callback(&mut self, generation: u64, destroyed: bool) {
        if self.requested == Some(generation) && destroyed {
            self.callback_seen = true;
        }
    }
    pub fn frame(&mut self, generation: u64) {
        if self.callback_seen && self.requested.is_some_and(|old| generation > old) {
            self.restored = true;
        }
    }
    pub fn complete(&self) -> bool {
        !self.enabled || self.restored
    }
}
#[cfg(test)]
mod device_tests {
    use super::*;
    #[test]
    fn new_frame_without_actual_loss_callback_is_not_a_pass() {
        let mut test = DeviceLossTest::new(true);
        test.request(3);
        test.frame(4);
        assert!(!test.complete());
        test.callback(2, true);
        test.frame(4);
        assert!(!test.complete());
        test.callback(3, false);
        test.frame(4);
        assert!(!test.complete());
        test.callback(3, true);
        test.frame(3);
        assert!(!test.complete());
        test.frame(4);
        assert!(test.complete());
    }
}
