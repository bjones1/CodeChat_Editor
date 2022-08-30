# <h1>A server that server CodeChat Editor files, in addition to regular
#     files.</h1>
# <h2>Import</h2>
import html
from http import HTTPStatus
import http.server
import io
import os
from pathlib import Path
import socketserver
import sys
from textwrap import dedent
import urllib.parse
import webbrowser

# <h2>Globals</h2>
LP_SUFFIXES = [".cc", ".cpp", ".html", ".js", ".py", ".v"]
PORT = 8000


# <h2>Monkeypatching</h2>
# <p>Monkeypatch the open function. It's only used to open a file to
#     send back to the client. Ugly hack!</p>
# <p>Fundamentally, this is a dumb, fragile approach. I should use
#     Bottle/Flask/FastAPI/whatever and do this right. I thought this
#     approach would be simpler...but I was wrong. Probably
#     https://flask-autoindex.readthedocs.io/en/latest/. Add a save
#     function by allowing a HTTP PUT, where the URL specifies the file
#     name and contents are the file contents. This is a complete
#     CodeChat Editor environment!</p>
def codechat_open(path, *args, **kwargs):
    # <p>See if the path meets LP criteria.</p>
    p = Path(path)
    if is_codechat(p):
        # <p>Construct the path to the source file and open it.</p>
        lp_path = p.with_name(p.stem)
        print(lp_path)
        try:
            f = open(lp_path, encoding="utf-8")
        except OSError:
            # <p>On failure, open the original path, so the resulting error message
            #     makes more sense.</p>
            return open(path, *args, **kwargs)

        # <p>Read this LP source, then return its contents as a file-like
        #     object.</p>
        source_code = f.read()
        # <p>We need the path from the current URL to here (where the JS we need
        #     lives). TODO - a kludge. Put it in some <code>/static</code> path,
        #     on the web, etc.</p>
        path_to_js = (Path("../" * len(p.parent.relative_to(Path(".").resolve()).parents)) / Path(__file__).parent.relative_to(Path(".").resolve()) / "CodeChatEditor.js").as_posix()
        s = p.suffixes
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
                    <script>
                        on_dom_content_loaded(() => open_lp(
                    {"</scr'+'ipt>".join(repr(source_code).split("</script>"))}
                        , {repr(s[-2][1:])}));
                    </script>

                    <link rel="stylesheet" href="css/CodeChatEditor.css">
                </head>
                <body>
                    <p>
                        <button onclick="on_save_as(on_save_codechat);" id="CodeChat-save-as-button">
                            Save as
                        </button>
                        <button disabled onclick="on_save_codechat();" id="CodeChat-save-button">
                            Save
                        </button>
                    </p>
                    <div id="CodeChat-body">
                    </div>
                </body>
            </html>
            """)
        html_encoded = html.encode("utf-8")

        # <p>Ugly hack: return a file-like object <code>b</code> such that
        #     <code>os.fstat(b.fileno())</code> produces valid information. In
        #     particular, modify the length of the file to match the length of
        #     the output just generated. To do this:</p>
        # <ol>
        #     <li>Create a <code>BytesIO</code> object, since the returned
        #         file-like object was opened in binary mode.</li>
        # </ol>
        b = io.BytesIO(html_encoded)
        # <ol>
        #     <li>Recreate the <code>stat_result</code> struct with a new size
        #         (which is item 6 of the struct).</li>
        # </ol>
        fstats = os.fstat(f.fileno())
        fixed_fstats = os.stat_result((*fstats[:6], len(html_encoded), *fstats[7:]))
        # <ol>
        #     <li>Call this "<code>fileno</code>". A monkeypatched
        #         <code>os.fstat</code> (see below) will recognize this and
        #         return it.</li>
        # </ol>
        b.fileno = lambda: fixed_fstats
        return b

    # <p>Revert to original open behavior if it's not an LP source file.</p>
    else:
        return open(path, *args, **kwargs)


# <p>Cause <code>os.fstat</code> to return the correct length for an LP
#     file. See above.</p>
def codechat_fstat(fileno):
    if isinstance(fileno, os.stat_result):
        return fileno
    # <p>Call the original <code>os.fstat</code>, since it's already
    #     monkeypatched in this context.</p>
    return os._fstat(fileno)


# <p>Return <code>True</code> is the provided <code>path</code> could be
#     interpreted using the CodeChat Editor.</p>
def is_codechat(path):
    p = Path(path)
    s = p.suffixes
    return not p.exists() and len(s) >= 2 and s[-1] == ".html" and s[-2] in LP_SUFFIXES


# <p>Do the monkeypatches.</p>
http.server.open = codechat_open
http.server.os._fstat = http.server.os.fstat
http.server.os.fstat = codechat_fstat


# <h2>Customize the response to provide links to CodeChat-capable
#     sources</h2>
class CodeChatSimpleHTTPRequestHandler(http.server.SimpleHTTPRequestHandler):
    # <p>This is copied directly from the Python stdlib, with slight
    #     modifications.</p>
    def list_directory(self, path):
        """Helper to produce a directory listing (absent index.html).

        Return value is either a file object, or None (indicating an
        error).  In either case, the headers are sent, making the
        interface the same as for send_head().

        """
        try:
            list = os.listdir(path)
        except OSError:
            self.send_error(
                HTTPStatus.NOT_FOUND,
                "No permission to list directory")
            return None
        list.sort(key=lambda a: a.lower())
        r = []
        try:
            displaypath = urllib.parse.unquote(self.path,
                                               errors='surrogatepass')
        except UnicodeDecodeError:
            displaypath = urllib.parse.unquote(path)
        displaypath = html.escape(displaypath, quote=False)
        enc = sys.getfilesystemencoding()
        title = 'Directory listing for %s' % displaypath
        r.append('<!DOCTYPE HTML PUBLIC "-//W3C//DTD HTML 4.01//EN" '
                 '"http://www.w3.org/TR/html4/strict.dtd">')
        r.append('<html>\n<head>')
        r.append('<meta http-equiv="Content-Type" '
                 'content="text/html; charset=%s">' % enc)
        r.append('<title>%s</title>\n</head>' % title)
        r.append('<body>\n<h1>%s</h1>' % title)
        r.append('<hr>\n<ul>')
        for name in list:
            fullname = os.path.join(path, name)
            displayname = linkname = name
            # <p>Append / for directories or @ for symbolic links</p>
            if os.path.isdir(fullname):
                displayname = name + "/"
                linkname = name + "/"
            if os.path.islink(fullname):
                displayname = name + "@"
                # <p>Note: a link to a directory displays with @ and links with /</p>
            # <p>MODIFIED FROM HERE...</p>
            p = Path(fullname)
            if not Path(fullname + ".html").exists() and p.suffix in LP_SUFFIXES:
                href = urllib.parse.quote(linkname, errors="surrogatepass")
                r.append(f'<li><a href="{href}.html" target="_blank">{html.escape(displayname, quote=False)}</a> <a href="{href}">(raw)</a></li>')
            else:
                # <p>...TO HERE.</p>
                r.append('<li><a href="%s">%s</a></li>'
                        % (urllib.parse.quote(linkname,
                                            errors='surrogatepass'),
                        html.escape(displayname, quote=False)))
        r.append('</ul>\n<hr>\n</body>\n</html>\n')
        encoded = '\n'.join(r).encode(enc, 'surrogateescape')
        f = io.BytesIO()
        f.write(encoded)
        f.seek(0)
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-type", "text/html; charset=%s" % enc)
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        return f


# <h2>Run the server</h2>
def main():
    os.chdir("/")
    with socketserver.TCPServer(("", PORT), CodeChatSimpleHTTPRequestHandler) as httpd:
        webbrowser.open_new_tab(f"http://127.0.0.1:{PORT}/")
        httpd.serve_forever()


if __name__ == "__main__":
    sys.exit(main())
