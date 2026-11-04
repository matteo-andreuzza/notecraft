//! The reader's error type: every malformed, hostile or unsupported input ends here.

/// Why a file could not be read.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum OneError {
    /// The bytes break a rule of the specification at absolute file offset `at`.
    #[error("damaged OneNote file at byte {at}: {what}")]
    Corrupt {
        /// Absolute offset in the file where the problem was found.
        at: u64,
        /// What was wrong, in words a user can pass on.
        what: String,
    },
    /// A valid file using something this reader does not handle yet.
    #[error("unsupported OneNote file: {0}")]
    Unsupported(String),
    /// The section is password-protected ([MS-ONESTORE] §2.5.19 ObjectDataEncryptionKeyV2FNDX).
    #[error("password-protected sections can't be opened yet")]
    Encrypted,
    /// Over one of the reader's limits (see [`crate::Limits`]).
    #[error("OneNote file too large: {0}")]
    TooLarge(String),
}

impl OneError {
    /// A [`OneError::Corrupt`] at `at`.
    pub(crate) fn corrupt(at: u64, what: impl Into<String>) -> Self {
        Self::Corrupt { at, what: what.into() }
    }
}

/// Result alias.
pub type Result<T> = std::result::Result<T, OneError>;
