# Ratatui Scrolling-Regions Bug Fix Journal

## Bug Description
- Scrolling-regions feature causes render corruption with insert_before()
- Bug appears when viewport is full height or one line shorter than screen
- Works correctly when viewport is two lines shorter

## Investigation Progress

### Step 1: Setup Verification ✓
- Checked run_repro.sh script - runs repro008 example with configurable viewport height
- Verified local ratatui checkout is being used: Cargo.toml points to `/var/home/eitan/projects/termin.ai/ratatui`
- Confirmed scrolling-regions feature is enabled

### Step 2: Bug Reproduction ✓
- Running `./run_repro.sh 1`: Shows visible corruption - output is jumbled/malformed
- Running `./run_repro.sh 2`: No corruption - output renders correctly
- Bug confirmed: viewport height = (rows - 1) causes corruption, (rows - 2) works fine
- Repro008 example: Creates inline viewport, adds scrolling lines, uses `terminal.insert_before()` to push scrolled content to native scrollback

### Step 3: Code Analysis (In Progress)
**Location:** `ratatui/src/terminal/terminal.rs::insert_before_scrolling_regions()` (lines 695-762)

**Flow of problematic code (viewport height = rows - 1):**
1. Line 714: Special case for full-screen viewport (height == rows) - NOT triggered
2. Lines 736-751: Handles viewport not at bottom of screen
   - Example: Terminal 24 rows, viewport 23 rows starting at y=0
   - viewport_bottom (23) < screen_bottom (24) ✓ Condition triggers
   - to_draw = 1 (one empty row below viewport)
   - Calls `scroll_region_down(0..24, 1)` - scrolls ENTIRE screen down
   - Then sets viewport to start at y=1 (moves viewport down)
   - Problem: Creates gap at row 0, viewport now spans rows 1-23
3. Lines 753-759: Main loop to insert remaining lines
   - viewport_top is now 1 (not 0!)
   - Tries to scroll region 0..1 up
   - But row 0 is outside the viewport - corruption!

**Hypothesis:** When viewport is (rows-1), the code tries to expand viewport into empty space below, but this creates an off-by-one error where the viewport gets shifted and operations on row 0 become problematic.

### Step 4: Root Cause Identified ✓
**Problem:** The viewport expansion logic (lines 736-751) has a critical flaw:

1. When viewport_bottom < screen_bottom (e.g., 23 < 24), it:
   - Scrolls region(0..24) down by 1 (entire screen)
   - Draws new content at row 0
   - Moves viewport to start at y=1 (rows 1-23)

2. Result: Row 0 now contains inserted content but is OUTSIDE the viewport

3. On subsequent draw() calls:
   - Only viewport area (rows 1-23) gets updated
   - Row 0 remains with stale content from insert_before
   - Causes visible corruption

**Why rows-2 works:** With 2 empty rows below (viewport at rows 0-21, bottom=22):
- to_draw = 2
- Viewport moves to y=2 (rows 2-23)
- Main loop (lines 753-759) can scroll region 0..2 to insert content
- After all inserts, viewport naturally settles at correct position

**The Fix:** Treat viewport_height >= screen_height - 1 as "full screen case"
- Use the "borrow top line" approach (lines 714-732) for both full-screen and near-full-screen
- This keeps viewport fixed at y=0 and scrolls content up into scrollback
- Avoids the problematic viewport expansion that causes row 0 to be orphaned

### Step 5: Fix Applied and Tested ✓
**Change:** Modified `ratatui/src/terminal/terminal.rs:717`
```rust
// Before:
if self.viewport_area.height == self.last_known_area.height {

// After:
if self.viewport_area.height >= self.last_known_area.height.saturating_sub(1) {
```

**Testing Results:**
- `./run_repro.sh 0` (full screen): ✓ Clean output, no corruption
- `./run_repro.sh 1` (rows-1): ✓ Clean output, corruption FIXED!
- `./run_repro.sh 2` (rows-2): ✓ Clean output, still works correctly

All three cases now render properly without corruption.

### Step 6: Initial Fix Committed ✓
**Commit:** `5635daf3` in ratatui repo
**Branch:** fix-scrolling-regions-viewport
**Message:** Fix corruption bug in scrolling-regions insert_before with near-full viewport

### Step 7: Regression Test Added ✓
**File:** `tests/terminal.rs`
**Test:** `terminal_insert_before_near_full_viewport_no_corruption()`
**Description:** Tests that viewport height = screen_height - 1 doesn't cause corruption
**Result:** Test passes ✓

The test:
1. Creates a 24-row terminal with 23-row viewport (rows-1)
2. Draws initial content
3. Performs 5 insert_before operations (simulating scrolling output)
4. Redraws viewport with new content
5. Verifies first line shows new content, not leftover from insert_before

### Step 8: Final Verification ✓
**Test Commit:** `e072a776` in ratatui repo

**Full Test Suite:** All 12 terminal tests pass ✓
- terminal_insert_before_near_full_viewport_no_corruption ✓ (new test)
- All existing insert_before tests continue to pass ✓

**Manual Verification:**
- `./run_repro.sh 0` (full screen): Clean output ✓
- `./run_repro.sh 1` (rows-1): Clean output, no corruption ✓
- `./run_repro.sh 2` (rows-2): Clean output ✓

**Changes Review:**
- Only necessary changes made to `src/terminal/terminal.rs` (7 lines)
- Comprehensive regression test added
- No unnecessary changes to revert

## Summary

**Problem:** Ratatui's scrolling-regions feature had a bug where viewport height = (screen_height - 1) caused render corruption due to content being left outside the viewport boundaries.

**Root Cause:** The `insert_before_scrolling_regions()` function tried to expand the viewport downward when there was one empty row below, which moved the viewport's y coordinate and left row 0 orphaned with stale content.

**Solution:** Treat viewports within 1 row of full-screen the same as full-screen viewports, using the "borrow top line" approach that keeps the viewport fixed at y=0.

**Commits:**
- `5635daf3`: Fix corruption bug
- `e072a776`: Add regression test

**Status:** ✅ COMPLETE - Bug fixed, tested, and ready for upstream
