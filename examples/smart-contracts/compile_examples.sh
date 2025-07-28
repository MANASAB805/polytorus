#!/bin/bash
# Script to compile WAT examples to WASM

echo "🔧 Compiling PolyTorus Smart Contract Examples..."

# Check if wat2wasm is installed
if ! command -v wat2wasm &> /dev/null; then
    echo "❌ wat2wasm not found. Please install wabt:"
    echo "   cargo install wabt"
    echo "   or visit: https://github.com/WebAssembly/wabt"
    exit 1
fi

# Function to compile a WAT file
compile_wat() {
    local wat_file="$1"
    local wasm_file="${wat_file%.wat}.wasm"
    
    echo "📦 Compiling $(basename "$wat_file")..."
    
    if wat2wasm "$wat_file" -o "$wasm_file"; then
        echo "✅ Generated $(basename "$wasm_file")"
        echo "   Size: $(du -h "$wasm_file" | cut -f1)"
    else
        echo "❌ Failed to compile $(basename "$wat_file")"
        return 1
    fi
}

# Compile all examples
echo ""
echo "Token Contract:"
compile_wat "token/simple_token.wat"

echo ""
echo "Voting Contract:"
compile_wat "voting/simple_voting.wat"

echo ""
echo "Escrow Contract:"
compile_wat "escrow/simple_escrow.wat"

echo ""
echo "🎉 Compilation complete!"
echo ""
echo "📋 Usage examples:"
echo "# Deploy token contract:"
echo "cargo run deploy-contract --wasm-file examples/smart-contracts/token/simple_token.wasm --owner alice --name \"Simple Token\""
echo ""
echo "# Deploy voting contract:"
echo "cargo run deploy-contract --wasm-file examples/smart-contracts/voting/simple_voting.wasm --owner bob --name \"Voting System\""
echo ""
echo "# Deploy escrow contract:"
echo "cargo run deploy-contract --wasm-file examples/smart-contracts/escrow/simple_escrow.wasm --owner charlie --name \"Escrow Service\""
echo ""
echo "# Call a contract method:"
echo "cargo run call-contract --contract <CONTRACT_HASH> --method verify --from alice"