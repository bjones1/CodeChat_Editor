// <h1>HTML formatting tests</h1>
// <p>In this file, I will test as many markdown things as I can, to ensure that
//     the conversion between HTML and Markdown works correctly:</p>
// <p><strong>Bold Text</strong></p>
// <p><em>Italic Text</em></p>
// <p>Left-Justified Text</p>
// <p style="text-align: center;">Center-Justified Text</p>
// <p style="text-align: right;">Right-Justified Text</p>
// <p style="text-align: justify;">Fully-Justified Text. Obviously this one
//     needs to be a bit longer. Lorem Ipsum dolor sit amet. The Quick Brown Fox
//     Jumps Over The Lazy Dog. tHE qUICK bROWN fOX jUMPS oVER tHE lAZY dOG. foo
//     bar baz qux quem etc.</p>
// <h2>h2 Text</h2>
// <h3>h3 Text</h3>
// <h4>h4 Text</h4>
// <h5>h5 Text</h5>
// <h6>h6 Text</h6>
// <p><span style="text-decoration: underline;">Underlined Text</span></p>
// <p><s>Strikethrough Text</s></p>
// <ul>
//     <li>Unordered List 1</li>
//     <li>Unordered List 2</li>
//     <li>Unordered List 3</li>
// </ul>
// <ol>
//     <li>Ordered List 1</li>
//     <li>Ordered List 2</li>
//     <li>Ordered List 3</li>
// </ol>
// <ul>
//     <li><input type="checkbox">Task list 1</li>
//     <li><input checked="checked" type="checkbox">Task list 2</li>
//     <li><input type="checkbox">Task list 3</li>
// </ul>
// <p>Text in <sup>superscript</sup> and <sub>subscript</sub></p>
// <p><code>for good measure, here's a code-block</code></p>
// <blockquote>
//     <p>Blockquote Text</p>
// </blockquote>
// <div>Div Text</div>
// <p>Line<br>break</p>
// <p>Formatting Time:</p>
// <p><span style="font-family: 'times new roman', times, serif;">Different
//         Fonts</span></p>
// <p><span style="font-family: 'comic sans ms', sans-serif;">Fonts</span></p>
// <p><span style="font-family: wingdings, 'zapf dingbats';">Fonts</span></p>
// <p><span style="font-size: 8pt;">tiny font</span></p>
// <p><span style="font-size: 36pt;">LARGE FONT</span></p>
// <p style="line-height: 2;">large line height</p>
// <p style="line-height: 2;">like this all should be</p>
// <p style="line-height: 2;">double spaced</p>
// <p><span style="color: rgb(53, 152, 219);">Text in
//         nice colors</span></p>
// <p><span
//         style="background-color: rgb(132, 63, 161); color: rgb(241, 196, 15);">Text
//         in less-nice colors</span></p>
// <p>A nice little table</p>
// <table>
//     <thead>
//         <tr>
//             <th>1</th>
//             <th>2</th>
//             <th>3</th>
//             <th>4</th>
//             <th>5</th>
//         </tr>
//     </thead>
//     <tbody>
//         <tr>
//             <td>a</td>
//             <td>b</td>
//             <td>c</td>
//             <td>d</td>
//             <td>e</td>
//         </tr>
//         <tr>
//             <td>b</td>
//             <td>c</td>
//             <td>d</td>
//             <td>e</td>
//             <td>f</td>
//         </tr>
//         <tr>
//             <td>c</td>
//             <td>d</td>
//             <td>e</td>
//             <td>f</td>
//             <td>g</td>
//         </tr>
//         <tr>
//             <td>d</td>
//             <td>e</td>
//             <td>f</td>
//             <td>g</td>
//             <td>h</td>
//         </tr>
//         <tr>
//             <td>e</td>
//             <td>f</td>
//             <td>g</td>
//             <td>h</td>
//             <td>i</td>
//         </tr>
//     </tbody>
// </table>
// <p><a href="https://github.com">Here's a nice link</a></p>
// <p>&int; some unicode symbols &delta; &frac34;</p>
// <p>A line</p>
// <hr>
// <p>Embedded Video</p>
// <p><iframe src="https://www.youtube.com/embed/Hp076_dxuVU" width="560"
//         height="314" allowfullscreen="allowfullscreen"></iframe></p>
// <p>Some Emoji (more unicode)</p>
// <p>💯🅱️</p>
// <p>And some images:</p>
// <p><img src="CodeChat1.png" alt="This is an image"></p>
