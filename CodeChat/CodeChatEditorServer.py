# <h1>A server that server CodeChat Editor files, in addition to regular files.</h1>
# <h2>Import</h2>
import http.server
from io import BytesIO
import os
from pathlib import Path
import socketserver
from textwrap import dedent

# <h2>Globals</h2>
LP_SUFFIXES = [".cc", ".cpp", ".html", ".js", ".py", ".v"]
PORT = 8000
Handler = http.server.SimpleHTTPRequestHandler


# <h2>Monkeypatching</h2>
# <p>Monkeypatch the open function. It's only used to open a file to send back to the client. Ugly hack!</p>
# <p>Fundamentally, this is a dumb, fragile approach. I should use Bottle/Flask/FastAPI/whatever and do this right. I thought this approach would be simpler...but I was wrong. Probably https://flask-autoindex.readthedocs.io/en/latest/. Add a save function by allowing a HTTP PUT, where the URL specifies the file name and contents are the file contents. This is a complete CodeChat Editor environment!</p>
def codechat_open(path, *args, **kwargs):
    # See if the path meets LP criteria.
    p = Path(path)
    s = p.suffixes
    if not p.exists() and len(s) >= 2 and s[-1] == ".html" and s[-2] in LP_SUFFIXES:
        # Construct the path to the source file and open it.
        lp_path = p.with_name(p.stem)
        print(lp_path)
        try:
            f = open(lp_path, encoding="utf-8")
        except OSError:
            # On failure, open the original path, so the resulting error message makes more sense.
            return open(path, *args, **kwargs)

        # Read this LP source, then return its contents as a file-like object.
        source_code = f.read()
        # We need the path from the current URL to here (where the JS we need lives). TODO - a kludge. Put it in some <code>/static</code> path, on the web, etc.
        path_to_js = (Path("../" * len(p.parent.relative_to(Path(".").resolve()).parents)) / Path(__file__).parent.relative_to(Path(".").resolve()) / "CodeChatEditor.js").as_posix()
        html = dedent(f"""\
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <meta name="viewport" content="width=device-width, initial-scale=1">
                    <title>The CodeChat Editor</title>

                    <script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.9.5/ace.min.js"></script>
                    <script src="https://cdn.tiny.cloud/1/rrqw1m3511pf4ag8c5zao97ad7ymvnhqu6z0995b1v63rqb5/tinymce/6/tinymce.min.js" referrerpolicy="origin"></script>
                    <script src="https://cdnjs.cloudflare.com/ajax/libs/js-beautify/1.14.5/beautify-html.min.js"></script>
                    <script src="{path_to_js}"></script>

                    <link rel="stylesheet" href="css/CodeChatEditor.css">
                </head>
                <body>
                    <p>
                        <button disabled onclick="on_save_as();" id="CodeChat-save-as-button">
                            Save as
                        </button>
                        <button disabled onclick="on_save();" id="CodeChat-save-button">
                            Save
                        </button>
                    </p>
                    <div id="CodeChat-body">
                    </div>
                    <script>
                        open_lp(
                    {"</scr'+'ipt>".join(repr(source_code).split("</script>"))}
                        , {repr(s[-2][1:])});
                    </script>
                </body>
            </html>
            """)
        html_encoded = html.encode("utf-8")

        # <p>Ugly hack: return a file-like object <code>b</code> such that <code>os.fstat(b.fileno())</code> produces valid information. In particular, modify the length of the file to match the length of the output just generated. To do this:</p>
        # <ol>
        #   <li>Create a <code>BytesIO</code> object, since the returned file-like object was opened in binary mode.
        # </ol>
        b = BytesIO(html_encoded)
        # <ol>
        #   <li>Recreate the <code>stat_result</code> struct with a new size (which is item 6 of the struct).</li>
        # </ol>
        fstats = os.fstat(f.fileno())
        fixed_fstats = os.stat_result((*fstats[:6], len(html_encoded), *fstats[7:]))
        # <ol>
        #   <li>Call this "<code>fileno</code>". A monkeypatched <code>os.fstat</code> (see below) will recognize this and return it.
        # </ol>
        b.fileno = lambda: fixed_fstats
        return b

    # Revert to original open behavior if it's not an LP source file.
    else:
        return open(path, *args, **kwargs)


# Cause <code>os.fstat</code> to return the correct length for an LP file. See above.
def codechat_fstat(fileno):
    if isinstance(fileno, os.stat_result):
        return fileno
    # Call the original <code>os.fstat</code>, since it's already monkeypatched in this context.
    return os._fstat(fileno)


# Do the monkeypatches.
http.server.open = codechat_open
http.server.os._fstat = http.server.os.fstat
http.server.os.fstat = codechat_fstat

# <h2>Run the server</h2>
with socketserver.TCPServer(("", PORT), Handler) as httpd:
    print("serving at port", PORT)
    httpd.serve_forever()
