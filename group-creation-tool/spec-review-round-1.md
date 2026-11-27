Specification review – round 1
==============================

Prompting Claude Opus 5.5, high effort, with `Review group-creation-tool/spec.md
against the project description in the course notes. Report contradictions,
undefined cases, and statements an implementer could read two ways. Keep the
spec short; do not add features.` produced the following findings.
Strikethrough marks findings we judged incorrect or irrelevant.

1. **HIGH**: `T` (mean score of the other group members) is undefined for a
   group of one, which occurs when `m = 2` and `n` is odd (`n = 5` gives sizes
   2, 2, 1). *Fixed: `T = 0` in a group of one.*
2. **HIGH**: `w` and `c` could only be changed until the survey closed, but the
   organizer normally closes the survey first and forms groups afterwards, so
   the settings could never be tuned on final data. *Fixed: `m`, `w` and `c`
   can change at any time; closing only stops submissions.*
3. **MEDIUM**: Step 2 said "maximize quality", but the teammate part of
   satisfaction does not depend on projects and the tie-break cannot be
   expressed as an assignment problem. *Fixed: step 2 maximizes the sum of
   `w · P`.*
4. **MEDIUM**: CSV group names collide when the project list holds both
   `htmd fuzzer` and `htmd fuzzer 2`. *Fixed: such project lists are rejected.*
5. **MEDIUM**: The numbering of a project's extra groups depended on "the order
   the algorithm produced them", which is not defined. *Fixed: numbered by each
   group's alphabetically first userid.*
6. **MEDIUM**: "Only the organizer can view the survey" contradicts students
   opening it through the link. *Fixed: only the organizer can change settings,
   close the survey, or see submissions and results.*
7. **LOW**: The organizer has no way to see who has not answered yet before
   closing. *Fixed: the organizer sees which students have submitted.*
8. **LOW**: The quality test required the heuristic's result to "equal the
   optimum", although several assignments can share the optimal quality.
   *Fixed: compare quality only.*
9. ~~**MEDIUM**: Replace the local search with an integer linear program (PuLP)
   to guarantee the optimum.~~ Rejected: adds a solver dependency and
   overspecifies; local search is adequate for class-sized rosters (design
   decision 4).
10. ~~**LOW**: Notify students by email when a survey opens or closes.~~
    Rejected: new feature, out of scope.

8 findings applied, 2 rejected.
