//! Explicit job lifecycle. Failed or cancelled work is never reported as complete.
use crate::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Draft,
    Queued,
    Preflight,
    Discovering,
    Testing,
    Validating,
    Completed,
    Paused,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Queue,
    Start,
    Discover,
    Test,
    Validate,
    Complete,
    Pause,
    Resume,
    Cancel,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Lifecycle {
    phase: Phase,
    resume_phase: Option<Phase>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            phase: Phase::Draft,
            resume_phase: None,
        }
    }
}

impl Phase {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
    pub fn active(self) -> bool {
        matches!(
            self,
            Self::Preflight | Self::Discovering | Self::Testing | Self::Validating
        )
    }
}

impl Lifecycle {
    pub fn restore(phase: Phase, resume_phase: Option<Phase>) -> Result<Self, Error> {
        let valid = match (phase, resume_phase) {
            (Phase::Paused, Some(resume)) => resume.active(),
            (Phase::Paused, None) | (_, Some(_)) => false,
            (_, None) => true,
        };
        if !valid {
            return Err(Error::Conflict);
        }
        Ok(Self {
            phase,
            resume_phase,
        })
    }

    pub fn phase(self) -> Phase {
        self.phase
    }
    pub fn resume_phase(self) -> Option<Phase> {
        self.resume_phase
    }

    pub fn apply(self, event: Event) -> Result<Self, Error> {
        use Event as E;
        use Phase as P;
        let mut next = self;
        next.phase = match (self.phase, event) {
            (P::Draft, E::Queue) => P::Queued,
            (P::Queued, E::Start) => P::Preflight,
            (P::Preflight, E::Discover) => P::Discovering,
            (P::Discovering, E::Test) => P::Testing,
            (P::Testing, E::Validate) => P::Validating,
            (P::Validating, E::Complete) => P::Completed,
            (phase, E::Pause) if phase.active() => {
                next.resume_phase = Some(phase);
                P::Paused
            }
            (P::Paused, E::Resume) => {
                let phase = self
                    .resume_phase
                    .filter(|phase| phase.active())
                    .ok_or(Error::Conflict)?;
                next.resume_phase = None;
                phase
            }
            (phase, E::Cancel) if !phase.terminal() => {
                next.resume_phase = None;
                P::Cancelled
            }
            (phase, E::Fail) if phase.active() || matches!(phase, P::Queued | P::Paused) => {
                next.resume_phase = None;
                P::Failed
            }
            _ => return Err(Error::Conflict),
        };
        Ok(next)
    }
}

/// A reservation is made atomically in persistence before any work starts.
pub fn reserve_budget(available: i64, reserved: i64, requested: i64) -> Result<i64, Error> {
    if available < 0 || reserved < 0 || requested <= 0 {
        return Err(Error::Validation);
    }
    let total = reserved.checked_add(requested).ok_or(Error::LimitReached)?;
    if total > available {
        Err(Error::LimitReached)
    } else {
        Ok(total)
    }
}
