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

Everyone signs in with their Microsoft Teams (Microsoft 365 work or school)
account. A person's **userid** is the sign-in name of that account, such as
`student@university.edu`. Userids are compared without regard to case.

Sessions
--------

A **session** is one run of the tool: one organizer, one student list, one
project list, the students' preferences, and the formed groups. A session is
either **Open**, when students can change their preferences, or **Closed**,
when they cannot. A new session is Open.

The organizer can close and reopen a session at any time. Reopening discards
formed groups, after the organizer confirms. A session is kept until its
organizer deletes it; deleting asks for confirmation, removes all its data, and
cannot be undone.

The organizer is the only person who can see or change a session's
organizer pages. Anyone who signs in can create sessions and becomes the
organizer of the sessions they create. An organizer may also be on the student
list of their own session.

Organizer pages
---------------

### Sessions list

After signing in at the tool's address, the organizer sees their sessions,
newest first, each with its title, its state, the number of students who have
saved preferences out of the total, and whether groups are formed. A **New
session** button starts the setup below.

### Session setup

The organizer enters:

* A **title**, 1–100 characters, shown to the students.
* The **student list**: userids, separated by commas, spaces, or new lines.
* The **project list**: one project name per line, 1–200 characters each.

In both lists, leading and trailing spaces are trimmed, empty entries are
ignored, and an entry that repeats an earlier one (ignoring case) is dropped.
In the student list, an entry that is not of the form `name@domain` is
dropped. Every dropped entry is listed under its field with the reason, so the
organizer can correct it. The lists keep the order in which they were entered.

A session needs at least 1 student and 1 project, and holds at most 500
students and 100 projects. **Create** is unavailable until these hold.

Creating the session shows the session page.

### Session page

The session page shows:

1. The title, the state, and **Close** or **Reopen**.
2. The **student link** with a **Copy** button. If the browser does not allow
   copying, the link is shown selected, with a message asking the organizer to
   copy it manually. The link stays the same for the life of the session and
   cannot be guessed from the title or from other sessions' links.
3. The **student list**, showing for each student their userid, their display
   name once they have signed in, and one of: *not signed in*, *signed in*,
   *saved preferences*. A count above the list gives the number in each
   status.
4. The **project list**.
5. The **grouping options** and **Form groups** (see Forming groups).
6. The **results**, once groups are formed (see Results).
7. **Delete session**.

The organizer can edit both lists, using the same rules as at setup. A student
or project that is removed disappears from every student's preferences; one
that is added appears as unspecified for everyone. Editing the lists discards
formed groups, after the organizer confirms. If the organizer removes a student
or project and adds it again, its earlier preferences are not restored.

Student page
------------

### Signing in

The student link asks the student to sign in, unless they are already signed
in. Then:

* If their userid is on the student list, they see the preferences page.
* Otherwise, they see a message naming their userid and the organizer, saying
  they are not on this session's list and should contact the organizer, with
  a **Sign out** button so they can switch accounts.
* If the session was deleted, they see a message saying so.

### Preferences page

The page shows the session title, the organizer's name, the session's state,
and the student's own name with **Sign out**. It has two sections, **Projects**
and **Teammates**, which work the same way:

* Each section has three categories, shown in this order: **Preferred**,
  **Unspecified**, and **Not preferred**.
* **Projects** lists every project. **Teammates** lists every other student
  on the list, by display name when known and always by userid; it does not
  list the student themselves.
* At first, every item is in Unspecified. Unspecified items keep the order of
  the organizer's list; the student cannot reorder them.
* Within Preferred and Not preferred, the student ranks the items, **strongest
  feeling first**: the first Preferred item is the one they most want, and the
  first Not preferred item is the one they most want to avoid. Each category
  says this in its heading.
* An item moved into a category goes to its end. The student can move any item
  to another category and up or down within its category, by dragging it or by
  using buttons and keys on the item.
* A student may leave any category empty, including putting everything in
  Unspecified.

On narrow screens (under 800 px wide) the categories stack, Preferred first.

### Saving

Each change is saved automatically within about a second; no button is needed.
A save indicator shows *Saving…*, *Saved*, or *Not saved*, with a **Retry**
button for the last. Leaving the page with unsaved changes asks the student to
confirm. If the same student edits in two tabs, the later save wins, and the
other tab shows the saved state the next time it saves or is reloaded.

A student has *saved preferences* once any change of theirs has been saved.

While the session is Closed, the page shows the student's saved preferences
read-only, with a notice that preferences are closed. A change made just as the
session closes is not saved, and the page says so.

### Privacy between students

A student sees only their own preferences. Nobody, including the organizer,
ever sees a student's teammate preferences; the organizer sees how each
student's project preferences were met (see Results).

Forming groups
--------------

### Group sizes

The organizer chooses the **number of groups**, *G*. It ranges from 1 to the
smaller of the number of projects and the number of students, and defaults to
that upper limit, so that by default every project gets a group. With *S*
students, each group gets about *S*/*G* of them: every group has either
⌊*S*/*G*⌋ or ⌈*S*/*G*⌉ students. Every student is in exactly one group, and
each group works on a different project. When *G* is smaller than the number
of projects, some projects are left without a group.

A number outside the range is clamped to it, and an emptied number field
returns to its last value when it loses focus. When the lists change, *G* is
clamped to the new range; if it was at the default, it follows the new upper
limit.

### What "best" means

The tool scores every possible assignment and looks for the one with the
highest score.

**Item value.** Every project and every possible teammate has a value for each
student. In a category holding *n* items, the item at position *i* (1 is
first) has the value:

* Preferred: (*n* − *i* + 1) / *n*. The first choice is worth 1 and the last
  1/*n*.
* Not preferred: −(*n* − *i* + 1) / *n*. The most unwanted item is worth −1
  and the last −1/*n*.
* Unspecified: 0.

Because each category's values are scaled by its length, ranking more items
does not give a student more weight than ranking fewer.

**Student satisfaction.** For a student in a group, satisfaction is:

> *W*<sub>p</sub> × (value of the group's project)
> \+ *W*<sub>t</sub> × (sum of the values of the group's other members)

where *W*<sub>p</sub> and *W*<sub>t</sub> are the weights below.

**Score.** An assignment's score is the sum of every student's satisfaction.
A student who never saved preferences counts as having everything
Unspecified, but other students' preferences about them still count.

### Grouping options

| Option | Control | Range | Default |
| --- | --- | --- | --- |
| Number of groups (*G*) | number | see Group sizes | see Group sizes |
| Project preference weight (*W*<sub>p</sub>) | number | 0–5 | 1 |
| Teammate preference weight (*W*<sub>t</sub>) | number | 0–5 | 1 |

Each control has a one-line help text. The weights use the same clamping rule
as the number of groups. The options are saved with the session.

### Forming

**Form groups** is available only while the session is Closed; while it is
Open, the button offers to close the session first. Forming shows progress and
can be cancelled, which keeps any earlier results. Forming again replaces the
earlier results, after the organizer confirms.

The result must be:

* **Valid:** it satisfies every rule under Group sizes.
* **Best for small classes:** with 8 students or fewer, no valid assignment
  has a higher score.
* **Not improvable by a simple change** for any class size: no move of one
  student to another group that keeps the sizes valid, no swap of two students
  in different groups, no swap of the projects of two groups, and no change of
  a group's project to an unused project gives a higher score.
* **Repeatable:** forming again with the same lists, preferences, and options
  gives the same groups.

Results
-------

### Results view

The results show the score and, for each group, its number, its project, and
its members. Groups are numbered from 1 in the order of their projects in the
project list. Each member is shown with how their project preference was met:
*preferred #i*, *unspecified*, or *not preferred #i*. A summary gives, for the
whole class, the number of students who got a preferred project, an
unspecified project, and a not-preferred project, and the number of students
with at least one preferred teammate and with at least one not-preferred
teammate in their group. Projects without a group are listed as unused.

### CSV download

**Download CSV** saves the results as a file named after the session title,
with characters that are not allowed in file names replaced by `_`, followed
by `-groups.csv`. The file:

* is UTF-8 text, without a byte-order mark, following RFC 4180: CRLF line
  endings, and a field that contains a comma, a double quote, or a line break
  is quoted, with its double quotes doubled;
* starts with the header row `student_id,group,project`;
* has one row per student, in the order of the student list, giving their
  userid as entered by the organizer, their group number, and their group's
  project name.

For example, two groups, the first working on a project called
`Parser, part 2`:

```
student_id,group,project
a@uni.edu,1,"Parser, part 2"
b@uni.edu,2,Linter
c@uni.edu,1,"Parser, part 2"
```

Expected results
----------------

These rows are the reference behavior for forming groups. Each was checked by
scoring every valid assignment; each has exactly one best assignment. Every
row has students `a`–`d` (or `a`–`e`) and projects `X` and `Y`, in that order,
with every option at its default unless the Settings column says otherwise.
Preferences are written as `PP` (preferred projects), `NP` (not-preferred
projects), `PT` (preferred teammates) and `NT` (not-preferred teammates), each
listed strongest feeling first; anything not listed is Unspecified.

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
| a PP X; b PP X; c PP X; d PP Y; e PP Y (5 students) | | X: a, b, c; Y: d, e | 5 |
| a PP Z; b PP Z; c PP Y; d PP Y (projects `X`, `Y`, `Z`) | 2 groups | Y: c, d; Z: a, b | 4 |

The fifth and sixth rows differ only in the two not-preferred teammates, which
split `a` and `b`. With 7 students, 3 projects, and no preferences, the groups
have 3, 2, and 2 students.

Limits and failures
-------------------

* **Sign-in.** Signing in asks only for permission to read the user's name and
  sign-in name. If sign-in fails or is cancelled, the page says so and offers
  to try again.
* **Lost connection.** If the tool cannot reach its server, every page says so
  and keeps what the user has entered; the student page keeps retrying the
  save, as described under Saving.
* **Forming time.** Forming groups for 500 students and 100 projects finishes
  within 60 seconds.
* **Concurrent changes.** If the organizer edits the lists while a student's
  page is open, the student's page shows the new lists within a minute, and at
  the latest when it next saves. Preferences about removed items are dropped
  as described under Session page.

Constraints
-----------

* **Security.** All traffic uses HTTPS. Only the organizer can open a
  session's organizer pages, and only listed students can open its preferences
  page; every other request is refused, whatever address is used. Nothing a
  user enters, such as a title, project name, or display name, is ever run as
  a script or shown as anything other than plain text.
* **Browsers:** current Chrome, Edge, Firefox, and Safari, on desktop and
  phone.
* **Accessibility:** every control works from the keyboard, including ranking
  and moving items between categories; has a visible focus style and a label;
  and meets WCAG 2.1 AA, including its contrast requirements. Changes in the
  save indicator and in forming progress are announced to screen readers.
* **Themes:** the pages follow the system's light or dark setting.
* **Speed:** every page shows its content within 2 seconds on a typical
  connection, for a session of the largest size.

Acceptance criteria
-------------------

* Every row of Expected results gives the stated groups and score.
* With random lists of at most 8 students and preferences, the result's score
  equals the best score found by checking every valid assignment.
* End to end: an organizer creates a session with three students and two
  projects; two of the students sign in through the link and save
  preferences, and the third never signs in; the organizer closes the session,
  forms groups, and downloads a CSV file that lists all three students, in the
  order of the student list, with groups of 2 and 1.

Out of scope
------------

Sign-in other than with a Microsoft work or school account; running inside the
Teams app as a tab or bot; importing the student list from a Teams roster;
sending notifications or showing students their groups; deadlines that close a
session automatically; several organizers per session; editing groups by hand
in the tool; groups of unequal sizes or with several projects each; and
exporting any format other than the CSV file above.
