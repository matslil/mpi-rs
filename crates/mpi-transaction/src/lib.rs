//! Transaction decision logging over the persistent log storage protocol.

use core::fmt;

use mpi::{CallError, EndpointId, TransactionDecision, TransactionId, TransactionPath};
use persistent_log_storage::PersistentLogStorageProtocolV1;

const RECORD_MAGIC: &[u8; 5] = b"MPITX";
const RECORD_VERSION: u8 = 1;
const RECORD_DECISION: u8 = 1;
const DECISION_COMMIT: u8 = 1;
const DECISION_ABORT: u8 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionDecisionRecord {
    pub log_index: u64,
    pub path: TransactionPath,
    pub decision: TransactionDecision,
}

#[derive(Debug)]
pub enum TransactionLogError {
    Call(CallError),
    Storage(String),
    InvalidRecord(String),
}

impl fmt::Display for TransactionLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(error) => write!(f, "transaction log call error: {error}"),
            Self::Storage(message) => write!(f, "transaction log storage error: {message}"),
            Self::InvalidRecord(message) => write!(f, "invalid transaction log record: {message}"),
        }
    }
}

impl std::error::Error for TransactionLogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Call(error) => Some(error),
            Self::Storage(_) | Self::InvalidRecord(_) => None,
        }
    }
}

impl From<CallError> for TransactionLogError {
    fn from(value: CallError) -> Self {
        Self::Call(value)
    }
}

pub struct TransactionDecisionLog<H> {
    storage: PersistentLogStorageProtocolV1::Binding<H>,
}

impl<H> TransactionDecisionLog<H>
where
    H: PersistentLogStorageProtocolV1::store::Target
        + PersistentLogStorageProtocolV1::commit::Target
        + PersistentLogStorageProtocolV1::read::Target,
{
    #[must_use]
    pub const fn new(storage: PersistentLogStorageProtocolV1::Binding<H>) -> Self {
        Self { storage }
    }

    pub fn record_decision(
        &self,
        path: &TransactionPath,
        decision: TransactionDecision,
    ) -> Result<u64, TransactionLogError> {
        let payload = encode_decision_record(path, decision)?;
        let index = self
            .storage
            .store_blocking(payload)?
            .map_err(TransactionLogError::Storage)?;
        self.storage
            .commit_blocking(index)?
            .map_err(TransactionLogError::Storage)?;
        Ok(index)
    }

    pub fn recover_decisions(&self) -> Result<Vec<TransactionDecisionRecord>, TransactionLogError> {
        self.storage
            .read_blocking(None)?
            .map_err(TransactionLogError::Storage)?
            .into_iter()
            .filter_map(decode_decision_entry)
            .collect()
    }
}

fn decode_decision_entry(
    entry: (u64, Vec<u8>),
) -> Option<Result<TransactionDecisionRecord, TransactionLogError>> {
    if !entry.1.starts_with(RECORD_MAGIC) {
        return None;
    }
    Some(decode_decision_record(entry))
}

fn encode_decision_record(
    path: &TransactionPath,
    decision: TransactionDecision,
) -> Result<Vec<u8>, TransactionLogError> {
    let mut payload = Vec::new();
    payload.extend_from_slice(RECORD_MAGIC);
    payload.push(RECORD_VERSION);
    payload.push(RECORD_DECISION);
    payload.push(match decision {
        TransactionDecision::Commit => DECISION_COMMIT,
        TransactionDecision::Abort => DECISION_ABORT,
    });
    payload.extend_from_slice(&path.root_id().origin.0.to_le_bytes());
    payload.extend_from_slice(&path.root_id().sequence.to_le_bytes());
    let segment_len: u32 =
        path.segments().len().try_into().map_err(|_| {
            TransactionLogError::InvalidRecord("transaction path too deep".to_string())
        })?;
    payload.extend_from_slice(&segment_len.to_le_bytes());
    for segment in path.segments() {
        payload.extend_from_slice(&segment.to_le_bytes());
    }
    Ok(payload)
}

fn decode_decision_record(
    entry: (u64, Vec<u8>),
) -> Result<TransactionDecisionRecord, TransactionLogError> {
    let (log_index, bytes) = entry;
    let min_len = RECORD_MAGIC.len() + 1 + 1 + 1 + 8 + 8 + 4;
    if bytes.len() < min_len {
        return Err(TransactionLogError::InvalidRecord(
            "truncated transaction decision record".to_string(),
        ));
    }
    if &bytes[..RECORD_MAGIC.len()] != RECORD_MAGIC {
        return Err(TransactionLogError::InvalidRecord(
            "bad transaction record magic".to_string(),
        ));
    }
    let mut offset = RECORD_MAGIC.len();
    let version = bytes[offset];
    offset += 1;
    if version != RECORD_VERSION {
        return Err(TransactionLogError::InvalidRecord(format!(
            "unsupported transaction record version {version}"
        )));
    }
    let kind = bytes[offset];
    offset += 1;
    if kind != RECORD_DECISION {
        return Err(TransactionLogError::InvalidRecord(format!(
            "unsupported transaction record kind {kind}"
        )));
    }
    let decision = match bytes[offset] {
        DECISION_COMMIT => TransactionDecision::Commit,
        DECISION_ABORT => TransactionDecision::Abort,
        other => {
            return Err(TransactionLogError::InvalidRecord(format!(
                "unsupported transaction decision {other}"
            )));
        }
    };
    offset += 1;
    let origin = EndpointId(u64::from_le_bytes(
        bytes[offset..offset + 8].try_into().expect("slice length"),
    ));
    offset += 8;
    let sequence = u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("slice length"));
    offset += 8;
    let segment_len =
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice length")) as usize;
    offset += 4;

    let expected_len = min_len
        .checked_add(segment_len.checked_mul(8).ok_or_else(|| {
            TransactionLogError::InvalidRecord("transaction path length overflow".to_string())
        })?)
        .ok_or_else(|| {
            TransactionLogError::InvalidRecord("transaction record length overflow".to_string())
        })?;
    if bytes.len() != expected_len {
        return Err(TransactionLogError::InvalidRecord(
            "transaction decision record has trailing or missing bytes".to_string(),
        ));
    }

    let mut path = TransactionPath::root(TransactionId::new(origin, sequence));
    for _ in 0..segment_len {
        let segment =
            u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("slice length"));
        offset += 8;
        path = path.child(segment);
    }

    Ok(TransactionDecisionRecord {
        log_index,
        path,
        decision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use persistent_log_storage::start_file_log_storage;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mpi-transaction-{name}-{}-{nanos}.log",
            std::process::id()
        ))
    }

    #[test]
    fn mpi_req_119_req_126_decision_log_commits_decision_index_before_returning() {
        let path = unique_path("commit");
        let (storage, _runtime) = start_file_log_storage(path.clone()).expect("start storage task");
        let log = TransactionDecisionLog::new(storage);
        let tx_path = TransactionPath::root(TransactionId::new(EndpointId(11), 5));

        let index = log
            .record_decision(&tx_path, TransactionDecision::Commit)
            .expect("record commit decision");

        assert_eq!(index, 0);
    }

    #[test]
    fn mpi_req_120_recover_decisions_reads_committed_decision_records() {
        let path = unique_path("recover");
        let root = TransactionPath::root(TransactionId::new(EndpointId(12), 6));
        let child = root.child(1);
        {
            let (storage, _runtime) =
                start_file_log_storage(path.clone()).expect("start storage task");
            let log = TransactionDecisionLog::new(storage);
            log.record_decision(&root, TransactionDecision::Abort)
                .expect("record abort");
            log.record_decision(&child, TransactionDecision::Commit)
                .expect("record child commit");
        }

        let (storage, _runtime) =
            start_file_log_storage(path.clone()).expect("restart storage task");
        let log = TransactionDecisionLog::new(storage);
        let decisions = log.recover_decisions().expect("recover decisions");

        assert_eq!(
            decisions,
            vec![
                TransactionDecisionRecord {
                    log_index: 0,
                    path: root,
                    decision: TransactionDecision::Abort,
                },
                TransactionDecisionRecord {
                    log_index: 1,
                    path: child,
                    decision: TransactionDecision::Commit,
                },
            ]
        );
    }
}
