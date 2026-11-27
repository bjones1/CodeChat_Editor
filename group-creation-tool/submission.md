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

I'd rate it **7/10**. It's precise, mostly testable, scopes itself clearly, and
its worked examples are correct. I ran an exhaustive search over every row of
the Expected results table, and each one produces exactly the stated groups and
score, with a single best assignment. What keeps it from an 8–9 is a handful of
real contradictions, one false claim in the scoring model, and identity and
privacy promises that can't be kept as written.

##### Logical inconsistencies

1. **The weighting claim is false for teammates** (line 184). The spec says
   scaling by category length means ranking more items gives no extra weight.
   That holds for projects, because a group has only one. It fails for
   teammates, because satisfaction *sums* teammate values. A student with one
   preferred teammate can add at most 1; one with four preferred teammates, all
   placed together, adds 1 + .75 + .5 + .25 = 2.5. Students who rank more
   classmates get more pull on the result, and the effect grows with group
   size.
2. **Unspecified can't be reordered, yet items move to its end.** Unspecified
   "keeps the order of the organizer's list" and "cannot reorder" (line 117).
   But "an item moved into a category goes to its end" and "any item" can move
   "up or down within its category" (lines 123–125). Moving a Preferred item
   back to Unspecified breaks one rule or the other.
3. **The page reports a deletion that left no trace.** Deleting "removes all
   its data" (line 31), but a later visitor "see[s] a message saying so" (line
   104). The server can only tell "deleted" from "never existed" if it keeps a
   record of deleted sessions. The order of checks is also unstated: a deleted
   session versus a user who isn't on the list.
4. **"Nobody ever sees teammate preferences" can't be guaranteed** (line 147).
   The organizer sees the score, can set *W*<sub>p</sub> = 0, and sees how many
   students have at least one preferred or not-preferred teammate (line 238).
   In a small class, or by re-forming with different weights, teammate
   preferences can be worked out. The promise should be "never shown
   directly".
5. **The default number of groups contradicts its own rationale** (line 157).
   The default is the smaller of the student and project counts, "so that by
   default every project gets a group". When there are more projects than
   students, that's untrue: the default becomes one student per group,
   teammate preferences stop mattering, and some projects go unused. A class of
   20 with 25 project ideas gets 20 solo groups by default.
6. **The two-tab rule is self-defeating** (lines 136–137). "The later save
   wins" means that when the other tab next saves, *its* state wins. So "shows
   the saved state the next time it saves" never changes anything. Also, list
   changes are polled (they appear "within a minute", line 305), but
   preferences changed in another tab aren't, which is inconsistent.
7. **A cross-reference points to text that isn't there.** "Keeps retrying the
   save, as described under Saving" (line 301), but Saving describes only a
   manual **Retry** button, not automatic retries.
8. **Changing options doesn't say what happens to formed groups.** Editing the
   lists discards them (line 89), but changing the grouping options says
   nothing (line 207). The results and CSV can then disagree with the saved
   options.
9. **Repeatability conflicts with the time limit.** "Repeatable" (line 225) and
   "within 60 seconds" (line 303) only coexist if the search stops after a
   fixed amount of work, not a wall-clock deadline. Otherwise a slower machine
   gives different groups. ~~Which of several tied assignments wins is never
   specified either, for example when both weights are 0 and every assignment
   scores 0.~~
10. **Over-long project names have no stated outcome.** Projects must be 1–200
    characters (line 55), but the list of drop reasons (lines 57–61) doesn't
    cover a 201-character entry: is it dropped, truncated, or does it block
    **Create**?

##### Vague terms

* **Wording that can't be tested:** "about a second" (133), "just as the
  session closes" (142), "typical connection" (326), and "current" Chrome,
  Edge, Firefox and Safari (318).
* **"At the default" for the number of groups** (166): does it mean the value
  equals the upper limit, or that the organizer never touched it?
* **Number fields:** the step for the weights (integer or decimal?) and how a
  fractional number of groups is rounded are both unstated.
* **Input formats:** what counts as `name@domain` (is `a@b` valid?), and
  whether the 1–100 title length is counted in code points or graphemes.
* **File names:** "characters that are not allowed in file names" doesn't say
  on which operating system. Windows reserved names like `CON` aren't
  addressed.
* **Status labels:** does *signed in* mean signed in via this session's link or
  to the tool at all? Does a student removed and re-added keep their status and
  display name?
* **Display formats:** "Shows progress" (212) has no meaning for a local search
  with no known end point. Score precision (values like 1/3) and whether
  results show members by display name or userid are unspecified.
* **Renaming:** list identity is the name itself, so fixing a typo in a project
  name counts as remove plus add and silently wipes everyone's preferences for
  it (86–90). It's also unclear whether a case-only change (`Parser` →
  `parser`) is the same item.

##### Architectural mismatch

* **Identity is keyed on a value Microsoft says not to authorize on.** The
  userid is "the sign-in name" (17). With only name and sign-in-name permission
  (297), that's the `preferred_username` claim, which Microsoft documents as
  changeable and unsuitable for authorization decisions. Sign-in names also
  often differ from the email addresses an organizer will paste in, and guest
  accounts look like `x_gmail.com#EXT#@…`. Expect many false "not on this list"
  pages. A safer design keeps the typed userid for matching but binds the
  stable `tid` + `oid` account identifiers on first sign-in.
* **Any Microsoft work or school account can create sessions** (35), from any
  organization. Some organizations also block users from granting consent to
  outside apps, which is a likely sign-in failure the spec doesn't mention.
* **Safari on iPhone doesn't reliably support the leave-page warning** (135),
  which conflicts with the browser list on line 318.
* **Ranking at 500 students is impractical by keyboard.** A student sees up to
  499 teammates, and moving one item a step at a time is the only keyboard
  method (123, 320). That's technically accessible but unusable: it needs "move
  to top" or search.
* **"Keeps what the user has entered"** (301) doesn't say whether that survives
  a page reload, which decides whether browser storage is needed.
* **CSV gaps.** Project names starting with `=`, `+`, `-` or `@` become
  formulas when opened in Excel, and the "never run as a script" rule (316)
  doesn't cover the file. Without a byte-order mark, Excel on Windows also
  garbles non-ASCII names. Both may be acceptable if the file is only read by
  programs, but the spec should say so.
* **Data retention is unbounded.** Student data is kept indefinitely (30) and
  only one organizer can delete it, so an organizer who leaves orphans their
  sessions. That's a privacy and compliance concern for school data.
* ~~**Terminology:** a "Microsoft Teams account" (17) is really a Microsoft
  work or school account, and Teams integration is out of scope, so the Teams
  framing is misleading.~~
* ~~**Repo fit:** the spec sits in the CodeChat Editor repo, a local
  single-user Rust/TypeScript editor. Nothing there provides multi-user
  storage, Microsoft sign-in or polling, so this would be a standalone
  codebase. The "doesn't prescribe how the code is organized" claim (4) also
  softens once polling, a cancellable server job, and stable ids are
  effectively required.~~

##### What's strong

The scoring model is stated exactly, the sizing rules are clear, the CSV format
is precise, and the example table is correct and covers useful edge cases. The
acceptance criteria can be checked, the out-of-scope list is explicit, and the
accessibility, theme and security requirements are concrete. Fixing items 1–6
and the identity issue would put it at about 9.

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