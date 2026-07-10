//! Crash-safe file-backed persistent log storage.
//!
//! The crate-local baseline is documented in `se-design-baseline.md`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"PLS1";
const VERSION: u8 = 1;
const KIND_APPEND: u8 = 1;
const KIND_DISCARD: u8 = 2;
const HEADER_LEN: usize = 4 + 1 + 1 + 2 + 8 + 8 + 8;

pub type LogIndex = u64;

pub type LogEntry = RetrievedLogEntry<Vec<u8>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentLogStorageProtocol;

impl PersistentLogStorageProtocol {
    pub const NAME: &'static str = "persistent_log_storage.v1";
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreLogEntry<T = Vec<u8>> {
    pub payload: T,
}

impl<T> StoreLogEntry<T> {
    #[must_use]
    pub const fn new(payload: T) -> Self {
        Self { payload }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredLogEntry {
    pub index: LogIndex,
}

impl StoredLogEntry {
    #[must_use]
    pub const fn new(index: LogIndex) -> Self {
        Self { index }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitLogEntries {
    pub through: LogIndex,
}

impl CommitLogEntries {
    #[must_use]
    pub const fn through(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommittedLogEntries {
    pub through: LogIndex,
}

impl CommittedLogEntries {
    #[must_use]
    pub const fn new(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscardLogEntries {
    pub through: LogIndex,
}

impl DiscardLogEntries {
    #[must_use]
    pub const fn through(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscardedLogEntries {
    pub through: LogIndex,
}

impl DiscardedLogEntries {
    #[must_use]
    pub const fn new(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReadLogEntries {
    pub from: Option<LogIndex>,
}

impl ReadLogEntries {
    #[must_use]
    pub const fn all() -> Self {
        Self { from: None }
    }

    #[must_use]
    pub const fn from(from: LogIndex) -> Self {
        Self { from: Some(from) }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrievedLogEntry<T = Vec<u8>> {
    pub index: LogIndex,
    pub payload: T,
}

impl<T> RetrievedLogEntry<T> {
    #[must_use]
    pub const fn new(index: LogIndex, payload: T) -> Self {
        Self { index, payload }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrievedLogEntries<T = Vec<u8>> {
    pub entries: Vec<RetrievedLogEntry<T>>,
}

impl<T> RetrievedLogEntries<T> {
    #[must_use]
    pub const fn new(entries: Vec<RetrievedLogEntry<T>>) -> Self {
        Self { entries }
    }

    pub fn into_entries(self) -> Vec<RetrievedLogEntry<T>> {
        self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistentLogStorageMessage {
    Store(StoreLogEntry<Vec<u8>>),
    Commit(CommitLogEntries),
    Discard(DiscardLogEntries),
    Read(ReadLogEntries),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistentLogStorageReply {
    Stored(StoredLogEntry),
    Committed(CommittedLogEntries),
    Discarded(DiscardedLogEntries),
    Retrieved(RetrievedLogEntries<Vec<u8>>),
}

pub trait PersistentLogStorage {
    fn store(&mut self, message: StoreLogEntry<Vec<u8>>)
    -> Result<StoredLogEntry, LogStorageError>;

    fn commit(&mut self, message: CommitLogEntries)
    -> Result<CommittedLogEntries, LogStorageError>;

    fn discard(
        &mut self,
        message: DiscardLogEntries,
    ) -> Result<DiscardedLogEntries, LogStorageError>;

    fn read(
        &self,
        message: ReadLogEntries,
    ) -> Result<RetrievedLogEntries<Vec<u8>>, LogStorageError>;

    fn handle(
        &mut self,
        message: PersistentLogStorageMessage,
    ) -> Result<PersistentLogStorageReply, LogStorageError> {
        match message {
            PersistentLogStorageMessage::Store(message) => {
                self.store(message).map(PersistentLogStorageReply::Stored)
            }
            PersistentLogStorageMessage::Commit(message) => self
                .commit(message)
                .map(PersistentLogStorageReply::Committed),
            PersistentLogStorageMessage::Discard(message) => self
                .discard(message)
                .map(PersistentLogStorageReply::Discarded),
            PersistentLogStorageMessage::Read(message) => {
                self.read(message).map(PersistentLogStorageReply::Retrieved)
            }
        }
    }
}

#[derive(Debug)]
pub enum LogStorageError {
    Io(io::Error),
    InvalidRecord(String),
    UnknownLogIndex(LogIndex),
    Serialization {
        codec: &'static str,
        message: String,
    },
    Deserialization {
        codec: &'static str,
        index: LogIndex,
        message: String,
    },
}

impl fmt::Display for LogStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "persistent log I/O error: {error}"),
            Self::InvalidRecord(message) => write!(f, "invalid persistent log record: {message}"),
            Self::UnknownLogIndex(index) => write!(f, "unknown log index {index}"),
            Self::Serialization { codec, message } => {
                write!(f, "{codec} log serialization error: {message}")
            }
            Self::Deserialization {
                codec,
                index,
                message,
            } => {
                write!(
                    f,
                    "{codec} log deserialization error at index {index}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for LogStorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidRecord(_)
            | Self::UnknownLogIndex(_)
            | Self::Serialization { .. }
            | Self::Deserialization { .. } => None,
        }
    }
}

impl From<io::Error> for LogStorageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(feature = "serde")]
pub struct SerializedLogEntry<T> {
    pub payload: T,
}

#[cfg(feature = "serde")]
impl<T> SerializedLogEntry<T> {
    #[must_use]
    pub const fn new(payload: T) -> Self {
        Self { payload }
    }
}

#[cfg(feature = "serde")]
pub trait SerializedPersistentLogStorageExt: PersistentLogStorage {
    fn store_serialized<T>(
        &mut self,
        message: SerializedLogEntry<T>,
    ) -> Result<StoredLogEntry, LogStorageError>
    where
        T: serde::Serialize,
    {
        let payload =
            <serde_wincode::SerdeCompat<T> as serde_wincode::wincode::Serialize>::serialize(
                &message.payload,
            )
            .map_err(|error| LogStorageError::Serialization {
                codec: "wincode",
                message: error.to_string(),
            })?;
        self.store(StoreLogEntry::new(payload))
    }

    fn read_serialized<T>(
        &self,
        message: ReadLogEntries,
    ) -> Result<RetrievedLogEntries<T>, LogStorageError>
    where
        T: serde::de::DeserializeOwned,
    {
        let entries = self
            .read(message)?
            .entries
            .into_iter()
            .map(|entry| {
                <serde_wincode::SerdeCompat<T> as serde_wincode::wincode::Deserialize>::deserialize(
                    &entry.payload,
                )
                .map(|payload| RetrievedLogEntry::new(entry.index, payload))
                .map_err(|error| LogStorageError::Deserialization {
                    codec: "wincode",
                    index: entry.index,
                    message: error.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RetrievedLogEntries::new(entries))
    }
}

#[cfg(feature = "serde")]
impl<S> SerializedPersistentLogStorageExt for S where S: PersistentLogStorage + ?Sized {}

#[derive(Debug)]
pub struct FileLogStorage {
    path: PathBuf,
    file: File,
    next_index: LogIndex,
    discarded_through: Option<LogIndex>,
    entries: BTreeMap<LogIndex, Vec<u8>>,
}

impl FileLogStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LogStorageError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            sync_directory(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;

        let RecoveredLog {
            entries,
            discarded_through,
            next_index,
        } = recover(&path)?;

        file.seek(SeekFrom::End(0))?;

        Ok(Self {
            path,
            file,
            next_index,
            discarded_through,
            entries,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn discarded_through(&self) -> Option<LogIndex> {
        self.discarded_through
    }

    pub fn next_index(&self) -> LogIndex {
        self.next_index
    }
}

impl PersistentLogStorage for FileLogStorage {
    fn store(
        &mut self,
        message: StoreLogEntry<Vec<u8>>,
    ) -> Result<StoredLogEntry, LogStorageError> {
        let index = self.next_index;
        write_record(&mut self.file, KIND_APPEND, index, &message.payload)?;
        self.entries.insert(index, message.payload);
        self.next_index = self
            .next_index
            .checked_add(1)
            .ok_or_else(|| LogStorageError::InvalidRecord("log index overflow".to_string()))?;
        Ok(StoredLogEntry::new(index))
    }

    fn commit(
        &mut self,
        message: CommitLogEntries,
    ) -> Result<CommittedLogEntries, LogStorageError> {
        let index = message.through;
        if index >= self.next_index {
            return Err(LogStorageError::UnknownLogIndex(index));
        }
        self.file.flush()?;
        self.file.sync_all()?;
        Ok(CommittedLogEntries::new(index))
    }

    fn discard(
        &mut self,
        message: DiscardLogEntries,
    ) -> Result<DiscardedLogEntries, LogStorageError> {
        let index = message.through;
        write_record(&mut self.file, KIND_DISCARD, index, &[])?;
        self.file.flush()?;
        self.file.sync_all()?;
        self.discarded_through = Some(
            self.discarded_through
                .map_or(index, |current| current.max(index)),
        );
        self.entries
            .retain(|entry_index, _| !is_discarded(*entry_index, self.discarded_through));
        Ok(DiscardedLogEntries::new(index))
    }

    fn read(
        &self,
        message: ReadLogEntries,
    ) -> Result<RetrievedLogEntries<Vec<u8>>, LogStorageError> {
        Ok(RetrievedLogEntries::new(
            self.entries
                .iter()
                .filter(|(index, _)| !is_discarded(**index, self.discarded_through))
                .filter(|(index, _)| message.from.is_none_or(|from| **index >= from))
                .map(|(index, payload)| RetrievedLogEntry::new(*index, payload.clone()))
                .collect(),
        ))
    }
}

#[derive(Debug)]
struct RecoveredLog {
    entries: BTreeMap<LogIndex, Vec<u8>>,
    discarded_through: Option<LogIndex>,
    next_index: LogIndex,
}

fn recover(path: &Path) -> Result<RecoveredLog, LogStorageError> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let mut offset = 0;
    let mut entries = BTreeMap::new();
    let mut discarded_through = None;
    let mut next_index = 0;

    while offset < bytes.len() {
        let Some(record) = parse_record(&bytes[offset..])? else {
            break;
        };

        offset += record.consumed;
        next_index = next_index.max(record.index.saturating_add(1));

        match record.kind {
            KIND_APPEND => {
                if !is_discarded(record.index, discarded_through) {
                    entries.insert(record.index, record.payload);
                }
            }
            KIND_DISCARD => {
                discarded_through = Some(
                    discarded_through.map_or(record.index, |current| current.max(record.index)),
                );
                entries.retain(|entry_index, _| !is_discarded(*entry_index, discarded_through));
            }
            _ => {
                return Err(LogStorageError::InvalidRecord(format!(
                    "unknown record kind {}",
                    record.kind
                )));
            }
        }
    }

    Ok(RecoveredLog {
        entries,
        discarded_through,
        next_index,
    })
}

fn is_discarded(index: LogIndex, discarded_through: Option<LogIndex>) -> bool {
    discarded_through.is_some_and(|discarded| index <= discarded)
}

#[derive(Debug)]
struct ParsedRecord {
    kind: u8,
    index: LogIndex,
    payload: Vec<u8>,
    consumed: usize,
}

fn parse_record(bytes: &[u8]) -> Result<Option<ParsedRecord>, LogStorageError> {
    if bytes.len() < HEADER_LEN {
        return Ok(None);
    }
    if &bytes[0..4] != MAGIC {
        return Err(LogStorageError::InvalidRecord("bad magic".to_string()));
    }
    if bytes[4] != VERSION {
        return Err(LogStorageError::InvalidRecord(format!(
            "unsupported version {}",
            bytes[4]
        )));
    }

    let kind = bytes[5];
    let index = u64::from_le_bytes(bytes[8..16].try_into().expect("slice length"));
    let len = u64::from_le_bytes(bytes[16..24].try_into().expect("slice length"));
    let checksum = u64::from_le_bytes(bytes[24..32].try_into().expect("slice length"));
    let len: usize = len
        .try_into()
        .map_err(|_| LogStorageError::InvalidRecord("record length too large".to_string()))?;
    let total_len = HEADER_LEN
        .checked_add(len)
        .ok_or_else(|| LogStorageError::InvalidRecord("record length overflow".to_string()))?;

    if bytes.len() < total_len {
        return Ok(None);
    }

    let payload = &bytes[HEADER_LEN..total_len];
    if checksum_record(kind, index, payload) != checksum {
        return Ok(None);
    }

    Ok(Some(ParsedRecord {
        kind,
        index,
        payload: payload.to_vec(),
        consumed: total_len,
    }))
}

fn write_record(
    file: &mut File,
    kind: u8,
    index: LogIndex,
    payload: &[u8],
) -> Result<(), LogStorageError> {
    let len: u64 = payload
        .len()
        .try_into()
        .map_err(|_| LogStorageError::InvalidRecord("payload too large".to_string()))?;
    let checksum = checksum_record(kind, index, payload);

    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.push(VERSION);
    header.push(kind);
    header.extend_from_slice(&0u16.to_le_bytes());
    header.extend_from_slice(&index.to_le_bytes());
    header.extend_from_slice(&len.to_le_bytes());
    header.extend_from_slice(&checksum.to_le_bytes());

    file.write_all(&header)?;
    file.write_all(payload)?;
    Ok(())
}

fn checksum_record(kind: u8, index: LogIndex, payload: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in [VERSION, kind] {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in index.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in payload {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn sync_directory(path: &Path) -> Result<(), LogStorageError> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
    struct DecisionPayload {
        transaction: u64,
        decision: String,
    }

    fn unique_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "persistent-log-storage-{name}-{}-{nanos}.log",
            std::process::id()
        ))
    }

    #[test]
    fn pls_req_002_append_assigns_increasing_indexes() {
        let path = unique_path("append");
        let mut log = FileLogStorage::open(&path).expect("open log");

        assert_eq!(
            log.store(StoreLogEntry::new(b"first".to_vec()))
                .expect("store first")
                .index,
            0
        );
        assert_eq!(
            log.store(StoreLogEntry::new(b"second".to_vec()))
                .expect("store second")
                .index,
            1
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_003_req_005_commit_message_persists_entries_for_reopen() {
        let path = unique_path("commit");
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let index = log
                .store(StoreLogEntry::new(b"decision".to_vec()))
                .expect("store")
                .index;
            log.commit(CommitLogEntries::through(index))
                .expect("commit through index");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read(ReadLogEntries::all())
                .expect("read entries")
                .entries,
            vec![RetrievedLogEntry::new(0, b"decision".to_vec())]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_004_discard_message_is_applied_after_reopen() {
        let path = unique_path("discard");
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let first = log
                .store(StoreLogEntry::new(b"first".to_vec()))
                .expect("store first")
                .index;
            let second = log
                .store(StoreLogEntry::new(b"second".to_vec()))
                .expect("store second")
                .index;
            log.commit(CommitLogEntries::through(second))
                .expect("commit through second");
            log.discard(DiscardLogEntries::through(first))
                .expect("discard first");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read(ReadLogEntries::all())
                .expect("read entries")
                .entries,
            vec![RetrievedLogEntry::new(1, b"second".to_vec())]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_006_recovery_ignores_torn_trailing_record() {
        let path = unique_path("torn");
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let index = log
                .store(StoreLogEntry::new(b"complete".to_vec()))
                .expect("store complete")
                .index;
            log.commit(CommitLogEntries::through(index))
                .expect("commit complete");
        }

        {
            let mut file = OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("open for torn append");
            file.write_all(b"PLS1\x01\x01")
                .expect("write partial record");
            file.sync_all().expect("sync partial record");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read(ReadLogEntries::all())
                .expect("read entries")
                .entries,
            vec![RetrievedLogEntry::new(0, b"complete".to_vec())]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_007_payload_bytes_are_opaque() {
        let path = unique_path("opaque");
        let payload = vec![0, 159, 146, 150, 255];
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let index = log
                .store(StoreLogEntry::new(payload.clone()))
                .expect("store opaque payload")
                .index;
            log.commit(CommitLogEntries::through(index))
                .expect("commit opaque payload");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read(ReadLogEntries::all())
                .expect("read entries")
                .entries[0]
                .payload,
            payload
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_009_message_read_can_start_from_index() {
        let path = unique_path("read-from");
        let mut log = FileLogStorage::open(&path).expect("open log");
        log.store(StoreLogEntry::new(b"first".to_vec()))
            .expect("store first");
        log.store(StoreLogEntry::new(b"second".to_vec()))
            .expect("store second");

        assert_eq!(
            log.read(ReadLogEntries::from(1))
                .expect("read from index")
                .entries,
            vec![RetrievedLogEntry::new(1, b"second".to_vec())]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_009_protocol_message_dispatches_store_commit_and_read() {
        let path = unique_path("protocol");
        let mut log = FileLogStorage::open(&path).expect("open log");

        assert_eq!(
            PersistentLogStorageProtocol::NAME,
            "persistent_log_storage.v1"
        );

        let stored = log
            .handle(PersistentLogStorageMessage::Store(StoreLogEntry::new(
                b"decision".to_vec(),
            )))
            .expect("store through protocol");
        let PersistentLogStorageReply::Stored(stored) = stored else {
            panic!("expected stored reply");
        };

        assert_eq!(
            log.handle(PersistentLogStorageMessage::Commit(
                CommitLogEntries::through(stored.index)
            ))
            .expect("commit through protocol"),
            PersistentLogStorageReply::Committed(CommittedLogEntries::new(stored.index))
        );

        assert_eq!(
            log.handle(PersistentLogStorageMessage::Read(ReadLogEntries::all()))
                .expect("read through protocol"),
            PersistentLogStorageReply::Retrieved(RetrievedLogEntries::new(vec![
                RetrievedLogEntry::new(0, b"decision".to_vec())
            ]))
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_010_req_011_serde_interface_uses_wincode_payloads() {
        let path = unique_path("wincode");
        let payload = DecisionPayload {
            transaction: 42,
            decision: "commit".to_string(),
        };
        let mut log = FileLogStorage::open(&path).expect("open log");
        let index = log
            .store_serialized(SerializedLogEntry::new(&payload))
            .expect("store serialized payload")
            .index;
        log.commit(CommitLogEntries::through(index))
            .expect("commit serialized payload");

        let recovered = log
            .read_serialized::<DecisionPayload>(ReadLogEntries::all())
            .expect("read serialized payload")
            .entries;

        assert_eq!(recovered, vec![RetrievedLogEntry::new(0, payload)]);

        let _ = std::fs::remove_file(path);
    }
}
