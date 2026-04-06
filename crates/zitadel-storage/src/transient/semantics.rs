use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::SessionRecord;

pub(crate) const DEFAULT_TRANSIENT_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub(crate) enum SessionLookupOutcome {
    Active(SessionRecord),
    Inactive,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransientStateOutcome<T> {
    Active(T),
    Inactive,
    Missing,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TransientStateMeta {
    pub expires_at_epoch: Option<u64>,
    pub consumed_or_done: bool,
    pub revoked: bool,
}

impl TransientStateMeta {
    pub fn is_expired_at(self, now_epoch: u64) -> bool {
        self.expires_at_epoch
            .is_some_and(|expires_at| expires_at <= now_epoch)
    }

    pub fn is_inactive_at(self, now_epoch: u64) -> bool {
        self.revoked || self.consumed_or_done || self.is_expired_at(now_epoch)
    }
}

pub(crate) fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn default_transient_state_meta() -> TransientStateMeta {
    TransientStateMeta {
        expires_at_epoch: Some(now_epoch_secs() + DEFAULT_TRANSIENT_TTL.as_secs()),
        ..TransientStateMeta::default()
    }
}

pub(crate) fn session_lookup_outcome(
    record: SessionRecord,
    expires_at_epoch: Option<u64>,
) -> SessionLookupOutcome {
    let meta = TransientStateMeta {
        expires_at_epoch,
        consumed_or_done: false,
        revoked: record.revoked_at.is_some(),
    };
    if meta.is_inactive_at(now_epoch_secs()) {
        SessionLookupOutcome::Inactive
    } else {
        SessionLookupOutcome::Active(record)
    }
}

pub(crate) fn transient_state_outcome<T>(
    value: T,
    meta: TransientStateMeta,
) -> TransientStateOutcome<T> {
    if meta.is_inactive_at(now_epoch_secs()) {
        TransientStateOutcome::Inactive
    } else {
        TransientStateOutcome::Active(value)
    }
}
