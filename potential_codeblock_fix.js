// This is a code snippet of a turndown rule that
// might help fix issues with "&lt;" and "&gt;" being rendered as
// those escape sequences rather than the corresponding angle
// bracket characters.
// This does perform the correct operations to correctly convert code blocks.
// Unfortunately, the parser starts at the most-nested elements, so
// the parent elements of the <code> elements then undo this conversion.
// One possible (BAD!) solution would be to create another instance of
// the turndownService in such a way that this rule is its only rule, and
// then apply that to the entire document at the end. Definitely a last resort.
interface AngleBracketMap {
    "&lt;": string,
    "&gt;": string
};
turndownService.addRule("code blocks angle brackets",
    {
        filter: ["code"],
        replacement: function (content: string, node: any) {
            let angleBracketMap: AngleBracketMap = {
                "&lt;": "<",
                "&gt;": ">"
            };
            let formattedString: string =  "`" + content.replace(/(&lt;)|(&gt;)/g, function(m) {return angleBracketMap[m as keyof AngleBracketMap]}) + "`";
            console.log(formattedString);
            return formattedString;
        }
    });
