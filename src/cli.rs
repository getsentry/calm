use std::env;
use std::io;
use std::path::Path;
use std::io::Write;
use std::process;

use prelude::*;
use config::Config;
use ctx::Context;

use clap::{App, Arg, AppSettings};

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
            .arg(Arg::with_name("files")
                .index(1)
                .multiple(true)));
    let matches = app.get_matches_from_safe(args)?;
    let ctx = Context::new(config)?;

    if let Some(_sub_matches) = matches.subcommand_matches("update") {
        ctx.update()?;
    } else if let Some(sub_matches) = matches.subcommand_matches("lint") {
        let paths = sub_matches.values_of("files")
            .map(|values| values.map(|x| Path::new(x)).collect::<Vec<_>>());
        if !ctx.lint(paths.as_ref().map(|x| &x[..]))? {
            return Err(Error::from(ErrorKind::QuietExit(1)));
        }
    } else {
        unreachable!();
    }

    Ok(())
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
