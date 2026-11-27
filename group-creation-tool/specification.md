Project group creation tool
===========================

This document specifies what the tool does, as seen by its users. It does not
prescribe how the code is organized.

Purpose
-------

The tool is a web app that splits a class into project groups. An
**organizer** lists the students and the projects and posts a link. Each
**student** follows the link and says which projects and which classmates they
would like, or would rather avoid. The organizer then asks the tool to form the
groups that best satisfy these preferences and downloads the result as a
`.csv` file, which is the input to the next project.

Everyone signs in with the Microsoft 365 work or school account they use for
Microsoft Teams. A person's **userid** is the sign-in name of that account,
such as `student@university.edu`. Userids are compared without regard to case.
Lengths in this document are counted in Unicode code points.

Sessions
--------

A **session** is one run of the tool: one organizer, one student list, one
project list, the students' preferences, and the formed groups. A session is
either **Open**, when students can change their preferences, or **Closed**,
when they cannot. A new session is Open.

The organizer can close and reopen a session at any time. Reopening discards
formed groups, after the organizer confirms.

A session is deleted automatically 365 days after its last change, or earlier
when its organizer deletes it; deleting asks for confirmation and cannot be
undone. Deleting removes everything the session holds; the tool remembers only
that its link no longer works.

The organizer is the only person who can see or change a session's organizer
pages. Anyone who signs in can create sessions and becomes the organizer of
the sessions they create. An organizer may also be on the student list of
their own session.

Organizer pages
---------------

### Sessions list

After signing in at the tool's address, the organizer sees their sessions,
newest first, each with its title, its state, the number of students who have
saved preferences out of the total, whether groups are formed, and the date it
will be deleted. A **New session** button starts the setup below.

### Session setup

The organizer enters:

* A **title**, 1–100 characters, shown to the students.
* The **student list**: userids, separated by commas, spaces, or new lines.
* The **project list**: one project name per line, 1–200 characters each.

In both lists, leading and trailing spaces are trimmed and empty entries are
ignored. These entries are dropped:

* an entry that repeats an earlier one, ignoring case;
* a project name longer than 200 characters;
* a student entry that is not a valid userid: one `@`, with text before it and
  a domain after it that contains at least one `.`, such as `a@b.edu`.

Every dropped entry is listed under its field with the reason, so the
organizer can correct it. The lists keep the order in which they were entered.

A session needs a title, at least 1 student, and at least 1 project, and holds
at most 500 students and 100 projects. **Create** is unavailable until these
hold.

Creating the session shows the session page.

### Session page

The session page shows:

1. The title, the state, and **Close** or **Reopen**.
2. The **student link** with a **Copy** button. If the browser does not allow
   copying, the link is shown selected, with a message asking the organizer to
   copy it manually. The link stays the same for the life of the session and
   cannot be guessed from the title or from other sessions' links.
3. The **student list**, showing for each student their userid, their display
   name once they have signed in, and one of: *not signed in*, *signed in*
   (they have opened this session's link and signed in), or *saved
   preferences*. A count above the list gives the number in each status.
4. The **project list**.
5. The **grouping options** and **Form groups** (see Forming groups).
6. The **results**, once groups are formed (see Results).
7. **Delete session**.

The organizer can edit both lists as text, using the same rules as at setup.
A student or project that is removed disappears from every student's
preferences; one that is added appears as unspecified for everyone. Changing
only the case of an entry keeps it as the same entry. Any other change to an
entry's text counts as removing it and adding a new one, so an entry that is
removed and added again starts over, without its earlier preferences and, for
a student, as *not signed in*.

To correct a userid or a project name without losing preferences, the
organizer uses the entry's **Rename** action instead. A renamed entry keeps
every preference by it and about it, and a renamed student who had signed in
stays recognized, as described under Signing in.

Editing or renaming entries discards formed groups, after the organizer
confirms.

Student page
------------

### Signing in

If the student link does not belong to an existing session, for example
because the session was deleted, the page says so without asking the student
to sign in. Otherwise, the page asks the student to sign in, unless they are
already signed in. Then:

* A student is **on the list** when their sign-in name or their account's
  email address matches a userid on the student list. They see the
  preferences page.
* The first time a student opens the session, it links that userid to their
  account. From then on, the session recognizes them by their account even if
  its sign-in name or email address changes, and no other account can use that
  userid.
* Anyone else sees a message naming their sign-in name and the organizer,
  saying they are not on this session's list and should contact the
  organizer, with a **Sign out** button so they can switch accounts.

### Preferences page

The page shows the session title, the organizer's name, the session's state,
and the student's own name with **Sign out**. It has two sections, **Projects**
and **Teammates**, which work the same way:

* Each section has three categories, shown in this order: **Preferred**,
  **Unspecified**, and **Not preferred**.
* **Projects** lists every project. **Teammates** lists every other student
  on the list, by display name when known and always by userid; it does not
  list the student themselves.
* At first, every item is in Unspecified. Unspecified items are always in the
  order of the organizer's list: an item moved back to Unspecified takes its
  place in that order, and the student cannot reorder them.
* Within Preferred and Not preferred, the student ranks the items, **strongest
  feeling first**: the first Preferred item is the one they most want, and the
  first Not preferred item is the one they most want to avoid. Each category
  says this in its heading. An item moved into one of these categories goes to
  its end.
* The student can move any item to another category and, within Preferred and
  Not preferred, up or down one place or to the top or the bottom. Every move
  can be made by dragging the item or with buttons and keys on it.
* Each section has a filter field, which shows only the items whose name or
  userid contains its text. Items can be moved while the section is filtered.
* A student may leave any category empty, including putting everything in
  Unspecified.

On narrow screens (under 800 px wide) the categories stack, Preferred first.

### Saving

Each change is saved automatically within 2 seconds; no button is needed. A
save indicator shows *Saving…*, *Saved*, or *Not saved*. While a save is
failing, the page retries every 10 seconds, and *Not saved* has a **Retry**
button that retries at once. Leaving the page with unsaved changes asks the
student to confirm, in browsers that allow this; unsaved changes are lost when
the page is closed or reloaded.

Each save replaces all of the student's saved preferences, so if the student
has the page open in two tabs, the last save wins. An open page shows
preferences saved from another tab within a minute, unless it has unsaved
changes of its own.

A student has *saved preferences* once any change of theirs has been saved.

While the session is Closed, the page shows the student's saved preferences
read-only, with a notice that preferences are closed. A change that reaches
the tool after the session closed is not saved, and the page says so.

### Privacy between students

A student sees only their own preferences. No page or file shows a student's
teammate preferences, or any figure computed from them, to anyone but that
student. The organizer sees how each student's project preference was met
(see Results).

The groups themselves follow from the teammate preferences, so these cannot be
hidden completely: an organizer who compares groups formed with different
weights, or who knows a very small class, may infer some of them.

Forming groups
--------------

### Group sizes

The organizer sets the **group size**, *k*. With *S* students:

* *k* ranges from 1 to *S*. Until the organizer changes it, it is √*S*
  rounded to the nearest whole number, with halves rounded up, but at least 2
  when there are 2 or more students, and it follows the student list as that
  changes. Once the organizer changes it, it keeps its value, clamped to the
  range when the student list changes.
* The tool forms *G* = ⌊*S*/*k*⌋ groups, but at least 1. Every group has
  either ⌊*S*/*G*⌋ or ⌈*S*/*G*⌉ students, so the students left over after
  filling groups of *k* are spread over the groups. Every student is in
  exactly one group.

For example, 50 students give a group size of 7 and 7 groups: six of 7
students and one of 8. 500 students give 22 groups of 22 or 23.

Projects are spread evenly over the groups. With *P* projects, each project
gets ⌊*G*/*P*⌋ or ⌈*G*/*P*⌉ groups. So several groups can work on the same
project, and when there are fewer groups than projects, some projects get no
group. The preferences decide which projects get the extra groups, and which
group gets which project.

### What "best" means

The score below measures how well an assignment of students to groups and
projects meets the preferences. The tool looks for an assignment with a high
score, with the guarantees under Forming. For a large class it cannot check
every assignment, so it need not find the very best one.

**Item value.** Every project and every possible teammate has a value for each
student. In a category holding *n* items, the item at position *i* (1 is
first) has the value:

* Preferred: (*n* − *i* + 1) / *n*. The first choice is worth 1 and the last
  1/*n*.
* Not preferred: −(*n* − *i* + 1) / *n*. The most unwanted item is worth −1
  and the last −1/*n*.
* Unspecified: 0.

**Project part.** A student's project part is the value of their group's
project.

**Teammate part.** For a student whose group has *m* other members:

* The *preferred share* is the sum of the values of the members in their
  Preferred category, divided by the largest sum any *m* members could give:
  the sum of the first *m* values of that category, or of all of them if it
  holds fewer than *m*.
* The *not-preferred share* is worked out the same way from their Not
  preferred category, and is 0 or negative.
* An empty category's share is 0. The teammate part is the sum of the two
  shares.

So the project part and the teammate part each range from −1 to 1, however
many items a student ranks.

**Student satisfaction** is:

> *W*<sub>p</sub> × project part + *W*<sub>t</sub> × teammate part

where *W*<sub>p</sub> and *W*<sub>t</sub> are the weights below.

**Score.** An assignment's score is the sum of every student's satisfaction.
A student who never saved preferences counts as having everything
Unspecified, but other students' preferences about them still count.

These teammate parts show the scaling, for a student `a`:

| `a`'s teammate preferences | `a`'s group's other members | Teammate part |
| --- | --- | --- |
| Preferred: `b`, `c` | `b`, `c` | 1 |
| Preferred: `b`, `c` | `b`, `d` | 2/3 |
| Preferred: `b` | `b`, `c` | 1 |
| Not preferred: `b`, `c` | `b`, `d` | −2/3 |
| Preferred: `b`, `c`; Not preferred: `d` | `c`, `d` | −2/3 |

### Grouping options

| Option | Control | Range | Default |
| --- | --- | --- | --- |
| Group size (*k*) | number | 1 to the number of students | see Group sizes |
| Project preference weight (*W*<sub>p</sub>) | number | 0–5 | 1 |
| Teammate preference weight (*W*<sub>t</sub>) | number | 0–5 | 1 |

Each control has a one-line help text. The fields take whole numbers: a
fraction is rounded to the nearest whole number, a number outside the range
is clamped to it, and an emptied field returns to its last value when it loses
focus.

The options are saved with the session. Changing one after groups are formed
marks the results as out of date: the results and the CSV file keep the groups
formed earlier, and the results show the options used to form them, until
groups are formed again.

### Forming

**Form groups** is available only while the session is Closed; while it is
Open, the button offers to close the session first. While groups are being
formed, the page says so, shows the time elapsed, and offers **Cancel**, which
keeps any earlier results. Forming again replaces the earlier results, after
the organizer confirms.

The result must be:

* **Valid:** it follows every rule under Group sizes.
* **Best for small classes:** with 8 students or fewer, no valid assignment
  has a higher score.
* **Not improvable by a simple change** for any class size: none of these
  gives a higher score while keeping the assignment valid: moving one student
  to another group, swapping two students in different groups, swapping the
  projects of two groups, or giving one group a different project.
* **Repeatable:** forming again with the same lists, preferences, and options
  gives the same groups, on any computer; the result does not depend on how
  fast forming runs.

Results
-------

### Results view

The results show, for each group, its number, its project, and its members.
Groups are numbered from 1 in the order of their projects in the project
list; groups working on the same project are numbered in the order of their
first member in the student list. Each member is shown by display name when
known and always by userid, with how their project preference was met:
*preferred #i*, *unspecified*, or *not preferred #i*.

A summary gives the number of students who got a preferred project, an
unspecified project, and a not-preferred project, and lists the projects
without a group. The results also show the options they were formed with.

The score is not shown, since it is partly computed from teammate preferences
(see Privacy between students).

### CSV download

**Download CSV** saves the results as a file whose name is the session title,
with every character other than a letter, a digit, `-`, or `_` replaced by
`_`, followed by `-groups.csv`. The file:

* is UTF-8 text, without a byte-order mark, following RFC 4180: CRLF line
  endings, and a field that contains a comma, a double quote, or a line break
  is quoted, with its double quotes doubled;
* starts with the header row `student_id,group,project`;
* has one row per student, in the order of the student list, giving their
  userid as entered by the organizer, their group number, and their group's
  project name.

The file is meant to be read by programs, such as the next project. Values are
written exactly as entered, even ones that a spreadsheet would treat as
formulas.

For example, three groups, the first two working on a project called
`Parser, part 2`:

```
student_id,group,project
a@uni.edu,1,"Parser, part 2"
b@uni.edu,2,"Parser, part 2"
c@uni.edu,1,"Parser, part 2"
d@uni.edu,3,Linter
```

Expected results
----------------

These rows are the reference behavior for forming groups. Each was checked by
scoring every valid assignment; each has exactly one best assignment. The
score is not shown in the tool; it is given here for tests.

Each row has students `a`–`d` and projects `X` and `Y`, in that order, unless
the Preferences column says otherwise, and every option is at its default
unless the Settings column says otherwise. With 4 to 6 students the default
group size is 2. Preferences are written as `PP` (preferred projects), `NP`
(not-preferred projects), `PT` (preferred teammates) and `NT` (not-preferred
teammates), each listed strongest feeling first; anything not listed is
Unspecified.

| Preferences | Settings | Groups | Score |
| --- | --- | --- | --- |
| a PP X; b PP X; c PP Y; d PP Y | | X: a, b; Y: c, d | 4 |
| a PP X, Y; b PP X; c PP X; d PP Y | | X: b, c; Y: a, d | 3.5 |
| a PT b; b PP X; c PT d | | X: a, b; Y: c, d | 3 |
| a NP X; b PP X; c PP X | | X: b, c; Y: a, d | 2 |
| a PP X, Y; b PP X; c PP Y; d PP Y, X | | X: a, b; Y: c, d | 4 |
| a PP X, Y NT b; b PP X NT a; c PP Y; d PP Y, X | | X: b, d; Y: a, c | 3 |
| a PP X PT c; b PP X; c PP Y, X PT a; d PP Y | | X: a, c; Y: b, d | 4.5 |
| a PP X PT c; b PP X; c PP Y, X PT a; d PP Y | *W*<sub>t</sub> 0 | X: a, b; Y: c, d | 4 |
| students `a`–`e`; a PP X; b PP X; c PP X; d PP Y; e PP Y | | X: a, b, c; Y: d, e | 5 |
| projects `X`, `Y`, `Z`; a PP Z; b PP Z; c PP Y; d PP Y | | Y: c, d; Z: a, b | 4 |
| project `X` only; a PT b; c PT d | | X: a, b; X: c, d | 2 |
| students `a`–`f`; a PP X PT b; b PP X; c PP X PT d; d PP X; e PP Y; f PP Y | | X: a, b; X: c, d; Y: e, f | 8 |
| students `a`–`f`; a PT b, c; b PP X; d PT e | group size 3 | X: a, b, c; Y: d, e, f | 3 |

The fifth and sixth rows differ only in the two not-preferred teammates, which
split `a` and `b`. With 7 students and no preferences, the default group size
is 3, giving 2 groups, of 4 and 3 students.

Stack
-----

| Part | Choice | Used for |
| --- | --- | --- |
| Front end | Single-page web app in TypeScript, built with Vite | Every page in the specification |
| Hosting | Firebase Hosting | Serving the app, rewrites for session links, security headers |
| Sign-in | Microsoft Entra ID, through MSAL.js (`@azure/msal-browser`) | Signing in with the account used for Teams |
| Session with the back end | Firebase Authentication, signed in with custom tokens | Giving each account a stable Firebase user id |
| Database | Cloud Firestore | All stored data, and live updates to open pages |
| Server code | Cloud Functions for Firebase (2nd gen), in TypeScript | Sign-in, every organizer action, forming groups, deletion |
| Shared code | A TypeScript package used by the app, the functions and the tests | List rules, group sizes, scoring |
| Local testing | Firebase Local Emulator Suite | Testing security rules and functions without the cloud |
| Deployment | GitHub Actions, deploying on every push to `main` | Hosting, Firestore rules and indexes, functions |

Cloud Functions need Firebase's pay-as-you-go **Blaze** plan, so the project
needs a billing account. At the specification's sizes, use should stay within
the free monthly allowance.

The UI framework is not chosen yet; nothing below depends on it.

### Why these choices

* **Firebase Hosting rather than GitHub Pages.** Hosting can send security
  headers, rewrite session links to the app, and serve the app and its
  back end from one Firebase project.
* **MSAL.js and custom tokens rather than Firebase's built-in Microsoft
  provider.** The specification matches students on their sign-in name and
  never on their email address (Signing in), and recognizes people by their
  account, not their sign-in name. Firebase's provider centers on the email
  address. With MSAL.js, the app gets the Microsoft ID token itself, a
  function checks it, and the Firebase user id is built from the account's
  stable Microsoft ids. Sign-in also never depends on third-party storage,
  which some browsers block.
* **Cloud Functions for everything the organizer changes.** Security rules
  cannot check the specification's list rules on 500 entries, keep several
  documents consistent, or keep secrets. Students' saves are the exception:
  they go straight to Firestore, guarded by rules, so that a save lands within
  2 seconds without waiting for a function to start (Saving).
* **One shared package for the rules of the specification.** The list rules
  (Session setup), the group sizes (Group sizes) and the scoring (What "best"
  means) run in the browser for instant feedback, in the functions as the
  rule that counts, and in the tests that check Expected results.

Microsoft Entra setup
---------------------

* Register one app in the university's Entra tenant, as a **single-tenant**
  app. Students and organizers then sign in only with university accounts.
  Ask the university's IT to grant admin consent if users cannot consent to
  apps themselves (Limits and failures: Sign-in).
* Request only the `openid` and `profile` scopes. The ID token then holds the
  name (`name`), the sign-in name (`preferred_username`), the account id
  (`oid`) and the tenant id (`tid`), and nothing else is requested, as the
  specification requires.
* Add the Hosting domain as a redirect URI for a single-page app.

Request flow
------------

### Signing in

1. The app signs in with MSAL.js: a popup on desktop, a redirect on phones.
2. It sends the Microsoft ID token to the `signIn` function.
3. `signIn` checks the token's signature against Entra's published keys, its
   issuer, its audience (the app's client id), its tenant, and its expiry.
4. `signIn` returns a Firebase custom token for the user id
   `ms_<tid>_<oid>`, with the claims `signInName` (the sign-in name, case
   folded) and `name`.
5. The app signs in to Firebase with that token. Firebase keeps the user
   signed in and puts the claims in every request, where security rules and
   functions read them.

The user id never changes for an account, so it is how the tool recognizes
organizers and students, even when a sign-in name changes (Sessions, Signing
in). The functions' service account needs the *Service Account Token Creator*
role to create custom tokens.

### Opening a student link

1. Before sign-in, the app reads `links/{sessionId}`, which anyone can read.
   If it does not exist, the page says the link is not valid (Signing in).
2. After sign-in, the app calls `openSession`. The function looks in
   `members/{uid}` for an earlier link to an entry. Otherwise it looks for a
   student entry whose case-folded userid equals the caller's `signInName`
   and that no account has used yet, and links it in a transaction.
3. If no entry matches, the function returns the organizer's name, and the
   page shows the "not on this session's list" message.

### Saving preferences

The student page writes `preferences/{entryId}` straight to Firestore, as one
whole document, so each save replaces the last (Saving). The security rules
accept the write only if:

* the caller is linked to that entry in `members/{uid}`;
* the session is Open, read from the `roster` document;
* the document's `listVersion` equals the roster's, so a page that loaded the
  lists before an entry was added cannot save preferences about it;
* every id is a current project or another current student, no id is listed
  twice, and `savedAt` is the server's time.

A refusal because the session is Closed is a permission error, which the
Firestore SDK does not retry, as the specification requires. A refusal
because of an old `listVersion` makes the page reload the lists, drop ids
that are no longer on them, and save again.

The `onPreferencesWritten` function marks the entry as *saved preferences*
the first time it saves.

### Organizer actions

The organizer pages read their session's documents directly, with live
updates, but change them only by calling functions:

| Function | Does |
| --- | --- |
| `createSession` | Checks the title and lists with the shared rules, and creates the session, its entries, its roster and its link |
| `updateLists` | Applies edited list text: an entry whose text differs only in case keeps its id; any other change removes the entry and adds a new one; deletes a removed student's `members` and `preferences` documents; increments `listVersion`; discards results |
| `renameEntry` | Changes a userid or project name and keeps the entry's id, so every preference stays; discards results |
| `setState` | Closes or reopens the session; reopening discards results |
| `setOptions` | Saves the grouping options and marks results as out of date |
| `formGroups` | Checks that the session is Closed and creates a forming job |
| `cancelForming` | Asks a running job to stop |
| `deleteSession` | Deletes the session and everything under it |

Functions that discard results ask for the organizer's confirmation in the
page first, as the specification requires.

### Forming groups

`formGroups` writes a job document; the `runForming` function starts when the
job appears and runs on the server, so it carries on if the organizer closes
or reloads the page (Forming). It:

1. reads the lists, options and every preferences document, ignoring ids no
   longer on the lists;
2. searches for groups, checking the job's `cancelRequested` field between
   steps;
3. writes the results and marks the job as done, cancelled or failed.

The search does a fixed amount of work, seeded from the inputs, so the same
inputs always give the same groups on any machine (Forming: Repeatable). It
runs with 2 CPUs, the machine named in Limits and failures: Forming time.

### Deleting

`deleteSession` and the daily `deleteExpiredSessions` function delete the
session document with all its subcollections, and its `links` document. A
session expires 365 days after its last change: the later of its
`updatedAt`, set by every organizer action, and its `lastStudentSaveAt`.
Firestore's own time-to-live feature is not used, since it does not delete
subcollections.

Data storage
------------

### Collections

```
links/{sessionId}
sessions/{sessionId}
sessions/{sessionId}/roster/current
sessions/{sessionId}/students/{entryId}
sessions/{sessionId}/members/{uid}
sessions/{sessionId}/preferences/{entryId}
sessions/{sessionId}/jobs/{jobId}
sessions/{sessionId}/results/current
```

Session ids and entry ids are Firestore's random 20-character ids, so a
session link cannot be guessed (Session page). Entries are identified by id,
never by their text: a renamed entry keeps its id, and an entry removed and
added again gets a new one, which is what keeps preferences across a
**Rename** and drops them across a remove and add (Session page).

### `links/{sessionId}`

Lets a student's page tell whether a link is valid before sign-in. It holds
no other data.

| Field | Type | Meaning |
| --- | --- | --- |
| `createdAt` | timestamp | When the session was created |

### `sessions/{sessionId}`

The session as the organizer sees it.

| Field | Type | Meaning |
| --- | --- | --- |
| `organizerUid` | string | The organizer's user id |
| `organizerName` | string | The organizer's name, shown to students |
| `title` | string | 1–100 characters |
| `state` | `open` or `closed` | Session state |
| `projects` | array of `{id, name}` | The project list, in order |
| `options` | map | `minGroupSize` (a number, or null while it is the default), `projectWeight` and `teammateWeight` (0–5) |
| `listVersion` | number | Incremented whenever an entry is added or removed |
| `studentCount`, `savedCount` | number | For the sessions list |
| `resultsState` | `none`, `current` or `outOfDate` | Whether groups are formed, and whether options changed since |
| `createdAt`, `updatedAt` | timestamp | `updatedAt` changes on every organizer action |
| `lastStudentSaveAt` | timestamp | Updated at most once an hour, for deletion |

The deletion date shown in the sessions list is computed from `updatedAt` and
`lastStudentSaveAt`.

### `sessions/{sessionId}/roster/current`

What a linked student's page needs to show the lists. Kept apart from the
session document so that students cannot read other students' statuses or
the organizer's options.

| Field | Type | Meaning |
| --- | --- | --- |
| `title`, `organizerName`, `state`, `listVersion` | | Copies of the session's fields |
| `projects` | array of `{id, name}` | The project list, in order |
| `students` | array of `{id, userid, displayName}` | The student list, in order; `displayName` is null until the student signs in |
| `projectIds`, `studentIds` | array of strings | The ids alone, for the security rules |

At the largest size this document is about 150 KB, well under Firestore's 1
MiB limit. Sign-ins update it one at a time; if 500 students sign in within a
few minutes, the writes stay near Firestore's guideline of one write per
second to a document.

### `sessions/{sessionId}/students/{entryId}`

One document per student entry, for the organizer's student list.

| Field | Type | Meaning |
| --- | --- | --- |
| `order` | number | Position in the student list |
| `userid` | string | As entered by the organizer, with the latest casing |
| `useridFolded` | string | Case folded, for matching at sign-in |
| `accountUid` | string or null | The account linked to this entry |
| `displayName` | string or null | From the account, at its last sign-in |
| `status` | `notSignedIn`, `signedIn` or `saved` | As shown on the session page |

### `sessions/{sessionId}/members/{uid}`

Links an account to its entry, so that rules can check a save with one read.

| Field | Type | Meaning |
| --- | --- | --- |
| `entryId` | string | The student entry this account uses |

### `sessions/{sessionId}/preferences/{entryId}`

One student's preferences, readable and writable only by that student. The
organizer cannot read it (Privacy between students); only functions do.

| Field | Type | Meaning |
| --- | --- | --- |
| `projectsPreferred`, `projectsNotPreferred` | array of entry ids | Strongest feeling first |
| `teammatesPreferred`, `teammatesNotPreferred` | array of entry ids | Strongest feeling first |
| `listVersion` | number | The roster version the page had loaded |
| `savedAt` | timestamp | Set to the server's time |

Unspecified items are not stored: an id in neither array of a section is
Unspecified, and the page shows Unspecified items in list order (Preferences
page). A student with no document has everything Unspecified (What "best"
means: Score).

### `sessions/{sessionId}/jobs/{jobId}`

One forming run.

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | `queued`, `running`, `done`, `cancelled` or `failed` | Shown on the session page |
| `requestedAt`, `startedAt`, `finishedAt` | timestamp | For the time elapsed |
| `cancelRequested` | boolean | Set by `cancelForming` |
| `listVersion`, `options` | | The inputs the run used |

### `sessions/{sessionId}/results/current`

The formed groups, for the results view and the CSV file.

| Field | Type | Meaning |
| --- | --- | --- |
| `formedAt` | timestamp | When the run finished |
| `options` | map | The options used, shown with the results |
| `groups` | array of `{number, projectId, memberIds}` | Numbered as in Results view |
| `projectOutcomes` | map from entry id to `{category, rank}` | How each student's project preference was met |
| `unusedProjectIds` | array of strings | Projects without a group |

The score is not stored here. The organizer can read this document, and
anything the organizer can read counts as shown to them, so it holds nothing
computed from teammate preferences except the groups themselves (Privacy
between students). The CSV file is built in the organizer's browser from this
document and the student list.

### Who can read and write

| Data | Organizer | Linked student | Anyone signed in | Not signed in |
| --- | --- | --- | --- | --- |
| `links` | read | read | read | read |
| `sessions`, `students`, `jobs`, `results` | read | — | — | — |
| `roster` | read | read | — | — |
| `members` | — | read their own | — | — |
| `preferences` | — | read and write their own | — | — |

Only functions write anything not marked "write" above. An organizer who is
also on their own student list has both columns' access.

Security headers
----------------

Firebase Hosting sends, for every page:

* a content security policy that allows scripts only from the app itself and
  connections only to Firebase and `login.microsoftonline.com`, and forbids
  framing the app;
* `X-Content-Type-Options: nosniff` and `Referrer-Policy: no-referrer`.

Hosting also sends HSTS, so all traffic uses HTTPS (Constraints: Security).
Session links (`/s/{sessionId}`) are rewritten to the app.

Testing
-------

* **Shared package:** unit tests for every Expected results row, the
  teammate-part table, and the random small classes in Acceptance criteria,
  each against a brute-force search.
* **Security rules and functions:** tests against the emulators for every
  row of "Who can read and write", for a save after the session closes, and
  for a save with an old `listVersion`.
* **End to end:** the Acceptance criteria scenario, run in a browser against
  the emulators, with sign-in replaced by test custom tokens.

Open decisions
--------------

* The UI framework.
* The Firestore and Functions region, which cannot be changed later; one near
  the university keeps pages fast.
* Whether the university's IT will register the app and grant admin consent,
  or the team registers it in a test tenant until then.


Limits and failures
-------------------

* **Sign-in.** Signing in asks only for permission to read the user's name,
  sign-in name, and email address. If sign-in fails or is cancelled, the page
  says so and offers to try again. If the user's organization requires an
  administrator to approve the tool first, the page says so and suggests
  asking the organization's IT administrator.
* **Lost connection.** If the tool cannot reach its server, every page says
  so. The student page keeps retrying, as described under Saving; the
  organizer pages keep what the organizer has entered while the page stays
  open.
* **Forming time.** Forming groups for 500 students and 100 projects finishes
  within 60 seconds.
* **Concurrent changes.** If the organizer edits the lists while a student's
  page is open, the student's page shows the new lists within a minute.
  Preferences about removed items are dropped as described under Session
  page.



Constraints
-----------

* **Security.** All traffic uses HTTPS. Only the organizer can open a
  session's organizer pages, and only students on the list can open its
  preferences page; every other request is refused, whatever address is used.
  Nothing a user enters, such as a title, project name, or display name, is
  ever run as a script or shown on a page as anything other than plain text.
* **Browsers:** the two latest major versions of Chrome, Edge, Firefox, and
  Safari, on desktop and phone.
* **Accessibility:** every control works from the keyboard, including ranking
  and moving items between categories; has a visible focus style and a label;
  and meets WCAG 2.1 AA, including its contrast requirements. Changes in the
  save indicator and in the forming status are announced to screen readers.
* **Themes:** the pages follow the system's light or dark setting.
* **Speed:** every page shows its content within 2 seconds over a 10 Mbit/s
  connection with 100 ms latency, for a session of the largest size.

Acceptance criteria
-------------------

* Every row of Expected results gives the stated groups and score, and every
  row of the teammate-part table under What "best" means gives the stated
  value.
* With random lists of at most 8 students and preferences, the result's score
  equals the best score found by checking every valid assignment.
* End to end: an organizer creates a session with four students and two
  projects; three of the students sign in through the link and save
  preferences, and the fourth never signs in; the organizer closes the
  session, forms groups, and downloads a CSV file that lists all four
  students, in the order of the student list, in two groups of 2, one for each
  project.

Out of scope
------------

Sign-in other than with a Microsoft work or school account; running inside the
Teams app as a tab or bot; importing the student list from a Teams roster;
sending notifications or showing students their groups; deadlines that close a
session automatically; several organizers per session; editing groups by hand
in the tool; group sizes or spreads of projects other than those under Group
sizes; groups with several projects each; and exporting any format other than
the CSV file above.
