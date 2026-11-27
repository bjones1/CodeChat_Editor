Project 2 submission
====================

Team name: Project Group Creation Tool 2

The specification consists of the file [specification.md](specification.md).

The submission consists of the file [submission.md](submission.md).

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

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in the Claude Code tab of the Claude
desktop app.

Prompt:

> Review a specification at @group-creation-tool/specification.md .

Output (line numbers refer to [specification.md](specification.md) as it was
reviewed, before this round's changes; strikethrough marks findings judged
incorrect):

I'd rate it **9/10**, up from 8 in round 2. All the High and Medium findings
from round 2 are fixed, and the fixes don't contradict each other. I
brute-forced all 13 Expected results rows with exact fractions. Each one gives
exactly the stated groups and score, and each has exactly one best assignment.
All 5 rows of the teammate-part table come out as stated. The default sizes
for 4–9, 13, 50 and 500 students match the text. What keeps it from a 10 is
mostly the edges of the save rules and one hidden decision about how scores
are compared. I didn't modify the file.

##### Logical inconsistencies

1. **"After the session closed" can be read as "ever after"** (lines 184–187).
   Once a session has been closed even once, every later change reaches the
   tool "after the session closed". Read literally, that includes changes made
   after the session is reopened. It also doesn't say what happens to a change
   made before a Close that arrives only after the Reopen, for example after a
   network outage. A clearer wording: "a change that reaches the tool while the
   session is Closed is refused". The spec should also say whether a read-only
   page becomes editable again when the session is reopened.
2. **Saves refused for other reasons still retry forever** (lines 166–169).
   Round 2's fix covers only a Closed session. A save can also be refused
   because:
   * the session was deleted;
   * the student was removed from the list;
   * the student was removed and added again, so their entry is a new one and
     "*not signed in*".

   Each of these falls under "while a save is failing, the page retries every
   10 seconds". The page would retry forever and keep the leave-page warning
   on. In the third case, the rule on line 176 ("never applies to an entry that
   was added after the page last loaded") is about the entries a save mentions.
   It doesn't say whether a stale page can save for the student's own new
   entry.
3. **Unspecified isn't neutral for teammates** (lines 251–261). Unspecified is
   worth 0, but adding an Unspecified teammate can lower a student's score.
   Take a student with Preferred `b`, `c`, `d`:
   * in a group with `b` and `c`, their preferred share is
     (1 + 2/3)/(1 + 2/3) = **1**;
   * with an extra Unspecified member `x` it is (5/3)/2 = **5/6**.

   The same extra member softens a not-preferred penalty from −1 to −5/6. This
   only happens when the category holds more than *m* items. A student who
   lists just `b` gets 1 either way. So the scoring tends to put students with
   long lists into the smaller groups. If that's intended, say so; if not,
   scale the shares by the group's best possible sum.
4. **"Unless there are fewer than *k* students in all" can never apply** (line
   216). *k* is limited to 1…*S* (line 207) and is clamped when the list
   changes, so *S* ≥ *k* always holds.
5. **Dragging conflicts with "goes to its end"** (lines 152–156). An item moved
   into Preferred or Not preferred goes to the end, yet "every move can be made
   by dragging". Dragging normally drops an item where you let go. Say which
   one wins.

##### Vague terms

* **Moves while filtered** (lines 157–158). It isn't clear whether "up one
  place" moves past items the filter hides, or where a dragged item lands
  between two visible items that have hidden ones between them. It also
  doesn't say whether the filter ignores case.
* **List edits on the session page** (lines 97–111). Nothing says how an edit
  is applied: is there an Apply button, or is it saved on blur? Nor does it say
  what happens when an edit breaks the limits, such as 0 students or 501. At
  setup, **Create** is simply unavailable, but the session page has no
  equivalent.
* **Title after creation.** The spec doesn't say whether the title can be
  edited, and it decides the CSV file name.
* **"Out of date"** (lines 299–302). If an option is changed and then set back
  to the value used for forming, are the results still out of date?
* **"2 processor cores"** (line 427) still names no processor speed or
  generation, so the 60-second limit can't be tested exactly.
* **Page speed** (line 450): it isn't clear whether "shows its content within 2
  seconds" includes the Microsoft sign-in redirect, which the tool doesn't
  control.
* **Carried over from round 2**, left open on purpose: "last change", "changes
  it", rounding of halves, "letter" in file names, userid syntax and case
  folding, separators, title trimming, Rename collisions, and how fast Close,
  Reopen, Delete and removal reach open pages. Two of these now matter more.
  Reopening or editing during forming is undefined. And how a student page
  learns of a Reopen affects item 1.

##### Architectural mismatch

* **The spec doesn't say how scores are compared** (lines 316–327 and
  459–461). Scores are fractions such as 1/3 and 5/6. With floating-point
  numbers, two assignments that should tie can differ by a tiny rounding
  error. That can break three things:
  * "no valid assignment has a higher score";
  * "Not improvable" (a neutral swap looks like an improvement, and the search
    can loop);
  * the acceptance test's "equals the best score".

  The spec should require exact arithmetic or a stated tolerance. Exact
  arithmetic is easy here, because every value is *j*/*n* with *n* ≤ 500.
* **"Repeatable" has no version boundary** (line 325). As written, a later
  release with a better algorithm would break the rule. Limit it to "the same
  version of the tool", or record the version with the results.
* **Fixing mismatched sign-in names is slow.** At many universities the
  sign-in name differs from the email address the organizer knows, for
  example `s1234567@uni.edu` instead of `jane.doe@uni.edu`. Each such student
  must contact the organizer, who renames entries one at a time. A list of
  refused sign-ins on the session page, with a one-click Rename, would solve
  this without the email-matching risk from round 2.
* **There are no quotas.** Anyone with any work or school account can create
  any number of sessions of up to 500 × 100 entries, and each one is kept for
  365 days. The spec should cap sessions per organizer, or restrict sign-in to
  particular organizations.

##### What's strong

The scoring is fully stated and every example checks out exactly. The size
rules are tight and the "fewer than 2*k*" claim holds. The rules for stale tabs
and closed sessions are now close to complete. Forming on the server, with
Cancel and reload behavior, is well defined. The privacy section is honest
about what it can't hide. Fixing items 1 and 2 and stating exact arithmetic
would make it a 10.

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
| 1     | Rated 8/10; 27 findings in total, 8 of them new; the other 19 are still open from Daniil's round 2, since the spec hasn't changed since then. <strong>High:</strong> not-preferred share has the wrong sign as written (taken literally, it puts together the two students who asked to be kept apart), CSV example needs 3 groups from 4 students, *m* = 0 divides by zero (a 1-student session reaches it with the default group size), matching on the email claim lets one person pass as another, a stale organizer tab can silently remove students and their preferences. <strong>Medium:</strong> closed-session save rejection loops on retry, a stale student tab restores preferences for a re-added entry, session-page edits have no limits or save rule, moves in a filtered list are undefined, reordering list entries is undefined, privacy wording contradicts itself, **Sign out** doesn't let users switch Microsoft accounts, no quotas on sessions or forming jobs, the 60-second limit names no machine, organizer identity is undefined, the ≤8-student test oracle is unbounded in projects, repeatability across tool versions and tie-breaking are unstated. <strong>Low:</strong> 10 vague terms ("last change", Unicode normalization and case folding, rounding of halves and "changes it", "letter" in file names, userid syntax, Rename collisions, title trimming, separators, propagation of Close/Reopen/Delete, "group size" not being the actual size). Expected results table, teammate-part table and default group sizes verified correct by brute force. | <br>              |

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
