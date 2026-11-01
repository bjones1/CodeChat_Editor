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

# `bt.ps1` -- Shortcut to run the CodeChat Editor build tool
# ==========================================================
#
# See
# [passing all command-line arguments](https://www.scivision.dev/powershell-pass-all-command-args/).
# <fragment id="cc-MhYeYCCTxn"></fragment>
$manifest_path = Join-Path -Path $PSScriptRoot -ChildPath builder/Cargo.toml
echo $manifest_path
cargo run --manifest-path=$manifest_path -- $args
# A script's success, as seen by its caller's `&&`, doesn't reflect the exit
# code of its last native command; pass it on explicitly.
exit $LASTEXITCODE

# CodeChat Editor lexer: python
