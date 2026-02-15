#!/bin/bash
# Test SQL Transpiler

PROJECT_ROOT=$(pwd)
WRAPPER="$PROJECT_ROOT/aion"

if [ ! -f "$WRAPPER" ]; then
    echo "Wrapper not found at $WRAPPER"
    exit 1
fi

INPUT="tests/fixtures/006_sql.ai"
OUTPUT="tests/output.sql"

echo "🚀 Transpiling $INPUT to SQL..."
$WRAPPER transpile $INPUT -o $OUTPUT

if [ $? -ne 0 ]; then
    echo "❌ Transpilation failed"
    exit 1
fi

echo "📄 Generated SQL:"
cat $OUTPUT

echo "🔍 Verifying content..."
if grep -q "CREATE OR REPLACE FUNCTION is_adult" $OUTPUT; then
    echo "✅ is_adult found"
else
    echo "❌ is_adult NOT found"
    exit 1
fi

if grep -q "RETURNS BOOLEAN" $OUTPUT; then
    echo "✅ RETURNS BOOLEAN found"
else
    echo "❌ RETURNS BOOLEAN NOT found"
    exit 1
fi

if grep -q "price - discount" $OUTPUT; then
    echo "✅ Logic found"
else
    echo "❌ Logic NOT found"
    exit 1
fi

echo "✨ SQL Transpilation Test Passed!"
rm $OUTPUT
