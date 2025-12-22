#!/usr/bin/env node

import fs from 'fs'
import path from 'path'

// Parse CLI arguments
const [, , filePathArg, expressionArg] = process.argv

if (!filePathArg || !expressionArg) {
    console.error('Usage: node replace.mjs <filePath> <oldValue=>newValue>')
    process.exit(1)
}

const absolutePath = path.resolve(filePathArg)

// Validate expression format
const expressionMatch = expressionArg.split('=>')
if (!expressionMatch) {
    console.error('Invalid expression format. Use oldValue=>newValue')
    process.exit(1)
}

const [oldValue, newValue] = expressionMatch

// Read file
let fileContent
try {
    fileContent = fs.readFileSync(absolutePath, 'utf8')
} catch (err) {
    console.error(`Error reading file at ${filePathArg}:`, err.message)
    process.exit(1)
}

// Replace content
const updatedContent = fileContent.replaceAll(oldValue, newValue)

// Write updated content back to file
try {
    fs.writeFileSync(absolutePath, updatedContent, 'utf8')
    console.log(`Successfully replaced "${oldValue}" with "${newValue}" in ${filePathArg}`)
} catch (err) {
    console.error(`Error writing to file at ${filePathArg}:`, err.message)
    process.exit(1)
}