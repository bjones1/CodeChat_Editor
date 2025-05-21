function extend (destination) {
  for (var i = 1; i < arguments.length; i++) {
    var source = arguments[i];
    for (var key in source) {
      if (source.hasOwnProperty(key)) destination[key] = source[key];
    }
  }
  return destination
}

function repeat (character, count) {
  return Array(count + 1).join(character)
}

function trimLeadingNewlines (string) {
  return string.replace(/^\n*/, '')
}

function trimTrailingNewlines (string) {
  // avoid match-at-end regexp bottleneck, see #370
  var indexEnd = string.length;
  while (indexEnd > 0 && string[indexEnd - 1] === '\n') indexEnd--;
  return string.substring(0, indexEnd)
}

var blockElements = [
  'ADDRESS', 'ARTICLE', 'ASIDE', 'AUDIO', 'BLOCKQUOTE', 'BODY', 'CANVAS',
  'CENTER', 'DD', 'DIR', 'DIV', 'DL', 'DT', 'FIELDSET', 'FIGCAPTION', 'FIGURE',
  'FOOTER', 'FORM', 'FRAMESET', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'HEADER',
  'HGROUP', 'HR', 'HTML', 'ISINDEX', 'LI', 'MAIN', 'MENU', 'NAV', 'NOFRAMES',
  'NOSCRIPT', 'OL', 'OUTPUT', 'P', 'PRE', 'SECTION', 'TABLE', 'TBODY', 'TD',
  'TFOOT', 'TH', 'THEAD', 'TR', 'UL'
];

function isBlock (node) {
  return is(node, blockElements)
}

var voidElements = [
  'AREA', 'BASE', 'BR', 'COL', 'COMMAND', 'EMBED', 'HR', 'IMG', 'INPUT',
  'KEYGEN', 'LINK', 'META', 'PARAM', 'SOURCE', 'TRACK', 'WBR'
];

function isVoid (node) {
  return is(node, voidElements)
}

function hasVoid (node) {
  return has(node, voidElements)
}

var meaningfulWhenBlankElements = [
  'A', 'TABLE', 'THEAD', 'TBODY', 'TFOOT', 'TH', 'TD', 'IFRAME', 'SCRIPT',
  'AUDIO', 'VIDEO'
];

function isMeaningfulWhenBlank (node) {
  return is(node, meaningfulWhenBlankElements)
}

function hasMeaningfulWhenBlank (node) {
  return has(node, meaningfulWhenBlankElements)
}

function is (node, tagNames) {
  return tagNames.indexOf(node.nodeName) >= 0
}

function has (node, tagNames) {
  return (
    node.getElementsByTagName &&
    tagNames.some(function (tagName) {
      return node.getElementsByTagName(tagName).length
    })
  )
}

function Node (node, options) {
  node.isBlock = isBlock(node);
  node.isCode = node.nodeName === 'CODE' || node.nodeName === 'WC-MERMAID' || node.parentNode.isCode;
  node.isBlank = isBlank(node);
  node.flankingWhitespace = flankingWhitespace(node, options);
  // When true, this node will be rendered as pure Markdown; false indicates it
  // will be rendered using HTML. A value of true can indicate either that the
  // source HTML can be perfectly captured as Markdown, or that the source HTML
  // will be approximated as Markdown by discarding some HTML attributes
  // (options.renderAsPure === true). Note that the value computed below is an
  // initial estimate, which may be updated by a rule's `pureAttributes`
  // property.
  node.renderAsPure = options.renderAsPure || node.attributes === undefined || node.attributes.length === 0;
  // Given a dict of attributes that an HTML element may contain and still be
  // convertable to pure Markdown, update the `node.renderAsPure` attribute. The
  // keys of the dict define allowable attributes; the values define the value
  // allowed for that key. If the value is `undefined`, then any value is
  // allowed for the given key.
  node.addPureAttributes = (d) => {
    // Only perform this check if the node isn't pure and there's something to
    // check. Note that `d.length` is always `undefined` (JavaScript is fun).
    if (!node.renderAsPure && Object.keys(d).length) {
      // Check to see how many of the allowed attributes match the actual
      // attributes.
      let allowedLength = 0;
      for (const [key, value] of Object.entries(d)) {
        if (key in node.attributes && (value === undefined || node.attributes[key].value === value)) {
          ++allowedLength;
        }
      }
      // If the lengths are equal, then every attribute matched with an allowed
      // attribute: this node is representable in pure Markdown.
      if (node.attributes.length === allowedLength) {
        node.renderAsPure = true;
      }
    }
  };

  // Provide a means to escape HTML to conform to Markdown's requirements:
  // inside raw HTML, one
  // [end condition](https://spec.commonmark.org/0.31.2/#html-blocks) is a blank
  // line (two consecutive newlines). To avoid this, escape newline pairs. Note:
  // this is a bit conservative, since some tags end only with a closing tag,
  // not on a newline.
  node.cleanOuterHTML = () => node.outerHTML.replace(/\n\n/g, '\n&#10;').replace(/\r\r/g, '\r&#13;').replace(/\n\r\n\r/g, '\n\r&#10;&#13;').replace(/\r\n\r\n/g, '\r\n&#13;&#10;');
  // Output the provided string if `node.renderAsPure`; otherwise, output
  // `node.outerHTML`.
  node.ifPure = (str) => node.renderAsPure ? str : node.cleanOuterHTML();
  return node
}

function isBlank (node) {
  return (
    !isVoid(node) &&
    !isMeaningfulWhenBlank(node) &&
    /^\s*$/i.test(node.textContent) &&
    !hasVoid(node) &&
    !hasMeaningfulWhenBlank(node)
  )
}

function flankingWhitespace (node, options) {
  if (node.isBlock || (options.preformattedCode && node.isCode)) {
    return { leading: '', trailing: '' }
  }

  var edges = edgeWhitespace(node.textContent);

  // abandon leading ASCII WS if left-flanked by ASCII WS
  if (edges.leadingAscii && isFlankedByWhitespace('left', node, options)) {
    edges.leading = edges.leadingNonAscii;
  }

  // abandon trailing ASCII WS if right-flanked by ASCII WS
  if (edges.trailingAscii && isFlankedByWhitespace('right', node, options)) {
    edges.trailing = edges.trailingNonAscii;
  }

  return { leading: edges.leading, trailing: edges.trailing }
}

function edgeWhitespace (string) {
  var m = string.match(/^(([ \t\r\n]*)(\s*))(?:(?=\S)[\s\S]*\S)?((\s*?)([ \t\r\n]*))$/);
  return {
    leading: m[1], // whole string for whitespace-only strings
    leadingAscii: m[2],
    leadingNonAscii: m[3],
    trailing: m[4], // empty for whitespace-only strings
    trailingNonAscii: m[5],
    trailingAscii: m[6]
  }
}

function isFlankedByWhitespace (side, node, options) {
  var sibling;
  var regExp;
  var isFlanked;

  if (side === 'left') {
    sibling = node.previousSibling;
    regExp = / $/;
  } else {
    sibling = node.nextSibling;
    regExp = /^ /;
  }

  if (sibling) {
    if (sibling.nodeType === 3) {
      isFlanked = regExp.test(sibling.nodeValue);
    } else if (options.preformattedCode && sibling.nodeName === 'CODE') {
      isFlanked = false;
    } else if (sibling.nodeType === 1 && !isBlock(sibling)) {
      isFlanked = regExp.test(sibling.textContent);
    }
  }
  return isFlanked
}

/*!
 * word-wrap <https://github.com/jonschlinkert/word-wrap>
 *
 * Copyright (c) 2014-2023, Jon Schlinkert.
 * Released under the MIT License.
 */

function trimEnd(str) {
  let lastCharPos = str.length - 1;
  let lastChar = str[lastCharPos];
  while(lastChar === ' ' || lastChar === '\t') {
    lastChar = str[--lastCharPos];
  }
  return str.substring(0, lastCharPos + 1);
}

function trimTabAndSpaces(str) {
  const lines = str.split('\n');
  const trimmedLines = lines.map((line) => trimEnd(line));
  return trimmedLines.join('\n');
}

var wordWrap = function(str, options) {
  options = options || {};
  if (str == null) {
    return str;
  }

  var width = options.width || 50;
  var indent = (typeof options.indent === 'string')
    ? options.indent
    : '  ';

  var newline = options.newline || '\n' + indent;
  var escape = typeof options.escape === 'function'
    ? options.escape
    : identity;

  var regexString = '.{1,' + width + '}';
  if (options.cut !== true) {
    regexString += '([\\s\u200B]+|$)|[^\\s\u200B]+?([\\s\u200B]+|$)';
  }

  var re = new RegExp(regexString, 'g');
  var lines = str.match(re) || [];
  var result = indent + lines.map(function(line) {
    if (line.slice(-1) === '\n') {
      line = line.slice(0, line.length - 1);
    }
    return escape(line);
  }).join(newline);

  if (options.trim === true) {
    result = trimTabAndSpaces(result);
  }
  return result;
};

function identity(str) {
  return str;
}

// Determine the approximate left indent. It will be incorrect for list items
// whose numbers are over two digits.
const approxLeftIndent = (node) => {
  let leftIndent = 0;
  while (node) {
    if (node.nodeName === 'BLOCKQUOTE') {
      leftIndent += 2;
    } else if (node.nodeName === 'UL' || node.nodeName === 'OL') {
      leftIndent += 4;
    }
    node = node.parentNode;
  }
  return leftIndent
};

// Wrap the provided text if so requested by the options.
const wrapContent = (content, node, options) => {
  if (!options.wordWrap.length) {
    return content
  }
  const [wordWrapColumn, wordWrapMinWidth] = options.wordWrap;
  const wrapWidth = Math.max(wordWrapColumn - approxLeftIndent(node), wordWrapMinWidth);
  return wordWrap(content, {width: wrapWidth, indent: '', trim: true})
};

var rules = {};

rules.paragraph = {
  filter: 'p',

  replacement: function (content, node, options) {
    return '\n\n' + wrapContent(content, node, options) + '\n\n'
  }
};

rules.lineBreak = {
  filter: 'br',

  replacement: function (content, node, options) {
    return options.br + '\n'
  }
};

rules.heading = {
  filter: ['h1', 'h2', 'h3', 'h4', 'h5', 'h6'],

  replacement: function (content, node, options) {
    var hLevel = Number(node.nodeName.charAt(1));

    if (options.headingStyle === 'setext' && hLevel < 3) {
      // Only wrap setext headings -- atx heading don't work wrapped.
      content = wrapContent(content, node, options);
      // Split the contents into lines, then find the longest line length.
      const splitContent = content.split(/\r\n|\n|\r/);
      // From [SO](https://stackoverflow.com/a/43304999/16038919).
      const maxLineLength = Math.max(...(splitContent.map(el => el.length)));
      var underline = repeat((hLevel === 1 ? '=' : '-'), maxLineLength);
      return (
        '\n\n' + content + '\n' + underline + '\n\n'
      )
    } else {
      return '\n\n' + repeat('#', hLevel) + ' ' + content + '\n\n'
    }
  }
};

rules.blockquote = {
  filter: 'blockquote',

  replacement: function (content, node, options) {
    content = wrapContent(content, node, options);
    content = content.replace(/^\n+|\n+$/g, '');
    content = content.replace(/^/gm, '> ');
    return '\n\n' + content + '\n\n'
  }
};

rules.list = {
  filter: ['ul', 'ol'],
  pureAttributes: function (node, options) {
    // When rendering in faithful mode, check that all children are `<li>`
    // elements that can be faithfully rendered. If not, this must be rendered
    // as HTML.
    if (!options.renderAsPure) {
      var childrenPure = Array.prototype.reduce.call(node.childNodes,
        (previousValue, currentValue) =>
          previousValue &&
          currentValue.nodeName === 'LI' &&
          (new Node(currentValue, options)).renderAsPure, true
      );
      if (!childrenPure) {
        // If any of the children must be rendered as HTML, then this node must
        // also be rendered as HTML.
        node.renderAsPure = false;
        return
      }
    }
    // Allow a `start` attribute if this is an `ol`.
    return node.nodeName === 'OL' ? {start: undefined} : {}
  },

  replacement: function (content, node) {
    var parent = node.parentNode;
    if (parent.nodeName === 'LI' && parent.lastElementChild === node) {
      return '\n' + content
    } else {
      return '\n\n' + content + '\n\n'
    }
  }
};

rules.listItem = {
  filter: 'li',

  replacement: function (content, node, options) {
    const spaces = 2;
    let prefix = '';
    content = content
      .replace(/^\n+/, '') // remove leading newlines
      .replace(/\n+$/, '\n'); // replace trailing newlines with just a single one
    const parent = node.parentNode;
    if (parent.nodeName === 'OL') {
      const start = parseInt(parent.getAttribute('start')) || 0;
      const digits = Math.log(parent.children.length + start) * Math.LOG10E + 1 | 0;
      const index = Array.prototype.indexOf.call(parent.children, node);
      const itemNumber = (start ? Number(start) + index : index + 1);
      const suffix = '.';
      const padding = (digits > spaces ? digits + 1 : spaces + 1) + suffix.length; // increase padding if beyond 99
      prefix = (itemNumber + suffix).padEnd(padding);
      // Indent all non-blank lines.
      content = content.replace(/\n(.+)/gm, '\n  '.padEnd(1 + padding) + '$1');
    } else {
      prefix = options.bulletListMarker + ' '.padEnd(1 + spaces);
      // Indent all non-blank lines.
      content = content.replace(/\n(.+)/gm, '\n  '.padEnd(3 + spaces) + '$1');
    }
    return (
      prefix + content + (node.nextSibling && !content.endsWith('\n\n') ? '\n' : '')
    )
  }
};

// Determine if a code block is pure. It accepts the following structure:
//
// ```HTML
// <pre>
//   <code (optional) class="language-xxx">code contents, including newlines</code>
//   ...then 0 or more of either:
//   <br>   <-- this is translated to a newline
//   <code>more code</code>
// </pre>
// ```
let codeBlockPureAttributes = (node, options, isFenced) => {
  // Check the purity of the child block(s) which contain the code.
  node.renderAsPure = options.renderAsPure || (node.childNodes.length > 0 && Array.prototype.reduce.call(node.childNodes, (accumulator, childNode) => {
    const cn = new Node(childNode, options);
    // All previous siblings are pure and...
    return accumulator && (
      // ... it's either a `br` (which cannot have children) ...
      (cn.nodeName === 'BR' && cn.attributes.length === 0) ||
      // ... or a `code` element which has ...
      (cn.nodeName === 'CODE' &&
        // ... no attributes or (for a fenced code block) a class attribute
        // containing a language name...
        (cn.attributes.length === 0 || (isFenced && cn.attributes.length === 1 && cn.className.match(/language-(\S+)/))) &&
        // ... only one child...
        cn.childNodes.length === 1 &&
        // ... containing text, ...
        cn.firstChild.nodeType === 3
      )
    )
    // ... then this node and its subtree are pure.
  }, true));
};

rules.indentedCodeBlock = {
  filter: function (node, options) {
    return (
      options.codeBlockStyle === 'indented' &&
      node.nodeName === 'PRE' &&
      node.firstChild &&
      node.firstChild.nodeName === 'CODE'
    )
  },

  pureAttributes: (node, options) => codeBlockPureAttributes(node, options, false),

  replacement: function (content, node, options) {
    return (
      '\n\n    ' +
      node.firstChild.textContent.replace(/\n/g, '\n    ') +
      '\n\n'
    )
  }
};

rules.fencedCodeBlock = {
  filter: function (node, options) {
    return (
      options.codeBlockStyle === 'fenced' &&
      node.nodeName === 'PRE' &&
      node.firstChild &&
      node.firstChild.nodeName === 'CODE'
    )
  },

  pureAttributes: (node, options) => codeBlockPureAttributes(node, options, true),

  replacement: function (content, node, options) {
    var className = node.firstChild.getAttribute('class') || '';
    var language = (className.match(/language-(\S+)/) || [null, ''])[1];
    // In the HTML, combine the text inside `code` tags while translating `br`
    // tags to a newline.
    var code = Array.prototype.reduce.call(node.childNodes, (accumulator, childNode) => accumulator + (childNode.tagName === 'BR' ? '\n' : childNode.textContent), '');

    var fenceChar = options.fence.charAt(0);
    var fenceSize = 3;
    var fenceInCodeRegex = new RegExp('^' + fenceChar + '{3,}', 'gm');

    var match;
    while ((match = fenceInCodeRegex.exec(code))) {
      if (match[0].length >= fenceSize) {
        fenceSize = match[0].length + 1;
      }
    }

    var fence = repeat(fenceChar, fenceSize);

    return (
      '\n\n' + fence + language + '\n' +
      code.replace(/\n$/, '') +
      '\n' + fence + '\n\n'
    )
  }
};

rules.horizontalRule = {
  filter: 'hr',

  replacement: function (content, node, options) {
    return '\n\n' + options.hr + '\n\n'
  }
};

rules.inlineLink = {
  filter: function (node, options) {
    return (
      options.linkStyle === 'inlined' &&
      node.nodeName === 'A' &&
      node.getAttribute('href')
    )
  },

  pureAttributes: {href: undefined, title: undefined},

  replacement: function (content, node) {
    var href = node.getAttribute('href');
    if (href) href = href.replace(/([()])/g, '\\$1');
    var title = cleanAttribute(node.getAttribute('title'));
    if (title) title = ' "' + title.replace(/"/g, '\\"') + '"';
    return '[' + content + '](' + href + title + ')'
  }
};

rules.referenceLink = {
  filter: function (node, options) {
    return (
      options.linkStyle === 'referenced' &&
      node.nodeName === 'A' &&
      node.getAttribute('href')
    )
  },

  pureAttributes: {href: undefined, title: undefined},

  replacement: function (content, node, options) {
    var href = node.getAttribute('href');
    var title = cleanAttribute(node.getAttribute('title'));
    if (title) title = ' "' + title + '"';
    var replacement;
    var reference;

    switch (options.linkReferenceStyle) {
      case 'collapsed':
        replacement = '[' + content + '][]';
        reference = '[' + content + ']: ' + href + title;
        break
      case 'shortcut':
        replacement = '[' + content + ']';
        reference = '[' + content + ']: ' + href + title;
        break
      default:
        var id = this.references.length + 1;
        replacement = '[' + content + '][' + id + ']';
        reference = '[' + id + ']: ' + href + title;
    }

    this.references.push(reference);
    return replacement
  },

  references: [],

  append: function (options) {
    var references = '';
    if (this.references.length) {
      references = '\n\n' + this.references.join('\n') + '\n\n';
      this.references = []; // Reset references
    }
    return references
  }
};

const WHITESPACE_START = /^(\\?\n| )+/;
const WHITESPACE_END = /(\\?\n| )+$/;
rules.emphasis = {
  filter: ['em', 'i'],

  replacement: function (content, node, options) {
    if (!content.trim()) return ''
    var startWhitespace = '';
    var endWhitespace = '';
    var m = WHITESPACE_START.exec(content);
    if (m) {
      startWhitespace = m[0];
      content = content.slice(startWhitespace.length);
    }
    m = WHITESPACE_END.exec(content);
    if (m) {
      endWhitespace = m[0];
      content = content.slice(0, -endWhitespace.length);
    }
    return startWhitespace + options.emDelimiter + content + options.emDelimiter + endWhitespace
  }
};

rules.strong = {
  filter: ['strong', 'b'],

  replacement: function (content, node, options) {
    if (!content.trim()) return ''
    var startWhitespace = '';
    var endWhitespace = '';
    var m = WHITESPACE_START.exec(content);
    if (m) {
      startWhitespace = m[0];
      content = content.slice(startWhitespace.length);
    }
    m = WHITESPACE_END.exec(content);
    if (m) {
      endWhitespace = m[0];
      content = content.slice(0, -endWhitespace.length);
    }
    return startWhitespace + options.strongDelimiter + content + options.strongDelimiter + endWhitespace
  }
};

rules.code = {
  filter: function (node) {
    var hasSiblings = node.previousSibling || node.nextSibling;
    var isCodeBlock = node.parentNode.nodeName === 'PRE' && !hasSiblings;

    return node.nodeName === 'CODE' && !isCodeBlock
  },

  pureAttributes: function (node, options) {
    // An inline code block must contain only text to be rendered as Markdown.
    node.renderAsPure = options.renderAsPure || (node.renderAsPure && node.firstChild.nodeType === 3 && node.childNodes.length === 1);
  },

  replacement: function (content) {
    if (!content) return ''
    content = content.replace(/\r?\n|\r/g, ' ');

    var extraSpace = /^`|^ .*?[^ ].* $|`$/.test(content) ? ' ' : '';
    var delimiter = '`';
    var matches = content.match(/`+/gm) || [];
    while (matches.indexOf(delimiter) !== -1) delimiter = delimiter + '`';

    return delimiter + extraSpace + content + extraSpace + delimiter
  }
};

rules.image = {
  filter: 'img',
  pureAttributes: {alt: undefined, src: undefined, title: undefined},

  replacement: function (content, node) {
    var alt = cleanAttribute(node.getAttribute('alt'));
    var src = node.getAttribute('src') || '';
    var title = cleanAttribute(node.getAttribute('title'));
    var titlePart = title ? ' "' + title + '"' : '';
    return src ? '![' + alt + ']' + '(' + src + titlePart + ')' : ''
  }
};

function cleanAttribute (attribute) {
  return attribute ? attribute.replace(/(\n+\s*)+/g, '\n') : ''
}

/**
 * Manages a collection of rules used to convert HTML to Markdown
 */

function Rules (options) {
  this.options = options;
  this._keep = [];
  this._remove = [];

  this.blankRule = {
    replacement: options.blankReplacement
  };

  this.keepReplacement = options.keepReplacement;

  this.defaultRule = {
    replacement: options.defaultReplacement
  };

  this.array = [];
  for (var key in options.rules) this.array.push(options.rules[key]);
}

Rules.prototype = {
  add: function (key, rule) {
    this.array.unshift(rule);
  },

  keep: function (filter) {
    this._keep.unshift({
      filter: filter,
      replacement: this.keepReplacement
    });
  },

  remove: function (filter) {
    this._remove.unshift({
      filter: filter,
      replacement: function () {
        return ''
      }
    });
  },

  forNode: function (node) {
    if (node.isBlank) return this.blankRule
    var rule;

    if ((rule = findRule(this.array, node, this.options))) return rule
    if ((rule = findRule(this._keep, node, this.options))) return rule
    if ((rule = findRule(this._remove, node, this.options))) return rule

    return this.defaultRule
  },

  forEach: function (fn) {
    for (var i = 0; i < this.array.length; i++) fn(this.array[i], i);
  }
};

function findRule (rules, node, options) {
  for (var i = 0; i < rules.length; i++) {
    var rule = rules[i];
    if (filterValue(rule, node, options)) return rule
  }
  return void 0
}

function filterValue (rule, node, options) {
  var filter = rule.filter;
  if (typeof filter === 'string') {
    if (filter === node.nodeName.toLowerCase()) return true
  } else if (Array.isArray(filter)) {
    if (filter.indexOf(node.nodeName.toLowerCase()) > -1) return true
  } else if (typeof filter === 'function') {
    if (filter.call(rule, node, options)) return true
  } else {
    throw new TypeError('`filter` needs to be a string, array, or function')
  }
}

/**
 * The collapseWhitespace function is adapted from collapse-whitespace
 * by Luc Thevenard.
 *
 * The MIT License (MIT)
 *
 * Copyright (c) 2014 Luc Thevenard <lucthevenard@gmail.com>
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

/**
 * collapseWhitespace(options) removes extraneous whitespace from an the given element.
 *
 * @param {Object} options
 */
function collapseWhitespace (options) {
  var element = options.element;
  var isBlock = options.isBlock;
  var isVoid = options.isVoid;
  var isPre = options.isPre || function (node) {
    return node.nodeName === 'PRE' || node.nodeName === 'WC-MERMAID'
  };
  var renderAsPure = options.renderAsPure;

  if (!element.firstChild || isPre(element)) return

  var prevText = null;
  var keepLeadingWs = false;

  var prev = null;
  var node = next(prev, element, isPre);

  while (node !== element) {
    if (node.nodeType === 3 || node.nodeType === 4) { // Node.TEXT_NODE or Node.CDATA_SECTION_NODE
      var text = node.data.replace(/[ \r\n\t]+/g, ' ');

      if ((!prevText || / $/.test(prevText.data)) &&
          !keepLeadingWs && text[0] === ' ') {
        text = text.substr(1);
      }

      // `text` might be empty at this point.
      if (!text) {
        node = remove(node);
        continue
      }

      node.data = text;

      prevText = node;
    } else if (node.nodeType === 1) { // Node.ELEMENT_NODE
      if (isBlock(node) || node.nodeName === 'BR') {
        if (prevText) {
          prevText.data = prevText.data.replace(/ $/, '');
        }

        prevText = null;
        keepLeadingWs = false;
      } else if (isVoid(node) || isPre(node)) {
        // Avoid trimming space around non-block, non-BR void elements and inline PRE.
        prevText = null;
        keepLeadingWs = true;
      } else if (prevText) {
        // Drop protection if set previously.
        keepLeadingWs = false;
      }
    } else if (renderAsPure) {
      node = remove(node);
      continue
    }

    var nextNode = next(prev, node, isPre);
    prev = node;
    node = nextNode;
  }

  if (prevText) {
    prevText.data = prevText.data.replace(/ $/, '');
    if (!prevText.data) {
      remove(prevText);
    }
  }
}

/**
 * remove(node) removes the given node from the DOM and returns the
 * next node in the sequence.
 *
 * @param {Node} node
 * @return {Node} node
 */
function remove (node) {
  var next = node.nextSibling || node.parentNode;

  node.parentNode.removeChild(node);

  return next
}

/**
 * next(prev, current, isPre) returns the next node in the sequence, given the
 * current and previous nodes.
 *
 * @param {Node} prev
 * @param {Node} current
 * @param {Function} isPre
 * @return {Node}
 */
function next (prev, current, isPre) {
  if ((prev && prev.parentNode === current) || isPre(current)) {
    return current.nextSibling || current.parentNode
  }

  return current.firstChild || current.nextSibling || current.parentNode
}

/*
 * Set up window for Node.js
 */

var root = (typeof window !== 'undefined' ? window : {});

/*
 * Parsing HTML strings
 */

function canParseHTMLNatively () {
  var Parser = root.DOMParser;
  var canParse = false;

  // Adapted from https://gist.github.com/1129031
  // Firefox/Opera/IE throw errors on unsupported types
  try {
    // WebKit returns null on unsupported types
    if (new Parser().parseFromString('', 'text/html')) {
      canParse = true;
    }
  } catch (e) {}

  return canParse
}

function createHTMLParser () {
  var Parser = function () {};

  {
    if (shouldUseActiveX()) {
      Parser.prototype.parseFromString = function (string) {
        var doc = new window.ActiveXObject('htmlfile');
        doc.designMode = 'on'; // disable on-page scripts
        doc.open();
        doc.write(string);
        doc.close();
        return doc
      };
    } else {
      Parser.prototype.parseFromString = function (string) {
        var doc = document.implementation.createHTMLDocument('');
        doc.open();
        doc.write(string);
        doc.close();
        return doc
      };
    }
  }
  return Parser
}

function shouldUseActiveX () {
  var useActiveX = false;
  try {
    document.implementation.createHTMLDocument('').open();
  } catch (e) {
    if (root.ActiveXObject) useActiveX = true;
  }
  return useActiveX
}

var HTMLParser = canParseHTMLNatively() ? root.DOMParser : createHTMLParser();

function RootNode (input, options) {
  var root;
  if (typeof input === 'string') {
    var doc = htmlParser().parseFromString(
      // DOM parsers arrange elements in the <head> and <body>.
      // Wrapping in a custom element ensures elements are reliably arranged in
      // a single element.
      '<x-turndown id="turndown-root">' + input + '</x-turndown>',
      'text/html'
    );
    root = doc.getElementById('turndown-root');
  } else {
    root = input.cloneNode(true);
  }
  collapseWhitespace({
    element: root,
    isBlock: isBlock,
    isVoid: isVoid,
    isPre: options.preformattedCode ? isPreOrCode : null,
    renderAsPure: options.renderAsPure
  });

  return root
}

var _htmlParser;
function htmlParser () {
  _htmlParser = _htmlParser || new HTMLParser();
  return _htmlParser
}

function isPreOrCode (node) {
  return node.nodeName === 'PRE' || node.nodeName === 'CODE'
}

var reduce = Array.prototype.reduce;
// Taken from `commonmark.js/lib/common.js`.
var TAGNAME = '[A-Za-z][A-Za-z0-9-]*';
var ATTRIBUTENAME = '[a-zA-Z_:][a-zA-Z0-9:._-]*';
var UNQUOTEDVALUE = "[^\"'=<>`\\x00-\\x20]+";
var SINGLEQUOTEDVALUE = "'[^']*'";
var DOUBLEQUOTEDVALUE = '"[^"]*"';
var ATTRIBUTEVALUE =
    '(?:' +
    UNQUOTEDVALUE +
    '|' +
    SINGLEQUOTEDVALUE +
    '|' +
    DOUBLEQUOTEDVALUE +
    ')';
var ATTRIBUTEVALUESPEC = '(?:' + '\\s*=' + '\\s*' + ATTRIBUTEVALUE + ')';
var ATTRIBUTE = '(?:' + '\\s+' + ATTRIBUTENAME + ATTRIBUTEVALUESPEC + '?)';
var OPENTAG = '<' + TAGNAME + ATTRIBUTE + '*' + '\\s*/?>';
var CLOSETAG = '</' + TAGNAME + '\\s*[>]';
var HTMLCOMMENT = '<!-->|<!--->|<!--(?:[^-]+|-[^-]|--[^>])*-->';
var PROCESSINGINSTRUCTION = '[<][?][\\s\\S]*?[?][>]';
var DECLARATION = '<![A-Z]+' + '[^>]*>';
var CDATA = '<!\\[CDATA\\[[\\s\\S]*?\\]\\]>';
var HTMLTAG =
    '(?:' +
    OPENTAG +
    '|' +
    CLOSETAG +
    '|' +
    // Note: Turndown removes comments, so this portion of the regex isn't
    // necessary, but doesn't cause problems.
    HTMLCOMMENT +
    '|' +
    PROCESSINGINSTRUCTION +
    '|' +
    DECLARATION +
    '|' +
    CDATA +
    ')';
// End of copied commonmark code.
var escapes = [
  [/\\/g, '\\\\'],
  [/\*/g, '\\*'],
  [/^-/g, '\\-'],
  [/^\+ /g, '\\+ '],
  [/^(=+)/g, '\\$1'],
  [/^(#{1,6}) /g, '\\$1 '],
  [/`/g, '\\`'],
  [/^~~~/g, '\\~~~'],
  [/\[/g, '\\['],
  [/\]/g, '\\]'],
  [/^>/g, '\\>'],
  [/_/g, '\\_'],
  [/^(\d+)\. /g, '$1\\. '],
  // Per
  // [section 6.6 of the CommonMark spec](https://spec.commonmark.org/0.30/#raw-html),
  // Raw HTML, CommonMark recognizes and passes through HTML-like tags and their
  // contents. Therefore, Turndown needs to escape text that would parse as an
  // HTML-like tag. This regex recognizes these tags and escapes them by
  // inserting a leading backslash.
  [new RegExp(HTMLTAG, 'g'), '\\$&'],
  // Likewise,
  // [section 4.6 of the CommonMark spec](https://spec.commonmark.org/0.30/#html-blocks),
  // HTML blocks, requires the same treatment.
  //
  // This regex was copied from `commonmark.js/lib/blocks.js`, the
  // `reHtmlBlockOpen` variable. We only need regexps for patterns not matched
  // by the previous pattern, so this doesn't need all expressions there.
  //
  // TODO: this is too aggressive; it should only recognize this pattern at the
  // beginning of a line of CommonnMark source; these will recognize the pattern
  // at the beginning of any inline or block markup. The approach I tried was to
  // put this in `commonmark-rules.js` for the `paragraph` and `heading` rules
  // (the only block beginning-of-line rules). However, text outside a
  // paragraph/heading doesn't get escaped in this case.
  [/^<(?:script|pre|textarea|style)(?:\s|>|$)/i, '\\$&'],
  [/^<[/]?(?:address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[123456]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|nav|noframes|ol|optgroup|option|p|param|section|source|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?:\s|[/]?[>]|$)/i, '\\$&']
];

function TurndownService (options) {
  if (!(this instanceof TurndownService)) return new TurndownService(options)

  var defaults = {
    rules: rules,
    headingStyle: 'setext',
    hr: '* * *',
    bulletListMarker: '*',
    codeBlockStyle: 'indented',
    fence: '```',
    emDelimiter: '*',
    strongDelimiter: '**',
    linkStyle: 'inlined',
    linkReferenceStyle: 'full',
    br: '  ',
    preformattedCode: false,
    // Should the output be pure (pure Markdown, with no HTML blocks; this
    // discards any HTML input that can't be represented in "pure" Markdown) or
    // faithful (any input HTML that can't be exactly duplicated using Markdwon
    // remains HTML is the resulting output)? This is `false` by default,
    // following the original author's design.
    renderAsPure: true,
    // An array of \[word wrap column, minimum word wrap width\] indicates that
    // the output should be word wrapped based on these parameters; otherwise,
    // en empty list indicates no wrapping.
    wordWrap: [],
    blankReplacement: function (content, node) {
      return node.isBlock ? '\n\n' : ''
    },
    keepReplacement: function (content, node) {
      return node.isBlock ? '\n\n' + node.outerHTML + '\n\n' : node.outerHTML
    },
    defaultReplacement: function (content, node, options) {
      // A hack: for faithful output, always produce the HTML, rather than the
      // content. To get this, tell the node it's impure.
      node.renderAsPure = options.renderAsPure;
      return node.isBlock ? '\n\n' + node.ifPure(content) + '\n\n' : node.ifPure(content)
    }
  };
  this.options = extend({}, defaults, options);
  this.rules = new Rules(this.options);
}

TurndownService.prototype = {
  /**
   * The entry point for converting a string or DOM node to Markdown
   * @public
   * @param {String|HTMLElement} input The string or DOM node to convert
   * @returns A Markdown representation of the input
   * @type String
   */

  turndown: function (input) {
    if (!canConvert(input)) {
      throw new TypeError(
        input + ' is not a string, or an element/document/fragment node.'
      )
    }

    if (input === '') return ''

    var output = process.call(this, new RootNode(input, this.options));
    return postProcess.call(this, output)
  },

  /**
   * Like `turndown`, but functions like an iterator, so that the HTML to convert
   * is delivered in a sequnce of calls this method, then a single call to `last`.
   * @public
   * @param {String|HTMLElement} input The string or DOM node to convert
   * @returns A Markdown representation of the input
   * @type String
   */

  next: function (input) {
    if (!canConvert(input)) {
      throw new TypeError(
        input + ' is not a string, or an element/document/fragment node.'
      )
    }

    if (input === '') return ''

    var output = process.call(this, new RootNode(input, this.options));
    return cleanEmptyLines(output)
  },

  /**
   * See `next`; this finalizes the Markdown output produced by call to `next`.
   * @public
   * @param {String|HTMLElement} input The string or DOM node to convert
   * @returns A Markdown representation of the input
   * @type String
   */

  last: function (input) {
    return this.turndown(input)
  },

  /**
   * Add one or more plugins
   * @public
   * @param {Function|Array} plugin The plugin or array of plugins to add
   * @returns The Turndown instance for chaining
   * @type Object
   */

  use: function (plugin) {
    if (Array.isArray(plugin)) {
      for (var i = 0; i < plugin.length; i++) this.use(plugin[i]);
    } else if (typeof plugin === 'function') {
      plugin(this);
    } else {
      throw new TypeError('plugin must be a Function or an Array of Functions')
    }
    return this
  },

  /**
   * Adds a rule
   * @public
   * @param {String} key The unique key of the rule
   * @param {Object} rule The rule
   * @returns The Turndown instance for chaining
   * @type Object
   */

  addRule: function (key, rule) {
    this.rules.add(key, rule);
    return this
  },

  /**
   * Keep a node (as HTML) that matches the filter
   * @public
   * @param {String|Array|Function} filter The unique key of the rule
   * @returns The Turndown instance for chaining
   * @type Object
   */

  keep: function (filter) {
    this.rules.keep(filter);
    return this
  },

  /**
   * Remove a node that matches the filter
   * @public
   * @param {String|Array|Function} filter The unique key of the rule
   * @returns The Turndown instance for chaining
   * @type Object
   */

  remove: function (filter) {
    this.rules.remove(filter);
    return this
  },

  /**
   * Escapes Markdown syntax
   * @public
   * @param {String} string The string to escape
   * @returns A string with Markdown syntax escaped
   * @type String
   */

  escape: function (string) {
    return escapes.reduce(function (accumulator, escape) {
      return accumulator.replace(escape[0], escape[1])
    }, string)
  }
};

// These HTML elements are considered block nodes, as opposed to inline nodes. It's based on the Commonmark spec's selection of [HTML blocks](https://spec.commonmark.org/0.31.2/#html-blocks).
const blockNodeNames = new Set([
  'PRE', 'SCRIPT', 'STYLE', 'TEXTAREA', 'ADDRESS', 'ARTICLE', 'ASIDE', 'BASE', 'BASEFONT', 'BLOCKQUOTE', 'BODY', 'CAPTION', 'CENTER', 'COL', 'COLGROUP', 'DD', 'DETAILS', 'DIALOG', 'DIR', 'DIV', 'DL', 'DT', 'FIELDSET', 'FIGCAPTION', 'FIGURE', 'FOOTER', 'FORM', 'FRAME', 'FRAMESET', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'HEAD', 'HEADER', 'HR', 'HTML', 'IFRAME', 'LEGEND', 'LI', 'LINK', 'MAIN', 'MENU', 'MENUITEM', 'NAV', 'NOFRAMES', 'OL', 'OPTGROUP', 'OPTION', 'P', 'PARAM', 'SEARCH', 'SECTION', 'SUMMARY', 'TABLE', 'TBODY', 'TD', 'TFOOT', 'TH', 'THEAD', 'TITLE', 'TR', 'TRACK', 'UL'
]);

/**
 * Reduces a DOM node down to its Markdown string equivalent
 * @private
 * @param {HTMLElement} parentNode The node to convert
 * @returns A Markdown representation of the node
 * @type String
 */

function process (parentNode) {
  var self = this;
  const isLi = parentNode.nodeName === 'LI';
  // Note that the root node passed to Turndown isn't translated -- only its
  // children, since the root node is simply a container (a div or body tag) of
  // items to translate. Only the root node's `renderAsPure` attribute is
  // undefined; treat it as pure, since we never translate this node.
  if (parentNode.renderAsPure || parentNode.renderAsPure === undefined) {
    const output = reduce.call(parentNode.childNodes, function (output, node) {
      // `output` consists of [output so far, li accumulator]. For non-li nodes, this node's output is added to the output so far. Otherwise, accumulate content for wrapping. Wrap accumulation rules: accumulate any text and non-block node; wrap the accumulator when on a non-accumulating node.
      node = new Node(node, self.options);

      var replacement = '';
      const nodeType = node.nodeType;
      // Is this a text node?
      if (nodeType === 3) {
        replacement = node.isCode ? node.nodeValue : self.escape(node.nodeValue);
      // Is this an element node?
      } else if (nodeType === 1) {
        replacement = replacementForNode.call(self, node);
      // In faithful mode, return the contents for these special cases.
      } else if (!self.options.renderAsPure) {
        if (nodeType === 4) {
          replacement = `<!CDATA[[${node.nodeValue}]]>`;
        } else if (nodeType === 7) {
          replacement = `<?${node.nodeValue}?>`;
        } else if (nodeType === 8) {
          replacement = `<!--${node.nodeValue}-->`;
        } else if (nodeType === 10) {
          replacement = `<!${node.nodeValue}>`;
        } else {
          console.log(`Error: unexpected node type ${nodeType}.`);
        }
      }

      if (isLi) {
        // Is this a non-accumulating node?
        if (nodeType > 3 || (nodeType === 1 && blockNodeNames.has(node.nodeName))) {
          // This is a non-accumulating node. Wrap the accumulated content, then clear the accumulator.
          const wrappedAccumulator = wrapContent(output[1], node, self.options);
          return [join(join(wrappedAccumulator, output[0]), replacement), '']
        } else {
          // This is an accumulating node, so add this to the accumulator.
          return [output[0], join(output[1], replacement)]
        }
      } else {
        return [join(output[0], replacement), '']
      }
    }, ['', '']);
    return join(output[0], wrapContent(output[1], parentNode, self.options))
  } else {
    // If the `parentNode` represented itself as raw HTML, that contains all the
    // contents of the child nodes.
    return ''
  }
}

/**
 * Appends strings as each rule requires and trims the output
 * @private
 * @param {String} output The conversion output
 * @returns A trimmed version of the output
 * @type String
 */

function postProcess (output) {
  var self = this;
  this.rules.forEach(function (rule) {
    if (typeof rule.append === 'function') {
      output = join(output, rule.append(self.options));
    }
  });

  return cleanEmptyLines(output)
}

// Remove extraneous newlines/tabs at the beginning and end of lines. This is
// a postprocessing method to call just before returning the converted Markdown
// output.
const cleanEmptyLines = (output) => output.replace(/^[\t\r\n]+/, '').replace(/[\t\r\n\s]+$/, '');

/**
 * Converts an element node to its Markdown equivalent
 * @private
 * @param {HTMLElement} node The node to convert
 * @returns A Markdown representation of the node
 * @type String
 */

function replacementForNode (node) {
  var rule = this.rules.forNode(node);
  node.addPureAttributes((typeof rule.pureAttributes === 'function' ? rule.pureAttributes(node, this.options) : rule.pureAttributes) || {});
  var content = process.call(this, node);
  var whitespace = node.flankingWhitespace;
  if (whitespace.leading || whitespace.trailing) content = content.trim();
  return (
    whitespace.leading +
    // If this node contains impure content, then it must be replaced with HTML.
    // In this case, the `content` doesn't matter, so it's passed as an empty
    // string.
    (node.renderAsPure ? rule.replacement(content, node, this.options) : this.options.defaultReplacement('', node, this.options)) +
    whitespace.trailing
  )
}

/**
 * Joins replacement to the current output with appropriate number of new lines
 * @private
 * @param {String} output The current conversion output
 * @param {String} replacement The string to append to the output
 * @returns Joined output
 * @type String
 */

function join (output, replacement) {
  var s1 = trimTrailingNewlines(output);
  var s2 = trimLeadingNewlines(replacement);
  var nls = Math.max(output.length - s1.length, replacement.length - s2.length);
  var separator = '\n\n'.substring(0, nls);

  return s1 + separator + s2
}

/**
 * Determines whether an input can be converted
 * @private
 * @param {String|HTMLElement} input Describe this parameter
 * @returns Describe what it returns
 * @type String|Object|Array|Boolean|Number
 */

function canConvert (input) {
  return (
    input != null && (
      typeof input === 'string' ||
      (input.nodeType && (
        input.nodeType === 1 || input.nodeType === 9 || input.nodeType === 11
      ))
    )
  )
}

export default TurndownService;
