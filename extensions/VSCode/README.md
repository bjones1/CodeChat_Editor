The CodeChat System
===================
The CodeChat System provides a powerful literate programming authoring system to a variety of text editors and IDEs. Specifically, it provides a GUI to automatically render source code and/or markup documents to HTML, displaying the HTML document produced by the rendering process next to the source. For example:

![The Visual Studio Code editor with the CodeChat extension.](https://raw.githubusercontent.com/bjones1/CodeChat_system/master/docs/CodeChat_screenshot_annotated.png)

In ❶, the left panel shows a the Visual Studio Code text editor with Python source code. CodeChat renders this source code to ❷, the right panel, which shows the resulting HTML document. Finally, ❸ displays output from the build process. A splitter between ❷ and ❸ allows the user to adjust the build output size or hide it entirely. Below ❸, a status bar displays the build status and a count of errors and warnings produced by the build.

In addition to native support for Markdown and reStructuredText, the CodeChat System supports almost any external renderer via user-provided JSON configuration files. For example, CodeChat can:

-   invoke [Pandoc](https://pandoc.org/) to render a wide variety of markup formats;
-   use [Sphinx](https://www.sphinx-doc.org) to build project documentation;
-   call [Runestone](https://runestone.academy/) to create interactive textbooks;
-   employ [Doxygen](https://www.doxygen.nl/) to generate documentation from source code;

... and many more.

See the [getting started guide](https://codechat-system.readthedocs.io/en/latest/extensions/VSCode/contents.html) to install and use the CodeChat System.
