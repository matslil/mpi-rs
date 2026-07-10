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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEntry {
    pub index: LogIndex,
    pub payload: Vec<u8>,
}

pub trait PersistentLogStorage {
    fn append(&mut self, payload: &[u8]) -> Result<LogIndex, LogStorageError>;
    fn commit_through(&mut self, index: LogIndex) -> Result<(), LogStorageError>;
    fn discard_through(&mut self, index: LogIndex) -> Result<(), LogStorageError>;
    fn read_entries(&self) -> Result<Vec<LogEntry>, LogStorageError>;
}

#[derive(Debug)]
pub enum LogStorageError {
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
    fn append(&mut self, payload: &[u8]) -> Result<LogIndex, LogStorageError> {
        let index = self.next_index;
        write_record(&mut self.file, KIND_APPEND, index, payload)?;
        self.entries.insert(index, payload.to_vec());
        self.next_index = self
            .next_index
            .checked_add(1)
            .ok_or_else(|| LogStorageError::InvalidRecord("log index overflow".to_string()))?;
        Ok(index)
    }

    fn commit_through(&mut self, index: LogIndex) -> Result<(), LogStorageError> {
        if index >= self.next_index {
            return Err(LogStorageError::UnknownLogIndex(index));
        }
        self.file.flush()?;
        self.file.sync_all()?;
        Ok(())
    }

    fn discard_through(&mut self, index: LogIndex) -> Result<(), LogStorageError> {
        write_record(&mut self.file, KIND_DISCARD, index, &[])?;
        self.file.flush()?;
        self.file.sync_all()?;
        self.discarded_through = Some(
            self.discarded_through
                .map_or(index, |current| current.max(index)),
        );
        self.entries
            .retain(|entry_index, _| !is_discarded(*entry_index, self.discarded_through));
        Ok(())
    }

    fn read_entries(&self) -> Result<Vec<LogEntry>, LogStorageError> {
        Ok(self
            .entries
            .iter()
            .filter(|(index, _)| !is_discarded(**index, self.discarded_through))
            .map(|(index, payload)| LogEntry {
                index: *index,
                payload: payload.clone(),
            })
            .collect())
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
    use std::time::{SystemTime, UNIX_EPOCH};

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

        assert_eq!(log.append(b"first").expect("append first"), 0);
        assert_eq!(log.append(b"second").expect("append second"), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_003_req_005_commit_through_persists_entries_for_reopen() {
        let path = unique_path("commit");
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let index = log.append(b"decision").expect("append");
            log.commit_through(index).expect("commit through index");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read_entries().expect("read entries"),
            vec![LogEntry {
                index: 0,
                payload: b"decision".to_vec(),
            }]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_004_discard_through_is_applied_after_reopen() {
        let path = unique_path("discard");
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let first = log.append(b"first").expect("append first");
            let second = log.append(b"second").expect("append second");
            log.commit_through(second).expect("commit through second");
            log.discard_through(first).expect("discard first");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read_entries().expect("read entries"),
            vec![LogEntry {
                index: 1,
                payload: b"second".to_vec(),
            }]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_006_recovery_ignores_torn_trailing_record() {
        let path = unique_path("torn");
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let index = log.append(b"complete").expect("append complete");
            log.commit_through(index).expect("commit complete");
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
            log.read_entries().expect("read entries"),
            vec![LogEntry {
                index: 0,
                payload: b"complete".to_vec(),
            }]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pls_req_007_payload_bytes_are_opaque() {
        let path = unique_path("opaque");
        let payload = vec![0, 159, 146, 150, 255];
        {
            let mut log = FileLogStorage::open(&path).expect("open log");
            let index = log.append(&payload).expect("append opaque payload");
            log.commit_through(index).expect("commit opaque payload");
        }

        let log = FileLogStorage::open(&path).expect("reopen log");
        assert_eq!(
            log.read_entries().expect("read entries")[0].payload,
            payload
        );

        let _ = std::fs::remove_file(path);
    }
}
