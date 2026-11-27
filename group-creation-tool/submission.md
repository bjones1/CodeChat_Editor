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

Generated initial specification for the project, conducted multiple calls to clarify requirements.

#### Build and run test

Build and run tests are not included, as current project stage only required the specification (as of Nov. 28, 2026). Also, current repository (CodeChat Editor) is not directly related to the project, and only used for pull request tracking.

#### Specification review rounds

| Round | Findings | Main Changes Made |
| --- | --- | --- |
| 1 | Rated 7/10; 25 findings in total. **High:** teammate scoring contradicts the "no extra weight" claim, Unspecified ordering rules conflict, teammate privacy can't be guaranteed, default group count gives solo groups when projects outnumber students, userid relies on a mutable sign-in claim. **Medium:** deleted-session message vs. deleting all data, two-tab save rule, repeatability vs. time limit, stale results after option changes. **Low:** 7 vague terms and minor gaps (over-long names, CSV, retention, terminology). Expected results table verified correct. | Fixed every finding except the 2 struck through below and the tie-break part of item 9, plus 2 findings from CodeRabbit's review of the pull request. Group size is now about √S (an organizer option, at least 2), projects are spread evenly and a project can have several groups, and the teammate part of satisfaction is scaled to −1…1. Results no longer show the score or teammate counts, and the privacy promise now covers only showing teammate preferences. Unspecified always keeps list order. Added Rename, filter fields, and move to top or bottom. Students are matched by sign-in name or email, then by account. Added automatic save retries, last-save-wins across tabs, out-of-date results after option changes, 365-day retention, and testable timings and browser versions. Added Expected results rows and a teammate-part table, all rechecked by brute force. |
| 2 | Rated 8/10; 20 findings in total. **High:** not-preferred share has the wrong sign as written, CSV example needs 3 groups from 4 students (impossible), group size 1 divides by zero, matching on the email claim lets one person pass as another and lock out the real student. **Medium:** closed-session save rejection loops on retry, a stale tab can restore preferences for a removed and re-added entry, privacy wording contradicts itself, "group size" isn't the actual size, organizer identity is undefined, the 60-second limit names no machine, the ≤8-student test oracle is unbounded in projects. **Low:** 9 vague terms ("last change", "changes it", rounding of halves, "letter" in file names, userid syntax, separators, title trimming, Rename collisions, propagation of Close/Reopen/Delete). Expected results table and default group sizes verified correct by brute force. | Fixed the 11 High and Medium findings and CodeRabbit's 2 comments on the pull request, which repeat items 1 and 3; left the 9 Low vague terms as they are. The not-preferred share divides by the size of the most negative sum, and both shares are 0 for a student alone in a group. The CSV example now has 6 students in 3 groups. Students are matched on sign-in name only, not email, and organizers are recognized by their account. A save refused because the session closed is final, and a save from a stale page never applies to a re-added entry. The privacy rule says "number" instead of "figure". The option is renamed "Minimum group size", with sizes from *k* to under 2*k*. Forming runs on the server, survives a page reload, and its 60-second limit names a 2-core server. The small-class acceptance test is limited to 4 projects. Expected results rechecked by brute force. |

#### Last specification review

LLM: Claude Opus 5.5 (`claude-opus-5-5`), in the Claude Code tab of the Claude
desktop app.

Prompt:

> Review a specification at @group-creation-tool/specification.md . Rate it
> from 1 to 10. Do not modify the file. Find logical inconsistencies, vague
> terms, architectural mismatch.

Output (line numbers refer to [specification.md](specification.md) as it was
reviewed, before this round's changes; strikethrough marks findings judged
incorrect):

I'd rate it **8/10**, up from 7 in round 1. Most of round 1's findings are
fixed and the scoring model is now tight. I brute-forced all 13 Expected
results rows. Each one gives exactly the stated groups and score, and each has
exactly one best assignment. The default group sizes for 4–7, 50 and 500
students also match the text. Three things keep it from a 9: a sign error in
the teammate formula, a CSV example that the spec's own rules can't produce,
and an identity rule that lets one person pass as another.

##### Logical inconsistencies

1. **The not-preferred share has the wrong sign as written** (lines 241–247).
   The share is "the sum of the values … divided by the largest sum", and the
   not-preferred share is "worked out the same way". For not-preferred
   teammates both of those sums are negative, so dividing one by the other
   gives a positive number. Taken literally, table row 4 gives **+2/3** instead
   of −2/3, and row 5 gives **4/3** instead of −2/3. The text needs "divided by
   the absolute value of" (or "by the size of") the largest sum. The table and
   the Expected results only come out right with that reading.
2. **The CSV example can't happen** (lines 349–358). It shows 4 students in 3
   groups. But G = ⌊4/*k*⌋ gives 4, 2, 1, 1 for *k* = 1…4, so 3 groups is
   impossible with 4 students. Use 5 or 6 students, or 2 groups.
3. **Group size 1 divides by zero** (lines 200 and 239–248). *k* can be 1,
   which leaves *m* = 0 other members. The denominator is then "the sum of the
   first 0 values", which is 0. "An empty category's share is 0" covers an
   empty category, not a group with no other members.
4. **A save rejected because the session closed collides with the retry
   rule** (lines 165–167 and 178–180). "While a save is failing, the page
   retries every 10 seconds." A save refused because the session is Closed
   counts as failing, so the page would retry forever, keep the leave-page
   warning on, and could succeed later if the organizer reopens. Say that a
   closed-session refusal is final and the page switches to read-only.
5. **A stale tab can bring back preferences that should have been dropped**
   (lines 96–102, 171–174 and 409–412). "Each save replaces all of the
   student's saved preferences." Suppose the organizer removes project `Foo`
   and adds it again; it is now a new entry that "starts over". A student page
   loaded before that edit still ranks the old `Foo`. Its next save can attach
   that ranking to the new `Foo` unless saves are checked against the current
   list version.
6. **The privacy rule contradicts itself** (lines 184–191). "No page or file
   shows … any figure computed from them", yet the groups are computed from
   teammate preferences and are shown. The next paragraph admits this. Say "no
   figure other than the groups themselves".
7. **The "group size" isn't the size of a group.** With 13 students and *k* =
   7, G = ⌊13/7⌋ = 1, so the organizer gets one group of 13. The real size
   ranges from *k* up to almost 2*k*−1. The help text should say *k* is the
   minimum size, or the control should be renamed.

##### Vague terms

* **"Last change"** (line 33): deletion comes 365 days after the last change.
  Does a student's save, a sign-in, or an organizer just viewing the session
  count?
* **"Until the organizer changes it"** (line 202): does typing the default
  value into the field count as a change?
* **Number fields** (lines 282–284): "rounded to the nearest whole number"
  doesn't say how halves round. Line 201 does say it for √*S*.
* **File name** (line 334): does "a letter" mean ASCII letters only, or
  Unicode letters? That decides whether `Проект` keeps its letters or becomes
  `______`.
* **Userid syntax** (lines 66–67): `a@b.` and `a@.edu` both pass ("contains at
  least one `.`"). The case-insensitive comparison (line 19) also doesn't say
  whether it uses full Unicode case folding.
* **Separators** (line 58): semicolons and tabs aren't separators. A list
  pasted from Outlook (`Jane Doe <jane@uni.edu>;`) is dropped entry by entry.
* **Title** (line 57): trimming is stated only for list entries, so a title
  made only of spaces seems valid.
* **Rename collisions** (lines 104–108): what happens when a project or userid
  is renamed to one already on the list, ignoring case? Also, the lists are
  edited "as text", but Rename is a per-entry action; how the textarea and the
  status table coexist isn't described. After a case-only edit, it's unclear
  whether the CSV's userid "as entered" (line 342) uses the new or the
  original casing. If one account's sign-in name matches one entry and its
  email matches another (lines 122–128), which userid gets linked?
* **Propagation of Close, Reopen, Delete and removal:** list edits reach open
  student pages "within a minute" (line 410), but nothing says how fast a
  student page notices a Close, a Reopen, a deletion, or that the student was
  removed. Nor does the spec say what happens if the organizer reopens the
  session, edits the lists or changes the options while groups are being
  formed (lines 293–297).

##### Architectural mismatch

* **Matching on the email claim lets one person pass as another** (lines
  122–124 and 398). Anyone with any work or school account can sign in, and
  the spec matches students by their account's email address. In Microsoft
  Entra the `email` claim is set by the user's own organization's admin and
  may be unverified. Someone controlling their own tenant can set it to
  `victim@uni.edu`. Because the first account to match keeps that userid for
  good (line 127), the impostor also locks the real student out. This is the
  known "nOAuth" class of bug. Either match on sign-in name only, or accept
  email only when the domain is verified (the `xms_edov` claim), or restrict
  which tenants can sign in.
* **The organizer's own identity isn't defined.** Students are bound to their
  stable account on first sign-in, but the organizer is identified only as
  "the organizer" (lines 38–40). If that means the organizer's sign-in name,
  the same mutable-identifier problem applies.
* **It's unclear which machine the 60-second limit applies to** (lines 307–310
  and 407–408). The supported browsers include phones (line 423). If forming
  runs in the browser, 500 × 100 has to finish on a two-year-old phone. If it
  runs on the server, the spec needs to cover a job that survives a page
  reload, and what the page shows after a reload during forming. The same
  choice decides what **Cancel** does and whether "on any computer" refers to
  clients or servers. Since the result may not depend on speed, the search has
  to stop on a fixed criterion. So "within 60 seconds" needs a reference
  machine, or it's untestable.
* **The test oracle in the acceptance criteria can be infeasible** (lines
  438–439). "Random lists of at most 8 students" puts no bound on projects.
  With 8 students, *k* = 1 and 100 projects, checking every valid assignment
  means about 100!/92! ≈ 7.5 × 10¹⁵ project assignments. The criterion should
  bound *P*, for example to 4 or fewer.

##### What's strong

The scoring is normalized and fully stated, and the sizing rules are exact,
with examples that check out. The Expected results table is correct and every
row has exactly one best assignment. Rename versus remove-plus-add is clearly
separated, the CSV format is precise, the statement that teammate preferences
can't be fully hidden is honest, and the out-of-scope list is concrete. Fixing
items 1–3 and the email-claim issue would make it a solid 9.

### (Team member 2 name)

(Same sections as Team member 1)

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