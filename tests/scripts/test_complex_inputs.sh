#!/bin/bash
set -e

# Compile the example
cargo build --example complex_inputs

# Define valid input
INPUT=$(cat <<EOF
{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0"}},"id":1}
{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
{"jsonrpc":"2.0","method":"tools/call","params":{"name":"name_shrimp","arguments":{"tank":{"shrimp":[{"name":"Bubba"},{"name":"Gump"}]},"extra_names":["Jacques"]}},"id":2}
EOF
)

# Run and capture output
echo "$INPUT" | ./target/debug/examples/complex_inputs > output_complex.json

# Check output
if grep -q "Bubba" output_complex.json && grep -q "Jacques" output_complex.json; then
  echo "Verification PASSED: Found expected shrimp names."
else
  echo "Verification FAILED: Expected names not found."
  cat output_complex.json
  exit 1
fi
