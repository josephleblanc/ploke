use std::io;

struct AppState {
    chat_history: Vec<Messages>, // Conversation History
    input_buffer: String, // User's current input
    rag_context: RagContext, // From backend
    message_queue: flume::Receiver<BackendMessage>,

    // ... more state fields here
}

#[derive(Clone)]
struct AppChannels {
    ui_to_backend: flume::Sender<BackendRequest>,
    backend_to_ui: flume::Sender<UiUpdate>
}

struct Messages {
    // TODO: Decide what these look like.
    // Questions:
    //  - Do these hold metadata from the LLM's response?
    //  - Is there some way to design this struct that will facilitate the user being able to go in
    //  and edit the LLM's code suggestions?
    //  - To what degree should `Messages` be structured? For example, do we want to have a field
    //  for `text_response` and `code_suggestion`? This would necessitate a processing step between
    //  the raw response from the LLM's API, e.g. if the LLM usually puts the code snippets into
    //  triple backticks, we would need to identify and extract those segments of the LLM response.
    //  - What methods would we want to implement on the `Message` so it can be displayed if we
    //  decide to go the route of a more structured `Messages` data structure?
    //  - If we want to implement a way to approve/deny/edit the LLM's code suggestions, how do we
    //  add the TUI elements that would allow for this kind of user interaction?
    //  
    // Re: Code snippets from LLM:
    //  Advanced feature ideas:
    //  1. Buffering code suggestions with linting
    //      We are currently using the `span` of a given node, e.g. a `FunctionNode` in the code
    //      graph backend. Is there some way we can hook into rust-analyzer and create a buffer file
    //      that automatically will provide linting here?
    //  2. git-like conversation control
    //      We want to provide a way for the user to manage the conversation history effectively.
    //      Ideally we can implement something extremely similar to git commands to manage this
    //      interaction, possibly using git itself (though this would present some challenges, as
    //      the user's crate likely is already in a git repo). The goal would be to enable a
    //      branching structure of conversation history that allows the user to revisit earlier
    //      points in the conversation, branch the conversation and switch branches, and merge
    //      different conversations (though this would require careful design, possibly querying
    //      the LLM with both branches and suggesting how they could be merged). 
    //      This feature would need to be implemented across several data structures, but is there
    //      something we can do now to lay the foundation for this later feature?
}

struct RagContext {
    // NOTE: RAG backend interface not yet designed.
    //
    // I want to understand better how the TUI works before designing the interface with the RAG
    // backend.
    // Will need to look at other files in the project and decide what the pipeline looks like to
    // and from the RAG for this to work.
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Initialize app state and channels
    let (tx, rx) = flume::unbounded();
    let mut app = AppState::new(tx.clone());
    
    tokio::spawn(async move {
        backend_processor(rx).await; // Handles DB/LLM comms
    });

    let mut event_stream = crossterm::event::EventStream::new();

    loop {
         terminal.draw(|f| ui::render(&app, f))?;

         tokio::select! {
             // Handle user input events
             Some(event) = event_stream.next() => {
                 handle_input(&mut app, event?);
             }

             // Handle backend messages
             Ok(msg) = app.message_queue.recv_async() => {
                 process_backend_message(&mut app, msg);
             }

             // Frame rate limiter (60 FPS)
             _ = sleep(Duration::from_millis(16)) => {
                 // Force redraw even if no events
             }
         }
    }
}
