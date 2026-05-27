# Setting up a loop run

## New worktree
git worktree add -b \
  p1-gemini35-flash-direct-15g2x3-par2-20260526-02 \
  ~/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-par2-20260526-02 \
  HEAD

## Setup the run-profile.json

First build ploke-eval in the new directory.

cd ~/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-par2-20260526-01
cargo build -p ploke-eval

Use the profile from the last run. This can be found with:

ls -t ~/.ploke-eval/profiles/prototype1/*.toml | head -n 1


Copy that profile into a new name. For example:

cp /home/brasides/.ploke-eval/profiles/prototype1/gemini35-flash-direct-15g2x3-par2-20260526-051954.toml /home/brasides/.ploke-eval/profiles/prototype1/p1-gemini35-flash-direct-15g2x3-par2-20260526-02.toml

That profile can be edited or changed as necessary, but serves as a good baseline.

Run the setup for the loop:
./target/debug/ploke-eval loop prototype1-setup \
    --campaign p1-gemini35-flash-direct-15g2x3-par2-20260526-01 \
    --profile  \
    --format j
