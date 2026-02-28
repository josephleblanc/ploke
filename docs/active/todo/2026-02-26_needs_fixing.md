# Stuff that needs fixing

1. Long wait after `/index start <crate-name>` on some targets

Even with a remote embedding model this takes a while to run through parsing.
So I guess I need to start profiling to at least get a sense of what is taking
so long.

Probably the easiest way to do this is look at the indexing pipeline and just
add some time deltas around when functions are called, then write those to a
tracing::info! target like "timing" or something, and add a tracing subscriber
to write those into a file.

A more robust approach would be to setup an event listener or receiver, then
send events with the time delta for different functions.

2. Parallelize embeddings more

It seems like remote embedding takes way too long, which doesn't really make
sense since we should be able to just send as many requests as we want in
parallel. Like i think we are sending just 8 or something in parallel, but
realistically there isn't any reason why we shouldn't be sending 100 at a time.
Try it out, maybe run some tests sending 8 vs 100 size batches to confirm the
parallelization cuts down on embedding time as expected, which should more or
less be just time before/after embedding divided by number of parallel channels
used.

3. `/help` message somewhat broken

When I use the `/help` command running ploke-tui, I'm seeing some weird
coloration on the first couple paragraphs, likely due to either the content of
the `/help` message or possibly our markdown renderer.

Also, I'm not sure we really have a good way of testing the color of the
messages here. Might be good to take a look.

4. Need token cancellation support

I think we might have some framework for token cancellation in terms of
generating new requests from the chat in ploke-tui, maybe not. Actually, we
should at least add a way for a key like `Esc` or `Ctrl-c` to cause the token
generation or agent tool-calling loop to short-circuit. Right now we don't
really have a way for the user to stop the agent loop, which feels bad.

5. Pending code edits are weird

Pending code edits are just inherited to the current session, even before
loading a database or indexing a workspace. That doesn't really make sense
since the changes would be applied to something that isn't the current crate
focus, and could easily be disorienting. Instead we should probably just change
all pending changes to be changed to "cancelled" or something when the
application is opened.

6. Save conversation for analysis

I'm not sure what we have for saving conversation history for analysis, but it
would be good to have a fiarly basic converssation history that saves what the
user sees in the chat history. If we don't ahve something for this right now,
we should add it. I think we have something for this to save a json or
something through tracing, but I don't recall.

What I'd like to start doing is having a way of looking through how the LLM is
using the tools and how the context window is changing, to try finding patterns
in how the tools are being used inefficiently, then either try different
prompting strategies or different tool use flows or something.

7. Develop some eval structure

Kind of related to (6) above, but we don't really have a way of measuring how
efficient or useful our tools are. Even just a basic way of measuring the
success or failure of the tools would be good, but also trying to get an idea
of whether or not the LLMs are using teh tools made available would be good.
I'm noticing that there are different behaviors for different LLMs, and more
guidance around tool usage, through tool descriptions and feedback, mgiht be a
good idea, since I'm seeing the LLMs do things like read the first 100 lines of
a file when they are looking for a certain struct, then look through different
lines without really having a good reason for pulling those particular lines.

Maybe we should either adda  new tool or include some new fields in our current
tools, like in the search and file tools include a "goal" field that encourages
the LLM to explain what they are hoping to do with the tool call, maybe even
followed up by a request for feedback like "did you find what you were looking
for?". which might help us with evaluating the effectiveness of the available
tools, or help guide us on crafting more prompts or a tool-use flow or tool
calling feedback and suggestions to feed back to the LLM if it answers that it
did not achieve the goal. Kind of a customer survey for LLMs that would guide
our design. Not perfect, but it could be helpful, at least worth a shot.

8. Annoying UI quirk

One annoying thing I'm noticing with the UI is that when I click with the mouse
on a longer message, it snaps the focus so the message end is at the bottom of
the screen, which is annoying. I kind of wish it would just select the message
instead, since the change of viewed text on click like that is disorienting.

9. Experiment with context management

Related to (7), but I'm noticing that there are some pretty long agentic tool
call loops, resulting in higher context windows. I'd like to experiment with
the TTL idea again, or maybe asking the LLM if they want to pin some results.
This idea is also tied to the concept of doing evals, since we would really
want to have some objective way of measuring the effectiveness of these
changes.

This really points to a larger issue, which is that we want to have repeatable
sequences of tasks that are similar to the way we imagine the user will utilize
this application, but doing so is kind of difficult since the user will be
responding to the LLM, so using a canned series of requests mgiht not be very
effective. It might be worth just starting with a request to make a change,
then tracking some statistics around how efficiently the tools and context
window are being used, but longer-term it would be nice to somehow automate a
back-and-forth. My first instinct here is to try using different LLM's
together, maybe by having a harness which instructs one LLM to be the user or
something, but that might be more complex to set up. Maybe a good place to
start would be looking into studies trying to do these kinds of evaluations and
seeing what we can find. There are fewer Rust-focused studies that I've found,
but its worth a shot.
