//! Crash-safe file-backed persistent log storage service.
//!
//! The crate-local baseline is documented in `se-design-baseline.md`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mpi::{protocol, task};

const MAGIC: &[u8; 4] = b"PLS1";
const VERSION: u8 = 1;
const KIND_APPEND: u8 = 1;
const KIND_DISCARD: u8 = 2;
const HEADER_LEN: usize = 4 + 1 + 1 + 2 + 8 + 8 + 8;

type LogIndex = u64;

protocol! {
    pub protocol PersistentLogStorageProtocolV1 {
        call store(Vec<u8>) -> Result<u64, String>;
        call commit(u64) -> Result<(), String>;
        call discard(u64) -> Result<(), String>;
        call read(Option<u64>) -> Result<Vec<(u64, Vec<u8>)>, String>;
    }
}

type FileLogStorageBinding =
    PersistentLogStorageProtocolV1::Binding<file_task::FileLogStorageTaskHandle>;
pub type PersistentLogStoreResult = Result<u64, String>;
pub type PersistentLogUnitResult = Result<(), String>;
pub type PersistentLogReadResult = Result<Vec<(u64, Vec<u8>)>, String>;

struct PersistentLogStorageServiceInner {
    binding: FileLogStorageBinding,
    runtime: Mutex<Option<mpi::TaskRuntime<()>>>,
}

impl Drop for PersistentLogStorageServiceInner {
    fn drop(&mut self) {
        self.binding.handle().close();
        let Some(runtime) = self
            .runtime
            .lock()
            .expect("persistent log storage service runtime lock poisoned")
            .take()
        else {
            return;
        };
        runtime
            .join()
            .expect("persistent log storage service task panicked");
    }
}

/// Owning file-backed persistent log storage service instance.
///
/// Clones share ownership of the same running storage task. Dropping the final
/// clone closes the task queue and waits for clean termination.
#[derive(Clone)]
pub struct PersistentLogStorageServiceInstance {
    inner: Arc<PersistentLogStorageServiceInner>,
}

impl PersistentLogStorageServiceInstance {
    /// Store one opaque log payload.
    pub fn store_blocking(
        &self,
        payload: Vec<u8>,
    ) -> Result<PersistentLogStoreResult, mpi::CallError> {
        self.inner.binding.store_blocking(payload)
    }

    /// Commit all log entries through `through`.
    pub fn commit_blocking(&self, through: u64) -> Result<PersistentLogUnitResult, mpi::CallError> {
        self.inner.binding.commit_blocking(through)
    }

    /// Discard all log entries through `through`.
    pub fn discard_blocking(
        &self,
        through: u64,
    ) -> Result<PersistentLogUnitResult, mpi::CallError> {
        self.inner.binding.discard_blocking(through)
    }

    /// Read complete non-discarded log entries starting at `from`, if supplied.
    pub fn read_blocking(
        &self,
        from: Option<u64>,
    ) -> Result<PersistentLogReadResult, mpi::CallError> {
        self.inner.binding.read_blocking(from)
    }

    /// Stop the service and wait for clean termination.
    pub fn stop(&self) {
        self.inner.binding.handle().close();
        let Some(runtime) = self
            .inner
            .runtime
            .lock()
            .expect("persistent log storage service runtime lock poisoned")
            .take()
        else {
            return;
        };
        runtime
            .join()
            .expect("persistent log storage service task panicked");
    }
}

pub fn start_file_log_storage_service(
    path: impl AsRef<Path>,
) -> Result<PersistentLogStorageServiceInstance, String> {
    let storage = FileLogStorage::open(path).map_err(|error| error.to_string())?;
    let (handle, runtime) =
        file_task::FileLogStorageTask::spawn(file_task::FileLogStorageTask::new(storage))
            .map_err(|error| error.to_string())?;
    Ok(PersistentLogStorageServiceInstance {
        inner: Arc::new(PersistentLogStorageServiceInner {
            binding: PersistentLogStorageProtocolV1::bind(handle),
            runtime: Mutex::new(Some(runtime)),
        }),
    })
}

#[cfg(feature = "serde")]
impl PersistentLogStorageServiceInstance {
    pub fn store_serialized_blocking<T>(
        &self,
        payload: T,
    ) -> Result<Result<u64, String>, mpi::CallError>
    where
        T: serde::Serialize,
    {
        let payload =
            match <serde_wincode::SerdeCompat<T> as serde_wincode::wincode::Serialize>::serialize(
                &payload,
            ) {
                Ok(payload) => payload,
                Err(error) => return Ok(Err(format!("wincode log serialization error: {error}"))),
            };
        self.store_blocking(payload)
    }

    pub fn read_serialized_blocking<T>(
        &self,
        from: Option<u64>,
    ) -> Result<Result<Vec<(u64, T)>, String>, mpi::CallError>
    where
        T: serde::de::DeserializeOwned,
    {
        let entries = self.read_blocking(from)?;
        Ok(entries.and_then(|entries| {
            entries
                .into_iter()
                .map(|(index, payload)| {
                    <serde_wincode::SerdeCompat<T> as serde_wincode::wincode::Deserialize>::deserialize(
                        &payload,
                    )
                    .map(|payload| (index, payload))
                    .map_err(|error| {
                        format!("wincode log deserialization error at index {index}: {error}")
                    })
                })
                .collect()
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoreLogEntry<T = Vec<u8>> {
    payload: T,
}

impl<T> StoreLogEntry<T> {
    #[must_use]
    pub const fn new(payload: T) -> Self {
        Self { payload }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StoredLogEntry {
    index: LogIndex,
}

impl StoredLogEntry {
    #[must_use]
    pub const fn new(index: LogIndex) -> Self {
        Self { index }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommitLogEntries {
    through: LogIndex,
}

impl CommitLogEntries {
    #[must_use]
    pub const fn through(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommittedLogEntries {
    through: LogIndex,
}

impl CommittedLogEntries {
    #[must_use]
    pub const fn new(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiscardLogEntries {
    through: LogIndex,
}

impl DiscardLogEntries {
    #[must_use]
    pub const fn through(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiscardedLogEntries {
    through: LogIndex,
}

impl DiscardedLogEntries {
    #[must_use]
    pub const fn new(through: LogIndex) -> Self {
        Self { through }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReadLogEntries {
    from: Option<LogIndex>,
}

impl ReadLogEntries {
    #[cfg(test)]
    #[must_use]
    const fn all() -> Self {
        Self { from: None }
    }

    #[cfg(test)]
    #[must_use]
    const fn from(from: LogIndex) -> Self {
        Self { from: Some(from) }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetrievedLogEntry<T = Vec<u8>> {
    index: LogIndex,
    payload: T,
}

impl<T> RetrievedLogEntry<T> {
    #[must_use]
    pub const fn new(index: LogIndex, payload: T) -> Self {
        Self { index, payload }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetrievedLogEntries<T = Vec<u8>> {
    entries: Vec<RetrievedLogEntry<T>>,
}

impl<T> RetrievedLogEntries<T> {
    #[must_use]
    pub const fn new(entries: Vec<RetrievedLogEntry<T>>) -> Self {
        Self { entries }
    }
}

trait PersistentLogStorage {
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
}

#[derive(Debug)]
enum LogStorageError {
    Io(io::Error),
    InvalidRecord(String),
    UnknownLogIndex(LogIndex),
}

impl fmt::Display for LogStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "persistent log I/O error: {error}"),
            Self::InvalidRecord(message) => write!(f, "invalid persistent log record: {message}"),
            Self::UnknownLogIndex(index) => write!(f, "unknown log index {index}"),
        }
    }
}

impl std::error::Error for LogStorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidRecord(_) | Self::UnknownLogIndex(_) => None,
        }
    }
}

impl From<io::Error> for LogStorageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug)]
struct FileLogStorage {
    file: File,
    next_index: LogIndex,
    discarded_through: Option<LogIndex>,
    entries: BTreeMap<LogIndex, Vec<u8>>,
}

impl FileLogStorage {
    fn open(path: impl AsRef<Path>) -> Result<Self, LogStorageError> {
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
            file,
            next_index,
            discarded_through,
            entries,
        })
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

mod file_task {
    use super::*;

    pub(super) struct FileLogStorageTask {
        storage: FileLogStorage,
    }

    impl FileLogStorageTask {
        pub(super) const fn new(storage: FileLogStorage) -> Self {
            Self { storage }
        }
    }

    #[task(queue_size = 32)]
    impl FileLogStorageTask {
        #[start]
        fn start(_ctx: &mut FileLogStorageTaskContext) {}

        #[call(protocol = PersistentLogStorageProtocolV1::store)]
        fn store(ctx: &mut FileLogStorageTaskContext, payload: Vec<u8>) -> Result<u64, String> {
            ctx.with_state(|state| {
                state
                    .storage
                    .store(StoreLogEntry::new(payload))
                    .map(|stored| stored.index)
                    .map_err(|error| error.to_string())
            })
        }

        #[call(protocol = PersistentLogStorageProtocolV1::commit)]
        fn commit(ctx: &mut FileLogStorageTaskContext, through: u64) -> Result<(), String> {
            ctx.with_state(|state| {
                state
                    .storage
                    .commit(CommitLogEntries::through(through))
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
        }

        #[call(protocol = PersistentLogStorageProtocolV1::discard)]
        fn discard(ctx: &mut FileLogStorageTaskContext, through: u64) -> Result<(), String> {
            ctx.with_state(|state| {
                state
                    .storage
                    .discard(DiscardLogEntries::through(through))
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
        }

        #[call(protocol = PersistentLogStorageProtocolV1::read)]
        fn read(
            ctx: &mut FileLogStorageTaskContext,
            from: Option<u64>,
        ) -> Result<Vec<(u64, Vec<u8>)>, String> {
            ctx.with_state(|state| {
                state
                    .storage
                    .read(ReadLogEntries { from })
                    .map(|entries| {
                        entries
                            .entries
                            .into_iter()
                            .map(|entry| (entry.index, entry.payload))
                            .collect()
                    })
                    .map_err(|error| error.to_string())
            })
        }
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
            "persistent-log-storage-service-{name}-{}-{nanos}.log",
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
        let log = start_file_log_storage_service(path.clone()).expect("start log service");

        let stored = log
            .store_blocking(b"decision".to_vec())
            .expect("store call")
            .expect("store through protocol");

        assert_eq!(stored, 0);
        log.commit_blocking(stored)
            .expect("commit call")
            .expect("commit through protocol");

        assert_eq!(
            log.read_blocking(None)
                .expect("read call")
                .expect("read through protocol"),
            vec![(0, b"decision".to_vec())]
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
        let log = start_file_log_storage_service(path.clone()).expect("start log service");
        let index = log
            .store_serialized_blocking(&payload)
            .expect("store serialized call")
            .expect("store serialized payload");
        log.commit_blocking(index)
            .expect("commit call")
            .expect("commit serialized payload");

        let recovered = log
            .read_serialized_blocking::<DecisionPayload>(None)
            .expect("read serialized call")
            .expect("read serialized payload");

        assert_eq!(recovered, vec![(0, payload)]);

        let _ = std::fs::remove_file(path);
    }
}
