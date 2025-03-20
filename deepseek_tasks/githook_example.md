I'll modify the example to include output redirection while maintaining WIP flexibility. Here's an adjusted version directing all validation output to a file while keeping user messages visible:

```bash
#!/usr/bin/env bash
# .git/hooks/pre-commit

BREAKING_FEATURE="breaking_changes"
OUTPUT_FILE="GIT_HOOK_OUTPUT.txt"

# Clear previous output
> "$OUTPUT_FILE"

# Handle WIP commits
if [ -n "$ALLOW_BREAKING_WIP" ]; then
    echo "⚠️  WIP commit - writing partial output to $OUTPUT_FILE"
    echo "WIP COMMIT" >> "$OUTPUT_FILE"
    cargo check --features "$BREAKING_FEATURE" >> "$OUTPUT_FILE" 2>&1 || true
    exit 0
fi

# Full validation logic
if grep -qr "#\[cfg(feature = \"$BREAKING_FEATURE\")]" crates/syn_parser/src; then
    echo "🔍 Running full validation (results in $OUTPUT_FILE)"
    cargo test --features "$BREAKING_FEATURE" >> "$OUTPUT_FILE" 2>&1
    TEST_RESULT=$?
    
    # Show brief status but preserve details in file
    if [ $TEST_RESULT -ne 0 ]; then
        echo "❌ Tests failed - see $OUTPUT_FILE"
        exit $TEST_RESULT
    else
        echo "✅ Validation passed"
    fi
else
    echo "➡️  Normal validation"
    cargo check >> "$OUTPUT_FILE" 2>&1 && cargo test >> "$OUTPUT_FILE" 2>&1
    exit $?
fi
```

Key changes from previous example:
1. Stores technical output in `GIT_HOOK_OUTPUT.txt`
2. Preserves user-facing messages in terminal
3. Captures artifacts even for WIP commits
4. Shows pass/fail status while keeping detailed logs

This maintains:
- WIP bypass capability
- Feature-based validation
- Clear developer feedback
- Full output preservation
