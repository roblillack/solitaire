# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added

- First release: a complete Klondike Solitaire in the Windows 3.1 style, built
  on the [Saudade](https://github.com/roblillack/saudade) toolkit.
- **Gameplay**: a deterministic, seeded deal; build the tableau down in
  alternating colours and the foundations up in suit; turn / recycle the stock
  in draw-one or draw-three mode; full undo history.
- **Controls**: drag a card or a valid run between piles; double-click or
  right-click a card to send it straight to its foundation; Space / Enter turns
  the stock.
- **Auto-finish**: once the outcome is decided, one action plays the rest of the
  cards home — followed by the classic bouncing-cards victory cascade and a
  "deal again?" prompt.
- **Menu bar** (Game / Help) with Alt-mnemonics and `Ctrl`-key accelerators, and
  a modal About box.
- **Pixel-snapshot tests** of the UI and a deterministic rules-engine test suite.
- A scripted **demo** that records `demo.gif` (`--features recorder`) or plays
  itself live in a window, and a **screenshots** example for the README images.

<!-- next-url -->
[Unreleased]: https://github.com/roblillack/solitaire/commits/main
