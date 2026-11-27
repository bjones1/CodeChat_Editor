Project 2 submission
====================

Team name: Project group creation tool

The specification consists of the file [spec.md](spec.md).

Draft submission
----------------

### Baltabayev Timur

GitHub userid: timikxd

#### Contributions

Set up the fork and branch, wrote the specification draft (sign-in, surveys,
scoring and the grouping algorithm), and assembled this submission.

#### Build and run test

This draft contains only the specification of a new app (`group-creation-tool/`); there is no code to build or run yet. The build test applies once the implementation exists.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1     | 10       | Defined satisfaction for a group of one; settings stay editable after closing; project step maximizes only `w · P`; CSV name collisions rejected and extra groups numbered deterministically; organizer sees who has submitted (2 findings rejected) |
| 2     | 3        | Project assignment replaced only when it improves the result, so the search cannot cycle (2 findings rejected) |
| 3     | 2        | No changes: both findings trivial and rejected |

#### Last specification review

Prompting Claude Opus 5.5, high effort, with `Review group-creation-tool/spec.md against the project description in the course notes. Report contradictions, undefined cases, and statements an implementer could read two ways. Keep the spec short; do not add features.` produced the following output (round 3, full text in [spec-review-round-3.md](spec-review-round-3.md)); strikethrough indicates LLM output that we deemed incorrect or irrelevant.

1. ~~**LOW**: Naming SortableJS in Implementation overspecifies the user interface.~~ The section only records the chosen stack; behavior is specified elsewhere.
2. ~~**LOW**: The 10⁻⁹ comparison tolerance is unnecessary because scores are rational.~~ They are computed as floating-point numbers, so the tolerance keeps comparisons and results stable.

No findings applied; only trivial or contradictory findings remain.

### Kurmanbek Beksultan

GitHub userid: Bexultan-KBTU

#### Contributions

Ran the first specification review round and recorded its findings, including
the group-of-one and settings-after-closing fixes.

#### Build and run test

This draft contains only the specification of a new app (`group-creation-tool/`); there is no code to build or run yet. The build test applies once the implementation exists.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1     | 10       | Defined satisfaction for a group of one; settings stay editable after closing; project step maximizes only `w · P`; CSV name collisions rejected and extra groups numbered deterministically; organizer sees who has submitted (2 findings rejected) |
| 2     | 3        | Project assignment replaced only when it improves the result, so the search cannot cycle (2 findings rejected) |
| 3     | 2        | No changes: both findings trivial and rejected |

#### Last specification review

Prompting Claude Opus 5.5, high effort, with `Review group-creation-tool/spec.md against the project description in the course notes. Report contradictions, undefined cases, and statements an implementer could read two ways. Keep the spec short; do not add features.` produced the following output (round 3, full text in [spec-review-round-3.md](spec-review-round-3.md)); strikethrough indicates LLM output that we deemed incorrect or irrelevant.

1. ~~**LOW**: Naming SortableJS in Implementation overspecifies the user interface.~~ The section only records the chosen stack; behavior is specified elsewhere.
2. ~~**LOW**: The 10⁻⁹ comparison tolerance is unnecessary because scores are rational.~~ They are computed as floating-point numbers, so the tolerance keeps comparisons and results stable.

No findings applied; only trivial or contradictory findings remain.

### Sagynbayev Alisher

GitHub userid: a-sagynbayev

#### Contributions

Ran the second and third specification review rounds and recorded their
findings, including the fix that keeps the search from cycling.

#### Build and run test

This draft contains only the specification of a new app (`group-creation-tool/`); there is no code to build or run yet. The build test applies once the implementation exists.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1     | 10       | Defined satisfaction for a group of one; settings stay editable after closing; project step maximizes only `w · P`; CSV name collisions rejected and extra groups numbered deterministically; organizer sees who has submitted (2 findings rejected) |
| 2     | 3        | Project assignment replaced only when it improves the result, so the search cannot cycle (2 findings rejected) |
| 3     | 2        | No changes: both findings trivial and rejected |

#### Last specification review

Prompting Claude Opus 5.5, high effort, with `Review group-creation-tool/spec.md against the project description in the course notes. Report contradictions, undefined cases, and statements an implementer could read two ways. Keep the spec short; do not add features.` produced the following output (round 3, full text in [spec-review-round-3.md](spec-review-round-3.md)); strikethrough indicates LLM output that we deemed incorrect or irrelevant.

1. ~~**LOW**: Naming SortableJS in Implementation overspecifies the user interface.~~ The section only records the chosen stack; behavior is specified elsewhere.
2. ~~**LOW**: The 10⁻⁹ comparison tolerance is unnecessary because scores are rational.~~ They are computed as floating-point numbers, so the tolerance keeps comparisons and results stable.

No findings applied; only trivial or contradictory findings remain.
