# TODO-095 — (fullstack) — Import presets for the brokers used across Europe

From gh285 (owner, 2026-10-17), which this entry closes when it ships. TODO-025 brings a CSV import where the user maps the columns. For a first import the mapping is the hard part, and a few brokers used in many countries have exports of a known shape. National brokers and insurers come in phase 2 (gh287). Waits on TODO-025.

**User value:** A user of one of these brokers imports years of history in a few clicks, wherever they live.
**Done when:** Presets exist for Interactive Brokers, DEGIRO, Trade Republic and Scalable Capital, each tested against an anonymised sample export; a preset is a data file, adding one needs no code change, and the contribution guide (TODO-105) says how; a file that does not match its preset is refused with the columns that differ.
**Design:** none
**Open questions:**

- [x] Which export of each broker (transactions CSV, account statement, Flex Query for Interactive Brokers)? — Each broker's transactions CSV from its client area; Interactive Brokers through a Flex Query (CSV), which the guide describes how to set up; PDF statements are no preset's — an agent reads them (TODO-067); a broker with no transactions CSV gets no preset (gh285). (owner, 2026-10-18)
