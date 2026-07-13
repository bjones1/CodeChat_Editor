#!/usr/bin/env bash

# The chrometesting feature installs the Chrome binary as
# `/usr/local/bin/chrome`, but `thirtyfour`'s webdriver manager (Selenium
# Manager) only looks for binaries named `google-chrome`, `chromium-browser`,
# or `chromium` on Linux. Symlink it so the browser can be found.
if [ -x /usr/local/bin/chrome ] && [ ! -e /usr/local/bin/google-chrome ]; then
    sudo ln -s /usr/local/bin/chrome /usr/local/bin/google-chrome
fi

cd server
./bt install --dev
./bt build