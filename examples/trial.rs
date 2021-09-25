use anyhow::{bail, Result};

use hiercmd::prelude::*;

async fn do_thing_list(mut l: Level) -> Result<()> {
    l.add_column("name", 16, true);
    l.add_column("number", 6, false);

    let mut a = args!(l);
    let t = a.table();

    let mut r = Row::default();
    r.add_str("name", "Thing One");
    r.add_u64("number", 1);
    t.add_row(r);

    let mut r = Row::default();
    r.add_str("name", "Thing Two");
    r.add_u64("number", 2);
    t.add_row(r);

    print!("{}", t.output()?);
    Ok(())
}

async fn do_thing(mut l: Level) -> Result<()> {
    l.cmd(cmd!("list", "list things", do_thing_list))?;
    sel!(l).run().await
}

async fn do_info(mut l: Level) -> Result<()> {
    no_args!(l);
    bail!("nyi");
}

async fn do_nothing(mut l: Level) -> Result<()> {
    no_args!(l);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut l = hiercmd::Level::new("trial");
    l.cmd(cmd!("info", "get information", do_info))?;
    l.cmd(cmd!("thing", "manage things", do_thing))?;
    l.cmd(cmd!("nothing", "do nothing", do_nothing))?;

    l.opts().optflag("x", "", "extend");

    let s = sel!(l);
    if s.opts().opt_present("x") {
        println!("eXtended!");
    }

    s.run().await
}
