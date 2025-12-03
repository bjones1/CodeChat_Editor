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
// `shared_types.mts` -- Shared type definitions
// =============================================
// The time, in ms, to wait between the last user edit and sending updated data to the Server.
export const autosave_timeout_ms = 300;

// Produce a whole random number. Fractional numbers aren't consistently converted to the same number. Note that the mantissa of a JavaScript `Number` is 53 bits per the [docs](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Number#number_encoding). To be certain, also round the result.
export const rand = () => Math.round(Math.random() * 2 ** 53);

// ### Message types
//
// These mirror the same definitions in the Rust webserver, so that the two can
// exchange messages. All these files are build by running `cargo test
// export_bindings`.
import { EditorMessageContents } from "./rust-types/EditorMessageContents.js";
import { EditorMessage } from "./rust-types/EditorMessage.js";
import { CodeChatForWeb } from "./rust-types/CodeChatForWeb.js";
import { CodeMirrorDiffable } from "./rust-types/CodeMirrorDiffable.js";
import { CodeMirror } from "./rust-types/CodeMirror.js";
import { StringDiff } from "./rust-types/StringDiff.js";
import { CodeMirrorDocBlockTuple } from "./rust-types/CodeMirrorDocBlockTuple.js";
import { UpdateMessageContents } from "./rust-types/UpdateMessageContents.js";
import { ResultOkTypes } from "./rust-types/ResultOkTypes.js";
import { ResultErrTypes } from "./rust-types/ResultErrTypes.js";

// Manually define this, since `ts-rs` can't export `webserver.MessageResult`.
type MessageResult = { Ok: ResultOkTypes } | { Err: ResultErrTypes };

export type {
    EditorMessageContents,
    CodeMirror,
    CodeMirrorDocBlockTuple,
    CodeChatForWeb,
    StringDiff,
    CodeMirrorDiffable,
    UpdateMessageContents,
    EditorMessage,
    MessageResult,
};
