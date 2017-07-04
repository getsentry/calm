use std::env;
use std::io;
use std::path::Path;
use std::io::Write;
use std::process;

use prelude::*;
use config::Config;
use ctx::Context;
use utils::whatchanged::get_changed_files;

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
        .arg(Arg::with_name("no_update")
             .long("no-update")
             .help("Disables the update step"))
        .subcommand(App::new("update")
            .about("Update all calm toolchains"))
        .subcommand(App::new("lint")
            .about("Lint all files in the project or a subset")
            .arg(Arg::with_name("fmt")
                 .long("format")
                 .short("f")
                 .value_name("FORMAT")
                 .possible_values(&["human", "simple", "checkstyle"])
                 .help("Sets the output format"))
            .arg(Arg::with_name("changed_files")
                 .long("changed-files")
                 .help("Lint files changed in the current git work tree."))
            .arg(Arg::with_name("files")
                .index(1)
                .multiple(true)));

    let matches = app.get_matches_from_safe(args)?;
    let mut ctx = Context::new(config)?;

    if let Some(_sub_matches) = matches.subcommand_matches("update") {
        cmd_update_installation(&mut ctx)
    } else if let Some(sub_matches) = matches.subcommand_matches("lint") {
        cmd_lint(&ctx, sub_matches)
    } else {
        unreachable!();
    }
}

fn cmd_update_installation(ctx: &mut Context) -> Result<()> {
    ctx.pull_dependencies()?;
    ctx.update()?;
    Ok(())
}

fn cmd_lint(ctx: &Context, matches: &ArgMatches) -> Result<()> {
    let format = matches.value_of("fmt").unwrap_or("human");
    let changed_files;
    let paths: Option<Vec<&Path>>;

    if matches.is_present("changed_files") {
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
    if report.has_errors() {
        Err(Error::from(ErrorKind::QuietExit(1)))
    } else {
        Ok(())
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
