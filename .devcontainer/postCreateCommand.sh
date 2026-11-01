#!/usr/bin/env bash
# The CodeChat Editor, a word processor for software developers.<br>Copyright
# (C) 2024-2026 Bryan A. Jones.
#
# This file is part of the CodeChat Editor.
#
# The CodeChat Editor is free software: you can redistribute it and/or modify it
# under the terms of the GNU General Public License as published by the Free
# Software Foundation, either version 3 of the License, or (at your option) any
# later version.
#
# The CodeChat Editor is distributed in the hope that it will be useful, but
# WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
# FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
# details.
#
# You should have received a copy of the GNU General Public License along with
# the CodeChat Editor. If not, see
# [http://www.gnu.org/licenses](http://www.gnu.org/licenses).
#
# # `postCreateCommand.sh` -- runs after the container is created.

# The chrometesting feature installs the Chrome binary as
# `/usr/local/bin/chrome`, but `thirtyfour`'s webdriver manager (Selenium
# Manager) only looks for binaries named `google-chrome`, `chromium-browser`,
# or `chromium` on Linux. Symlink it so the browser can be found.
if [ -x /usr/local/bin/chrome ] && [ ! -e /usr/local/bin/google-chrome ]; then
    sudo ln -s /usr/local/bin/chrome /usr/local/bin/google-chrome
fi

# <fragment id="cc-ZTqrakWBqI"></fragment>
./bt install
./bt build