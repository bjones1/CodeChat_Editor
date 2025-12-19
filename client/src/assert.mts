// Copyright (C) 2025 Bryan A. Jones.
//
// This file is part of the CodeChat Editor. The CodeChat Editor is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//
// The CodeChat Editor is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// the CodeChat Editor. If not, see
// [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
//
// `assert.mts`
// =============================================================================
//
// Provide a simple `assert` function to check conditions at runtime. Using
// things like [assert](https://nodejs.org/api/assert.html) causes problems --
// somehow, this indicates that the code is running in a development environment
// (see
// [this](https://github.com/micromark/micromark/issues/87#issuecomment-908924233)).
// Taken from the TypeScript
// [docs](https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#assertion-functions).
export function assert(condition: boolean, msg?: string): asserts condition {
    if (!condition) {
        throw new Error(msg);
    }
}
