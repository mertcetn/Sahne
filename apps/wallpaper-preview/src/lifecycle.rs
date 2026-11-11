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
