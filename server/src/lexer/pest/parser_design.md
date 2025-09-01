Parser design
=============

The CodeChat Editor uses the [Pest parser](https://pest.rs/), a Rust
implementation of a parsing expression grammar (or PEG). The purpose of the
parser from a CodeChat Editor perspective is to classify a source file into code
blocks and doc blocks. To accomplish this goal, grammar files (`.pest`) are
divided into:

*   A shared grammar ([shared.pest](shared.pest)), which contains basic
    definitions applicable to all languages;
*   A language-specific grammar, which builds on these shared definitions by
    providing necessary language-specific customizations.

In particular, a language-specific grammar must provide:

*   The definition of a `doc_block`; for most languages, `doc_block = _{
    inline_comment | block_comment }`. However, languages which lack an inline
    comment (such as CSS) or a block comment (such as Python) would contain only
    the appropriate comment type.

*   Inline comment definitions:

    *   Opening inline delimiter(s) supported by the language. Three inline
        comment delimiters must be defined for a language. For C, this is:

        ```
        inline_comment_delims  = _{ inline_comment_delim_0 }
        inline_comment_delim_0 =  { "//" }
        inline_comment_delim_1 =  { unused }
        inline_comment_delim_2 =  { unused }
        ```

    *   A token which defines characters in the body of on an inline comment.
        For Python, this is:

        ```
        inline_comment_char = { not_newline }
        ```

*   Block comment definitions: provide opening and closing delimiter
    definitions. For C, this is:

    ```
    block_comment                 =  { block_comment_0 }
    block_comment_opening_delim_0 =  { "/*" }
    block_comment_opening_delim_1 =  { unused }
    block_comment_opening_delim_2 =  { unused }
    block_comment_closing_delim_0 =  { "*/" }
    block_comment_closing_delim_1 =  { unused }
    block_comment_closing_delim_2 =  { unused }
    ```

*   `code_line_token`, a token used to recognize tokens in a code line.