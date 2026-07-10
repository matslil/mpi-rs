//! Transaction identifiers, transactional envelopes, and recovery log support.

use core::fmt;

use persistent_log_storage::{LogEntry, LogIndex, LogStorageError, PersistentLogStorage};

use crate::session::{EndpointId, SessionId};

const RECORD_MAGIC: &[u8; 5] = b"MPITX";
const RECORD_VERSION: u8 = 1;
const RECORD_DECISION: u8 = 1;
const DECISION_COMMIT: u8 = 1;
const DECISION_ABORT: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TransactionId {
    pub origin: EndpointId,
    pub sequence: u64,
}

impl TransactionId {
    #[must_use]
    pub const fn new(origin: EndpointId, sequence: u64) -> Self {
        Self { origin, sequence }
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "transaction:{}#{}", self.origin, self.sequence)
    }
}

#[derive(Debug, Clone)]
pub struct TransactionIdAllocator {
    origin: EndpointId,
    next_sequence: u64,
}

impl TransactionIdAllocator {
    #[must_use]
    pub const fn new(origin: EndpointId) -> Self {
        Self {
            origin,
            next_sequence: 0,
        }
    }

    #[must_use]
    pub const fn origin(&self) -> EndpointId {
        self.origin
    }

    pub fn next_transaction_id(&mut self) -> TransactionId {
        let transaction_id = TransactionId::new(self.origin, self.next_sequence);
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("transaction sequence exhausted");
        transaction_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TransactionPath {
    root: TransactionId,
    segments: Vec<u64>,
}

impl TransactionPath {
    #[must_use]
    pub fn root(root: TransactionId) -> Self {
        Self {
            root,
            segments: Vec::new(),
        }
    }

    #[must_use]
    pub fn child(&self, segment: u64) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self {
            root: self.root,
            segments,
        }
    }

    #[must_use]
    pub const fn root_id(&self) -> TransactionId {
        self.root
    }

    #[must_use]
    pub fn segments(&self) -> &[u64] {
        &self.segments
    }

    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let mut segments = self.segments.clone();
        segments.pop()?;
        Some(Self {
            root: self.root,
            segments,
        })
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for TransactionPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        for segment in &self.segments {
            write!(f, "/{segment}")?;
        }
        Ok(())
    }
}

pub trait HasTransactionPath {
    fn transaction_path(&self) -> &TransactionPath;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionalMessage<T> {
    pub transaction_path: TransactionPath,
    pub session_id: SessionId,
    pub value: T,
}

impl<T> TransactionalMessage<T> {
    #[must_use]
    pub fn new(transaction_path: TransactionPath, session_id: SessionId, value: T) -> Self {
        Self {
            transaction_path,
            session_id,
            value,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn into_value(self) -> T {
        self.value
    }
}

impl<T> HasTransactionPath for TransactionalMessage<T> {
    fn transaction_path(&self) -> &TransactionPath {
        &self.transaction_path
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionDecision {
    Commit,
    Abort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionDecisionRecord {
    pub log_index: LogIndex,
    pub path: TransactionPath,
    pub decision: TransactionDecision,
}

#[derive(Debug)]
pub enum TransactionLogError {
    Storage(LogStorageError),
    InvalidRecord(String),
}

impl fmt::Display for TransactionLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "transaction log storage error: {error}"),
            Self::InvalidRecord(message) => write!(f, "invalid transaction log record: {message}"),
        }
    }
}

impl std::error::Error for TransactionLogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::InvalidRecord(_) => None,
        }
    }
}

impl From<LogStorageError> for TransactionLogError {
    fn from(value: LogStorageError) -> Self {
        Self::Storage(value)
    }
}

pub struct TransactionDecisionLog<S> {
    storage: S,
}

impl<S> TransactionDecisionLog<S>
where
    S: PersistentLogStorage,
{
    #[must_use]
    pub const fn new(storage: S) -> Self {
        Self { storage }
    }

    pub fn into_storage(self) -> S {
        self.storage
    }

    pub fn record_decision(
        &mut self,
        path: &TransactionPath,
        decision: TransactionDecision,
    ) -> Result<LogIndex, TransactionLogError> {
        let payload = encode_decision_record(path, decision)?;
        let index = self.storage.append(&payload)?;
        self.storage.commit_through(index)?;
        Ok(index)
    }

    pub fn recover_decisions(&self) -> Result<Vec<TransactionDecisionRecord>, TransactionLogError> {
        self.storage
            .read_entries()?
            .into_iter()
            .filter_map(decode_decision_entry)
            .collect()
    }
}

fn decode_decision_entry(
    entry: LogEntry,
) -> Option<Result<TransactionDecisionRecord, TransactionLogError>> {
    if !entry.payload.starts_with(RECORD_MAGIC) {
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
    payload.extend_from_slice(&path.root.origin.0.to_le_bytes());
    payload.extend_from_slice(&path.root.sequence.to_le_bytes());
    let segment_len: u32 =
        path.segments.len().try_into().map_err(|_| {
            TransactionLogError::InvalidRecord("transaction path too deep".to_string())
        })?;
    payload.extend_from_slice(&segment_len.to_le_bytes());
    for segment in &path.segments {
        payload.extend_from_slice(&segment.to_le_bytes());
    }
    Ok(payload)
}

fn decode_decision_record(
    entry: LogEntry,
) -> Result<TransactionDecisionRecord, TransactionLogError> {
    let bytes = entry.payload;
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

    let mut segments = Vec::with_capacity(segment_len);
    for _ in 0..segment_len {
        let segment =
            u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("slice length"));
        offset += 8;
        segments.push(segment);
    }

    Ok(TransactionDecisionRecord {
        log_index: entry.index,
        path: TransactionPath {
            root: TransactionId::new(origin, sequence),
            segments,
        },
        decision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct RecordingStorage {
        entries: RefCell<Vec<LogEntry>>,
        committed: RefCell<Vec<LogIndex>>,
        next_index: Cell<LogIndex>,
    }

    impl PersistentLogStorage for RecordingStorage {
        fn append(&mut self, payload: &[u8]) -> Result<LogIndex, LogStorageError> {
            let index = self.next_index.get();
            self.next_index.set(index + 1);
            self.entries.borrow_mut().push(LogEntry {
                index,
                payload: payload.to_vec(),
            });
            Ok(index)
        }

        fn commit_through(&mut self, index: LogIndex) -> Result<(), LogStorageError> {
            self.committed.borrow_mut().push(index);
            Ok(())
        }

        fn discard_through(&mut self, index: LogIndex) -> Result<(), LogStorageError> {
            self.entries
                .borrow_mut()
                .retain(|entry| entry.index > index);
            Ok(())
        }

        fn read_entries(&self) -> Result<Vec<LogEntry>, LogStorageError> {
            Ok(self.entries.borrow().clone())
        }
    }

    #[test]
    fn mpi_req_110_transaction_paths_identify_root_and_children() {
        let root_id = TransactionId::new(EndpointId(7), 3);
        let root = TransactionPath::root(root_id);
        let child = root.child(0);
        let grandchild = child.child(2);

        assert!(root.is_root());
        assert_eq!(child.root_id(), root_id);
        assert_eq!(grandchild.segments(), &[0, 2]);
        assert_eq!(grandchild.parent(), Some(child));
    }

    #[test]
    fn mpi_req_111_transactional_message_carries_transaction_path_and_session_id() {
        let path = TransactionPath::root(TransactionId::new(EndpointId(9), 1)).child(4);
        let session_id = SessionId::new(EndpointId(10), 2);
        let message = TransactionalMessage::new(path.clone(), session_id, "payload");

        assert_eq!(message.transaction_path(), &path);
        assert_eq!(message.session_id(), session_id);
        assert_eq!(message.into_value(), "payload");
    }

    #[test]
    fn mpi_req_119_req_126_decision_log_commits_decision_index_before_returning() {
        let storage = RecordingStorage::default();
        let mut log = TransactionDecisionLog::new(storage);
        let path = TransactionPath::root(TransactionId::new(EndpointId(11), 5));

        let index = log
            .record_decision(&path, TransactionDecision::Commit)
            .expect("record commit decision");
        let storage = log.into_storage();

        assert_eq!(index, 0);
        assert_eq!(storage.committed.borrow().as_slice(), &[0]);
    }

    #[test]
    fn mpi_req_120_recover_decisions_reads_committed_decision_records() {
        let storage = RecordingStorage::default();
        let mut log = TransactionDecisionLog::new(storage);
        let root = TransactionPath::root(TransactionId::new(EndpointId(12), 6));
        let child = root.child(1);

        log.record_decision(&root, TransactionDecision::Abort)
            .expect("record abort");
        log.record_decision(&child, TransactionDecision::Commit)
            .expect("record child commit");

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
