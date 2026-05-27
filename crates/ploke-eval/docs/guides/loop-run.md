# Setting up a loop run

## New worktree
git worktree add -b \
  p1-gemini35-flash-direct-15g2x3-par2-20260526-01 \
  ~/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-par2-20260526-01 \
  HEAD

## Setup the run-profile.json

First build ploke-eval in the new directory.

cd ~/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-par2-20260526-164443
cargo build -p ploke-eval

Run the setup for the loop:
~/code/ploke/target/debug/ploke-eval loop prototype1-setup \
    --campaign p1-gemini35-flash-direct-15g2x3-par2-20260526-164443 \
    --profile /home/brasides/.ploke-eval/profiles/prototype1/gemini35-flash-direct-15g2x3-par2-20260526-051954.toml \
    --format json
