use anyhow::{bail, Result};
use std::future::Future;
use std::pin::Pin;

pub mod table;

pub type Caller = fn(Level) -> Pin<Box<(dyn Future<Output = Result<()>>)>>;

#[derive(Clone)]
pub struct CommandInfo {
    pub name: String,
    pub desc: String,
    pub func: Caller,
}

#[macro_export]
macro_rules! cmd {
    ($func:ident) => {
        |level| Box::pin($func(level))
    };
}

#[macro_export]
macro_rules! sel {
    ($level:ident) => {
        match $level.select() {
            Ok(None) => return Ok(Default::default()),
            Ok(Some(sel)) => sel,
            Err(e) => {
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            }
        }
    };
}

#[macro_export]
macro_rules! args {
    ($level:ident) => {
        match $level.parse() {
            Ok(None) => return Ok(Default::default()),
            Ok(Some(mat)) => mat,
            Err(e) => {
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            }
        }
    };
}

#[macro_export]
macro_rules! no_args {
    ($level:ident) => {
        match $level.parse() {
            Ok(None) => return Ok(Default::default()),
            Ok(Some(args)) => {
                if !args.opts().free.is_empty() {
                    eprintln!("ERROR: unexpected arguments");
                    std::process::exit(1);
                }
                args
            }
            Err(e) => {
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            }
        }
    };
}

pub mod prelude {
    pub use super::table::Row;
    pub use super::{args, cmd, no_args, sel, Level};
}

pub struct Level {
    names: Vec<String>,
    args: Option<Vec<String>>,
    commands: Vec<CommandInfo>,
    options: getopts::Options,
    table: Option<table::TableBuilder>,
}

impl Level {
    pub fn new(name: &str) -> Level {
        Level::new_sub(vec![name.to_string()], None)
    }

    fn new_sub(names: Vec<String>, args: Option<Vec<String>>) -> Level {
        let mut options = getopts::Options::new();
        options.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
        options.optflag("", "help", "usage information");

        Level {
            names,
            args,
            commands: Vec::new(),
            options,
            table: None,
        }
    }

    pub fn add_column(&mut self, name: &str, width: usize, default: bool) {
        if self.table.is_none() {
            self.table = Some(table::TableBuilder::default());

            let o = &mut self.options;
            o.optopt("s", "", "sort by column list (asc)", "COLUMNS");
            o.optopt("S", "", "sort by column list (desc)", "COLUMNS");
            o.optopt("o", "", "output column list", "COLUMNS");
            //opts.optflag("a", "", "all fields");
            o.optflag("H", "", "no header");
            o.optflag("p", "", "print numbers in parseable (exact) format");
        }

        self.table
            .as_mut()
            .unwrap()
            .add_column(name, width, default);
    }

    pub fn cmd(&mut self, name: &str, desc: &str, func: Caller) -> Result<()> {
        if self.commands.iter().any(|ci| ci.name == name) {
            bail!("duplicate command \"{}\"", name);
        }
        self.commands.push(CommandInfo {
            name: name.to_string(),
            desc: desc.to_string(),
            func,
        });
        Ok(())
    }

    pub fn optflagmulti(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
    ) {
        self.options.optflagmulti(short_name, long_name, desc);
    }

    pub fn optmulti(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
    ) {
        self.options.optmulti(short_name, long_name, desc, hint);
    }

    pub fn optflag(&mut self, short_name: &str, long_name: &str, desc: &str) {
        self.options.optflag(short_name, long_name, desc);
    }

    pub fn optopt(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
    ) {
        self.options.optopt(short_name, long_name, desc, hint);
    }

    pub fn parse(&mut self) -> Result<Option<Arguments>> {
        let res = if let Some(args) = &self.args {
            self.options.parse(args)
        } else {
            self.options.parse(std::env::args().skip(1))
        };

        match res {
            Ok(res) => {
                if res.opt_present("help") {
                    self.usage();
                    return Ok(None);
                }

                let table = if let Some(table) = self.table.as_mut() {
                    table
                        .output_from_list(res.opt_str("o").as_deref())
                        .sort_from_list_asc(res.opt_str("s").as_deref())
                        .sort_from_list_desc(res.opt_str("S").as_deref())
                        .show_header(!res.opt_present("H"))
                        .tab_separated(res.opt_present("H"))
                        .parseable(res.opt_present("p"));

                    let mcn = table.missing_column_names();
                    if !mcn.is_empty() {
                        self.usage();
                        eprintln!(
                            "ERROR: invalid column names: {}",
                            mcn.join(", ")
                        );
                        std::process::exit(1);
                    }

                    Some(table.build())
                } else {
                    None
                };

                Ok(Some(Arguments {
                    matches: res,
                    table,
                }))
            }
            Err(e) => {
                self.usage();
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            }
        }
    }

    pub fn select(&mut self) -> Result<Option<Selection>> {
        let args = args!(self);

        /*
         * Determine which command the user is trying to run.
         */
        if args.matches.free.is_empty() {
            self.usage();
            bail!("choose a command");
        }

        for cmd in &self.commands {
            if cmd.name == args.matches.free[0] {
                return Ok(Some(Selection {
                    names: self.names.clone(),
                    command: cmd.clone(),
                    matches: args.matches,
                }));
            }
        }

        self.usage();
        bail!("command \"{}\" not understood", &args.matches.free[0]);
    }

    pub fn usage(&self) {
        let mut out = "Usage:".to_string();
        for n in self.names.iter() {
            out.push_str(&format!(" {}", n));
        }
        if !self.commands.is_empty() {
            out.push_str(" COMMAND");
        }
        out.push_str(" [OPTS] [ARGS...]\n");
        if !self.commands.is_empty() {
            out.push_str("\nCommands:\n");
            for cmd in self.commands.iter() {
                out.push_str(&format!("    {:<19} {}\n", cmd.name, cmd.desc));
            }
        }
        println!("{}", self.options.usage(&out));
        if let Some(table) = &self.table {
            let mut out = String::new();
            let cols = table.column_names();
            if !cols.is_empty() {
                out.push_str("Columns:\n");
                for col in cols.iter() {
                    out.push_str(&format!("    {:<19}\n", col));
                }
            }
            println!("{}", out);
        }
    }
}

pub struct Selection {
    names: Vec<String>,
    command: CommandInfo,
    matches: getopts::Matches,
}

impl Selection {
    pub fn opts(&self) -> &getopts::Matches {
        &self.matches
    }

    pub async fn run(&self) -> Result<()> {
        let mut names = self.names.clone();
        names.push(self.command.name.clone());
        let l = Level::new_sub(names, Some(self.matches.free[1..].to_vec()));
        (self.command.func)(l).await
    }
}

pub struct Arguments {
    matches: getopts::Matches,
    table: Option<table::Table>,
}

impl Arguments {
    pub fn opts(&self) -> &getopts::Matches {
        &self.matches
    }

    pub fn args(&self) -> &[String] {
        &self.matches.free
    }

    pub fn table(&mut self) -> &mut table::Table {
        self.table.as_mut().unwrap()
    }
}
