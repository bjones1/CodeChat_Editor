Project 2 submission
====================

Team name: line number support

The specification consists of the file [line-number-spec.md](line-number-spec.md).

Draft submission
----------------

### Dias Jakupov

GitHub userid: diasjakupovkbtu

#### Contributions

TBA

#### Build and run test

TBA

#### Specification review rounds

TBA

#### Last specification review

TBA

### Amy Smith

GitHub userid: Kenzpie

#### Contributions

Wrote first draft of spec and review rounds 1 & 2.

#### Build and run test

Prompt: "Show that you can build and run the project in the current git repo."

- Environment: Windows; VS Code PowerShell terminal; Node.js 26.13.1 and npm 11.20.0.
- `cargo build`: completed successfully.
- `cargo test`: 223 workspace tests and 1 builder test passed. Six tests were skipped, with no failures.
- Execution was verified through the baseline test suite, including browser integration tests. The line-number feature was still at the specification stage.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1 | 3 | Clarify how to distinguish generated markers and add a test to verify beharvior. Clarify line placement, and assert exact markers for selected lines. |
| 2 | 1 | Skip continuation lines represented as opaque and clarify generated numbers stay hidden when the document is in edit mode. |

#### Last specification review

LLM: GPT-6.1 Sol Light.

Prompt: "Run another review to find any remaining issues."

Output: "The line-number spec is ready for submission. It clearly defines which Markdown lines receive markers, how those markers preserve the original file’s line numbers, and when a marker should be skipped."

### Alisher Arginbekov

GitHub userid: aarginbekov

#### Contributions

TBA

#### Build and run test

TBA

#### Specification review rounds

TBA

#### Last specification review

TBA

### Aslan Tasmagambetov

GitHub userid: tasma-07

#### Contributions

TBA

#### Build and run test

TBA

#### Specification review rounds

TBA

#### Last specification review

TBA

Final submission
----------------

TBA