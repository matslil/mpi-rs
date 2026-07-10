//! Transaction identifiers and transactional envelopes.

use core::fmt;

use crate::session::{EndpointId, SessionId};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
