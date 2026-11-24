Codex review notes — Daniil
==========================

Scope: specification only. No implementation or runtime tests were supplied.

| Round | Reviewed version | Findings | Changes / result |
| -- | -- | -- | -- |
| Local review 1 | fc02540 | 8 groups | Addressed in d1e4650; see the review below and its commit message. |
| Local follow-up 1 | d1e4650 | 0 further blocking inconsistencies reported | Text checks passed; this was not a build or feasibility check and did not establish that all requirements were supported by existing code. Askar's subsequent review found 14 issues. |
| Local review 2, Daniil scope | Working changes after Askar's round 2 | 3 assigned issues, plus 2 related consistency issues | Defined local cursor/selection relocation; removed exact keyboard navigation promises; replaced left with before; removed rule 3's cursor-driven expansion reference; clarified that cursor relocation may send a cursor update under rule 6. |
| Local follow-up 2, Daniil scope | Current working changes | 0 further findings in rules 1–4 and 6 | Checked wording and cross-references. Full-spec consistency remains pending teammates' assigned changes. |

The original round 1 with 13 findings and Askar's round 2 with 14 findings
are Askar's reviews, not additional Codex reviews performed in this chat.
Their attachments were not available here. The first Daniil edits were based
on the findings quoted in Askar's email.

Local review 1 — findings recorded in d1e4650
-------------------------------------------

1. Clarify document order, Rust doc headings, empty sections and source sections
   ending within one doc block.
2. Replace serializer-only guidance with requirements covering raw saves,
   direct innerHTML reads and cursor mapping.
3. Distinguish IDE cursor movement from repeated positions and scroll updates.
4. Define heading matching for incremental and full updates, including the
   limitation with duplicate headings.
5. Clarify independently collapsed descendants when a parent is removed.
6. Preserve existing heading ids and exclude persisted collapse metadata.
7. Replace the save-without-edit example and add missing acceptance cases.
8. Align test guidance with browser Client tests and end-to-end checks.

All eight groups were accepted then; none were rejected. These notes summarize
the review retained in the commit message, rather than a separate verbatim
review transcript. Askar subsequently requested simplification of the result.

Last local review — Daniil scope
-------------------------------

Rules 1–4 and 6: no further findings after the current corrections. Keyboard
access is required without promising an exact Tab order. Collapsing relocates
a cursor or selection endpoint from hidden content to the heading. Rule 3 no
longer promises cursor-driven expansion. Rule 6 allows normal cursor updates
while forbidding content updates caused solely by toggling.

No findings were rejected, so there are no struck-through findings.

Integration note: the current rule 7, examples and Implementation still reflect
older cursor-expansion behavior. Askar and Zhan own those round 2 changes;
this scoped review does not claim the whole document is ready for submission.
Build and runtime tests were not run for these prose-only changes.
