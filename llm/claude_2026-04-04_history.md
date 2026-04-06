# Conversation History

## User

Review and critique proposed new features as documented in `server/src/processing/cache.rs` with supporting routines in `server/src/processing.rs`. Look for missing specifications, consistency errors, or faulty reasoning.

## Assistant

Here is a detailed critique organized by severity:

---

## Critical Bugs

### 1. Test type mismatch: `Arc` vs `Weak` in `cache_id` (cache.rs:465-472)

`Cache::id` is declared as `HashMap<String, Weak<Mutex<Target>>>`, but the test inserts `Arc<Mutex<Target>>` values directly (from `file_baz_cpp.lock().unwrap().target[0].clone()`). This won't compile.

### 2. Test sets `h1` to the wrong target (cache.rs:458-459)

```rust
let h1 = Arc::downgrade(&file_baz_cpp.lock().unwrap().target[1]);
```
`target[1]` is the `gathering_tag` (a `Gather` type). The `h1` field should point to a heading (`target[0]`, `id = "one"`). This is logically wrong.

### 3. `#[test]` attribute missing on `test_1` (cache.rs:386)

`fn test_1()` will never run under `cargo test`. It needs `#[test]`.

---

## Design Inconsistencies

### 4. `Target` references a non-existent `type_` field (cache.rs:233-234)

The doc comment says "ignored if the `type_` is `File`", but `Target` has no `type_` field. Either the field was removed and the comment is stale, or a `TargetType` enum was intended and was never added. Related: how would a file-level target (e.g., the file itself as a link target) be distinguished from an in-page target?

### 5. `LinkOptions` combinatorics won't scale (cache.rs:279-284)

`AutoTitleAndNumber` is a manually-combined variant. The TODO even suggests bitflags. If `Plain` + `AutoTitle` + `AutoNumber` ever gains a third axis, this blows up. Either use `bitflags!` or a small struct (`struct LinkOptions { auto_title: bool, auto_number: bool }`).

### 6. `Pending` and `Dirty` statuses are functionally indistinguishable in `pending_files` (cache.rs:214-222)

The doc says `pending_files` processes a file only when its status is not `Clean`. Both `Pending` and `Dirty` pass this check. No code in the pseudocode ever differentiates between them. If there's a meaningful behavioral difference (e.g., `Pending` means no prior data to reuse; `Dirty` means prior data exists), that distinction needs to be made explicit in the processing logic.

### 7. `Target::contents` spec is vague for empty anchors (cache.rs:239-241)

For `<a id="x"></a>`, the comment says it stores "HTML context" rather than contents, but doesn't define what "context" means — is it the surrounding paragraph? The enclosing heading? The next sentence? This needs a concrete definition, including how far the context window extends.

---

## Missing Specifications

### 8. No deletion logic for `Cache::id` (cache.rs:184-185)

The "Supported operations" section lists "Delete a given file from the cache," but there is no implementation or pseudocode for it. Deleting a file requires:
- Removing all its `Target`s from `Cache::id`
- Cleaning up `references` and `dependencies` in *other* `Target`s that pointed to the deleted targets
- Marking dependents dirty

None of this is addressed.

### 9. Stale `Weak` pointers in `Cache::id` and `LinkType` will accumulate (cache.rs:185, cache.rs:260-263)

When a file is re-processed, its old `Target` `Arc`s are replaced (see the `mem::take` in processing.rs:1126). But `Cache::id` holds `Weak` references to the *old* `Arc`s — those `Weak`s will dangle. There's no described mechanism to update `Cache::id` when targets are replaced. Same issue for `Target::references` and `Target::dependencies` holding `Weak<Mutex<File>>` or `Weak<Mutex<Target>>`.

### 10. No `Serialize`/`Deserialize` for on-disk cache persistence (cache.rs:180-192)

The spec mentions "Keep the in-memory cache synchronized with the on-disk cache," but none of the structs derive `serde::Serialize`/`Deserialize`. This is a significant omission that will require retrofitting.

### 11. Tag `?end` query parameter semantics are ambiguous (cache.rs:347-352)

The pseudocode says "if this tag includes an end query parameter, set the end index of the current tag, **or (if there's no tag) set the start index to the current index - 1**." The second branch is unclear: a link with `?end=N` that is *not* a tag? How does that arise? The spec in the module doc only describes `?end` on tags. This needs either removal or a concrete example.

### 12. `File::toc: Vec<u32>` meaning is underspecified (cache.rs:205-208)

Is this `[1, 2, 3]` representing section 1.2.3? Or a flat chapter index? How is it populated, and by what process? The "Revise the TOC loader" note at the bottom doesn't connect to this field.

---

## Threading Issues

### 13. `Rc<Node>` in a multi-threaded context (cache.rs:322-323, processing.rs:1118)

`upsert_file_core` takes `dom: Rc<Node>`, and `hydrate_html` also uses `Rc<Node>` while holding `Arc<Mutex<Cache>>`. `Rc` is not `Send`, so if processing is ever dispatched to a thread pool, this won't compile. The design mixes single-threaded and multi-threaded ownership in a way that will need resolution before any async/parallel processing is added.

---

## Minor Issues

### 14. Incomplete FIXME comment (cache.rs:303)

`// FIXME: add new file to` — sentence is cut off. Presumably "add new file to `pending_files`," but this is unverified.

### 15. Auto-title indirection boundary with tags is unspecified (cache.rs:48-49)

The spec says auto-titled links don't support indirection, but tags *do* support indirection (auto-titled links inside a captured doc block propagate as indirect dependencies). The spec doesn't address what happens if a tag captures a doc block containing an auto-titled link that points to another auto-titled link. Since auto-title indirection is prohibited, does this make the outer tag's indirect dependency resolution fail silently or error?

### 16. `pending_files: Vec<PathBuf>` allows unbounded duplicates (cache.rs:188-190)

A `Vec` with deduplication deferred to status-check at processing time is wasteful in high-churn scenarios. A `HashSet` or `IndexSet` would provide O(1) deduplication at insertion time and is semantically more accurate (ordering doesn't appear to matter for correctness here, only for fairness).
