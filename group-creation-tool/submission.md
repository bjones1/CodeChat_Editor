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