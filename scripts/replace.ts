import { readFile, writeFile } from 'node:fs/promises'
import { argv, exit, stderr } from 'node:process'

const USAGE = 'Usage: node scripts/replace.ts <file> <find> <replace>'

const args = argv.slice(2)
if (args.length !== 3) {
    stderr.write(`${USAGE}\n`)
    exit(1)
}

const [file, find, replace] = args as [string, string, string]

if (find.length === 0) {
    stderr.write('Error: <find> must not be empty.\n')
    exit(1)
}

try {
    const original = await readFile(file, 'utf8')
    const updated = original.replaceAll(find, replace)
    if (updated !== original) {
        await writeFile(file, updated, 'utf8')
    }
} catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    stderr.write(`Error processing ${file}: ${message}\n`)
    exit(1)
}
