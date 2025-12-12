# Bug report

2025-12-10

## Issue switching from input to command mode by using the `/` key while in `Insert` mode

### Description

Pressing `/` while in `Insert` mode causes the cursor to flicker to two locations for a moment, one perhaps 4-5 rows down and a similar number of cols to the right. The other location is on the right side of the input box border.

After less than a second the cursor returns to the expected position just after the `/` in the input area.
