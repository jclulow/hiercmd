use anyhow::{bail, Result};
use std::future::Future;
use std::pin::Pin;

pub mod table;

pub mod prelude {
    pub use super::table::Row;
    pub use super::{args, cmd, no_args, sel, Level};
}

/**
 * Context object available to each command handler
 *
 * Automatically implemented for all Send + Sync types.
 */
pub trait LevelContext: Send + Sync + 'static {}

impl<T: 'static> LevelContext for T where T: Send + Sync {}

pub type Caller<C> =
    fn(Level<C>) -> Pin<Box<(dyn Future<Output = Result<()>>)>>;

#[derive(Clone)]
struct CommandInfo<C: LevelContext> {
    name: String,
    alias: Option<String>,
    desc: String,
    func: Caller<C>,
    visible: bool,
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

pub struct Level<C: LevelContext> {
    names: Vec<String>,
    args: Option<Vec<String>>,
    commands: Vec<CommandInfo<C>>,
    options: getopts::Options,
    table: Option<table::TableBuilder>,
    private: C,
}

impl<C: LevelContext> Level<C> {
    /**
     * Create a new top-level command handling object.  The `name` is the
     * project command name, and `private` is the consumer-provided context
     * object to be passed to other level handlers.
     */
    pub fn new(name: &str, private: C) -> Level<C> {
        Level::new_sub(vec![name.to_string()], private, None)
    }

    fn new_sub(
        names: Vec<String>,
        private: C,
        args: Option<Vec<String>>,
    ) -> Level<C> {
        let mut options = getopts::Options::new();
        options.parsing_style(getopts::ParsingStyle::StopAtFirstFree);
        options.optflag("", "help", "usage information");

        Level {
            names,
            args,
            commands: Vec::new(),
            options,
            table: None,
            private,
        }
    }

    /**
     * Access the consumer-provided context object which is passed to all level
     * handlers.
     */
    pub fn context(&self) -> &C {
        &self.private
    }

    pub fn context_mut(&mut self) -> &mut C {
        &mut self.private
    }

    /**
     * Add a column to the table definition for this level.  The first time this
     * is called for a level, table mode is activated.  Subsequent calls
     * continue to add column definitions.
     */
    pub fn add_column(&mut self, name: &str, width: usize, default: bool) {
        if self.table.is_none() {
            self.table = Some(table::TableBuilder::default());

            /*
             * Include the standard tabular data formatting options for this
             * level.  They will be handled as part of printing the table after
             * option parsing.
             */
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

    /**
     * Add a handler for a next level sub-command.  The `name` is what the user
     * would pass on the command line to nominate the sub-command.  The `desc`
     * is descriptive text that will show up in usage information.  The `func`
     * callback is a function pointer for an asynchronous function wrapped by
     * the `cmd!()` macro.
     */
    pub fn cmd(
        &mut self,
        name: &str,
        desc: &str,
        func: Caller<C>,
    ) -> Result<()> {
        self.cmd_common(name, None, desc, func, true)
    }

    /**
     * Add a handler for a next level sub-command with a short alias.  Otherwise
     * identical to the `cmd()` method.
     */
    pub fn cmda(
        &mut self,
        name: &str,
        alias: &str,
        desc: &str,
        func: Caller<C>,
    ) -> Result<()> {
        self.cmd_common(name, Some(alias), desc, func, true)
    }

    /**
     * Add a handler for a next level sub-command that is not shown in the usage
     * output.  Otherwise identical to the `cmd()` method.
     */
    pub fn hcmd(
        &mut self,
        name: &str,
        desc: &str,
        func: Caller<C>,
    ) -> Result<()> {
        self.cmd_common(name, None, desc, func, false)
    }

    fn cmd_common(
        &mut self,
        name: &str,
        alias: Option<&str>,
        desc: &str,
        func: Caller<C>,
        visible: bool,
    ) -> Result<()> {
        if self.commands.iter().any(|ci| ci.name == name) {
            bail!("duplicate command \"{}\"", name);
        }
        self.commands.push(CommandInfo {
            name: name.to_string(),
            alias: alias.map(|s| s.to_string()),
            desc: desc.to_string(),
            func,
            visible,
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

    /**
     * If this command level is a terminal node, just parse arguments and the
     * optional table.  This should be called via the `args()!` macro, or if the
     * command does not expect any arguments, the `no_args()!` macro.
     * Automatically handles `--help` and any table output formatting options.
     */
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

                let table = if let Some(mut table) = self.table.take() {
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

                    Some(table)
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

    /**
     * Parse options for this command level and select the next command.  The
     * best way to call this routine is using the `sel!()` macro, which handles
     * the early return and exit-on-failure conditions automatically.
     */
    pub fn select(mut self) -> Result<Option<Selection<C>>> {
        if self.commands.is_empty() {
            bail!("no commands provided by consumer");
        }

        let args = args!(self);

        /*
         * Determine which command the user is trying to run.
         */
        if args.matches.free.is_empty() {
            self.usage();
            bail!("choose a command");
        }

        let usage = self.gen_usage();

        let want = args.matches.free[0].as_str();
        for command in self.commands {
            if command.name != want {
                if let Some(alias) = &command.alias {
                    if alias.as_str() != want {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            return Ok(Some(Selection {
                names: self.names,
                private: self.private,
                command,
                matches: args.matches,
            }));
        }

        print!("{}", usage);
        bail!("command \"{}\" not understood", &args.matches.free[0]);
    }

    pub fn usage(&self) {
        print!("{}", self.gen_usage());
    }

    fn gen_usage(&self) -> String {
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
                if !cmd.visible {
                    continue;
                }
                let cn = if let Some(alias) = &cmd.alias {
                    format!("{} ({})", cmd.name, alias)
                } else {
                    cmd.name.to_string()
                };
                out.push_str(&format!("    {:<19} {}\n", cn, cmd.desc));
            }
        }
        let mut out = self.options.usage(&out);
        out.push('\n');
        if let Some(table) = &self.table {
            let cols = table.column_names();
            if !cols.is_empty() {
                out.push_str("Columns:\n");
                for col in cols.iter() {
                    out.push_str(&format!("    {:<19}\n", col));
                }
            }
            out.push('\n');
        }
        out
    }
}

pub struct Selection<C: LevelContext> {
    private: C,
    names: Vec<String>,
    command: CommandInfo<C>,
    matches: getopts::Matches,
}

impl<C: LevelContext> Selection<C> {
    pub fn opts(&self) -> &getopts::Matches {
        &self.matches
    }

    pub async fn run(self) -> Result<()> {
        let mut names = self.names.clone();
        names.push(self.command.name.clone());
        let l = Level::new_sub(
            names,
            self.private,
            Some(self.matches.free[1..].to_vec()),
        );
        (self.command.func)(l).await
    }

    pub fn context(&self) -> &C {
        &self.private
    }

    pub fn context_mut(&mut self) -> &mut C {
        &mut self.private
    }
}

pub struct Arguments {
    matches: getopts::Matches,
    table: Option<table::TableBuilder>,
}

impl Arguments {
    pub fn opts(&self) -> &getopts::Matches {
        &self.matches
    }

    pub fn args(&self) -> &[String] {
        &self.matches.free
    }

    pub fn table(&self) -> table::Table {
        self.table.as_ref().unwrap().build()
    }
}
