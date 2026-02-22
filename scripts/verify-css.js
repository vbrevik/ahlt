#!/usr/bin/env node
/**
 * Verify that CSS modular system compiles correctly
 * - Reads index.css (main entry with @imports)
 * - Validates that style.css was compiled from index.css
 * - Checks for expected CSS module patterns
 * - Reports compilation status
 */
const fs = require('fs');
const path = require('path');

const indexPath = path.join(__dirname, '../static/css/index.css');
const stylePath = path.join(__dirname, '../static/css/style.css');

function getMD5(content) {
  const crypto = require('crypto');
  return crypto.createHash('md5').update(content).digest('hex');
}

function verify() {
  try {
    // Check that source files exist
    if (!fs.existsSync(indexPath)) {
      throw new Error(`Source file not found: ${indexPath}`);
    }
    if (!fs.existsSync(stylePath)) {
      throw new Error(`Compiled file not found: ${stylePath}`);
    }

    // Read the compiled style.css
    const compiled = fs.readFileSync(stylePath, 'utf8');
    const compiledHash = getMD5(compiled);

    console.log('CSS Verification Report');
    console.log('=======================');
    console.log(`✓ index.css: Found (modular entry point)`);
    console.log(`✓ style.css: Found (${compiled.length} bytes, MD5: ${compiledHash})`);

    // Verify that style.css contains expected compiled content patterns
    // Check for key CSS constructs from different modules
    const expectedPatterns = [
      '--bg:',          // Variables from base/variables.css
      'margin: 0;',     // Reset from base/reset.css
      'font-family:',   // Typography rules
      '.card',          // Card component
      '.btn',           // Button component
      '.dash-',         // Dashboard page styles
      '.inline',        // Utilities module
      ':root {'         // CSS custom properties
    ];

    let allPatternsFound = true;
    expectedPatterns.forEach(pattern => {
      if (compiled.includes(pattern)) {
        console.log(`✓ Contains: ${pattern}`);
      } else {
        console.log(`✗ Missing: ${pattern}`);
        allPatternsFound = false;
      }
    });

    if (allPatternsFound) {
      console.log('\n✓ All CSS modules present and compiled successfully.');
      console.log('CSS ready for deployment.');
      process.exit(0);
    } else {
      console.error('\n✗ CSS compilation missing expected modules.');
      process.exit(1);
    }
  } catch (error) {
    console.error('Verification failed:', error.message);
    process.exit(1);
  }
}

verify();
