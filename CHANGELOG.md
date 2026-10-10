# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.1] - 2026-10-10

### Fixed

- starting sync over from the sync page can be completed
- the date field cancels its delayed close when it disappears

## [0.7.0] - 2026-10-10

### Added

- an agent can record purchases and sales, marked as its own
- an agent can read the portfolio through the open application
- the command line adds an asset of each kind and finds it by ISIN
- a new asset starts with its kind: listed, crypto or custom
- an asset created twice by mistake is refused

### Fixed

- a recording an agent started completes even if you disconnect
- a date an agent sends as a number is refused, not read as today
- the full-page purchase form no longer offers cash as an asset
- an archived asset can no longer be sold by a command or an agent
- a purchase on an archived asset no longer promises to unarchive
- just after midnight, a date of today is no longer refused
- joining a folder from a newer version says to update the app

## [0.6.0] - 2026-10-05

### Added

- numbers are typed and shown with the comma in French

### Fixed

- correcting a transaction keeps the figures you did not touch
- the fetch progress counts an asset whose price could not be saved
- editing a dividend shows what a dividend has
- the performance page keeps the year you chose when it refreshes

## [0.5.0] - 2026-10-03

### Added

- sync has its own page, with its health and grouped actions
- a transaction form says why it cannot be saved
- add an asset from the command line
- list accounts and assets from the command line
- the command line's help is easy to read

### Fixed

- a deposit or withdrawal can be corrected after cash was withdrawn
- the core refuses a purchase or a sale of cash

## [0.4.0] - 2026-09-29

### Added

- explain why lifetime performance shows a dash
- record holdings from PowerShell and WSL on Windows
- record holdings, buys and sells from the command line on Linux

### Fixed

- switching accounts or assets fast never shows the one left behind
- the journal's cash balance counts interest on the cash line

## [0.3.0] - 2026-09-27

### Added

- prices are entered by hand; the app no longer fetches them

### Fixed

- the sync date shows the last sync that succeeded
- lists stay in place while they refresh after a change
- the daily price download runs when the app is an AppImage

## [0.2.0] - 2026-09-27

### Added

- exchange rates update by themselves at every launch

### Fixed

- the Settings page scrolls when it is taller than the window

## [0.1.0] - 2026-09-20

### Added

- track your investment portfolio privately, on your own computers
