use crate::parser::graph::CodeGraph;
use flume::{Receiver, Sender};
use std::path::PathBuf;

/// Message types for parser communication
#[derive(Debug)]
pub enum ParserMessage {
    /// Request to parse a file
    ParseFile(PathBuf),
    /// Result of parsing a file
    ParseResult(Result<CodeGraph, syn::Error>),
    /// Shutdown the parser
    Shutdown,
}

/// Creates a channel pair for parser communication
pub fn create_parser_channel() -> (Sender<ParserMessage>, Receiver<ParserMessage>) {
    flume::unbounded()
}

/// Creates a bounded channel pair for parser communication with specified capacity
pub fn create_bounded_parser_channel(capacity: usize) -> (Sender<ParserMessage>, Receiver<ParserMessage>) {
    flume::bounded(capacity)
}
