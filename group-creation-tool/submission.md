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
scoring and the grouping algorithm), applied the round 4 fixes from the pull
request review, and assembled this submission.

#### Build and run test

This draft contains only the specification of a new app (`group-creation-tool/`); there is no code to build or run yet. The build test applies once the implementation exists.

#### Specification review rounds

| Round | Findings | Main changes made |
| ----- | -------- | ----------------- |
| 1     | 10       | Defined satisfaction for a group of one; settings stay editable after closing; project step maximizes only `w · P`; CSV name collisions rejected and extra groups numbered deterministically; organizer sees who has submitted (2 findings rejected) |
| 2     | 3        | Project assignment replaced only when it improves the result, so the search cannot cycle (2 findings rejected) |
| 3     | 2        | No changes: both findings trivial and rejected |
| 4     | 4        | CodeRabbit findings on the PR, verified with Claude: comparisons round to 9 decimals so "better" is a strict order and the search terminates; bound accounts matched before userids; scoring described as a shared −1…1 scale; step 2's tie-break limitation stated |

#### Last specification review

Prompting Claude Opus 5.5, high effort, with `Check each of these review comments against group-creation-tool/spec.md; say whether it is correct and propose the smallest fix.` on the four CodeRabbit comments on pull request #162 produced the following output (round 4, full text in [spec-review-round-4.md](spec-review-round-4.md)).

1. **HIGH**: With a 10⁻⁹ tolerance, "better" is not transitive (A > B > C > A is possible), so the termination claim did not hold. Fixed: values are rounded to 9 decimal places before comparing, making "better" a strict order.
2. **HIGH**: A student whose sign-in name changed was rejected before the `tid`/`oid` match. Fixed: bound entries are looked up first.
3. **MEDIUM**: "Same total influence" is false (one preferred item totals 1, two total 1.5). Fixed: described as a shared scale from 1 to −1.
4. **MEDIUM**: Step 2 ignores the tie-break among equal-sum project assignments. Fixed by stating this limitation of the heuristic.

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
| 4     | 4        | CodeRabbit findings on the PR, verified with Claude: comparisons round to 9 decimals so "better" is a strict order and the search terminates; bound accounts matched before userids; scoring described as a shared −1…1 scale; step 2's tie-break limitation stated |

#### Last specification review

Prompting Claude Opus 5.5, high effort, with `Check each of these review comments against group-creation-tool/spec.md; say whether it is correct and propose the smallest fix.` on the four CodeRabbit comments on pull request #162 produced the following output (round 4, full text in [spec-review-round-4.md](spec-review-round-4.md)).

1. **HIGH**: With a 10⁻⁹ tolerance, "better" is not transitive (A > B > C > A is possible), so the termination claim did not hold. Fixed: values are rounded to 9 decimal places before comparing, making "better" a strict order.
2. **HIGH**: A student whose sign-in name changed was rejected before the `tid`/`oid` match. Fixed: bound entries are looked up first.
3. **MEDIUM**: "Same total influence" is false (one preferred item totals 1, two total 1.5). Fixed: described as a shared scale from 1 to −1.
4. **MEDIUM**: Step 2 ignores the tie-break among equal-sum project assignments. Fixed by stating this limitation of the heuristic.

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
| 4     | 4        | CodeRabbit findings on the PR, verified with Claude: comparisons round to 9 decimals so "better" is a strict order and the search terminates; bound accounts matched before userids; scoring described as a shared −1…1 scale; step 2's tie-break limitation stated |

#### Last specification review

Prompting Claude Opus 5.5, high effort, with `Check each of these review comments against group-creation-tool/spec.md; say whether it is correct and propose the smallest fix.` on the four CodeRabbit comments on pull request #162 produced the following output (round 4, full text in [spec-review-round-4.md](spec-review-round-4.md)).

1. **HIGH**: With a 10⁻⁹ tolerance, "better" is not transitive (A > B > C > A is possible), so the termination claim did not hold. Fixed: values are rounded to 9 decimal places before comparing, making "better" a strict order.
2. **HIGH**: A student whose sign-in name changed was rejected before the `tid`/`oid` match. Fixed: bound entries are looked up first.
3. **MEDIUM**: "Same total influence" is false (one preferred item totals 1, two total 1.5). Fixed: described as a shared scale from 1 to −1.
4. **MEDIUM**: Step 2 ignores the tie-break among equal-sum project assignments. Fixed by stating this limitation of the heuristic.

