.. Copyright (C) 2012-2020 Bryan A. Jones.

    This file is part of the CodeChat System.

    The CodeChat System is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

    The CodeChat System is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.

    You should have received a `copy of the GNU General Public License </docs/LICENSE>` along with the CodeChat System.  If not, see http://www.gnu.org/licenses/.

*****************************************
The Visual Studio Code CodeChat extension
*****************************************
This extension provides CodeChat's capabilities within the Visual Studio Code editor, as illustrated in `the CodeChat System for Visual Studio Code <README>` page.


Installation
============
First, install `Visual Studio Code <https://code.visualstudio.com/>`_. Next:

#.  `Install the CodeChat Server <../../CodeChat_Server/contents>`, which performs all the back-end work and is required for the extension to work.
#.  Run ``CodeChat_Server vscode_install`` to install and configure the Visual Studio Code CodeChat extension.
#.  (Recommended) `switch to a light theme <https://code.visualstudio.com/docs/getstarted/themes>`_, since the CodeChat System only provides a light theme.


.. _use CodeChat:

Use
===
#.  Open a file that CodeChat can render (`most source files <https://codechat.readthedocs.io/en/master/CodeChat/CommentDelimiterInfo.py.html#supported-languages>`_, along with ``.rst``, ``.md``, and ``.html`` files).
#.  Open the `Visual Studio Code command palette <https://code.visualstudio.com/docs/getstarted/userinterface#_command-palette>`_ by pressing ``Ctrl+Shift+P``. Type ``CodeChat``, then press enter to run the extension. After a moment, the rendered file should load. If it doesn't:

    #.  Determine the location of the ``CodeChat_Server`` by entering ``which CodeChat_Server`` (Linux/OS X) or ``where CodeChat_Server`` (Windows) at the terminal/command line.
    #.  Open the Visual Studio Code settings for CodeChat by navigating to ``File`` > ``Preferences`` > ``Settings`` then typing ``CodeChat`` in the search box. Enter this path for the ``Code Chat.Code Chat Server: Command``. **Important**: in Windows, replace ``\`` in the location you determined with either ``\\`` or ``/``.
    #.  Run the extension again (``Ctrl+Shift+P`` then select CodeChat).

At any time, run the CodeChat extension again (``Ctrl+Shift+P``, then ``CodeChat``) to show the CodeChat panel, re-start the CodeChat Server if it's not running, then reconnect with the server. Close the CodeChat panel then run the extension for a more complete reset.

See the `CodeChat tutorial <https://codechat.readthedocs.io/en/master/docs/tutorial.html>`_ for step-by-step instructions on authoring literate programming documents using Sphinx. For other documentation systems, create a `project configuration file <../../codechat_config.yaml>` then place it in the root directory of your project.


Usage tips
==========
Documentation using CodeChat often involves long lines. To wrap lines, `enable word wrap <https://docs.microsoft.com/en-us/visualstudio/ide/reference/how-to-manage-word-wrap-in-the-editor?view=vs-2022>`_.



Remote Development
==================
The `VS Code Remote Development <https://code.visualstudio.com/docs/remote/remote-overview>`_ toolset allows the CodeChat System to run on another computer. To set this up:

#.  Create an `OpenSSH configuration file <https://www.ssh.com/academy/ssh/config>`_ which forwards the HTTP and websocket ports from the client (where VSCode runs) to the server (where the CodeChat Server and the VSCode extension run). To do this, in VSCode press ctrl+shift+p, then type "Remote-SSH: Open SSH Configuration File..." The contents should include:

    .. code:: text

        # Replace ``Development_Ubuntu`` with a a user-friendly name for your
        # host here.
        Host Development_Ubuntu
            # Replace this IP with the IP or address of the server to connect
            # to.
            HostName 1.2.3.4
            # Provide the username used to log in to the server.
            User bob
            # Don't change this.
            LocalForward 27377 127.0.0.1:27377
            LocalForward 27378 127.0.0.1:27378

#.  Install the CodeChat Server on the server.

#.  (Optional, but highly recommended -- it saves a lot of time) Set up `SSH key-based authentication <https://code.visualstudio.com/docs/remote/troubleshooting#_configuring-key-based-authentication>`_.

#.  `Connect to the remote host <https://code.visualstudio.com/docs/remote/ssh#_connect-to-a-remote-host>`_.

Remote containers
-----------------
This is preliminary. It's slow to attach this way. I'd prefer to SSH directly to the running container -- perhaps this `SO <https://stackoverflow.com/questions/57040499/automate-starting-ssh-service-after-running-the-container>`__ post? Even better -- find a way to run ``docker-tools shell`` before VSCode runs its stuff. Things that don't work:

    -   Adding the following to my SSH config::

            # Taken from `SO <https://unix.stackexchange.com/a/417373>`__. VSCode ignores this, unfortunately.
            RemoteCommand /home/ubuntu/.local/bin/docker-tools shell
            # To avoid the error ``the input device is not a TTY``.
            RequestTTY force

        It works as expected from a command prompt, but VSCode seems to ignore it.

#.  Install the `Docker for Visual Studio Code extension <https://marketplace.visualstudio.com/items?itemName=ms-azuretools.vscode-docker>`_.
#.  In the setting for that plugin, set ``docker.host`` to ``username@address`` of the server running Docker.
#.  Set up `SSH key-based authentication`_ and make sure the `SSH agent is running locally <https://code.visualstudio.com/docs/remote/troubleshooting#_setting-up-the-ssh-agent>`_.

Helpful links:

-   `Connect to remote Docker over SSH <https://code.visualstudio.com/docs/containers/ssh>`_.
-   `Develop on a remote Docker host <https://code.visualstudio.com/remote/advancedcontainers/develop-remote-host>`_.
-   `Remote Development Tips and Tricks <https://code.visualstudio.com/docs/remote/troubleshooting>`_.


Developer docs
==============
See also the `developer docs <developer>`.

.. toctree::
    :hidden:

    developer