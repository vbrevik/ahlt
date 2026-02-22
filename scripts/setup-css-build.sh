#!/bin/bash
# Setup PostCSS build pipeline for modular CSS

set -e

echo "Setting up CSS build pipeline..."
echo ""

# Install dependencies
if [ ! -d "node_modules" ]; then
    echo "Installing npm dependencies..."
    npm install -D postcss postcss-cli postcss-import
else
    echo "npm dependencies already installed"
fi

echo ""
echo "Building CSS..."
npx postcss static/css/index.css -o static/css/style.css.new

echo ""
echo "Verifying byte-for-byte match..."
if cmp -s static/css/style.css static/css/style.css.new; then
    echo "✓ Compiled CSS is byte-identical to original"
    rm static/css/style.css.new
    echo "✓ CSS modularization complete!"
    echo ""
    echo "Next: Update HTML templates to link static/css/index.css instead of static/css/style.css"
else
    echo "✗ WARNING: Compiled CSS differs from original"
    echo "Differences:"
    diff -u static/css/style.css static/css/style.css.new | head -50
fi
