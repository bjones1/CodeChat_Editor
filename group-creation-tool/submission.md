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

| Round | Findings                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Main Changes Made                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | Rated 7/10; 25 findings in total. <strong>High:</strong> teammate scoring contradicts the "no extra weight" claim, Unspecified ordering rules conflict, teammate privacy can't be guaranteed, default group count gives solo groups when projects outnumber students, userid relies on a mutable sign-in claim. <strong>Medium:</strong> deleted-session message vs. deleting all data, two-tab save rule, repeatability vs. time limit, stale results after option changes. <strong>Low:</strong> 7 vague terms and minor gaps (over-long names, CSV, retention, terminology). Expected results table verified correct.                                                                                                                                                                                                                                                                                                                        | Fixed every finding except the 2 struck through below and the tie-break part of item 9, plus 2 findings from CodeRabbit's review of the pull request. Group size is now about √S (an organizer option, at least 2), projects are spread evenly and a project can have several groups, and the teammate part of satisfaction is scaled to −1…1. Results no longer show the score or teammate counts, and the privacy promise now covers only showing teammate preferences. Unspecified always keeps list order. Added Rename, filter fields, and move to top or bottom. Students are matched by sign-in name or email, then by account. Added automatic save retries, last-save-wins across tabs, out-of-date results after option changes, 365-day retention, and testable timings and browser versions. Added Expected results rows and a teammate-part table, all rechecked by brute force. |
| 2     | Rated 8/10; 20 findings in total. <strong>High:</strong> not-preferred share has the wrong sign as written, CSV example needs 3 groups from 4 students (impossible), group size 1 divides by zero, matching on the email claim lets one person pass as another and lock out the real student. <strong>Medium:</strong> closed-session save rejection loops on retry, a stale tab can restore preferences for a removed and re-added entry, privacy wording contradicts itself, "group size" isn't the actual size, organizer identity is undefined, the 60-second limit names no machine, the ≤8-student test oracle is unbounded in projects. <strong>Low:</strong> 9 vague terms ("last change", "changes it", rounding of halves, "letter" in file names, userid syntax, separators, title trimming, Rename collisions, propagation of Close/Reopen/Delete). Expected results table and default group sizes verified correct by brute force. | <br>                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in the Claude Code tab of the Claude
desktop app.

Prompt:

> Review a specification at @group-creation-tool/specification.md . Rate it from
> 1 to 10. Do not modify the file. Find logical inconsistencies, vague terms,
> architectural mismatch.

Output (line numbers refer to [specification.md](specification.md) as it was
reviewed, before this round's changes; strikethrough marks findings judged
incorrect):

I'd rate it **8/10**, up from 7 in round 1. Most of round 1's findings are fixed
and the scoring model is now tight. I brute-forced all 13 Expected results rows.
Each one gives exactly the stated groups and score, and each has exactly one
best assignment. The default group sizes for 4–7, 50 and 500 students also match
the text. Three things keep it from a 9: a sign error in the teammate formula, a
CSV example that the spec's own rules can't produce, and an identity rule that
lets one person pass as another.

##### Logical inconsistencies

1. **The not-preferred share has the wrong sign as written** (lines 241–247).
   The share is "the sum of the values … divided by the largest sum", and the
   not-preferred share is "worked out the same way". For not-preferred teammates
   both of those sums are negative, so dividing one by the other gives a
   positive number. Taken literally, table row 4 gives <strong>+2/3</strong>
   instead of −2/3, and row 5 gives **4/3** instead of −2/3. The text needs
   "divided by the absolute value of" (or "by the size of") the largest sum. The
   table and the Expected results only come out right with that reading.
2. **The CSV example can't happen** (lines 349–358). It shows 4 students in 3
   groups. But G = ⌊4/*k*⌋ gives 4, 2, 1, 1 for *k* = 1…4, so 3 groups is
   impossible with 4 students. Use 5 or 6 students, or 2 groups.
3. **Group size 1 divides by zero** (lines 200 and 239–248). *k* can be 1, which
   leaves *m* = 0 other members. The denominator is then "the sum of the first 0
   values", which is 0. "An empty category's share is 0" covers an empty
   category, not a group with no other members.
4. **A save rejected because the session closed collides with the retry rule**
   (lines 165–167 and 178–180). "While a save is failing, the page retries every
   10 seconds." A save refused because the session is Closed counts as failing,
   so the page would retry forever, keep the leave-page warning on, and could
   succeed later if the organizer reopens. Say that a closed-session refusal is
   final and the page switches to read-only.
5. **A stale tab can bring back preferences that should have been dropped**
   (lines 96–102, 171–174 and 409–412). "Each save replaces all of the student's
   saved preferences." Suppose the organizer removes project `Foo` and adds it
   again; it is now a new entry that "starts over". A student page loaded before
   that edit still ranks the old `Foo`. Its next save can attach that ranking to
   the new `Foo` unless saves are checked against the current list version.
6. **The privacy rule contradicts itself** (lines 184–191). "No page or file
   shows … any figure computed from them", yet the groups are computed from
   teammate preferences and are shown. The next paragraph admits this. Say "no
   figure other than the groups themselves".
7. <strong>The "group size" isn't the size of a group.</strong> With 13 students
   and *k* = 7, G = ⌊13/7⌋ = 1, so the organizer gets one group of 13. The real
   size ranges from *k* up to almost 2*k*−1. The help text should say *k* is the
   minimum size, or the control should be renamed.

##### Vague terms

* <strong>"Last change"</strong> (line 33): deletion comes 365 days after the
  last change. Does a student's save, a sign-in, or an organizer just viewing
  the session count?
* <strong>"Until the organizer changes it"</strong> (line 202): does typing the
  default value into the field count as a change?
* **Number fields** (lines 282–284): "rounded to the nearest whole number"
  doesn't say how halves round. Line 201 does say it for √*S*.
* **File name** (line 334): does "a letter" mean ASCII letters only, or Unicode
  letters? That decides whether `Проект` keeps its letters or becomes `______`.
* **Userid syntax** (lines 66–67): `a@b.` and `a@.edu` both pass ("contains at
  least one `.`"). The case-insensitive comparison (line 19) also doesn't say
  whether it uses full Unicode case folding.
* **Separators** (line 58): semicolons and tabs aren't separators. A list pasted
  from Outlook (`Jane Doe <jane@uni.edu>;`) is dropped entry by entry.
* **Title** (line 57): trimming is stated only for list entries, so a title made
  only of spaces seems valid.
* **Rename collisions** (lines 104–108): what happens when a project or userid
  is renamed to one already on the list, ignoring case? Also, the lists are
  edited "as text", but Rename is a per-entry action; how the textarea and the
  status table coexist isn't described. After a case-only edit, it's unclear
  whether the CSV's userid "as entered" (line 342) uses the new or the original
  casing. If one account's sign-in name matches one entry and its email matches
  another (lines 122–128), which userid gets linked?
* <strong>Propagation of Close, Reopen, Delete and removal:</strong> list edits
  reach open student pages "within a minute" (line 410), but nothing says how
  fast a student page notices a Close, a Reopen, a deletion, or that the student
  was removed. Nor does the spec say what happens if the organizer reopens the
  session, edits the lists or changes the options while groups are being formed
  (lines 293–297).

##### Architectural mismatch

* **Matching on the email claim lets one person pass as another** (lines 122–124
  and 398). Anyone with any work or school account can sign in, and the spec
  matches students by their account's email address. In Microsoft Entra the
  `email` claim is set by the user's own organization's admin and may be
  unverified. Someone controlling their own tenant can set it to
  `victim@uni.edu`. Because the first account to match keeps that userid for
  good (line 127), the impostor also locks the real student out. This is the
  known "nOAuth" class of bug. Either match on sign-in name only, or accept
  email only when the domain is verified (the `xms_edov` claim), or restrict
  which tenants can sign in.
* <strong>The organizer's own identity isn't defined.</strong> Students are
  bound to their stable account on first sign-in, but the organizer is
  identified only as "the organizer" (lines 38–40). If that means the
  organizer's sign-in name, the same mutable-identifier problem applies.
* **It's unclear which machine the 60-second limit applies to** (lines 307–310
  and 407–408). The supported browsers include phones (line 423). If forming
  runs in the browser, 500 × 100 has to finish on a two-year-old phone. If it
  runs on the server, the spec needs to cover a job that survives a page reload,
  and what the page shows after a reload during forming. The same choice decides
  what **Cancel** does and whether "on any computer" refers to clients or
  servers. Since the result may not depend on speed, the search has to stop on a
  fixed criterion. So "within 60 seconds" needs a reference machine, or it's
  untestable.
* **The test oracle in the acceptance criteria can be infeasible** (lines
  438–439). "Random lists of at most 8 students" puts no bound on projects. With
  8 students, *k* = 1 and 100 projects, checking every valid assignment means
  about 100!/92! ≈ 7.5 × 10¹⁵ project assignments. The criterion should bound
  *P*, for example to 4 or fewer.

##### What's strong

The scoring is normalized and fully stated, and the sizing rules are exact, with
examples that check out. The Expected results table is correct and every row has
exactly one best assignment. Rename versus remove-plus-add is clearly separated,
the CSV format is precise, the statement that teammate preferences can't be
fully hidden is honest, and the out-of-scope list is concrete. Fixing items 1–3
and the email-claim issue would make it a solid 9.

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
