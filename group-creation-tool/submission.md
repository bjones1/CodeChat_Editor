Project 2 submission
====================

Team name: Project Group Creation Tool 2

The specification consists of the file [specification.md](specification.md), with links to the architecture file.

The submission consists of the file [submission.md](submission.md).

The architecture consists of the file [architecture.md](architecture.md).
Draft submission
----------------

### Daniil Kaimekin

GitHub userid: dkaimekinmasters

Git userid: dkaimekin

#### Contributions

Generated initial specification for the project, conducted multiple calls to
clarify requirements.

#### Build and run test

Build and run tests are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Specification review rounds

| Round | Findings | Main Changes Made |
| --- | --- | --- |
| 1 | Rated 7/10; 25 findings in total. **High:** teammate scoring contradicts the "no extra weight" claim, Unspecified ordering rules conflict, teammate privacy can't be guaranteed, default group count gives solo groups when projects outnumber students, userid relies on a mutable sign-in claim. **Medium:** deleted-session message vs. deleting all data, two-tab save rule, repeatability vs. time limit, stale results after option changes. **Low:** 7 vague terms and minor gaps (over-long names, CSV, retention, terminology). Expected results table verified correct. | Fixed every finding except the 2 struck through below and the tie-break part of item 9, plus 2 findings from CodeRabbit's review of the pull request. Group size is now about √S (an organizer option, at least 2), projects are spread evenly and a project can have several groups, and the teammate part of satisfaction is scaled to −1…1. Results no longer show the score or teammate counts, and the privacy promise now covers only showing teammate preferences. Unspecified always keeps list order. Added Rename, filter fields, and move to top or bottom. Students are matched by sign-in name or email, then by account. Added automatic save retries, last-save-wins across tabs, out-of-date results after option changes, 365-day retention, and testable timings and browser versions. Added Expected results rows and a teammate-part table, all rechecked by brute force. |
| 2 | Rated 8/10; 20 findings in total. **High:** not-preferred share has the wrong sign as written, CSV example needs 3 groups from 4 students (impossible), group size 1 divides by zero, matching on the email claim lets one person pass as another and lock out the real student. **Medium:** closed-session save rejection loops on retry, a stale tab can restore preferences for a removed and re-added entry, privacy wording contradicts itself, "group size" isn't the actual size, organizer identity is undefined, the 60-second limit names no machine, the ≤8-student test oracle is unbounded in projects. **Low:** 9 vague terms ("last change", "changes it", rounding of halves, "letter" in file names, userid syntax, separators, title trimming, Rename collisions, propagation of Close/Reopen/Delete). Expected results table and default group sizes verified correct by brute force. | Fixed the 11 High and Medium findings and CodeRabbit's 2 comments on the pull request, which repeat items 1 and 3; left the 9 Low vague terms as they are. The not-preferred share divides by the size of the most negative sum, and both shares are 0 for a student alone in a group. The CSV example now has 6 students in 3 groups. Students are matched on sign-in name only, not email, and organizers are recognized by their account. A save refused because the session closed is final, and a save from a stale page never applies to a re-added entry. The privacy rule says "number" instead of "figure". The option is renamed "Minimum group size", with sizes from *k* to under 2*k*. Forming runs on the server, survives a page reload, and its 60-second limit names a 2-core server. The small-class acceptance test is limited to 4 projects. Expected results rechecked by brute force. |
| 3 | Rated 9/10; 15 new findings in total, plus the 9 Low vague terms carried over from round 2. **Medium:** "after the session closed" can be read as refusing every change even after a reopen, saves refused because the session was deleted or the student was removed still retry forever, an extra Unspecified teammate lowers the teammate part, the spec doesn't require exact score comparison (floating-point ties break "best", "not improvable" and the acceptance test), "Repeatable" has no version boundary. **Low:** a "fewer than *k* students" clause that can never apply, dragging vs. "goes to its end", 6 vague terms (moves while filtered, how list edits are applied, editing the title, "out of date", the 2-core server, page speed including sign-in), slow correction of mismatched sign-in names, no quotas on sessions. Expected results table, teammate-part table and default group sizes verified correct by brute force. | |
| 5 | Rated 7/10 (9/10 without the pasted architecture); 11 new findings in total, plus every round-3 finding, all still open since the behavior text hasn't changed. **High:** the architecture document was pasted into the spec (lines 414–751), against the spec's own scope, splitting its behavior sections, with self-references that no longer make sense and a second copy that will drift. **Medium:** single-tenant sign-in vs. "anyone who signs in" and "the user's organization", display names in the Teammates list show who has signed in, a fixed amount of search work vs. "not improvable", forming races with list edits, Reopen and option changes, one permission error for a closed session, a deleted session and a removed student, Rename can't fix a link to the wrong person, the privacy wording still forbids the result counts. **Low:** "on the list" doesn't cover linked accounts whose sign-in name changed, the acceptance criteria skip the hardest guarantees, "but at least 1" can never apply. Expected results table, teammate-part table and default group sizes verified correct by brute force with exact fractions. | Fixed the High and Medium findings, including the round-3 Medium findings still open, and left the Low ones (items 8–10, the dead "fewer than *k*" clause, dragging, and the vague terms) as they are. Removed the pasted architecture from the spec; [architecture.md](architecture.md) is now the only copy, and the spec's opening points to it instead of the clause about code organization. The spec now says the tool serves one organization and only its accounts sign in, matching the single-tenant design. Students still see classmates' display names: the spec now says this shows who has signed in, and that this is accepted, since the tool is used within a faculty whose students largely know each other and names are easier to find than userids. Forming improves until no simple change helps and never stops because time runs out; Repeatable holds for one version of the tool; scores are compared exactly as fractions. While forming runs, the organizer cannot edit or rename entries, reopen the session or change options, and the architecture refuses these in the same transaction that starts a job. A closed session, a deleted session and a removed student each get their own message, and none is retried; after a reopen the page is editable again. The spec says removing and re-adding an entry, not Rename, gives it to a different person. The privacy rule now allows the groups and what follows from them. The finding that an extra Unspecified teammate lowers the teammate part is struck through: this is intended, and the spec now explains why. The architecture was updated to match each change. |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in the Claude Code tab of the Claude
desktop app.

Prompt:

> My teammate has added new changes to the repo and specification. Review a
> specification at @group-creation-tool/specification.md .

Output (line numbers refer to [specification.md](specification.md) as it was
reviewed, before this round's changes; strikethrough marks findings judged
incorrect):

I'd rate it **7/10**, down from 9 in round 3. None of the behavior text has
changed since round 3. The only change is commit `bb9fab1`, which pastes the
whole architecture document into the spec, at lines 414–751. That block is
identical to lines 9–345 of `architecture.md`. Without the paste the spec
would still be a 9.

I checked the 13 Expected results rows by testing every valid assignment with
exact fractions. Each gives exactly the stated groups and score, and each has
exactly one best assignment. All 5 teammate-part rows are correct, and the
default group sizes for 1–7, 13, 50 and 500 students match the text. I didn't
modify the file.

##### The pasted architecture

* **It contradicts the spec's own scope.** Lines 4–5 say the spec "does not
  prescribe how the code is organized". It now names Firestore collections,
  Cloud Functions and security rules.
* **It breaks the reading order.** It sits between Expected results and Limits
  and failures, so the behavior sections are split by 340 lines of
  implementation detail.
* **Its references no longer make sense.**
  * "As the specification requires" (lines 468, 517, 541) now points the spec
    at itself.
  * "Nothing below depends on it" (line 433) now covers Limits, Constraints
    and Acceptance.
  * The paste left out the opening paragraph of `architecture.md`, which says
    the spec wins when the two disagree.
* **There are now two copies** (this one and `architecture.md`) that will
  drift apart. There are also stray blank lines at 751–752 and 773–775.

Fix: remove lines 414–752, commit `architecture.md` as its own file, and link
to it from the spec. The paste does have one benefit: it shows where the spec
and the design disagree. Those disagreements need fixing wherever the
architecture ends up.

##### Spec and architecture disagree

1. **Which organizations can sign in.** Line 461 makes the app single-tenant,
   for university accounts only. The spec says "Anyone who signs in can create
   sessions" (line 39), and lines 758–760 handle "the user's organization",
   which implies many organizations. Decide this in the spec. Single-tenant
   would also solve the missing-quotas finding from round 3.
2. **Students can see who has signed in.** The spec has the Teammates list
   show a classmate's display name "when known" (lines 143–145). The roster
   holds `displayName = null` until that student signs in (line 630). So any
   student can tell which classmates have signed in, which is exactly what
   lines 622–624 say the roster is designed to hide. The spec should either
   accept this or show only userids to students.
3. **A fixed amount of work vs. "not improvable".** Lines 555–556 give the
   search a fixed amount of work so that results repeat. Lines 321–324 require
   that no simple change improves the result. If the work runs out before no
   simple change helps, the second guarantee fails. If the search keeps going
   until then, the amount of work isn't fixed. The spec should say that
   repeatable means a fixed process followed by improvement until no simple
   change helps.
4. **Forming can race with edits.** The job records its `listVersion` (line
   685), but no rule throws its results away if the lists are edited, the
   session is reopened, or the options change while it runs. `updateLists` and
   `setState` discard the results first (lines 532 and 534), and then the job
   writes stale groups that look current. The spec has left editing during
   forming open since round 2; the design now shows it's a real bug.
5. **One error for three different refusals.** Lines 516–517 treat every
   refused save as a permission error. A closed session, a deleted session and
   a removed student all look the same, so a removed student is told that
   preferences are closed. This is round 3's item 2 again: the spec should
   give each case its own behavior.

##### New spec findings

6. **Rename can't fix a link to the wrong person** (lines 105–108 and
   128–130). Suppose the organizer typed a userid that belongs to another real
   person, and that person signs in first. Rename keeps them linked, and the
   intended student is locked out. Only removing and re-adding the entry
   breaks the link, and the spec never says so. Say that, or add an **Unlink**
   action.
7. **The privacy wording still contradicts itself** (lines 191–193). The
   result counts (lines 341–342) and *preferred #i* depend on the assignment,
   which depends on teammate preferences. So these are a "number computed from
   them". Lines 699–701 already read the rule as "except the groups
   themselves". Put that wording in the spec, covering everything that follows
   from the groups.
8. **The definition of "on the list" is incomplete** (lines 123–124). It
   doesn't cover a linked account whose sign-in name has changed. Nor does it
   say which entry wins if an account linked to `x` is renamed to match an
   unlinked `y`.
9. **The acceptance criteria skip the hardest guarantees** (lines 794–808).
   Nothing tests that a large class can't be improved by a simple change, that
   results repeat, the 60-second limit, privacy, the CSV format or file name,
   or refusal after a close.
10. **"But at least 1"** (line 212) can never apply, since *k* ≤ *S*. It's the
    same kind of dead clause as round 3's item 4.

##### Still open from round 3

Every round-3 finding still stands, because the behavior text hasn't changed:

* "after the session closed" can be read as "ever after";
* refused saves retry forever;
* ~~an extra Unspecified teammate lowers the teammate part;~~
* the "fewer than *k*" clause can never apply;
* dragging vs. "goes to its end";
* how scores are compared (exact arithmetic);
* "Repeatable" has no version boundary;
* the 6 vague terms and the 9 carried over from round 2.

Some of these are already decided in the architecture and could move up into
the spec:

* "last change" (lines 563–564);
* case-only edits keep the latest casing (line 645);
* a stale save is refused and saved again (lines 518–519).

### Aruzhan Akhanayeva

Github userid: akhanayeva

Git userid: akhanayeva

#### Contributions

Added backend configuration and architecture

#### Build and run test

Build and run tests are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Specification review rounds

| Round | Findings                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Main Changes Made |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------- |
| 4     | Rated 8/10; 27 findings in total, 8 of them new; the other 19 are still open from Daniil's round 2, since the spec hasn't changed since then. <strong>High:</strong> not-preferred share has the wrong sign as written (taken literally, it puts together the two students who asked to be kept apart), CSV example needs 3 groups from 4 students, *m* = 0 divides by zero (a 1-student session reaches it with the default group size), matching on the email claim lets one person pass as another, a stale organizer tab can silently remove students and their preferences. <strong>Medium:</strong> closed-session save rejection loops on retry, a stale student tab restores preferences for a re-added entry, session-page edits have no limits or save rule, moves in a filtered list are undefined, reordering list entries is undefined, privacy wording contradicts itself, **Sign out** doesn't let users switch Microsoft accounts, no quotas on sessions or forming jobs, the 60-second limit names no machine, organizer identity is undefined, the ≤8-student test oracle is unbounded in projects, repeatability across tool versions and tie-breaking are unstated. <strong>Low:</strong> 10 vague terms ("last change", Unicode normalization and case folding, rounding of halves and "changes it", "letter" in file names, userid syntax, Rename collisions, title trimming, separators, propagation of Close/Reopen/Delete, "group size" not being the actual size). Expected results table, teammate-part table and default group sizes verified correct by brute force. | <br>              |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in Claude Code.

Prompt:

> Review a specification at @group-creation-tool/specification.md , add your
> findings into the @group-creation-tool/submission.md Specification Review
> Rounds under Aruzhan Akhanayeva. Leave the Main Changes Made part empty for
> now, just the findings and round number. Also, add the Last Specification
> Review part with your current review under Aruzhan Akhanayeva's name, just
> like Daniil Kaimekin did.

Output (line numbers refer to [specification.md](specification.md) as it was
reviewed, before this round's changes):

I'd rate it **8/10**. The spec hasn't changed since Daniil's round 2, so most of
those findings still stand. They're repeated below and marked <em>(open since
round 2)</em>, so this review stands on its own. Eight findings are new; they're
marked **New**. I wrote an independent brute-force checker using exact
fractions. All 13 Expected results rows give exactly the stated groups and
score, and each has exactly one best assignment. All 5 teammate-part table rows
give the stated value. The default group sizes for 4–7, 50 and 500 students
match the text. Those results hold only if the not-preferred share is read as
negative (item 1). Fixing the sign error, the impossible CSV example, the *m* =
0 case, the email-claim identity rule and the stale organizer tab would make it
a 9.

##### Logical inconsistencies

1. **The not-preferred share has the wrong sign as written** (lines 241–247).
   <em>(Open since round 2.)</em> "Worked out the same way" divides a negative
   sum by a negative maximum, which gives a positive share. Taken literally,
   table row 5 gives 4/3, not −2/3. In Expected results row 6, the best
   assignment becomes `X: a, b; Y: c, d` with a score of 6. That puts together
   exactly the two students who asked to be kept apart. Say "divided by the
   absolute value of the largest sum".
2. **The CSV example can't happen** (lines 349–358). <em>(Open since round
   2.)</em> 4 students give G = 4, 2, 1 or 1 groups for *k* = 1…4, never 3.
3. **A group with no other members divides by zero** (lines 200–202 and
   239–248). <em>(Open since round 2, with a new detail.)</em> The organizer
   doesn't even have to pick *k* = 1. A session may have 1 student (line 72).
   √1 = 1, and the "at least 2" rule applies only from 2 students. So the
   default gives *k* = 1 and *m* = 0, and the denominator is the sum of the
   first 0 values, which is 0. Say that the teammate part is 0 when *m* = 0.
4. **A save refused because the session closed collides with the retry rule**
   (lines 165–167 and 178–180). <em>(Open since round 2.)</em> The page would
   retry every 10 seconds forever and keep the leave-page warning on. It would
   also save after a Reopen, which the organizer didn't expect.
5. **A stale student tab can bring back preferences that should have been
   dropped** (lines 96–102 and 171–174). <em>(Open since round 2.)</em> Suppose
   a page was loaded before the organizer removed project `Foo` and added it
   again. Its next save attaches the old `Foo` ranking to the new entry, which
   was supposed to "start over".
6. **New. A stale organizer page can silently remove students** (lines 96–102
   and 403–406). List edits are decided by comparing the new text with the
   stored list. Suppose the organizer adds students in one tab and then saves an
   older copy of the list from another tab. Or suppose they edit while offline:
   the page "keeps what the organizer has entered". Either way, the students
   missing from that text count as removed. Their sign-in links and preferences
   are dropped, and adding them back makes them start over. Students get a
   last-save-wins rule (line 171); organizers get no rule at all. Either reject
   an edit made against an outdated list, or show the organizer what will be
   removed before saving.
7. **New. Edits on the session page have no limits or save rule** (lines 72–74
   and 96). Setup makes **Create** unavailable until the title and list limits
   hold, but the session page has no **Create**. Nothing says whether an edit
   that empties the student list, or brings it to 501 students, is refused.
   Nothing says when an edit takes effect, either: on a **Save** button, on
   leaving the field, or as the organizer types. The title also can't be edited
   after creation by any stated means, yet a typo in it ends up in the CSV file
   name.
8. **New. Reordering entries is undefined** (lines 70, 96–102, 109). List order
   decides the order of Unspecified items (line 145), group numbering (lines
   318–320) and the CSV row order (line 341). An edit that only moves entries is
   neither a case change nor a remove-and-add. Nothing says whether it's
   allowed, whether it discards formed groups ("editing … discards", line 109),
   or whether the lists still count as "the same lists" under Repeatable (line
   308).
9. **The privacy rule contradicts itself** (lines 184–191). <em>(Open since
   round 2.)</em> "No … figure computed from them" also forbids the groups,
   which the next paragraph admits are computed from teammate preferences.
10. **New. Moving items while the list is filtered is undefined** (lines
    150–156). Items "can be moved while the section is filtered", but "up or
    down one place" could mean past the next visible item, or past the next item
    in the full ranking, which may be hidden. Dropping an item between two
    visible items doesn't say where it lands among the hidden items between
    them. The readings give different rankings, and so different scores.

##### Vague terms

* <strong>"Last change"</strong> (line 33). <em>(Open since round 2.)</em> Does
  a student's save move the date shown on the sessions list (line 51)? Does the
  organizer just opening the session move it?
* **New. Unicode normalization and case folding** (lines 19–20 and 63). Lengths
  are counted in code points, but no normalization form is named. `é` is 1 code
  point in NFC and 2 in NFD, the form text pasted from macOS often arrives in.
  So two names that look identical are not duplicates, and a 200-character name
  can pass or fail depending on how it was typed. "Ignoring case" also doesn't
  say whether it means simple or full case folding (`ß` versus `SS`).
  Implementers should also be warned that HTML `maxlength` and JavaScript
  `.length` count UTF-16 code units, where an emoji counts as 2.
* **New. Repeatable** (lines 308–310). "On any computer" doesn't say "on any
  version of the tool". If the search is improved in a later release, forming an
  old session again gives different groups. Nothing says which of several
  equally good assignments is chosen, either. Ties are routine: a class where
  nobody saved preferences, or *W*<sub>p</sub> = *W*<sub>t</sub> = 0, which the
  options allow, makes every valid assignment best. Say whether tests may check
  the groups in such cases or only the score.
* **Number fields** (lines 200–204 and 281–284). <em>(Open since round 2.)</em>
  How do halves round? Does typing the default value count as "changing" the
  group size?
* <strong>"A letter" in the file name</strong> (line 334). <em>(Open since round
  2.)</em> Is it ASCII only, or Unicode letters, so that `Проект` stays
  `Проект`?
* **Userid syntax** (lines 66–67). <em>(Open since round 2.)</em> `a@b.` and
  `a@.edu` both pass.
* **Rename collisions** (lines 104–108). <em>(Open since round 2.)</em> What
  happens when an entry is renamed to one already on the list? How do the text
  field and the per-entry **Rename** action coexist? After a case-only edit,
  does the CSV use the old or the new casing?
* **Title trimming** (line 57). <em>(Open since round 2.)</em> A title made only
  of spaces looks valid.
* **Separators** (line 58). <em>(Open since round 2.)</em> Semicolons and tabs
  aren't separators, so a list pasted from Outlook is dropped entry by entry.
* **Propagation** (lines 293–297 and 409–410). <em>(Open since round 2.)</em>
  How soon does an open student page notice a Close, a Reopen, a deletion, or
  that the student was removed? What happens when the organizer edits the lists
  or options while groups are being formed?
* <strong>"Group size" isn't the size of a group</strong> (lines 198–208).
  <em>(Open since round 2.)</em> With 13 students and *k* = 7, there is one
  group of 13. The help text should call *k* the minimum size.

##### Architectural mismatch

* **Matching on the email claim lets one person pass as another** (lines
  122–128). <em>(Open since round 2.)</em> The `email` claim is set by the
  user's own organization and may be unverified. Since the first matching
  account keeps the userid for good, an impostor also locks the real student
  out. There's a further tension. Line 18 defines the userid as the sign-in
  name, but organizers will usually paste email addresses from Teams. At many
  universities these differ: the sign-in name is `s1234567@uni.edu` and the
  email is `jane.doe@uni.edu`. Dropping email matching to fix the impostor
  problem would break such lists. Accepting email only when its domain is
  verified (`xms_edov`), or only from allowed tenants, keeps both working.
* **New. Sign out doesn't let users switch accounts** (lines 129–131). With
  Microsoft single sign-on, signing out of the tool alone leaves the Microsoft
  session in place. The next sign-in silently returns the same account. To
  switch, the tool must either ask Microsoft for its account picker
  (`prompt=select_account`) or sign the user out of Microsoft entirely, which
  also signs them out of Teams and Outlook in that browser. The spec should say
  which.
* **New. No quotas** (lines 33, 39–40 and 407–408). Anyone with any work or
  school account, in any organization, can create sessions. Each can hold 500
  students and 100 projects, run a 60-second forming job, and be stored for 365
  days. There's no limit on sessions per organizer, forming jobs running at
  once, or tenants allowed, so one user can exhaust the server's CPU and
  storage. Set per-organizer limits, or restrict which tenants can sign in.
  Restricting tenants would also help with the email claim above.
* **Which machine the 60-second limit applies to** (lines 407–408). <em>(Open
  since round 2.)</em> Phones are supported browsers. If forming runs on the
  server, the spec must cover a page reload during forming and what **Cancel**
  does.
* **The organizer's identity is undefined** (lines 38–40). <em>(Open since round
  2.)</em> Students are bound to a stable account; the organizer isn't.
* **The ≤8-student test oracle is unbounded in projects** (lines 438–439).
  <em>(Open since round 2.)</em> 8 students, *k* = 1 and 100 projects give about
  7.5 × 10¹⁵ project assignments to check. Bound *P*.

##### What's strong

The scoring model is fully stated and normalized. With the sign fixed, it
reproduces every Expected results row exactly, and each row has a unique best
assignment, so the table works as a real test oracle. The sizing rules are
exact, and their examples (50 and 500 students, 7 students) check out. Rename
versus remove-and-add is cleanly separated. The CSV format is precise down to
line endings and quoting. The privacy section honestly admits what can't be
hidden, and the out-of-scope list is concrete.

### (Team member 3 name)

(Same sections as Team member 1)

### (Team member 4 name)

(Same sections as Team member 1)

Final submission
----------------

### (Team member 1 name)

GitHub userid: (team member 1's GitHub userid)

#### Contributions

(Team member 1's contributions)

#### Specification review rounds

(Table showing spec review rounds)

#### Last specification review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

#### Implementation review rounds

(Table showing implementation review rounds)

#### Last implementation review

(LLM and prompt used)

(LLM output, possibly with strikethrough)

### (Team member 2 name)

(Same sections as Team member 1)

### (Team member 3 name)

(Same sections as Team member 1)

### (Team member 4 name)

(Same sections as Team member 1)
