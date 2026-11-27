Specification review – round 4
==============================

Findings from the automated CodeRabbit review on pull request
[#162](https://github.com/bjones1/CodeChat_Editor/pull/162), each verified by
prompting Claude Opus 5.5, high effort, with `Check each of these review
comments against group-creation-tool/spec.md; say whether it is correct and
propose the smallest fix.` Strikethrough marks findings we judged incorrect or
irrelevant.

1. **HIGH**: With a 10⁻⁹ tolerance, "better" is not transitive: totals that tie
   pairwise within the tolerance can still form a cycle A > B > C > A, so the
   claim that the search terminates did not hold. *Fixed: quality and the least
   satisfaction are rounded to 9 decimal places before comparing, which makes
   "better" a strict order.*
2. **HIGH**: A student whose sign-in name changed was rejected as "not on the
   roster" before the `tid`/`oid` match could recognize them. *Fixed: the app
   looks for an entry bound to the `tid`/`oid` first, and matches the userid
   only for unbound entries.*
3. **MEDIUM**: "Every student gets the same total influence" is false: one
   preferred item totals 1, two total 1.5. *Fixed: the text now says scores
   share the same scale, from 1 for the top item to −1 for the least wanted.*
4. **MEDIUM**: Step 2 maximizes only the sum, so among equal-sum project
   assignments it may miss the one with the better tie-break. *Fixed by stating
   it: the solver's choice is used, since the search is a heuristic and does not
   promise the best possible result. Making step 2 tie-break-aware would need a
   different solver for little benefit.*

4 findings applied.
