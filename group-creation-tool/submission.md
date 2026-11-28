Project 2 submission
====================

Team name: Project Group Creation Tool 2

Team members: Daniil Kaimekin, Aruzhan Akhanayeva, Ademi Bakhytova, Alisher Akim.

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
| 6 | Rated 9/10; 7 new findings in total, plus the Low findings left open in round 5. **High:** the architecture's search, which improves until no simple change helps, can stop short of the best assignment for 8 students or fewer (a 4-student, 3-project counterexample stops at a score of 7 where the best is 22/3). **Medium:** a single-tenant Entra app still lets B2B guest accounts sign in, against "accounts of other organizations cannot sign in"; a save refused for any reason other than the three checked ones is saved again forever, and a close-then-reopen race saves a change the spec says is dropped; the 60-second limit names no input preferences and the search has no bound on its steps. **Low:** "the same lists" under Repeatable vs. a new entry id after a remove and re-add, the 10-second retry and **Retry** don't match how the Firestore SDK resends writes, the filter doesn't say whether it ignores case or what "name" means for teammates. Expected results table, teammate-part table and default group sizes verified correct by brute force with exact fractions. | |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in the Claude Code tab of the Claude
desktop app.

Prompt:

> Review a specification at @group-creation-tool/specification.md .

Output (line numbers refer to [specification.md](specification.md) and
[architecture.md](architecture.md) as they were reviewed, before this round's
changes):

I'd rate the spec **9/10**, up from 7 in round 5. The pasted architecture is
gone, and every High and Medium finding from round 5 has been fixed in the
text. The most serious problem left is in `architecture.md`: its search can't
keep the spec's "best for small classes" promise. I didn't modify any files.

I tested all 13 Expected results rows against every valid assignment, using
exact fractions. Each one gives exactly the stated groups and score, and each
has exactly one best assignment. All 5 teammate-part rows are correct. The
group sizes in the text are right for 50, 500, 13 (with *k* = 7), 7, and 1–6
students.

##### Spec and architecture disagree

1. **The search can stop short of the best answer for small classes** (spec
   lines 354–355; architecture lines 170–174). The design starts from one
   assignment and makes simple changes until none helps. That reaches a point
   no simple change can improve, but not always the best assignment. Here is a
   counterexample I verified:
   * 4 students and projects `X`, `Y`, `Z`.
   * a PP Z, Y, PT b; b PP Z, Y, PT a.
   * c PP Z, X, Y, PT d; d PP Z, X, Y, PT c.
   * **Y: a, b; Z: c, d** scores 7, and every simple change from it lowers the
     score. The best of them is 20/3.
   * The best assignment is **Z: a, b; X: c, d**, at 22/3. Reaching it means
     changing both groups' projects at once.

   For 8 students or fewer, the architecture needs an exact method. There are
   at most 105 ways to split the students into groups. For each split,
   choosing the projects is small:
   * When *P* ≤ *G*, every allowed spread of projects can be tried.
   * When *P* > *G*, each project gets at most one group, so it's an
     assignment problem (for example, the Hungarian algorithm). This covers up
     to 100 projects.

   Local search alone should be used only above 8 students.
2. **Guest accounts can sign in** (spec lines 19–21 and 457–458; architecture
   lines 56–60 and 74–75). A single-tenant Entra app accepts B2B guests.
   Guests are common in Teams, and their tokens carry the university's tenant
   id, so the `tid` check lets them through. The spec says accounts of other
   organizations can't sign in. Decide in the spec whether guests are allowed.
   If not, have `signIn` reject them, for example through the `acct` optional
   claim, or an `idp` claim that differs from the issuer.
3. **A refused save can loop** (spec line 177; architecture lines 112–120).
   Step 4 assumes any other refusal means an old `listVersion`, and saves
   again. A refusal for any other reason repeats forever, against "a save the
   tool refuses is never retried". Examples: a duplicate id, or an id the
   rules reject because of a bug.
   * There's also a race. If the session closes and reopens between the save
     and the check, step 4 saves the change, which the spec says must be
     dropped (lines 197–199).
   * Fix: save again only if the roster's `listVersion` differs from the one
     that was sent. Otherwise show *Not saved*.

##### New spec findings

4. **The 60-second limit has no test input** (lines 465–468). "500 students
   and 100 projects" doesn't say what preferences to use. Improving until no
   simple change helps has no bound on how many steps it takes, so the limit
   can't be checked as written.
   * Name a benchmark, for example random preferences in which every student
     ranks a set number of projects and teammates.
   * Say what happens when forming runs long: it still finishes, it only
     misses the target.
5. **"Repeatable" depends on what "same lists" means** (lines 361–365).
   Remove a student who never saved anything and add them back. The lists and
   preferences read the same, but the entry has a new random id. If the
   architecture's seed (line 171) includes entry ids, the groups change.
   * Either say the seed comes from the list text and order, or define "same
     lists" as the same entries.
6. **The 10-second retry doesn't fit the Firestore SDK** (lines 175–177). The
   SDK queues writes and sends them on its own schedule when it reconnects. A
   write it can't send never fails, so the page has no event for *Not saved*.
   * The architecture should say how the page detects that the tool can't be
     reached and how **Retry** forces a resend. Or the spec should describe
     only what users see, such as "saved as soon as the connection returns".
7. **The filter isn't fully defined** (lines 164–165). It doesn't say whether
   matching ignores case, or whether "name" means the display name for
   teammates.

##### Still open (Low, left as is in round 5)

* "On the list" doesn't say which entry wins when a linked account's new
  sign-in name matches another, unlinked entry.
* The acceptance criteria don't test:
  * that a simple change can't improve the result;
  * that results repeat;
  * the 60-second limit;
  * privacy;
  * the CSV format;
  * that saves are refused after a close.
* Three clauses can never apply:
  * "but at least 1" (line 235);
  * "unless there are fewer than *k* students in all" (lines 239–240);
  * "halves rounded up" (line 231), since √*S* is never a half. It does no
    harm.
* Dragging can contradict "goes to its end", and some vague terms remain:
  * "last change";
  * whether typing the default *k* counts as changing it;
  * "letter" in file names;
  * title trimming and editing;
  * separators;
  * Rename collisions;
  * whether the 1–500 and 1–100 limits apply to edits on the session page;
  * how fast Close, Reopen and Delete reach open pages.

##### What's strong

* Each of the three save refusals now has its own message, and the text says
  plainly that a change refused because the session closed is dropped.
* The privacy section is now consistent with itself. It states openly what it
  can't hide: the groups themselves, and who has signed in.
* Forming during edits is closed off in both documents, with the check in the
  same transaction as the edit.
* Exact fraction scoring and the version limit on Repeatable remove the tie and
  drift problems from round 3.

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

### Ademi Bakhytova

Github user id: adeemss

Git user id: adeemss

#### Contributions



#### Build and run test

Build and run tests are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Specification review rounds

| Round | Findings | Main Changes Made |
| --- | --- | --- |
| 7 | Rated 9/10; 14 findings in total, several of them Low findings carried over from earlier rounds. **Medium:** exact `BigInt` fractions put the 60-second limit at risk (a pass over ~125,000 swaps, each changing ~46 students' satisfaction, with denominators of hundreds of bits), the forming-time limit names no *k* or preference density (*k* = 250 is far costlier than the default), Rename has no collision or validity rules, "letter" in the CSV file name is undefined for Cyrillic or Kazakh titles, lists pasted from Outlook or Teams (`Name <a@uni.edu>; …`) are mostly dropped, "last change" for automatic deletion is undefined in the spec. **Low:** the dead "fewer than *k* students in all" clause, loose userid syntax (`a@.edu`, `a@b.`) and no warning for other organizations' domains, rounding of halves in option fields, both weights at 0 leave the groups to the tie-break, "until the organizer changes it" for typing the default value, WCAG 2.1 instead of 2.2, the 2-second page limit vs. a cold-started `signIn` function, no way for a student to withdraw saved preferences. Expected results table, teammate-part table and group-size examples (50, 500, 13 and 7 students) verified correct by hand, each with one best assignment. | |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in Claude Code.

Prompt:

> review specification at @specification.md

Output (line numbers refer to [specification.md](specification.md) and
[architecture.md](architecture.md) as they were reviewed, before this round's
changes):

I reviewed `group-creation-tool/specification.md` and checked it against
`architecture.md`. It's in good shape. I recomputed every row of Expected
results, every row of the teammate-part table, and the group-size examples (50,
500, 13 and 7 students) by hand. All the stated groups and scores are correct,
and each listed best assignment is the only one with that score. Nothing I
found is a contradiction; the points below are gaps, ambiguities and one
feasibility risk.

##### Worth fixing

1. **Forming time vs. exact fractions** (spec lines 300–304 and 465–468). This
   is the biggest risk. Each student's share has its own denominator, up to
   about 250,000. Adding them up over 500 students makes the common
   denominator very large, often hundreds of bits. Each swap changes the
   satisfaction of about 46 students. A full pass over about 125,000 swap
   pairs using `BigInt` fractions could take tens of seconds, and the search
   needs many passes. `architecture.md` already lists this as an open decision
   (lines 375–378). Its fallback (compare with floating point first, and use
   exact fractions only for close calls) keeps the spec's wording valid, so
   build a 500×100 prototype before treating the 60-second target as met.
2. **The forming-time requirement doesn't say which inputs it covers** (lines
   465–468). It only says "500 students and 100 projects". Run time depends
   heavily on *k* and on how much students rank. *k* = 250 gives two groups of
   250, so *m* = 249 and every move costs far more than with the default *k* =
   22. Students ranking all 499 classmates is also worse than sparse
   preferences. Name the case: say whether it means any *k*, and every student
   ranking every item.
3. **Rename has no collision rules** (lines 108–115). It doesn't say what
   happens when a project or userid is renamed to one already on the list,
   ignoring case. It also doesn't say whether the userid rule and the
   200-character limit still apply. Setup drops duplicates, but Rename has no
   rule. Suggest: "Rename applies the same rules as setup and is refused when
   it would produce a duplicate or invalid entry."
4. **"Letter" in the CSV file name is undefined** (lines 388–390). It's unclear
   whether it means ASCII letters or Unicode letters. A Cyrillic or Kazakh
   title like `Топтар` would give either `Топтар-groups.csv` or
   `______-groups.csv`. Lengths are counted in Unicode code points elsewhere,
   so pick one explicitly.
5. **Pasting the student list** (lines 62 and 70–71). Lists copied from Outlook
   or Teams look like `Name <a@uni.edu>; Name2 <b@uni.edu>`. Under the current
   rules that gives a pile of dropped entries. Consider allowing `;` as a
   separator and taking the address from `<…>`. Otherwise, add this format to
   Out of scope so it's a deliberate choice.
6. **"Last change" for automatic deletion is undefined** (lines 36 and 55). The
   sessions list shows a deletion date, so users need to know what resets it.
   `architecture.md` decides this (lines 182–184: any organizer action, or a
   student save), but the spec should say it, including whether a student
   signing in without saving counts.

##### Minor

7. **A dead clause in Group sizes** (lines 239–240). "Unless there are fewer
   than *k* students in all" can never apply, because *k* is limited to 1–*S*.
   The sentence is also hard to parse. Suggest: "So every group has at least
   *k* and fewer than 2*k* students."
8. **The userid check is loose** (lines 70–71). `a@.edu` and `a@b.` both pass
   ("a domain that contains at least one `.`"). Also, an entry with another
   organization's domain is accepted but can never sign in. A warning for
   entries outside the organization's domain would catch typos early.
9. **Rounding halves in the options fields** (lines 324–327) isn't specified
   ("a fraction is rounded to the nearest whole number"). Say which way 2.5
   goes, as Group sizes already does.
10. **Both weights set to 0** (lines 321–322) makes every assignment score 0,
    so the groups come only from the tie-break. A short help-text warning
    would avoid confusion.
11. **"Until the organizer changes it"** (lines 230–234, default *k*) doesn't
    say what happens if the organizer types the current default value: does
    *k* still follow the list afterwards? `architecture.md` uses `null` for
    "still the default" (line 230), so define a reset, such as clearing the
    field.
12. **Accessibility** (lines 484–487): consider WCAG 2.2 AA instead of 2.1. It
    adds 2.5.7 (dragging movements), which the button and key moves already
    meet, and 2.5.8 (target size).
13. **Speed vs. sign-in** (lines 489–490): the 2-second page requirement
    probably doesn't hold on a cold start of the `signIn` Cloud Function.
    Either exclude sign-in from the requirement or plan for keeping one
    instance warm.
14. **A student can't withdraw their preferences** (lines 189 and 204–210)
    after saving; they can only move everything back to Unspecified, and the
    status stays *saved preferences*. That may be fine, but state it given the
    Privacy section.

##### Checked and fine

* The privacy model, and the inference it openly accepts from the groups.
* The three save-refusal cases.
* Forming being blocked while edits are possible, and vice versa.
* The CSV example: numbering and quoting are correct.
* The Rename vs. remove-and-add behavior.
* Results being marked out of date when options change.

### Alisher Akim

Couldn't contact them, unfortunately.

Final submission
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
| 6 | Rated 9/10; 7 new findings in total, plus the Low findings left open in round 5. **High:** the architecture's search, which improves until no simple change helps, can stop short of the best assignment for 8 students or fewer (a 4-student, 3-project counterexample stops at a score of 7 where the best is 22/3). **Medium:** a single-tenant Entra app still lets B2B guest accounts sign in, against "accounts of other organizations cannot sign in"; a save refused for any reason other than the three checked ones is saved again forever, and a close-then-reopen race saves a change the spec says is dropped; the 60-second limit names no input preferences and the search has no bound on its steps. **Low:** "the same lists" under Repeatable vs. a new entry id after a remove and re-add, the 10-second retry and **Retry** don't match how the Firestore SDK resends writes, the filter doesn't say whether it ignores case or what "name" means for teammates. Expected results table, teammate-part table and default group sizes verified correct by brute force with exact fractions. | |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in the Claude Code tab of the Claude
desktop app.

Prompt:

> Review a specification at @group-creation-tool/specification.md .

Output (line numbers refer to [specification.md](specification.md) and
[architecture.md](architecture.md) as they were reviewed, before this round's
changes):

I'd rate the spec **9/10**, up from 7 in round 5. The pasted architecture is
gone, and every High and Medium finding from round 5 has been fixed in the
text. The most serious problem left is in `architecture.md`: its search can't
keep the spec's "best for small classes" promise. I didn't modify any files.

I tested all 13 Expected results rows against every valid assignment, using
exact fractions. Each one gives exactly the stated groups and score, and each
has exactly one best assignment. All 5 teammate-part rows are correct. The
group sizes in the text are right for 50, 500, 13 (with *k* = 7), 7, and 1–6
students.

##### Spec and architecture disagree

1. **The search can stop short of the best answer for small classes** (spec
   lines 354–355; architecture lines 170–174). The design starts from one
   assignment and makes simple changes until none helps. That reaches a point
   no simple change can improve, but not always the best assignment. Here is a
   counterexample I verified:
   * 4 students and projects `X`, `Y`, `Z`.
   * a PP Z, Y, PT b; b PP Z, Y, PT a.
   * c PP Z, X, Y, PT d; d PP Z, X, Y, PT c.
   * **Y: a, b; Z: c, d** scores 7, and every simple change from it lowers the
     score. The best of them is 20/3.
   * The best assignment is **Z: a, b; X: c, d**, at 22/3. Reaching it means
     changing both groups' projects at once.

   For 8 students or fewer, the architecture needs an exact method. There are
   at most 105 ways to split the students into groups. For each split,
   choosing the projects is small:
   * When *P* ≤ *G*, every allowed spread of projects can be tried.
   * When *P* > *G*, each project gets at most one group, so it's an
     assignment problem (for example, the Hungarian algorithm). This covers up
     to 100 projects.

   Local search alone should be used only above 8 students.
2. **Guest accounts can sign in** (spec lines 19–21 and 457–458; architecture
   lines 56–60 and 74–75). A single-tenant Entra app accepts B2B guests.
   Guests are common in Teams, and their tokens carry the university's tenant
   id, so the `tid` check lets them through. The spec says accounts of other
   organizations can't sign in. Decide in the spec whether guests are allowed.
   If not, have `signIn` reject them, for example through the `acct` optional
   claim, or an `idp` claim that differs from the issuer.
3. **A refused save can loop** (spec line 177; architecture lines 112–120).
   Step 4 assumes any other refusal means an old `listVersion`, and saves
   again. A refusal for any other reason repeats forever, against "a save the
   tool refuses is never retried". Examples: a duplicate id, or an id the
   rules reject because of a bug.
   * There's also a race. If the session closes and reopens between the save
     and the check, step 4 saves the change, which the spec says must be
     dropped (lines 197–199).
   * Fix: save again only if the roster's `listVersion` differs from the one
     that was sent. Otherwise show *Not saved*.

##### New spec findings

4. **The 60-second limit has no test input** (lines 465–468). "500 students
   and 100 projects" doesn't say what preferences to use. Improving until no
   simple change helps has no bound on how many steps it takes, so the limit
   can't be checked as written.
   * Name a benchmark, for example random preferences in which every student
     ranks a set number of projects and teammates.
   * Say what happens when forming runs long: it still finishes, it only
     misses the target.
5. **"Repeatable" depends on what "same lists" means** (lines 361–365).
   Remove a student who never saved anything and add them back. The lists and
   preferences read the same, but the entry has a new random id. If the
   architecture's seed (line 171) includes entry ids, the groups change.
   * Either say the seed comes from the list text and order, or define "same
     lists" as the same entries.
6. **The 10-second retry doesn't fit the Firestore SDK** (lines 175–177). The
   SDK queues writes and sends them on its own schedule when it reconnects. A
   write it can't send never fails, so the page has no event for *Not saved*.
   * The architecture should say how the page detects that the tool can't be
     reached and how **Retry** forces a resend. Or the spec should describe
     only what users see, such as "saved as soon as the connection returns".
7. **The filter isn't fully defined** (lines 164–165). It doesn't say whether
   matching ignores case, or whether "name" means the display name for
   teammates.

##### Still open (Low, left as is in round 5)

* "On the list" doesn't say which entry wins when a linked account's new
  sign-in name matches another, unlinked entry.
* The acceptance criteria don't test:
  * that a simple change can't improve the result;
  * that results repeat;
  * the 60-second limit;
  * privacy;
  * the CSV format;
  * that saves are refused after a close.
* Three clauses can never apply:
  * "but at least 1" (line 235);
  * "unless there are fewer than *k* students in all" (lines 239–240);
  * "halves rounded up" (line 231), since √*S* is never a half. It does no
    harm.
* Dragging can contradict "goes to its end", and some vague terms remain:
  * "last change";
  * whether typing the default *k* counts as changing it;
  * "letter" in file names;
  * title trimming and editing;
  * separators;
  * Rename collisions;
  * whether the 1–500 and 1–100 limits apply to edits on the session page;
  * how fast Close, Reopen and Delete reach open pages.

##### What's strong

* Each of the three save refusals now has its own message, and the text says
  plainly that a change refused because the session closed is dropped.
* The privacy section is now consistent with itself. It states openly what it
  can't hide: the groups themselves, and who has signed in.
* Forming during edits is closed off in both documents, with the check in the
  same transaction as the edit.
* Exact fraction scoring and the version limit on Repeatable remove the tie and
  drift problems from round 3.

#### Implementation review rounds

Implementation rounds are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Last implementation review

Implementation rounds are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

### Aruzhan Akhanayeva

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

#### Implementation review rounds

Implementation rounds are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Last implementation review

Implementation rounds are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

### Ademi Bakhytova

Github user id: adeemss

Git user id: adeemss

#### Contributions



#### Build and run test

Build and run tests are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Specification review rounds

| Round | Findings | Main Changes Made |
| --- | --- | --- |
| 7 | Rated 9/10; 14 findings in total, several of them Low findings carried over from earlier rounds. **Medium:** exact `BigInt` fractions put the 60-second limit at risk (a pass over ~125,000 swaps, each changing ~46 students' satisfaction, with denominators of hundreds of bits), the forming-time limit names no *k* or preference density (*k* = 250 is far costlier than the default), Rename has no collision or validity rules, "letter" in the CSV file name is undefined for Cyrillic or Kazakh titles, lists pasted from Outlook or Teams (`Name <a@uni.edu>; …`) are mostly dropped, "last change" for automatic deletion is undefined in the spec. **Low:** the dead "fewer than *k* students in all" clause, loose userid syntax (`a@.edu`, `a@b.`) and no warning for other organizations' domains, rounding of halves in option fields, both weights at 0 leave the groups to the tie-break, "until the organizer changes it" for typing the default value, WCAG 2.1 instead of 2.2, the 2-second page limit vs. a cold-started `signIn` function, no way for a student to withdraw saved preferences. Expected results table, teammate-part table and group-size examples (50, 500, 13 and 7 students) verified correct by hand, each with one best assignment. | |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in Claude Code.

Prompt:

> review specification at @specification.md

Output (line numbers refer to [specification.md](specification.md) and
[architecture.md](architecture.md) as they were reviewed, before this round's
changes):

I reviewed `group-creation-tool/specification.md` and checked it against
`architecture.md`. It's in good shape. I recomputed every row of Expected
results, every row of the teammate-part table, and the group-size examples (50,
500, 13 and 7 students) by hand. All the stated groups and scores are correct,
and each listed best assignment is the only one with that score. Nothing I
found is a contradiction; the points below are gaps, ambiguities and one
feasibility risk.

##### Worth fixing

1. **Forming time vs. exact fractions** (spec lines 300–304 and 465–468). This
   is the biggest risk. Each student's share has its own denominator, up to
   about 250,000. Adding them up over 500 students makes the common
   denominator very large, often hundreds of bits. Each swap changes the
   satisfaction of about 46 students. A full pass over about 125,000 swap
   pairs using `BigInt` fractions could take tens of seconds, and the search
   needs many passes. `architecture.md` already lists this as an open decision
   (lines 375–378). Its fallback (compare with floating point first, and use
   exact fractions only for close calls) keeps the spec's wording valid, so
   build a 500×100 prototype before treating the 60-second target as met.
2. **The forming-time requirement doesn't say which inputs it covers** (lines
   465–468). It only says "500 students and 100 projects". Run time depends
   heavily on *k* and on how much students rank. *k* = 250 gives two groups of
   250, so *m* = 249 and every move costs far more than with the default *k* =
   22. Students ranking all 499 classmates is also worse than sparse
   preferences. Name the case: say whether it means any *k*, and every student
   ranking every item.
3. **Rename has no collision rules** (lines 108–115). It doesn't say what
   happens when a project or userid is renamed to one already on the list,
   ignoring case. It also doesn't say whether the userid rule and the
   200-character limit still apply. Setup drops duplicates, but Rename has no
   rule. Suggest: "Rename applies the same rules as setup and is refused when
   it would produce a duplicate or invalid entry."
4. **"Letter" in the CSV file name is undefined** (lines 388–390). It's unclear
   whether it means ASCII letters or Unicode letters. A Cyrillic or Kazakh
   title like `Топтар` would give either `Топтар-groups.csv` or
   `______-groups.csv`. Lengths are counted in Unicode code points elsewhere,
   so pick one explicitly.
5. **Pasting the student list** (lines 62 and 70–71). Lists copied from Outlook
   or Teams look like `Name <a@uni.edu>; Name2 <b@uni.edu>`. Under the current
   rules that gives a pile of dropped entries. Consider allowing `;` as a
   separator and taking the address from `<…>`. Otherwise, add this format to
   Out of scope so it's a deliberate choice.
6. **"Last change" for automatic deletion is undefined** (lines 36 and 55). The
   sessions list shows a deletion date, so users need to know what resets it.
   `architecture.md` decides this (lines 182–184: any organizer action, or a
   student save), but the spec should say it, including whether a student
   signing in without saving counts.

##### Minor

7. **A dead clause in Group sizes** (lines 239–240). "Unless there are fewer
   than *k* students in all" can never apply, because *k* is limited to 1–*S*.
   The sentence is also hard to parse. Suggest: "So every group has at least
   *k* and fewer than 2*k* students."
8. **The userid check is loose** (lines 70–71). `a@.edu` and `a@b.` both pass
   ("a domain that contains at least one `.`"). Also, an entry with another
   organization's domain is accepted but can never sign in. A warning for
   entries outside the organization's domain would catch typos early.
9. **Rounding halves in the options fields** (lines 324–327) isn't specified
   ("a fraction is rounded to the nearest whole number"). Say which way 2.5
   goes, as Group sizes already does.
10. **Both weights set to 0** (lines 321–322) makes every assignment score 0,
    so the groups come only from the tie-break. A short help-text warning
    would avoid confusion.
11. **"Until the organizer changes it"** (lines 230–234, default *k*) doesn't
    say what happens if the organizer types the current default value: does
    *k* still follow the list afterwards? `architecture.md` uses `null` for
    "still the default" (line 230), so define a reset, such as clearing the
    field.
12. **Accessibility** (lines 484–487): consider WCAG 2.2 AA instead of 2.1. It
    adds 2.5.7 (dragging movements), which the button and key moves already
    meet, and 2.5.8 (target size).
13. **Speed vs. sign-in** (lines 489–490): the 2-second page requirement
    probably doesn't hold on a cold start of the `signIn` Cloud Function.
    Either exclude sign-in from the requirement or plan for keeping one
    instance warm.
14. **A student can't withdraw their preferences** (lines 189 and 204–210)
    after saving; they can only move everything back to Unspecified, and the
    status stays *saved preferences*. That may be fine, but state it given the
    Privacy section.

##### Checked and fine

* The privacy model, and the inference it openly accepts from the groups.
* The three save-refusal cases.
* Forming being blocked while edits are possible, and vice versa.
* The CSV example: numbering and quoting are correct.
* The Rename vs. remove-and-add behavior.
* Results being marked out of date when options change.

#### Implementation review rounds

Implementation rounds are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

#### Last implementation review

Implementation rounds are not included, as current project stage only required the
specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor)
is not directly related to the project, and only used for pull request tracking.

### Alisher Akim

Couldn't contact them, unfortunately.
