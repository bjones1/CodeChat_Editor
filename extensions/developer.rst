***********************
Developer documentation
***********************

Release procedures
==================
Each extension/plugin has a unique release procedure. See each extension/plugin's developer docs.

-   `Visual studio code <VSCode/developer>`


Resources
=========
Extensions need the following support from an editor/IDE:

-   An activation point -- a menu item/command to enable and disable CodeChat in the editor/IDE.
-   Access to settings/persistent user-editable data specifying the location of the CodeChat Server executable.
-   (Optional, but strongly preferred) The ability to open a web browser inside the editor/IDE.
-   A method to display errors and status messages.
-   A method for emitting logging data (for debug, typically).
-   Notifications when the active editor changes to another file, or when an edit is made to the text of a file.
-   Notification when a file is saved.
-   (Future) Notification when the cursor moves or scroll bars change; if possible, the x, y location of the cursor and location of the vertical scroll bar.
-   (Future) The ability to scroll the editor to a given x, y coordinate and to place the cursor at a given character.

Extensions need the following support from the host programming language and its libraries:

-   The ability to run a subprocess until it completes. The ability to read the return code from the process and to capture its stdout/stderr.
-   A timer, which will ask for a render after the specified timeout.

A typical plugin/extension has these modules:

-   A ``thrift_connection``: Thrift network connection to the server, along with a ``thrift_client`` created from that connection. There should be just one instance of these classes for each process.
-   A set of functions/methods to invoke `CodeChat editor/IDE services <editor_services>` along with a ``codechat_client_id`` used to communicate with the CodeChat Client. Each window needs its own client ID.


Pseudocode
==========
The typical operation of a plugin/extension is:

Initialization
--------------
#.  Register an activation point which invokes the start-up sequence below.

Enable sequence
---------------
This sequence may occur after CodeChat has already been enabled.

#.  Is the Thrift network connection open? If not, the server may not be running. Therefore:

    #.  Use IDE/editor's settings to determine the path to the CodeChat Server executable. If this path is not empty, then:

        #.  Use the IDE's status message facility to tell the user that the CodeChat System is starting; this is important since starting the server usually takes a few seconds.
        #.  Run the CodeChat Server with the ``start`` subcommand and wait for it to finish. If the return value was 0, the server is running. Otherwise, a non-zero return value indicates an error; stop here, reporting stdout and stderr to the user via the editor/IDE's error message facility.

    #.  Open a network connection to the server. If the connection fails, stop here and report the error; if the CodeChat Server executable path was empty, include this in the error message.

#.  If the Thrift client isn't running, open it. If this fails, stop here and report the error.
#.  Invoke ``get_client``; send the returned HTML/URL to an in-editor web browser if so instructed. Save the returned client ID.
#.  Register for all relevant notifications (text edited, editor widow switched, etc.). Allocate a timer which will fire a timeout after the last notifications completes.

Main loop
---------
At this point, the CodeChat System is up and running. Now, the system should:

-   Watch for IDE events, then send render requests to the server.
-   Respond to and report connection errors.
-   Respond to closing of the extension or the CodeChat Client web browser window.

Disable sequence
----------------
#.  If the Thrift client is running, call ``stop_client()``. Close the client.
#.  If the Thrift network connection is open, close it.
#.  Close the in-editor/IDE web browser (if applicable).
#.  Disable the idle timer and unregister for all notifications.


Logging
=======
To help track down bugs, each side of a network connection needs to provide logging:

-   The server logs all requests from the IDE, web server activity, and CodeChat Client activity.
-   The CodeChat Client emits ``console.log`` info.
-   Each extension should provide IDE-specific logging capabilities.


.. _Settings system:

Settings
========
Levels for settings:

-   The system (admin user) level, where a given user may have only read access.
-   The per-user level.
-   The per-project level. This most naturally belongs in a CodeChat configuration file.
-   The per-file level.

Where to store:

-   For the system, store in CodeChat's installation location?
-   For each user, store in the user's home directory.
-   For projects, store in the CodeChat project configuration file.
-   For files, store in the user's home directory, in a dict of settings for that file.
-   Store the path to the CodeChat Server executable in the editor/IDE's settings facility.

Data stored, and where each setting is needed:

-   \(S) The renderer to use for a given file.
-   \(C) The theme.
-   \(S) The number of simultaneous renders to run.
-   \(B) The location of the CodeChat Server; an empty string means don't start it.
-   (P/S? Currently, the client makes this decision, but the server could easily make it instead.) Where to render (in IDE/editor or in an external browser).
-   (P, S - for efficiency, the client should only send render requests that are honored.) Whether to render on each change or only when the file is saved.
-   (all) Log levels (debug, info, etc.)
-   (C, P) The time to wait before invoking a render.
-   (all) Whether to sync or not.
-   \(S) Whether to shut down the server when all clients are stopped, or keep it running.

Legend:

:S: Settings used on the server.
:C: Settings used by the CodeChat Client.
:P: Settings used by a CodeChat plugin/extension.
:B: Settings used by a CodeChat plugin/extension which are used before the CodeChat Server is started.
