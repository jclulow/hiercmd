use anyhow::Result;

use hiercmd::prelude::*;

async fn do_thing_list(mut l: Level<()>) -> Result<()> {
    l.add_column("name", 16, true);
    l.add_column("number", 6, false);

    l.usage_args(None);

    let a = args!(l);
    let mut t = a.table();

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

async fn do_thing(mut l: Level<()>) -> Result<()> {
    l.cmd("list", "list things", cmd!(do_thing_list))?;
    sel!(l).run().await
}

async fn do_info(mut l: Level<()>) -> Result<()> {
    l.usage_args(Some("[THING...]"));
    let a = args!(l);
    for (i, arg) in a.args().iter().enumerate() {
        println!("[{:02}] {}", i, arg);
    }
    Ok(())
}

async fn do_nothing(mut l: Level<()>) -> Result<()> {
    no_args!(l);
    Ok(())
}

async fn do_check(mut l: Level<()>) -> Result<()> {
    l.usage_args(Some("WORD"));
    let a = args!(l);
    if !a.args().len() != 1 {
        bad_args!(l, "specify exactly one word");
    }
    Ok(())
}

async fn do_withreq(mut l: Level<()>) -> Result<()> {
    l.reqopt("a", "first", "first letter", "LETTER");
    l.reqopt("", "second", "second letter", "LETTER");
    l.reqopt("c", "", "third letter", "LETTER");
    l.optopt("x", "", "optional extra letter", "LETTER");
    let a = args!(l);
    for opt in ["a", "second", "c"] {
        println!("{} = {}", opt, a.opts().opt_str(opt).unwrap());
    }
    for opt in ["x"] {
        println!("{} = {:?}", opt, a.opts().opt_str(opt));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut l = hiercmd::Level::new("trial", ());
    l.cmd("info", "get information", cmd!(do_info))?;
    l.cmda("thing", "th", "manage things", cmd!(do_thing))?;
    l.cmd("nothing", "do nothing", cmd!(do_nothing))?;
    l.cmd("check", "check to see if a word is valid", cmd!(do_check))?;
    l.cmd("withreq", "try required arguments", cmd!(do_withreq))?;

    l.optflag("x", "", "extend");

    let s = sel!(l);
    if s.opts().opt_present("x") {
        println!("eXtended!");
    }

    s.run().await
}
