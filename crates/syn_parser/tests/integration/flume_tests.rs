#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;
    use syn_parser::{create_parser_channel, ParserMessage, start_parser_worker};

    #[test]
    fn test_flume_based_parsing() {
        // Create channels for communication
        let (sender, receiver) = create_parser_channel();
        let (result_sender, result_receiver) = create_parser_channel();
        
        // Start the parser worker in a background thread
        let worker = start_parser_worker(receiver, result_sender);
        
        // Send a file to parse
        let fixture_path = PathBuf::from("tests/fixtures/functions.rs");
        sender.send(ParserMessage::ParseFile(fixture_path)).unwrap();
        
        // Send shutdown signal
        sender.send(ParserMessage::Shutdown).unwrap();
        
        // Wait for the result
        if let ParserMessage::ParseResult(result) = result_receiver.recv().unwrap() {
            let code_graph = result.expect("Failed to parse file");
            assert!(!code_graph.functions.is_empty(), "No functions found in the parsed file");
        } else {
            panic!("Expected ParseResult message");
        }
        
        // Wait for the worker to finish
        worker.join().unwrap();
    }
}
