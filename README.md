# Calm

Calm is a meta development tool that helps you manage your development
environment in a way that you can automate as much as possible without
having to manually configure countless tools.  It automatically manages
linters, formatters and other tools so you don't have to.

If you don't have calm yet you can easily install it:

```
$ curl -sL https://ensure.getcalm.org/ | bash
```

## Commands

``calm``
  When run without any arguments `calm` runs in default mode where it
  will configure all the tools silently in the background, runs them
  and reports the status to stdout.

``calm down``
  In this mode `calm` will write out config files for the individual
  tools into your repository based on the reference sources.  This
  way if you have an editor or another tool that can pick up on these
  config files they should work as you expect.  Calm will also inject
  itself into your git config if wanted.

``calm undo``
  If you can undo calm will undo the changes from `calm down`.

``calm ci``
  In this mode calm will detect common CI environments and write reports
  to github pull request comments or other appropriate sources.  How
  calm behaves in CI environments is configured in the `.calm.yml` file.
