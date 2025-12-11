// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor.
//
// The CodeChat Editor is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
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
const eslintPluginPrettierRecommended = require("eslint-plugin-prettier/recommended");
const eslint = require("@eslint/js");
const { defineConfig } = require("eslint/config");
const tseslint = require("typescript-eslint");

module.exports = defineConfig(
    eslint.configs.recommended,
    tseslint.configs.recommended,
    eslintPluginPrettierRecommended,
    defineConfig([
        {
            // This must be the only key in this dict to be treated as a global ignore. Only global ignores can ignore directories. See the [docs](https://eslint.org/docs/latest/use/configure/configuration-files#globally-ignoring-files-with-ignores).
            ignores: ["src/third-party/**"],
        },
        {
            name: "local",
            rules: {
                "@typescript-eslint/no-unused-vars": [
                    "off",
                    { argsIgnorePattern: "^_" },
                ],
            },
        },
    ]),
);
