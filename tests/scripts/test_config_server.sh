#!/bin/bash
set -e

# Compile the example
cargo build --example fastmcp_config

# Define inputs
INPUT=$(cat <<EOF
{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0"}},"id":1}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
{"jsonrpc":"2.0","method":"resources/read","params":{"uri":"config://settings"},"id":2}
{"jsonrpc":"2.0","method":"tools/call","params":{"name":"set_setting","arguments":{"key":"debug","value":true}},"id":3}
{"jsonrpc":"2.0","method":"resources/read","params":{"uri":"config://settings"},"id":4}
EOF
)

# Run and capture output
echo "$INPUT" | ./target/debug/examples/fastmcp_config > output.json

# Check output (simple grep check)
if grep -q "debug" output.json && grep -q "true" output.json; then
  echo "Verification PASSED: Found updated config value."
else
  echo "Verification FAILED: Updated config value not found."
  cat output.json
  exit 1
fi
