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
/// # `pest_parser.rs` -- Lex source code into code and doc blocks
// ## Imports
//
// ### Standard library
//
// None.
//
// ### Third-party
//
// None.
//
// ### Local
//
// None.

/// # Parser generator
// This macro generates a parser function that converts the provided string into
// a series of code and doc blocks. I'd prefer to use traits, but don't see a
// way to pass the `Rule` types as a usable. (Using `RuleType` means we can't
// access `Rule::file`, etc.)
#[macro_export]
macro_rules! make_parse_to_code_doc_blocks {
    ($parser: ty) => {
        pub fn parse_to_code_doc_blocks(input: &str) -> Vec<$crate::lexer::CodeDocBlock> {
            // While Pest has no problem working with all types of line endings,
            // CodeMirror converts all line endings to `\n` then indexes strings
            // based on that assumption. Normalize line endings to `\n` so that
            // CodeMirror indexes work.
            let normalized_input =
                &String::from_iter(normalize_line_endings::normalized(input.chars()));
            let pairs = <$parser>::parse(Rule::file, normalized_input)
                // The parser should never produce a syntax error.
                .unwrap()
                // The first (and only) element is the `file` token.
                .next()
                .unwrap()
                // Return the contents of this token (code and doc block
                // tokens).
                .into_inner();
            //println!("{:#?}", pairs);
            // Transforms these tokens into code and doc blocks.
            pairs
                .map(|block| match block.as_rule() {
                    Rule::inline_comment => {
                        // Gather all tokens in the inline comment.
                        let mut inline_comment = block.into_inner();
                        let whitespace = inline_comment.next().unwrap().as_str();
                        let inline_comment_delim = inline_comment.next().unwrap().as_rule();
                        // Combine the text of all the inline comments.
                        let comment = &mut inline_comment.fold(String::new(), |mut acc, e| {
                            let s = e.as_str();
                            let inner = &mut e.into_inner();
                            let contents = if let Some(inline_comment_contents) = inner.next() {
                                // For comments which contains contents, include
                                // that.
                                inline_comment_contents.as_str()
                            } else {
                                // For comment which are just a newline, include
                                // that.
                                s
                            };
                            assert!(inner.next().is_none());

                            // Add this string (the raw newline, or the comment
                            // contents) to the accumulator.
                            acc.push_str(contents);
                            acc
                        });

                        // Determine which opening delimiter was used.
                        let _delimiter_index = match inline_comment_delim {
                            Rule::inline_comment_delim_0 => 0,
                            Rule::inline_comment_delim_1 => 1,
                            Rule::inline_comment_delim_2 => 2,
                            _ => unreachable!(),
                        };
                        //println!("Delimiter: {delimiter_index}");

                        //println!("Inline comment: {whitespace}{comment:#?}");
                        let lines = comment.lines().count();
                        $crate::lexer::CodeDocBlock::DocBlock($crate::lexer::DocBlock {
                            indent: whitespace.to_string(),
                            delimiter: "//".to_string(),
                            contents: comment.to_string(),
                            lines,
                        })
                    }
                    Rule::block_comment => {
                        // Gather all tokens in the block comment.
                        let mut block_comment = block.into_inner();
                        let pre_whitespace = block_comment.next().unwrap().as_str();
                        let block_comment_opening_delim = block_comment.next().unwrap().as_rule();
                        let block_comment_pre = block_comment.next().unwrap().as_str();
                        let comment = block_comment.next().unwrap().as_str();
                        let optional_space = block_comment.next().unwrap().as_str();
                        let post_whitespace = block_comment.next().unwrap().as_str();
                        // If this is an EOI, then its string is empty -- which is exactly what we want. Otherwise, use the newline provided by the `block_comment_ending` token.
                        let block_comment_ending = block_comment.next().unwrap().as_str();
                        assert!(block_comment.next().is_none());

                        // Determine which opening delimiter was used.
                        let _opening_delim_index = match block_comment_opening_delim {
                            Rule::block_comment_opening_delim_0 => 0,
                            Rule::block_comment_opening_delim_1 => 1,
                            Rule::block_comment_opening_delim_2 => 2,
                            _ => unreachable!(),
                        };
                        // TODO -- use this in the future.
                        //println!("Opening delimiter index: {}", opening_delim_index);

                        // Build the full comment; include any whitespace
                        // following the comment, rather than discarding it --
                        // this seems safer.
                        let full_comment = format!(
                            "{}{comment}{optional_space}{post_whitespace}{block_comment_ending}",
                            // If there's a newline immediately after the block
                            // comment opening delimiter, include it; omit the
                            // space if that instead follows block comment
                            // opening delimiter.
                            if block_comment_pre == " " {
                                ""
                            } else {
                                block_comment_pre
                            }
                        );

                        // Remove indents, if possible.
                        let mut full_comment = parse_block_comment(&pre_whitespace, &full_comment);
                        // Trim the optional space, if it exists.
                        if !optional_space.is_empty() && full_comment.ends_with(optional_space) {
                            full_comment.pop();
                        }

                        // Transform this to a doc block.
                        //println!("Block comment: {pre_whitespace}{full_comment:#?}");
                        let lines = full_comment.lines().count();
                        $crate::lexer::CodeDocBlock::DocBlock($crate::lexer::DocBlock {
                            indent: pre_whitespace.to_string(),
                            delimiter: "/*".to_string(),
                            contents: full_comment.to_string(),
                            lines,
                        })
                    }
                    Rule::code_block => {
                        //println!("Code block: {:#?}", &block.as_str());
                        $crate::lexer::CodeDocBlock::CodeBlock(block.as_str().to_string())
                    }
                    _ => unreachable!(),
                })
                .collect()
        }
    };
}

#[macro_export]
macro_rules! make_parse_block_comment {
    ($parser: ty) => {
        pub fn parse_block_comment(indent: &str, comment: &str) -> String {
            //println!("Indent: '{indent}', comment: '{comment}'");
            let combined = format!("{}\n{}", indent, comment);
            let Ok(mut pairs) = <$parser>::parse(Rule::dedenter, &combined)
                else {
                    //println!("Block comment cannot be dedented.");
                    // The parse failed -- this comment can't be de-indented.
                    return comment.to_string();
                };
            let dedenter =
                // The first (and only) element is the `dedenter` token.
                pairs.next()
                .unwrap()
                // Return the contents of this token (code and doc block
                // tokens).
                .into_inner();
            // Combine all remaining tokens into a single string.
            //println!("{:#?}", dedenter);
            dedenter.fold(String::new(), |mut acc, e| {
                acc.push_str(e.as_str());
                acc
            })
        }
    };
}

// # Parsers
//
// Each parser is kept in a separate module to avoid name conflicts, since Pest
// generates a `Rule` enum for each grammar.
pub mod c {
    use pest::Parser;
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "lexer/pest/c.pest"]
    struct ThisParser;
    make_parse_to_code_doc_blocks!(ThisParser);
    make_parse_block_comment!(ThisParser);
}

pub mod python {
    use pest::Parser;
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "lexer/pest/python.pest"]
    struct ThisParser;
    make_parse_to_code_doc_blocks!(ThisParser);
    make_parse_block_comment!(ThisParser);
}

// ## Tests
#[cfg(test)]
mod test {
    use indoc::indoc;

    use super::{c, python};
    use crate::lexer::{CodeDocBlock, DocBlock};

    #[test]
    fn test_pest_c_1() {
        assert_eq!(
            c::parse_to_code_doc_blocks(indoc!(
                r#"
                code;
                /* Testing
                   1,

                   2, 3
                 */"#
            )),
            vec![
                CodeDocBlock::CodeBlock("code;\n".to_string()),
                CodeDocBlock::DocBlock(DocBlock {
                    indent: "".to_string(),
                    delimiter: "/*".to_string(),
                    contents: "Testing\n1,\n\n2, 3\n".to_string(),
                    lines: 4,
                })
            ],
        );
        assert_eq!(
            c::parse_to_code_doc_blocks(indoc!(
                r#"
                code;
                /* Testing
                 * 1,
                 *
                 * 2, 3
                 */"#
            )),
            vec![
                CodeDocBlock::CodeBlock("code;\n".to_string()),
                CodeDocBlock::DocBlock(DocBlock {
                    indent: "".to_string(),
                    delimiter: "/*".to_string(),
                    contents: "Testing\n1,\n\n2, 3\n".to_string(),
                    lines: 4,
                })
            ],
        );
    }

    #[test]
    fn test_pest_python_1() {
        assert_eq!(
            python::parse_to_code_doc_blocks(indoc!(
                r#"
                code("""not
                # a comment.""")
                # A comment."#
            )),
            vec![
                CodeDocBlock::CodeBlock(r#"code("""not\na comment.""")\n"#.to_string()),
                CodeDocBlock::DocBlock(DocBlock {
                    indent: "".to_string(),
                    delimiter: "#".to_string(),
                    contents: "A comment.".to_string(),
                    lines: 1,
                })
            ],
        );
    }
}
