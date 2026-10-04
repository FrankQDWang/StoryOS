# IndexedDB observations

Cell: returned records / write calls / stored records. Repeated reads count again. Stored totals are a post-window census.

Web windows with delayed editor remount can be lower bounds. FAIL retains the failed window counts. Per-store details are in operations.json.

| Operation | 30,000 | 300,000 | 1,000,000 | 3,000,000 |
| --- | --- | --- | --- | --- |
| web-create-chapter | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-create-volume | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-delete-chapter | 627 / 4 / 42 | 627 / 4 / 42 | 419 / 3 / 42 | 627 / 4 / 42 |
| web-delete-volume | 627 / 4 / 42 | 570 / 4 / 42 | 465 / 2 / 42 | 627 / 4 / 42 |
| web-export | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-input-save | 109 / 13 / 13 | 109 / 13 / 13 | 109 / 13 / 13 | 109 / 13 / 13 |
| web-open-project | 30 / 6 / 5 | 30 / 6 / 5 | 30 / 6 / 5 | 30 / 6 / 5 |
| web-proposal-accept | 875 / 13 / 42 | 875 / 13 / 42 | 875 / 13 / 42 | 822 / 13 / 42 |
| web-proposal-open | 268 / 0 / 38 | 268 / 0 / 38 | 268 / 0 / 38 | 268 / 0 / 38 |
| web-reorder-chapter | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-reorder-volume | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 | 0 / 0 / 42 |
| web-search | 0 / 0 / 13 | 0 / 0 / 13 | 0 / 0 / 13 | 0 / 0 / 13 |
| web-session-recovery | 88 / 3 / 13 | 88 / 3 / 13 | 88 / 3 / 13 | 88 / 3 / 13 |
| web-session-recovery-after-structure | FAIL; 1 / 0 / 42 | FAIL; 1 / 0 / 42 | FAIL; 1 / 0 / 42 | FAIL; 1 / 0 / 42 |
| web-switch-chapter | 102 / 4 / 13 | 102 / 4 / 13 | 102 / 4 / 13 | 102 / 4 / 13 |
| web-undo | 83 / 1 / 13 | 83 / 1 / 13 | 83 / 1 / 13 | 83 / 1 / 13 |
| web-unicode-insert | 184 / 13 / 18 | 184 / 13 / 18 | 184 / 13 / 18 | 184 / 13 / 18 |
