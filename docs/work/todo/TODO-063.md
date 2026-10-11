# TODO-063 — (backend) — An agent adds an asset the portfolio does not have

Raised on 2026-10-10 while comparing the three interfaces: the command line adds an asset (CLI-026), an agent cannot. Rebuilding an old account stops at the first instrument the owner no longer holds and never entered.

**Size:** M — one pull request: a tool, a rule of the agent's spec, a line of the session dialog and a line of the connection dialog (owner, 2026-10-11).
**User value:** An agent reading an old statement adds the instruments it names — a listed one by its ISIN, a custom one by its name — instead of stopping to ask the owner to create each.
**Done when:** a tool adds an asset of each kind through the rules of the window and of the command line (AST-031, AST-032), refusing the same asset twice; an added asset takes one place of the session's limit; the session dialog says how many assets the session added and names the last one; the connection dialog names adding assets in what a yes covers.
**Design:** none
**Open questions:**

- [x] An asset has no mark today. Should the assets table say which assets an agent added, as the journal does for transactions? — No mark for now: the session dialog gains a line, how many assets the session added and the last one's name, and nothing is kept once the session ends. A way to mark this change as well, as for a price, is DEBT-119 (owner, 2026-10-11). The line is a change of a screen: its mock-up is drawn and shown when the entry's turn comes.
- [x] The connection dialog lists what a yes covers in two lines, a read and a recording "each marked as recorded by it": what does it say once an agent adds assets? — A third line of its own: "add assets your portfolio does not have yet — counted in the same 500;" ("ajouter des actifs que votre portefeuille n'a pas encore — comptés dans les mêmes 500 ;"). An added asset takes one place of the session's limit, like a price (owner, 2026-10-11).
- [x] Done when ended with "the capabilities table says so (TODO-064)", and TODO-064 is not queued: does this entry wait for it? — No: the clause moves to TODO-064, whose table lists the tool when it is built (owner, 2026-10-11).
