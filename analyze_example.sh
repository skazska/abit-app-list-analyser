#!/bin/bash

# Example usage script for the Abitur Analyzer

echo "🎓 Abitur Analyzer - Example Usage"
echo "=================================="

# Test with a few different SNILS from the data
SNILS_LIST=(
    "С25-00946"
    "С25-00479" 
    "С25-00474"
    "147-337-065"
    "157-240-173"
)

for snils in "${SNILS_LIST[@]}"; do
    echo ""
    echo "📊 Analyzing chances for SNILS: $snils"
    echo "----------------------------------------"
    
    cargo run --release -- --snils "$snils"
    
    echo ""
    echo "📄 Generated reports in output/ directory:"
    ls -la output/
    
    echo ""
    echo "Press Enter to continue to next SNILS or Ctrl+C to exit..."
    read
done

echo ""
echo "✅ All examples completed!"
