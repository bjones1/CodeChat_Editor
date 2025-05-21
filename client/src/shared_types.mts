// Copyright (C) 2023 Bryan A. Jones.
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
//
// ### Message types
//
// These mirror the same definitions in the Rust webserver, so that the two can
// exchange messages.
export type IdeType = {
    VSCode: boolean;
};

export type ResultOkTypes = {
    LoadFile: string | null;
};

export type MessageResult =
    | {
          Ok: "Void" | ResultOkTypes;
      }
    | {
          Err: string;
      };

export type EditorMessageContents =
    | {
          Update: UpdateMessageContents;
      }
    | {
          CurrentFile: [string, boolean?];
      }
    | {
          Opened: IdeType;
      }
    | {
          RequestClose: null;
      }
    | {
          OpenUrl: string;
      }
    | {
          LoadFile: string;
      }
    | {
          ClientHtml: string;
          // Not included, since this is server->server only.
          //Closed?: null;
      }
    | {
          Result: MessageResult;
      };

export type EditorMessage = {
    id: number;
    message: EditorMessageContents;
};

// The server passes this to the client to load a file. See
// [LexedSourceFile](../../server/src/webserver.rs#LexedSourceFile).
export type CodeChatForWeb = {
    metadata: { mode: string };
    source: {
        doc: DiffableSource;
        doc_blocks: DocBlockJSON[];
        // Added by CodeMirror; not sent to/from the Server.
        selection?: any;
    };
};

export type DiffableSource =
    | {
          /// The source code, in a single string.
          Plain: string;
      }
    | {
          /// The source code, as a series of diffs to apply to the current source.
          Diff: [ChangeSpec];
      };

export type ChangeSpec = {
    /// The index of the start of the change.
    from: Number;
    /// The index of the end of the change; defined for deletions and replacements.
    to: Number | undefined;
    /// The text to insert/replace; an empty string indicates deletion.
    insert: String;
};

// How a doc block is stored using CodeMirror.
export type DocBlockJSON = [
    // From
    number,
    // To
    number,
    // Indent
    string,
    // Delimiter
    string,
    // Contents
    string,
];

export type UpdateMessageContents = {
    file_path: string;
    contents: CodeChatForWeb | undefined;
    cursor_position: number | undefined;
    scroll_position: number | undefined;
};
