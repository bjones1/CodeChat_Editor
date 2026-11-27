Project group creation tool: architecture
=========================================

This document describes how the tool in [specification.md](specification.md)
is built: the services it runs on, how requests flow between them, and the
data it stores. The specification says what the tool does; where the two
disagree, the specification wins and this document is corrected.

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
  app, so that only the organization's accounts can sign in (Purpose). Entra
  rejects accounts of other tenants, and the app shows the specification's
  message for them. Ask the university's IT to grant admin consent if users
  cannot consent to apps themselves (Limits and failures: Sign-in).
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

Every refusal is a permission error, which the Firestore SDK does not retry,
while a write that cannot reach Firestore stays queued and is retried. After a
permission error the page reads, in this order:

1. `links/{sessionId}`: if it is gone, the session was deleted;
2. its own `members/{uid}` document: if it is gone, the student was removed;
3. the roster: if the session is Closed, the save was refused for that;
4. otherwise the save carried an old `listVersion`, so the page reloads the
   lists, drops ids that are no longer on them, and saves again.

The first three give the three messages under Saving.

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
| `formGroups` | Checks that the session is Closed and no job is under way, creates a forming job, and sets `formingJobId` |
| `cancelForming` | Asks a running job to stop |
| `deleteSession` | Deletes the session and everything under it, including any job |

Functions that discard results ask for the organizer's confirmation in the
page first, as the specification requires.

While `formingJobId` is set, `updateLists`, `renameEntry`, `setOptions`, and
`setState` when reopening refuse to run (Forming). Each of them, and
`formGroups`, reads `formingJobId` in the same transaction as its change, so
an edit and the start of a job cannot both succeed.

### Forming groups

`formGroups` writes a job document; the `runForming` function starts when the
job appears and runs on the server, so it carries on if the organizer closes
or reloads the page (Forming). It:

1. reads the lists, options and every preferences document, ignoring ids no
   longer on the lists;
2. searches for groups, checking the job's `cancelRequested` field between
   steps;
3. in one transaction, checks that the session still exists, that the job
   was not cancelled, and that the session's `listVersion` and options equal
   the ones the job recorded; then writes the results, marks the job as done
   and clears `formingJobId`. If a check fails, it writes no results and
   marks the job as cancelled or failed.

The refusals under Organizer actions should make the checks in step 3 always
pass; they guard against a bug that lets an edit through.

The search starts from an assignment built from the inputs alone, with any
random choices drawn from a generator seeded from them. It then applies the
four simple changes under Forming until none raises the score. It never reads
the clock, so the same inputs give the same groups on any machine with the
same version of the code (Forming: Repeatable). Scores are exact fractions,
with `BigInt` numerators and denominators, never floating-point numbers (What
"best" means: Score). It runs with 2 CPUs, the machine named in Limits and
failures: Forming time.

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
| `formingJobId` | string or null | The job that is queued or running, if any |
| `createdAt`, `updatedAt` | timestamp | `updatedAt` changes on every organizer action |
| `lastStudentSaveAt` | timestamp | Updated at most once an hour, for deletion |

The deletion date shown in the sessions list is computed from `updatedAt` and
`lastStudentSaveAt`.

### `sessions/{sessionId}/roster/current`

What a linked student's page needs to show the lists. Kept apart from the
session document so that students cannot read whether classmates have saved
preferences, or the organizer's options. A classmate's `displayName` does show
that they have signed in, which the specification accepts (Privacy between
students).

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
computed from teammate preferences except the groups and what follows from
them (Privacy between students). The CSV file is built in the organizer's
browser from this document and the student list.

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
  row of "Who can read and write"; for a save after the session closes, after
  it is deleted, after the student is removed, and with an old `listVersion`;
  and for every organizer action while a job is under way.
* **End to end:** the Acceptance criteria scenario, run in a browser against
  the emulators, with sign-in replaced by test custom tokens.

Open decisions
--------------

* The UI framework.
* The Firestore and Functions region, which cannot be changed later; one near
  the university keeps pages fast.
* Whether the university's IT will register the app and grant admin consent,
  or the team registers it in a test tenant until then.
* Whether exact fractions are fast enough for the 60-second limit at 500
  students. If not, the search can compare floating-point scores first and
  fall back to exact fractions only when two scores are too close for
  floating point to tell apart.
