import prettier from 'eslint-config-prettier';
import { fileURLToPath } from 'node:url';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';
import { includeIgnoreFile } from '@eslint/compat';
import svelteConfig from './svelte.config.js';

const gitignorePath = fileURLToPath(new URL('../../.gitignore', import.meta.url));

export default defineConfig(
    includeIgnoreFile(gitignorePath),
    js.configs.recommended,
    ...ts.configs.recommended,
    ...svelte.configs.recommended,

    // These lines prevent ESLint from applying any formatting
    // The formatting is handled by Prettier
    prettier,
    ...svelte.configs.prettier,
    {
        ignores: ['bindings/**', 'dist/**']
    },
    {
        languageOptions: {
            globals: { ...globals.browser, ...globals.node }
        },
        rules: {
            'no-undef': 'off'
        }
    },
    {
        files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
        languageOptions: {
            parserOptions: {
                projectService: true,
                extraFileExtensions: ['.svelte'],
                parser: ts.parser,
                svelteConfig
            }
        }
    }
);
