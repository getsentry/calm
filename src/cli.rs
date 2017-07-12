use std::env;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process;

use prelude::*;
use config::Config;
use ctx::Context;
use report::Format;
use utils::whatchanged::get_changed_files;
use utils::hooks::HookManager;
use utils::watch::watch_files;
use utils::ui::clear_term;

use console::style;
use clap::{App, Arg, AppSettings, ArgMatches};

const ABOUT: &'static str = "
Calm makes your development experience delightful.";

fn execute(args: Vec<String>, config: Config) -> Result<()> {
    let app = App::new("calm")
        .about(ABOUT)
        .max_term_width(100)
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .global_setting(AppSettings::UnifiedHelpMessage)
        .subcommand(App::new("update")
            .about("Update all calm toolchains"))
        .subcommand(App::new("clear-cache")
            .about("Clears the runtime cache"))
        .subcommand(App::new("hook")
            .about("Manages the git hook integration")
            .arg(Arg::with_name("install")
                 .long("install")
                 .help("Installs a pre-commit hook for git"))
            .arg(Arg::with_name("pre_commit")
                 .long("exec-pre-commit")
                 .help("Execute the pre-commit hook"))
            .arg(Arg::with_name("uninstall")
                 .long("uninstall")
                 .help("Uninstalls a pre-commit hook for git")))
        .subcommand(App::new("lint")
            .about("Lint all files in the project or a subset")
            .arg(Arg::with_name("fmt")
                 .long("format")
                 .short("f")
                 .value_name("FORMAT")
                 .possible_values(&["human", "human-extended", "simple", "checkstyle"])
                 .help("Sets the output format"))
            .arg(Arg::with_name("watch")
                 .long("watch")
                 .help("Keeps watching for linting errors."))
            .arg(Arg::with_name("all")
                 .long("all")
                 .help("This will lint all files.  This is the default if no paths are \
                        given and watching is not enabled but has to be explicitly \
                        provided for watch as otherwise watch will only lint the \
                        changed file."))
            .arg(Arg::with_name("changed_files")
                 .long("changed-files")
                 .help("Lint files changed in the current git work tree."))
            .arg(Arg::with_name("files")
                .index(1)
                .multiple(true)))
        .subcommand(App::new("format")
            .about("Format the given files with configured formatters.")
            .arg(Arg::with_name("write")
                 .long("write")
                 .help("Write the changes back instead of printing a diff."))
            .arg(Arg::with_name("changed_files")
                 .long("changed-files")
                 .help("Format files changed in the current git work tree."))
            .arg(Arg::with_name("files")
                .index(1)
                .multiple(true)))
        .subcommand(App::new("which")
            .about("Given a command returns the path where it lives.")
            .arg(Arg::with_name("cmd")
                 .index(1)
                 .value_name("COMMAND")
                 .required(true)
                 .help("The command to find")));

    let matches = app.get_matches_from_safe(args)?;
    let mut ctx = Context::new(config)?;

    if let Some(_sub_matches) = matches.subcommand_matches("update") {
        cmd_update_installation(&mut ctx)
    } else if let Some(_sub_matches) = matches.subcommand_matches("clear-cache") {
        cmd_clear_cache(&ctx)
    } else if let Some(sub_matches) = matches.subcommand_matches("lint") {
        if sub_matches.is_present("watch") {
            cmd_lint_watch(&ctx, sub_matches)
        } else {
            cmd_lint(&ctx, sub_matches)
        }
    } else if let Some(sub_matches) = matches.subcommand_matches("format") {
        cmd_format(&ctx, sub_matches)
    } else if let Some(sub_matches) = matches.subcommand_matches("hook") {
        cmd_hook(&ctx, sub_matches)
    } else if let Some(sub_matches) = matches.subcommand_matches("which") {
        cmd_which(&ctx, sub_matches)
    } else {
        unreachable!();
    }
}

fn cmd_update_installation(ctx: &mut Context) -> Result<()> {
    ctx.pull_dependencies()?;
    ctx.update()?;
    Ok(())
}

fn cmd_clear_cache(ctx: &Context) -> Result<()> {
    ctx.clear_cache()?;
    Ok(())
}

fn cmd_lint(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let all = matches.is_present("all");
    let format = matches.value_of("fmt").unwrap_or("human");
    let changed_files;
    let paths: Option<Vec<&Path>>;

    if all {
        paths = None;
    } else  if matches.is_present("changed_files") {
        changed_files = get_changed_files()?;
        if changed_files.is_empty() {
            return Ok(());
        }
        paths = Some(changed_files.iter().map(|x| x.as_path()).collect());
    } else {
        paths = matches.values_of("files")
            .map(|values| values.map(|x| Path::new(x)).collect::<Vec<_>>());
    }

    let report = ctx.lint(paths.as_ref().map(|x| &x[..]))?;
    ctx.clear_log();
    report.print(format.parse().unwrap())?;
    if report.did_fail() {
        Err(Error::from(ErrorKind::QuietExit(1)))
    } else {
        Ok(())
    }
}

fn cmd_lint_watch(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let all = matches.is_present("all");
    let format = matches.value_of("fmt").unwrap_or("human-extended");

    if matches.is_present("files") {
        fail!("Lint watcher does not accept any arguments");
    }

    clear_term();
    println_stderr!("Linting on changes ...");
    let fmt = format.parse().unwrap();
    watch_files(ctx.base_dir(), &|path: &Path| -> Result<()> {
        if ctx.is_lintable_file(path)? {
            clear_term();
            println_stderr!("Detected change in {}", style(path.display()).cyan());
            let report = if all {
                ctx.lint(None)
            } else {
                ctx.lint(Some(&[path][..]))
            }?;
            ctx.clear_log();
            clear_term();
            if !all {
                println_stderr!("Results for {}:", style(path.display()).cyan());
                println_stderr!("");
            }
            report.print(fmt)?;
        }
        Ok(())
    })?;

    unreachable!();
}

fn cmd_format(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let changed_files;
    let paths: Vec<&Path>;

    if matches.is_present("changed_files") {
        changed_files = get_changed_files()?;
        if changed_files.is_empty() {
            return Ok(());
        }
        paths = changed_files.iter().map(|x| x.as_path()).collect();
    } else if let Some(files) = matches.values_of("files") {
        paths = files.map(|x| Path::new(x)).collect::<Vec<_>>();
    } else {
        return Ok(());
    }

    let rv = ctx.format(&paths)?;
    ctx.clear_log();
    if matches.is_present("write") {
        rv.apply()?;
    } else {
        rv.print_diff()?;
    }
    Ok(())
}

fn cmd_hook(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let mgr = HookManager::new()?;
    if matches.is_present("install") {
        mgr.install_hooks()?;
        println!("Enabled hooks.");
    } else if matches.is_present("uninstall") {
        mgr.uninstall_hooks()?;
        println!("Disabled hooks.");
    } else if matches.is_present("pre_commit") {
        let changed_files = get_changed_files()?;
        if changed_files.is_empty() {
            return Ok(());
        }

        let paths: Vec<_> = changed_files.iter().map(|x| x.as_path()).collect();

        // format
        ctx.format(&paths)?.apply()?;

        // lint
        let report = ctx.lint(Some(&paths[..]))?;
        ctx.clear_log();
        report.print(Format::Human)?;
        if report.did_fail() {
            return Err(Error::from(ErrorKind::QuietExit(1)));
        }
    } else {
        let status = mgr.status()?;
        println!("Current hook status:");
        println!("  pre-commit hook: {}", if status.pre_commit_installed {
            "installed"
        } else {
            "not installed"
        });
    }
    Ok(())
}

fn cmd_which(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let cmd = matches.value_of("cmd").unwrap();
    if let Some(path) = ctx.find_command(cmd)? {
        println!("{}", path.display());
        Ok(())
    } else {
        Err(ErrorKind::QuietExit(1).into())
    }
}

fn run() -> Result<()> {
    execute(env::args().collect(), Config::from_env()?)
}

/// Helper that renders an error to stderr.
pub fn print_error(err: &Error) {
    use std::error::Error;

    if let &ErrorKind::Clap(ref clap_err) = err.kind() {
        clap_err.exit();
    }

    writeln!(&mut io::stderr(), "error: {}", err).ok();
    let mut cause = err.cause();
    while let Some(the_cause) = cause {
        writeln!(&mut io::stderr(), "  caused by: {}", the_cause).ok();
        cause = the_cause.cause();
    }

    if env::var("RUST_BACKTRACE") == Ok("1".into()) {
        writeln!(&mut io::stderr(), "").ok();
        writeln!(&mut io::stderr(), "{:?}", err.backtrace()).ok();
    }
}

pub fn main() {
    match run() {
        Ok(()) => process::exit(0),
        Err(err) => {
            if let &ErrorKind::QuietExit(code) = err.kind() {
                process::exit(code);
            } else {
                print_error(&err);
                process::exit(1);
            }
        }
    }
}
