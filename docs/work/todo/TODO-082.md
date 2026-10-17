# TODO-082 — (frontend) — Moving the portfolio to another folder asks to confirm

Raised at the hand-over of the 0.10.0 batch and decided with the owner on 2026-10-12. Since DEBT-106, "Change folder" to a folder that holds no portfolio is a new beginning, as starting over is: every other computer has to rejoin from a fresh installation (SYN-074). "Start over" says so and asks to confirm (SYN-071); "Change folder" is a folder, Cancel and Save, and says nothing.

A confirmation like starting over's, shown only when the folder chosen holds no portfolio: it says the other computers will have to rejoin from a fresh installation, and keeping the current folder is the action in focus. A folder that holds the portfolio the computer follows is changed to as today, with no confirmation.

**Size:** M — one pull request: a dialog, its sentences in two languages, its screenshots, a rule of the sync spec.
**User value:** The owner knows what moving the portfolio costs his other computers before he moves it.
**Done when:** choosing a folder that holds no portfolio asks to confirm, saying the other computers rejoin from a fresh installation, keeping the current folder by default; a folder that holds the portfolio followed asks nothing; SYN-074 says so.
**Design:** none
**Open questions:**

- [x] A new confirmation dialog is a screen change: its mock-up and its sentences in two languages, when? — Drawn in the next batch's opening block on the start-over confirmation's model, keeping the current folder in focus, and validated there (owner, 2026-10-17).
