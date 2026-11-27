Specification review – round 2
==============================

Same prompt and model as round 1. Strikethrough marks findings we judged
incorrect or irrelevant.

1. **MEDIUM**: Steps 2 and 3 could cycle. A swap in step 3 may improve only the
   tie-break (the least satisfied student), after which step 2 may pick another
   project assignment with the same sum but a worse tie-break, which the next
   swap undoes, and so on. *Fixed: step 2 replaces the current project
   assignment only if the result is better, so every change strictly improves
   the assignment and the search terminates.*
2. ~~**LOW**: Specify that a swap is evaluated from the change in the two
   affected groups only, to keep a pass fast.~~ Rejected: an implementation
   detail that does not change behavior.
3. ~~**LOW**: Let students mark another student as a required teammate.~~
   Rejected: new feature, out of scope.

1 finding applied, 2 rejected.
