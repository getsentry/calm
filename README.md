# Calm

Calm is a meta development tool that helps you manage your development
environment in a way that you can automate as much as possible without
having to manually configure countless tools.  It automatically manages
linters, formatters and other tools so you don't have to.

Currently only supports linters.  For a default project see `demo`

<img width="574" alt="screen shot 2017-07-04 at 16 32 44" src="https://user-images.githubusercontent.com/7396/27834243-8523e93e-60d6-11e7-834b-e2abe7d062ab.png">

## Commands

``calm update``
  Updates the toolchain and links things.  Run this once to update the
  required toolchains.

``calm lint``
  Runs the configured linters and reports an exit status.

``calm hook``
  Manage hooks.  `--install` installs the git hook, `--uninstall`
  removes it.  Currently always runs the linter.
