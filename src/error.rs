use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EdfError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
    
    #[error("File contains format errors")]
    FormatError,
    
    #[error("Signal index {0} out of range")]
    InvalidSignalIndex(usize),
    
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),
    
    #[error("File is discontinuous")]
    DiscontinuousFile,
    
    #[error("Memory allocation error")]
    MemoryError,
    
    #[error("Invalid header size")]
    InvalidHeader,
    
    #[error("Invalid number of signals: {0}")]
    InvalidSignalCount(i32),
    
    #[error("Physical min equals physical max")]
    PhysicalMinEqualsMax,
    
    #[error("Digital min equals digital max")]
    DigitalMinEqualsMax,
}

pub type Result<T> = std::result::Result<T, EdfError>;
