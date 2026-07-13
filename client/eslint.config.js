// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor.
//
// The CodeChat Editor is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option) any
// later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses/](http://www.gnu.org/licenses/).
//
// `.eslintrc.yml` -- Configure ESLint for this project
// ====================================================
import eslintPluginPrettierRecommended from "eslint-plugin-prettier/recommended";
import css from "@eslint/css";
import eslint from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import globals from "globals";

// Glob matching the JS/TS files the JavaScript/TypeScript configs below should
// apply to. Without this, those configs (and their core rules) also run against
// `.css` files, which crashes since CSS uses a different language.
const jsFiles = ["**/*.{js,mjs,cjs,ts,mts,cts,jsx,tsx}"];

export default defineConfig(
    { files: jsFiles, extends: [eslint.configs.recommended] },
    { files: jsFiles, extends: [tseslint.configs.recommended] },
    { files: jsFiles, extends: [eslintPluginPrettierRecommended] },
    defineConfig([
        {
            // This must be the only key in this dict to be treated as a global
            // ignore. Only global ignores can ignore directories. See the
            // [docs](https://eslint.org/docs/latest/use/configure/configuration-files#globally-ignoring-files-with-ignores).
            ignores: ["src/third-party/**"],
        },
        {
            name: "local",
            files: jsFiles,
            languageOptions: {
                globals: {
                    ...globals.browser,
                },
            },
            rules: {
                "no-unused-vars": "off",
                "@typescript-eslint/no-unused-vars": [
                    "error",
                    {
                        args: "all",
                        argsIgnorePattern: "^_",
                        caughtErrors: "all",
                        caughtErrorsIgnorePattern: "^_",
                        destructuredArrayIgnorePattern: "^_",
                        varsIgnorePattern: "^_",
                        ignoreRestSiblings: true,
                    },
                ],
            },
        },
        {
            name: "css",
            files: ["**/*.css"],
            ignores: ["src/third-party/**"],
            language: "css/css",
            plugins: { css },
            extends: ["css/recommended"],
        },
    ]),
);
