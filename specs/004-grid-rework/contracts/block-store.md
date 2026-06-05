# Contract — Block Store (internal interface)

The block store is the canonical, in-memory source of block text, behind a **stable
accessor seam** so a database backing can be added later with **no caller changes**
(FR-019, FR-020, SC-010). This contract is the first TDD target.

## Surface

```text
BlockId            : opaque, monotonic, Copy

trait BlockText:                       // the DB-ready seam
    text(&self) -> &str                // output text only
    text_with_command(&self) -> String // command + "\n" + output

BlockStore:
    begin(command: String, cwd: Option<PathBuf>) -> BlockId
    set_start_row(id: BlockId, start_row: StableRowIndex)     // at OSC 133 C; stamps started_at
    seal(id: BlockId, exit_code: Option<i32>, end_row: StableRowIndex)  // at OSC 133 D; stamps ended_at
    get(id: BlockId) -> Option<&Block>
    block_at_row(row: StableRowIndex) -> Option<BlockId>
    text(id: BlockId) -> Option<&str>
    text_with_command(id: BlockId) -> Option<String>
    duration(id: BlockId) -> Option<Duration>   // ended_at − started_at; None until sealed
    len() -> usize
    iter() -> impl Iterator<Item = &Block>      // insertion order
```

## Guarantees

1. **Caller isolation**: `/save`, `/filter`, and render obtain block text **only** via
   `text` / `text_with_command`. No caller reaches into `Block.output` or grid rows. This is
   what makes SC-010 (DB backing requires no caller changes) hold.
2. **Bounded memory**: at most `cap` blocks retained (from `Caps`); `begin` past cap evicts
   the oldest. Eviction never panics; `get`/`text` on an evicted id return `None`.
3. **Text survives grid eviction**: a block's retained `output` text remains queryable even
   after its grid rows scroll past the scrollback cap (R3). `block_at_row` for an evicted
   row returns `None`, but `text(id)` for a still-retained block returns the text.
4. **Boundaries from marks**: `begin`/`seal` are driven by OSC 133 `B`/`D` (or sentinel
   fallback), never grid heuristics (R7).
5. **Timing**: `set_start_row` (OSC 133 `C`, command execution) stamps `started_at`; `seal`
   (OSC 133 `D`) stamps `ended_at` (both wall-clock, serializable). `duration` is derived and
   is `None` until the block is sealed. When both are set, `started_at ≤ ended_at`.

## Test obligations (TDD, write first)

- `begin` then `seal` yields a `Finished` block with the recorded exit code and row range.
- `text_with_command` == `command + "\n" + text`.
- `duration` is `None` before `seal` and `Some(ended_at − started_at)` after, with `started_at ≤ ended_at`.
- Inserting `cap + 1` blocks evicts exactly the oldest; its id returns `None` everywhere.
- `block_at_row` maps a row inside a sealed range to its block; an evicted/unknown row → `None`.
- A block whose grid rows are evicted still returns its `text` (grid eviction ⊥ store eviction).
- A swappable `BlockText` impl (a stub "secondary store") satisfies the same accessor tests —
  proving the seam (SC-010) without writing a real DB.

## Future backing (designed-for, not built)

A `SecondaryStore` (e.g. SQLite) implements `BlockText` for evicted-from-memory blocks; the
store consults it on `text`/`text_with_command` miss. No `/save`/`/filter` change. Out of
scope for this MVP — only the seam ships.
