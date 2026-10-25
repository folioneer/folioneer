# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.15.0] - 2026-10-25

### Added

- the owner answers an agent's ask to replace four with a transfer
- an agent asks to replace four typed transactions with a transfer
- the journals read a transfer as one, edited and deleted whole
- transfer a holding to another account from its row
- a transfer is read as one, with its rate and its other account
- an agent records a transfer between two accounts
- a transfer moves a holding between accounts at its cost
- an agent creates the account a statement names
- every account offers the month view, and has no update frequency
- an agent corrects any transaction it recorded, as the window does

### Fixed

- the shipped crates carry no known vulnerability
- an agent's removal never takes a whole transfer for one line
- a split is read even for an asset served no new close
- a write-off without a reason is never given one when corrected
- an agent's note is shown as written, its other text bounded
- a refused sale or write-off names what was held on its date

## [0.14.0] - 2026-10-20

### Added

- an agent records interest and a management fee
- record and correct a write-off from the holding's row
- an agent records a write-off with its reason
- a write-off takes a quantity out of a holding at its cost

### Fixed

- a holding says when Yahoo reports a split it has not recorded
- an agent reads the splits Yahoo reports that an account lacks
- prices filled from Yahoo are no longer divided by later splits

## [0.13.0] - 2026-10-18

### Added

- choose the currency your whole portfolio is reported in
- choose an account's wrapper, and group the accounts by it
- each account carries its tax wrapper
- a price not updated says why
- an outdated price stands out, and each account counts them
- an outdated price is judged per asset class
- an asset whose price you type by hand says so, not "locked"

### Fixed

- a device joining a shared portfolio takes its reference currency
- a refused wrapper says it is not in the wrapper lists
- an account in another currency counts in the portfolio total
- no return figure for a window a holding was not held through
- an agent records quantities below 0.00001 as sent

## [0.12.0] - 2026-10-17

### Added

- agents preview cash, dividends, free shares, prices, corrections
- an agent previews a purchase, a sale, an opening balance, a split
- moving the portfolio to an empty folder asks to confirm first
- an agent asks before changing a transaction you typed

### Fixed

- an agent's preview shows the right cash on every account
- the updater no longer carries an unpacking library with known flaws
- a cloud conflict copy of the sync header no longer pauses sync
- the session dialog says its removal takes your corrections too

## [0.11.0] - 2026-10-16

### Added

- Claude Desktop connects to Folioneer from one file in the release
- the window says what an agent records, whatever view is open
- the window asks you before an agent removes what you typed
- an agent asks to remove a transaction you typed, one at a time

### Fixed

- the network layer takes rustls 0.23.46, past four advisories
- a computer never joins another's moved sync folder as its own
- a call an agent gave up on withdraws what it asked the window
- an agent's tools do what their rules say, with a test each
- what an agent adds or records shows in the window at once

## [0.10.0] - 2026-10-12

### Added

- an agent reads an asset's price at a past date
- an agent lists an account's transactions
- an agent adds an asset the portfolio does not have
- an agent client knows a read from a recording
- an agent records a split and free shares

### Fixed

- a computer joins a folder after starting over or a move

## [0.9.0] - 2026-10-11

### Added

- an agent records the price of an asset at a date, shown as "Agent"
- an agent or a command records a deposit, a withdrawal, a dividend

## [0.8.0] - 2026-10-10

### Added

- what a connected agent did, and one action to remove it
- an agent corrects or cancels what its own session recorded
- a computer that left sync can start over from the join refusal
- agents connect to the open application on Windows

### Fixed

- a computer that only declared a currency pair can join
- a fee schedule at exactly 100 % a year is refused
- a paused fee schedule is not charged for its pause
- cancelling around a management fee is refused with its reason

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
