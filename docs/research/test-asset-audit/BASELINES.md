# Targeted preparation and baselines

These checks prepare the later random mutation experiment. They are not mutation results and do not count toward the required 30 samples. Compact runner records are in baseline-evidence.json. Times in those records use the runner UTC timestamps; the user calendar is Asia/Singapore.

| Command or selected scope | Outcome | Evidence |
|---|---|---|
| make verify-status BASE=origin/main | Missing policy evidence; requested verify-policy only | Clean baseline 479224809cdaae997cda51cb8853e3fafa242b65 |
| make verify-targeted CHECK=verify-policy | SOURCE-CHANGED, exit 2; printed Python groups passed, but the managed run is not PASS | 62fab459d82d4bb29a1967e8f6fcbe95; 389.26 s; report files changed during the run |
| make release-package | PASS; install, TypeScript, Vite, Rust release build, offline Server/Worker checks | 88f7e20a28e345b380ae15c2133bd0e4; 352.63 s |
| node-contract/protocol-http.integration.test.ts | PASS, 7 tests | 6f092501aea044ad9f3e6a57d5d3aaf7; 5.32 s |
| browser-source/chapter-navigation.integration.test.ts and list-open.integration.test.ts | PASS, 2 tests | be14876b3d724cafa7e5c66ab6fe4f83; 21.75 s |

Both test commands use the existing repository step wrapper, with the stage names audit-startup-baseline and audit-browser-baseline. Each invokes pnpm --dir apps/web exec vitest run with one project and explicit files. No runner or Make target was added.

One parallel browser-baseline launch was refused because the repository execution budget was busy with the startup baseline. It ran successfully after that command exited. Run managed checks serially on this worktree.

The packaged product source equals the audit baseline. Its Git build identity includes the report commits through d1d8393cd32156f0485b5982a2cf37f5515fdcc3. Mutations to Rust must rebuild the affected binary; running a previously packaged binary cannot validate a source mutation. Package construction requires a clean worktree. Plan a temporary build with the existing manifest binding for mutation runs, restore binaries and sources afterward, and do not call a stale-binary pass coverage evidence.
