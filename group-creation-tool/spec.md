Project group creation tool
===========================

A web app that collects students' project and teammate preferences and forms
project groups that satisfy them as well as possible. The result is a `.csv`
file used as input by the team evaluation tool.

Roles and sign-in
-----------------

There are two roles. An **organizer** creates and manages a survey. A
**student** submits preferences for a survey.

Everyone signs in with a Microsoft Entra ID (Teams) account using the OpenID
Connect authorization-code flow (`msal.ConfidentialClientApplication`), as
described in [Signing in with Microsoft Teams accounts](https://github.com/bjones1/literate-programming-book/blob/main/project_notes/teams_sign_in.md).
A signed-in user is identified by the pair `tid`/`oid` from the ID token. The
`name` claim is used only for display.

Any signed-in user may create a survey and becomes its organizer. Only the
organizer of a survey can view or change it.

Surveys
-------

To create a survey, the organizer provides:

* **Roster**: the students' userids, one per line. A userid is the account's
  sign-in name (`preferred_username`, such as `jdoe@school.edu`) and is compared
  case-insensitively. Blank lines and duplicates are dropped.
* **Projects**: project names, one per line. Blank lines and duplicates are
  dropped.
* **Group size** `m`: an integer of at least 2.
* **Project weight** `w`: a number from 0 to 1, default 0.5. The teammate
  weight is `1 − w`.
* **Groups per project** `c`: the most groups one project may have. It defaults
  to the smallest value that makes grouping possible (see Group sizes).

The app rejects a survey with fewer than 2 students or fewer than 1 project.
After creation, the roster and project list cannot be changed; the organizer
creates a new survey instead. `w` and `c` can be changed until the survey is
closed.

Creating a survey produces a link containing a random, unguessable token. The
organizer posts this link.

A survey is **open** until the organizer closes it. Closing is final.

Student flow
------------

A student follows the link and signs in. If the account's userid is not on the
roster, the app shows an error and stores nothing. On the first successful
sign-in, the app binds the roster entry to the account's `tid`/`oid`. Later
sign-ins are matched by `tid`/`oid`, so a later change of the sign-in name
does not lock the student out.

The student then sees two lists: the survey's projects, and the other students
on the roster (by display name if they have signed in, otherwise by userid).
For each list, every item is in exactly one of three categories:

* **Preferred**, ranked from most to least wanted.
* **Unspecified**, not ranked. All items start here.
* **Not preferred**, ranked from least to most unwanted (the last item is the
  least wanted).

The student clicks **Submit** to save both lists. While the survey is open, the
student can return and change them; each submission replaces the previous one.
After the survey closes, the student can only view what was submitted. A
student never sees another student's preferences.

Forming groups
--------------

The organizer can form groups at any time from the submissions saved so far.
Students who have not submitted are treated as having every item unspecified.

### Group sizes

With `n` students and group size `m`, the app forms `g = ⌈n / m⌉` groups. Their
sizes differ by at most one: `n mod g` groups have `⌈n / g⌉` students and the
rest have `⌊n / g⌋`. Each group is assigned one project, and no project is
assigned to more than `c` groups. Grouping requires `k · c ≥ g` for `k`
projects, so `c` defaults to `⌈g / k⌉`; a smaller value is rejected. Projects
may end up with no group.

### Score

A student's ranking of a list is turned into a score for each item in the
list:

* the item at position `i` (from 1) of `a` preferred items scores
  `(a − i + 1) / a`, from 1 for the top item down to `1 / a`;
* an unspecified item scores 0;
* the item at position `j` (from 1) of `b` not-preferred items scores `−j / b`,
  from `−1 / b` down to −1 for the least wanted item.

Dividing by the list length gives every student the same total influence,
however many items they rank.

A student's **satisfaction** with an assignment is

`w · P + (1 − w) · T`

where `P` is the student's score for the group's project and `T` is the mean of
the student's scores for the other members of the group.

The **quality** of an assignment is the sum of all students' satisfaction. One
assignment is **better** than another if its quality is higher, or if the
qualities are equal and its least satisfied student is more satisfied. Values
are compared with a tolerance of 10⁻⁹, so rounding never decides a comparison.

### Algorithm

Finding the best assignment is NP-hard, so the app uses a deterministic local
search:

1. **Start.** Shuffle the roster with a pseudo-random generator seeded from the
   survey id and the restart number, then fill the groups in that order, largest
   groups first.
2. **Assign projects.** Given the groups, assign projects to maximize quality.
   This is a weighted assignment problem with `c` copies of each project, solved
   exactly with `scipy.optimize.linear_sum_assignment`.
3. **Improve members.** With projects fixed, consider every swap of two
   students in different groups, in roster order. Apply a swap whenever it
   makes the assignment better, and repeat until a full pass makes no swap.
4. Repeat steps 2–3 until neither changes the assignment.
5. Run steps 1–4 for 20 restarts and keep the best result.

The same inputs (submissions, `m`, `w`, `c`) always produce the same groups.

### Result

The organizer sees each group with its project and members, each student's
satisfaction, and how many of each student's not-preferred items (projects or
teammates) they received. The organizer can download the result as a `.csv`
file.

CSV format
----------

UTF-8, comma-separated, quoted as in RFC 4180, with a header row:

```
userid,team
jdoe@school.edu,YAML support
asmith@school.edu,htmd fuzzer 2
```

There is one row per student, sorted by team and then userid. `team` is the
project name for the project's first group; further groups of the same project
are named `<project> 2`, `<project> 3`, and so on, in the order the algorithm
produced them.

Design decisions
----------------

1. **The organizer sets the group size.** The assignment's "≈ 1/n" group size
   cannot be meant literally. A target size, rather than one group per project,
   keeps groups at a workable size when there are many projects, and allows a
   popular project to have several groups, as in past courses.
2. **"Not preferred" is a penalty, not a ban.** The assignment calls it a
   preference. A ban could make grouping impossible (for example, when most
   students reject one project and every project must be used); a penalty
   always produces a result, and the result view shows each violation.
3. **Sum of satisfaction, ties by the worst-off student.** Maximizing the sum is
   simple and predictable; the tie-break prevents needlessly sacrificing one
   student when it costs nothing.
4. **Local search instead of an exact solver.** The problem is NP-hard. Swaps
   plus an exact project assignment give good results for class-sized rosters
   (up to a few hundred students) in seconds, with no solver dependency beyond
   SciPy.
5. **Determinism.** Seeding from the survey id makes results reproducible, so
   forming groups twice from the same data never surprises the organizer.
6. **Identity by `tid`/`oid`.** Names and emails can change; `tid`/`oid` cannot.
   The roster uses sign-in names only because that is what an organizer knows.
7. **Fixed roster and projects.** Editing them after students have answered
   would invalidate rankings; creating a new survey is simpler and safe.

Implementation
--------------

Python 3.12 with FastAPI, server-rendered Jinja2 pages, SortableJS for
drag-and-drop ranking, `msal` for sign-in, SciPy for the assignment step, and
SQLite (through the standard `sqlite3` module) for storage. The code lives in
`group-creation-tool/`; the Entra client id, secret and redirect URI come from
environment variables.

Testing
-------

1. **Score**: unit tests for each category, list lengths 1 to 5, and the
   satisfaction formula, including `w = 0`, `w = 1` and a group of one.
2. **Group sizes**: for all `n ≤ 30` and `m ≤ 6`, sizes sum to `n`, differ by at
   most one, and none exceeds `m`.
3. **Algorithm invariants**: every student is in exactly one group, no project
   exceeds `c` groups, and repeated runs give identical output.
4. **Algorithm quality**: on fixed random instances with `n ≤ 8`, the result
   equals the optimum found by exhaustive search.
5. **Sign-in**: with MSAL mocked, an unlisted userid is rejected, a listed one
   is bound to its `tid`/`oid`, and a renamed account is still recognized.
6. **CSV**: the export of a known result matches an expected file exactly.
